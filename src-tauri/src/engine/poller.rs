//! Poll thread: owns the core, the screen and the clock of the active profile.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rand::Rng;
use uuid::Uuid;

use super::executor::CancelledJobs;
use super::machine::{EngineCore, Job};
use super::EngineStatus;
use crate::config::Profile;
use crate::foreground::{self, WindowHandle};
use crate::keyboard::TimerResolution;
use crate::processes;
use crate::screen::PrimaryScreen;

pub const TICK: Duration = Duration::from_millis(50);
const SCREEN_RETRY_INTERVAL: Duration = Duration::from_secs(1);

pub enum Command {
    Activate(Profile),
    /// New settings of the active profile.
    Update(Profile),
    Deactivate,
    ActionDone(u64),
}

/// Tick times on a fixed grid; a tick late by more than a period restarts the grid instead of
/// catching up with the missed ones.
pub struct Deadlines {
    period: Duration,
    deadline: Duration,
}

impl Deadlines {
    pub fn new(start: Duration, period: Duration) -> Self {
        Self {
            period,
            deadline: start,
        }
    }

    /// Called when a tick starts at `now`; returns when the next one is due.
    pub fn next(&mut self, now: Duration) -> Duration {
        if now.saturating_sub(self.deadline) > self.period {
            self.deadline = now;
        }
        self.deadline += self.period;
        self.deadline
    }
}

/// Exe name of the foreground process, looked up again only when the window or process changes.
#[derive(Default)]
pub struct ForegroundCache {
    key: Option<(WindowHandle, u32)>,
    exe_name: Option<String>,
}

impl ForegroundCache {
    pub fn is_process(
        &mut self,
        ids: Option<(WindowHandle, u32)>,
        process_name: &str,
        exe_name: impl FnOnce(u32) -> Option<String>,
    ) -> bool {
        if ids != self.key {
            self.key = ids;
            self.exe_name = ids.and_then(|(_, pid)| exe_name(pid));
        }
        self.exe_name
            .as_deref()
            .is_some_and(|exe_name| foreground::is_process(exe_name, process_name))
    }
}

enum Flow {
    Deactivated,
    Exit,
}

pub struct Poller<E> {
    pub commands: Receiver<Command>,
    pub jobs: Sender<Job>,
    pub cancelled: Arc<CancelledJobs>,
    pub paused: Arc<AtomicBool>,
    pub emit: E,
}

impl<E: Fn(EngineStatus)> Poller<E> {
    /// Runs until the engine handle is dropped; sleeps on the command channel while inactive.
    pub fn run(self) {
        let started = Instant::now();
        let mut rng = rand::rng();
        loop {
            let profile = match self.commands.recv() {
                Ok(Command::Activate(profile)) => profile,
                Ok(_) => continue,
                Err(_) => return,
            };
            if let Flow::Exit = self.run_active(profile, started, &mut rng) {
                return;
            }
        }
    }

    fn run_active(&self, profile: Profile, started: Instant, rng: &mut impl Rng) -> Flow {
        let _resolution = TimerResolution::begin();
        let clock = || started.elapsed();
        let mut session = Session::new(profile);
        let mut deadlines = Deadlines::new(clock(), TICK);
        #[cfg(debug_assertions)]
        let mut tick_log = TickLog::default();

        loop {
            let tick_start = clock();
            #[cfg(debug_assertions)]
            tick_log.record(tick_start);
            let due = deadlines.next(tick_start);

            let paused = self.paused.load(Ordering::SeqCst);
            for job in session.tick(tick_start, paused, rng, &self.emit) {
                let _ = self.jobs.send(job);
            }

            loop {
                let remaining = due.saturating_sub(clock());
                if remaining.is_zero() {
                    break;
                }
                let command = match self.commands.recv_timeout(remaining) {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.cancelled.cancel(session.core.clear());
                        return Flow::Exit;
                    }
                };
                match command {
                    Command::Activate(profile) => {
                        self.cancelled.cancel(session.core.clear());
                        session = Session::new(profile);
                    }
                    Command::Update(profile) => {
                        self.cancelled.cancel(session.update(profile));
                    }
                    Command::Deactivate => {
                        self.cancelled.cancel(session.core.clear());
                        return Flow::Deactivated;
                    }
                    Command::ActionDone(ticket) => session.core.action_done(ticket, clock()),
                }
            }
        }
    }
}

struct Session {
    profile_id: Uuid,
    process_name: String,
    core: EngineCore,
    foreground: ForegroundCache,
    game_active: Option<bool>,
    screen: Option<PrimaryScreen>,
    screen_retry_at: Duration,
    last_screen_error: Option<String>,
}

impl Session {
    fn new(profile: Profile) -> Self {
        let mut session = Self {
            profile_id: profile.id,
            process_name: String::new(),
            core: EngineCore::default(),
            foreground: ForegroundCache::default(),
            game_active: None,
            screen: None,
            screen_retry_at: Duration::ZERO,
            last_screen_error: None,
        };
        session.update(profile);
        session
    }

    /// Returns tickets of queued jobs of trackers that no longer run.
    fn update(&mut self, profile: Profile) -> Vec<u64> {
        self.process_name = profile.process_name;
        self.core.set_trackers(&profile.trackers)
    }

    fn tick(
        &mut self,
        now: Duration,
        paused: bool,
        rng: &mut impl Rng,
        emit: &impl Fn(EngineStatus),
    ) -> Vec<Job> {
        let game_active = self.foreground.is_process(
            foreground::foreground_ids(),
            &self.process_name,
            processes::exe_name,
        );
        if self.game_active != Some(game_active) {
            self.game_active = Some(game_active);
            emit(EngineStatus {
                profile_id: self.profile_id,
                game_active,
            });
        }
        if paused {
            // The capture session duplicates the same output, which allows one duplication.
            self.screen = None;
        }

        let process_name = self.process_name.clone();
        let Self {
            core,
            screen,
            screen_retry_at,
            last_screen_error,
            ..
        } = self;
        core.tick(
            now,
            paused,
            &process_name,
            |points| {
                if !game_active {
                    return None;
                }
                if screen.is_none() {
                    if now < *screen_retry_at {
                        return None;
                    }
                    *screen_retry_at = now + SCREEN_RETRY_INTERVAL;
                    match PrimaryScreen::new() {
                        Ok(created) => *screen = Some(created),
                        Err(error) => {
                            report_screen_error(last_screen_error, error.to_string());
                            return None;
                        }
                    }
                }
                match screen.as_mut()?.read_pixels(points) {
                    Ok(colors) => {
                        *last_screen_error = None;
                        Some(colors)
                    }
                    Err(error) => {
                        report_screen_error(last_screen_error, error.to_string());
                        None
                    }
                }
            },
            rng,
        )
    }
}

fn report_screen_error(last: &mut Option<String>, message: String) {
    if last.as_ref() != Some(&message) {
        eprintln!("Движок пропускает опрос: {message}");
        *last = Some(message);
    }
}

/// Prints the tick period statistics about once a second in debug builds.
#[cfg(debug_assertions)]
#[derive(Default)]
struct TickLog {
    last: Option<Duration>,
    periods: Vec<Duration>,
}

#[cfg(debug_assertions)]
impl TickLog {
    fn record(&mut self, now: Duration) {
        if let Some(last) = self.last.replace(now) {
            self.periods.push(now - last);
        }
        if self.periods.len() == 20 {
            let ms = |duration: Duration| duration.as_secs_f64() * 1000.0;
            let total: Duration = self.periods.iter().sum();
            eprintln!(
                "Период опроса: среднее {:.1} мс, мин {:.1} мс, макс {:.1} мс",
                ms(total) / 20.0,
                ms(*self.periods.iter().min().unwrap()),
                ms(*self.periods.iter().max().unwrap()),
            );
            self.periods.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ms(value: u64) -> Duration {
        Duration::from_millis(value)
    }

    #[test]
    fn ticks_on_a_fixed_grid_without_drift() {
        let mut deadlines = Deadlines::new(ms(0), TICK);

        assert_eq!(deadlines.next(ms(0)), ms(50));
        assert_eq!(deadlines.next(ms(51)), ms(100));
        assert_eq!(deadlines.next(ms(102)), ms(150));
        assert_eq!(deadlines.next(ms(150)), ms(200));
    }

    #[test]
    fn slightly_late_tick_keeps_the_grid() {
        let mut deadlines = Deadlines::new(ms(0), TICK);
        deadlines.next(ms(0));

        assert_eq!(deadlines.next(ms(95)), ms(100));
    }

    #[test]
    fn tick_late_by_more_than_a_period_does_not_catch_up() {
        let mut deadlines = Deadlines::new(ms(0), TICK);
        deadlines.next(ms(0));

        assert_eq!(deadlines.next(ms(130)), ms(180));
        assert_eq!(deadlines.next(ms(180)), ms(230));
    }

    #[test]
    fn looks_up_the_exe_only_when_the_foreground_changes() {
        let mut cache = ForegroundCache::default();
        let mut lookups = 0;
        let game = Some((WindowHandle(1), 10));
        let is_game = |cache: &mut ForegroundCache, ids, lookups: &mut u32| {
            cache.is_process(ids, "game.exe", |pid| {
                *lookups += 1;
                Some(if pid == 10 { "Game.EXE" } else { "notepad.exe" }.to_string())
            })
        };

        assert!(is_game(&mut cache, game, &mut lookups));
        assert!(is_game(&mut cache, game, &mut lookups));
        assert_eq!(lookups, 1);

        assert!(!is_game(
            &mut cache,
            Some((WindowHandle(2), 20)),
            &mut lookups
        ));
        assert!(!is_game(&mut cache, None, &mut lookups));
        assert!(is_game(&mut cache, game, &mut lookups));
        assert_eq!(lookups, 3);
    }

    #[test]
    fn rechecks_the_cached_exe_against_a_new_process_name() {
        let mut cache = ForegroundCache::default();
        let game = Some((WindowHandle(1), 10));

        assert!(cache.is_process(game, "game.exe", |_| Some("game.exe".into())));
        assert!(!cache.is_process(game, "other.exe", |_| unreachable!()));
    }
}
