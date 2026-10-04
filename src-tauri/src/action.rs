use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::DelayRange;
use crate::error::{AppError, AppResult};
use crate::key_codes;

pub const DEFAULT_STEP_DELAY_MS: DelayRange = DelayRange { min: 300, max: 600 };

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Action {
    KeySequence(KeySequenceAction),
}

impl Default for Action {
    fn default() -> Self {
        Action::KeySequence(KeySequenceAction::default())
    }
}

impl Action {
    pub fn normalize(&mut self) {
        match self {
            Action::KeySequence(action) => action.normalize(),
        }
    }

    pub fn validate(&self) -> AppResult<()> {
        match self {
            Action::KeySequence(action) => action.validate(),
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
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct KeyStep {
    pub id: Uuid,
    pub delay_before_ms: Option<DelayRange>,
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

    fn step(key: &str, modifiers: Vec<Modifier>, delay: Option<DelayRange>) -> KeyStep {
        KeyStep {
            id: Uuid::new_v4(),
            delay_before_ms: delay,
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
        let second = step("Digit1", vec![Modifier::Ctrl], Some(DelayRange { min: 300, max: 600 }));
        let action = Action::KeySequence(sequence(vec![first.clone(), second.clone()]));

        let value = serde_json::to_value(&action).unwrap();

        assert_eq!(
            value,
            json!({
                "type": "keySequence",
                "steps": [
                    { "id": first.id, "delayBeforeMs": null, "combo": { "modifiers": [], "key": "KeyQ" } },
                    {
                        "id": second.id,
                        "delayBeforeMs": { "min": 300, "max": 600 },
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

        action.steps.remove(0);
        action.normalize();

        assert_eq!(action.steps[0].delay_before_ms, None);
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
            vec![Modifier::Alt, Modifier::Ctrl, Modifier::Shift, Modifier::Ctrl],
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
        let mut action = sequence(vec![step("ShiftLeft", vec![Modifier::Shift, Modifier::Ctrl], None)]);

        action.normalize();

        assert_eq!(action.steps[0].combo.modifiers, [Modifier::Ctrl]);
    }

    #[test]
    fn rejects_unknown_key_code() {
        let action = sequence(vec![step("MetaLeft", vec![], None)]);

        assert!(matches!(action.validate(), Err(AppError::Validation(_))));
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
