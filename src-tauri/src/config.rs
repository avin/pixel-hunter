use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::action::{Action, KeyCombo};
use crate::error::{AppError, AppResult};

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_RESUME_DELAY_MS: u32 = 1000;
pub const DEFAULT_TOLERANCE_PERCENT: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub profiles: Vec<Profile>,
    #[serde(default)]
    pub settings: Settings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            profiles: Vec::new(),
            settings: Settings::default(),
        }
    }
}

/// Session preferences stored next to profiles. Pause itself is not here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub pause_hotkey: Option<KeyCombo>,
    pub last_screen: LastScreen,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            pause_hotkey: None,
            last_screen: LastScreen::Profiles,
        }
    }
}

/// Screen open when the app last quit. Missing data means the profile list.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum LastScreen {
    Profiles,
    #[serde(rename_all = "camelCase")]
    Profile {
        profile_id: Uuid,
    },
    #[serde(rename_all = "camelCase")]
    Editor {
        profile_id: Uuid,
        tracker_id: Uuid,
    },
}

impl Default for LastScreen {
    fn default() -> Self {
        Self::Profiles
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: Uuid,
    pub name: String,
    pub process_name: String,
    pub trackers: Vec<Tracker>,
}

impl Profile {
    pub fn new(name: String, process_name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            process_name,
            trackers: Vec::new(),
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        validate_name(&self.name, "Имя профиля не может быть пустым")?;
        validate_name(&self.process_name, "Процесс профиля не выбран")
    }
}

/// What starts the tracker's action. Missing data means pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Trigger {
    #[default]
    Pixels,
    Hotkey,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tracker {
    pub id: Uuid,
    pub name: String,
    pub pre_action_delay_ms: DelayRange,
    pub resume_delay_ms: u32,
    #[serde(default)]
    pub pixels: Vec<Pixel>,
    #[serde(default)]
    pub trigger: Trigger,
    #[serde(default)]
    pub hotkey: Option<KeyCombo>,
    #[serde(default)]
    pub action: Action,
    /// Chosen by the user; the engine runs the tracker only while it is also complete.
    #[serde(default)]
    pub is_running: bool,
}

/// Part of the tracker that must be configured before it can run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MissingPart {
    Pixels,
    Hotkey,
    ActionSteps,
}

impl MissingPart {
    pub fn message(self) -> &'static str {
        match self {
            MissingPart::Pixels => "Добавьте пиксель, чтобы запустить отслеживание",
            MissingPart::Hotkey => "Назначьте сочетание, чтобы запустить отслеживание",
            MissingPart::ActionSteps => "Добавьте шаг действия, чтобы запустить отслеживание",
        }
    }
}

impl Tracker {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            pre_action_delay_ms: DelayRange::default(),
            resume_delay_ms: DEFAULT_RESUME_DELAY_MS,
            pixels: Vec::new(),
            trigger: Trigger::Pixels,
            hotkey: None,
            action: Action::default(),
            is_running: false,
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        validate_name(&self.name, "Имя отслеживания не может быть пустым")?;
        self.pre_action_delay_ms.validate()
    }

    pub fn missing_part(&self) -> Option<MissingPart> {
        let condition = match self.trigger {
            Trigger::Pixels if self.pixels.is_empty() => Some(MissingPart::Pixels),
            Trigger::Hotkey if self.hotkey.is_none() => Some(MissingPart::Hotkey),
            _ => None,
        };
        condition.or_else(|| self.action.is_empty().then_some(MissingPart::ActionSteps))
    }

    pub fn is_complete(&self) -> bool {
        self.missing_part().is_none()
    }

    /// A tracker that lost its last pixel or step stops instead of staying "running" but idle.
    pub fn stop_if_incomplete(&mut self) {
        if !self.is_complete() {
            self.is_running = false;
        }
    }
}

/// Screen point in physical pixels of the primary monitor, (0, 0) at its top-left corner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pixel {
    pub id: Uuid,
    pub x: u32,
    pub y: u32,
    pub color: Rgb,
    pub condition: PixelCondition,
    pub tolerance_percent: u8,
}

impl Pixel {
    pub fn new(id: Uuid, x: u32, y: u32, color: Rgb) -> Self {
        Self {
            id,
            x,
            y,
            color,
            condition: PixelCondition::Equals,
            tolerance_percent: DEFAULT_TOLERANCE_PERCENT,
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        if self.tolerance_percent > 100 {
            return Err(AppError::Validation(
                "Допуск должен быть от 0 до 100 процентов".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PixelCondition {
    Equals,
    NotEquals,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct DelayRange {
    pub min: u32,
    pub max: u32,
}

impl Default for DelayRange {
    fn default() -> Self {
        Self { min: 100, max: 300 }
    }
}

impl DelayRange {
    pub fn validate(&self) -> AppResult<()> {
        if self.min > self.max {
            return Err(AppError::Validation(
                "Минимальная задержка не может быть больше максимальной".into(),
            ));
        }
        Ok(())
    }
}

fn validate_name(value: &str, message: &str) -> AppResult<()> {
    if value.trim().is_empty() {
        return Err(AppError::Validation(message.into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tracker_json(
        pre_action_delay_ms: serde_json::Value,
        resume_delay_ms: i64,
    ) -> serde_json::Value {
        json!({
            "id": "6f1b9a52-7f0c-4f0e-9d7b-4a3b1e2c5d6f",
            "name": "Здоровье",
            "preActionDelayMs": pre_action_delay_ms,
            "resumeDelayMs": resume_delay_ms,
        })
    }

    #[test]
    fn new_tracker_has_default_timings() {
        let tracker = Tracker::new("Здоровье".into());

        assert_eq!(
            tracker.pre_action_delay_ms,
            DelayRange { min: 100, max: 300 }
        );
        assert_eq!(tracker.resume_delay_ms, 1000);
    }

    #[test]
    fn serializes_in_camel_case_with_version() {
        let mut profile = Profile::new("Diablo".into(), "game.exe".into());
        let tracker = Tracker::new("Здоровье".into());
        profile.trackers.push(tracker.clone());
        let config = Config {
            profiles: vec![profile.clone()],
            ..Config::default()
        };

        let value = serde_json::to_value(&config).unwrap();

        assert_eq!(
            value,
            json!({
                "version": 1,
                "profiles": [{
                    "id": profile.id,
                    "name": "Diablo",
                    "processName": "game.exe",
                    "trackers": [{
                        "id": tracker.id,
                        "name": "Здоровье",
                        "preActionDelayMs": { "min": 100, "max": 300 },
                        "resumeDelayMs": 1000,
                        "pixels": [],
                        "trigger": "pixels",
                        "hotkey": null,
                        "action": { "type": "keySequence", "steps": [], "sound": { "id": null, "volumePercent": 50 } },
                        "isRunning": false,
                    }],
                }],
                "settings": {
                    "pauseHotkey": null,
                    "lastScreen": { "kind": "profiles" },
                },
            })
        );
        assert_eq!(serde_json::from_value::<Config>(value).unwrap(), config);
    }

    #[test]
    fn old_config_without_settings_opens_as_the_profile_list_without_a_hotkey() {
        let mut profile = Profile::new("Diablo".into(), "game.exe".into());
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.is_running = true;
        profile.trackers.push(tracker);
        let saved = json!({
            "version": 1,
            "profiles": [profile],
        });

        let config: Config = serde_json::from_value(saved).unwrap();

        assert_eq!(config.settings.pause_hotkey, None);
        assert_eq!(config.settings.last_screen, LastScreen::Profiles);
        assert!(config.profiles[0].trackers[0].is_running);
    }

    #[test]
    fn last_screen_uses_camel_case_fields() {
        let profile_id = Uuid::new_v4();
        let tracker_id = Uuid::new_v4();
        let screen = LastScreen::Editor {
            profile_id,
            tracker_id,
        };

        let value = serde_json::to_value(&screen).unwrap();
        assert_eq!(
            value,
            json!({
                "kind": "editor",
                "profileId": profile_id,
                "trackerId": tracker_id,
            })
        );
        assert_eq!(serde_json::from_value::<LastScreen>(value).unwrap(), screen);
    }

    #[test]
    fn round_trip_keeps_the_hotkey_the_screen_and_run_flags() {
        let mut profile = Profile::new("Diablo".into(), "game.exe".into());
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.is_running = true;
        profile.trackers.push(tracker);
        let config = Config {
            profiles: vec![profile],
            settings: Settings {
                pause_hotkey: Some(KeyCombo {
                    modifiers: vec![crate::action::Modifier::Ctrl],
                    key: "KeyQ".into(),
                }),
                last_screen: LastScreen::Editor {
                    profile_id: Uuid::new_v4(),
                    tracker_id: Uuid::new_v4(),
                },
            },
            ..Config::default()
        };

        let restored: Config =
            serde_json::from_value(serde_json::to_value(&config).unwrap()).unwrap();

        assert_eq!(restored, config);
        assert!(restored.profiles[0].trackers[0].is_running);
    }

    #[test]
    fn reads_tracker_saved_before_pixels_existed() {
        let tracker: Tracker =
            serde_json::from_value(tracker_json(json!({ "min": 100, "max": 300 }), 1000)).unwrap();

        assert!(tracker.pixels.is_empty());
    }

    #[test]
    fn reads_tracker_saved_before_actions_existed() {
        let tracker: Tracker =
            serde_json::from_value(tracker_json(json!({ "min": 100, "max": 300 }), 1000)).unwrap();

        assert_eq!(tracker.action, Action::default());
    }

    #[test]
    fn reads_tracker_saved_before_run_state_existed_as_stopped() {
        let tracker: Tracker =
            serde_json::from_value(tracker_json(json!({ "min": 100, "max": 300 }), 1000)).unwrap();

        assert!(!tracker.is_running);
    }

    #[test]
    fn tracker_saved_without_a_trigger_opens_as_pixels_and_round_trips_both_fields() {
        let mut saved = tracker_json(json!({ "min": 100, "max": 300 }), 1000);
        let pixel_id = Uuid::new_v4();
        saved["pixels"] = json!([{
            "id": pixel_id,
            "x": 1,
            "y": 2,
            "color": { "r": 3, "g": 4, "b": 5 },
            "condition": "equals",
            "tolerancePercent": 10,
        }]);
        saved["action"] = json!({
            "type": "keySequence",
            "steps": [{ "id": Uuid::new_v4(), "delayBeforeMs": null, "combo": { "modifiers": [], "key": "KeyQ" } }],
            "sound": { "id": null, "volumePercent": 50 },
        });

        let opened: Tracker = serde_json::from_value(saved).unwrap();

        assert_eq!(opened.trigger, Trigger::Pixels);
        assert_eq!(opened.hotkey, None);
        assert_eq!(opened.pixels.len(), 1);
        assert_eq!(opened.pixels[0].id, pixel_id);
        assert!(!opened.action.is_empty());

        let mut stored = opened.clone();
        stored.trigger = Trigger::Hotkey;
        stored.hotkey = Some(KeyCombo {
            modifiers: vec![crate::action::Modifier::Ctrl],
            key: "Digit1".into(),
        });
        let restored: Tracker =
            serde_json::from_value(serde_json::to_value(&stored).unwrap()).unwrap();

        assert_eq!(restored.trigger, Trigger::Hotkey);
        assert_eq!(restored.hotkey, stored.hotkey);
        assert_eq!(restored.pixels, stored.pixels);
        assert_eq!(restored.action, stored.action);
    }

    fn complete_tracker() -> Tracker {
        let mut tracker = Tracker::new("Здоровье".into());
        tracker
            .pixels
            .push(Pixel::new(Uuid::new_v4(), 0, 0, Rgb { r: 0, g: 0, b: 0 }));
        tracker.action = serde_json::from_value(json!({
            "type": "keySequence",
            "steps": [{ "id": Uuid::new_v4(), "delayBeforeMs": null, "combo": { "modifiers": [], "key": "KeyQ" } }],
        }))
        .unwrap();
        tracker
    }

    #[test]
    fn network_disconnect_with_pixels_is_complete_and_an_empty_sequence_is_not() {
        use crate::action::ActionKind;
        let mut disconnect = complete_tracker();
        disconnect.action.kind = ActionKind::NetworkDisconnect;
        let mut empty_sequence = complete_tracker();
        empty_sequence.action = Action::default();

        assert_eq!(disconnect.missing_part(), None);
        assert!(disconnect.is_complete());
        assert_eq!(
            empty_sequence.missing_part(),
            Some(MissingPart::ActionSteps)
        );
        assert!(!empty_sequence.is_complete());
    }

    #[test]
    fn running_tracker_stops_when_the_action_becomes_an_empty_sequence() {
        use crate::action::ActionKind;
        let mut tracker = complete_tracker();
        tracker.is_running = true;
        tracker.action.kind = ActionKind::NetworkDisconnect;
        tracker.stop_if_incomplete();
        assert!(tracker.is_running);

        tracker.action = Action::default();
        tracker.stop_if_incomplete();
        assert!(!tracker.is_running);
    }

    #[test]
    fn tracker_is_complete_with_pixels_and_action_steps() {
        let complete = complete_tracker();
        let mut without_pixels = complete.clone();
        without_pixels.pixels.clear();
        let mut without_steps = complete.clone();
        without_steps.action = Action::default();

        assert_eq!(complete.missing_part(), None);
        assert_eq!(without_pixels.missing_part(), Some(MissingPart::Pixels));
        assert_eq!(without_steps.missing_part(), Some(MissingPart::ActionSteps));
        assert_eq!(
            Tracker::new("Новое".into()).missing_part(),
            Some(MissingPart::Pixels)
        );
    }

    #[test]
    fn incomplete_tracker_stops() {
        let mut tracker = complete_tracker();
        tracker.is_running = true;

        tracker.stop_if_incomplete();
        assert!(tracker.is_running);

        tracker.pixels.clear();
        tracker.stop_if_incomplete();
        assert!(!tracker.is_running);
    }

    fn hotkey_combo() -> KeyCombo {
        KeyCombo {
            modifiers: vec![crate::action::Modifier::Ctrl],
            key: "Digit1".into(),
        }
    }

    #[test]
    fn hotkey_tracker_is_complete_without_pixels_when_the_combo_and_action_are_ready() {
        let mut ready = complete_tracker();
        ready.trigger = Trigger::Hotkey;
        ready.hotkey = Some(hotkey_combo());
        ready.pixels.clear();
        let mut without_combo = ready.clone();
        without_combo.hotkey = None;

        assert_eq!(ready.missing_part(), None);
        assert!(ready.is_complete());
        assert_eq!(without_combo.missing_part(), Some(MissingPart::Hotkey));
        assert!(!without_combo.is_complete());
    }

    #[test]
    fn clearing_the_combo_or_switching_to_an_unready_variant_stops_a_running_tracker() {
        let mut pixels = complete_tracker();
        pixels.is_running = true;
        pixels.hotkey = Some(hotkey_combo());

        pixels.trigger = Trigger::Hotkey;
        pixels.stop_if_incomplete();
        assert!(pixels.is_running, "a ready hotkey variant stays running");

        pixels.hotkey = None;
        pixels.stop_if_incomplete();
        assert!(!pixels.is_running);

        let mut running_pixels = complete_tracker();
        running_pixels.is_running = true;
        running_pixels.trigger = Trigger::Hotkey;
        running_pixels.stop_if_incomplete();
        assert!(!running_pixels.is_running, "switching to a hotkey without a combo stops");

        let mut back = complete_tracker();
        back.is_running = true;
        back.trigger = Trigger::Hotkey;
        back.hotkey = Some(hotkey_combo());
        back.pixels.clear();
        back.stop_if_incomplete();
        assert!(back.is_running);
        back.trigger = Trigger::Pixels;
        back.stop_if_incomplete();
        assert!(!back.is_running, "switching to pixels without pixels stops");
    }

    #[test]
    fn serializes_pixel_in_camel_case() {
        let id = Uuid::new_v4();
        let mut pixel = Pixel::new(
            id,
            812,
            1040,
            Rgb {
                r: 200,
                g: 30,
                b: 30,
            },
        );
        pixel.condition = PixelCondition::NotEquals;

        assert_eq!(
            serde_json::to_value(&pixel).unwrap(),
            json!({
                "id": id,
                "x": 812,
                "y": 1040,
                "color": { "r": 200, "g": 30, "b": 30 },
                "condition": "notEquals",
                "tolerancePercent": 10,
            })
        );
    }

    #[test]
    fn rejects_tolerance_above_100_percent() {
        let mut pixel = Pixel::new(Uuid::new_v4(), 0, 0, Rgb { r: 0, g: 0, b: 0 });

        pixel.tolerance_percent = 100;
        assert!(pixel.validate().is_ok());
        pixel.tolerance_percent = 101;
        assert!(pixel.validate().is_err());
    }

    #[test]
    fn rejects_negative_values_when_deserializing() {
        let negative_pause = tracker_json(json!({ "min": 100, "max": 300 }), -100);
        let negative_delay = tracker_json(json!({ "min": -1, "max": 300 }), 1000);

        assert!(serde_json::from_value::<Tracker>(negative_pause).is_err());
        assert!(serde_json::from_value::<Tracker>(negative_delay).is_err());
    }

    #[test]
    fn accepts_zero_pause() {
        let tracker: Tracker =
            serde_json::from_value(tracker_json(json!({ "min": 0, "max": 0 }), 0)).unwrap();

        assert!(tracker.validate().is_ok());
    }

    #[test]
    fn rejects_blank_profile_fields() {
        assert!(Profile::new("  ".into(), "game.exe".into())
            .validate()
            .is_err());
        assert!(Profile::new("Diablo".into(), "".into()).validate().is_err());
        assert!(Profile::new("Diablo".into(), "game.exe".into())
            .validate()
            .is_ok());
    }

    #[test]
    fn rejects_blank_tracker_name() {
        assert!(Tracker::new(" ".into()).validate().is_err());
    }

    #[test]
    fn rejects_min_greater_than_max() {
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.pre_action_delay_ms = DelayRange { min: 500, max: 200 };

        assert!(tracker.validate().is_err());

        tracker.pre_action_delay_ms = DelayRange { min: 200, max: 200 };
        assert!(tracker.validate().is_ok());
    }
}
