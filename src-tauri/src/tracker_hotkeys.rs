//! Global shortcuts of running hotkey trackers. The plugin handler only records a press;
//! registration happens on the poll thread.

use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};

use tauri_plugin_global_shortcut::{Shortcut, ShortcutState};
use uuid::Uuid;

use crate::config::{Tracker, Trigger};
use crate::hotkey::{to_shortcut, ShortcutHost};
use crate::settings::{self, Accelerator};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredShortcut {
    pub tracker_id: Uuid,
    pub accelerator: Accelerator,
}

/// Shortcuts that should be registered right now. Empty unless a press could start a cycle.
pub fn desired_tracker_hotkeys(
    profile_active: bool,
    paused: bool,
    key_capture: u32,
    game_active: bool,
    trackers: &[Tracker],
) -> Vec<DesiredShortcut> {
    if !profile_active || paused || key_capture > 0 || !game_active {
        return Vec::new();
    }
    trackers
        .iter()
        .filter(|tracker| {
            tracker.trigger == Trigger::Hotkey && tracker.is_running && tracker.is_complete()
        })
        .filter_map(|tracker| {
            let combo = tracker.hotkey.as_ref()?;
            Some(DesiredShortcut {
                tracker_id: tracker.id,
                accelerator: settings::accelerator(combo)?,
            })
        })
        .collect()
}

#[derive(Clone)]
struct Binding {
    shortcut_id: u32,
    tracker_id: Uuid,
    held: bool,
}

#[derive(Default)]
pub struct TrackerShortcutTable {
    bindings: Vec<Binding>,
}

impl TrackerShortcutTable {
    /// Rising edge of a registered shortcut. A repeated hold returns `None`.
    pub fn on_shortcut(&mut self, shortcut: &Shortcut, state: ShortcutState) -> Option<Uuid> {
        let binding = self
            .bindings
            .iter_mut()
            .find(|binding| binding.shortcut_id == shortcut.id())?;
        match state {
            ShortcutState::Pressed if !binding.held => {
                binding.held = true;
                Some(binding.tracker_id)
            }
            ShortcutState::Released => {
                binding.held = false;
                None
            }
            _ => None,
        }
    }
}

#[derive(Clone)]
struct ActiveShortcut {
    tracker_id: Uuid,
    accelerator: Accelerator,
}

pub struct TrackerHotkeyRegistry {
    host: Box<dyn ShortcutHost + Send>,
    active: Vec<ActiveShortcut>,
}

impl TrackerHotkeyRegistry {
    pub fn new(host: Box<dyn ShortcutHost + Send>) -> Self {
        Self {
            host,
            active: Vec::new(),
        }
    }

    pub fn holds(&self, tracker_id: Uuid) -> bool {
        self.active.iter().any(|item| item.tracker_id == tracker_id)
    }

    /// Unregisters shortcuts that left the set and registers those that joined it.
    /// An OS refusal leaves that shortcut out of the active set.
    pub fn sync(&mut self, desired: &[DesiredShortcut], events: &Mutex<TrackerShortcutTable>) {
        let still_wanted = |active: &ActiveShortcut| {
            desired.iter().any(|item| {
                item.tracker_id == active.tracker_id && item.accelerator == active.accelerator
            })
        };
        let removed: Vec<_> = self
            .active
            .iter()
            .filter(|item| !still_wanted(item))
            .cloned()
            .collect();
        self.active.retain(still_wanted);
        self.publish(events);
        for item in &removed {
            let _ = self.host.unregister(&item.accelerator);
        }
        for item in desired {
            if self.holds(item.tracker_id) {
                continue;
            }
            if self.host.register(&item.accelerator).is_err() {
                continue;
            }
            self.active.push(ActiveShortcut {
                tracker_id: item.tracker_id,
                accelerator: item.accelerator.clone(),
            });
        }
        self.publish(events);
    }

    fn publish(&self, events: &Mutex<TrackerShortcutTable>) {
        let mut table = events.lock().unwrap_or_else(PoisonError::into_inner);
        let previous = std::mem::take(&mut table.bindings);
        table.bindings = self
            .active
            .iter()
            .filter_map(|item| {
                let shortcut_id = to_shortcut(&item.accelerator)?.id();
                let held = previous
                    .iter()
                    .find(|binding| binding.shortcut_id == shortcut_id)
                    .is_some_and(|binding| binding.held);
                Some(Binding {
                    shortcut_id,
                    tracker_id: item.tracker_id,
                    held,
                })
            })
            .collect();
    }
}

/// Tracker shortcuts plus the set the poller last asked for.
///
/// While a key sequence is injected, the set stays empty. `RegisterHotKey` would otherwise
/// take the same key from `SendInput` and deliver it back here instead of to the foreground window.
struct GateState {
    registry: TrackerHotkeyRegistry,
    desired: Vec<DesiredShortcut>,
    injecting: u32,
}

pub struct ShortcutGate {
    state: Mutex<GateState>,
    events: Arc<Mutex<TrackerShortcutTable>>,
}

impl ShortcutGate {
    pub fn new(
        host: Box<dyn ShortcutHost + Send>,
        events: Arc<Mutex<TrackerShortcutTable>>,
    ) -> Self {
        Self {
            state: Mutex::new(GateState {
                registry: TrackerHotkeyRegistry::new(host),
                desired: Vec::new(),
                injecting: 0,
            }),
            events,
        }
    }

    /// Applies the poller's set. A set asked for during injection is remembered and registered
    /// when the injection ends.
    pub fn sync(&self, desired: &[DesiredShortcut]) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.desired = desired.to_vec();
        let active = (state.injecting == 0).then(|| state.desired.clone());
        let registered = active.as_deref().unwrap_or(&[]);
        state.registry.sync(registered, &self.events);
    }

    pub fn suspend(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.injecting = state.injecting.saturating_add(1);
        state.registry.sync(&[], &self.events);
    }

    pub fn resume(&self) {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        state.injecting = state.injecting.saturating_sub(1);
        if state.injecting == 0 {
            let desired = state.desired.clone();
            state.registry.sync(&desired, &self.events);
        }
    }
}

/// What the plugin handler should do. Pause wins, and its closure is the only one called then.
pub enum ShortcutDispatch {
    TogglePause,
    Tracker(Uuid),
    Ignore,
}

pub fn dispatch_shortcut(
    mut pause_toggles: impl FnMut() -> bool,
    mut tracker_press: impl FnMut() -> Option<Uuid>,
) -> ShortcutDispatch {
    if pause_toggles() {
        ShortcutDispatch::TogglePause
    } else if let Some(tracker_id) = tracker_press() {
        ShortcutDispatch::Tracker(tracker_id)
    } else {
        ShortcutDispatch::Ignore
    }
}

/// Shared with the plugin handler. The poll thread owns the matching receiver and registry.
pub struct TrackerHotkeyLink {
    presses: Sender<Uuid>,
    events: Arc<Mutex<TrackerShortcutTable>>,
    key_capture: Arc<AtomicU32>,
}

pub struct TrackerHotkeyParts {
    pub link: TrackerHotkeyLink,
    pub presses: Receiver<Uuid>,
    pub events: Arc<Mutex<TrackerShortcutTable>>,
    pub key_capture: Arc<AtomicU32>,
}

impl TrackerHotkeyParts {
    pub fn open() -> Self {
        let (presses, receiver) = mpsc::channel();
        let events = Arc::new(Mutex::new(TrackerShortcutTable::default()));
        let key_capture = Arc::new(AtomicU32::new(0));
        Self {
            link: TrackerHotkeyLink {
                presses,
                events: Arc::clone(&events),
                key_capture: Arc::clone(&key_capture),
            },
            presses: receiver,
            events,
            key_capture,
        }
    }
}

impl TrackerHotkeyLink {
    pub fn take_press(&self, shortcut: &Shortcut, state: ShortcutState) -> Option<Uuid> {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .on_shortcut(shortcut, state)
    }

    pub fn send(&self, tracker_id: Uuid) {
        let _ = self.presses.send(tracker_id);
    }

    pub fn begin_capture(&self) {
        self.key_capture.fetch_add(1, Ordering::SeqCst);
    }

    pub fn end_capture(&self) {
        let _ = self
            .key_capture
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                count.checked_sub(1)
            });
    }
}

/// Host used when the engine starts without a window, so tests can boot the poll thread.
#[cfg(test)]
pub struct NoopShortcuts;

#[cfg(test)]
impl ShortcutHost for NoopShortcuts {
    fn register(&mut self, _accelerator: &Accelerator) -> Result<(), ()> {
        Ok(())
    }

    fn unregister(&mut self, _accelerator: &Accelerator) -> Result<(), ()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{Action, KeyCombo, KeySequenceAction, KeyStep, Modifier};
    use crate::config::Tracker;
    use crate::engine::EngineCore;
    use crate::hotkey::{to_shortcut, HotkeyBinding, ShortcutHost};
    use crate::settings;
    use rand::SeedableRng;
    use std::time::Duration;

    #[derive(Default)]
    struct FakeHost {
        fail: bool,
        registered: Vec<String>,
    }

    impl ShortcutHost for FakeHost {
        fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            if self.fail {
                self.fail = false;
                return Err(());
            }
            self.registered.push(accelerator.key.clone());
            Ok(())
        }

        fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            self.registered.retain(|key| key != &accelerator.key);
            Ok(())
        }
    }

    struct SharedHost(Arc<Mutex<FakeHost>>);

    impl ShortcutHost for SharedHost {
        fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            self.0.lock().unwrap().register(accelerator)
        }

        fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            self.0.lock().unwrap().unregister(accelerator)
        }
    }

    fn combo(modifiers: Vec<Modifier>, key: &str) -> KeyCombo {
        KeyCombo {
            modifiers,
            key: key.into(),
        }
    }

    fn running_hotkey(key: &str) -> Tracker {
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.trigger = Trigger::Hotkey;
        tracker.hotkey = Some(combo(vec![], key));
        tracker.action = Action::key_sequence(KeySequenceAction {
            steps: vec![KeyStep {
                id: Uuid::new_v4(),
                delay_before_ms: None,
                hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
                combo: combo(vec![], "KeyQ"),
            }],
        });
        tracker.pre_action_delay_ms = crate::config::DelayRange { min: 0, max: 0 };
        tracker.is_running = true;
        tracker
    }

    fn pixels_tracker() -> Tracker {
        let mut tracker = running_hotkey("F5");
        tracker.trigger = Trigger::Pixels;
        tracker.name = "Пиксели".into();
        tracker
    }

    #[test]
    fn the_set_is_empty_for_each_reason_and_otherwise_only_running_hotkey_trackers() {
        let ready = running_hotkey("F5");
        let mut stopped = running_hotkey("F6");
        stopped.is_running = false;
        let mut incomplete = running_hotkey("F7");
        incomplete.action = Action::default();
        let pixels = pixels_tracker();
        let trackers = [ready.clone(), stopped, incomplete, pixels];

        assert!(desired_tracker_hotkeys(false, false, 0, true, &trackers).is_empty());
        assert!(desired_tracker_hotkeys(true, true, 0, true, &trackers).is_empty());
        assert!(desired_tracker_hotkeys(true, false, 1, true, &trackers).is_empty());
        assert!(desired_tracker_hotkeys(true, false, 0, false, &trackers).is_empty());

        let desired = desired_tracker_hotkeys(true, false, 0, true, &trackers);
        assert_eq!(
            desired
                .iter()
                .map(|item| item.tracker_id)
                .collect::<Vec<_>>(),
            [ready.id]
        );
        assert_eq!(desired[0].accelerator.key, "F5");
    }

    #[test]
    fn focus_loss_and_pause_drop_the_shortcut_and_a_press_reaches_the_core_once() {
        let tracker = running_hotkey("F5");
        let host = Arc::new(Mutex::new(FakeHost::default()));
        let events = Mutex::new(TrackerShortcutTable::default());
        let mut registry = TrackerHotkeyRegistry::new(Box::new(SharedHost(Arc::clone(&host))));
        let desired = desired_tracker_hotkeys(true, false, 0, true, &[tracker.clone()]);

        registry.sync(&desired, &events);
        assert!(registry.holds(tracker.id));

        registry.sync(
            &desired_tracker_hotkeys(true, false, 0, false, &[tracker.clone()]),
            &events,
        );
        assert!(!registry.holds(tracker.id), "losing the game unregisters");

        registry.sync(&desired, &events);
        assert!(
            registry.holds(tracker.id),
            "focus registers the combo again"
        );

        registry.sync(
            &desired_tracker_hotkeys(true, true, 0, true, &[tracker.clone()]),
            &events,
        );
        assert!(!registry.holds(tracker.id), "pause unregisters");

        registry.sync(&desired, &events);
        host.lock().unwrap().fail = true;
        registry.sync(&[], &events);
        registry.sync(&desired, &events);
        assert!(!registry.holds(tracker.id), "an OS refusal is not active");

        registry.sync(&desired, &events);
        assert!(registry.holds(tracker.id));
        let shortcut = to_shortcut(&desired[0].accelerator).unwrap();
        let mut table = events.lock().unwrap();
        let first = table.on_shortcut(&shortcut, ShortcutState::Pressed);
        let repeat = table.on_shortcut(&shortcut, ShortcutState::Pressed);
        drop(table);

        assert_eq!(first, Some(tracker.id));
        assert_eq!(repeat, None);
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker.clone()]);
        let jobs = core.tick(
            Duration::ZERO,
            false,
            "game.exe",
            &[first.unwrap()],
            |_| None,
            &mut rand::rngs::StdRng::seed_from_u64(1),
        );
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].tracker_id, tracker.id);
    }

    #[test]
    fn a_pause_shortcut_does_not_start_a_tracker_and_capture_clears_the_set() {
        let tracker = running_hotkey("F5");
        let pause_combo = combo(vec![Modifier::Ctrl], "KeyP");
        let pause = HotkeyBinding::install(Some(pause_combo.clone()), FakeHost::default());
        let pause_shortcut = to_shortcut(&settings::accelerator(&pause_combo).unwrap()).unwrap();
        let events = Mutex::new(TrackerShortcutTable::default());
        let mut registry = TrackerHotkeyRegistry::new(Box::new(FakeHost::default()));
        let desired = desired_tracker_hotkeys(true, false, 0, true, &[tracker.clone()]);
        registry.sync(&desired, &events);
        let tracker_shortcut = to_shortcut(&desired[0].accelerator).unwrap();

        let mut tracker_checked = false;
        let dispatch = dispatch_shortcut(
            || pause.on_shortcut(&pause_shortcut, ShortcutState::Pressed),
            || {
                tracker_checked = true;
                events
                    .lock()
                    .unwrap()
                    .on_shortcut(&pause_shortcut, ShortcutState::Pressed)
            },
        );
        assert!(matches!(dispatch, ShortcutDispatch::TogglePause));
        assert!(!tracker_checked);

        let start = dispatch_shortcut(
            || false,
            || {
                events
                    .lock()
                    .unwrap()
                    .on_shortcut(&tracker_shortcut, ShortcutState::Pressed)
            },
        );
        assert!(matches!(start, ShortcutDispatch::Tracker(id) if id == tracker.id));

        registry.sync(
            &desired_tracker_hotkeys(true, false, 1, true, &[tracker.clone()]),
            &events,
        );
        assert!(!registry.holds(tracker.id));
        registry.sync(
            &desired_tracker_hotkeys(true, false, 0, true, &[tracker.clone()]),
            &events,
        );
        assert!(registry.holds(tracker.id));
    }

    #[test]
    fn injection_drops_the_hotkey_until_the_keys_are_sent() {
        let tracker = running_hotkey("Tab");
        let host = Arc::new(Mutex::new(FakeHost::default()));
        let events = Arc::new(Mutex::new(TrackerShortcutTable::default()));
        let gate = ShortcutGate::new(Box::new(SharedHost(Arc::clone(&host))), events);
        let desired = desired_tracker_hotkeys(true, false, 0, true, &[tracker]);

        gate.sync(&desired);
        assert_eq!(host.lock().unwrap().registered, ["Tab"]);

        gate.suspend();
        gate.sync(&desired);
        assert!(
            host.lock().unwrap().registered.is_empty(),
            "a poll during injection must not register the hotkey again"
        );

        gate.resume();
        assert_eq!(host.lock().unwrap().registered, ["Tab"]);
    }
}
