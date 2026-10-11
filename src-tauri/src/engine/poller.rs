//! Poll thread: owns the core, the screen and the clock of the active profile.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender, TryRecvError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rand::Rng;
use uuid::Uuid;

use super::executor::CancelledJobs;
use super::machine::{EngineCore, Job, VisualResults};
use super::EngineStatus;
use super::IconStatus;
use crate::config::{Profile, Tracker};
use crate::foreground::{self, WindowHandle};
use crate::icon_search::{IconResult, TemplateCache};
use crate::keyboard::TimerResolution;
use crate::processes;
use crate::screen::PrimaryScreen;
use crate::screenshots::Screenshots;
use crate::tracker_hotkeys::{desired_tracker_hotkeys, ShortcutGate};
use std::{collections::HashMap, path::PathBuf};

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

pub struct Poller<E, I> {
    pub commands: Receiver<Command>,
    pub jobs: Sender<Job>,
    pub cancelled: Arc<CancelledJobs>,
    pub paused: Arc<AtomicBool>,
    pub polling: Arc<Mutex<()>>,
    pub emit: E,
    pub emit_icon: I,
    pub screenshot_root: PathBuf,
    pub gate: Arc<ShortcutGate>,
    pub presses: Receiver<Uuid>,
    pub key_capture: Arc<AtomicU32>,
}

impl<E: Fn(EngineStatus), I: Fn(IconStatus)> Poller<E, I> {
    /// Runs until the engine handle is dropped; sleeps on the command channel while inactive.
    pub fn run(mut self) {
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

    fn run_active(&mut self, profile: Profile, started: Instant, rng: &mut impl Rng) -> Flow {
        let _resolution = TimerResolution::begin();
        let clock = || started.elapsed();
        let mut session = Session::new(profile, self.screenshot_root.clone());
        let mut deadlines = Deadlines::new(clock(), TICK);
        #[cfg(debug_assertions)]
        let mut tick_log = TickLog::default();

        loop {
            let tick_start = clock();
            #[cfg(debug_assertions)]
            tick_log.record(tick_start);
            let due = deadlines.next(tick_start);

            let polling = self.polling.clone();
            let poll_guard = polling.lock().unwrap_or_else(PoisonError::into_inner);
            let paused = self.paused.load(Ordering::SeqCst);
            let game_active = session.refresh_game(&self.emit);
            let capture = self.key_capture.load(Ordering::SeqCst);
            let desired =
                desired_tracker_hotkeys(true, paused, capture, game_active, session.trackers());
            self.gate.sync(&desired);
            let presses = self.drain_presses(game_active && !paused);
            for job in session.tick(tick_start, paused, &presses, rng, &self.emit_icon) {
                let _ = self.jobs.send(job);
            }
            drop(poll_guard);

            loop {
                let remaining = due.saturating_sub(clock());
                // Always drain settings/completion commands even when matching exceeds the deadline.
                let received = if remaining.is_zero() {
                    match self.commands.try_recv() {
                        Ok(command) => Ok(command),
                        Err(TryRecvError::Empty) => break,
                        Err(TryRecvError::Disconnected) => Err(RecvTimeoutError::Disconnected),
                    }
                } else {
                    self.commands.recv_timeout(remaining)
                };
                let command = match received {
                    Ok(command) => command,
                    Err(RecvTimeoutError::Timeout) => break,
                    Err(RecvTimeoutError::Disconnected) => {
                        self.release_hotkeys();
                        self.cancelled.cancel(session.core.clear());
                        return Flow::Exit;
                    }
                };
                match command {
                    Command::Activate(profile) => {
                        self.release_hotkeys();
                        self.cancelled.cancel(session.core.clear());
                        session = Session::new(profile, self.screenshot_root.clone());
                    }
                    Command::Update(profile) => {
                        self.cancelled.cancel(session.update(profile));
                    }
                    Command::Deactivate => {
                        self.release_hotkeys();
                        self.cancelled.cancel(session.core.clear());
                        return Flow::Deactivated;
                    }
                    Command::ActionDone(ticket) => session.core.action_done(ticket, clock()),
                }
            }
        }
    }

    fn drain_presses(&self, accept: bool) -> Vec<Uuid> {
        let mut presses = Vec::new();
        loop {
            match self.presses.try_recv() {
                Ok(tracker_id) if accept => presses.push(tracker_id),
                Ok(_) => {}
                Err(TryRecvError::Empty | TryRecvError::Disconnected) => break,
            }
        }
        presses
    }

    fn release_hotkeys(&mut self) {
        self.gate.sync(&[]);
        self.drain_presses(false);
    }
}

struct Session {
    profile_id: Uuid,
    process_name: String,
    trackers: Vec<Tracker>,
    core: EngineCore,
    foreground: ForegroundCache,
    game_active: Option<bool>,
    screen: Option<PrimaryScreen>,
    screen_retry_at: Duration,
    last_screen_error: Option<String>,
    screenshots: Screenshots,
    templates: TemplateCache,
    icon_errors: HashMap<Uuid, String>,
}

impl Session {
    fn new(profile: Profile, screenshot_root: PathBuf) -> Self {
        let mut session = Self {
            profile_id: profile.id,
            process_name: String::new(),
            trackers: Vec::new(),
            core: EngineCore::default(),
            foreground: ForegroundCache::default(),
            game_active: None,
            screen: None,
            screen_retry_at: Duration::ZERO,
            last_screen_error: None,
            screenshots: Screenshots::new(screenshot_root),
            templates: TemplateCache::default(),
            icon_errors: HashMap::new(),
        };
        session.update(profile);
        session
    }

    /// Returns tickets of queued jobs of trackers that no longer run.
    fn update(&mut self, profile: Profile) -> Vec<u64> {
        self.process_name = profile.process_name;
        self.trackers = profile.trackers;
        let icons: Vec<_> = self
            .trackers
            .iter()
            .flat_map(|t| t.icons.iter().map(|i| i.id))
            .collect();
        self.templates.retain(&icons);
        if let Some(screen) = &mut self.screen {
            screen.retain_icons(&icons);
        }
        self.icon_errors.retain(|id, _| icons.contains(id));
        self.core.set_trackers(&self.trackers)
    }

    fn trackers(&self) -> &[Tracker] {
        &self.trackers
    }

    fn refresh_game(&mut self, emit: &impl Fn(EngineStatus)) -> bool {
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
        game_active
    }

    fn tick(
        &mut self,
        now: Duration,
        paused: bool,
        presses: &[Uuid],
        rng: &mut impl Rng,
        emit_icon: &impl Fn(IconStatus),
    ) -> Vec<Job> {
        let game_active = self.game_active.unwrap_or(false);
        if paused {
            // The capture session duplicates the same output, which allows one duplication.
            self.screen = None;
        }

        let accepted = if game_active && !paused { presses } else { &[] };
        let Self {
            core,
            screen,
            screen_retry_at,
            last_screen_error,
            templates,
            screenshots,
            icon_errors,
            profile_id,
            process_name,
            ..
        } = self;
        core.tick_visual(
            now,
            paused,
            process_name,
            accepted,
            |batch| {
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
                match screen
                    .as_mut()?
                    .read_visuals_selected(&batch.points, |colors| batch.icon_rectangles(colors))
                {
                    Ok((colors, images)) => {
                        *last_screen_error = None;
                        let icons = images
                            .into_iter()
                            .map(|(id, image)| {
                                let result = match image {
                                    Ok(image) => templates.check(
                                        id,
                                        &screenshots.template_path(*profile_id, id),
                                        &image,
                                    ),
                                    Err(error) => IconResult::Unavailable(error),
                                };
                                let tracker_id = batch
                                    .icons
                                    .iter()
                                    .find(|(_, icon)| icon.id == id)
                                    .unwrap()
                                    .0;
                                let error = match &result {
                                    IconResult::Unavailable(error) => Some(error.clone()),
                                    _ => None,
                                };
                                if icon_status_changed(icon_errors, id, error.clone()) {
                                    emit_icon(IconStatus {
                                        profile_id: *profile_id,
                                        tracker_id,
                                        icon_id: id,
                                        error,
                                    });
                                }
                                (id, result)
                            })
                            .collect();
                        Some(VisualResults { colors, icons })
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

fn icon_status_changed(
    errors: &mut HashMap<Uuid, String>,
    id: Uuid,
    error: Option<String>,
) -> bool {
    match error {
        Some(error) if errors.get(&id) != Some(&error) => {
            errors.insert(id, error);
            true
        }
        None => errors.remove(&id).is_some(),
        _ => false,
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
    fn icon_errors_emit_only_changes_and_recovery_for_the_specific_icon() {
        let mut errors = HashMap::new();
        let id = Uuid::new_v4();
        let second = Uuid::new_v4();
        assert!(!icon_status_changed(&mut errors, id, None));
        assert!(icon_status_changed(&mut errors, id, Some("missing".into())));
        assert!(!icon_status_changed(
            &mut errors,
            id,
            Some("missing".into())
        ));
        assert!(icon_status_changed(
            &mut errors,
            second,
            Some("missing".into())
        ));
        assert!(icon_status_changed(&mut errors, id, Some("corrupt".into())));
        assert!(icon_status_changed(&mut errors, id, None));
        assert!(!icon_status_changed(&mut errors, id, None));
        assert_eq!(errors.get(&second).map(String::as_str), Some("missing"));
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
