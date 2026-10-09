//! Interactive pixel capture: wait for the game, count down, snapshot, let the user pick a point.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use image::RgbImage;

use serde::Serialize;
use uuid::Uuid;

use crate::config::Pixel;
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::foreground::WindowHandle;
use crate::profiles;
use crate::screen::Frame;
use crate::screenshots::{self, Screenshots};

pub const STATUS_EVENT: &str = "pixel-capture://status";
const WAIT_FOR_GAME: Duration = Duration::from_secs(10);
const COUNTDOWN: Duration = Duration::from_millis(200);
const PREPARE_LIMIT: Duration = Duration::from_secs(10);
const FRAME_LIMIT: Duration = Duration::from_secs(10);
const POLL_INTERVAL: Duration = Duration::from_millis(100);
const PREPARE_FAILED: &str = "Не удалось подготовить окно выбора";
const FRAME_FAILED: &str = "Не удалось показать снимок";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum CaptureStatus {
    #[serde(rename_all = "camelCase")]
    WaitingForGame {
        remaining_ms: u64,
    },
    #[serde(rename_all = "camelCase")]
    Countdown {
        remaining_ms: u64,
    },
    Picking,
    Done,
    Cancelled,
    Timeout,
    Failed {
        reason: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureCommand {
    Pick {
        x: u32,
        y: u32,
    },
    Cancel,
    /// The overlay window was destroyed; only ends the session while it shows the picker.
    OverlayClosed,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureRequest {
    pub profile_id: Uuid,
    pub tracker_id: Uuid,
}

/// Clock, desktop and UI the session runs against; faked in tests.
pub trait CaptureEnv {
    /// Monotonic time since the session started.
    fn now(&self) -> Duration;
    fn sleep(&self, duration: Duration);
    /// The foreground window when it belongs to `process_name`.
    fn foreground_game(&self, process_name: &str) -> Option<WindowHandle>;
    /// Starts hidden-window and shutter preparation. Polled with [`prepare_result`].
    fn begin_prepare(&self);
    fn prepare_result(&self) -> Option<AppResult<()>>;
    fn capture_frame(&self) -> AppResult<Frame>;
    fn write_screenshot(&self, path: &Path, image: &RgbImage) -> AppResult<()>;
    /// Tells the already loaded picker page which file to decode. The window stays hidden.
    fn publish_frame(&self, request: CaptureRequest, pixel_id: Uuid) -> AppResult<()>;
    fn frame_result(&self) -> Option<AppResult<()>>;
    fn hide_picker(&self);
    fn minimize_game(&self, game: WindowHandle);
    /// Plays the shutter and reveals the picker as one step.
    fn reveal_picker(&self) -> AppResult<()>;
    /// Hides the picker and activates the main window.
    fn close_picker(&self);
    fn emit(&self, status: CaptureStatus);
}

enum Stop {
    Cancelled,
    Timeout,
    Failed(AppError),
}

impl From<AppError> for Stop {
    fn from(error: AppError) -> Self {
        Stop::Failed(error)
    }
}

pub fn run_session(
    env: &impl CaptureEnv,
    store: &ConfigStore,
    screenshots: &Screenshots,
    request: CaptureRequest,
    commands: &Receiver<CaptureCommand>,
) {
    let pixel_id = Uuid::new_v4();
    let status = match capture(env, store, screenshots, request, pixel_id, commands) {
        Ok(()) => CaptureStatus::Done,
        Err(stop) => {
            screenshots.remove_pixel(request.profile_id, pixel_id);
            match stop {
                Stop::Cancelled => CaptureStatus::Cancelled,
                Stop::Timeout => CaptureStatus::Timeout,
                Stop::Failed(error) => CaptureStatus::Failed {
                    reason: error.to_string(),
                },
            }
        }
    };
    env.emit(status);
}

fn capture(
    env: &impl CaptureEnv,
    store: &ConfigStore,
    screenshots: &Screenshots,
    request: CaptureRequest,
    pixel_id: Uuid,
    commands: &Receiver<CaptureCommand>,
) -> Result<(), Stop> {
    let process_name =
        profiles::profile_process_name(store, request.profile_id, request.tracker_id)?;
    let prepare_started = env.now();
    env.begin_prepare();

    let game = match wait_for_game(env, &process_name, commands) {
        Ok(game) => match count_down(env, commands) {
            Ok(()) => Ok(game),
            Err(stop) => Err(stop),
        },
        Err(stop) => Err(stop),
    };
    let game = match game {
        Ok(game) => game,
        Err(stop) => {
            finish_prepare(env, prepare_started);
            return Err(stop);
        }
    };
    match wait_for_prepare(env, commands, prepare_started) {
        Ok(()) => {}
        Err(Stop::Cancelled) => {
            finish_prepare(env, prepare_started);
            return Err(Stop::Cancelled);
        }
        Err(stop) => return Err(stop),
    }

    let frame = env.capture_frame()?;
    let image = frame.to_rgb_image();
    env.write_screenshot(
        &screenshots.image_path(request.profile_id, pixel_id),
        &image,
    )?;
    if let Err(error) = env.publish_frame(request, pixel_id) {
        env.hide_picker();
        return Err(error.into());
    }
    if let Err(stop) = wait_for_frame(env, commands) {
        env.hide_picker();
        return Err(stop);
    }

    env.minimize_game(game);
    env.reveal_picker()?;
    env.emit(CaptureStatus::Picking);
    let picked = pick(
        store,
        screenshots,
        request,
        pixel_id,
        &frame,
        &image,
        commands,
    );
    env.close_picker();
    picked
}

fn wait_for_prepare(
    env: &impl CaptureEnv,
    commands: &Receiver<CaptureCommand>,
    started: Duration,
) -> Result<(), Stop> {
    loop {
        check_cancelled(commands)?;
        match env.prepare_result() {
            Some(result) => return result.map_err(Stop::from),
            None => {
                let elapsed = env.now().saturating_sub(started);
                if elapsed >= PREPARE_LIMIT {
                    return Err(Stop::Failed(AppError::Capture(PREPARE_FAILED.into())));
                }
                env.sleep(POLL_INTERVAL.min(PREPARE_LIMIT - elapsed));
            }
        }
    }
}

/// Waits out preparation after the session has already stopped, so a late window cannot appear.
fn finish_prepare(env: &impl CaptureEnv, started: Duration) {
    loop {
        if env.prepare_result().is_some() {
            return;
        }
        let elapsed = env.now().saturating_sub(started);
        if elapsed >= PREPARE_LIMIT {
            return;
        }
        env.sleep(POLL_INTERVAL.min(PREPARE_LIMIT - elapsed));
    }
}

fn wait_for_frame(env: &impl CaptureEnv, commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    let started = env.now();
    loop {
        check_cancelled(commands)?;
        match env.frame_result() {
            Some(Ok(())) => return Ok(()),
            Some(Err(error)) => return Err(error.into()),
            None => {
                let elapsed = env.now().saturating_sub(started);
                if elapsed >= FRAME_LIMIT {
                    return Err(Stop::Failed(AppError::Capture(FRAME_FAILED.into())));
                }
                env.sleep(POLL_INTERVAL.min(FRAME_LIMIT - elapsed));
            }
        }
    }
}

fn wait_for_game(
    env: &impl CaptureEnv,
    process_name: &str,
    commands: &Receiver<CaptureCommand>,
) -> Result<WindowHandle, Stop> {
    let started = env.now();
    loop {
        check_cancelled(commands)?;
        if let Some(game) = env.foreground_game(process_name) {
            return Ok(game);
        }
        let remaining = WAIT_FOR_GAME.saturating_sub(env.now() - started);
        if remaining.is_zero() {
            return Err(Stop::Timeout);
        }
        env.emit(CaptureStatus::WaitingForGame {
            remaining_ms: remaining.as_millis() as u64,
        });
        env.sleep(POLL_INTERVAL.min(remaining));
    }
}

/// The game may lose focus during the countdown; the snapshot is taken anyway.
fn count_down(env: &impl CaptureEnv, commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    let started = env.now();
    loop {
        check_cancelled(commands)?;
        let remaining = COUNTDOWN.saturating_sub(env.now() - started);
        if remaining.is_zero() {
            return Ok(());
        }
        env.emit(CaptureStatus::Countdown {
            remaining_ms: remaining.as_millis() as u64,
        });
        env.sleep(POLL_INTERVAL.min(remaining));
    }
}

fn check_cancelled(commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    loop {
        match commands.try_recv() {
            Ok(CaptureCommand::Cancel) | Err(TryRecvError::Disconnected) => {
                return Err(Stop::Cancelled)
            }
            Ok(CaptureCommand::Pick { .. } | CaptureCommand::OverlayClosed) => {}
            Err(TryRecvError::Empty) => return Ok(()),
        }
    }
}

fn pick(
    store: &ConfigStore,
    screenshots: &Screenshots,
    request: CaptureRequest,
    pixel_id: Uuid,
    frame: &Frame,
    image: &RgbImage,
    commands: &Receiver<CaptureCommand>,
) -> Result<(), Stop> {
    loop {
        match commands.recv() {
            Ok(CaptureCommand::Pick { x, y }) => {
                let Some(color) = frame.rgb_at(x, y) else {
                    continue;
                };
                let mut thumbnail = screenshots::thumbnail(image);
                screenshots::draw_marker(&mut thumbnail, frame.width, x, y);
                screenshots::save_png(
                    &screenshots.thumbnail_path(request.profile_id, pixel_id),
                    &thumbnail,
                )?;
                let pixel = Pixel::new(pixel_id, x, y, color);
                profiles::add_pixel(store, request.profile_id, request.tracker_id, pixel)?;
                return Ok(());
            }
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed) | Err(_) => {
                return Err(Stop::Cancelled)
            }
        }
    }
}

/// Runs at most one capture session at a time and routes commands to it.
#[derive(Default)]
pub struct PixelCapture {
    active: Arc<Mutex<Option<Sender<CaptureCommand>>>>,
}

impl PixelCapture {
    pub fn start(
        &self,
        session: impl FnOnce(Receiver<CaptureCommand>) + Send + 'static,
    ) -> AppResult<JoinHandle<()>> {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Err(AppError::Validation("Захват пикселя уже идёт".into()));
        }
        let (sender, receiver) = mpsc::channel();
        *active = Some(sender);

        let release = ReleaseOnDrop(self.active.clone());
        Ok(thread::spawn(move || {
            let _release = release;
            session(receiver);
        }))
    }

    pub fn send(&self, command: CaptureCommand) -> AppResult<()> {
        match &*lock(&self.active) {
            Some(sender) => {
                let _ = sender.send(command);
                Ok(())
            }
            None => Err(AppError::NotFound("Захват пикселя не идёт")),
        }
    }
}

/// Frees the session slot even if the session panics.
struct ReleaseOnDrop(Arc<Mutex<Option<Sender<CaptureCommand>>>>);

impl Drop for ReleaseOnDrop {
    fn drop(&mut self) {
        *lock(&self.0) = None;
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::*;
    use crate::config::{PixelCondition, Rgb};
    use crate::profiles::ProfileInput;
    use image::RgbImage;
    use tempfile::TempDir;

    const GAME: WindowHandle = WindowHandle(42);

    #[derive(Default)]
    enum Outcome {
        #[default]
        Pending,
        Ready,
        Failed(String),
    }

    #[derive(Default)]
    struct FakeEnv {
        now: Mutex<Duration>,
        /// Virtual time from which the game window is in the foreground.
        game_active_from: Option<Duration>,
        /// Sends `Cancel` once virtual time reaches this point.
        cancel_at: Mutex<Option<(Duration, Sender<CaptureCommand>)>>,
        /// What the user does once the picker is shown.
        on_picker: Mutex<Option<(Vec<CaptureCommand>, Sender<CaptureCommand>)>>,
        /// Becomes ready once virtual time reaches this point. `None` means already ready.
        prepare_ready_at: Option<Duration>,
        prepare_error: Option<String>,
        prepare_hangs: bool,
        fails_capture: bool,
        fails_frame: bool,
        prepare: Mutex<Outcome>,
        frame: Mutex<Outcome>,
        calls: Mutex<Vec<String>>,
        statuses: Mutex<Vec<CaptureStatus>>,
    }

    impl FakeEnv {
        fn statuses(&self) -> Vec<CaptureStatus> {
            lock(&self.statuses).clone()
        }

        fn calls(&self) -> Vec<String> {
            lock(&self.calls).clone()
        }

        fn last_status(&self) -> CaptureStatus {
            self.statuses()
                .last()
                .cloned()
                .expect("a status must be emitted")
        }

        fn outcome(outcome: &Outcome) -> Option<AppResult<()>> {
            match outcome {
                Outcome::Pending => None,
                Outcome::Ready => Some(Ok(())),
                Outcome::Failed(reason) => Some(Err(AppError::Capture(reason.clone()))),
            }
        }
    }

    impl CaptureEnv for FakeEnv {
        fn now(&self) -> Duration {
            *lock(&self.now)
        }

        fn sleep(&self, duration: Duration) {
            let now = {
                let mut now = lock(&self.now);
                *now += duration;
                *now
            };
            if self.prepare_ready_at.is_some_and(|at| now >= at) {
                let mut prepare = lock(&self.prepare);
                if matches!(*prepare, Outcome::Pending) {
                    *prepare = Outcome::Ready;
                }
            }
            let mut cancel_at = lock(&self.cancel_at);
            if cancel_at.as_ref().is_some_and(|(at, _)| now >= *at) {
                let (_, sender) = cancel_at.take().unwrap();
                sender.send(CaptureCommand::Cancel).unwrap();
            }
        }

        fn foreground_game(&self, process_name: &str) -> Option<WindowHandle> {
            assert_eq!(process_name, "game.exe");
            self.game_active_from
                .filter(|from| self.now() >= *from)
                .map(|_| GAME)
        }

        fn capture_frame(&self) -> AppResult<Frame> {
            lock(&self.calls).push(format!("capture at {}ms", self.now().as_millis()));
            if self.fails_capture {
                return Err(AppError::Capture("Не удалось захватить экран".into()));
            }
            Ok(Frame {
                width: 4,
                height: 2,
                bgra: [[1, 2, 3, 255], [30, 30, 200, 255]].repeat(4).concat(),
            })
        }

        fn begin_prepare(&self) {
            lock(&self.calls).push("prepare".into());
            *lock(&self.prepare) = if let Some(reason) = &self.prepare_error {
                Outcome::Failed(reason.clone())
            } else if self.prepare_hangs || self.prepare_ready_at.is_some() {
                Outcome::Pending
            } else {
                Outcome::Ready
            };
        }

        fn prepare_result(&self) -> Option<AppResult<()>> {
            Self::outcome(&lock(&self.prepare))
        }

        fn write_screenshot(&self, path: &Path, image: &RgbImage) -> AppResult<()> {
            lock(&self.calls).push("write png".into());
            crate::screenshots::save_stored_png(path, image)
        }

        fn publish_frame(&self, _: CaptureRequest, _: Uuid) -> AppResult<()> {
            lock(&self.calls).push("publish frame".into());
            *lock(&self.frame) = if self.fails_frame {
                Outcome::Failed(FRAME_FAILED.into())
            } else {
                Outcome::Ready
            };
            Ok(())
        }

        fn frame_result(&self) -> Option<AppResult<()>> {
            Self::outcome(&lock(&self.frame))
        }

        fn hide_picker(&self) {
            lock(&self.calls).push("hide picker".into());
        }

        fn minimize_game(&self, game: WindowHandle) {
            assert_eq!(game, GAME);
            lock(&self.calls).push("minimize".into());
        }

        fn reveal_picker(&self) -> AppResult<()> {
            let mut calls = lock(&self.calls);
            calls.push("shutter".into());
            calls.push("show picker".into());
            drop(calls);
            if let Some((commands, sender)) = lock(&self.on_picker).take() {
                for command in commands {
                    sender.send(command).unwrap();
                }
            }
            Ok(())
        }

        fn close_picker(&self) {
            lock(&self.calls).push("close picker".into());
        }

        fn emit(&self, status: CaptureStatus) {
            lock(&self.statuses).push(status);
        }
    }

    struct Fixture {
        _dir: TempDir,
        store: Arc<ConfigStore>,
        screenshots: Arc<Screenshots>,
        request: CaptureRequest,
    }

    impl Fixture {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let store = ConfigStore::load(dir.path()).unwrap().store;
            let profile = profiles::create_profile(
                &store,
                ProfileInput {
                    name: "Diablo".into(),
                    process_name: "game.exe".into(),
                },
            )
            .unwrap();
            let tracker = profiles::create_tracker(&store, profile.id, "Здоровье".into()).unwrap();
            Self {
                screenshots: Arc::new(Screenshots::new(dir.path().join("screenshots"))),
                _dir: dir,
                store: Arc::new(store),
                request: CaptureRequest {
                    profile_id: profile.id,
                    tracker_id: tracker.id,
                },
            }
        }

        fn run(
            &self,
            env: &FakeEnv,
            sender: Sender<CaptureCommand>,
            receiver: Receiver<CaptureCommand>,
        ) {
            run_session(env, &self.store, &self.screenshots, self.request, &receiver);
            drop(sender);
        }

        fn pixels(&self) -> Vec<Pixel> {
            profiles::list_profiles(&self.store)[0].trackers[0]
                .pixels
                .clone()
        }

        fn screenshot_files(&self) -> usize {
            std::fs::read_dir(self.screenshots.profile_dir(self.request.profile_id))
                .map(|entries| entries.count())
                .unwrap_or(0)
        }
    }

    type PickerActions = Mutex<Option<(Vec<CaptureCommand>, Sender<CaptureCommand>)>>;

    fn picker_actions(
        commands: &[CaptureCommand],
        sender: &Sender<CaptureCommand>,
    ) -> PickerActions {
        Mutex::new(Some((commands.to_vec(), sender.clone())))
    }

    #[test]
    fn times_out_when_the_game_is_never_activated() {
        let fixture = Fixture::new();
        let env = FakeEnv::default();
        let (sender, receiver) = mpsc::channel();

        fixture.run(&env, sender, receiver);

        let statuses = env.statuses();
        assert_eq!(
            statuses[0],
            CaptureStatus::WaitingForGame {
                remaining_ms: 10_000
            }
        );
        assert_eq!(env.last_status(), CaptureStatus::Timeout);
        assert_eq!(env.now(), WAIT_FOR_GAME);
        assert!(!statuses
            .iter()
            .any(|s| matches!(s, CaptureStatus::Countdown { .. })));
        assert!(!has_shutter_or_show(&env.calls()));
        assert!(fixture.pixels().is_empty());
    }

    #[test]
    fn snapshots_200_ms_after_the_game_is_activated_and_adds_the_picked_pixel() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::from_millis(2_500)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 1, y: 1 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(
            env.calls(),
            [
                "prepare",
                "capture at 2700ms",
                "write png",
                "publish frame",
                "minimize",
                "shutter",
                "show picker",
                "close picker",
            ]
        );
        let statuses = env.statuses();
        assert!(statuses.contains(&CaptureStatus::Countdown {
            remaining_ms: 200
        }));
        assert_eq!(
            statuses[statuses.len() - 2..],
            [CaptureStatus::Picking, CaptureStatus::Done]
        );

        let pixels = fixture.pixels();
        assert_eq!(pixels.len(), 1);
        assert_eq!((pixels[0].x, pixels[0].y), (1, 1));
        assert_eq!(
            pixels[0].color,
            Rgb {
                r: 200,
                g: 30,
                b: 30
            }
        );
        assert_eq!(pixels[0].condition, PixelCondition::Equals);
        assert_eq!(pixels[0].tolerance_percent, 10);
        let id = pixels[0].id;
        assert!(fixture
            .screenshots
            .image_path(fixture.request.profile_id, id)
            .exists());
        assert!(fixture
            .screenshots
            .thumbnail_path(fixture.request.profile_id, id)
            .exists());
    }

    #[test]
    fn ignores_clicks_outside_the_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            on_picker: picker_actions(
                &[
                    CaptureCommand::Pick { x: 4, y: 0 },
                    CaptureCommand::Pick { x: 0, y: 0 },
                ],
                &sender,
            ),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Done);
        assert_eq!((fixture.pixels()[0].x, fixture.pixels()[0].y), (0, 0));
    }

    #[test]
    fn user_cancel_during_countdown_takes_no_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::from_secs(1)),
            cancel_at: Mutex::new(Some((Duration::from_millis(1_100), sender.clone()))),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(!has_shutter_or_show(&env.calls()));
        assert!(fixture.pixels().is_empty());
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn escape_in_the_picker_discards_the_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            on_picker: picker_actions(&[CaptureCommand::Cancel], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        let calls = env.calls();
        let shutter = calls.iter().position(|call| call == "shutter").unwrap();
        assert_eq!(calls[shutter + 1], "show picker");
        assert!(calls[..shutter].iter().any(|call| call == "write png"));
        assert!(fixture.pixels().is_empty());
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn closing_the_overlay_while_waiting_does_not_cancel() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };
        sender.send(CaptureCommand::OverlayClosed).unwrap();

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Done);
    }

    #[test]
    fn reports_a_failed_screen_capture() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            fails_capture: true,
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(
            env.last_status(),
            CaptureStatus::Failed {
                reason: "Не удалось захватить экран".into()
            }
        );
        assert!(!has_shutter_or_show(&env.calls()));
        assert!(!env.calls().iter().any(|call| call == "write png"));
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn prepare_failure_ends_before_the_shutter_and_leaves_no_file() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            prepare_error: Some(PREPARE_FAILED.into()),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(
            env.last_status(),
            CaptureStatus::Failed {
                reason: PREPARE_FAILED.into()
            }
        );
        assert!(!has_shutter_or_show(&env.calls()));
        assert!(!env
            .calls()
            .iter()
            .any(|call| call == "show picker" || call == "write png"));
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn prepare_timeout_ends_before_the_shutter_and_leaves_no_file() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            prepare_hangs: true,
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.now(), PREPARE_LIMIT);
        assert_eq!(
            env.last_status(),
            CaptureStatus::Failed {
                reason: PREPARE_FAILED.into()
            }
        );
        assert!(!has_shutter_or_show(&env.calls()));
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn snapshot_waits_for_prepare_that_is_still_running_after_the_countdown() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            prepare_ready_at: Some(Duration::from_secs(5)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert!(env.calls().contains(&"capture at 5000ms".to_string()));
        let calls = env.calls();
        let shutter = calls.iter().position(|call| call == "shutter").unwrap();
        assert_eq!(calls[shutter + 1], "show picker");
        assert!(calls.iter().position(|call| call == "write png").unwrap() < shutter);
    }

    #[test]
    fn prepare_that_finishes_during_the_countdown_does_not_delay_the_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            prepare_ready_at: Some(Duration::from_millis(100)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert!(env.calls().contains(&"capture at 200ms".to_string()));
        assert_eq!(env.last_status(), CaptureStatus::Done);
    }

    #[test]
    fn frame_show_failure_removes_the_file_and_skips_the_shutter() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            fails_frame: true,
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(
            env.last_status(),
            CaptureStatus::Failed {
                reason: FRAME_FAILED.into()
            }
        );
        let calls = env.calls();
        assert!(calls.iter().any(|call| call == "write png"));
        assert!(calls.iter().any(|call| call == "hide picker"));
        assert!(!has_shutter_or_show(&calls));
        assert_eq!(fixture.screenshot_files(), 0);
    }

    fn has_shutter_or_show(calls: &[String]) -> bool {
        calls
            .iter()
            .any(|call| call == "shutter" || call == "show picker")
    }

    #[test]
    fn allows_one_session_at_a_time() {
        let fixture = Fixture::new();
        let capture = PixelCapture::default();
        let env = Arc::new(FakeEnv {
            game_active_from: Some(Duration::ZERO),
            ..FakeEnv::default()
        });

        let session = {
            let (env, store, screenshots, request) = (
                env.clone(),
                fixture.store.clone(),
                fixture.screenshots.clone(),
                fixture.request,
            );
            move |commands: Receiver<CaptureCommand>| {
                run_session(&*env, &store, &screenshots, request, &commands)
            }
        };
        let first = capture.start(session).unwrap();
        while !env.statuses().contains(&CaptureStatus::Picking) {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(matches!(
            capture.start(|_| {}),
            Err(AppError::Validation(_))
        ));

        capture.send(CaptureCommand::Cancel).unwrap();
        first.join().unwrap();
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(capture.send(CaptureCommand::Cancel).is_err());

        let second = capture.start(|_| {}).unwrap();
        second.join().unwrap();
    }

    #[test]
    fn serializes_status_for_the_frontend() {
        let json = |status| serde_json::to_value(status).unwrap();

        assert_eq!(
            json(CaptureStatus::WaitingForGame {
                remaining_ms: 9_900
            }),
            serde_json::json!({ "phase": "waitingForGame", "remainingMs": 9_900 })
        );
        assert_eq!(
            json(CaptureStatus::Timeout),
            serde_json::json!({ "phase": "timeout" })
        );
        assert_eq!(
            json(CaptureStatus::Failed { reason: "x".into() }),
            serde_json::json!({ "phase": "failed", "reason": "x" })
        );
    }
}
