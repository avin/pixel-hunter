//! Borderless top-most window over the primary monitor that shows a screenshot one to one,
//! to pick a point (`view=picker`). The window is created hidden during capture prepare
//! and reused; it is shown only after the page has drawn the current frame.

use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
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
use crate::printscreen::PrintScreenHook;
use crate::screen::{Frame, PrimaryScreen};
use crate::screenshots;
use crate::shutter;
use std::sync::mpsc::Sender;

const LABEL: &str = "screen-overlay";
const MAIN_LABEL: &str = "main";
const FRAME_EVENT: &str = "screen-overlay://frame";
const CLEAR_EVENT: &str = "screen-overlay://clear";
/// Technical page-readiness limit; it does not time the user's PrintScreen input.
const PREPARE_LIMIT: Duration = Duration::from_secs(10);
const PREPARE_FAILED: &str = "Не удалось подготовить окно выбора";
const FRAME_FAILED: &str = "Не удалось показать снимок";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PickerFrame {
    pub session_id: Uuid,
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
    pixels: Option<Arc<[u8]>>,
    preparing_session: Option<Uuid>,
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

    fn publish(&mut self, frame: PickerFrame, pixels: Vec<u8>) {
        self.pixels = Some(pixels.into());
        self.frame = Some(frame);
        self.drawn = false;
        self.failed = false;
        self.visible = false;
    }

    fn pixels_for(
        &self,
        session_id: Uuid,
        profile_id: Uuid,
        pixel_id: Uuid,
    ) -> AppResult<Arc<[u8]>> {
        if !self.frame.as_ref().is_some_and(|frame| {
            frame.session_id == session_id
                && frame.profile_id == profile_id
                && frame.pixel_id == pixel_id
        }) {
            return Err(AppError::Validation("Сессия снимка устарела".into()));
        }
        self.pixels
            .clone()
            .ok_or_else(|| AppError::Capture(FRAME_FAILED.into()))
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
        self.pixels = None;
        self.drawn = false;
        self.failed = false;
    }

    fn mark_destroyed(&mut self) {
        self.destroyed = true;
        self.created = false;
        self.visible = false;
        self.page_ready = false;
        self.frame = None;
        self.pixels = None;
        self.drawn = false;
        self.failed = false;
    }
}

#[derive(Default)]
pub struct PickerOverlay {
    state: Mutex<PickerState>,
    changed: Condvar,
    creation: Mutex<()>,
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
pub fn prepare(app: &AppHandle, session_id: Uuid) -> AppResult<()> {
    shutter::prepare();
    ensure_hidden_window(app)?;
    let overlay = app.state::<PickerOverlay>();
    wait_for_page(&overlay, session_id)
}

fn wait_for_page(overlay: &PickerOverlay, session_id: Uuid) -> AppResult<()> {
    let (state, timeout) = overlay
        .changed
        .wait_timeout_while(overlay.lock(), PREPARE_LIMIT, |state| {
            !state.page_ready && !state.destroyed && state.preparing_session == Some(session_id)
        })
        .unwrap_or_else(PoisonError::into_inner);
    if state.preparing_session != Some(session_id) || state.destroyed {
        return Err(AppError::Capture("Подготовка отменена".into()));
    }
    if timeout.timed_out() && !state.page_ready {
        return Err(AppError::Capture(PREPARE_FAILED.into()));
    }
    Ok(())
}

pub fn publish_frame(
    app: &AppHandle,
    session_id: Uuid,
    profile_id: Uuid,
    pixel_id: Uuid,
    mode: CaptureMode,
    pixels: Vec<u8>,
) -> AppResult<()> {
    let frame = PickerFrame {
        session_id,
        profile_id,
        pixel_id,
        mode,
    };
    app.state::<PickerOverlay>()
        .lock()
        .publish(frame.clone(), pixels);
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
    let overlay = app.state::<PickerOverlay>();
    let _creation = lock_slot(&overlay.creation);
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
            app_handle.state::<PickerOverlay>().changed.notify_all();
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
    let current = window.app_handle().get_webview_window(LABEL);
    if window.label() != LABEL
        || current
            .as_ref()
            .is_none_or(|current| current.hwnd().ok() != window.hwnd().ok())
    {
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
    overlay.changed.notify_all();
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
pub async fn picker_pixels(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
    session_id: Uuid,
    profile_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<tauri::ipc::Response> {
    validate_window(&window)?;
    let pixels = overlay
        .lock()
        .pixels_for(session_id, profile_id, pixel_id)?;
    // Binary Response uses Tauri's raw IPC body, without JSON/base64 or asset-protocol disk IO.
    Ok(tauri::ipc::Response::new(pixels.to_vec()))
}

#[tauri::command]
pub async fn picker_frame_drawn(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
    capture: State<'_, PixelCapture>,
    session_id: Uuid,
    profile_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut state = overlay.lock();
    let mode = state.frame.as_ref().map(|f| f.mode).unwrap_or_default();
    let matched = state.mark_drawn(&PickerFrame {
        session_id,
        profile_id,
        pixel_id,
        mode,
    });
    drop(state);
    if matched {
        capture.send_to(session_id, CaptureCommand::FrameReady);
    }
    Ok(())
}

#[tauri::command]
pub async fn picker_frame_failed(
    window: WebviewWindow,
    overlay: State<'_, PickerOverlay>,
    capture: State<'_, PixelCapture>,
    session_id: Uuid,
    profile_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut state = overlay.lock();
    let mode = state.frame.as_ref().map(|f| f.mode).unwrap_or_default();
    let matched = state.mark_failed(&PickerFrame {
        session_id,
        profile_id,
        pixel_id,
        mode,
    });
    drop(state);
    if matched {
        capture.send_to(session_id, CaptureCommand::FrameFailed);
    }
    Ok(())
}

pub struct TauriCaptureEnv {
    app: AppHandle,
    started: Instant,
    prepare: Arc<Mutex<PrepareSlot>>,
    preparation: Mutex<Option<thread::JoinHandle<()>>>,
    hook: Mutex<Option<PrintScreenHook>>,
    request: CaptureRequest,
    sender: Sender<CaptureCommand>,
    screen: Mutex<Option<PrimaryScreen>>,
}

impl TauriCaptureEnv {
    pub fn new(app: AppHandle, request: CaptureRequest, sender: Sender<CaptureCommand>) -> Self {
        Self {
            app,
            started: Instant::now(),
            prepare: Arc::new(Mutex::new(PrepareSlot::default())),
            preparation: Mutex::new(None),
            hook: Mutex::new(None),
            request,
            sender,
            screen: Mutex::new(None),
        }
    }
}

impl CaptureEnv for TauriCaptureEnv {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn begin_wait(&self) -> AppResult<()> {
        let sender = self.sender.clone();
        let hook = PrintScreenHook::install(move || {
            let _ = sender.send(CaptureCommand::PrintScreen);
        })
        .map_err(AppError::Capture)?;
        *lock_slot(&self.hook) = Some(hook);
        // Device creation is front-loaded into starting. No desktop frame is acquired here.
        *lock_slot(&self.screen) = Some(PrimaryScreen::new()?);
        Ok(())
    }

    fn end_wait(&self) {
        lock_slot(&self.hook).take();
    }

    fn cleanup_prepare(&self) {
        let overlay = self.app.state::<PickerOverlay>();
        {
            let mut state = overlay.lock();
            if state.preparing_session == Some(self.request.session_id) {
                state.preparing_session = None;
                overlay.changed.notify_all();
            }
        }
        if let Some(worker) = lock_slot(&self.preparation).take() {
            let _ = worker.join();
        }
    }

    fn begin_prepare(&self) {
        self.app.state::<PickerOverlay>().lock().preparing_session = Some(self.request.session_id);
        let session_id = self.request.session_id;
        let generation = {
            let mut slot = lock_slot(&self.prepare);
            slot.generation += 1;
            slot.outcome = PrepareOutcome::Pending;
            slot.generation
        };
        let app = self.app.clone();
        let slot = Arc::clone(&self.prepare);
        let sender = self.sender.clone();
        *lock_slot(&self.preparation) = Some(thread::spawn(move || {
            let outcome = match prepare(&app, session_id) {
                Ok(()) => PrepareOutcome::Ready,
                Err(error) => PrepareOutcome::Failed(error.to_string()),
            };
            let notification = match &outcome {
                PrepareOutcome::Ready => CaptureCommand::PrepareReady,
                PrepareOutcome::Failed(reason) => CaptureCommand::PrepareFailed(reason.clone()),
                PrepareOutcome::Pending => unreachable!(),
            };
            let mut current = lock_slot(&slot);
            if current.generation == generation {
                current.outcome = outcome;
                drop(current);
                let _ = sender.send(notification);
            }
        }));
    }

    fn prepare_result(&self) -> Option<AppResult<()>> {
        match &lock_slot(&self.prepare).outcome {
            PrepareOutcome::Pending => None,
            PrepareOutcome::Ready => Some(Ok(())),
            PrepareOutcome::Failed(reason) => Some(Err(AppError::Capture(reason.clone()))),
        }
    }

    fn capture_frame(&self) -> AppResult<Frame> {
        lock_slot(&self.screen)
            .as_mut()
            .ok_or_else(|| AppError::Capture("Устройство снимка не готово".into()))?
            .capture_frame()
    }

    fn write_screenshot(&self, path: &Path, image: &RgbImage) -> AppResult<()> {
        screenshots::save_stored_png(path, image)
    }

    fn publish_frame(
        &self,
        request: CaptureRequest,
        pixel_id: Uuid,
        frame: &Frame,
    ) -> AppResult<()> {
        publish_frame(
            &self.app,
            request.session_id,
            request.profile_id,
            pixel_id,
            request.mode,
            frame.picker_pixels(),
        )
    }

    fn frame_result(&self) -> Option<AppResult<()>> {
        frame_result(&self.app)
    }

    fn hide_picker(&self) {
        hide(&self.app);
    }

    fn reveal_picker(&self) -> AppResult<()> {
        place(&self.app)?;
        show(&self.app)?;
        shutter::play();
        Ok(())
    }

    fn close_picker(&self) {
        hide(&self.app);
        activate_main(&self.app);
    }

    fn selection_accepted(&self, final_selection: bool) {
        if final_selection {
            self.close_picker();
        }
        shutter::play_selection();
    }

    fn emit(&self, status: CaptureStatus) {
        #[derive(Serialize, Clone)]
        #[serde(rename_all = "camelCase")]
        struct StatusEvent {
            session_id: Uuid,
            profile_id: Uuid,
            tracker_id: Uuid,
            mode: CaptureMode,
            #[serde(flatten)]
            status: CaptureStatus,
        }
        let _ = self.app.emit(
            STATUS_EVENT,
            StatusEvent {
                session_id: self.request.session_id,
                profile_id: self.request.profile_id,
                tracker_id: self.request.tracker_id,
                mode: self.request.mode,
                status,
            },
        );
    }
}

impl Drop for TauriCaptureEnv {
    fn drop(&mut self) {
        self.end_wait();
        self.cleanup_prepare();
        // Hidden preparation can finish after cancellation, but it can never publish or reveal.
        hide(&self.app);
    }
}

fn lock_slot<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame() -> PickerFrame {
        PickerFrame {
            session_id: Uuid::new_v4(),
            profile_id: Uuid::new_v4(),
            pixel_id: Uuid::new_v4(),
            mode: CaptureMode::Pixel,
        }
    }

    #[test]
    fn page_readiness_and_cancellation_wake_without_polling() {
        for cancel in [false, true] {
            let overlay = Arc::new(PickerOverlay::default());
            let session_id = Uuid::new_v4();
            overlay.lock().preparing_session = Some(session_id);
            let worker_overlay = overlay.clone();
            let (finished, completion) = std::sync::mpsc::channel();
            let worker = thread::spawn(move || {
                finished
                    .send(wait_for_page(&worker_overlay, session_id))
                    .unwrap();
            });
            {
                let mut state = overlay.lock();
                if cancel {
                    state.preparing_session = None;
                } else {
                    state.mark_page_ready();
                }
                overlay.changed.notify_all();
            }
            let result = completion
                .recv_timeout(Duration::from_secs(1))
                .expect("notification wakes readiness wait immediately");
            assert_eq!(result.is_err(), cancel);
            worker.join().unwrap();
        }
    }

    #[test]
    fn raw_pixels_are_addressed_and_removed_on_hide_or_destroy() {
        let mut picker = PickerState::default();
        let frame = frame();
        let pixels = vec![2, 0, 0, 0, 1, 0, 0, 0, 3, 2, 1, 255, 6, 5, 4, 255];
        picker.publish(frame.clone(), pixels.clone());
        assert_eq!(
            &*picker
                .pixels_for(frame.session_id, frame.profile_id, frame.pixel_id)
                .unwrap(),
            pixels
        );
        assert!(picker
            .pixels_for(Uuid::new_v4(), frame.profile_id, frame.pixel_id)
            .is_err());
        assert!(picker
            .pixels_for(frame.session_id, Uuid::new_v4(), frame.pixel_id)
            .is_err());
        assert!(picker
            .pixels_for(frame.session_id, frame.profile_id, Uuid::new_v4())
            .is_err());
        picker.hide();
        assert!(picker
            .pixels_for(frame.session_id, frame.profile_id, frame.pixel_id)
            .is_err());
        assert!(picker.pixels.is_none());
        picker.publish(frame.clone(), pixels);
        picker.mark_destroyed();
        assert!(picker.pixels.is_none());
    }

    #[test]
    fn stays_hidden_until_the_frame_is_drawn() {
        let mut picker = PickerState::default();
        assert!(picker.prepare());
        let frame = frame();
        picker.publish(frame.clone(), vec![]);
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
        picker.publish(first.clone(), vec![]);
        picker.mark_drawn(&first);
        assert!(picker.mark_visible());

        picker.hide();
        assert!(!picker.visible);
        assert!(picker.frame.is_none());
        assert!(!picker.destroyed);
        assert!(!picker.prepare());

        let second = frame();
        picker.publish(second.clone(), vec![]);
        assert!(!picker.mark_drawn(&first));
        assert!(picker.mark_drawn(&second));
        assert!(picker.mark_visible());
        assert!(picker.created);
        assert!(!picker.destroyed);
    }

    #[test]
    fn stale_session_cannot_confirm_or_fail_current_frame() {
        let mut picker = PickerState::default();
        picker.prepare();
        let current = frame();
        let stale = PickerFrame {
            session_id: Uuid::new_v4(),
            ..current.clone()
        };
        picker.publish(current.clone(), vec![]);
        assert!(!picker.mark_drawn(&stale));
        assert!(!picker.mark_failed(&stale));
        assert!(!picker.can_reveal());
        assert!(picker.mark_drawn(&current));
        assert!(picker.can_reveal());
    }

    #[test]
    fn cancel_before_show_does_not_open_the_window() {
        let mut picker = PickerState::default();
        assert!(picker.prepare());
        picker.publish(frame(), vec![]);
        picker.hide();

        assert!(!picker.mark_visible());
        assert!(!picker.visible);
        assert!(picker.created);
        assert!(!picker.destroyed);
        assert!(picker.frame.is_none());
    }
}
