//! Borderless top-most window over the primary monitor that shows a screenshot one to one,
//! to pick a point (`view=picker`).

use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, WebviewUrl, WebviewWindow,
    WebviewWindowBuilder, WindowEvent,
};
use uuid::Uuid;

use crate::error::{AppError, AppResult};
use crate::foreground::{self, WindowHandle};
use crate::pixel_capture::{
    CaptureCommand, CaptureEnv, CaptureRequest, CaptureStatus, PixelCapture, STATUS_EVENT,
};
use crate::screen::{Frame, PrimaryScreen};
use crate::shutter;

const LABEL: &str = "screen-overlay";
const MAIN_LABEL: &str = "main";

pub fn open_picker(app: &AppHandle, profile_id: Uuid, pixel_id: Uuid) -> AppResult<()> {
    open(
        app,
        &format!("view=picker&profileId={profile_id}&pixelId={pixel_id}"),
    )
}

pub fn close(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.destroy();
    }
}

pub fn activate_main(app: &AppHandle) {
    if let Some(main) = app.get_webview_window(MAIN_LABEL) {
        let _ = main.unminimize();
        let _ = main.show();
        bring_to_front(&main);
    }
}

fn open(app: &AppHandle, query: &str) -> AppResult<()> {
    let monitor = app
        .primary_monitor()
        .map_err(window_error)?
        .ok_or_else(|| AppError::Capture("Основной монитор не найден".into()))?;

    let window = match app.get_webview_window(LABEL) {
        Some(window) => {
            let mut url = window.url().map_err(window_error)?;
            url.set_query(Some(query));
            window.navigate(url).map_err(window_error)?;
            window
        }
        None => {
            let url = WebviewUrl::App(format!("index.html?{query}").into());
            let window = WebviewWindowBuilder::new(app, LABEL, url)
                .title("Pixel Hunter")
                .decorations(false)
                .always_on_top(true)
                .skip_taskbar(true)
                .resizable(false)
                .shadow(false)
                .visible(false)
                .build()
                .map_err(window_error)?;
            let app = app.clone();
            window.on_window_event(move |event| {
                if let WindowEvent::Destroyed = event {
                    let _ = app
                        .state::<PixelCapture>()
                        .send(CaptureCommand::OverlayClosed);
                }
            });
            window
        }
    };

    let position = monitor.position();
    let size = monitor.size();
    window
        .set_position(PhysicalPosition::new(position.x, position.y))
        .and_then(|()| window.set_size(PhysicalSize::new(size.width, size.height)))
        .and_then(|()| window.show())
        .map_err(window_error)?;
    bring_to_front(&window);
    Ok(())
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

pub struct TauriCaptureEnv {
    app: AppHandle,
    started: Instant,
}

impl TauriCaptureEnv {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            started: Instant::now(),
        }
    }
}

impl CaptureEnv for TauriCaptureEnv {
    fn now(&self) -> Duration {
        self.started.elapsed()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn foreground_game(&self, process_name: &str) -> Option<WindowHandle> {
        foreground::foreground_window()
            .filter(|window| foreground::is_process(&window.exe_name, process_name))
            .map(|window| window.window)
    }

    fn capture_frame(&self) -> AppResult<Frame> {
        PrimaryScreen::new()?.capture_frame()
    }

    fn play_shutter(&self) {
        shutter::play();
    }

    fn show_picker(
        &self,
        game: WindowHandle,
        request: CaptureRequest,
        pixel_id: Uuid,
    ) -> AppResult<()> {
        foreground::minimize(game);
        open_picker(&self.app, request.profile_id, pixel_id)
    }

    fn close_picker(&self) {
        close(&self.app);
        activate_main(&self.app);
    }

    fn emit(&self, status: CaptureStatus) {
        let _ = self.app.emit(STATUS_EVENT, status);
    }
}
