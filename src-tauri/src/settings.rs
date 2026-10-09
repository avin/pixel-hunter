//! Reading and writing the settings stored beside profiles.

use crate::action::KeyCombo;
use crate::config::{LastScreen, Settings};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::key_codes;

/// A physical combo the global shortcut can occupy. Unsupported codes are `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accelerator {
    pub modifiers: Vec<crate::action::Modifier>,
    pub key: String,
}

pub fn accelerator(combo: &KeyCombo) -> Option<Accelerator> {
    if key_codes::lookup(&combo.key).is_none() {
        return None;
    }
    let mut modifiers = combo.modifiers.clone();
    modifiers.sort();
    modifiers.dedup();
    Some(Accelerator {
        modifiers,
        key: combo.key.clone(),
    })
}

pub fn settings(store: &ConfigStore) -> Settings {
    store.read(|config| config.settings.clone())
}

pub fn set_last_screen(store: &ConfigStore, last_screen: LastScreen) -> AppResult<LastScreen> {
    store.update(|config| {
        config.settings.last_screen = last_screen.clone();
        Ok(last_screen)
    })
}

/// Writes a supported combo, or clears it when `pause_hotkey` is `None`.
/// An unsupported combo is rejected and the stored combo stays as it was.
pub fn set_pause_hotkey(
    store: &ConfigStore,
    pause_hotkey: Option<KeyCombo>,
) -> AppResult<Option<KeyCombo>> {
    if let Some(combo) = &pause_hotkey {
        if accelerator(combo).is_none() {
            return Err(AppError::Validation("Эта клавиша не поддерживается".into()));
        }
    }
    store.update(|config| {
        config.settings.pause_hotkey = pause_hotkey.clone();
        Ok(pause_hotkey)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Modifier;
    use crate::config::{Profile, Tracker};
    use std::fs;

    fn combo(key: &str) -> KeyCombo {
        KeyCombo {
            modifiers: vec![Modifier::Ctrl, Modifier::Shift],
            key: key.into(),
        }
    }

    fn store_with_running_tracker() -> (tempfile::TempDir, ConfigStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;
        store
            .update(|config| {
                let mut tracker = Tracker::new("Здоровье".into());
                tracker.is_running = true;
                let mut profile = Profile::new("Diablo".into(), "game.exe".into());
                profile.trackers.push(tracker);
                config.profiles.push(profile);
                config.settings.pause_hotkey = Some(combo("KeyQ"));
                config.settings.last_screen = LastScreen::Profile {
                    profile_id: config.profiles[0].id,
                };
                Ok(())
            })
            .unwrap();
        (dir, store)
    }

    #[test]
    fn translates_supported_combos_and_rejects_an_unsupported_code() {
        let translated = accelerator(&combo("KeyQ")).unwrap();
        assert_eq!(translated.key, "KeyQ");
        assert_eq!(translated.modifiers, vec![Modifier::Ctrl, Modifier::Shift]);
        assert!(accelerator(&combo("PrintScreen")).is_none());
        assert!(accelerator(&combo("MetaLeft")).is_none());
    }

    #[test]
    fn writing_settings_is_atomic_and_keeps_run_flags() {
        let (dir, store) = store_with_running_tracker();
        let profile_id = store.read(|config| config.profiles[0].id);
        let tracker_id = store.read(|config| config.profiles[0].trackers[0].id);
        let screen = LastScreen::Editor {
            profile_id,
            tracker_id,
        };

        set_last_screen(&store, screen.clone()).unwrap();
        set_pause_hotkey(&store, Some(combo("F5"))).unwrap();

        let reloaded = ConfigStore::load(dir.path()).unwrap().store;
        let settings = settings(&reloaded);
        assert_eq!(settings.last_screen, screen);
        assert_eq!(settings.pause_hotkey, Some(combo("F5")));
        assert!(reloaded.read(|config| config.profiles[0].trackers[0].is_running));
        assert!(!dir.path().join("config.json.tmp").exists());
    }

    #[test]
    fn a_failed_write_keeps_the_previous_screen() {
        let (dir, store) = store_with_running_tracker();
        let before = settings(&store);
        let profile_id = store.read(|config| config.profiles[0].id);
        fs::create_dir(dir.path().join("config.json.tmp")).unwrap();

        let result = set_last_screen(
            &store,
            LastScreen::Editor {
                profile_id,
                tracker_id: uuid::Uuid::new_v4(),
            },
        );

        assert!(matches!(result, Err(AppError::Io(_))));
        assert_eq!(settings(&store), before);
        assert_eq!(
            settings(&ConfigStore::load(dir.path()).unwrap().store),
            before
        );
    }

    #[test]
    fn clearing_the_hotkey_removes_it() {
        let (_dir, store) = store_with_running_tracker();

        assert_eq!(set_pause_hotkey(&store, None).unwrap(), None);
        assert_eq!(settings(&store).pause_hotkey, None);
    }

    #[test]
    fn an_unsupported_combo_does_not_replace_the_saved_one() {
        let (dir, store) = store_with_running_tracker();
        let saved = fs::read(dir.path().join("config.json")).unwrap();

        let result = set_pause_hotkey(&store, Some(combo("PrintScreen")));

        assert!(matches!(result, Err(AppError::Validation(_))));
        assert_eq!(settings(&store).pause_hotkey, Some(combo("KeyQ")));
        assert_eq!(fs::read(dir.path().join("config.json")).unwrap(), saved);
    }
}
