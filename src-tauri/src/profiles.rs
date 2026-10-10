use serde::Deserialize;
use uuid::Uuid;

use crate::action::{Action, KeyCombo};
use crate::config::{
    Config, DelayRange, Icon, IconCondition, Pixel, PixelCondition, Profile, Tracker, Trigger,
};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::screenshots::Screenshots;
use crate::settings;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInput {
    pub name: String,
    pub process_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackerInput {
    pub name: String,
    pub pre_action_delay_ms: DelayRange,
    pub resume_delay_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PixelInput {
    pub condition: PixelCondition,
    pub tolerance_percent: u8,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IconInput {
    pub condition: IconCondition,
    pub similarity_percent: u8,
}

pub fn list_profiles(store: &ConfigStore) -> Vec<Profile> {
    store.read(|config| config.profiles.clone())
}

pub fn create_profile(store: &ConfigStore, input: ProfileInput) -> AppResult<Profile> {
    let profile = Profile::new(input.name.trim().into(), input.process_name.trim().into());
    profile.validate()?;
    store.update(|config| {
        config.profiles.push(profile.clone());
        Ok(profile)
    })
}

pub fn update_profile(store: &ConfigStore, id: Uuid, input: ProfileInput) -> AppResult<Profile> {
    store.update(|config| {
        let profile = find_profile(config, id)?;
        let mut updated = profile.clone();
        updated.name = input.name.trim().into();
        updated.process_name = input.process_name.trim().into();
        updated.validate()?;
        *profile = updated.clone();
        Ok(updated)
    })
}

pub fn delete_profile(store: &ConfigStore, screenshots: &Screenshots, id: Uuid) -> AppResult<()> {
    store.update(|config| {
        let index = config
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or(PROFILE_NOT_FOUND)?;
        config.profiles.remove(index);
        Ok(())
    })?;
    screenshots.remove_profile(id);
    Ok(())
}

pub fn create_tracker(store: &ConfigStore, profile_id: Uuid, name: String) -> AppResult<Tracker> {
    let tracker = Tracker::new(name.trim().into());
    tracker.validate()?;
    store.update(|config| {
        find_profile(config, profile_id)?
            .trackers
            .push(tracker.clone());
        Ok(tracker)
    })
}

pub fn update_tracker(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    input: TrackerInput,
) -> AppResult<Tracker> {
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        let mut updated = tracker.clone();
        updated.name = input.name.trim().into();
        updated.pre_action_delay_ms = input.pre_action_delay_ms;
        updated.resume_delay_ms = input.resume_delay_ms;
        updated.validate()?;
        *tracker = updated.clone();
        Ok(updated)
    })
}

/// Replaces the whole action; returns it normalized as stored.
pub fn update_action(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    mut action: Action,
) -> AppResult<Action> {
    action.normalize();
    action.validate()?;
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        tracker.action = action.clone();
        tracker.stop_if_incomplete();
        Ok(action)
    })
}

/// Switches the condition. Pixels and the saved combo stay where they are.
pub fn set_trigger(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    trigger: Trigger,
) -> AppResult<Tracker> {
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        tracker.trigger = trigger;
        tracker.stop_if_incomplete();
        Ok(tracker.clone())
    })
}

/// Writes one combo, or clears it. A combo already used by the pause hotkey or by another
/// tracker of this profile is rejected and the previous value stays.
pub fn set_hotkey(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    hotkey: Option<KeyCombo>,
) -> AppResult<Tracker> {
    store.update(|config| {
        let normalized = match hotkey {
            None => None,
            Some(combo) => Some(normalize_hotkey(config, profile_id, tracker_id, combo)?),
        };
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        tracker.hotkey = normalized;
        tracker.stop_if_incomplete();
        Ok(tracker.clone())
    })
}

fn normalize_hotkey(
    config: &Config,
    profile_id: Uuid,
    tracker_id: Uuid,
    combo: KeyCombo,
) -> AppResult<KeyCombo> {
    let Some(accelerator) = settings::accelerator(&combo) else {
        return Err(AppError::Validation("Эта клавиша не поддерживается".into()));
    };
    let normalized = KeyCombo {
        modifiers: accelerator.modifiers,
        key: accelerator.key,
    };
    if config
        .settings
        .pause_hotkey
        .as_ref()
        .is_some_and(|pause| settings::same_combo(pause, &normalized))
    {
        return Err(AppError::Validation(
            "Это сочетание уже назначено паузе".into(),
        ));
    }
    let taken = config
        .profiles
        .iter()
        .find(|profile| profile.id == profile_id)
        .is_some_and(|profile| {
            profile.trackers.iter().any(|other| {
                other.id != tracker_id
                    && other
                        .hotkey
                        .as_ref()
                        .is_some_and(|hotkey| settings::same_combo(hotkey, &normalized))
            })
        });
    if taken {
        return Err(AppError::Validation(
            "Это сочетание уже назначено другому отслеживанию".into(),
        ));
    }
    Ok(normalized)
}

/// Saves whether the tracker should run; an incomplete tracker cannot be started.
pub fn set_tracker_running(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    is_running: bool,
) -> AppResult<Tracker> {
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        if is_running {
            if let Some(missing) = tracker.missing_part() {
                return Err(AppError::Validation(missing.message().into()));
            }
        }
        tracker.is_running = is_running;
        Ok(tracker.clone())
    })
}

pub fn get_action(store: &ConfigStore, profile_id: Uuid, tracker_id: Uuid) -> AppResult<Action> {
    store.read(|config| {
        config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PROFILE_NOT_FOUND)?
            .trackers
            .iter()
            .find(|tracker| tracker.id == tracker_id)
            .map(|tracker| tracker.action.clone())
            .ok_or(TRACKER_NOT_FOUND)
    })
}

pub fn delete_tracker(
    store: &ConfigStore,
    screenshots: &Screenshots,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    let removed = store.update(|config| {
        let trackers = &mut find_profile(config, profile_id)?.trackers;
        let index = trackers
            .iter()
            .position(|tracker| tracker.id == tracker_id)
            .ok_or(TRACKER_NOT_FOUND)?;
        Ok(trackers.remove(index))
    })?;
    for pixel in removed.pixels {
        screenshots.remove_pixel(profile_id, pixel.id);
    }
    for icon in removed.icons {
        screenshots.remove_icon(profile_id, icon.id);
    }
    Ok(())
}

pub fn add_pixel(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel: Pixel,
) -> AppResult<Pixel> {
    pixel.validate()?;
    store.update(|config| {
        find_tracker(config, profile_id, tracker_id)?
            .pixels
            .push(pixel.clone());
        Ok(pixel)
    })
}

pub fn update_pixel(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
    input: PixelInput,
) -> AppResult<Pixel> {
    store.update(|config| {
        let pixel = find_tracker(config, profile_id, tracker_id)?
            .pixels
            .iter_mut()
            .find(|pixel| pixel.id == pixel_id)
            .ok_or(PIXEL_NOT_FOUND)?;
        let mut updated = pixel.clone();
        updated.condition = input.condition;
        updated.tolerance_percent = input.tolerance_percent;
        updated.validate()?;
        *pixel = updated.clone();
        Ok(updated)
    })
}

pub fn delete_pixel(
    store: &ConfigStore,
    screenshots: &Screenshots,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        let index = tracker
            .pixels
            .iter()
            .position(|pixel| pixel.id == pixel_id)
            .ok_or(PIXEL_NOT_FOUND)?;
        tracker.pixels.remove(index);
        tracker.stop_if_incomplete();
        Ok(())
    })?;
    screenshots.remove_pixel(profile_id, pixel_id);
    Ok(())
}

pub fn get_pixel(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<Pixel> {
    store.read(|config| {
        config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PROFILE_NOT_FOUND)?
            .trackers
            .iter()
            .find(|tracker| tracker.id == tracker_id)
            .ok_or(TRACKER_NOT_FOUND)?
            .pixels
            .iter()
            .find(|pixel| pixel.id == pixel_id)
            .cloned()
            .ok_or(PIXEL_NOT_FOUND)
    })
}

/// Process name of the profile that owns the tracker.
pub fn profile_process_name(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<String> {
    store.read(|config| {
        let profile = config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PROFILE_NOT_FOUND)?;
        if !profile
            .trackers
            .iter()
            .any(|tracker| tracker.id == tracker_id)
        {
            return Err(TRACKER_NOT_FOUND);
        }
        Ok(profile.process_name.clone())
    })
}

const PROFILE_NOT_FOUND: AppError = AppError::NotFound("Профиль не найден");
const TRACKER_NOT_FOUND: AppError = AppError::NotFound("Отслеживание не найдено");
const PIXEL_NOT_FOUND: AppError = AppError::NotFound("Пиксель не найден");

pub fn add_icon(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    icon: Icon,
) -> AppResult<Icon> {
    icon.validate()?;
    store.update(|config| {
        find_tracker(config, profile_id, tracker_id)?
            .icons
            .push(icon.clone());
        Ok(icon)
    })
}

pub fn get_icon(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    icon_id: Uuid,
) -> AppResult<Icon> {
    store.read(|config| {
        config
            .profiles
            .iter()
            .find(|p| p.id == profile_id)
            .ok_or(PROFILE_NOT_FOUND)?
            .trackers
            .iter()
            .find(|t| t.id == tracker_id)
            .ok_or(TRACKER_NOT_FOUND)?
            .icons
            .iter()
            .find(|i| i.id == icon_id)
            .cloned()
            .ok_or(AppError::NotFound("Иконка не найдена"))
    })
}

pub fn update_icon(
    store: &ConfigStore,
    profile_id: Uuid,
    tracker_id: Uuid,
    icon_id: Uuid,
    input: IconInput,
) -> AppResult<Icon> {
    store.update(|config| {
        let icon = find_tracker(config, profile_id, tracker_id)?
            .icons
            .iter_mut()
            .find(|i| i.id == icon_id)
            .ok_or(AppError::NotFound("Иконка не найдена"))?;
        icon.condition = input.condition;
        icon.similarity_percent = input.similarity_percent;
        icon.validate()?;
        Ok(icon.clone())
    })
}

pub fn delete_icon(
    store: &ConfigStore,
    screenshots: &Screenshots,
    profile_id: Uuid,
    tracker_id: Uuid,
    icon_id: Uuid,
) -> AppResult<()> {
    store.update(|config| {
        let tracker = find_tracker(config, profile_id, tracker_id)?;
        let index = tracker
            .icons
            .iter()
            .position(|i| i.id == icon_id)
            .ok_or(AppError::NotFound("Иконка не найдена"))?;
        tracker.icons.remove(index);
        tracker.stop_if_incomplete();
        Ok(())
    })?;
    screenshots.remove_icon(profile_id, icon_id);
    Ok(())
}

fn find_profile(config: &mut Config, id: Uuid) -> AppResult<&mut Profile> {
    config
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
        .ok_or(PROFILE_NOT_FOUND)
}

fn find_tracker(
    config: &mut Config,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<&mut Tracker> {
    find_profile(config, profile_id)?
        .trackers
        .iter_mut()
        .find(|tracker| tracker.id == tracker_id)
        .ok_or(TRACKER_NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionKind, KeyCombo, KeySequenceAction, KeyStep};
    use crate::config::Rgb;
    use std::fs;
    use tempfile::TempDir;

    fn fixture_icon(fixture: &Fixture, profile_id: Uuid, tracker_id: Uuid) -> Icon {
        let rect = crate::config::PhysicalRect {
            x: 0,
            y: 0,
            width: 2,
            height: 2,
        };
        let icon = Icon::new(Uuid::new_v4(), rect, rect);
        fs::create_dir_all(fixture.screenshots.profile_dir(profile_id)).unwrap();
        for path in [
            fixture.screenshots.image_path(profile_id, icon.id),
            fixture.screenshots.thumbnail_path(profile_id, icon.id),
            fixture.screenshots.template_path(profile_id, icon.id),
        ] {
            fs::write(path, b"png").unwrap();
        }
        add_icon(&fixture.store, profile_id, tracker_id, icon).unwrap()
    }

    #[test]
    fn icon_mutations_save_immediately_preserve_other_fields_and_stop_only_last_visual() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Game");
        let (tracker, pixel) = complete_tracker(&fixture, profile.id);
        let combo = combo(&[], "F5");
        set_hotkey(&fixture.store, profile.id, tracker.id, Some(combo.clone())).unwrap();
        let icon = fixture_icon(&fixture, profile.id, tracker.id);
        let saved = fixture.reloaded()[0].trackers[0].clone();
        assert_eq!(saved.icons, [icon.clone()]);
        set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();
        let updated = update_icon(
            &fixture.store,
            profile.id,
            tracker.id,
            icon.id,
            IconInput {
                condition: IconCondition::Absent,
                similarity_percent: 75,
            },
        )
        .unwrap();
        let reloaded = fixture.reloaded()[0].trackers[0].clone();
        assert_eq!(reloaded.icons, [updated.clone()]);
        assert_eq!(reloaded.pixels, saved.pixels);
        assert_eq!(reloaded.hotkey, Some(combo));
        assert_eq!(reloaded.action, saved.action);
        assert_eq!(reloaded.name, saved.name);
        assert_eq!(reloaded.pre_action_delay_ms, saved.pre_action_delay_ms);
        for threshold in [0, 101] {
            assert!(update_icon(
                &fixture.store,
                profile.id,
                tracker.id,
                icon.id,
                IconInput {
                    condition: IconCondition::Present,
                    similarity_percent: threshold
                }
            )
            .is_err());
        }
        assert_eq!(fixture.reloaded()[0].trackers[0].icons, [updated]);
        delete_pixel(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            pixel.id,
        )
        .unwrap();
        assert!(is_running(&fixture));
        delete_icon(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            icon.id,
        )
        .unwrap();
        assert!(!is_running(&fixture));
        assert!(!fixture
            .screenshots
            .template_path(profile.id, icon.id)
            .exists());
        assert!(!fixture.has_screenshot(profile.id, icon.id));
    }

    #[test]
    fn deleting_tracker_or_profile_removes_all_icon_files_and_preserves_other_trackers() {
        for whole_profile in [false, true] {
            let fixture = Fixture::new();
            let profile = fixture.profile("Game");
            let tracker = create_tracker(&fixture.store, profile.id, "First".into()).unwrap();
            let icon = fixture_icon(&fixture, profile.id, tracker.id);
            let kept = create_tracker(&fixture.store, profile.id, "Second".into()).unwrap();
            let kept_icon = fixture_icon(&fixture, profile.id, kept.id);
            if whole_profile {
                delete_profile(&fixture.store, &fixture.screenshots, profile.id).unwrap();
            } else {
                delete_tracker(&fixture.store, &fixture.screenshots, profile.id, tracker.id)
                    .unwrap();
            }
            assert!(!fixture
                .screenshots
                .template_path(profile.id, icon.id)
                .exists());
            assert!(!fixture.has_screenshot(profile.id, icon.id));
            assert_eq!(
                fixture
                    .screenshots
                    .template_path(profile.id, kept_icon.id)
                    .exists(),
                !whole_profile
            );
        }
    }

    struct Fixture {
        dir: TempDir,
        store: ConfigStore,
        screenshots: Screenshots,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let store = ConfigStore::load(dir.path()).unwrap().store;
            let screenshots = Screenshots::new(dir.path().join("screenshots"));
            Self {
                dir,
                store,
                screenshots,
            }
        }

        fn reloaded(&self) -> Vec<Profile> {
            list_profiles(&ConfigStore::load(self.dir.path()).unwrap().store)
        }

        fn profile(&self, name: &str) -> Profile {
            create_profile(&self.store, profile_input(name, "game.exe")).unwrap()
        }

        /// Adds a pixel to the tracker together with its screenshot files.
        fn pixel(&self, profile_id: Uuid, tracker_id: Uuid) -> Pixel {
            let pixel = Pixel::new(
                Uuid::new_v4(),
                812,
                1040,
                Rgb {
                    r: 200,
                    g: 30,
                    b: 30,
                },
            );
            fs::create_dir_all(self.screenshots.profile_dir(profile_id)).unwrap();
            fs::write(self.screenshots.image_path(profile_id, pixel.id), b"png").unwrap();
            fs::write(
                self.screenshots.thumbnail_path(profile_id, pixel.id),
                b"png",
            )
            .unwrap();
            add_pixel(&self.store, profile_id, tracker_id, pixel).unwrap()
        }

        fn has_screenshot(&self, profile_id: Uuid, pixel_id: Uuid) -> bool {
            self.screenshots.image_path(profile_id, pixel_id).exists()
                || self
                    .screenshots
                    .thumbnail_path(profile_id, pixel_id)
                    .exists()
        }
    }

    fn pixel_input(condition: PixelCondition, tolerance_percent: u8) -> PixelInput {
        PixelInput {
            condition,
            tolerance_percent,
        }
    }

    fn profile_input(name: &str, process_name: &str) -> ProfileInput {
        ProfileInput {
            name: name.into(),
            process_name: process_name.into(),
        }
    }

    fn tracker_input(name: &str, min: u32, max: u32, resume_delay_ms: u32) -> TrackerInput {
        TrackerInput {
            name: name.into(),
            pre_action_delay_ms: DelayRange { min, max },
            resume_delay_ms,
        }
    }

    fn assert_validation_error<T: std::fmt::Debug>(result: AppResult<T>) {
        assert!(matches!(result, Err(AppError::Validation(_))), "{result:?}");
    }

    #[test]
    fn creates_and_persists_profile() {
        let fixture = Fixture::new();

        let profile =
            create_profile(&fixture.store, profile_input(" Diablo ", "game.exe")).unwrap();

        assert_eq!(profile.name, "Diablo");
        assert_eq!(profile.process_name, "game.exe");
        assert!(profile.trackers.is_empty());
        assert_eq!(list_profiles(&fixture.store), [profile.clone()]);
        assert_eq!(fixture.reloaded(), [profile]);
    }

    #[test]
    fn rejects_profile_without_name_or_process() {
        let fixture = Fixture::new();

        assert_validation_error(create_profile(
            &fixture.store,
            profile_input(" ", "game.exe"),
        ));
        assert_validation_error(create_profile(&fixture.store, profile_input("Diablo", "")));
        assert!(list_profiles(&fixture.store).is_empty());
    }

    #[test]
    fn updates_profile_name_and_process() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        let updated = update_profile(
            &fixture.store,
            profile.id,
            profile_input("Diablo IV", "diablo4.exe"),
        )
        .unwrap();

        assert_eq!(updated.id, profile.id);
        assert_eq!(updated.process_name, "diablo4.exe");
        assert_eq!(fixture.reloaded(), [updated]);
    }

    #[test]
    fn rejects_invalid_profile_update() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        assert_validation_error(update_profile(
            &fixture.store,
            profile.id,
            profile_input("", "x.exe"),
        ));
        assert_eq!(list_profiles(&fixture.store), [profile]);
    }

    #[test]
    fn deletes_profile_with_trackers_and_their_screenshots() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let kept = fixture.profile("Path of Exile");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        fixture.pixel(profile.id, tracker.id);
        let kept_tracker = create_tracker(&fixture.store, kept.id, "Мана".into()).unwrap();
        let kept_pixel = fixture.pixel(kept.id, kept_tracker.id);

        delete_profile(&fixture.store, &fixture.screenshots, profile.id).unwrap();

        let remaining: Vec<_> = fixture
            .reloaded()
            .iter()
            .map(|profile| profile.id)
            .collect();
        assert_eq!(remaining, [kept.id]);
        assert!(!fixture.screenshots.profile_dir(profile.id).exists());
        assert!(fixture.has_screenshot(kept.id, kept_pixel.id));
    }

    #[test]
    fn reports_missing_profile() {
        let fixture = Fixture::new();
        let missing = Uuid::new_v4();

        assert!(matches!(
            delete_profile(&fixture.store, &fixture.screenshots, missing),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            update_profile(&fixture.store, missing, profile_input("Diablo", "game.exe")),
            Err(AppError::NotFound(_))
        ));
        assert!(matches!(
            create_tracker(&fixture.store, missing, "Здоровье".into()),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn creates_tracker_with_default_timings() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        assert_eq!(tracker.name, "Здоровье");
        assert_eq!(
            tracker.pre_action_delay_ms,
            DelayRange { min: 100, max: 300 }
        );
        assert_eq!(tracker.resume_delay_ms, 1000);
        assert_eq!(fixture.reloaded()[0].trackers, [tracker]);
    }

    #[test]
    fn rejects_tracker_with_blank_name() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        assert_validation_error(create_tracker(&fixture.store, profile.id, "  ".into()));
        assert!(fixture.reloaded()[0].trackers.is_empty());
    }

    #[test]
    fn updates_tracker_settings() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        let updated = update_tracker(
            &fixture.store,
            profile.id,
            tracker.id,
            tracker_input("Мана", 50, 50, 0),
        )
        .unwrap();

        assert_eq!(updated.id, tracker.id);
        assert_eq!(updated.name, "Мана");
        assert_eq!(updated.pre_action_delay_ms, DelayRange { min: 50, max: 50 });
        assert_eq!(updated.resume_delay_ms, 0);
        assert_eq!(fixture.reloaded()[0].trackers, [updated]);
    }

    #[test]
    fn rejects_invalid_tracker_update() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        assert_validation_error(update_tracker(
            &fixture.store,
            profile.id,
            tracker.id,
            tracker_input("Здоровье", 500, 200, 1000),
        ));
        assert_validation_error(update_tracker(
            &fixture.store,
            profile.id,
            tracker.id,
            tracker_input("", 100, 300, 1000),
        ));
        assert_eq!(fixture.reloaded()[0].trackers, [tracker]);
    }

    fn key_sequence(steps: &[(&str, Option<DelayRange>)]) -> Action {
        Action::key_sequence(KeySequenceAction {
            steps: steps
                .iter()
                .map(|(key, delay)| KeyStep {
                    id: Uuid::new_v4(),
                    delay_before_ms: *delay,
                    hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
                    combo: KeyCombo {
                        modifiers: vec![],
                        key: (*key).into(),
                    },
                })
                .collect(),
        })
    }

    #[test]
    fn creates_tracker_with_empty_key_sequence() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        assert_eq!(tracker.action, key_sequence(&[]));
    }

    #[test]
    fn saves_normalized_action() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let delay = Some(DelayRange { min: 300, max: 600 });
        let mut action = key_sequence(&[("KeyW", delay), ("KeyE", delay)]);
        let ActionKind::KeySequence(sequence) = &mut action.kind else {
            unreachable!()
        };
        sequence.steps[0].hold_ms = DelayRange {
            min: 2000,
            max: 3000,
        };
        sequence.steps[1].hold_ms = DelayRange { min: 100, max: 200 };

        let saved = update_action(&fixture.store, profile.id, tracker.id, action).unwrap();

        let ActionKind::KeySequence(sequence) = &saved.kind else {
            panic!("saved action is a key sequence");
        };
        assert_eq!(sequence.steps[0].delay_before_ms, None);
        assert_eq!(sequence.steps[1].delay_before_ms, delay);
        assert_eq!(
            sequence.steps[0].hold_ms,
            DelayRange {
                min: 2000,
                max: 3000
            }
        );
        assert_eq!(sequence.steps[1].hold_ms, DelayRange { min: 100, max: 200 });
        assert_eq!(fixture.reloaded()[0].trackers[0].action, saved);
    }

    #[test]
    fn invalid_hold_update_keeps_memory_and_disk_unchanged() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        let saved = get_action(&fixture.store, profile.id, tracker.id).unwrap();
        let bytes = fs::read(fixture.dir.path().join("config.json")).unwrap();
        let mut action = saved.clone();
        let ActionKind::KeySequence(sequence) = &mut action.kind else {
            unreachable!()
        };
        sequence.steps[0].hold_ms = DelayRange {
            min: 3000,
            max: 2000,
        };
        assert_validation_error(update_action(
            &fixture.store,
            profile.id,
            tracker.id,
            action,
        ));
        assert_eq!(
            get_action(&fixture.store, profile.id, tracker.id).unwrap(),
            saved
        );
        assert_eq!(fixture.reloaded()[0].trackers[0].action, saved);
        assert_eq!(
            fs::read(fixture.dir.path().join("config.json")).unwrap(),
            bytes
        );
    }

    #[test]
    fn persists_sound_per_tracker_without_changing_steps_or_run_state() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (first, _) = complete_tracker(&fixture, profile.id);
        let (second, _) = complete_tracker(&fixture, profile.id);
        set_tracker_running(&fixture.store, profile.id, first.id, true).unwrap();
        let second_action = get_action(&fixture.store, profile.id, second.id).unwrap();
        let mut action = get_action(&fixture.store, profile.id, first.id).unwrap();
        let steps = action.kind.clone();
        action.sound = crate::action::SoundSettings {
            id: Some("arcade-coin".into()),
            volume_percent: 35,
        };
        update_action(&fixture.store, profile.id, first.id, action.clone()).unwrap();
        let reloaded = fixture.reloaded();
        assert_eq!(reloaded[0].trackers[0].action, action);
        assert_eq!(reloaded[0].trackers[0].action.kind, steps);
        assert!(reloaded[0].trackers[0].is_running);
        assert_eq!(reloaded[0].trackers[1].action, second_action);
    }

    #[test]
    fn invalid_sound_update_keeps_memory_and_disk_unchanged() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        let saved = get_action(&fixture.store, profile.id, tracker.id).unwrap();
        let bytes = fs::read(fixture.dir.path().join("config.json")).unwrap();
        for sound in [
            crate::action::SoundSettings {
                id: Some("unknown-sound".into()),
                volume_percent: 50,
            },
            crate::action::SoundSettings {
                id: Some("soft-ping".into()),
                volume_percent: 101,
            },
        ] {
            let mut action = saved.clone();
            action.sound = sound;
            assert_validation_error(update_action(
                &fixture.store,
                profile.id,
                tracker.id,
                action,
            ));
            assert_eq!(
                get_action(&fixture.store, profile.id, tracker.id).unwrap(),
                saved
            );
            assert_eq!(fixture.reloaded()[0].trackers[0].action, saved);
            assert_eq!(
                fs::read(fixture.dir.path().join("config.json")).unwrap(),
                bytes
            );
        }
    }

    #[test]
    fn rejects_action_with_unknown_key_or_reversed_delay() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        assert_validation_error(update_action(
            &fixture.store,
            profile.id,
            tracker.id,
            key_sequence(&[("PrintScreen", None)]),
        ));
        assert_validation_error(update_action(
            &fixture.store,
            profile.id,
            tracker.id,
            key_sequence(&[
                ("KeyQ", None),
                ("KeyW", Some(DelayRange { min: 600, max: 300 })),
            ]),
        ));
        assert_eq!(fixture.reloaded()[0].trackers[0].action, key_sequence(&[]));
    }

    #[test]
    fn reports_missing_tracker_for_action() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        assert!(matches!(
            update_action(
                &fixture.store,
                profile.id,
                Uuid::new_v4(),
                key_sequence(&[])
            ),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn deletes_tracker_with_its_screenshots() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let deleted = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let deleted_pixel = fixture.pixel(profile.id, deleted.id);
        let kept = create_tracker(&fixture.store, profile.id, "Мана".into()).unwrap();
        let kept_pixel = fixture.pixel(profile.id, kept.id);

        delete_tracker(&fixture.store, &fixture.screenshots, profile.id, deleted.id).unwrap();

        assert_eq!(fixture.reloaded()[0].trackers.len(), 1);
        assert_eq!(fixture.reloaded()[0].trackers[0].id, kept.id);
        assert!(!fixture.has_screenshot(profile.id, deleted_pixel.id));
        assert!(fixture.has_screenshot(profile.id, kept_pixel.id));
        assert!(matches!(
            delete_tracker(&fixture.store, &fixture.screenshots, profile.id, deleted.id),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn adds_pixel_with_equals_condition_and_10_percent_tolerance() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        let pixel = fixture.pixel(profile.id, tracker.id);

        assert_eq!(pixel.condition, PixelCondition::Equals);
        assert_eq!(pixel.tolerance_percent, 10);
        assert_eq!(fixture.reloaded()[0].trackers[0].pixels, [pixel]);
    }

    #[test]
    fn updates_pixel_condition_and_tolerance() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let pixel = fixture.pixel(profile.id, tracker.id);

        let updated = update_pixel(
            &fixture.store,
            profile.id,
            tracker.id,
            pixel.id,
            pixel_input(PixelCondition::NotEquals, 15),
        )
        .unwrap();

        assert_eq!(updated.condition, PixelCondition::NotEquals);
        assert_eq!(updated.tolerance_percent, 15);
        assert_eq!(
            (updated.x, updated.y, updated.color),
            (pixel.x, pixel.y, pixel.color)
        );
        assert_eq!(fixture.reloaded()[0].trackers[0].pixels, [updated]);
    }

    #[test]
    fn rejects_tolerance_above_100() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let pixel = fixture.pixel(profile.id, tracker.id);

        assert_validation_error(update_pixel(
            &fixture.store,
            profile.id,
            tracker.id,
            pixel.id,
            pixel_input(PixelCondition::Equals, 120),
        ));
        assert_eq!(fixture.reloaded()[0].trackers[0].pixels, [pixel]);
    }

    #[test]
    fn deletes_pixel_with_its_screenshot() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let deleted = fixture.pixel(profile.id, tracker.id);
        let kept = fixture.pixel(profile.id, tracker.id);

        delete_pixel(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            deleted.id,
        )
        .unwrap();

        assert!(!fixture.has_screenshot(profile.id, deleted.id));
        assert!(fixture.has_screenshot(profile.id, kept.id));
        assert_eq!(fixture.reloaded()[0].trackers[0].pixels, [kept]);
        assert!(matches!(
            delete_pixel(
                &fixture.store,
                &fixture.screenshots,
                profile.id,
                tracker.id,
                deleted.id
            ),
            Err(AppError::NotFound(_))
        ));
    }

    /// Tracker with one pixel and one step, ready to run.
    fn complete_tracker(fixture: &Fixture, profile_id: Uuid) -> (Tracker, Pixel) {
        let tracker = create_tracker(&fixture.store, profile_id, "Здоровье".into()).unwrap();
        let pixel = fixture.pixel(profile_id, tracker.id);
        update_action(
            &fixture.store,
            profile_id,
            tracker.id,
            key_sequence(&[("KeyQ", None)]),
        )
        .unwrap();
        (tracker, pixel)
    }

    fn is_running(fixture: &Fixture) -> bool {
        fixture.reloaded()[0].trackers[0].is_running
    }

    #[test]
    fn starts_and_stops_a_complete_tracker_persistently() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        assert!(!is_running(&fixture));

        let started = set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();
        assert!(started.is_running);
        assert!(is_running(&fixture));

        set_tracker_running(&fixture.store, profile.id, tracker.id, false).unwrap();
        assert!(!is_running(&fixture));
    }

    #[test]
    fn refuses_to_start_a_tracker_without_pixels_or_steps() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        assert_validation_error(set_tracker_running(
            &fixture.store,
            profile.id,
            tracker.id,
            true,
        ));

        fixture.pixel(profile.id, tracker.id);
        assert_validation_error(set_tracker_running(
            &fixture.store,
            profile.id,
            tracker.id,
            true,
        ));
        assert!(!is_running(&fixture));
        assert!(set_tracker_running(&fixture.store, profile.id, tracker.id, false).is_ok());
    }

    #[test]
    fn deleting_the_last_pixel_stops_the_tracker() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, first) = complete_tracker(&fixture, profile.id);
        let second = fixture.pixel(profile.id, tracker.id);
        set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();

        delete_pixel(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            first.id,
        )
        .unwrap();
        assert!(is_running(&fixture));

        delete_pixel(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            second.id,
        )
        .unwrap();
        assert!(!is_running(&fixture));
    }

    #[test]
    fn removing_the_last_step_stops_the_tracker() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();

        update_action(
            &fixture.store,
            profile.id,
            tracker.id,
            key_sequence(&[("KeyE", None)]),
        )
        .unwrap();
        assert!(is_running(&fixture));

        update_action(&fixture.store, profile.id, tracker.id, key_sequence(&[])).unwrap();
        assert!(!is_running(&fixture));
    }

    #[test]
    fn hotkey_without_pixels_stays_complete_and_an_unready_variant_stops_it() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();
        let combo = KeyCombo {
            modifiers: vec![crate::action::Modifier::Ctrl],
            key: "Digit1".into(),
        };

        fixture
            .store
            .update(|config| {
                let tracker = &mut config.profiles[0].trackers[0];
                tracker.trigger = Trigger::Hotkey;
                tracker.hotkey = Some(combo.clone());
                tracker.pixels.clear();
                tracker.stop_if_incomplete();
                assert!(tracker.is_complete());
                Ok(())
            })
            .unwrap();
        assert!(is_running(&fixture));

        fixture
            .store
            .update(|config| {
                let tracker = &mut config.profiles[0].trackers[0];
                tracker.hotkey = None;
                tracker.stop_if_incomplete();
                assert_eq!(
                    tracker.missing_part(),
                    Some(crate::config::MissingPart::Hotkey)
                );
                Ok(())
            })
            .unwrap();
        assert!(!is_running(&fixture));

        fixture
            .store
            .update(|config| {
                let tracker = &mut config.profiles[0].trackers[0];
                tracker.trigger = Trigger::Pixels;
                tracker
                    .pixels
                    .push(Pixel::new(Uuid::new_v4(), 0, 0, Rgb { r: 0, g: 0, b: 0 }));
                tracker.hotkey = Some(combo);
                tracker.is_running = true;
                tracker.stop_if_incomplete();
                Ok(())
            })
            .unwrap();
        assert!(is_running(&fixture));

        fixture
            .store
            .update(|config| {
                let tracker = &mut config.profiles[0].trackers[0];
                tracker.trigger = Trigger::Hotkey;
                tracker.hotkey = None;
                tracker.stop_if_incomplete();
                Ok(())
            })
            .unwrap();
        assert!(!is_running(&fixture));
    }

    fn combo(modifiers: &[&str], key: &str) -> KeyCombo {
        KeyCombo {
            modifiers: modifiers
                .iter()
                .map(|name| match *name {
                    "ctrl" => crate::action::Modifier::Ctrl,
                    "shift" => crate::action::Modifier::Shift,
                    "alt" => crate::action::Modifier::Alt,
                    _ => unreachable!(),
                })
                .collect(),
            key: key.into(),
        }
    }

    #[test]
    fn trigger_keeps_pixels_and_the_combo_and_hotkey_conflicts_stay_on_the_previous_value() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let other = fixture.profile("PoE");
        let (tracker, pixel) = complete_tracker(&fixture, profile.id);
        let (other_tracker, _) = complete_tracker(&fixture, other.id);
        let second = create_tracker(&fixture.store, profile.id, "Мана".into()).unwrap();
        settings::set_pause_hotkey(&fixture.store, Some(combo(&["ctrl"], "KeyP"))).unwrap();

        let saved = set_hotkey(
            &fixture.store,
            other.id,
            other_tracker.id,
            Some(combo(&[], "F5")),
        )
        .unwrap();
        assert_eq!(
            saved.hotkey.as_ref().map(|item| item.key.as_str()),
            Some("F5")
        );

        let saved = set_hotkey(
            &fixture.store,
            profile.id,
            tracker.id,
            Some(combo(&[], "F5")),
        )
        .unwrap();
        assert_eq!(
            saved.hotkey.as_ref().map(|item| item.key.as_str()),
            Some("F5")
        );

        let conflict = set_hotkey(
            &fixture.store,
            profile.id,
            second.id,
            Some(combo(&[], "F5")),
        );
        assert!(matches!(conflict, Err(AppError::Validation(_))));
        assert_eq!(fixture.reloaded()[0].trackers[1].hotkey, None);

        set_hotkey(
            &fixture.store,
            profile.id,
            tracker.id,
            Some(combo(&["ctrl", "shift"], "Digit1")),
        )
        .unwrap();
        let reversed = set_hotkey(
            &fixture.store,
            profile.id,
            second.id,
            Some(combo(&["shift", "ctrl"], "Digit1")),
        );
        assert!(matches!(reversed, Err(AppError::Validation(_))));
        assert_eq!(fixture.reloaded()[0].trackers[1].hotkey, None);

        let pause = set_hotkey(
            &fixture.store,
            profile.id,
            second.id,
            Some(combo(&["ctrl"], "KeyP")),
        );
        assert!(matches!(pause, Err(AppError::Validation(_))));
        assert_eq!(fixture.reloaded()[0].trackers[1].hotkey, None);
        assert_eq!(
            fixture.reloaded()[0].trackers[0]
                .hotkey
                .as_ref()
                .map(|item| item.key.as_str()),
            Some("Digit1")
        );

        let switched =
            set_trigger(&fixture.store, profile.id, tracker.id, Trigger::Hotkey).unwrap();
        assert_eq!(switched.trigger, Trigger::Hotkey);
        assert_eq!(switched.pixels, [pixel.clone()]);
        assert!(switched.hotkey.is_some());

        let back = set_trigger(&fixture.store, profile.id, tracker.id, Trigger::Pixels).unwrap();
        assert_eq!(back.trigger, Trigger::Pixels);
        assert_eq!(back.pixels, [pixel]);
        assert_eq!(
            back.hotkey.as_ref().map(|item| item.key.as_str()),
            Some("Digit1")
        );
    }

    #[test]
    fn keeps_run_state_when_tracker_settings_change() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let (tracker, _) = complete_tracker(&fixture, profile.id);
        set_tracker_running(&fixture.store, profile.id, tracker.id, true).unwrap();

        update_tracker(
            &fixture.store,
            profile.id,
            tracker.id,
            tracker_input("Мана", 0, 0, 0),
        )
        .unwrap();

        assert!(is_running(&fixture));
    }

    #[test]
    fn deletes_pixel_even_when_its_screenshot_is_gone() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let pixel = fixture.pixel(profile.id, tracker.id);
        fs::remove_dir_all(fixture.screenshots.profile_dir(profile.id)).unwrap();

        delete_pixel(
            &fixture.store,
            &fixture.screenshots,
            profile.id,
            tracker.id,
            pixel.id,
        )
        .unwrap();

        assert!(fixture.reloaded()[0].trackers[0].pixels.is_empty());
    }
}
