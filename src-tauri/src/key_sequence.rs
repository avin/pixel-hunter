//! Plays a key sequence as keyboard input with natural timing.

use std::time::Duration;

use rand::{Rng, RngExt};

use crate::action::{KeySequenceAction, KeyStep};
use crate::error::{AppError, AppResult};
use crate::key_codes::{self, KeyDef};

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
#[cfg(test)]
pub fn execute_key_sequence(
    action: &KeySequenceAction,
    input: &mut dyn InputSink,
    clock: &dyn Sleeper,
    rng: &mut impl Rng,
) -> AppResult<()> {
    execute_key_sequence_until(action, input, clock, rng, &|| false)
}

/// Stops on capture pause and releases every key already pressed before returning.
pub fn execute_key_sequence_until(
    action: &KeySequenceAction,
    input: &mut dyn InputSink,
    clock: &dyn Sleeper,
    rng: &mut impl Rng,
    interrupted: &dyn Fn() -> bool,
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
        if interrupted() {
            break;
        }
        if let Some(delay) = step.delay_before_ms {
            let ms = rng.random_range(delay.min..=delay.max);
            if !sleep_until(clock, Duration::from_millis(ms.into()), interrupted) {
                break;
            }
        }
        if !press_combo(step, key, input, clock, rng, interrupted) {
            break;
        }
    }
    Ok(())
}

fn press_combo(
    step: &KeyStep,
    key: &KeyDef,
    input: &mut dyn InputSink,
    clock: &dyn Sleeper,
    rng: &mut impl Rng,
    interrupted: &dyn Fn() -> bool,
) -> bool {
    let modifiers: Vec<_> = step
        .combo
        .modifiers
        .iter()
        .map(|modifier| key_codes::modifier_key(*modifier))
        .collect();

    let mut held = Vec::new();
    for modifier in &modifiers {
        if interrupted() {
            release_keys(input, &held);
            return false;
        }
        input.send(event(modifier, false));
        held.push(*modifier);
        if !sleep_until(clock, bell(rng, MODIFIER_GAP_MS), interrupted) {
            release_keys(input, &held);
            return false;
        }
    }
    if interrupted() {
        release_keys(input, &held);
        return false;
    }
    input.send(event(key, false));
    held.push(key);
    // The step controls the main key's hold; modifier gaps keep their own timing.
    if !sleep_until(
        clock,
        bell(rng, (step.hold_ms.min.into(), step.hold_ms.max.into())),
        interrupted,
    ) {
        release_keys(input, &held);
        return false;
    }
    input.send(event(key, true));
    held.pop();
    for modifier in modifiers.iter().rev() {
        if !sleep_until(clock, bell(rng, MODIFIER_GAP_MS), interrupted) {
            release_keys(input, &held);
            return false;
        }
        input.send(event(modifier, true));
        held.pop();
    }
    true
}

fn release_keys(input: &mut dyn InputSink, held: &[&KeyDef]) {
    for key in held.iter().rev() {
        input.send(event(key, true));
    }
}

fn sleep_until(
    clock: &dyn Sleeper,
    mut duration: Duration,
    interrupted: &dyn Fn() -> bool,
) -> bool {
    while !duration.is_zero() {
        if interrupted() {
            return false;
        }
        let chunk = duration.min(Duration::from_millis(10));
        clock.sleep(chunk);
        duration -= chunk;
    }
    !interrupted()
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
            execute_key_sequence(
                action,
                &mut input,
                &recorder,
                &mut StdRng::seed_from_u64(seed),
            )
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
            hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
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
    fn capture_pause_releases_held_keys_and_skips_remaining_steps() {
        use std::cell::Cell;
        struct PauseClock<'a>(&'a Cell<bool>);
        impl Sleeper for PauseClock<'_> {
            fn sleep(&self, _: Duration) {
                self.0.set(true);
            }
        }
        for modifiers in [vec![], vec![Modifier::Ctrl, Modifier::Shift]] {
            let paused = Cell::new(false);
            let recorder = Recorder::default();
            let action = sequence(vec![
                step("KeyQ", modifiers.clone(), None),
                step("KeyW", vec![], Some((1000, 1000))),
            ]);
            execute_key_sequence_until(
                &action,
                &mut recorder.clone(),
                &PauseClock(&paused),
                &mut StdRng::seed_from_u64(1),
                &|| paused.get(),
            )
            .unwrap();
            let first_code = if modifiers.is_empty() { 0x10 } else { 0x1D };
            assert_eq!(
                keys(&recorder.0.borrow()),
                [(first_code, DOWN), (first_code, UP)]
            );
        }
    }

    #[test]
    fn capture_pause_during_delay_never_presses_the_next_key() {
        use std::cell::Cell;
        struct PauseClock<'a>(&'a Cell<bool>);
        impl Sleeper for PauseClock<'_> {
            fn sleep(&self, _: Duration) {
                self.0.set(true);
            }
        }
        let paused = Cell::new(false);
        let recorder = Recorder::default();
        let action = sequence(vec![step("KeyQ", vec![], Some((1000, 1000)))]);
        execute_key_sequence_until(
            &action,
            &mut recorder.clone(),
            &PauseClock(&paused),
            &mut StdRng::seed_from_u64(1),
            &|| paused.get(),
        )
        .unwrap();
        assert!(keys(&recorder.0.borrow()).is_empty());
    }

    #[test]
    fn presses_modifiers_around_the_key() {
        let action = sequence(vec![step(
            "Digit1",
            vec![Modifier::Ctrl, Modifier::Shift],
            None,
        )]);

        let entries = Recorder::run(&action, 1);

        assert_eq!(
            keys(&entries),
            [
                (0x1D, DOWN),
                (0x2A, DOWN),
                (0x02, DOWN),
                (0x02, UP),
                (0x2A, UP),
                (0x1D, UP)
            ]
        );
        for (from, to) in [(0, 1), (1, 2), (3, 4), (4, 5)] {
            let gap = time_between(&entries, from, to);
            assert!((15..=35).contains(&gap.as_millis()), "{from}→{to}: {gap:?}");
        }
    }

    #[test]
    fn waits_a_random_delay_from_the_step_range() {
        let action = sequence(vec![
            step("KeyQ", vec![], None),
            step("KeyW", vec![], Some((300, 600))),
        ]);

        let delays: Vec<_> = (0..200)
            .map(|seed| time_between(&Recorder::run(&action, seed), 1, 2).as_millis())
            .collect();

        assert!(
            delays.iter().all(|delay| (300..=600).contains(delay)),
            "{delays:?}"
        );
        assert!(delays.iter().any(|delay| *delay < 400));
        assert!(delays.iter().any(|delay| *delay > 500));
    }

    #[test]
    fn holds_the_key_for_a_varying_time_within_limits() {
        let action = sequence(vec![step("KeyQ", vec![], None)]);

        let holds: Vec<_> = (0..200)
            .map(|seed| time_between(&Recorder::run(&action, seed), 0, 1).as_millis())
            .collect();

        assert!(
            holds.iter().all(|hold| (40..=90).contains(hold)),
            "{holds:?}"
        );
        assert!(holds.iter().any(|hold| *hold != holds[0]));
    }

    #[test]
    fn holds_each_step_within_its_own_range() {
        let mut action = sequence(vec![
            step("KeyQ", vec![], None),
            step("KeyW", vec![], Some((300, 600))),
        ]);
        action.steps[0].hold_ms = DelayRange {
            min: 1000,
            max: 1500,
        };
        action.steps[1].hold_ms = DelayRange { min: 200, max: 300 };
        for seed in 0..100 {
            let entries = Recorder::run(&action, seed);
            assert!((1000..=1500).contains(&time_between(&entries, 0, 1).as_millis()));
            assert!((200..=300).contains(&time_between(&entries, 2, 3).as_millis()));
        }
    }

    #[test]
    fn fixed_and_zero_holds_are_exact() {
        for hold in [0, 1200] {
            let mut action = sequence(vec![step("KeyQ", vec![], None)]);
            action.steps[0].hold_ms = DelayRange {
                min: hold,
                max: hold,
            };
            for seed in 0..10 {
                assert_eq!(
                    time_between(&Recorder::run(&action, seed), 0, 1),
                    Duration::from_millis(hold.into())
                );
            }
        }
    }

    #[test]
    fn releases_all_modifiers_before_the_next_step_delay() {
        let mut action = sequence(vec![
            step(
                "Digit1",
                vec![Modifier::Ctrl, Modifier::Shift, Modifier::Alt],
                None,
            ),
            step("KeyW", vec![], Some((600, 600))),
        ]);
        action.steps[0].hold_ms = DelayRange {
            min: 1200,
            max: 1200,
        };
        action.steps[1].hold_ms = DelayRange { min: 0, max: 0 };
        let entries = Recorder::run(&action, 1);
        assert_eq!(
            keys(&entries),
            [
                (0x1D, DOWN),
                (0x2A, DOWN),
                (0x38, DOWN),
                (0x02, DOWN),
                (0x02, UP),
                (0x38, UP),
                (0x2A, UP),
                (0x1D, UP),
                (0x11, DOWN),
                (0x11, UP),
            ]
        );
        assert_eq!(time_between(&entries, 3, 4), Duration::from_millis(1200));
        for (from, to) in [(0, 1), (1, 2), (2, 3), (4, 5), (5, 6), (6, 7)] {
            assert!((15..=35).contains(&time_between(&entries, from, to).as_millis()));
        }
        assert_eq!(time_between(&entries, 7, 8), Duration::from_millis(600));
        assert_eq!(time_between(&entries, 8, 9), Duration::ZERO);
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
            [
                (0x10, DOWN),
                (0x10, UP),
                (0x48, DOWN),
                (0x48, UP),
                (0x3F, DOWN),
                (0x3F, UP)
            ]
        );
        let arrow = entries.iter().find_map(|entry| match entry {
            Entry::Key(event) if event.scan_code == 0x48 => Some(event.is_extended),
            _ => None,
        });
        assert_eq!(arrow, Some(true));
    }

    #[test]
    fn rejects_unsupported_key_before_any_input() {
        let action = sequence(vec![
            step("KeyQ", vec![], None),
            step("MetaLeft", vec![], Some((0, 0))),
        ]);
        let recorder = Recorder::default();
        let mut input = recorder.clone();

        let result = execute_key_sequence(
            &action,
            &mut input,
            &recorder,
            &mut StdRng::seed_from_u64(0),
        );

        assert!(matches!(result, Err(AppError::Validation(_))));
        assert!(recorder.0.borrow().is_empty());
    }
}
