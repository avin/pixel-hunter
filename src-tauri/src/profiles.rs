use serde::Deserialize;
use uuid::Uuid;

use crate::config::{Config, DelayRange, Profile, Tracker};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};

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

pub fn delete_profile(store: &ConfigStore, id: Uuid) -> AppResult<()> {
    store.update(|config| {
        let index = config
            .profiles
            .iter()
            .position(|profile| profile.id == id)
            .ok_or(PROFILE_NOT_FOUND)?;
        config.profiles.remove(index);
        Ok(())
    })
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

pub fn delete_tracker(store: &ConfigStore, profile_id: Uuid, tracker_id: Uuid) -> AppResult<()> {
    store.update(|config| {
        let trackers = &mut find_profile(config, profile_id)?.trackers;
        let index = trackers
            .iter()
            .position(|tracker| tracker.id == tracker_id)
            .ok_or(TRACKER_NOT_FOUND)?;
        trackers.remove(index);
        Ok(())
    })
}

const PROFILE_NOT_FOUND: AppError = AppError::NotFound("Профиль не найден");
const TRACKER_NOT_FOUND: AppError = AppError::NotFound("Отслеживание не найдено");

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
    use tempfile::TempDir;

    struct Fixture {
        dir: TempDir,
        store: ConfigStore,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let store = ConfigStore::load(dir.path()).unwrap().store;
            Self { dir, store }
        }

        fn reloaded(&self) -> Vec<Profile> {
            list_profiles(&ConfigStore::load(self.dir.path()).unwrap().store)
        }

        fn profile(&self, name: &str) -> Profile {
            create_profile(&self.store, profile_input(name, "game.exe")).unwrap()
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
    fn deletes_profile_with_trackers() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let kept = fixture.profile("Path of Exile");
        create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();

        delete_profile(&fixture.store, profile.id).unwrap();

        assert_eq!(fixture.reloaded(), [kept]);
    }

    #[test]
    fn reports_missing_profile() {
        let fixture = Fixture::new();
        let missing = Uuid::new_v4();

        assert!(matches!(delete_profile(&fixture.store, missing), Err(AppError::NotFound(_))));
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
    fn deletes_tracker() {
        let fixture = Fixture::new();
        let profile = fixture.profile("Diablo");
        let deleted = create_tracker(&fixture.store, profile.id, "Здоровье".into()).unwrap();
        let kept = create_tracker(&fixture.store, profile.id, "Мана".into()).unwrap();

        delete_tracker(&fixture.store, profile.id, deleted.id).unwrap();

        assert_eq!(fixture.reloaded()[0].trackers, [kept]);
        assert!(matches!(
            delete_tracker(&fixture.store, profile.id, deleted.id),
            Err(AppError::NotFound(_))
        ));
    }
}
