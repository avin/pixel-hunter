use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::config::Config;
use crate::error::AppResult;

const CONFIG_FILE: &str = "config.json";

type Listener = Box<dyn Fn(&Config) + Send + Sync>;

pub struct ConfigStore {
    path: PathBuf,
    config: Mutex<Config>,
    listener: OnceLock<Listener>,
}

pub struct LoadedConfigStore {
    pub store: ConfigStore,
    /// Where an unreadable `config.json` was moved before starting with an empty config.
    pub corrupt_backup: Option<PathBuf>,
}

impl ConfigStore {
    pub fn load(dir: &Path) -> io::Result<LoadedConfigStore> {
        fs::create_dir_all(dir)?;
        let path = dir.join(CONFIG_FILE);

        let (config, corrupt_backup) = match fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice::<Config>(&bytes) {
                Ok(config) => (config, None),
                Err(_) => (Config::default(), Some(move_aside(&path)?)),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => (Config::default(), None),
            Err(error) => return Err(error),
        };

        Ok(LoadedConfigStore {
            store: ConfigStore {
                path,
                config: Mutex::new(config),
                listener: OnceLock::new(),
            },
            corrupt_backup,
        })
    }

    pub fn read<R>(&self, read: impl FnOnce(&Config) -> R) -> R {
        read(&self.lock())
    }

    /// Applies `mutate` to a copy and commits it only after the copy is written to disk.
    pub fn update<R>(&self, mutate: impl FnOnce(&mut Config) -> AppResult<R>) -> AppResult<R> {
        let mut current = self.lock();
        let mut next = current.clone();
        let result = mutate(&mut next)?;
        write_atomically(&self.path, &next)?;
        *current = next;
        if let Some(listener) = self.listener.get() {
            listener(&current);
        }
        Ok(result)
    }

    /// `listener` gets every committed config, under the store lock and so in commit order;
    /// it must not call back into the store.
    pub fn subscribe(&self, listener: impl Fn(&Config) + Send + Sync + 'static) {
        if self.listener.set(Box::new(listener)).is_err() {
            panic!("ConfigStore supports a single listener");
        }
    }

    fn lock(&self) -> MutexGuard<'_, Config> {
        self.config.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

fn write_atomically(path: &Path, config: &Config) -> io::Result<()> {
    let json = serde_json::to_vec_pretty(config).map_err(io::Error::other)?;
    let temp_path = path.with_extension("json.tmp");

    let result = File::create(&temp_path)
        .and_then(|mut file| {
            file.write_all(&json)?;
            file.sync_all()
        })
        .and_then(|()| fs::rename(&temp_path, path));

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn move_aside(path: &Path) -> io::Result<PathBuf> {
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let backup = path.with_file_name(format!("config.corrupt-{timestamp}.json"));
    fs::rename(path, &backup)?;
    Ok(backup)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Profile;
    use crate::error::AppError;

    fn add_profile(store: &ConfigStore, name: &str) -> AppResult<()> {
        store.update(|config| {
            config
                .profiles
                .push(Profile::new(name.into(), "game.exe".into()));
            Ok(())
        })
    }

    fn profile_names(store: &ConfigStore) -> Vec<String> {
        store.read(|config| config.profiles.iter().map(|p| p.name.clone()).collect())
    }

    #[test]
    fn starts_empty_without_config_file() {
        let dir = tempfile::tempdir().unwrap();

        let loaded = ConfigStore::load(dir.path()).unwrap();

        assert_eq!(loaded.store.read(Clone::clone), Config::default());
        assert!(loaded.corrupt_backup.is_none());
    }

    #[test]
    fn reloads_saved_config() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;
        add_profile(&store, "Diablo").unwrap();

        let reloaded = ConfigStore::load(dir.path()).unwrap().store;

        assert_eq!(reloaded.read(Clone::clone), store.read(Clone::clone));
        assert!(!dir.path().join("config.json.tmp").exists());
    }

    #[test]
    fn loads_version_one_actions_without_sound_preserving_steps_and_run_state() {
        use crate::action::{Action, KeyCombo, KeySequenceAction, KeyStep, SoundSettings};
        use crate::config::{Pixel, Rgb, Tracker};
        use uuid::Uuid;
        let dir = tempfile::tempdir().unwrap();
        let mut tracker = Tracker::new("Health".into());
        tracker
            .pixels
            .push(Pixel::new(Uuid::new_v4(), 1, 2, Rgb { r: 1, g: 2, b: 3 }));
        tracker.action = Action::key_sequence(KeySequenceAction {
            steps: vec![KeyStep {
                id: Uuid::new_v4(),
                delay_before_ms: None,
                hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
                combo: KeyCombo {
                    modifiers: vec![],
                    key: "KeyQ".into(),
                },
            }],
        });
        tracker.is_running = true;
        let mut profile = Profile::new("Game".into(), "game.exe".into());
        profile.trackers.push(tracker.clone());
        let mut config = Config::default();
        config.profiles.push(profile);
        let mut json = serde_json::to_value(&config).unwrap();
        json["profiles"][0]["trackers"][0]["action"]
            .as_object_mut()
            .unwrap()
            .remove("sound");
        json["profiles"][0]["trackers"][0]["action"]["steps"][0]
            .as_object_mut()
            .unwrap()
            .remove("holdMs");
        fs::write(
            dir.path().join(CONFIG_FILE),
            serde_json::to_vec(&json).unwrap(),
        )
        .unwrap();
        let loaded = ConfigStore::load(dir.path()).unwrap();
        assert!(loaded.corrupt_backup.is_none());
        let restored = loaded
            .store
            .read(|config| config.profiles[0].trackers[0].clone());
        assert_eq!(restored, tracker);
        assert_eq!(restored.action.sound, SoundSettings::default());
        let crate::action::ActionKind::KeySequence(sequence) = &restored.action.kind else {
            unreachable!()
        };
        assert_eq!(
            sequence.steps[0].hold_ms,
            crate::action::DEFAULT_STEP_HOLD_MS
        );
        loaded.store.update(|_| Ok(())).unwrap();
        assert_eq!(
            ConfigStore::load(dir.path())
                .unwrap()
                .store
                .read(Clone::clone),
            config
        );
    }

    #[test]
    fn moves_corrupt_file_aside_and_starts_empty() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join(CONFIG_FILE), "{ not json").unwrap();

        let loaded = ConfigStore::load(dir.path()).unwrap();

        let backup = loaded
            .corrupt_backup
            .expect("corrupt file must be moved aside");
        let backup_name = backup.file_name().unwrap().to_string_lossy().into_owned();
        assert!(backup_name.starts_with("config.corrupt-") && backup_name.ends_with(".json"));
        assert_eq!(fs::read_to_string(&backup).unwrap(), "{ not json");
        assert!(!dir.path().join(CONFIG_FILE).exists());
        assert_eq!(loaded.store.read(Clone::clone), Config::default());
    }

    #[test]
    fn keeps_previous_state_when_write_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;
        add_profile(&store, "Diablo").unwrap();
        let saved = fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap();
        fs::create_dir(dir.path().join("config.json.tmp")).unwrap();

        let result = add_profile(&store, "Path of Exile");

        assert!(matches!(result, Err(AppError::Io(_))));
        assert_eq!(profile_names(&store), ["Diablo"]);
        assert_eq!(
            fs::read_to_string(dir.path().join(CONFIG_FILE)).unwrap(),
            saved
        );
        assert!(ConfigStore::load(dir.path())
            .unwrap()
            .corrupt_backup
            .is_none());
    }

    #[test]
    fn does_not_write_when_mutation_fails() {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;

        let result: AppResult<()> = store.update(|config| {
            config
                .profiles
                .push(Profile::new("Diablo".into(), "game.exe".into()));
            Err(AppError::Validation("invalid".into()))
        });

        assert!(result.is_err());
        assert!(profile_names(&store).is_empty());
        assert!(!dir.path().join(CONFIG_FILE).exists());
    }

    fn subscribed_store(dir: &Path) -> (ConfigStore, std::sync::Arc<Mutex<Vec<usize>>>) {
        let store = ConfigStore::load(dir).unwrap().store;
        let seen = std::sync::Arc::new(Mutex::new(Vec::new()));
        let recorder = seen.clone();
        store.subscribe(move |config| recorder.lock().unwrap().push(config.profiles.len()));
        (store, seen)
    }

    #[test]
    fn notifies_the_listener_with_each_committed_config() {
        let dir = tempfile::tempdir().unwrap();
        let (store, seen) = subscribed_store(dir.path());

        add_profile(&store, "Diablo").unwrap();
        add_profile(&store, "Path of Exile").unwrap();

        assert_eq!(*seen.lock().unwrap(), [1, 2]);
    }

    #[test]
    fn does_not_notify_about_failed_mutations() {
        let dir = tempfile::tempdir().unwrap();
        let (store, seen) = subscribed_store(dir.path());
        fs::create_dir(dir.path().join("config.json.tmp")).unwrap();

        let _ = add_profile(&store, "Diablo");
        let _: AppResult<()> = store.update(|_| Err(AppError::Validation("invalid".into())));

        assert!(seen.lock().unwrap().is_empty());
    }
}
