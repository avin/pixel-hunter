use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

use crate::config::{Pixel, Profile, Tracker};
use crate::config_store::ConfigStore;
use crate::error::AppResult;
use crate::pixel_capture::{self, CaptureCommand, CaptureRequest, PixelCapture};
use crate::processes::{self, WindowedProcess};
use crate::profiles::{self, PixelInput, ProfileInput, TrackerInput};
use crate::screen_overlay::{self, TauriCaptureEnv};
use crate::screenshots::Screenshots;

#[tauri::command]
pub fn profiles_list(store: State<'_, ConfigStore>) -> Vec<Profile> {
    profiles::list_profiles(&store)
}

#[tauri::command]
pub fn profile_create(store: State<'_, ConfigStore>, input: ProfileInput) -> AppResult<Profile> {
    profiles::create_profile(&store, input)
}

#[tauri::command]
pub fn profile_update(
    store: State<'_, ConfigStore>,
    id: Uuid,
    input: ProfileInput,
) -> AppResult<Profile> {
    profiles::update_profile(&store, id, input)
}

#[tauri::command]
pub fn profile_delete(
    store: State<'_, ConfigStore>,
    screenshots: State<'_, Screenshots>,
    id: Uuid,
) -> AppResult<()> {
    profiles::delete_profile(&store, &screenshots, id)
}

#[tauri::command]
pub fn tracker_create(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    name: String,
) -> AppResult<Tracker> {
    profiles::create_tracker(&store, profile_id, name)
}

#[tauri::command]
pub fn tracker_update(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    input: TrackerInput,
) -> AppResult<Tracker> {
    profiles::update_tracker(&store, profile_id, tracker_id, input)
}

#[tauri::command]
pub fn tracker_delete(
    store: State<'_, ConfigStore>,
    screenshots: State<'_, Screenshots>,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    profiles::delete_tracker(&store, &screenshots, profile_id, tracker_id)
}

#[tauri::command]
pub fn pixel_update(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
    input: PixelInput,
) -> AppResult<Pixel> {
    profiles::update_pixel(&store, profile_id, tracker_id, pixel_id, input)
}

#[tauri::command]
pub fn pixel_delete(
    store: State<'_, ConfigStore>,
    screenshots: State<'_, Screenshots>,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    profiles::delete_pixel(&store, &screenshots, profile_id, tracker_id, pixel_id)
}

#[tauri::command]
pub fn pixel_capture_start(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    capture: State<'_, PixelCapture>,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    profiles::profile_process_name(&store, profile_id, tracker_id)?;
    let request = CaptureRequest {
        profile_id,
        tracker_id,
    };
    capture.start(move |commands| {
        let env = TauriCaptureEnv::new(app.clone());
        let store = app.state::<ConfigStore>();
        let screenshots = app.state::<Screenshots>();
        pixel_capture::run_session(&env, &store, &screenshots, request, &commands);
    })?;
    Ok(())
}

/// Without a running session there is nobody to close a leftover picker, so it is closed here.
#[tauri::command]
pub fn pixel_capture_cancel(app: AppHandle, capture: State<'_, PixelCapture>) {
    if capture.send(CaptureCommand::Cancel).is_err() {
        screen_overlay::close(&app);
    }
}

#[tauri::command]
pub fn pixel_capture_pick(capture: State<'_, PixelCapture>, x: u32, y: u32) -> AppResult<()> {
    capture.send(CaptureCommand::Pick { x, y })
}

#[tauri::command]
pub fn pixel_view_open(
    app: AppHandle,
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
) -> AppResult<()> {
    let pixel = profiles::get_pixel(&store, profile_id, tracker_id, pixel_id)?;
    screen_overlay::open_viewer(&app, profile_id, pixel.id, pixel.x, pixel.y)
}

#[tauri::command]
pub fn screenshots_dir(screenshots: State<'_, Screenshots>) -> String {
    screenshots.root().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn windowed_processes_list() -> Vec<WindowedProcess> {
    processes::windowed_processes()
}
