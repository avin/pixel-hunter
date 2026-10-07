//! Plays queued actions one at a time, in queue order.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::Duration;

use super::machine::Job;
use crate::action::Action;

const PAUSE_CHECK_INTERVAL: Duration = Duration::from_millis(10);

/// Tickets of jobs dropped by the engine while they waited in the queue.
#[derive(Default)]
pub struct CancelledJobs(Mutex<HashSet<u64>>);

impl CancelledJobs {
    pub fn cancel(&self, tickets: impl IntoIterator<Item = u64>) {
        self.lock().extend(tickets);
    }

    fn take(&self, ticket: u64) -> bool {
        self.lock().remove(&ticket)
    }

    fn lock(&self) -> MutexGuard<'_, HashSet<u64>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Runs until the queue is closed. A job cancelled before it starts is skipped; a started one is
/// played to the end. Nothing starts while `paused`. `done` gets the ticket of each played job.
pub fn run(
    jobs: Receiver<Job>,
    cancelled: &CancelledJobs,
    paused: &AtomicBool,
    mut play: impl FnMut(&Action),
    sleep: impl Fn(Duration),
    mut done: impl FnMut(u64),
) {
    for job in jobs {
        while paused.load(Ordering::SeqCst) {
            sleep(PAUSE_CHECK_INTERVAL);
        }
        if cancelled.take(job.ticket) {
            continue;
        }
        play(&job.action);
        cancelled.take(job.ticket);
        done(job.ticket);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{ActionKind, KeyCombo, KeySequenceAction, KeyStep};
    use crate::key_sequence::{execute_key_sequence, InputSink, KeyEvent, Sleeper};
    use rand::rngs::StdRng;
    use rand::SeedableRng;
    use std::cell::RefCell;
    use std::sync::mpsc;
    use uuid::Uuid;

    #[derive(Debug, Clone, PartialEq)]
    enum Entry {
        Sound(String, u8),
        Key(u16, bool),
        Done(u64),
    }

    type Log = RefCell<Vec<Entry>>;

    struct FakeInput<'a>(&'a Log);

    impl InputSink for FakeInput<'_> {
        fn send(&mut self, event: KeyEvent) {
            self.0.borrow_mut().push(Entry::Key(event.scan_code, event.is_up));
        }
    }

    /// Instant sleeper that runs `on_sleep` on its first call.
    struct FakeSleeper<'a> {
        on_first_sleep: RefCell<Option<Box<dyn FnOnce() + 'a>>>,
    }

    impl Sleeper for FakeSleeper<'_> {
        fn sleep(&self, _: Duration) {
            if let Some(callback) = self.on_first_sleep.borrow_mut().take() {
                callback();
            }
        }
    }

    fn keys(codes: &[&str]) -> Action {
        Action::key_sequence(KeySequenceAction {
            steps: codes
                .iter()
                .enumerate()
                .map(|(index, key)| KeyStep {
                    id: Uuid::new_v4(),
                    delay_before_ms: (index > 0).then_some(crate::config::DelayRange { min: 0, max: 0 }),
                    combo: KeyCombo {
                        modifiers: vec![],
                        key: (*key).into(),
                    },
                })
                .collect(),
        })
    }

    fn job(ticket: u64, action: Action) -> Job {
        Job {
            ticket,
            tracker_id: Uuid::new_v4(),
            action,
        }
    }

    fn run_jobs(
        jobs: Vec<Job>,
        cancelled: &CancelledJobs,
        on_first_sleep: Option<Box<dyn FnOnce() + '_>>,
    ) -> Vec<Entry> {
        let (sender, receiver) = mpsc::channel();
        for job in jobs {
            sender.send(job).unwrap();
        }
        drop(sender);
        let log = Log::default();
        let sleeper = FakeSleeper {
            on_first_sleep: RefCell::new(on_first_sleep),
        };
        let mut rng = StdRng::seed_from_u64(0);
        run(
            receiver,
            cancelled,
            &AtomicBool::new(false),
            |action| {
                super::super::play_action(
                    action,
                    |sound| log.borrow_mut().push(Entry::Sound(sound.id.clone().unwrap(), sound.volume_percent)),
                    |action| {
                        let ActionKind::KeySequence(sequence) = &action.kind;
                        execute_key_sequence(sequence, &mut FakeInput(&log), &sleeper, &mut rng)
                    },
                );
            },
            |_| {},
            |ticket| log.borrow_mut().push(Entry::Done(ticket)),
        );
        log.take()
    }

    const Q: u16 = 0x10;
    const W: u16 = 0x11;
    const E: u16 = 0x12;

    fn with_sound(mut action: Action, id: &str, volume: u8) -> Action {
        action.sound.id = Some(id.into());
        action.sound.volume_percent = volume;
        action
    }

    #[test]
    fn sound_starts_once_before_keys_and_done_does_not_wait_for_audio() {
        let entries = run_jobs(
            vec![job(1, with_sound(keys(&["KeyQ", "KeyW"]), "double-tap", 40))],
            &CancelledJobs::default(),
            None,
        );
        assert_eq!(entries, [
            Entry::Sound("double-tap".into(), 40),
            Entry::Key(Q, false), Entry::Key(Q, true),
            Entry::Key(W, false), Entry::Key(W, true), Entry::Done(1),
        ]);
    }

    #[test]
    fn cancelled_and_muted_jobs_do_not_submit_audio() {
        let cancelled = CancelledJobs::default();
        cancelled.cancel([1]);
        let entries = run_jobs(
            vec![
                job(1, with_sound(keys(&["KeyQ"]), "soft-ping", 50)),
                job(2, with_sound(keys(&["KeyW"]), "soft-ping", 0)),
                job(3, keys(&["KeyE"])),
            ],
            &cancelled, None,
        );
        assert_eq!(entries, [
            Entry::Key(W, false), Entry::Key(W, true), Entry::Done(2),
            Entry::Key(E, false), Entry::Key(E, true), Entry::Done(3),
        ]);
    }

    #[test]
    fn cancellation_during_capture_pause_skips_the_sound_callback() {
        let (sender, receiver) = mpsc::channel();
        sender.send(job(1, with_sound(keys(&["KeyQ"]), "soft-ping", 50))).unwrap();
        drop(sender);
        let paused = AtomicBool::new(true);
        let cancelled = CancelledJobs::default();
        run(receiver, &cancelled, &paused,
            |_| panic!("cancelled action must not submit sound or keys"),
            |_| { cancelled.cancel([1]); paused.store(false, Ordering::SeqCst); },
            |_| panic!("cancelled job must not finish as played"),
        );
    }

    #[test]
    fn unavailable_audio_device_does_not_skip_keys_or_job_completion() {
        let audio = crate::action_sound::AudioService::unavailable_for_test();
        assert!(audio.preview("soft-ping", 50).is_err());
        let (sender, receiver) = mpsc::channel();
        sender.send(job(1, with_sound(keys(&["KeyQ"]), "soft-ping", 50))).unwrap();
        drop(sender);
        let log = Log::default();
        let sleeper = FakeSleeper { on_first_sleep: RefCell::new(None) };
        let mut rng = StdRng::seed_from_u64(0);
        run(receiver, &CancelledJobs::default(), &AtomicBool::new(false),
            |action| super::super::play_action(action,
                |sound| audio.play_runtime(sound),
                |action| {
                    let ActionKind::KeySequence(sequence) = &action.kind;
                    execute_key_sequence(sequence, &mut FakeInput(&log), &sleeper, &mut rng)
                }),
            |_| {},
            |ticket| log.borrow_mut().push(Entry::Done(ticket)),
        );
        audio.shutdown();
        assert_eq!(log.take(), [Entry::Key(Q, false), Entry::Key(Q, true), Entry::Done(1)]);
    }

    #[test]
    fn plays_jobs_strictly_one_after_another() {
        let entries = run_jobs(
            vec![job(1, keys(&["KeyQ", "KeyW"])), job(2, keys(&["KeyE"]))],
            &CancelledJobs::default(),
            None,
        );

        assert_eq!(
            entries,
            [
                Entry::Key(Q, false),
                Entry::Key(Q, true),
                Entry::Key(W, false),
                Entry::Key(W, true),
                Entry::Done(1),
                Entry::Key(E, false),
                Entry::Key(E, true),
                Entry::Done(2),
            ]
        );
    }

    #[test]
    fn skips_a_job_cancelled_while_queued() {
        let cancelled = CancelledJobs::default();
        cancelled.cancel([1]);

        let entries = run_jobs(
            vec![job(1, keys(&["KeyQ"])), job(2, keys(&["KeyE"]))],
            &cancelled,
            None,
        );

        assert_eq!(entries, [Entry::Key(E, false), Entry::Key(E, true), Entry::Done(2)]);
    }

    #[test]
    fn finishes_a_job_cancelled_after_it_started() {
        let cancelled = CancelledJobs::default();

        let entries = run_jobs(
            vec![job(1, keys(&["KeyQ", "KeyW"]))],
            &cancelled,
            Some(Box::new(|| cancelled.cancel([1]))),
        );

        assert_eq!(
            entries,
            [
                Entry::Key(Q, false),
                Entry::Key(Q, true),
                Entry::Key(W, false),
                Entry::Key(W, true),
                Entry::Done(1),
            ]
        );
        assert!(cancelled.lock().is_empty());
    }

    #[test]
    fn waits_for_the_capture_pause_to_end_before_starting() {
        let (sender, receiver) = mpsc::channel();
        sender.send(job(1, keys(&["KeyQ"]))).unwrap();
        drop(sender);
        let paused = AtomicBool::new(true);
        let waits = RefCell::new(0);
        let played = RefCell::new(false);

        run(
            receiver,
            &CancelledJobs::default(),
            &paused,
            |_| {
                assert!(!paused.load(Ordering::SeqCst));
                *played.borrow_mut() = true;
            },
            |_| {
                *waits.borrow_mut() += 1;
                if *waits.borrow() == 3 {
                    paused.store(false, Ordering::SeqCst);
                }
            },
            |_| {},
        );

        assert!(*played.borrow());
        assert_eq!(*waits.borrow(), 3);
    }
}
