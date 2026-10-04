//! Plays a key sequence as keyboard input with natural timing.

use std::time::Duration;

use rand::{Rng, RngExt};

use crate::action::{KeySequenceAction, KeyStep};
use crate::error::{AppError, AppResult};
use crate::key_codes::{self, KeyDef};

const HOLD_MS: (u64, u64) = (40, 90);
/// Between modifier presses, the last modifier and the key, and between releases.
const MODIFIER_GAP_MS: (u64, u64) = (15, 35);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyEvent {
    pub scan_code: u16,
    pub is_extended: bool,
    pub is_up: bool,
}

pub trait InputSink {
    fn send(&mut self, event: KeyEvent);
}

pub trait Sleeper {
    fn sleep(&self, duration: Duration);
}

/// Fails before sending anything when a step has an unsupported key, otherwise runs every step.
pub fn execute_key_sequence(
    action: &KeySequenceAction,
    input: &mut dyn InputSink,
    clock: &dyn Sleeper,
    rng: &mut impl Rng,
) -> AppResult<()> {
    let steps = action
        .steps
        .iter()
        .map(|step| {
            key_codes::lookup(&step.combo.key)
                .map(|key| (step, key))
                .ok_or_else(|| {
                    AppError::Validation(format!("Клавиша {} не поддерживается", step.combo.key))
                })
        })
        .collect::<AppResult<Vec<_>>>()?;

    for (step, key) in steps {
        if let Some(delay) = step.delay_before_ms {
            let ms = rng.random_range(delay.min..=delay.max);
            clock.sleep(Duration::from_millis(ms.into()));
        }
        press_combo(step, key, input, clock, rng);
    }
    Ok(())
}

fn press_combo(
    step: &KeyStep,
    key: &KeyDef,
    input: &mut dyn InputSink,
    clock: &dyn Sleeper,
    rng: &mut impl Rng,
) {
    let modifiers: Vec<_> = step
        .combo
        .modifiers
        .iter()
        .map(|modifier| key_codes::modifier_key(*modifier))
        .collect();

    for modifier in &modifiers {
        input.send(event(modifier, false));
        clock.sleep(bell(rng, MODIFIER_GAP_MS));
    }
    input.send(event(key, false));
    clock.sleep(bell(rng, HOLD_MS));
    input.send(event(key, true));
    for modifier in modifiers.iter().rev() {
        clock.sleep(bell(rng, MODIFIER_GAP_MS));
        input.send(event(modifier, true));
    }
}

fn event(key: &KeyDef, is_up: bool) -> KeyEvent {
    KeyEvent {
        scan_code: key.scan_code,
        is_extended: key.is_extended,
        is_up,
    }
}

/// Irwin–Hall with three terms mapped onto `[min, max]`: values cluster around the middle.
fn bell(rng: &mut impl Rng, (min, max): (u64, u64)) -> Duration {
    let t = (0..3).map(|_| rng.random::<f64>()).sum::<f64>() / 3.0;
    Duration::from_millis(min + (t * (max - min) as f64).round() as u64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{KeyCombo, Modifier};
    use crate::config::DelayRange;
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::cell::RefCell;
    use std::rc::Rc;
    use uuid::Uuid;

    #[derive(Debug, Clone, Copy, PartialEq)]
    enum Entry {
        Key(KeyEvent),
        Sleep(Duration),
    }

    /// Input and clock writing into one log, so sleeps can be placed between key events.
    #[derive(Default, Clone)]
    struct Recorder(Rc<RefCell<Vec<Entry>>>);

    impl InputSink for Recorder {
        fn send(&mut self, event: KeyEvent) {
            self.0.borrow_mut().push(Entry::Key(event));
        }
    }

    impl Sleeper for Recorder {
        fn sleep(&self, duration: Duration) {
            self.0.borrow_mut().push(Entry::Sleep(duration));
        }
    }

    impl Recorder {
        fn run(action: &KeySequenceAction, seed: u64) -> Vec<Entry> {
            let recorder = Recorder::default();
            let mut input = recorder.clone();
            execute_key_sequence(action, &mut input, &recorder, &mut StdRng::seed_from_u64(seed))
                .unwrap();
            recorder.0.take()
        }
    }

    fn keys(entries: &[Entry]) -> Vec<(u16, bool)> {
        entries
            .iter()
            .filter_map(|entry| match entry {
                Entry::Key(event) => Some((event.scan_code, event.is_up)),
                Entry::Sleep(_) => None,
            })
            .collect()
    }

    /// Total sleep between the `from`-th and the `to`-th key event.
    fn time_between(entries: &[Entry], from: usize, to: usize) -> Duration {
        let positions: Vec<_> = entries
            .iter()
            .enumerate()
            .filter(|(_, entry)| matches!(entry, Entry::Key(_)))
            .map(|(index, _)| index)
            .collect();
        entries[positions[from]..positions[to]]
            .iter()
            .map(|entry| match entry {
                Entry::Sleep(duration) => *duration,
                Entry::Key(_) => Duration::ZERO,
            })
            .sum()
    }

    fn step(key: &str, modifiers: Vec<Modifier>, delay: Option<(u32, u32)>) -> KeyStep {
        KeyStep {
            id: Uuid::new_v4(),
            delay_before_ms: delay.map(|(min, max)| DelayRange { min, max }),
            combo: KeyCombo {
                modifiers,
                key: key.into(),
            },
        }
    }

    fn sequence(steps: Vec<KeyStep>) -> KeySequenceAction {
        KeySequenceAction { steps }
    }

    const DOWN: bool = false;
    const UP: bool = true;

    #[test]
    fn presses_modifiers_around_the_key() {
        let action = sequence(vec![step("Digit1", vec![Modifier::Ctrl, Modifier::Shift], None)]);

        let entries = Recorder::run(&action, 1);

        assert_eq!(
            keys(&entries),
            [(0x1D, DOWN), (0x2A, DOWN), (0x02, DOWN), (0x02, UP), (0x2A, UP), (0x1D, UP)]
        );
        for (from, to) in [(0, 1), (1, 2), (3, 4), (4, 5)] {
            let gap = time_between(&entries, from, to);
            assert!((15..=35).contains(&gap.as_millis()), "{from}→{to}: {gap:?}");
        }
    }

    #[test]
    fn waits_a_random_delay_from_the_step_range() {
        let action = sequence(vec![step("KeyQ", vec![], None), step("KeyW", vec![], Some((300, 600)))]);

        let delays: Vec<_> = (0..200)
            .map(|seed| time_between(&Recorder::run(&action, seed), 1, 2).as_millis())
            .collect();

        assert!(delays.iter().all(|delay| (300..=600).contains(delay)), "{delays:?}");
        assert!(delays.iter().any(|delay| *delay < 400));
        assert!(delays.iter().any(|delay| *delay > 500));
    }

    #[test]
    fn holds_the_key_for_a_varying_time_within_limits() {
        let action = sequence(vec![step("KeyQ", vec![], None)]);

        let holds: Vec<_> = (0..200)
            .map(|seed| time_between(&Recorder::run(&action, seed), 0, 1).as_millis())
            .collect();

        assert!(holds.iter().all(|hold| (40..=90).contains(hold)), "{holds:?}");
        assert!(holds.iter().any(|hold| *hold != holds[0]));
    }

    #[test]
    fn runs_every_step_in_order() {
        let action = sequence(vec![
            step("KeyQ", vec![], None),
            step("ArrowUp", vec![], Some((0, 0))),
            step("F5", vec![], Some((10, 20))),
        ]);

        let entries = Recorder::run(&action, 7);

        assert_eq!(
            keys(&entries),
            [(0x10, DOWN), (0x10, UP), (0x48, DOWN), (0x48, UP), (0x3F, DOWN), (0x3F, UP)]
        );
        let arrow = entries.iter().find_map(|entry| match entry {
            Entry::Key(event) if event.scan_code == 0x48 => Some(event.is_extended),
            _ => None,
        });
        assert_eq!(arrow, Some(true));
    }

    #[test]
    fn rejects_unsupported_key_before_any_input() {
        let action = sequence(vec![step("KeyQ", vec![], None), step("MetaLeft", vec![], Some((0, 0)))]);
        let recorder = Recorder::default();
        let mut input = recorder.clone();

        let result =
            execute_key_sequence(&action, &mut input, &recorder, &mut StdRng::seed_from_u64(0));

        assert!(matches!(result, Err(AppError::Validation(_))));
        assert!(recorder.0.borrow().is_empty());
    }
}
