//! Trigger logic of the running trackers, free of screen, input and real time.

use std::time::Duration;

use rand::{Rng, RngExt};
use uuid::Uuid;

use super::color::pixel_passes;
use crate::action::Action;
use crate::config::{Rgb, Tracker};

/// Action handed to the executor; `ticket` tells this run apart from later runs of the tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub ticket: u64,
    pub tracker_id: Uuid,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Polling,
    PreDelay { until: Duration },
    /// Waiting in the executor queue or being played.
    Queued { ticket: u64 },
    Resume { until: Duration },
}

struct Run {
    tracker: Tracker,
    phase: Phase,
}

#[derive(Default)]
pub struct EngineCore {
    runs: Vec<Run>,
    next_ticket: u64,
}

impl EngineCore {
    /// Takes the running complete trackers of the profile. Kept trackers stay in their phase and
    /// use the new settings from the next tick; returns tickets of queued jobs of dropped ones.
    pub fn set_trackers(&mut self, trackers: &[Tracker]) -> Vec<u64> {
        let mut previous = std::mem::take(&mut self.runs);
        self.runs = trackers
            .iter()
            .filter(|tracker| tracker.is_running && tracker.is_complete())
            .map(|tracker| {
                let phase = previous
                    .iter()
                    .position(|run| run.tracker.id == tracker.id)
                    .map_or(Phase::Polling, |index| previous.swap_remove(index).phase);
                Run {
                    tracker: tracker.clone(),
                    phase,
                }
            })
            .collect();
        queued_tickets(&previous)
    }

    /// Drops every tracker; returns tickets of their queued jobs.
    pub fn clear(&mut self) -> Vec<u64> {
        queued_tickets(&std::mem::take(&mut self.runs))
    }

    /// One poll: `read` gets the points of all polling trackers and returns their colors, or
    /// `None` when the game is not active or the screen is unavailable. While `paused` nothing is
    /// read and no job is queued; elapsed pre-action delays wait for the pause to end.
    pub fn tick(
        &mut self,
        now: Duration,
        paused: bool,
        read: impl FnOnce(&[(u32, u32)]) -> Option<Vec<Rgb>>,
        rng: &mut impl Rng,
    ) -> Vec<Job> {
        for run in &mut self.runs {
            if let Phase::Resume { until } = run.phase {
                if now >= until {
                    run.phase = Phase::Polling;
                }
            }
        }

        if !paused {
            self.observe(now, read, rng);
        }

        let mut jobs = Vec::new();
        for run in &mut self.runs {
            if let Phase::PreDelay { until } = run.phase {
                if now >= until && !paused {
                    let ticket = self.next_ticket;
                    self.next_ticket += 1;
                    run.phase = Phase::Queued { ticket };
                    jobs.push(Job {
                        ticket,
                        tracker_id: run.tracker.id,
                        action: run.tracker.action.clone(),
                    });
                }
            }
        }
        jobs
    }

    pub fn action_done(&mut self, ticket: u64, now: Duration) {
        if let Some(run) = self
            .runs
            .iter_mut()
            .find(|run| run.phase == Phase::Queued { ticket })
        {
            let pause = Duration::from_millis(run.tracker.resume_delay_ms.into());
            run.phase = Phase::Resume { until: now + pause };
        }
    }

    fn observe(
        &mut self,
        now: Duration,
        read: impl FnOnce(&[(u32, u32)]) -> Option<Vec<Rgb>>,
        rng: &mut impl Rng,
    ) {
        let points: Vec<_> = self
            .polling()
            .flat_map(|run| run.tracker.pixels.iter().map(|pixel| (pixel.x, pixel.y)))
            .collect();
        if points.is_empty() {
            return;
        }
        let Some(colors) = read(&points).filter(|colors| colors.len() == points.len()) else {
            return;
        };

        let mut colors = colors.into_iter();
        for run in self.runs.iter_mut().filter(|run| run.phase == Phase::Polling) {
            let samples: Vec<_> = colors.by_ref().take(run.tracker.pixels.len()).collect();
            let all_passed = run
                .tracker
                .pixels
                .iter()
                .zip(samples)
                .all(|(pixel, sample)| pixel_passes(sample, pixel));
            if all_passed {
                let range = run.tracker.pre_action_delay_ms;
                let delay = rng.random_range(range.min..=range.max);
                run.phase = Phase::PreDelay {
                    until: now + Duration::from_millis(delay.into()),
                };
            }
        }
    }

    fn polling(&self) -> impl Iterator<Item = &Run> {
        self.runs.iter().filter(|run| run.phase == Phase::Polling)
    }
}

fn queued_tickets(runs: &[Run]) -> Vec<u64> {
    runs.iter()
        .filter_map(|run| match run.phase {
            Phase::Queued { ticket } => Some(ticket),
            _ => None,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::{KeyCombo, KeySequenceAction, KeyStep};
    use crate::config::{DelayRange, Pixel, PixelCondition};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const RED: Rgb = Rgb { r: 200, g: 30, b: 30 };
    const BLACK: Rgb = Rgb { r: 0, g: 0, b: 0 };

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    /// Running tracker whose pixels expect red at the given points.
    fn tracker(points: &[(u32, u32)], delay: (u32, u32), resume_delay_ms: u32) -> Tracker {
        let mut tracker = Tracker::new("Здоровье".into());
        tracker.pixels = points
            .iter()
            .map(|&(x, y)| Pixel::new(Uuid::new_v4(), x, y, RED))
            .collect();
        tracker.pre_action_delay_ms = DelayRange {
            min: delay.0,
            max: delay.1,
        };
        tracker.resume_delay_ms = resume_delay_ms;
        tracker.action = Action::KeySequence(KeySequenceAction {
            steps: vec![KeyStep {
                id: Uuid::new_v4(),
                delay_before_ms: None,
                combo: KeyCombo {
                    modifiers: vec![],
                    key: "KeyQ".into(),
                },
            }],
        });
        tracker.is_running = true;
        tracker
    }

    /// Drives the core like the poll thread does, with a virtual clock.
    struct Harness {
        core: EngineCore,
        rng: StdRng,
        /// Colors on the "screen" by point; unknown points are black.
        screen: Vec<((u32, u32), Rgb)>,
        game_active: bool,
        paused: bool,
        screen_fails: bool,
        reads: Vec<Vec<(u32, u32)>>,
    }

    impl Harness {
        fn new(trackers: &[Tracker]) -> Self {
            let mut core = EngineCore::default();
            core.set_trackers(trackers);
            Self {
                core,
                rng: StdRng::seed_from_u64(1),
                screen: Vec::new(),
                game_active: true,
                paused: false,
                screen_fails: false,
                reads: Vec::new(),
            }
        }

        fn paint(&mut self, point: (u32, u32), color: Rgb) {
            self.screen.retain(|(p, _)| *p != point);
            self.screen.push((point, color));
        }

        fn tick(&mut self, now: Duration) -> Vec<Job> {
            let Self {
                core,
                rng,
                screen,
                game_active,
                paused,
                screen_fails,
                reads,
            } = self;
            core.tick(
                now,
                *paused,
                |points| {
                    if !*game_active || *screen_fails {
                        return None;
                    }
                    reads.push(points.to_vec());
                    Some(
                        points
                            .iter()
                            .map(|point| {
                                screen
                                    .iter()
                                    .find(|(p, _)| p == point)
                                    .map_or(BLACK, |(_, color)| *color)
                            })
                            .collect(),
                    )
                },
                rng,
            )
        }

        /// Ticks every 50 ms in `[from, to]`, collecting `(time, job)` pairs.
        fn run(&mut self, from: u64, to: u64) -> Vec<(u64, Job)> {
            (from..=to)
                .step_by(50)
                .flat_map(|t| self.tick(ms(t)).into_iter().map(move |job| (t, job)))
                .collect()
        }
    }

    #[test]
    fn full_cycle_waits_the_delay_runs_once_and_pauses_after_the_action() {
        let health = tracker(&[(1, 1)], (100, 100), 1000);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);

        assert!(h.tick(ms(0)).is_empty());
        let jobs = h.run(50, 100);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, 100);
        assert_eq!(jobs[0].1.tracker_id, health.id);
        assert_eq!(jobs[0].1.action, health.action);

        let reads_before = h.reads.len();
        assert!(h.run(150, 500).is_empty(), "no second run while queued");
        h.core.action_done(jobs[0].1.ticket, ms(500));
        assert!(h.run(550, 1450).is_empty(), "no polling during the resume pause");
        assert_eq!(h.reads.len(), reads_before, "pixels are not read until the pause ends");

        let next = h.run(1500, 1600);
        assert_eq!(next.first().map(|(t, _)| *t), Some(1600));
    }

    #[test]
    fn delay_is_drawn_from_the_whole_range() {
        let delays: Vec<u64> = (0..100)
            .map(|seed| {
                let mut h = Harness::new(&[tracker(&[(1, 1)], (100, 300), 0)]);
                h.rng = StdRng::seed_from_u64(seed);
                h.paint((1, 1), RED);
                h.tick(ms(0));
                (1..=400)
                    .find(|t| !h.tick(ms(*t)).is_empty())
                    .expect("the job is queued within the range")
            })
            .collect();

        assert!(delays.iter().all(|delay| (100..=300).contains(delay)), "{delays:?}");
        assert!(delays.iter().any(|delay| *delay < 150));
        assert!(delays.iter().any(|delay| *delay > 250));
    }

    #[test]
    fn runs_even_if_the_condition_is_gone_after_the_delay_started() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (200, 200), 0)]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        h.paint((1, 1), BLACK);

        assert_eq!(h.run(50, 200).len(), 1);
    }

    #[test]
    fn zero_pause_triggers_again_on_the_next_poll() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 0)]);
        h.paint((1, 1), RED);

        let first = h.tick(ms(0));
        assert_eq!(first.len(), 1);
        h.core.action_done(first[0].ticket, ms(30));

        assert_eq!(h.tick(ms(50)).len(), 1);
    }

    #[test]
    fn needs_every_pixel_to_pass_in_the_same_poll() {
        let mut h = Harness::new(&[tracker(&[(1, 1), (2, 2)], (0, 0), 0)]);
        h.paint((1, 1), RED);

        assert!(h.run(0, 200).is_empty());

        h.paint((2, 2), RED);
        assert_eq!(h.tick(ms(250)).len(), 1);
    }

    #[test]
    fn reads_all_polling_trackers_in_one_call() {
        let a = tracker(&[(1, 1)], (0, 0), 0);
        let b = tracker(&[(2, 2), (3, 3)], (0, 0), 0);
        let mut h = Harness::new(&[a, b]);

        h.tick(ms(0));

        assert_eq!(h.reads, [vec![(1, 1), (2, 2), (3, 3)]]);
    }

    #[test]
    fn matches_each_tracker_against_its_own_pixels() {
        let a = tracker(&[(1, 1)], (0, 0), 0);
        let b = tracker(&[(2, 2)], (0, 0), 0);
        let mut h = Harness::new(&[a, b.clone()]);
        h.paint((2, 2), RED);

        let jobs = h.tick(ms(0));

        assert_eq!(jobs.iter().map(|job| job.tracker_id).collect::<Vec<_>>(), [b.id]);
    }

    #[test]
    fn does_not_trigger_while_another_window_is_active() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 0)]);
        h.paint((1, 1), RED);
        h.game_active = false;

        assert!(h.run(0, 500).is_empty());

        h.game_active = true;
        assert_eq!(h.tick(ms(550)).len(), 1);
    }

    #[test]
    fn started_delay_finishes_while_another_window_is_active() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (100, 100), 0)]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        h.game_active = false;

        assert_eq!(h.run(50, 100).len(), 1);
    }

    #[test]
    fn stopping_during_the_delay_cancels_the_run() {
        let health = tracker(&[(1, 1)], (200, 200), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        let mut stopped = health;
        stopped.is_running = false;
        let cancelled = h.core.set_trackers(&[stopped]);

        assert!(cancelled.is_empty());
        assert!(h.run(50, 1000).is_empty());
        assert_eq!(h.reads.len(), 1, "a stopped tracker is not polled");
    }

    #[test]
    fn stopping_a_queued_tracker_returns_its_ticket() {
        let health = tracker(&[(1, 1)], (0, 0), 0);
        let mana = tracker(&[(2, 2)], (0, 0), 0);
        let mut h = Harness::new(&[health.clone(), mana.clone()]);
        h.paint((1, 1), RED);
        let jobs = h.tick(ms(0));

        let cancelled = h.core.set_trackers(&[mana]);

        assert_eq!(cancelled, [jobs[0].ticket]);
    }

    #[test]
    fn clearing_returns_tickets_of_every_queued_job() {
        let a = tracker(&[(1, 1)], (0, 0), 0);
        let b = tracker(&[(2, 2)], (0, 0), 0);
        let c = tracker(&[(3, 3)], (500, 500), 0);
        let mut h = Harness::new(&[a, b, c]);
        for point in [(1, 1), (2, 2), (3, 3)] {
            h.paint(point, RED);
        }
        let jobs = h.tick(ms(0));

        let mut cancelled = h.core.clear();
        cancelled.sort();

        assert_eq!(cancelled, jobs.iter().map(|job| job.ticket).collect::<Vec<_>>());
        assert!(h.run(50, 1000).is_empty());
    }

    #[test]
    fn restarted_tracker_starts_polling_again() {
        let health = tracker(&[(1, 1)], (0, 0), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        let first = h.tick(ms(0));

        let mut stopped = health.clone();
        stopped.is_running = false;
        h.core.set_trackers(&[stopped]);
        h.core.set_trackers(&[health]);
        h.core.action_done(first[0].ticket, ms(10));

        let again = h.tick(ms(50));
        assert_eq!(again.len(), 1);
        assert_ne!(again[0].ticket, first[0].ticket);
    }

    #[test]
    fn ignores_incomplete_trackers() {
        let mut no_steps = tracker(&[(1, 1)], (0, 0), 0);
        no_steps.action = Action::default();
        let mut h = Harness::new(&[no_steps]);
        h.paint((1, 1), RED);

        assert!(h.tick(ms(0)).is_empty());
        assert!(h.reads.is_empty());
    }

    #[test]
    fn new_tolerance_applies_from_the_next_poll() {
        let mut health = tracker(&[(1, 1)], (0, 0), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), Rgb { r: 200, g: 30, b: 70 });
        assert!(h.tick(ms(0)).is_empty());

        health.pixels[0].tolerance_percent = 20;
        h.core.set_trackers(&[health]);

        assert_eq!(h.tick(ms(50)).len(), 1);
    }

    #[test]
    fn new_pixel_applies_from_the_next_poll() {
        let mut health = tracker(&[(1, 1)], (0, 0), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.tick(ms(0));

        let mut added = Pixel::new(Uuid::new_v4(), 2, 2, RED);
        added.condition = PixelCondition::NotEquals;
        health.pixels.push(added);
        h.core.set_trackers(&[health]);
        h.tick(ms(50));

        assert_eq!(h.reads[1], [(1, 1), (2, 2)]);
    }

    #[test]
    fn changed_settings_keep_the_current_phase() {
        let mut health = tracker(&[(1, 1)], (100, 100), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        health.resume_delay_ms = 2000;
        h.core.set_trackers(&[health.clone()]);
        let jobs = h.run(50, 100);

        assert_eq!(jobs.len(), 1, "the delay already started is kept");
        assert_eq!(jobs[0].1.ticket, 0);
    }

    #[test]
    fn new_action_is_used_by_the_next_run() {
        let mut health = tracker(&[(1, 1)], (100, 100), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        let Action::KeySequence(sequence) = &mut health.action;
        sequence.steps[0].combo.key = "KeyW".into();
        h.core.set_trackers(&[health.clone()]);

        assert_eq!(h.run(50, 100)[0].1.action, health.action);
    }

    #[test]
    fn capture_pause_skips_reading_and_holds_elapsed_delays() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (100, 100), 0)]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        h.paused = true;
        assert!(h.run(50, 500).is_empty());
        assert_eq!(h.reads.len(), 1);

        h.paused = false;
        assert_eq!(h.tick(ms(550)).len(), 1);
    }

    #[test]
    fn capture_pause_starts_no_new_runs_and_polling_resumes_after_it() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 0)]);
        h.paint((1, 1), RED);
        h.paused = true;

        assert!(h.run(0, 500).is_empty());
        assert!(h.reads.is_empty());

        h.paused = false;
        assert_eq!(h.tick(ms(550)).len(), 1);
    }

    #[test]
    fn resume_pause_runs_out_during_capture_pause() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 100)]);
        h.paint((1, 1), RED);
        let jobs = h.tick(ms(0));
        h.core.action_done(jobs[0].ticket, ms(10));

        h.paused = true;
        h.run(50, 500);
        h.paused = false;

        assert_eq!(h.tick(ms(550)).len(), 1);
    }

    #[test]
    fn unavailable_screen_skips_the_poll_until_it_is_back() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 0)]);
        h.paint((1, 1), RED);
        h.screen_fails = true;

        assert!(h.run(0, 300).is_empty());

        h.screen_fails = false;
        assert_eq!(h.tick(ms(350)).len(), 1);
    }

    #[test]
    fn ignores_a_read_with_a_wrong_number_of_colors() {
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker(&[(1, 1), (2, 2)], (0, 0), 0)]);

        let jobs = core.tick(ms(0), false, |_| Some(vec![RED]), &mut StdRng::seed_from_u64(0));

        assert!(jobs.is_empty());
    }

    #[test]
    fn stale_action_done_is_ignored() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 1000)]);
        h.paint((1, 1), RED);
        let first = h.tick(ms(0));
        h.core.action_done(first[0].ticket, ms(10));

        h.core.action_done(first[0].ticket + 7, ms(20));

        assert!(h.run(50, 950).is_empty());
        assert_eq!(h.tick(ms(1050)).len(), 1);
    }
}
