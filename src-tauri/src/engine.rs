//! Tracking engine: polls the pixels of the active profile and plays the triggered actions.

mod color;
mod executor;
mod machine;
mod poller;

#[cfg(test)]
pub(crate) use machine::EngineCore;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use serde::Serialize;
use uuid::Uuid;

use crate::action::{Action, ActionKind};
use crate::action_sound::AudioService;
use crate::config::Config;
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::hotkey::{ShortcutHost, ShortcutMute};
use crate::keyboard;
use crate::network;
#[cfg(test)]
use crate::tracker_hotkeys::NoopShortcuts;
use crate::tracker_hotkeys::{ShortcutGate, TrackerHotkeyLink, TrackerHotkeyParts};

#[cfg(test)]
struct NoShortcutMute;

#[cfg(test)]
impl ShortcutMute for NoShortcutMute {
    fn mute(&self) {}
    fn unmute(&self) {}
}
use executor::CancelledJobs;
use machine::Job;
use poller::{Command, Poller};

pub const ICON_STATUS_EVENT: &str = "engine://icon-status";
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct IconStatus {
    pub profile_id: Uuid,
    pub tracker_id: Uuid,
    pub icon_id: Uuid,
    pub error: Option<String>,
}

pub const STATUS_EVENT: &str = "engine://status";
pub const PAUSE_EVENT: &str = "work://pause";

/// Payload of [`PAUSE_EVENT`] and the toggle command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkPause {
    pub paused: bool,
}

/// Capture pause and the user's pause, published as one signal for the poller and executor.
struct PauseControl {
    capture: AtomicBool,
    user: AtomicBool,
    /// Serializes updates so one pause cannot publish over the other's newer value.
    lock: Mutex<()>,
    signal: Arc<AtomicBool>,
}

impl PauseControl {
    fn new() -> Self {
        Self {
            capture: AtomicBool::new(false),
            user: AtomicBool::new(false),
            lock: Mutex::new(()),
            signal: Arc::new(AtomicBool::new(false)),
        }
    }

    fn publish(&self) {
        let paused = self.capture.load(Ordering::SeqCst) || self.user.load(Ordering::SeqCst);
        self.signal.store(paused, Ordering::SeqCst);
    }

    fn set_capture(&self, paused: bool) {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        self.capture.store(paused, Ordering::SeqCst);
        self.publish();
    }

    /// Flips the user pause and returns the new value.
    fn toggle_user(&self) -> bool {
        let _guard = self.lock.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = self.user.fetch_xor(true, Ordering::SeqCst);
        self.publish();
        !previous
    }
}

/// Sent when the game window of the active profile gains or loses the foreground.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub profile_id: Uuid,
    pub game_active: bool,
}

struct Activation {
    profile_id: Option<Uuid>,
    /// Identifies the frontend request that started the current activation.
    epoch: u64,
}

pub struct Engine {
    commands: Sender<Command>,
    activation: Mutex<Activation>,
    pause: Arc<PauseControl>,
}

impl Engine {
    /// Starts the poll and executor threads; they idle until a profile is activated.
    #[cfg(test)]
    pub fn start(emit: impl Fn(EngineStatus) + Send + 'static) -> Self {
        Self::start_with_audio(
            AudioService::start(),
            emit,
            Box::new(NoopShortcuts),
            Arc::new(NoShortcutMute),
        )
        .0
    }

    #[cfg(test)]
    pub fn start_with_audio(
        audio: AudioService,
        emit: impl Fn(EngineStatus) + Send + 'static,
        shortcuts: Box<dyn ShortcutHost + Send>,
        muted: Arc<dyn ShortcutMute>,
    ) -> (Self, TrackerHotkeyLink, Arc<ShortcutGate>) {
        Self::start_with_visuals(
            audio,
            emit,
            shortcuts,
            muted,
            std::path::PathBuf::new(),
            |_| {},
        )
    }

    pub fn start_with_visuals(
        audio: AudioService,
        emit: impl Fn(EngineStatus) + Send + 'static,
        shortcuts: Box<dyn ShortcutHost + Send>,
        muted: Arc<dyn ShortcutMute>,
        screenshot_root: std::path::PathBuf,
        emit_icon: impl Fn(IconStatus) + Send + 'static,
    ) -> (Self, TrackerHotkeyLink, Arc<ShortcutGate>) {
        let (commands, command_receiver) = mpsc::channel();
        let hotkeys = TrackerHotkeyParts::open();
        let gate = Arc::new(ShortcutGate::new(shortcuts, Arc::clone(&hotkeys.events)));
        let (jobs, job_receiver) = mpsc::channel();
        let cancelled = Arc::new(CancelledJobs::default());
        let pause = Arc::new(PauseControl::new());

        let done = commands.clone();
        let (executor_cancelled, executor_paused) = (cancelled.clone(), pause.signal.clone());
        let playback_gate = Arc::clone(&gate);
        thread::spawn(move || {
            executor::run(
                job_receiver,
                &executor_cancelled,
                &executor_paused,
                |job| {
                    play_action(
                        job,
                        |sound| audio.play_runtime(sound),
                        |job| {
                            perform_action(
                                &job.action,
                                &job.process_name,
                                &playback_gate,
                                muted.as_ref(),
                            )
                        },
                    );
                },
                thread::sleep,
                |ticket| {
                    let _ = done.send(Command::ActionDone(ticket));
                },
            )
        });

        let poller = Poller {
            commands: command_receiver,
            jobs,
            cancelled,
            paused: pause.signal.clone(),
            emit,
            emit_icon,
            screenshot_root,
            gate: Arc::clone(&gate),
            presses: hotkeys.presses,
            key_capture: hotkeys.key_capture,
        };
        thread::spawn(move || poller.run());

        (
            Self {
                commands,
                activation: Mutex::new(Activation {
                    profile_id: None,
                    epoch: 0,
                }),
                pause,
            },
            hotkeys.link,
            gate,
        )
    }

    /// Makes the profile the only one that runs, replacing a previously active one.
    /// `epoch` lets a newer request ignore a stop that was issued for an older one.
    pub fn activate(&self, store: &ConfigStore, profile_id: Uuid, epoch: u64) -> AppResult<()> {
        // Reading under the store lock orders this with `config_changed` notifications.
        store.read(|config| {
            let profile = config
                .profiles
                .iter()
                .find(|profile| profile.id == profile_id)
                .ok_or(AppError::NotFound("Профиль не найден"))?;
            let mut activation = self.activation();
            activation.epoch = epoch;
            activation.profile_id = Some(profile_id);
            drop(activation);
            self.send(Command::Activate(profile.clone()));
            Ok(())
        })
    }

    /// Stops the profile started by `epoch`. A stop for an older request does nothing.
    pub fn deactivate(&self, epoch: u64) {
        let mut activation = self.activation();
        if activation.epoch != epoch {
            return;
        }
        activation.epoch = 0;
        if activation.profile_id.take().is_some() {
            drop(activation);
            self.send(Command::Deactivate);
        }
    }

    /// Called by `ConfigStore` after each committed mutation.
    pub fn config_changed(&self, config: &Config) {
        let mut activation = self.activation();
        let Some(profile_id) = activation.profile_id else {
            return;
        };
        match config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
        {
            Some(profile) => {
                drop(activation);
                self.send(Command::Update(profile.clone()));
            }
            None => {
                activation.epoch = 0;
                activation.profile_id = None;
                drop(activation);
                self.send(Command::Deactivate);
            }
        }
    }

    /// Suspends polling and new actions until the guard is dropped. Does not clear a user pause.
    pub fn pause(&self) -> PauseGuard {
        self.pause.set_capture(true);
        PauseGuard(self.pause.clone())
    }

    /// Session pause. Not written to the config and independent of capture pause.
    pub fn toggle_user_pause(&self) -> bool {
        self.pause.toggle_user()
    }

    #[cfg(test)]
    fn is_user_paused(&self) -> bool {
        self.pause.user.load(Ordering::SeqCst)
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn activation(&self) -> MutexGuard<'_, Activation> {
        self.activation
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Plays a key sequence, or disconnects TCP using the process name stored on the job.
/// A key sequence lifts every registered shortcut first: Windows `RegisterHotKey` would
/// otherwise swallow an injected key that matches the condition hotkey.
pub(crate) fn perform_action(
    action: &Action,
    process_name: &str,
    gate: &ShortcutGate,
    muted: &dyn ShortcutMute,
) -> AppResult<()> {
    match &action.kind {
        ActionKind::KeySequence(sequence) => {
            with_hotkeys_lifted(gate, muted, || keyboard::run_action(sequence))
        }
        ActionKind::NetworkDisconnect => {
            network::disconnect(process_name);
            Ok(())
        }
    }
}

fn with_hotkeys_lifted<T>(
    gate: &ShortcutGate,
    muted: &dyn ShortcutMute,
    body: impl FnOnce() -> T,
) -> T {
    gate.suspend();
    muted.mute();
    let restore = HotkeyRestore { gate, muted };
    let result = body();
    drop(restore);
    result
}

struct HotkeyRestore<'a> {
    gate: &'a ShortcutGate,
    muted: &'a dyn ShortcutMute,
}

impl Drop for HotkeyRestore<'_> {
    fn drop(&mut self) {
        self.muted.unmute();
        self.gate.resume();
    }
}

/// Called only after executor cancellation and capture-pause checks. Audio submission
/// does not wait for playback, so action completion still controls ActionDone.
fn play_action(
    job: &Job,
    mut sound: impl FnMut(&crate::action::SoundSettings),
    perform: impl FnOnce(&Job) -> AppResult<()>,
) {
    if job.action.sound.id.is_some() && job.action.sound.volume_percent > 0 {
        sound(&job.action.sound);
    }
    if let Err(error) = perform(job) {
        eprintln!("Действие не выполнено: {error}");
    }
}

pub struct PauseGuard(Arc<PauseControl>);

impl Drop for PauseGuard {
    fn drop(&mut self) {
        self.0.set_capture(false);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use std::sync::mpsc::Receiver;
    use tempfile::TempDir;

    fn engine() -> (Engine, Receiver<Command>) {
        let (commands, receiver) = mpsc::channel();
        let engine = Engine {
            commands,
            activation: Mutex::new(Activation {
                profile_id: None,
                epoch: 0,
            }),
            pause: Arc::new(PauseControl::new()),
        };
        (engine, receiver)
    }

    #[derive(Debug, PartialEq)]
    enum Sent {
        Activate(Uuid),
        Update(Uuid),
        Deactivate,
    }

    fn sent(receiver: &Receiver<Command>) -> Vec<Sent> {
        receiver
            .try_iter()
            .map(|command| match command {
                Command::Activate(profile) => Sent::Activate(profile.id),
                Command::Update(profile) => Sent::Update(profile.id),
                Command::Deactivate => Sent::Deactivate,
                Command::ActionDone(_) => unreachable!(),
            })
            .collect()
    }

    fn store_with(names: &[&str]) -> (TempDir, ConfigStore, Vec<Uuid>) {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;
        let ids = names
            .iter()
            .map(|name| {
                store
                    .update(|config| {
                        let profile = Profile::new((*name).into(), "game.exe".into());
                        config.profiles.push(profile.clone());
                        Ok(profile.id)
                    })
                    .unwrap()
            })
            .collect();
        (dir, store, ids)
    }

    #[test]
    fn activates_an_existing_profile() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let (engine, receiver) = engine();

        engine.activate(&store, ids[0], 1).unwrap();

        assert_eq!(sent(&receiver), [Sent::Activate(ids[0])]);
    }

    #[test]
    fn rejects_an_unknown_profile() {
        let (_dir, store, _) = store_with(&[]);
        let (engine, receiver) = engine();

        let result = engine.activate(&store, Uuid::new_v4(), 1);

        assert!(matches!(result, Err(AppError::NotFound(_))));
        assert!(sent(&receiver).is_empty());
    }

    #[test]
    fn activating_another_profile_deactivates_the_previous_one() {
        let (_dir, store, ids) = store_with(&["Diablo", "Path of Exile"]);
        let (engine, receiver) = engine();

        engine.activate(&store, ids[0], 1).unwrap();
        engine.activate(&store, ids[1], 2).unwrap();

        assert_eq!(
            sent(&receiver),
            [Sent::Activate(ids[0]), Sent::Activate(ids[1])]
        );
    }

    #[test]
    fn a_stale_stop_does_not_cancel_the_newer_activation() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let (engine, receiver) = engine();

        engine.activate(&store, ids[0], 1).unwrap();
        engine.activate(&store, ids[0], 2).unwrap();
        engine.deactivate(1);

        assert_eq!(
            sent(&receiver),
            [Sent::Activate(ids[0]), Sent::Activate(ids[0])]
        );
    }

    #[test]
    fn deactivates_once() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let (engine, receiver) = engine();
        engine.activate(&store, ids[0], 1).unwrap();

        engine.deactivate(1);
        engine.deactivate(1);

        assert_eq!(sent(&receiver), [Sent::Activate(ids[0]), Sent::Deactivate]);
    }

    #[test]
    fn forwards_changes_of_the_active_profile_only() {
        let (_dir, store, ids) = store_with(&["Diablo", "Path of Exile"]);
        let (engine, receiver) = engine();
        engine.activate(&store, ids[0], 1).unwrap();
        sent(&receiver);

        let config = store.read(Clone::clone);
        engine.config_changed(&config);

        assert_eq!(sent(&receiver), [Sent::Update(ids[0])]);
    }

    #[test]
    fn ignores_changes_while_inactive() {
        let (_dir, store, _) = store_with(&["Diablo"]);
        let (engine, receiver) = engine();

        engine.config_changed(&store.read(Clone::clone));

        assert!(sent(&receiver).is_empty());
    }

    #[test]
    fn deleting_the_active_profile_deactivates_the_engine() {
        let (_dir, store, ids) = store_with(&["Diablo", "Path of Exile"]);
        let (engine, receiver) = engine();
        engine.activate(&store, ids[0], 1).unwrap();
        sent(&receiver);

        let mut config = store.read(Clone::clone);
        config.profiles.retain(|profile| profile.id != ids[0]);
        engine.config_changed(&config);
        engine.deactivate(1);

        assert_eq!(sent(&receiver), [Sent::Deactivate]);
    }

    /// Runs the real threads for two seconds; the debug log shows the tick period statistics:
    /// `cargo test -- --ignored prints_tick_period --nocapture`.
    #[test]
    #[ignore]
    fn prints_tick_period() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let engine = Engine::start(|status| println!("{status:?}"));

        engine.activate(&store, ids[0], 1).unwrap();
        thread::sleep(std::time::Duration::from_secs(2));
        engine.deactivate(1);
    }

    #[test]
    fn pause_lasts_while_the_guard_is_alive() {
        let (engine, _receiver) = engine();

        let guard = engine.pause();
        assert!(engine.pause.signal.load(Ordering::SeqCst));

        drop(guard);
        assert!(!engine.pause.signal.load(Ordering::SeqCst));
    }

    #[test]
    fn dropping_capture_pause_does_not_clear_user_pause() {
        let (engine, _receiver) = engine();
        assert!(engine.toggle_user_pause());

        let guard = engine.pause();
        assert!(engine.pause.signal.load(Ordering::SeqCst));
        drop(guard);

        assert!(engine.is_user_paused());
        assert!(engine.pause.signal.load(Ordering::SeqCst));

        assert!(!engine.toggle_user_pause());
        assert!(!engine.pause.signal.load(Ordering::SeqCst));
    }

    #[test]
    fn user_pause_toggles_without_writing_run_flags_and_is_off_after_reload() {
        let (dir, store, _ids) = store_with(&["Diablo"]);
        store
            .update(|config| {
                let mut tracker = crate::config::Tracker::new("Здоровье".into());
                tracker.is_running = true;
                config.profiles[0].trackers.push(tracker);
                Ok(())
            })
            .unwrap();
        let before = std::fs::read(dir.path().join("config.json")).unwrap();
        let (running, _receiver) = engine();

        assert!(running.toggle_user_pause());
        assert!(!running.toggle_user_pause());
        assert_eq!(
            std::fs::read(dir.path().join("config.json")).unwrap(),
            before
        );

        let reloaded = ConfigStore::load(dir.path()).unwrap().store;
        assert!(reloaded.read(|config| config.profiles[0].trackers[0].is_running));
        let (fresh, _receiver) = engine();
        assert!(!fresh.is_user_paused());
        assert!(!fresh.pause.signal.load(Ordering::SeqCst));
    }

    #[test]
    fn an_injected_key_is_sent_while_its_hotkey_is_unregistered() {
        use crate::action::{KeyCombo, Modifier};
        use crate::config::Tracker;
        use crate::hotkey::ShortcutHost;
        use crate::settings::Accelerator;
        use crate::tracker_hotkeys::desired_tracker_hotkeys;
        use std::sync::Mutex as LogMutex;

        #[derive(Default)]
        struct Host {
            registered: Vec<String>,
        }

        impl ShortcutHost for Host {
            fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
                self.registered.push(accelerator.key.clone());
                Ok(())
            }

            fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
                self.registered.retain(|key| key != &accelerator.key);
                Ok(())
            }
        }

        struct SharedHost(Arc<Mutex<Host>>);

        impl ShortcutHost for SharedHost {
            fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
                self.0.lock().unwrap().register(accelerator)
            }

            fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
                self.0.lock().unwrap().unregister(accelerator)
            }
        }

        struct PauseLog(LogMutex<Vec<&'static str>>);

        impl ShortcutMute for PauseLog {
            fn mute(&self) {
                self.0.lock().unwrap().push("mute");
            }

            fn unmute(&self) {
                self.0.lock().unwrap().push("unmute");
            }
        }

        let host = Arc::new(Mutex::new(Host::default()));
        let gate = ShortcutGate::new(
            Box::new(SharedHost(Arc::clone(&host))),
            Arc::new(Mutex::new(
                crate::tracker_hotkeys::TrackerShortcutTable::default(),
            )),
        );
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.trigger = crate::config::Trigger::Hotkey;
        tracker.hotkey = Some(KeyCombo {
            modifiers: vec![],
            key: "Tab".into(),
        });
        tracker.action = crate::action::Action::key_sequence(crate::action::KeySequenceAction {
            steps: vec![crate::action::KeyStep {
                id: Uuid::new_v4(),
                delay_before_ms: None,
                hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
                combo: KeyCombo {
                    modifiers: vec![Modifier::Ctrl],
                    key: "Tab".into(),
                },
            }],
        });
        tracker.is_running = true;
        let desired = desired_tracker_hotkeys(true, false, 0, true, &[tracker]);
        gate.sync(&desired);
        let pause = PauseLog(LogMutex::new(Vec::new()));

        let sent = with_hotkeys_lifted(&gate, &pause, || {
            gate.sync(&desired);
            host.lock().unwrap().registered.is_empty()
        });

        assert!(sent);
        assert_eq!(pause.0.lock().unwrap().as_slice(), ["mute", "unmute"]);
        assert_eq!(host.lock().unwrap().registered, ["Tab"]);
    }
}
