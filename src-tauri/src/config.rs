use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

pub const CONFIG_VERSION: u32 = 1;
pub const DEFAULT_RESUME_DELAY_MS: u32 = 1000;
pub const DEFAULT_TOLERANCE_PERCENT: u8 = 10;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    pub version: u32,
    pub profiles: Vec<Profile>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            version: CONFIG_VERSION,
            profiles: Vec::new(),
        }
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

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Tracker {
    pub id: Uuid,
    pub name: String,
    pub pre_action_delay_ms: DelayRange,
    pub resume_delay_ms: u32,
    #[serde(default)]
    pub pixels: Vec<Pixel>,
}

impl Tracker {
    pub fn new(name: String) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            pre_action_delay_ms: DelayRange::default(),
            resume_delay_ms: DEFAULT_RESUME_DELAY_MS,
            pixels: Vec::new(),
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        validate_name(&self.name, "Имя отслеживания не может быть пустым")?;
        self.pre_action_delay_ms.validate()
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

    fn tracker_json(pre_action_delay_ms: serde_json::Value, resume_delay_ms: i64) -> serde_json::Value {
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

        assert_eq!(tracker.pre_action_delay_ms, DelayRange { min: 100, max: 300 });
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
                    }],
                }],
            })
        );
        assert_eq!(serde_json::from_value::<Config>(value).unwrap(), config);
    }

    #[test]
    fn reads_tracker_saved_before_pixels_existed() {
        let tracker: Tracker =
            serde_json::from_value(tracker_json(json!({ "min": 100, "max": 300 }), 1000)).unwrap();

        assert!(tracker.pixels.is_empty());
    }

    #[test]
    fn serializes_pixel_in_camel_case() {
        let id = Uuid::new_v4();
        let mut pixel = Pixel::new(id, 812, 1040, Rgb { r: 200, g: 30, b: 30 });
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
        assert!(Profile::new("  ".into(), "game.exe".into()).validate().is_err());
        assert!(Profile::new("Diablo".into(), "".into()).validate().is_err());
        assert!(Profile::new("Diablo".into(), "game.exe".into()).validate().is_ok());
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
