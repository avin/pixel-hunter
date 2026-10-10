use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::DelayRange;
use crate::error::{AppError, AppResult};
use crate::key_codes;

pub const DEFAULT_STEP_DELAY_MS: DelayRange = DelayRange { min: 300, max: 600 };
pub const DEFAULT_STEP_HOLD_MS: DelayRange = DelayRange { min: 40, max: 90 };

fn default_step_hold_ms() -> DelayRange {
    DEFAULT_STEP_HOLD_MS
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Action {
    #[serde(flatten)]
    pub kind: ActionKind,
    #[serde(default)]
    pub sound: SoundSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ActionKind {
    KeySequence(KeySequenceAction),
    NetworkDisconnect,
}

impl Default for ActionKind {
    fn default() -> Self {
        Self::KeySequence(KeySequenceAction::default())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SoundSettings {
    pub id: Option<String>,
    pub volume_percent: u8,
}

impl Default for SoundSettings {
    fn default() -> Self {
        Self {
            id: None,
            volume_percent: 50,
        }
    }
}

impl SoundSettings {
    pub fn validate(&self) -> AppResult<()> {
        if self.volume_percent > 100 {
            return Err(AppError::Validation(
                "Громкость должна быть от 0 до 100%".into(),
            ));
        }
        if let Some(id) = &self.id {
            if !crate::action_sound::is_known_id(id) {
                return Err(AppError::Validation(format!("Звук {id} не поддерживается")));
            }
        }
        Ok(())
    }
}

impl Action {
    #[cfg(test)]
    pub fn key_sequence(sequence: KeySequenceAction) -> Self {
        Self {
            kind: ActionKind::KeySequence(sequence),
            sound: SoundSettings::default(),
        }
    }
    pub fn normalize(&mut self) {
        match &mut self.kind {
            ActionKind::KeySequence(action) => action.normalize(),
            ActionKind::NetworkDisconnect => {}
        }
    }
    pub fn validate(&self) -> AppResult<()> {
        self.sound.validate()?;
        match &self.kind {
            ActionKind::KeySequence(action) => action.validate(),
            ActionKind::NetworkDisconnect => Ok(()),
        }
    }
    pub fn is_empty(&self) -> bool {
        match &self.kind {
            ActionKind::KeySequence(action) => action.steps.is_empty(),
            ActionKind::NetworkDisconnect => false,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeySequenceAction {
    pub steps: Vec<KeyStep>,
}

impl KeySequenceAction {
    /// The first step has no delay and every later one has it; modifiers are unique,
    /// ordered Ctrl, Shift, Alt and never repeat the modifier pressed as the key itself.
    pub fn normalize(&mut self) {
        for (index, step) in self.steps.iter_mut().enumerate() {
            if index == 0 {
                step.delay_before_ms = None;
            } else if step.delay_before_ms.is_none() {
                step.delay_before_ms = Some(DEFAULT_STEP_DELAY_MS);
            }
            step.combo.normalize();
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        for step in &self.steps {
            if key_codes::lookup(&step.combo.key).is_none() {
                return Err(AppError::Validation(format!(
                    "Клавиша {} не поддерживается",
                    step.combo.key
                )));
            }
            if let Some(delay) = &step.delay_before_ms {
                delay.validate()?;
            }
            step.hold_ms.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStep {
    pub id: Uuid,
    pub delay_before_ms: Option<DelayRange>,
    #[serde(default = "default_step_hold_ms")]
    pub hold_ms: DelayRange,
    pub combo: KeyCombo,
}

/// `key` is a physical key in `KeyboardEvent.code` terms, e.g. `KeyQ` or `ShiftLeft`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyCombo {
    pub modifiers: Vec<Modifier>,
    pub key: String,
}

impl KeyCombo {
    fn normalize(&mut self) {
        let own = key_codes::modifier_of(&self.key);
        self.modifiers.retain(|modifier| Some(*modifier) != own);
        self.modifiers.sort();
        self.modifiers.dedup();
    }
}

/// Declaration order is the order modifiers are pressed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Modifier {
    Ctrl,
    Shift,
    Alt,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_missing_and_partial_sound_settings() {
        for (sound, expected) in [
            (None, SoundSettings::default()),
            (Some(json!({})), SoundSettings::default()),
            (
                Some(json!({ "id": "soft-ping" })),
                SoundSettings {
                    id: Some("soft-ping".into()),
                    volume_percent: 50,
                },
            ),
            (
                Some(json!({ "volumePercent": 20 })),
                SoundSettings {
                    id: None,
                    volume_percent: 20,
                },
            ),
        ] {
            let mut value = json!({ "type": "keySequence", "steps": [] });
            if let Some(sound) = sound {
                value["sound"] = sound;
            }
            let action: Action = serde_json::from_value(value).unwrap();
            assert_eq!(action.sound, expected);
            assert!(action.is_empty());
        }
    }

    #[test]
    fn validates_sound_id_and_volume_boundaries() {
        for volume_percent in [0, 100] {
            assert!(SoundSettings {
                id: Some("soft-ping".into()),
                volume_percent
            }
            .validate()
            .is_ok());
        }
        assert!(SoundSettings {
            id: Some("unknown-sound".into()),
            volume_percent: 50
        }
        .validate()
        .is_err());
        assert!(SoundSettings {
            id: None,
            volume_percent: 101
        }
        .validate()
        .is_err());
        for volume in [json!(-1), json!(50.5), json!(256)] {
            assert!(serde_json::from_value::<Action>(json!({
                "type": "keySequence", "steps": [], "sound": { "volumePercent": volume }
            }))
            .is_err());
        }
    }

    fn step(key: &str, modifiers: Vec<Modifier>, delay: Option<DelayRange>) -> KeyStep {
        KeyStep {
            id: Uuid::new_v4(),
            delay_before_ms: delay,
            hold_ms: DEFAULT_STEP_HOLD_MS,
            combo: KeyCombo {
                modifiers,
                key: key.into(),
            },
        }
    }

    fn sequence(steps: Vec<KeyStep>) -> KeySequenceAction {
        KeySequenceAction { steps }
    }

    #[test]
    fn serializes_tagged_by_type() {
        let first = step("KeyQ", vec![], None);
        let second = step(
            "Digit1",
            vec![Modifier::Ctrl],
            Some(DelayRange { min: 300, max: 600 }),
        );
        let action = Action::key_sequence(sequence(vec![first.clone(), second.clone()]));

        let value = serde_json::to_value(&action).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "keySequence",
                "sound": { "id": null, "volumePercent": 50 },
                "steps": [
                    { "id": first.id, "delayBeforeMs": null, "holdMs": { "min": 40, "max": 90 }, "combo": { "modifiers": [], "key": "KeyQ" } },
                    {
                        "id": second.id,
                        "delayBeforeMs": { "min": 300, "max": 600 },
                        "holdMs": { "min": 40, "max": 90 },
                        "combo": { "modifiers": ["ctrl"], "key": "Digit1" },
                    },
                ],
            })
        );
        assert_eq!(serde_json::from_value::<Action>(value).unwrap(), action);
    }

    #[test]
    fn drops_delay_of_the_new_first_step() {
        let mut action = sequence(vec![
            step("KeyQ", vec![], None),
            step("KeyW", vec![], Some(DelayRange { min: 300, max: 600 })),
        ]);
        action.steps[1].hold_ms = DelayRange { min: 1000, max: 1500 };

        action.steps.remove(0);
        action.normalize();

        assert_eq!(action.steps[0].delay_before_ms, None);
        assert_eq!(
            action.steps[0].hold_ms,
            DelayRange { min: 1000, max: 1500 }
        );
    }

    #[test]
    fn validates_hold_range_boundaries() {
        for (min, max) in [
            (0, 0),
            (1200, 1200),
            (2000, 3000),
            (0, u32::MAX),
            (u32::MAX, u32::MAX),
        ] {
            let mut action = sequence(vec![step("KeyQ", vec![], None)]);
            action.steps[0].hold_ms = DelayRange { min, max };
            assert!(action.validate().is_ok());
        }
        let mut action = sequence(vec![step("KeyQ", vec![], None)]);
        action.steps[0].hold_ms = DelayRange { min: 3000, max: 2000 };
        assert!(matches!(action.validate(), Err(AppError::Validation(_))));
    }

    #[test]
    fn reads_missing_and_explicit_hold_ranges() {
        let mut value = json!({
            "id": Uuid::new_v4(), "delayBeforeMs": null,
            "combo": { "modifiers": [], "key": "KeyQ" }
        });
        let old_step: KeyStep = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(old_step.hold_ms, DEFAULT_STEP_HOLD_MS);
        value["holdMs"] = json!({ "min": 2000, "max": 3000 });
        let explicit: KeyStep = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(
            explicit.hold_ms,
            DelayRange { min: 2000, max: 3000 }
        );
        assert_eq!(
            serde_json::to_value(explicit).unwrap()["holdMs"],
            value["holdMs"]
        );
    }

    #[test]
    fn rejects_unsupported_hold_boundaries_in_json() {
        for boundary in [json!(-1), json!(40.5), json!(4294967296_u64)] {
            for field in ["min", "max"] {
                let mut value = serde_json::to_value(step("KeyQ", vec![], None)).unwrap();
                value["holdMs"][field] = boundary.clone();
                assert!(serde_json::from_value::<KeyStep>(value).is_err());
            }
        }
    }

    #[test]
    fn gives_later_steps_the_default_delay() {
        let mut action = sequence(vec![step("KeyQ", vec![], None), step("KeyW", vec![], None)]);

        action.normalize();

        assert_eq!(action.steps[1].delay_before_ms, Some(DEFAULT_STEP_DELAY_MS));
    }

    #[test]
    fn orders_modifiers_canonically() {
        let mut action = sequence(vec![step(
            "Digit1",
            vec![
                Modifier::Alt,
                Modifier::Ctrl,
                Modifier::Shift,
                Modifier::Ctrl,
            ],
            None,
        )]);

        action.normalize();

        assert_eq!(
            action.steps[0].combo.modifiers,
            [Modifier::Ctrl, Modifier::Shift, Modifier::Alt]
        );
    }

    #[test]
    fn drops_modifier_pressed_as_the_key() {
        let mut action = sequence(vec![step(
            "ShiftLeft",
            vec![Modifier::Shift, Modifier::Ctrl],
            None,
        )]);

        action.normalize();

        assert_eq!(action.steps[0].combo.modifiers, [Modifier::Ctrl]);
    }

    #[test]
    fn rejects_unknown_key_code() {
        let action = sequence(vec![step("MetaLeft", vec![], None)]);

        assert!(matches!(action.validate(), Err(AppError::Validation(_))));
    }

    #[test]
    fn roundtrips_network_disconnect() {
        let action = Action {
            kind: ActionKind::NetworkDisconnect,
            sound: SoundSettings {
                id: Some("arcade-coin".into()),
                volume_percent: 40,
            },
        };

        let value = serde_json::to_value(&action).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "networkDisconnect",
                "sound": { "id": "arcade-coin", "volumePercent": 40 },
            })
        );
        assert_eq!(serde_json::from_value::<Action>(value).unwrap(), action);
        assert!(!action.is_empty());
    }

    #[test]
    fn reads_a_profile_that_only_has_key_sequences() {
        let profile: crate::config::Profile = serde_json::from_value(json!({
            "id": "6f1b9a52-7f0c-4f0e-9d7b-4a3b1e2c5d6f",
            "name": "Diablo",
            "processName": "game.exe",
            "trackers": [{
                "id": "11111111-1111-1111-1111-111111111111",
                "name": "Здоровье",
                "preActionDelayMs": { "min": 100, "max": 300 },
                "resumeDelayMs": 1000,
                "action": {
                    "type": "keySequence",
                    "steps": [{
                        "id": "22222222-2222-2222-2222-222222222222",
                        "delayBeforeMs": null,
                        "combo": { "modifiers": [], "key": "KeyQ" }
                    }],
                    "sound": { "id": null, "volumePercent": 50 }
                }
            }]
        }))
        .unwrap();

        let ActionKind::KeySequence(sequence) = &profile.trackers[0].action.kind else {
            panic!("a profile saved with key sequences stays a key sequence");
        };
        assert_eq!(sequence.steps.len(), 1);
        assert_eq!(sequence.steps[0].combo.key, "KeyQ");
    }

    #[test]
    fn rejects_an_invalid_sound_on_network_disconnect() {
        assert!(Action {
            kind: ActionKind::NetworkDisconnect,
            sound: SoundSettings {
                id: Some("unknown-sound".into()),
                volume_percent: 50,
            },
        }
        .validate()
        .is_err());
        assert!(Action {
            kind: ActionKind::NetworkDisconnect,
            sound: SoundSettings {
                id: None,
                volume_percent: 101,
            },
        }
        .validate()
        .is_err());
    }

    #[test]
    fn rejects_reversed_delay_range() {
        let action = sequence(vec![
            step("KeyQ", vec![], None),
            step("KeyW", vec![], Some(DelayRange { min: 600, max: 300 })),
        ]);

        assert!(matches!(action.validate(), Err(AppError::Validation(_))));
    }
}
