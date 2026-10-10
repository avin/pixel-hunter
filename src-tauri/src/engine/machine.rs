//! Trigger logic of the running trackers, free of screen, input and real time.

use std::time::Duration;

use rand::{Rng, RngExt};
use uuid::Uuid;

use super::color::pixel_passes;
use crate::action::Action;
use crate::config::{Icon, Rgb, Tracker, Trigger};
use crate::icon_search::IconResult;

#[derive(Default)]
pub struct VisualBatch {
    pub points: Vec<(u32, u32)>,
    pub icons: Vec<(Uuid, Icon)>,
}
pub struct VisualResults {
    pub colors: Vec<Rgb>,
    pub icons: Vec<(Uuid, IconResult)>,
}

/// Action handed to the executor; `ticket` tells this run apart from later runs of the tracker.
#[derive(Debug, Clone, PartialEq)]
pub struct Job {
    pub ticket: u64,
    pub tracker_id: Uuid,
    /// Profile process name at the moment this job was queued.
    pub process_name: String,
    pub action: Action,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Polling,
    PreDelay {
        until: Duration,
    },
    /// Waiting in the executor queue or being played.
    Queued {
        ticket: u64,
    },
    Resume {
        until: Duration,
    },
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
    /// `process_name` is copied onto every job this tick queues.
    pub fn tick_visual(
        &mut self,
        now: Duration,
        paused: bool,
        process_name: &str,
        presses: &[Uuid],
        read: impl FnOnce(&VisualBatch) -> Option<VisualResults>,
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
            for tracker_id in presses {
                self.press(*tracker_id, now, false, rng);
            }
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
                        process_name: process_name.to_string(),
                        action: run.tracker.action.clone(),
                    });
                }
            }
        }
        jobs
    }

    #[cfg(test)]
    pub fn tick(
        &mut self,
        now: Duration,
        paused: bool,
        process_name: &str,
        presses: &[Uuid],
        read: impl FnOnce(&[(u32, u32)]) -> Option<Vec<Rgb>>,
        rng: &mut impl Rng,
    ) -> Vec<Job> {
        self.tick_visual(
            now,
            paused,
            process_name,
            presses,
            |batch| {
                read(&batch.points).map(|colors| VisualResults {
                    colors,
                    icons: vec![],
                })
            },
            rng,
        )
    }

    /// A hotkey press enters the same pre-action delay as a pixel match. Presses during a pause,
    /// or while this run is already in the cycle, do nothing.
    pub fn press(&mut self, tracker_id: Uuid, now: Duration, paused: bool, rng: &mut impl Rng) {
        if paused {
            return;
        }
        let Some(run) = self
            .runs
            .iter_mut()
            .find(|run| run.tracker.id == tracker_id)
        else {
            return;
        };
        if run.tracker.trigger != Trigger::Hotkey || run.phase != Phase::Polling {
            return;
        }
        enter_pre_delay(run, now, rng);
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
        read: impl FnOnce(&VisualBatch) -> Option<VisualResults>,
        rng: &mut impl Rng,
    ) {
        let polling: Vec<_> = self.polling_pixels().collect();
        let batch = VisualBatch {
            points: polling
                .iter()
                .flat_map(|run| run.tracker.pixels.iter().map(|p| (p.x, p.y)))
                .collect(),
            icons: polling
                .iter()
                .flat_map(|run| {
                    run.tracker
                        .icons
                        .iter()
                        .cloned()
                        .map(|icon| (run.tracker.id, icon))
                })
                .collect(),
        };
        if batch.points.is_empty() && batch.icons.is_empty() {
            return;
        }
        let Some(results) = read(&batch).filter(|r| r.colors.len() == batch.points.len()) else {
            return;
        };
        let mut colors = results.colors.into_iter();
        for run in self.runs.iter_mut().filter(|run| is_pixel_poll(run)) {
            let samples: Vec<_> = colors.by_ref().take(run.tracker.pixels.len()).collect();
            let pixels_pass = run
                .tracker
                .pixels
                .iter()
                .zip(samples)
                .all(|(p, c)| pixel_passes(c, p));
            let icons_pass = run.tracker.icons.iter().all(|icon| {
                results
                    .icons
                    .iter()
                    .find(|(id, _)| *id == icon.id)
                    .is_some_and(|(_, result)| result.passes(icon))
            });
            if pixels_pass && icons_pass {
                enter_pre_delay(run, now, rng);
            }
        }
    }

    fn polling_pixels(&self) -> impl Iterator<Item = &Run> {
        self.runs.iter().filter(|run| is_pixel_poll(run))
    }
}

fn is_pixel_poll(run: &Run) -> bool {
    run.phase == Phase::Polling && run.tracker.trigger == Trigger::Pixels
}

fn enter_pre_delay(run: &mut Run, now: Duration, rng: &mut impl Rng) {
    let range = run.tracker.pre_action_delay_ms;
    let delay = rng.random_range(range.min..=range.max);
    run.phase = Phase::PreDelay {
        until: now + Duration::from_millis(delay.into()),
    };
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
    use crate::action::{ActionKind, KeyCombo, KeySequenceAction, KeyStep};
    use crate::config::{DelayRange, Pixel, PixelCondition, Trigger};
    use rand::rngs::StdRng;
    use rand::SeedableRng;

    const RED: Rgb = Rgb {
        r: 200,
        g: 30,
        b: 30,
    };
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
        tracker.action = Action::key_sequence(KeySequenceAction {
            steps: vec![KeyStep {
                id: Uuid::new_v4(),
                delay_before_ms: None,
                hold_ms: crate::action::DEFAULT_STEP_HOLD_MS,
                combo: KeyCombo {
                    modifiers: vec![],
                    key: "KeyQ".into(),
                },
            }],
        });
        tracker.is_running = true;
        tracker
    }

    fn hotkey_tracker(delay: (u32, u32), resume_delay_ms: u32) -> Tracker {
        let mut tracker = tracker(&[(1, 1)], delay, resume_delay_ms);
        tracker.trigger = Trigger::Hotkey;
        tracker.hotkey = Some(KeyCombo {
            modifiers: vec![],
            key: "F5".into(),
        });
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
        process_name: String,
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
                process_name: "game.exe".into(),
            }
        }

        fn paint(&mut self, point: (u32, u32), color: Rgb) {
            self.screen.retain(|(p, _)| *p != point);
            self.screen.push((point, color));
        }

        fn press(&mut self, id: Uuid, now: Duration) {
            let paused = self.paused;
            self.core.press(id, now, paused, &mut self.rng);
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
                process_name,
            } = self;
            core.tick(
                now,
                *paused,
                process_name,
                &[],
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

    fn icon(condition: crate::config::IconCondition) -> Icon {
        Icon {
            id: Uuid::new_v4(),
            search_rect: crate::config::PhysicalRect {
                x: 0,
                y: 0,
                width: 3,
                height: 3,
            },
            template_rect: crate::config::PhysicalRect {
                x: 0,
                y: 0,
                width: 1,
                height: 1,
            },
            condition,
            similarity_percent: 90,
        }
    }
    fn visual_tick(
        core: &mut EngineCore,
        now: u64,
        paused: bool,
        color: Rgb,
        scores: &[IconResult],
        reads: &mut Vec<usize>,
    ) -> Vec<Job> {
        core.tick_visual(
            ms(now),
            paused,
            "game.exe",
            &[],
            |batch| {
                reads.push(batch.icons.len());
                Some(VisualResults {
                    colors: vec![color; batch.points.len()],
                    icons: batch
                        .icons
                        .iter()
                        .zip(scores)
                        .map(|((_, icon), result)| (icon.id, result.clone()))
                        .collect(),
                })
            },
            &mut StdRng::seed_from_u64(1),
        )
    }
    #[test]
    fn visual_and_requires_current_pixels_and_every_icon() {
        let mut tracker = tracker(&[(1, 1)], (0, 0), 0);
        tracker.icons = vec![
            icon(crate::config::IconCondition::Present),
            icon(crate::config::IconCondition::Absent),
        ];
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker]);
        let mut reads = vec![];
        assert!(visual_tick(
            &mut core,
            0,
            false,
            RED,
            &[IconResult::Similarity(0.0), IconResult::Similarity(0.0)],
            &mut reads
        )
        .is_empty());
        assert!(visual_tick(
            &mut core,
            50,
            false,
            BLACK,
            &[IconResult::Similarity(100.0), IconResult::Similarity(0.0)],
            &mut reads
        )
        .is_empty());
        assert!(visual_tick(
            &mut core,
            100,
            false,
            RED,
            &[
                IconResult::Similarity(100.0),
                IconResult::Unavailable("file".into())
            ],
            &mut reads
        )
        .is_empty());
        assert_eq!(
            visual_tick(
                &mut core,
                150,
                false,
                RED,
                &[IconResult::Similarity(100.0), IconResult::Similarity(0.0)],
                &mut reads
            )
            .len(),
            1
        );
    }
    #[test]
    fn icons_only_updates_next_poll_and_preserves_delay_queue_pause_cooldown() {
        let mut tracker = tracker(&[], (100, 100), 200);
        tracker.icons = vec![icon(crate::config::IconCondition::Present)];
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker.clone()]);
        let mut reads = vec![];
        assert!(visual_tick(
            &mut core,
            0,
            false,
            BLACK,
            &[IconResult::Similarity(85.0)],
            &mut reads
        )
        .is_empty());
        tracker.icons[0].similarity_percent = 80;
        core.set_trackers(&[tracker.clone()]);
        assert!(visual_tick(
            &mut core,
            50,
            false,
            BLACK,
            &[IconResult::Similarity(85.0)],
            &mut reads
        )
        .is_empty());
        tracker.icons[0].condition = crate::config::IconCondition::Absent;
        core.set_trackers(&[tracker]);
        assert!(visual_tick(&mut core, 150, true, BLACK, &[], &mut reads).is_empty());
        let jobs = visual_tick(&mut core, 200, false, BLACK, &[], &mut reads);
        assert_eq!(jobs.len(), 1);
        assert!(visual_tick(&mut core, 250, false, BLACK, &[], &mut reads).is_empty());
        core.action_done(jobs[0].ticket, ms(300));
        assert!(visual_tick(&mut core, 450, false, BLACK, &[], &mut reads).is_empty());
        assert!(visual_tick(
            &mut core,
            500,
            false,
            BLACK,
            &[IconResult::Similarity(100.0)],
            &mut reads
        )
        .is_empty());
        assert_eq!(reads, vec![1, 1, 1]);
        assert!(visual_tick(
            &mut core,
            550,
            false,
            BLACK,
            &[IconResult::Similarity(0.0)],
            &mut reads
        )
        .is_empty());
        assert_eq!(
            visual_tick(&mut core, 650, false, BLACK, &[], &mut reads).len(),
            1
        );
    }
    #[test]
    fn hotkey_saved_icons_do_not_poll_and_missing_result_blocks() {
        let mut tracker = hotkey_tracker((0, 0), 0);
        tracker.icons = vec![icon(crate::config::IconCondition::Present)];
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker.clone()]);
        let mut reads = vec![];
        assert!(visual_tick(
            &mut core,
            0,
            false,
            RED,
            &[IconResult::Similarity(100.0)],
            &mut reads
        )
        .is_empty());
        assert!(reads.is_empty());
        tracker.trigger = Trigger::Pixels;
        core.set_trackers(&[tracker]);
        assert!(visual_tick(&mut core, 50, false, RED, &[], &mut reads).is_empty());
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
        assert!(
            h.run(550, 1450).is_empty(),
            "no polling during the resume pause"
        );
        assert_eq!(
            h.reads.len(),
            reads_before,
            "pixels are not read until the pause ends"
        );

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

        assert!(
            delays.iter().all(|delay| (100..=300).contains(delay)),
            "{delays:?}"
        );
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

        assert_eq!(
            jobs.iter().map(|job| job.tracker_id).collect::<Vec<_>>(),
            [b.id]
        );
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

        assert_eq!(
            cancelled,
            jobs.iter().map(|job| job.ticket).collect::<Vec<_>>()
        );
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
        h.paint(
            (1, 1),
            Rgb {
                r: 200,
                g: 30,
                b: 70,
            },
        );
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

        let ActionKind::KeySequence(sequence) = &mut health.action.kind else {
            panic!("fixture action is a key sequence");
        };
        sequence.steps[0].combo.key = "KeyW".into();
        h.core.set_trackers(&[health.clone()]);

        assert_eq!(h.run(50, 100)[0].1.action, health.action);
    }

    #[test]
    fn sound_changes_during_pre_delay_apply_when_queued() {
        let mut health = tracker(&[(1, 1)], (100, 100), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        assert!(h.tick(ms(0)).is_empty());
        health.action.sound = crate::action::SoundSettings {
            id: Some("bright-bell".into()),
            volume_percent: 20,
        };
        h.core.set_trackers(&[health.clone()]);
        assert_eq!(h.tick(ms(100))[0].action.sound, health.action.sound);
    }

    #[test]
    fn queued_sound_is_a_snapshot_and_next_job_uses_updated_settings() {
        let mut health = tracker(&[(1, 1)], (0, 0), 100);
        health.action.sound.id = Some("soft-ping".into());
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        let first = h.tick(ms(0)).remove(0);
        health.action.sound = crate::action::SoundSettings {
            id: Some("bright-bell".into()),
            volume_percent: 20,
        };
        h.core.set_trackers(&[health.clone()]);
        assert_eq!(first.action.sound.id.as_deref(), Some("soft-ping"));
        assert_eq!(first.action.sound.volume_percent, 50);
        assert!(h.tick(ms(50)).is_empty());
        h.core.action_done(first.ticket, ms(50));
        assert!(h.tick(ms(100)).is_empty());
        assert_eq!(h.tick(ms(150))[0].action.sound, health.action.sound);
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
        let jobs = h.tick(ms(550));
        assert_eq!(jobs.len(), 1);
        assert_eq!(
            h.reads.len(),
            1,
            "the elapsed delay does not read pixels again"
        );
    }

    #[test]
    fn stopping_during_pause_drops_a_waiting_delay() {
        let health = tracker(&[(1, 1)], (100, 100), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        h.tick(ms(0));

        h.paused = true;
        let mut stopped = health;
        stopped.is_running = false;
        assert!(h.core.set_trackers(&[stopped]).is_empty());
        h.paused = false;

        assert!(h.run(550, 750).is_empty());
    }

    #[test]
    fn stopping_during_pause_cancels_a_queued_job() {
        let health = tracker(&[(1, 1)], (0, 0), 0);
        let mut h = Harness::new(&[health.clone()]);
        h.paint((1, 1), RED);
        let jobs = h.tick(ms(0));

        h.paused = true;
        let mut stopped = health;
        stopped.is_running = false;
        let cancelled = h.core.set_trackers(&[stopped]);

        assert_eq!(cancelled, [jobs[0].ticket]);
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
    fn queued_job_keeps_the_process_name_passed_to_tick() {
        let mut h = Harness::new(&[tracker(&[(1, 1)], (0, 0), 0)]);
        h.process_name = "Game.EXE".into();
        h.paint((1, 1), RED);

        let jobs = h.tick(ms(0));

        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].process_name, "Game.EXE");
    }

    #[test]
    fn ignores_a_read_with_a_wrong_number_of_colors() {
        let mut core = EngineCore::default();
        core.set_trackers(&[tracker(&[(1, 1), (2, 2)], (0, 0), 0)]);

        let jobs = core.tick(
            ms(0),
            false,
            "game.exe",
            &[],
            |_| Some(vec![RED]),
            &mut StdRng::seed_from_u64(0),
        );

        assert!(jobs.is_empty());
    }

    #[test]
    fn a_hotkey_trackers_saved_pixels_do_not_start_a_run() {
        let hotkey = hotkey_tracker((0, 0), 0);
        let mut h = Harness::new(&[hotkey]);
        h.paint((1, 1), RED);

        assert!(h.tick(ms(0)).is_empty());
        assert!(
            h.reads.is_empty(),
            "a hotkey tracker does not ask for a screen snapshot"
        );

        let pixels = tracker(&[(1, 1)], (0, 0), 0);
        h.core.set_trackers(&[pixels]);
        assert_eq!(h.tick(ms(50)).len(), 1);
        assert_eq!(h.reads, [vec![(1, 1)]]);
    }

    #[test]
    fn a_hotkey_press_waits_the_delay_and_ignores_repeats_until_the_pause_ends() {
        let hotkey = hotkey_tracker((100, 100), 1000);
        let mut h = Harness::new(&[hotkey.clone()]);

        h.press(hotkey.id, ms(0));
        assert!(h.tick(ms(0)).is_empty());
        let jobs = h.run(50, 100);
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].0, 100);
        assert_eq!(jobs[0].1.action, hotkey.action);

        h.press(hotkey.id, ms(150));
        assert!(
            h.tick(ms(150)).is_empty(),
            "a press while queued does nothing"
        );
        h.core.action_done(jobs[0].1.ticket, ms(200));
        h.press(hotkey.id, ms(250));
        assert!(
            h.run(250, 1200).is_empty(),
            "a press during the resume pause does not start another run"
        );

        h.press(hotkey.id, ms(1200));
        let next = h.run(1200, 1300);
        assert_eq!(next.len(), 1);
        assert_eq!(next[0].0, 1300);
    }

    #[test]
    fn a_hotkey_with_zero_pause_accepts_the_next_press_after_the_action() {
        let hotkey = hotkey_tracker((0, 0), 0);
        let mut h = Harness::new(&[hotkey.clone()]);

        h.press(hotkey.id, ms(0));
        let first = h.tick(ms(0));
        assert_eq!(first.len(), 1);
        h.press(hotkey.id, ms(10));
        assert!(h.tick(ms(10)).is_empty());

        h.core.action_done(first[0].ticket, ms(20));
        assert!(
            h.tick(ms(50)).is_empty(),
            "zero pause returns to waiting for a press"
        );
        h.press(hotkey.id, ms(50));
        assert_eq!(h.tick(ms(50)).len(), 1);
    }

    #[test]
    fn a_press_during_pause_does_not_queue_and_a_waiting_delay_finishes_without_another_press() {
        let hotkey = hotkey_tracker((100, 100), 0);
        let mut h = Harness::new(&[hotkey.clone()]);
        h.press(hotkey.id, ms(0));

        h.paused = true;
        h.press(hotkey.id, ms(50));
        assert!(h.run(50, 400).is_empty());

        h.paused = false;
        let jobs = h.tick(ms(450));
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].tracker_id, hotkey.id);

        let idle = hotkey_tracker((0, 0), 0);
        let mut waiting = Harness::new(&[idle.clone()]);
        waiting.paused = true;
        waiting.press(idle.id, ms(0));
        assert!(waiting.run(0, 100).is_empty());
        waiting.paused = false;
        assert!(
            waiting.tick(ms(150)).is_empty(),
            "a press during pause is not remembered"
        );
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
