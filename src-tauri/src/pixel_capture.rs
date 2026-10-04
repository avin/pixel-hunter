//! Interactive pixel capture: wait for the game, count down, snapshot, let the user pick a point.

use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

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
const COUNTDOWN: Duration = Duration::from_secs(3);
const POLL_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum CaptureStatus {
    #[serde(rename_all = "camelCase")]
    WaitingForGame { remaining_ms: u64 },
    #[serde(rename_all = "camelCase")]
    Countdown { remaining_ms: u64 },
    Picking,
    Done,
    Cancelled,
    Timeout,
    Failed { reason: String },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum CaptureCommand {
    Pick { x: u32, y: u32 },
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
    fn capture_frame(&self) -> AppResult<Frame>;
    fn play_shutter(&self);
    /// Minimizes the game and shows the full-screen picker for the saved screenshot.
    fn show_picker(&self, game: WindowHandle, request: CaptureRequest, pixel_id: Uuid) -> AppResult<()>;
    /// Closes the picker and activates the main window.
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
    let process_name = profiles::profile_process_name(store, request.profile_id, request.tracker_id)?;
    let game = wait_for_game(env, &process_name, commands)?;
    count_down(env, commands)?;

    let frame = env.capture_frame()?;
    env.play_shutter();
    let image = frame.to_rgb_image();
    screenshots::save_png(&screenshots.image_path(request.profile_id, pixel_id), &image)?;
    let thumbnail = screenshots::thumbnail(&image);
    drop(image);

    env.show_picker(game, request, pixel_id)?;
    env.emit(CaptureStatus::Picking);
    let picked = pick(store, screenshots, request, pixel_id, &frame, thumbnail, commands);
    env.close_picker();
    picked
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
    mut thumbnail: image::RgbImage,
    commands: &Receiver<CaptureCommand>,
) -> Result<(), Stop> {
    loop {
        match commands.recv() {
            Ok(CaptureCommand::Pick { x, y }) => {
                let Some(color) = frame.rgb_at(x, y) else {
                    continue;
                };
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
    use super::*;
    use crate::config::{PixelCondition, Rgb};
    use crate::profiles::ProfileInput;
    use tempfile::TempDir;

    const GAME: WindowHandle = WindowHandle(42);

    #[derive(Default)]
    struct FakeEnv {
        now: Mutex<Duration>,
        /// Virtual time from which the game window is in the foreground.
        game_active_from: Option<Duration>,
        /// Sends `Cancel` once virtual time reaches this point.
        cancel_at: Mutex<Option<(Duration, Sender<CaptureCommand>)>>,
        /// What the user does once the picker is shown.
        on_picker: Mutex<Option<(Vec<CaptureCommand>, Sender<CaptureCommand>)>>,
        fails_capture: bool,
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
            self.statuses().last().cloned().expect("a status must be emitted")
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

        fn play_shutter(&self) {
            lock(&self.calls).push("shutter".into());
        }

        fn show_picker(&self, game: WindowHandle, _: CaptureRequest, _: Uuid) -> AppResult<()> {
            assert_eq!(game, GAME);
            lock(&self.calls).push("show picker".into());
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

        fn run(&self, env: &FakeEnv, sender: Sender<CaptureCommand>, receiver: Receiver<CaptureCommand>) {
            run_session(env, &self.store, &self.screenshots, self.request, &receiver);
            drop(sender);
        }

        fn pixels(&self) -> Vec<Pixel> {
            profiles::list_profiles(&self.store)[0].trackers[0].pixels.clone()
        }

        fn screenshot_files(&self) -> usize {
            std::fs::read_dir(self.screenshots.profile_dir(self.request.profile_id))
                .map(|entries| entries.count())
                .unwrap_or(0)
        }
    }

    type PickerActions = Mutex<Option<(Vec<CaptureCommand>, Sender<CaptureCommand>)>>;

    fn picker_actions(commands: &[CaptureCommand], sender: &Sender<CaptureCommand>) -> PickerActions {
        Mutex::new(Some((commands.to_vec(), sender.clone())))
    }

    #[test]
    fn times_out_when_the_game_is_never_activated() {
        let fixture = Fixture::new();
        let env = FakeEnv::default();
        let (sender, receiver) = mpsc::channel();

        fixture.run(&env, sender, receiver);

        let statuses = env.statuses();
        assert_eq!(statuses[0], CaptureStatus::WaitingForGame { remaining_ms: 10_000 });
        assert_eq!(env.last_status(), CaptureStatus::Timeout);
        assert_eq!(env.now(), WAIT_FOR_GAME);
        assert!(!statuses.iter().any(|s| matches!(s, CaptureStatus::Countdown { .. })));
        assert!(env.calls().is_empty());
        assert!(fixture.pixels().is_empty());
    }

    #[test]
    fn snapshots_three_seconds_after_the_game_is_activated_and_adds_the_picked_pixel() {
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
            ["capture at 5500ms", "shutter", "show picker", "close picker"]
        );
        let statuses = env.statuses();
        assert!(statuses.contains(&CaptureStatus::Countdown { remaining_ms: 3_000 }));
        assert_eq!(statuses[statuses.len() - 2..], [CaptureStatus::Picking, CaptureStatus::Done]);

        let pixels = fixture.pixels();
        assert_eq!(pixels.len(), 1);
        assert_eq!((pixels[0].x, pixels[0].y), (1, 1));
        assert_eq!(pixels[0].color, Rgb { r: 200, g: 30, b: 30 });
        assert_eq!(pixels[0].condition, PixelCondition::Equals);
        assert_eq!(pixels[0].tolerance_percent, 10);
        let id = pixels[0].id;
        assert!(fixture.screenshots.image_path(fixture.request.profile_id, id).exists());
        assert!(fixture.screenshots.thumbnail_path(fixture.request.profile_id, id).exists());
    }

    #[test]
    fn ignores_clicks_outside_the_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            game_active_from: Some(Duration::ZERO),
            on_picker: picker_actions(
                &[CaptureCommand::Pick { x: 4, y: 0 }, CaptureCommand::Pick { x: 0, y: 0 }],
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
            cancel_at: Mutex::new(Some((Duration::from_millis(2_500), sender.clone()))),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(env.calls().is_empty());
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
        assert_eq!(env.calls()[1..], ["shutter", "show picker", "close picker"]);
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
        assert!(!env.calls().contains(&"show picker".to_string()));
        assert_eq!(fixture.screenshot_files(), 0);
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
            let (env, store, screenshots, request) =
                (env.clone(), fixture.store.clone(), fixture.screenshots.clone(), fixture.request);
            move |commands: Receiver<CaptureCommand>| {
                run_session(&*env, &store, &screenshots, request, &commands)
            }
        };
        let first = capture.start(session).unwrap();
        while !env.statuses().contains(&CaptureStatus::Picking) {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(matches!(capture.start(|_| {}), Err(AppError::Validation(_))));

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
            json(CaptureStatus::WaitingForGame { remaining_ms: 9_900 }),
            serde_json::json!({ "phase": "waitingForGame", "remainingMs": 9_900 })
        );
        assert_eq!(json(CaptureStatus::Timeout), serde_json::json!({ "phase": "timeout" }));
        assert_eq!(
            json(CaptureStatus::Failed { reason: "x".into() }),
            serde_json::json!({ "phase": "failed", "reason": "x" })
        );
    }
}
