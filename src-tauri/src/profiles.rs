use serde::Deserialize;
use uuid::Uuid;

use crate::config::{Config, DelayRange, Pixel, PixelCondition, Profile, Tracker};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::screenshots::Screenshots;

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
        find_profile(config, profile_id)?.trackers.push(tracker.clone());
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
        let pixels = &mut find_tracker(config, profile_id, tracker_id)?.pixels;
        let index = pixels
            .iter()
            .position(|pixel| pixel.id == pixel_id)
            .ok_or(PIXEL_NOT_FOUND)?;
        pixels.remove(index);
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
pub fn profile_process_name(store: &ConfigStore, profile_id: Uuid, tracker_id: Uuid) -> AppResult<String> {
    store.read(|config| {
        let profile = config
            .profiles
            .iter()
            .find(|profile| profile.id == profile_id)
            .ok_or(PROFILE_NOT_FOUND)?;
        if !profile.trackers.iter().any(|tracker| tracker.id == tracker_id) {
            return Err(TRACKER_NOT_FOUND);
        }
        Ok(profile.process_name.clone())
    })
}

const PROFILE_NOT_FOUND: AppError = AppError::NotFound("Профиль не найден");
const TRACKER_NOT_FOUND: AppError = AppError::NotFound("Отслеживание не найдено");
const PIXEL_NOT_FOUND: AppError = AppError::NotFound("Пиксель не найден");

fn find_profile(config: &mut Config, id: Uuid) -> AppResult<&mut Profile> {
    config
        .profiles
        .iter_mut()
        .find(|profile| profile.id == id)
        .ok_or(PROFILE_NOT_FOUND)
}

fn find_tracker(config: &mut Config, profile_id: Uuid, tracker_id: Uuid) -> AppResult<&mut Tracker> {
    find_profile(config, profile_id)?
        .trackers
        .iter_mut()
        .find(|tracker| tracker.id == tracker_id)
        .ok_or(TRACKER_NOT_FOUND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Rgb;
    use std::fs;
    use tempfile::TempDir;

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
            let pixel = Pixel::new(Uuid::new_v4(), 812, 1040, Rgb { r: 200, g: 30, b: 30 });
            fs::create_dir_all(self.screenshots.profile_dir(profile_id)).unwrap();
            fs::write(self.screenshots.image_path(profile_id, pixel.id), b"png").unwrap();
            fs::write(self.screenshots.thumbnail_path(profile_id, pixel.id), b"png").unwrap();
            add_pixel(&self.store, profile_id, tracker_id, pixel).unwrap()
        }

        fn has_screenshot(&self, profile_id: Uuid, pixel_id: Uuid) -> bool {
            self.screenshots.image_path(profile_id, pixel_id).exists()
                || self.screenshots.thumbnail_path(profile_id, pixel_id).exists()
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

        let profile = create_profile(&fixture.store, profile_input(" Diablo ", "game.exe")).unwrap();

        assert_eq!(profile.name, "Diablo");
        assert_eq!(profile.process_name, "game.exe");
        assert!(profile.trackers.is_empty());
        assert_eq!(list_profiles(&fixture.store), [profile.clone()]);
        assert_eq!(fixture.reloaded(), [profile]);
    }

    #[test]
    fn rejects_profile_without_name_or_process() {
        let fixture = Fixture::new();

        assert_validation_error(create_profile(&fixture.store, profile_input(" ", "game.exe")));
        assert_validation_error(create_profile(&fixture.store, profile_input("Diablo", "")));
        assert!(list_profiles(&fixture.store).is_empty());
    }

    #[test]
    fn updates_profile_name_and_process() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        let updated =
            update_profile(&fixture.store, profile.id, profile_input("Diablo IV", "diablo4.exe"))
                .unwrap();

        assert_eq!(updated.id, profile.id);
        assert_eq!(updated.process_name, "diablo4.exe");
        assert_eq!(fixture.reloaded(), [updated]);
    }

    #[test]
    fn rejects_invalid_profile_update() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");

        assert_validation_error(update_profile(&fixture.store, profile.id, profile_input("", "x.exe")));
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

        let remaining: Vec<_> = fixture.reloaded().iter().map(|profile| profile.id).collect();
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
        assert_eq!(tracker.pre_action_delay_ms, DelayRange { min: 100, max: 300 });
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
        assert_eq!((updated.x, updated.y, updated.color), (pixel.x, pixel.y, pixel.color));
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

        delete_pixel(&fixture.store, &fixture.screenshots, profile.id, tracker.id, deleted.id)
            .unwrap();

        assert!(!fixture.has_screenshot(profile.id, deleted.id));
        assert!(fixture.has_screenshot(profile.id, kept.id));
        assert_eq!(fixture.reloaded()[0].trackers[0].pixels, [kept]);
        assert!(matches!(
            delete_pixel(&fixture.store, &fixture.screenshots, profile.id, tracker.id, deleted.id),
            Err(AppError::NotFound(_))
        ));
    }

    #[test]
    fn deletes_pixel_even_when_its_screenshot_is_gone() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let tracker = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let pixel = fixture.pixel(profile.id, tracker.id);
        fs::remove_dir_all(fixture.screenshots.profile_dir(profile.id)).unwrap();

        delete_pixel(&fixture.store, &fixture.screenshots, profile.id, tracker.id, pixel.id).unwrap();

        assert!(fixture.reloaded()[0].trackers[0].pixels.is_empty());
    }
}
