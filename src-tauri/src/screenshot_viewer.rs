//! A reusable screenshot window, revealed only after its current image is ready.

use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{
    window::Color, AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, State, WebviewUrl,
    WebviewWindow, WebviewWindowBuilder, WindowEvent,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::foreground::{self, WindowHandle};
use crate::screen_overlay;

const LABEL: &str = "screenshot-viewer";
const OPEN_EVENT: &str = "screenshot-viewer://open";
const ERROR_EVENT: &str = "screenshot-viewer://error";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ViewOrigin {
    pub x: f64,
    pub y: f64,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ViewRequest {
    sequence: u64,
    profile_id: Uuid,
    pixel_id: Uuid,
    x: u32,
    y: u32,
    origin: Option<ViewOrigin>,
}

#[derive(Default)]
struct Requests {
    sequence: u64,
    pending: Option<ViewRequest>,
    prepared: bool,
}

impl Requests {
    fn open(
        &mut self,
        profile_id: Uuid,
        pixel_id: Uuid,
        x: u32,
        y: u32,
        origin: Option<ViewOrigin>,
    ) -> ViewRequest {
        self.sequence += 1;
        let request = ViewRequest {
            sequence: self.sequence,
            profile_id,
            pixel_id,
            x,
            y,
            origin,
        };
        self.pending = Some(request.clone());
        self.prepared = false;
        request
    }

    fn matches(&self, sequence: u64) -> bool {
        self.pending
            .as_ref()
            .is_some_and(|request| request.sequence == sequence)
    }

    fn invalidate(&mut self, sequence: u64) -> bool {
        if !self.matches(sequence) {
            return false;
        }
        self.pending = None;
        self.prepared = false;
        true
    }

    fn prepare(&mut self, sequence: u64) {
        if self.matches(sequence) {
            self.prepared = true;
        }
    }

    fn request(&self) -> Option<ViewRequest> {
        self.pending.as_ref().filter(|_| self.prepared).cloned()
    }

    fn can_reveal(&self, sequence: u64) -> bool {
        self.prepared && self.matches(sequence)
    }
}

#[derive(Default)]
pub struct ScreenshotViewer {
    requests: Mutex<Requests>,
    // Serializes creation without blocking the startup request command on the state lock.
    opening: Mutex<()>,
}

pub fn register_cleanup(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let app = app.clone();
        main.on_window_event(move |event| {
            if let WindowEvent::Destroyed = event {
                if let Some(viewer) = app.get_webview_window(LABEL) {
                    let _ = viewer.destroy();
                }
            }
        });
    }
}

pub fn open_viewer(
    app: &AppHandle,
    profile_id: Uuid,
    pixel_id: Uuid,
    x: u32,
    y: u32,
    origin: Option<PhysicalPosition<f64>>,
) -> AppResult<()> {
    let viewer = app.state::<ScreenshotViewer>();
    let _opening = viewer.opening.lock().unwrap();
    let monitor = match origin.as_ref() {
        Some(point) => app
            .monitor_from_point(point.x, point.y)
            .map_err(window_error)?,
        None => None,
    }
    .or(app.primary_monitor().map_err(window_error)?)
    .ok_or_else(|| AppError::Capture("Монитор не найден".into()))?;
    let origin = origin.map(|point| ViewOrigin {
        x: (point.x - f64::from(monitor.position().x)) / monitor.scale_factor(),
        y: (point.y - f64::from(monitor.position().y)) / monitor.scale_factor(),
    });
    let request = viewer
        .requests
        .lock()
        .unwrap()
        .open(profile_id, pixel_id, x, y, origin);
    let result = (|| {
        let window = match app.get_webview_window(LABEL) {
            Some(window) => {
                window.hide().map_err(window_error)?;
                window
            }
            None => WebviewWindowBuilder::new(
                app,
                LABEL,
                WebviewUrl::App("index.html?view=viewer".into()),
            )
            .title("Pixel Hunter")
            .decorations(false)
            .always_on_top(true)
            .skip_taskbar(true)
            .resizable(false)
            .closable(false)
            .shadow(false)
            .transparent(true)
            .background_color(Color(0, 0, 0, 0))
            .visible(false)
            .build()
            .map_err(window_error)?,
        };
        let position = monitor.position();
        let size = monitor.size();
        window
            .set_position(PhysicalPosition::new(position.x, position.y))
            .and_then(|()| window.set_size(PhysicalSize::new(size.width, size.height)))
            .map_err(window_error)?;
        viewer.requests.lock().unwrap().prepare(request.sequence);
        window.emit(OPEN_EVENT, &request).map_err(window_error)
    })();
    if result.is_err() {
        viewer.requests.lock().unwrap().invalidate(request.sequence);
    }
    result
}

#[tauri::command]
pub async fn screenshot_view_request(
    window: WebviewWindow,
    viewer: State<'_, ScreenshotViewer>,
) -> AppResult<Option<ViewRequest>> {
    validate_window(&window)?;
    Ok(viewer.requests.lock().unwrap().request())
}

#[tauri::command]
pub async fn screenshot_view_ready(
    window: WebviewWindow,
    viewer: State<'_, ScreenshotViewer>,
    sequence: u64,
) -> AppResult<()> {
    validate_window(&window)?;
    let requests = viewer.requests.lock().unwrap();
    if requests.can_reveal(sequence) {
        window.show().map_err(window_error)?;
        window.set_focus().map_err(window_error)?;
        if let Ok(hwnd) = window.hwnd() {
            foreground::bring_to_front(WindowHandle(hwnd.0 as isize));
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn screenshot_view_close(
    app: AppHandle,
    window: WebviewWindow,
    viewer: State<'_, ScreenshotViewer>,
    sequence: u64,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut requests = viewer.requests.lock().unwrap();
    if requests.invalidate(sequence) {
        window.hide().map_err(window_error)?;
        screen_overlay::activate_main(&app);
    }
    Ok(())
}

#[tauri::command]
pub async fn screenshot_view_failed(
    app: AppHandle,
    window: WebviewWindow,
    viewer: State<'_, ScreenshotViewer>,
    sequence: u64,
) -> AppResult<()> {
    validate_window(&window)?;
    let mut requests = viewer.requests.lock().unwrap();
    if requests.invalidate(sequence) {
        window.hide().map_err(window_error)?;
        screen_overlay::activate_main(&app);
        if let Some(main) = app.get_webview_window("main") {
            main.emit(ERROR_EVENT, "Не удалось загрузить снимок экрана")
                .map_err(window_error)?;
        }
    }
    Ok(())
}

fn validate_window(window: &WebviewWindow) -> AppResult<()> {
    if window.label() != LABEL {
        return Err(AppError::Validation(
            "Команда доступна только окну просмотра снимка".into(),
        ));
    }
    Ok(())
}

fn window_error(error: tauri::Error) -> AppError {
    AppError::Capture(format!("Не удалось показать окно снимка: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_latest_request_can_reveal_or_close_the_viewer() {
        let mut requests = Requests::default();
        let profile = Uuid::new_v4();
        let first = requests.open(profile, Uuid::new_v4(), 1, 2, None);
        requests.prepare(first.sequence);
        let second = requests.open(
            profile,
            Uuid::new_v4(),
            3,
            4,
            Some(ViewOrigin { x: 120.0, y: 240.0 }),
        );
        requests.prepare(second.sequence);
        assert!(second.sequence > first.sequence);
        assert!(!requests.matches(first.sequence));
        assert!(!requests.invalidate(first.sequence));
        assert!(requests.matches(second.sequence));
        assert!(!requests.can_reveal(first.sequence));
        assert!(requests.can_reveal(second.sequence));
        let pending = requests.pending.as_ref().unwrap();
        assert_eq!(
            pending.origin.as_ref().map(|point| (point.x, point.y)),
            Some((120.0, 240.0))
        );
        assert_eq!(
            (pending.profile_id, pending.pixel_id, pending.x, pending.y),
            (profile, second.pixel_id, 3, 4)
        );
    }

    #[test]
    fn closed_request_cannot_reveal_the_window_again() {
        let mut requests = Requests::default();
        let request = requests.open(Uuid::new_v4(), Uuid::new_v4(), 1, 2, None);
        requests.prepare(request.sequence);
        assert!(requests.invalidate(request.sequence));
        assert!(!requests.matches(request.sequence));
        assert!(!requests.can_reveal(request.sequence));
        assert!(!requests.invalidate(request.sequence));
        assert!(requests.pending.is_none());
        let reopened = requests.open(request.profile_id, request.pixel_id, 1, 2, None);
        assert!(reopened.sequence > request.sequence);
        assert!(!requests.invalidate(request.sequence));
        assert!(requests.matches(reopened.sequence));
    }

    #[test]
    fn startup_request_is_available_only_after_window_geometry_is_prepared() {
        let mut requests = Requests::default();
        let request = requests.open(Uuid::new_v4(), Uuid::new_v4(), 1, 2, None);
        assert!(requests.request().is_none());
        assert!(!requests.can_reveal(request.sequence));
        requests.prepare(request.sequence);
        assert_eq!(requests.request().unwrap().sequence, request.sequence);
        assert!(requests.can_reveal(request.sequence));
        requests.invalidate(request.sequence);
        requests.prepare(request.sequence);
        assert!(requests.request().is_none());
        assert!(!requests.can_reveal(request.sequence));
    }
}
