//! Tracking engine: polls the pixels of the active profile and plays the triggered actions.

mod color;
mod executor;
mod machine;
mod poller;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;

use serde::Serialize;
use uuid::Uuid;

use crate::config::Config;
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::keyboard;
use executor::CancelledJobs;
use poller::{Command, Poller};

pub const STATUS_EVENT: &str = "engine://status";

/// Sent when the game window of the active profile gains or loses the foreground.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EngineStatus {
    pub profile_id: Uuid,
    pub game_active: bool,
}

pub struct Engine {
    commands: Sender<Command>,
    active_profile: Mutex<Option<Uuid>>,
    paused: Arc<AtomicBool>,
}

impl Engine {
    /// Starts the poll and executor threads; they idle until a profile is activated.
    pub fn start(emit: impl Fn(EngineStatus) + Send + 'static) -> Self {
        let (commands, command_receiver) = mpsc::channel();
        let (jobs, job_receiver) = mpsc::channel();
        let cancelled = Arc::new(CancelledJobs::default());
        let paused = Arc::new(AtomicBool::new(false));

        let done = commands.clone();
        let (executor_cancelled, executor_paused) = (cancelled.clone(), paused.clone());
        thread::spawn(move || {
            executor::run(
                job_receiver,
                &executor_cancelled,
                &executor_paused,
                |action| {
                    if let Err(error) = keyboard::run_action(action) {
                        eprintln!("Действие не выполнено: {error}");
                    }
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
            paused: paused.clone(),
            emit,
        };
        thread::spawn(move || poller.run());

        Self {
            commands,
            active_profile: Mutex::new(None),
            paused,
        }
    }

    /// Makes the profile the only one that runs, replacing a previously active one.
    pub fn activate(&self, store: &ConfigStore, profile_id: Uuid) -> AppResult<()> {
        // Reading under the store lock orders this with `config_changed` notifications.
        store.read(|config| {
            let profile = config
                .profiles
                .iter()
                .find(|profile| profile.id == profile_id)
                .ok_or(AppError::NotFound("Профиль не найден"))?;
            let mut active = self.active();
            if active.is_some() {
                self.send(Command::Deactivate);
            }
            *active = Some(profile_id);
            self.send(Command::Activate(profile.clone()));
            Ok(())
        })
    }

    /// Stops every tracker of the active profile without touching their saved run state.
    pub fn deactivate(&self) {
        if self.active().take().is_some() {
            self.send(Command::Deactivate);
        }
    }

    /// Called by `ConfigStore` after each committed mutation.
    pub fn config_changed(&self, config: &Config) {
        let mut active = self.active();
        let Some(profile_id) = *active else {
            return;
        };
        match config.profiles.iter().find(|profile| profile.id == profile_id) {
            Some(profile) => self.send(Command::Update(profile.clone())),
            None => {
                *active = None;
                self.send(Command::Deactivate);
            }
        }
    }

    /// Suspends polling and action playback until the guard is dropped.
    pub fn pause(&self) -> PauseGuard {
        self.paused.store(true, Ordering::SeqCst);
        PauseGuard(self.paused.clone())
    }

    fn send(&self, command: Command) {
        let _ = self.commands.send(command);
    }

    fn active(&self) -> MutexGuard<'_, Option<Uuid>> {
        self.active_profile
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

pub struct PauseGuard(Arc<AtomicBool>);

impl Drop for PauseGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
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
            active_profile: Mutex::new(None),
            paused: Arc::new(AtomicBool::new(false)),
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

        engine.activate(&store, ids[0]).unwrap();

        assert_eq!(sent(&receiver), [Sent::Activate(ids[0])]);
    }

    #[test]
    fn rejects_an_unknown_profile() {
        let (_dir, store, _) = store_with(&[]);
        let (engine, receiver) = engine();

        let result = engine.activate(&store, Uuid::new_v4());

        assert!(matches!(result, Err(AppError::NotFound(_))));
        assert!(sent(&receiver).is_empty());
    }

    #[test]
    fn activating_another_profile_deactivates_the_previous_one() {
        let (_dir, store, ids) = store_with(&["Diablo", "Path of Exile"]);
        let (engine, receiver) = engine();

        engine.activate(&store, ids[0]).unwrap();
        engine.activate(&store, ids[1]).unwrap();

        assert_eq!(
            sent(&receiver),
            [Sent::Activate(ids[0]), Sent::Deactivate, Sent::Activate(ids[1])]
        );
    }

    #[test]
    fn deactivates_once() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let (engine, receiver) = engine();
        engine.activate(&store, ids[0]).unwrap();

        engine.deactivate();
        engine.deactivate();

        assert_eq!(sent(&receiver), [Sent::Activate(ids[0]), Sent::Deactivate]);
    }

    #[test]
    fn forwards_changes_of_the_active_profile_only() {
        let (_dir, store, ids) = store_with(&["Diablo", "Path of Exile"]);
        let (engine, receiver) = engine();
        engine.activate(&store, ids[0]).unwrap();
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
        engine.activate(&store, ids[0]).unwrap();
        sent(&receiver);

        let mut config = store.read(Clone::clone);
        config.profiles.retain(|profile| profile.id != ids[0]);
        engine.config_changed(&config);
        engine.deactivate();

        assert_eq!(sent(&receiver), [Sent::Deactivate]);
    }

    /// Runs the real threads for two seconds; the debug log shows the tick period statistics:
    /// `cargo test -- --ignored prints_tick_period --nocapture`.
    #[test]
    #[ignore]
    fn prints_tick_period() {
        let (_dir, store, ids) = store_with(&["Diablo"]);
        let engine = Engine::start(|status| println!("{status:?}"));

        engine.activate(&store, ids[0]).unwrap();
        thread::sleep(std::time::Duration::from_secs(2));
        engine.deactivate();
    }

    #[test]
    fn pause_lasts_while_the_guard_is_alive() {
        let (engine, _receiver) = engine();

        let guard = engine.pause();
        assert!(engine.paused.load(Ordering::SeqCst));

        drop(guard);
        assert!(!engine.paused.load(Ordering::SeqCst));
    }
}
