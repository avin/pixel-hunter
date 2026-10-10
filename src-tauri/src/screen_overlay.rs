//! Borderless top-most window over the primary monitor that shows a screenshot one to one,
//! to pick a point (`view=picker`). The window is created hidden during capture prepare
//! and reused; it is shown only after the page has drawn the current frame.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use image::RgbImage;
use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::foreground::{self, WindowHandle};
use crate::pixel_capture::{
    CaptureCommand, CaptureEnv, CaptureMode, CaptureRequest, CaptureStatus, PixelCapture,
    STATUS_EVENT,
};
use crate::screen::{Frame, PrimaryScreen};
use crate::screenshots;
use crate::shutter;

const LABEL: &str = "screen-overlay";
const MAIN_LABEL: &str = "main";
const FRAME_EVENT: &str = "screen-overlay://frame";
const CLEAR_EVENT: &str = "screen-overlay://clear";
/// Matches the 10s game wait: a page that never reports ready must not hold the session.
const PREPARE_LIMIT: Duration = Duration::from_secs(10);
const PREPARE_FAILED: &str = "Не удалось подготовить окно выбора";
const FRAME_FAILED: &str = "Не удалось показать снимок";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerFrame {
    pub profile_id: Uuid,
    pub pixel_id: Uuid,
    #[serde(default)]
    pub mode: CaptureMode,
}

#[derive(Default)]
struct PickerState {
    created: bool,
    /// Set only from the window `Destroyed` event, which is what sends `OverlayClosed`.
    destroyed: bool,
    page_ready: bool,
    visible: bool,
    frame: Option<PickerFrame>,
    drawn: bool,
    failed: bool,
}

impl PickerState {
    /// Records the hidden window. A second call keeps the same window.
    fn prepare(&mut self) -> bool {
        if self.created && !self.destroyed {
            self.visible = false;
            return false;
        }
        self.created = true;
        self.destroyed = false;
        self.visible = false;
        true
    }

    fn mark_page_ready(&mut self) {
        self.page_ready = true;
    }

    fn publish(&mut self, frame: PickerFrame) {
        self.frame = Some(frame);
        self.drawn = false;
        self.failed = false;
        self.visible = false;
    }

    fn mark_drawn(&mut self, frame: &PickerFrame) -> bool {
        if self.frame.as_ref() == Some(frame) {
            self.drawn = true;
            self.failed = false;
            true
        } else {
            false
        }
    }

    fn mark_failed(&mut self, frame: &PickerFrame) -> bool {
        if self.frame.as_ref() == Some(frame) {
            self.failed = true;
            self.drawn = false;
            true
        } else {
            false
        }
    }

    fn can_reveal(&self) -> bool {
        self.created && !self.destroyed && self.drawn && self.frame.is_some() && !self.failed
    }

    fn mark_visible(&mut self) -> bool {
        if !self.can_reveal() {
            return false;
        }
        self.visible = true;
        true
    }

    /// Hides the picker and drops the frame. The warmed window stays alive.
    fn hide(&mut self) {
        self.visible = false;
        self.frame = None;
        self.drawn = false;
        self.failed = false;
    }

    fn mark_destroyed(&mut self) {
        self.destroyed = true;
        self.created = false;
        self.visible = false;
        self.page_ready = false;
        self.frame = None;
        self.drawn = false;
        self.failed = false;
    }
}

#[derive(Default)]
pub struct PickerOverlay {
    state: Mutex<PickerState>,
}

impl PickerOverlay {
    fn lock(&self) -> MutexGuard<'_, PickerState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

enum PrepareOutcome {
    Pending,
    Ready,
    Failed(String),
}

struct PrepareSlot {
    generation: u64,
    outcome: PrepareOutcome,
}

impl Default for PrepareSlot {
    fn default() -> Self {
        Self {
            generation: 0,
            outcome: PrepareOutcome::Pending,
        }
    }
}

/// Creates the hidden picker once and waits until its page is ready.
/// The capture session calls this on a background thread, not from a synchronous command.
pub fn prepare(app: &AppHandle) -> AppResult<()> {
    shutter::prepare();
    ensure_hidden_window(app)?;
    let started = Instant::now();
    loop {
        if app.state::<PickerOverlay>().lock().page_ready {
            return Ok(());
        }
        if started.elapsed() >= PREPARE_LIMIT {
            return Err(AppError::Capture(PREPARE_FAILED.into()));
        }
        thread::sleep(Duration::from_millis(50));
    }
}

pub fn publish_frame(
    app: &AppHandle,
    profile_id: Uuid,
    pixel_id: Uuid,
    mode: CaptureMode,
) -> AppResult<()> {
    let frame = PickerFrame {
        profile_id,
        pixel_id,
        mode,
    };
    app.state::<PickerOverlay>().lock().publish(frame.clone());
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::Capture(FRAME_FAILED.into()))?;
    window.emit(FRAME_EVENT, &frame).map_err(window_error)
}

pub fn frame_result(app: &AppHandle) -> Option<AppResult<()>> {
    let overlay = app.state::<PickerOverlay>();
    let state = overlay.lock();
    if state.failed {
        Some(Err(AppError::Capture(FRAME_FAILED.into())))
    } else if state.drawn {
        Some(Ok(()))
    } else {
        None
    }
}

pub fn place(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::Capture(PREPARE_FAILED.into()))?;
    place_window(app, &window)
}

pub fn show(app: &AppHandle) -> AppResult<()> {
    let window = app
        .get_webview_window(LABEL)
        .ok_or_else(|| AppError::Capture(FRAME_FAILED.into()))?;
    if !app.state::<PickerOverlay>().lock().can_reveal() {
        return Err(AppError::Capture(FRAME_FAILED.into()));
    }
    window.show().map_err(window_error)?;
    if !app.state::<PickerOverlay>().lock().mark_visible() {
        let _ = window.hide();
        return Err(AppError::Capture(FRAME_FAILED.into()));
    }
    bring_to_front(&window);
    Ok(())
}

/// Hides the warmed window and clears the frame. Does not destroy it and does not send `OverlayClosed`.
pub fn hide(app: &AppHandle) {
    app.state::<PickerOverlay>().lock().hide();
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.emit(CLEAR_EVENT, ());
        let _ = window.hide();
    }
}

pub fn activate_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN_LABEL) {
        let _ = main.unminimize();
        let _ = main.show();
        bring_to_front(&main);
    }
}

pub fn register_cleanup(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN_LABEL) {
        let app = app.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::Destroyed = event {
                if let Some(overlay) = app.get_webview_window(LABEL) {
                    let _ = overlay.destroy();
                }
            }
        });
    }
}

fn ensure_hidden_window(app: &AppHandle) -> AppResult<()> {
    if app.get_webview_window(LABEL).is_some() {
        let _ = app.state::<PickerOverlay>().lock().prepare();
        return Ok(());
    }

    let window =
        WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html?view=picker".into()))
            .title("Pixel Hunter")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .shadow(false)
            .focused(false)
            .visible(false)
            .build()
            .map_err(window_error)?;
    let app_handle = app.clone();
    window.on_window_event(move |event| {
        // Hiding the picker must not cancel a capture; only destroying the window does.
        if let WindowEvent::Destroyed = event {
            app_handle.state::<PickerOverlay>().lock().mark_destroyed();
            let _ = app_handle
                .state::<PixelCapture>()
                .send(CaptureCommand::OverlayClosed);
        }
    });
    let _ = app.state::<PickerOverlay>().lock().prepare();
    place_window(app, &window)
}

fn place_window(app: &AppHandle, window: &WebviewWindow) -> AppResult<()> {
    let monitor = app
        .primary_monitor()
        .map_err(window_error)?
        .ok_or_else(|| AppError::Capture("Основной монитор не найден".into()))?;
    let position = monitor.position();
    let size = monitor.size();
    window
        .set_position(PhysicalPosition::new(position.x, position.y))
        .and_then(|()| window.set_size(PhysicalSize::new(size.width, size.height)))
        .map_err(window_error)
}

fn bring_to_front(window: &WebviewWindow) {
    let _ = window.set_focus();
    if let Ok(hwnd) = window.hwnd() {
        foreground::bring_to_front(WindowHandle(hwnd.0 as isize));
    }
}

fn window_error(error: tauri::Error) -> AppError {
    AppError::Capture(format!("Не удалось показать окно снимка: {error}"))
}

fn validate_window(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != LABEL {
        return Err(AppError::Validation(
            "Команда доступна только окну выбора точки".into(),
        ));
    }
    Ok(())
}

#[tauri::command]
pub async fn picker_ready(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
) -> AppResult<()> {
    validate_window(&window)?;
    overlay.lock().mark_page_ready();
    Ok(())
}

#[tauri::command]
pub async fn picker_frame(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
) -> AppResult<Option<PickerFrame>> {
    validate_window(&window)?;
    Ok(overlay.lock().frame.clone())
}

#[tauri::command]
pub async fn picker_frame_drawn(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
    profile_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut state = overlay.lock();
    let mode = state.frame.as_ref().map(|f| f.mode).unwrap_or_default();
    state.mark_drawn(&PickerFrame {
        profile_id,
        pixel_id,
        mode,
    });
    Ok(())
}

#[tauri::command]
pub async fn picker_frame_failed(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
    profile_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut state = overlay.lock();
    let mode = state.frame.as_ref().map(|f| f.mode).unwrap_or_default();
    state.mark_failed(&PickerFrame {
        profile_id,
        pixel_id,
        mode,
    });
    Ok(())
}

pub struct TauriCaptureEnv {
    app: AppHandle,
    started: Instant,
    prepare: Arc<Mutex<PrepareSlot>>,
}

impl TauriCaptureEnv {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            started: Instant::now(),
            prepare: Arc::new(Mutex::new(PrepareSlot::default())),
        }
    }
}

impl CaptureEnv for TauriCaptureEnv {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        thread::sleep(duration);
    }

    fn foreground_game(&self, process_name: &str) -> Option<WindowHandle> {
        foreground::foreground_window()
            .filter(|window| foreground::is_process(&window.exe_name, process_name))
            .map(|window| window.window)
    }

    fn begin_prepare(&self) {
        let generation = {
            let mut slot = lock_slot(&self.prepare);
            slot.generation += 1;
            slot.outcome = PrepareOutcome::Pending;
            slot.generation
        };
        let app = self.app.clone();
        let slot = Arc::clone(&self.prepare);
        thread::spawn(move || {
            let outcome = match prepare(&app) {
                Ok(()) => PrepareOutcome::Ready,
                Err(error) => PrepareOutcome::Failed(error.to_string()),
            };
            let mut current = lock_slot(&slot);
            if current.generation == generation {
                current.outcome = outcome;
            }
        });
    }

    fn prepare_result(&self) -> Option<AppResult<()>> {
        match &lock_slot(&self.prepare).outcome {
            PrepareOutcome::Pending => None,
            PrepareOutcome::Ready => Some(Ok(())),
            PrepareOutcome::Failed(reason) => Some(Err(AppError::Capture(reason.clone()))),
        }
    }

    fn capture_frame(&self) -> AppResult<Frame> {
        PrimaryScreen::new()?.capture_frame()
    }

    fn write_screenshot(&self, path: &Path, image: &RgbImage) -> AppResult<()> {
        screenshots::save_stored_png(path, image)
    }

    fn publish_frame(&self, request: CaptureRequest, pixel_id: Uuid) -> AppResult<()> {
        publish_frame(&self.app, request.profile_id, pixel_id, request.mode)
    }

    fn frame_result(&self) -> Option<AppResult<()>> {
        frame_result(&self.app)
    }

    fn hide_picker(&self) {
        hide(&self.app);
    }

    fn minimize_game(&self, game: WindowHandle) {
        foreground::minimize(game);
    }

    fn reveal_picker(&self) -> AppResult<()> {
        place(&self.app)?;
        shutter::play();
        show(&self.app)
    }

    fn close_picker(&self) {
        hide(&self.app);
        activate_main(&self.app);
    }

    fn emit(&self, status: CaptureStatus) {
        let _ = self.app.emit(STATUS_EVENT, status);
    }
}

fn lock_slot(mutex: &Mutex<PrepareSlot>) -> MutexGuard<'_, PrepareSlot> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PickerFrame {
        PickerFrame {
            profile_id: Uuid::new_v4(),
            pixel_id: Uuid::new_v4(),
            mode: CaptureMode::Pixel,
        }
    }

    #[test]
    fn stays_hidden_until_the_frame_is_drawn() {
        let mut picker = PickerState::default();
        assert!(picker.prepare());
        let frame = frame();
        picker.publish(frame.clone());
        assert!(!picker.can_reveal());
        assert!(!picker.mark_visible());
        assert!(!picker.visible);

        assert!(picker.mark_drawn(&frame));
        assert!(picker.can_reveal());
        assert!(!picker.visible);
        assert!(picker.mark_visible());
        assert!(picker.visible);
    }

    #[test]
    fn second_show_reuses_the_hidden_window() {
        let mut picker = PickerState::default();
        assert!(picker.prepare());
        let first = frame();
        picker.publish(first.clone());
        picker.mark_drawn(&first);
        assert!(picker.mark_visible());

        picker.hide();
        assert!(!picker.visible);
        assert!(picker.frame.is_none());
        assert!(!picker.destroyed);
        assert!(!picker.prepare());

        let second = frame();
        picker.publish(second.clone());
        assert!(!picker.mark_drawn(&first));
        assert!(picker.mark_drawn(&second));
        assert!(picker.mark_visible());
        assert!(picker.created);
        assert!(!picker.destroyed);
    }

    #[test]
    fn cancel_before_show_does_not_open_the_window() {
        let mut picker = PickerState::default();
        assert!(picker.prepare());
        picker.publish(frame());
        picker.hide();

        assert!(!picker.mark_visible());
        assert!(!picker.visible);
        assert!(picker.created);
        assert!(!picker.destroyed);
        assert!(picker.frame.is_none());
    }
}
