//! One session owns PrintScreen, hidden editor preparation and the captured frame.
//! The session slot is released only after hook/editor cleanup and capture pause have dropped.
//! PrintScreen and cancellation are serialized through its command channel; editor presses are ignored.

use std::path::Path;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use image::RgbImage;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::config::{Icon, PhysicalRect, Pixel};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::profiles;
use crate::screen::Frame;
use crate::screenshots::{self, Screenshots};

pub const STATUS_EVENT: &str = "pixel-capture://status";
const PREPARE_LIMIT: Duration = Duration::from_secs(10);
const FRAME_LIMIT: Duration = Duration::from_secs(10);
const PREPARE_FAILED: &str = "Не удалось подготовить окно выбора";
const FRAME_FAILED: &str = "Не удалось показать снимок";

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "phase", rename_all = "camelCase")]
pub enum CaptureStatus {
    Starting,
    WaitingForPrintScreen,
    Capturing,
    Picking,
    SelectingSearch,
    SelectingTemplate,
    Saving,
    Done,
    Cancelled,
    Failed { reason: String },
}

#[derive(Debug, Clone)]
pub enum CaptureCommand {
    PrintScreen,
    PrepareFailed(String),
    PrepareReady,
    FrameReady,
    FrameFailed,
    Pick {
        x: u32,
        y: u32,
    },
    Cancel,
    /// The overlay window was destroyed; only ends the session while it shows the picker.
    OverlayClosed,
    Select {
        pixel_id: Uuid,
        rect: PhysicalRect,
        stage: SelectionStage,
        reply: Sender<AppResult<SelectionReply>>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CaptureMode {
    #[default]
    Pixel,
    Icon,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SelectionStage {
    Search,
    Template,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectionReply {
    pub stage: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub search_rect: Option<PhysicalRect>,
}

#[derive(Debug, Clone, Copy)]
pub struct CaptureRequest {
    pub session_id: Uuid,
    pub profile_id: Uuid,
    pub tracker_id: Uuid,
    pub mode: CaptureMode,
}

/// Clock, desktop and UI the session runs against; faked in tests.
pub trait CaptureEnv {
    /// Monotonic time since the session started.
    fn now(&self) -> Duration;
    /// Blocks until an input/readiness notification or the technical fault deadline.
    fn wait_notification(
        &self,
        commands: &Receiver<CaptureCommand>,
        duration: Duration,
    ) -> Option<CaptureCommand> {
        match commands.recv_timeout(duration) {
            Ok(command) => Some(command),
            Err(mpsc::RecvTimeoutError::Timeout) => None,
            Err(mpsc::RecvTimeoutError::Disconnected) => Some(CaptureCommand::Cancel),
        }
    }
    fn begin_wait(&self) -> AppResult<()>;
    fn end_wait(&self);
    fn cleanup_prepare(&self) {}
    /// Starts hidden preparation; readiness changes wake the session command channel.
    fn begin_prepare(&self);
    fn prepare_result(&self) -> Option<AppResult<()>>;
    fn capture_frame(&self) -> AppResult<Frame>;
    fn write_screenshot(&self, path: &Path, image: &RgbImage) -> AppResult<()>;
    /// Publishes in-memory pixels for the loaded picker page; the window stays hidden.
    fn publish_frame(
        &self,
        request: CaptureRequest,
        pixel_id: Uuid,
        frame: &Frame,
    ) -> AppResult<()>;
    fn frame_result(&self) -> Option<AppResult<()>>;
    fn hide_picker(&self);
    /// Plays the shutter and reveals the picker as one step.
    fn reveal_picker(&self) -> AppResult<()>;
    /// Hides the picker and activates the main window.
    fn close_picker(&self);
    /// Acknowledges a validated selection; final choices close before persistence starts.
    fn selection_accepted(&self, final_selection: bool);
    fn emit(&self, status: CaptureStatus);
}

enum Stop {
    Cancelled,
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
            screenshots.remove_icon(request.profile_id, pixel_id);
            match stop {
                Stop::Cancelled => CaptureStatus::Cancelled,
                Stop::Failed(error) => CaptureStatus::Failed {
                    reason: error.to_string(),
                },
            }
        }
    };
    env.end_wait();
    env.cleanup_prepare();
    env.hide_picker();
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
    env.emit(CaptureStatus::Starting);
    profiles::profile_process_name(store, request.profile_id, request.tracker_id)?;
    let mut early_press = false;
    loop {
        match commands.try_recv() {
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed)
            | Err(TryRecvError::Disconnected) => return Err(Stop::Cancelled),
            Ok(CaptureCommand::PrintScreen) => early_press = true,
            Ok(CaptureCommand::Select { reply, .. }) => {
                let _ = reply.send(Err(AppError::Validation("Выбор ещё не готов".into())));
            }
            Ok(_) => {}
            Err(TryRecvError::Empty) => break,
        }
    }
    env.begin_wait()?;
    let prepare_started = env.now();
    env.begin_prepare();
    env.emit(CaptureStatus::WaitingForPrintScreen);
    let triggered = if early_press {
        Ok(())
    } else {
        wait_for_printscreen(commands)
    };
    if let Err(stop) = triggered {
        env.end_wait();
        return Err(stop);
    }
    env.emit(CaptureStatus::Capturing);
    // Capture before waiting for the cold editor to finish loading.
    let frame = env.capture_frame()?;
    if let Err(stop) = wait_for_prepare(env, commands, prepare_started) {
        env.end_wait();
        return Err(stop);
    }
    if let Err(error) = env.publish_frame(request, pixel_id, &frame) {
        env.hide_picker();
        return Err(error.into());
    }
    if let Err(stop) = wait_for_frame(env, commands) {
        env.hide_picker();
        return Err(stop);
    }

    if let Err(error) = env.reveal_picker() {
        env.close_picker();
        return Err(error.into());
    }
    env.emit(match request.mode {
        CaptureMode::Pixel => CaptureStatus::Picking,
        CaptureMode::Icon => CaptureStatus::SelectingSearch,
    });
    let mut editor_closed = false;
    let picked = pick(
        store,
        screenshots,
        request,
        pixel_id,
        &frame,
        commands,
        env,
        &mut editor_closed,
    );
    if !editor_closed {
        env.close_picker();
    }
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
                handle_readiness_command(env.wait_notification(commands, PREPARE_LIMIT - elapsed))?;
            }
        }
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
                handle_readiness_command(env.wait_notification(commands, FRAME_LIMIT - elapsed))?;
            }
        }
    }
}

fn handle_readiness_command(command: Option<CaptureCommand>) -> Result<(), Stop> {
    match command {
        Some(CaptureCommand::Cancel | CaptureCommand::OverlayClosed) => Err(Stop::Cancelled),
        Some(CaptureCommand::PrepareFailed(reason)) => Err(Stop::Failed(AppError::Capture(reason))),
        Some(CaptureCommand::FrameFailed) => {
            Err(Stop::Failed(AppError::Capture(FRAME_FAILED.into())))
        }
        Some(CaptureCommand::Select { reply, .. }) => {
            let _ = reply.send(Err(AppError::Validation("Выбор ещё не готов".into())));
            Ok(())
        }
        _ => Ok(()),
    }
}

fn wait_for_printscreen(commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    loop {
        match commands.recv() {
            Ok(CaptureCommand::PrintScreen) => return Ok(()),
            Ok(CaptureCommand::PrepareFailed(reason)) => {
                return Err(Stop::Failed(AppError::Capture(reason)))
            }
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed) | Err(_) => {
                return Err(Stop::Cancelled)
            }
            Ok(CaptureCommand::Select { reply, .. }) => {
                let _ = reply.send(Err(AppError::Validation("Выбор ещё не готов".into())));
            }
            Ok(
                CaptureCommand::Pick { .. }
                | CaptureCommand::PrepareReady
                | CaptureCommand::FrameReady
                | CaptureCommand::FrameFailed,
            ) => {}
        }
    }
}

fn check_cancelled(commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    loop {
        match commands.try_recv() {
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed)
            | Err(TryRecvError::Disconnected) => return Err(Stop::Cancelled),
            Ok(
                CaptureCommand::PrepareFailed(_)
                | CaptureCommand::PrepareReady
                | CaptureCommand::FrameReady
                | CaptureCommand::FrameFailed
                | CaptureCommand::PrintScreen
                | CaptureCommand::Pick { .. },
            ) => {}
            Ok(CaptureCommand::Select { reply, .. }) => {
                let _ = reply.send(Err(AppError::Validation("Выбор ещё не готов".into())));
            }
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
    commands: &Receiver<CaptureCommand>,
    env: &impl CaptureEnv,
    editor_closed: &mut bool,
) -> Result<(), Stop> {
    let mut search_rect = None;
    loop {
        match commands.recv() {
            Ok(
                CaptureCommand::PrintScreen
                | CaptureCommand::PrepareFailed(_)
                | CaptureCommand::PrepareReady
                | CaptureCommand::FrameReady
                | CaptureCommand::FrameFailed,
            ) => {}
            Ok(CaptureCommand::Pick { x, y }) => {
                if request.mode != CaptureMode::Pixel {
                    continue;
                }
                let Some(color) = frame.rgb_at(x, y) else {
                    continue;
                };
                env.emit(CaptureStatus::Saving);
                env.selection_accepted(true);
                *editor_closed = true;
                let image = frame.to_rgb_image();
                env.write_screenshot(
                    &screenshots.image_path(request.profile_id, pixel_id),
                    &image,
                )?;
                let mut thumbnail = screenshots::thumbnail(&image);
                screenshots::draw_marker(&mut thumbnail, frame.width, x, y);
                screenshots::save_png(
                    &screenshots.thumbnail_path(request.profile_id, pixel_id),
                    &thumbnail,
                )?;
                check_picker_cancelled(commands)?;
                let pixel = Pixel::new(pixel_id, x, y, color);
                profiles::add_pixel(store, request.profile_id, request.tracker_id, pixel)?;
                return Ok(());
            }
            Ok(CaptureCommand::Select {
                pixel_id: selected_id,
                rect,
                stage,
                reply,
            }) => {
                let validation = validate_selection(
                    request.mode,
                    pixel_id,
                    selected_id,
                    rect,
                    stage,
                    search_rect,
                    frame.width,
                    frame.height,
                );
                if let Err(error) = validation {
                    let _ = reply.send(Err(error));
                    continue;
                }
                if stage == SelectionStage::Search {
                    search_rect = Some(rect);
                    env.selection_accepted(false);
                    env.emit(CaptureStatus::SelectingTemplate);
                    let _ = reply.send(Ok(SelectionReply {
                        stage: "template",
                        search_rect,
                    }));
                    continue;
                }
                let search = search_rect.expect("validated template stage");
                // Validate just the original BGRA region before allocating a full RGB image.
                // Alpha is ignored: Desktop Duplication may return zero or opaque alpha.
                if !template_has_color(frame, rect) {
                    let _ = reply.send(Err(AppError::Validation("Полностью чёрный образец не позволяет вычислить сходство. Выберите другой образец".into())));
                    continue;
                }
                env.emit(CaptureStatus::Saving);
                env.selection_accepted(true);
                *editor_closed = true;
                let image = frame.to_rgb_image();
                let template =
                    image::imageops::crop_imm(&image, rect.x, rect.y, rect.width, rect.height)
                        .to_image();
                let saved = (|| -> Result<(), Stop> {
                    profiles::profile_process_name(store, request.profile_id, request.tracker_id)?;
                    env.write_screenshot(
                        &screenshots.image_path(request.profile_id, pixel_id),
                        &image,
                    )?;
                    screenshots::save_png(
                        &screenshots.template_path(request.profile_id, pixel_id),
                        &template,
                    )?;
                    let mut thumbnail = screenshots::thumbnail(&image);
                    screenshots::draw_regions(&mut thumbnail, frame.width, search, rect);
                    screenshots::save_png(
                        &screenshots.thumbnail_path(request.profile_id, pixel_id),
                        &thumbnail,
                    )?;
                    check_picker_cancelled(commands)?;
                    profiles::add_icon(
                        store,
                        request.profile_id,
                        request.tracker_id,
                        Icon::new(pixel_id, search, rect),
                    )?;
                    Ok(())
                })();
                match saved {
                    Ok(()) => {
                        let _ = reply.send(Ok(SelectionReply {
                            stage: "done",
                            search_rect: Some(search),
                        }));
                        return Ok(());
                    }
                    Err(stop) => {
                        let message = match &stop {
                            Stop::Cancelled => "Выбор отменён".into(),
                            Stop::Failed(error) => error.to_string(),
                        };
                        let _ = reply.send(Err(AppError::Capture(message)));
                        return Err(stop);
                    }
                }
            }
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed) | Err(_) => {
                return Err(Stop::Cancelled)
            }
        }
    }
}

fn template_has_color(frame: &Frame, rect: PhysicalRect) -> bool {
    (rect.y..rect.y + rect.height).any(|y| {
        let start = (y as usize * frame.width as usize + rect.x as usize) * 4;
        let end = start + rect.width as usize * 4;
        frame.bgra[start..end]
            .as_chunks::<4>()
            .0
            .iter()
            .any(|pixel| pixel[0] | pixel[1] | pixel[2] != 0)
    })
}

/// Saving may take long enough for Esc to arrive after the template selection.
/// Drain that cancellation before the config commit makes the condition visible.
fn check_picker_cancelled(commands: &Receiver<CaptureCommand>) -> Result<(), Stop> {
    loop {
        match commands.try_recv() {
            Ok(CaptureCommand::Cancel | CaptureCommand::OverlayClosed)
            | Err(TryRecvError::Disconnected) => return Err(Stop::Cancelled),
            Ok(CaptureCommand::Select { reply, .. }) => {
                let _ = reply.send(Err(AppError::Validation("Образец уже сохраняется".into())));
            }
            Ok(
                CaptureCommand::PrepareFailed(_)
                | CaptureCommand::PrepareReady
                | CaptureCommand::FrameReady
                | CaptureCommand::FrameFailed
                | CaptureCommand::PrintScreen
                | CaptureCommand::Pick { .. },
            ) => {}
            Err(TryRecvError::Empty) => return Ok(()),
        }
    }
}

fn validate_selection(
    mode: CaptureMode,
    id: Uuid,
    selected_id: Uuid,
    rect: PhysicalRect,
    stage: SelectionStage,
    search: Option<PhysicalRect>,
    width: u32,
    height: u32,
) -> AppResult<()> {
    if mode != CaptureMode::Icon || id != selected_id {
        return Err(AppError::Validation("Сессия выбора иконки устарела".into()));
    }
    if (stage == SelectionStage::Search) != search.is_none() {
        return Err(AppError::Validation("Этап выбора уже изменился".into()));
    }
    if !rect.fits(width, height) {
        return Err(AppError::Validation(
            "Выделите непустую область внутри снимка".into(),
        ));
    }
    if search.is_some_and(|search| !search.contains(rect)) {
        return Err(AppError::Validation(
            "Образец должен находиться внутри области поиска".into(),
        ));
    }
    Ok(())
}

/// Runs at most one capture session at a time and routes commands to it.
#[derive(Default)]
pub struct PixelCapture {
    active: Arc<Mutex<Option<(Uuid, Sender<CaptureCommand>)>>>,
}

impl PixelCapture {
    pub fn start(
        &self,
        session_id: Uuid,
        session: impl FnOnce(Sender<CaptureCommand>, Receiver<CaptureCommand>) + Send + 'static,
    ) -> AppResult<JoinHandle<()>> {
        let mut active = lock(&self.active);
        if active.is_some() {
            return Err(AppError::Validation(
                "Захват пикселя или иконки уже идёт".into(),
            ));
        }
        let (sender, receiver) = mpsc::channel();
        *active = Some((session_id, sender.clone()));

        let release = ReleaseOnDrop(self.active.clone());
        Ok(thread::spawn(move || {
            let _release = release;
            session(sender, receiver);
        }))
    }

    pub fn send_for(&self, session_id: Uuid, command: CaptureCommand) -> AppResult<()> {
        match &*lock(&self.active) {
            Some((id, sender)) if *id == session_id => sender
                .send(command)
                .map_err(|_| AppError::NotFound("Захват не идёт")),
            _ => Err(AppError::Validation("Сессия захвата устарела".into())),
        }
    }

    pub fn send_to(&self, session_id: Uuid, command: CaptureCommand) {
        let _ = self.send_for(session_id, command);
    }

    pub fn cancel(&self, session_id: Uuid) {
        self.send_to(session_id, CaptureCommand::Cancel);
    }

    pub fn shutdown(&self) {
        let _ = self.send(CaptureCommand::Cancel);
        // Hook and pause are owned by the worker. Keep the app alive until cleanup completes.
        while lock(&self.active).is_some() {
            thread::sleep(Duration::from_millis(5));
        }
    }

    pub fn send(&self, command: CaptureCommand) -> AppResult<()> {
        match &*lock(&self.active) {
            Some((_, sender)) => {
                let _ = sender.send(command);
                Ok(())
            }
            None => Err(AppError::NotFound("Захват не идёт")),
        }
    }
}

/// Frees the session slot even if the session panics.
struct ReleaseOnDrop(Arc<Mutex<Option<(Uuid, Sender<CaptureCommand>)>>>);

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
        press_at: Option<Duration>,
        /// Sends `Cancel` once virtual time reaches this point.
        cancel_at: Mutex<Option<(Duration, Sender<CaptureCommand>)>>,
        /// What the user does once the picker is shown.
        on_picker: Mutex<Option<(Vec<CaptureCommand>, Sender<CaptureCommand>)>>,
        /// Becomes ready once virtual time reaches this point. `None` means already ready.
        prepare_ready_at: Option<Duration>,
        prepare_error: Option<String>,
        prepare_hangs: bool,
        fails_hook: bool,
        hook_active: Mutex<bool>,
        fails_capture: bool,
        fails_frame: bool,
        fails_save: bool,
        black_frame: bool,
        published_id: Mutex<Option<Uuid>>,
        before_picker: Mutex<Option<Box<dyn FnOnce() + Send>>>,
        before_save: Mutex<Option<Box<dyn FnOnce() + Send>>>,
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

        fn wait_notification(
            &self,
            commands: &Receiver<CaptureCommand>,
            duration: Duration,
        ) -> Option<CaptureCommand> {
            if let Ok(command) = commands.try_recv() {
                return Some(command);
            }
            let next_prepare = self
                .prepare_ready_at
                .filter(|at| *at > self.now())
                .map(|at| at - self.now());
            let next_cancel = lock(&self.cancel_at)
                .as_ref()
                .map(|(at, _)| at.saturating_sub(self.now()));
            let duration = next_prepare
                .into_iter()
                .chain(next_cancel)
                .fold(duration, Duration::min);
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
            commands.try_recv().ok()
        }

        fn begin_wait(&self) -> AppResult<()> {
            if self.fails_hook {
                return Err(AppError::Capture("Hook unavailable".into()));
            }
            *lock(&self.hook_active) = true;
            Ok(())
        }
        fn end_wait(&self) {
            *lock(&self.hook_active) = false;
        }

        fn capture_frame(&self) -> AppResult<Frame> {
            lock(&self.calls).push(format!("capture at {}ms", self.now().as_millis()));
            if self.fails_capture {
                return Err(AppError::Capture("Не удалось захватить экран".into()));
            }
            Ok(Frame {
                width: 4,
                height: 2,
                bgra: if self.black_frame {
                    vec![0; 32]
                } else {
                    [[1, 2, 3, 255], [30, 30, 200, 255]].repeat(4).concat()
                },
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
            if let Some(callback) = lock(&self.before_save).take() {
                callback();
            }
            if self.fails_save {
                return Err(AppError::Capture("Не удалось сохранить снимок".into()));
            }
            crate::screenshots::save_stored_png(path, image)
        }

        fn publish_frame(&self, _: CaptureRequest, id: Uuid, _: &Frame) -> AppResult<()> {
            *lock(&self.published_id) = Some(id);
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

        fn reveal_picker(&self) -> AppResult<()> {
            let mut calls = lock(&self.calls);
            calls.push("shutter".into());
            calls.push("show picker".into());
            drop(calls);
            if let Some(callback) = lock(&self.before_picker).take() {
                callback();
            }
            if let Some((commands, sender)) = lock(&self.on_picker).take() {
                for mut command in commands {
                    if let CaptureCommand::Select { pixel_id, .. } = &mut command {
                        if pixel_id.is_nil() {
                            *pixel_id = lock(&self.published_id).unwrap();
                        }
                    }
                    sender.send(command).unwrap();
                }
            }
            Ok(())
        }

        fn close_picker(&self) {
            lock(&self.calls).push("close picker".into());
        }

        fn selection_accepted(&self, final_selection: bool) {
            if final_selection {
                self.close_picker();
            }
            lock(&self.calls).push("selection sound".into());
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
                    session_id: Uuid::new_v4(),
                    profile_id: profile.id,
                    tracker_id: tracker.id,
                    mode: CaptureMode::Pixel,
                },
            }
        }

        fn run(
            &self,
            env: &FakeEnv,
            sender: Sender<CaptureCommand>,
            receiver: Receiver<CaptureCommand>,
        ) {
            if let Some((at, _)) = lock(&env.cancel_at).take() {
                *lock(&env.now) = at;
                sender.send(CaptureCommand::Cancel).unwrap();
            } else {
                *lock(&env.now) = env.press_at.unwrap_or_default();
                sender.send(CaptureCommand::PrintScreen).unwrap();
            }
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
    fn invalid_or_black_icon_choices_stay_open_and_only_accepted_search_plays_tick() {
        let mut fixture = Fixture::new();
        fixture.request.mode = CaptureMode::Icon;
        let (sender, commands) = mpsc::channel();
        let env = FakeEnv {
            black_frame: true,
            on_picker: picker_actions(
                &[
                    selection(rect(0, 0, 1, 1), SelectionStage::Template),
                    selection(rect(0, 0, 0, 1), SelectionStage::Search),
                    CaptureCommand::Select {
                        pixel_id: Uuid::new_v4(),
                        rect: rect(0, 0, 2, 2),
                        stage: SelectionStage::Search,
                        reply: mpsc::channel().0,
                    },
                    selection(rect(0, 0, 2, 2), SelectionStage::Search),
                    selection(rect(2, 0, 1, 1), SelectionStage::Template),
                    selection(rect(0, 0, 1, 1), SelectionStage::Template),
                    CaptureCommand::Cancel,
                ],
                &sender,
            ),
            ..FakeEnv::default()
        };
        fixture.run(&env, sender, commands);
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(env.statuses().contains(&CaptureStatus::SelectingTemplate));
        assert!(!env.statuses().contains(&CaptureStatus::Saving));
        let calls = env.calls();
        assert_eq!(
            calls
                .iter()
                .filter(|call| *call == "selection sound")
                .count(),
            1
        );
        assert_eq!(
            calls.iter().filter(|call| *call == "close picker").count(),
            1
        );
        assert!(!calls.iter().any(|call| call == "write png"));
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn template_black_check_uses_selected_rgb_region_and_ignores_alpha() {
        let frame = Frame {
            width: 3,
            height: 1,
            bgra: vec![0, 0, 0, 255, 1, 0, 0, 0, 0, 0, 0, 0],
        };
        assert!(!template_has_color(&frame, rect(0, 0, 1, 1)));
        assert!(!template_has_color(&frame, rect(2, 0, 1, 1)));
        assert!(template_has_color(&frame, rect(1, 0, 1, 1)));
    }

    #[test]
    fn cancelling_accepted_pixel_during_background_save_cleans_files_without_reclosing() {
        let fixture = Fixture::new();
        let (sender, commands) = mpsc::channel();
        let env = FakeEnv {
            on_picker: picker_actions(
                &[CaptureCommand::Pick { x: 1, y: 0 }, CaptureCommand::Cancel],
                &sender,
            ),
            ..FakeEnv::default()
        };
        fixture.run(&env, sender, commands);
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(env.statuses().contains(&CaptureStatus::Saving));
        assert_eq!(
            env.calls()
                .iter()
                .filter(|call| *call == "selection sound")
                .count(),
            1
        );
        assert_eq!(
            env.calls()
                .iter()
                .filter(|call| *call == "close picker")
                .count(),
            1
        );
        assert!(fixture.pixels().is_empty());
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn final_choice_closes_and_acknowledges_before_blocked_background_save() {
        for mode in [CaptureMode::Pixel, CaptureMode::Icon] {
            for fail_save in [false, true] {
                let mut fixture = Fixture::new();
                fixture.request.mode = mode;
                let request = fixture.request;
                let store = fixture.store.clone();
                let screenshots = fixture.screenshots.clone();
                let capture = PixelCapture::default();
                let engine = Arc::new(crate::engine::Engine::start(|_| {}));
                let worker_engine = engine.clone();
                let (sender, commands) = mpsc::channel();
                sender.send(CaptureCommand::PrintScreen).unwrap();
                let actions = match mode {
                    CaptureMode::Pixel => vec![CaptureCommand::Pick { x: 1, y: 0 }],
                    CaptureMode::Icon => vec![
                        selection(rect(0, 0, 4, 2), SelectionStage::Search),
                        selection(rect(1, 0, 1, 1), SelectionStage::Template),
                    ],
                };
                let (saving, save_started) = mpsc::channel();
                let (release, save_release) = mpsc::channel();
                let env = Arc::new(FakeEnv {
                    fails_save: fail_save,
                    before_save: Mutex::new(Some(Box::new(move || {
                        saving.send(()).unwrap();
                        let _ = save_release.recv();
                    }))),
                    on_picker: picker_actions(&actions, &sender),
                    ..FakeEnv::default()
                });
                let worker_env = env.clone();
                let worker = capture
                    .start(request.session_id, move |_, _| {
                        let _guard = worker_engine.pause();
                        run_session(&*worker_env, &store, &screenshots, request, &commands);
                    })
                    .unwrap();
                save_started
                    .recv_timeout(Duration::from_secs(2))
                    .expect("accepted final selection reaches background persistence");
                let calls = env.calls();
                let close = calls
                    .iter()
                    .position(|call| call == "close picker")
                    .expect("editor closes before saving");
                let sound = calls
                    .iter()
                    .rposition(|call| call == "selection sound")
                    .expect("accepted choice is acknowledged");
                let save = calls.iter().position(|call| call == "write png").unwrap();
                assert!(close < sound && sound < save);
                assert_eq!(env.last_status(), CaptureStatus::Saving);
                assert!(engine.is_paused());
                assert!(capture.start(Uuid::new_v4(), |_, _| {}).is_err());
                assert!(fixture.pixels().is_empty());
                assert!(fixture
                    .store
                    .read(|config| config.profiles[0].trackers[0].icons.is_empty()));
                release.send(()).unwrap();
                worker.join().unwrap();
                assert!(!engine.is_paused());
                assert_eq!(
                    env.calls()
                        .iter()
                        .filter(|call| *call == "close picker")
                        .count(),
                    1
                );
                if fail_save {
                    assert!(matches!(env.last_status(), CaptureStatus::Failed { .. }));
                    assert_eq!(fixture.screenshot_files(), 0);
                } else {
                    assert_eq!(env.last_status(), CaptureStatus::Done);
                    assert_eq!(
                        fixture.screenshot_files(),
                        if mode == CaptureMode::Pixel { 2 } else { 3 }
                    );
                }
            }
        }
    }

    #[test]
    fn delayed_pick_from_finished_session_cannot_reach_new_session() {
        let capture = PixelCapture::default();
        let old = Uuid::new_v4();
        capture.start(old, |_, _| {}).unwrap().join().unwrap();
        let current = Uuid::new_v4();
        let worker = capture
            .start(current, |_, commands| {
                assert!(matches!(commands.recv(), Ok(CaptureCommand::Cancel)));
            })
            .unwrap();
        assert!(capture
            .send_for(old, CaptureCommand::Pick { x: 1, y: 1 })
            .is_err());
        capture.cancel(old);
        capture.cancel(current);
        worker.join().unwrap();
    }

    #[test]
    fn successful_choice_is_the_first_persistent_full_frame_write() {
        for mode in [CaptureMode::Pixel, CaptureMode::Icon] {
            let mut fixture = Fixture::new();
            fixture.request.mode = mode;
            let screenshots = fixture.screenshots.clone();
            let request = fixture.request;
            let (sender, commands) = mpsc::channel();
            let actions = match mode {
                CaptureMode::Pixel => vec![CaptureCommand::Pick { x: 1, y: 0 }],
                CaptureMode::Icon => vec![
                    selection(rect(0, 0, 4, 2), SelectionStage::Search),
                    selection(rect(1, 0, 1, 1), SelectionStage::Template),
                ],
            };
            let env = FakeEnv {
                before_picker: Mutex::new(Some(Box::new(move || {
                    assert!(
                        !screenshots.profile_dir(request.profile_id).exists(),
                        "editor reveal must not depend on disk screenshot IO"
                    );
                }))),
                on_picker: picker_actions(&actions, &sender),
                ..FakeEnv::default()
            };
            fixture.run(&env, sender, commands);
            assert_eq!(env.last_status(), CaptureStatus::Done);
            let calls = env.calls();
            assert!(
                calls.iter().position(|c| c == "write png").unwrap()
                    > calls.iter().position(|c| c == "show picker").unwrap()
            );
        }
    }

    #[test]
    fn full_frame_persistence_failure_does_not_commit_pixel_or_icon() {
        for mode in [CaptureMode::Pixel, CaptureMode::Icon] {
            let mut fixture = Fixture::new();
            fixture.request.mode = mode;
            let (sender, commands) = mpsc::channel();
            let actions = match mode {
                CaptureMode::Pixel => vec![CaptureCommand::Pick { x: 0, y: 0 }],
                CaptureMode::Icon => vec![
                    selection(rect(0, 0, 4, 2), SelectionStage::Search),
                    selection(rect(1, 0, 1, 1), SelectionStage::Template),
                ],
            };
            let env = FakeEnv {
                fails_save: true,
                on_picker: picker_actions(&actions, &sender),
                ..FakeEnv::default()
            };
            fixture.run(&env, sender, commands);
            assert!(matches!(env.last_status(), CaptureStatus::Failed { .. }));
            assert_eq!(fixture.screenshot_files(), 0);
            assert!(fixture.pixels().is_empty());
            assert!(fixture
                .store
                .read(|config| config.profiles[0].trackers[0].icons.is_empty()));
        }
    }

    #[test]
    fn shutdown_keeps_slot_reserved_until_worker_cleanup_finishes() {
        let capture = Arc::new(PixelCapture::default());
        let (cleaning, cleanup_started) = mpsc::channel();
        let (release, cleanup_release) = mpsc::channel();
        let worker = capture
            .start(Uuid::new_v4(), move |_, commands| {
                assert!(matches!(commands.recv(), Ok(CaptureCommand::Cancel)));
                cleaning.send(()).unwrap();
                cleanup_release.recv().unwrap();
            })
            .unwrap();
        let shutdown_capture = capture.clone();
        let (finished, shutdown_finished) = mpsc::channel();
        let shutdown = thread::spawn(move || {
            shutdown_capture.shutdown();
            finished.send(()).unwrap();
        });
        cleanup_started.recv().unwrap();
        assert!(shutdown_finished.try_recv().is_err());
        assert!(capture.start(Uuid::new_v4(), |_, _| {}).is_err());
        release.send(()).unwrap();
        shutdown.join().unwrap();
        worker.join().unwrap();
        shutdown_finished.recv().unwrap();
        capture
            .start(Uuid::new_v4(), |_, _| {})
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn every_terminal_outcome_releases_hook_pause_and_session_slot() {
        for case in 0..12 {
            let outcome = case % 6;
            let user_paused = case >= 6;
            let engine = Arc::new(crate::engine::Engine::start(|_| {}));
            if user_paused {
                assert!(engine.toggle_user_pause());
            }
            let worker_engine = engine.clone();
            let fixture = Fixture::new();
            let capture = PixelCapture::default();
            let (sender, receiver) = mpsc::channel();
            sender
                .send(if outcome == 0 {
                    CaptureCommand::Cancel
                } else {
                    CaptureCommand::PrintScreen
                })
                .unwrap();
            let env = Arc::new(FakeEnv {
                fails_hook: outcome == 1,
                fails_capture: outcome == 2,
                prepare_error: (outcome == 3).then(|| PREPARE_FAILED.into()),
                fails_frame: outcome == 4,
                on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
                ..FakeEnv::default()
            });
            let worker_env = env.clone();
            let store = fixture.store.clone();
            let screenshots = fixture.screenshots.clone();
            let request = fixture.request;
            let worker = capture
                .start(request.session_id, move |_, _| {
                    let _guard = worker_engine.pause();
                    assert!(worker_engine.is_paused());
                    run_session(&*worker_env, &store, &screenshots, request, &receiver);
                    assert!(!*lock(&worker_env.hook_active));
                    assert!(
                        worker_engine.is_paused(),
                        "pause lasts through terminal cleanup"
                    );
                })
                .unwrap();
            worker.join().unwrap();
            assert_eq!(engine.is_paused(), user_paused);
            assert!(
                fixture
                    .store
                    .read(|config| config.profiles[0].trackers.iter().all(|t| !t.is_running)),
                "capture must preserve stopped trackers"
            );
            assert!(!*lock(&env.hook_active));
            capture
                .start(Uuid::new_v4(), |_, _| {})
                .unwrap()
                .join()
                .unwrap();
            if outcome != 5 {
                assert_eq!(fixture.screenshot_files(), 0);
            }
        }
    }

    #[test]
    fn preparation_failure_ends_waiting_without_a_printscreen() {
        let fixture = Fixture::new();
        let (sender, commands) = mpsc::channel();
        let store = fixture.store.clone();
        let screenshots = fixture.screenshots.clone();
        let request = fixture.request;
        let env = Arc::new(FakeEnv::default());
        let worker_env = env.clone();
        let worker = thread::spawn(move || {
            run_session(&*worker_env, &store, &screenshots, request, &commands)
        });
        while !env
            .statuses()
            .contains(&CaptureStatus::WaitingForPrintScreen)
        {
            thread::yield_now();
        }
        sender
            .send(CaptureCommand::PrepareFailed(PREPARE_FAILED.into()))
            .unwrap();
        worker.join().unwrap();
        assert!(matches!(env.last_status(), CaptureStatus::Failed { .. }));
        assert!(!env.calls().iter().any(|c| c.starts_with("capture at")));
        assert!(!*lock(&env.hook_active));
    }

    #[test]
    fn waits_indefinitely_until_printscreen_without_capturing() {
        let fixture = Fixture::new();
        let capture = PixelCapture::default();
        let env = Arc::new(FakeEnv::default());
        let (ready, waiting) = mpsc::channel();
        let request = fixture.request;
        let store = fixture.store.clone();
        let screenshots = fixture.screenshots.clone();
        let worker_env = env.clone();
        let worker = capture
            .start(request.session_id, move |_, commands| {
                ready.send(()).unwrap();
                run_session(&*worker_env, &store, &screenshots, request, &commands);
            })
            .unwrap();
        waiting.recv().unwrap();
        while !env
            .statuses()
            .contains(&CaptureStatus::WaitingForPrintScreen)
        {
            thread::yield_now();
        }
        *lock(&env.now) = Duration::from_secs(60);
        assert!(!env.calls().iter().any(|c| c.starts_with("capture at")));
        capture.cancel(Uuid::new_v4());
        assert!(capture.start(Uuid::new_v4(), |_, _| {}).is_err());
        capture.cancel(request.session_id);
        worker.join().unwrap();
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn printscreen_outside_game_captures_immediately_and_adds_the_picked_pixel() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::from_millis(2_500)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 1, y: 1 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(
            env.calls(),
            [
                "prepare",
                "capture at 2500ms",
                "publish frame",
                "shutter",
                "show picker",
                "close picker",
                "selection sound",
                "write png",
                "hide picker",
            ]
        );
        let statuses = env.statuses();
        assert!(statuses.contains(&CaptureStatus::Capturing));
        assert_eq!(
            statuses[statuses.len() - 2..],
            [CaptureStatus::Saving, CaptureStatus::Done]
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
            press_at: Some(Duration::ZERO),
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
        assert_eq!(
            env.calls()
                .iter()
                .filter(|call| *call == "selection sound")
                .count(),
            1
        );
        assert_eq!(
            env.calls()
                .iter()
                .filter(|call| *call == "close picker")
                .count(),
            1
        );
    }

    #[test]
    fn user_cancel_before_printscreen_takes_no_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::from_secs(1)),
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
            press_at: Some(Duration::ZERO),
            on_picker: picker_actions(&[CaptureCommand::Cancel], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        let calls = env.calls();
        let shutter = calls.iter().position(|call| call == "shutter").unwrap();
        assert_eq!(calls[shutter + 1], "show picker");
        assert!(!calls.iter().any(|call| call == "write png"));
        assert!(fixture.pixels().is_empty());
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn closing_the_hidden_overlay_cancels_without_capturing() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };
        sender.send(CaptureCommand::OverlayClosed).unwrap();

        fixture.run(&env, sender, receiver);

        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(!env
            .calls()
            .iter()
            .any(|call| call.starts_with("capture at")));
    }

    #[test]
    fn reports_a_failed_screen_capture() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
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
            press_at: Some(Duration::ZERO),
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
            press_at: Some(Duration::ZERO),
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
    fn snapshot_precedes_cold_prepare_completion() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
            prepare_ready_at: Some(Duration::from_secs(5)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert!(env.calls().contains(&"capture at 0ms".to_string()));
        let calls = env.calls();
        let shutter = calls.iter().position(|call| call == "shutter").unwrap();
        assert_eq!(calls[shutter + 1], "show picker");
        assert!(calls.iter().position(|call| call == "write png").unwrap() > shutter);
    }

    #[test]
    fn prepare_does_not_delay_the_snapshot() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
            prepare_ready_at: Some(Duration::from_millis(100)),
            on_picker: picker_actions(&[CaptureCommand::Pick { x: 0, y: 0 }], &sender),
            ..FakeEnv::default()
        };

        fixture.run(&env, sender, receiver);

        assert!(env.calls().contains(&"capture at 0ms".to_string()));
        assert_eq!(env.last_status(), CaptureStatus::Done);
    }

    #[test]
    fn frame_show_failure_removes_the_file_and_skips_the_shutter() {
        let fixture = Fixture::new();
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
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
        assert!(!calls.iter().any(|call| call == "write png"));
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
    fn pixel_and_icon_modes_share_one_session_slot() {
        let mut fixture = Fixture::new();
        fixture.request.mode = CaptureMode::Icon;
        let capture = PixelCapture::default();
        let env = Arc::new(FakeEnv {
            press_at: Some(Duration::ZERO),
            ..FakeEnv::default()
        });

        let session = {
            let (env, store, screenshots, request) = (
                env.clone(),
                fixture.store.clone(),
                fixture.screenshots.clone(),
                fixture.request,
            );
            move |_: Sender<CaptureCommand>, commands: Receiver<CaptureCommand>| {
                run_session(&*env, &store, &screenshots, request, &commands)
            }
        };
        let first = capture.start(fixture.request.session_id, session).unwrap();
        while !env
            .statuses()
            .contains(&CaptureStatus::WaitingForPrintScreen)
        {
            thread::sleep(Duration::from_millis(5));
        }

        assert!(matches!(
            capture.start(Uuid::new_v4(), |_, _| {}),
            Err(AppError::Validation(_))
        ));

        capture.send(CaptureCommand::Cancel).unwrap();
        first.join().unwrap();
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(capture.send(CaptureCommand::Cancel).is_err());

        let second = capture.start(Uuid::new_v4(), |_, _| {}).unwrap();
        second.join().unwrap();
    }

    #[test]
    fn serializes_status_for_the_frontend() {
        let json = |status| serde_json::to_value(status).unwrap();

        assert_eq!(
            json(CaptureStatus::WaitingForPrintScreen),
            serde_json::json!({ "phase": "waitingForPrintScreen" })
        );
        assert_eq!(
            json(CaptureStatus::Failed { reason: "x".into() }),
            serde_json::json!({ "phase": "failed", "reason": "x" })
        );
    }

    fn rect(x: u32, y: u32, width: u32, height: u32) -> PhysicalRect {
        PhysicalRect {
            x,
            y,
            width,
            height,
        }
    }

    fn selection(rect: PhysicalRect, stage: SelectionStage) -> CaptureCommand {
        CaptureCommand::Select {
            pixel_id: Uuid::nil(),
            rect,
            stage,
            reply: mpsc::channel().0,
        }
    }

    #[test]
    fn icon_uses_one_frame_and_commits_original_crop_with_three_files() {
        let mut fixture = Fixture::new();
        fixture.request.mode = CaptureMode::Icon;
        let (sender, receiver) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
            on_picker: picker_actions(
                &[
                    selection(rect(0, 0, 4, 2), SelectionStage::Search),
                    CaptureCommand::PrintScreen,
                    selection(rect(1, 0, 1, 2), SelectionStage::Template),
                ],
                &sender,
            ),
            ..FakeEnv::default()
        };
        fixture.run(&env, sender, receiver);
        assert_eq!(env.last_status(), CaptureStatus::Done);
        assert!(env.statuses().contains(&CaptureStatus::SelectingTemplate));
        assert_eq!(
            env.calls()
                .iter()
                .filter(|c| c.starts_with("capture at"))
                .count(),
            1
        );
        assert!(env.calls().contains(&"capture at 0ms".into()));
        let icons = fixture
            .store
            .read(|c| c.profiles[0].trackers[0].icons.clone());
        assert_eq!(icons.len(), 1);
        assert!(fixture.pixels().is_empty());
        assert_eq!(fixture.screenshot_files(), 3);
        let template = image::open(
            fixture
                .screenshots
                .template_path(fixture.request.profile_id, icons[0].id),
        )
        .unwrap()
        .to_rgb8();
        assert_eq!(template.dimensions(), (1, 2));
        assert_eq!(template.get_pixel(0, 0).0, [200, 30, 30]);
        std::fs::remove_file(
            fixture
                .screenshots
                .image_path(fixture.request.profile_id, icons[0].id),
        )
        .unwrap();
        let mut cache = crate::icon_search::TemplateCache::default();
        let result = cache.check(
            icons[0].id,
            &fixture
                .screenshots
                .template_path(fixture.request.profile_id, icons[0].id),
            &template,
        );
        assert!(
            result.passes(&icons[0]),
            "missing viewer image must not disable matching"
        );
    }

    #[test]
    fn cancelling_both_icon_stages_or_rejecting_black_template_leaves_no_files() {
        for (select_search, black) in [(false, false), (true, false), (true, true)] {
            let mut fixture = Fixture::new();
            fixture.request.mode = CaptureMode::Icon;
            let (sender, receiver) = mpsc::channel();
            let mut commands = vec![];
            if select_search {
                commands.push(selection(rect(0, 0, 4, 2), SelectionStage::Search));
            }
            if black {
                commands.push(selection(rect(0, 0, 1, 1), SelectionStage::Template));
            }
            commands.push(CaptureCommand::Cancel);
            let env = FakeEnv {
                press_at: Some(Duration::ZERO),
                black_frame: black,
                on_picker: picker_actions(&commands, &sender),
                ..FakeEnv::default()
            };
            fixture.run(&env, sender, receiver);
            assert_eq!(env.last_status(), CaptureStatus::Cancelled);
            assert_eq!(fixture.screenshot_files(), 0);
            assert!(fixture
                .store
                .read(|c| c.profiles[0].trackers[0].icons.is_empty()));
        }
    }

    #[test]
    fn selection_rejects_empty_outside_stale_wrong_mode_and_wrong_stage() {
        let id = Uuid::new_v4();
        let search = rect(0, 0, 3, 2);
        for (mode, selected, area, stage, current) in [
            (
                CaptureMode::Icon,
                id,
                rect(0, 0, 0, 1),
                SelectionStage::Search,
                None,
            ),
            (
                CaptureMode::Icon,
                id,
                rect(3, 0, 2, 1),
                SelectionStage::Search,
                None,
            ),
            (
                CaptureMode::Icon,
                Uuid::new_v4(),
                search,
                SelectionStage::Search,
                None,
            ),
            (CaptureMode::Pixel, id, search, SelectionStage::Search, None),
            (
                CaptureMode::Icon,
                id,
                search,
                SelectionStage::Template,
                None,
            ),
            (
                CaptureMode::Icon,
                id,
                rect(2, 0, 2, 1),
                SelectionStage::Template,
                Some(search),
            ),
        ] {
            assert!(validate_selection(mode, id, selected, area, stage, current, 4, 2).is_err());
        }
        assert!(validate_selection(
            CaptureMode::Icon,
            id,
            id,
            search,
            SelectionStage::Template,
            Some(search),
            4,
            2
        )
        .is_ok());
    }

    #[test]
    fn removed_tracker_before_icon_commit_leaves_no_partial_files() {
        let fixture = Fixture::new();
        profiles::delete_tracker(
            &fixture.store,
            &fixture.screenshots,
            fixture.request.profile_id,
            fixture.request.tracker_id,
        )
        .unwrap();
        let (sender, receiver) = mpsc::channel();
        let (reply, replies) = mpsc::channel();
        let id = Uuid::new_v4();
        let request = CaptureRequest {
            mode: CaptureMode::Icon,
            ..fixture.request
        };
        let frame = Frame {
            width: 4,
            height: 2,
            bgra: vec![1; 32],
        };
        let image = frame.to_rgb_image();
        screenshots::save_png(
            &fixture.screenshots.image_path(request.profile_id, id),
            &image,
        )
        .unwrap();
        sender
            .send(CaptureCommand::Select {
                pixel_id: id,
                rect: rect(0, 0, 4, 2),
                stage: SelectionStage::Search,
                reply: reply.clone(),
            })
            .unwrap();
        sender
            .send(CaptureCommand::Select {
                pixel_id: id,
                rect: rect(1, 0, 1, 1),
                stage: SelectionStage::Template,
                reply,
            })
            .unwrap();
        assert!(pick(
            &fixture.store,
            &fixture.screenshots,
            request,
            id,
            &frame,
            &receiver,
            &FakeEnv::default(),
            &mut false
        )
        .is_err());
        assert!(replies.recv().unwrap().is_ok());
        assert!(replies.recv().unwrap().is_err());
        fixture.screenshots.remove_icon(request.profile_id, id);
        assert_eq!(fixture.screenshot_files(), 0);
    }

    #[test]
    fn commit_errors_remove_every_file_and_never_write_a_partial_icon() {
        for remove_tracker in [true, false] {
            let mut fixture = Fixture::new();
            fixture.request.mode = CaptureMode::Icon;
            let request = fixture.request;
            let store = fixture.store.clone();
            let screenshots = fixture.screenshots.clone();
            let config_path = fixture._dir.path().join("config.json");
            let callback = move || {
                if remove_tracker {
                    profiles::delete_tracker(
                        &store,
                        &screenshots,
                        request.profile_id,
                        request.tracker_id,
                    )
                    .unwrap();
                } else {
                    std::fs::remove_file(&config_path).unwrap();
                    std::fs::create_dir(&config_path).unwrap();
                }
            };
            let (sender, receiver) = mpsc::channel();
            let env = FakeEnv {
                press_at: Some(Duration::ZERO),
                before_picker: Mutex::new(Some(Box::new(callback))),
                on_picker: picker_actions(
                    &[
                        selection(rect(0, 0, 4, 2), SelectionStage::Search),
                        selection(rect(0, 0, 1, 1), SelectionStage::Template),
                    ],
                    &sender,
                ),
                ..FakeEnv::default()
            };
            fixture.run(&env, sender, receiver);
            assert!(matches!(env.last_status(), CaptureStatus::Failed { .. }));
            assert_eq!(fixture.screenshot_files(), 0);
            assert!(fixture
                .store
                .read(|c| c.profiles[0].trackers.iter().all(|t| t.icons.is_empty())));
            assert!(env.calls().contains(&"close picker".into()));
        }
    }

    #[test]
    fn escape_after_template_selection_cancels_saving_before_condition_commit() {
        let mut fixture = Fixture::new();
        fixture.request.mode = CaptureMode::Icon;
        let (sender, receiver) = mpsc::channel();
        let (reply, replies) = mpsc::channel();
        let env = FakeEnv {
            press_at: Some(Duration::ZERO),
            on_picker: picker_actions(
                &[
                    CaptureCommand::Select {
                        pixel_id: Uuid::nil(),
                        rect: rect(0, 0, 4, 2),
                        stage: SelectionStage::Search,
                        reply: reply.clone(),
                    },
                    CaptureCommand::Select {
                        pixel_id: Uuid::nil(),
                        rect: rect(0, 0, 1, 1),
                        stage: SelectionStage::Template,
                        reply,
                    },
                    CaptureCommand::Cancel,
                ],
                &sender,
            ),
            ..FakeEnv::default()
        };
        fixture.run(&env, sender, receiver);
        assert_eq!(env.last_status(), CaptureStatus::Cancelled);
        assert!(replies.recv().unwrap().is_ok());
        assert!(replies.recv().unwrap().is_err());
        assert_eq!(fixture.screenshot_files(), 0);
        assert!(fixture
            .store
            .read(|c| c.profiles[0].trackers[0].icons.is_empty()));
        assert!(ConfigStore::load(fixture._dir.path())
            .unwrap()
            .store
            .read(|c| c.profiles[0].trackers[0].icons.is_empty()));
        assert!(env.calls().contains(&"close picker".into()));
    }
}
