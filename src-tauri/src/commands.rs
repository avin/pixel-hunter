use tauri::{AppHandle, Emitter, Manager, State};
use uuid::Uuid;

use crate::action::{Action, KeyCombo};
use crate::action_sound::{self, AudioService, SoundInfo};
use crate::config::{LastScreen, Pixel, Profile, Settings, Tracker};
use crate::config_store::ConfigStore;
use crate::engine::{Engine, WorkPause};
use crate::error::AppResult;
use crate::hotkey::PauseHotkeys;
use crate::key_codes::{self, KeyCodeInfo};
use crate::pixel_capture::{self, CaptureCommand, CaptureRequest, PixelCapture};
use crate::processes::{self, WindowedProcess};
use crate::profiles::{self, PixelInput, ProfileInput, TrackerInput};
use crate::screen_overlay::{self, TauriCaptureEnv};
use crate::screenshots::Screenshots;
use crate::settings;

/// Toggles the session pause and tells every listener. Shared by the button and the hotkey.
pub fn toggle_work_pause(app: &AppHandle) -> WorkPause {
    let paused = app.state::<Engine>().toggle_user_pause();
    let state = WorkPause { paused };
    let _ = app.emit(crate::engine::PAUSE_EVENT, &state);
    state
}

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
pub fn tracker_set_running(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    is_running: bool,
) -> AppResult<Tracker> {
    profiles::set_tracker_running(&store, profile_id, tracker_id, is_running)
}

#[tauri::command]
pub fn engine_activate(
    store: State<'_, ConfigStore>,
    engine: State<'_, Engine>,
    profile_id: Uuid,
    epoch: u64,
) -> AppResult<()> {
    engine.activate(&store, profile_id, epoch)
}

#[tauri::command]
pub fn engine_deactivate(engine: State<'_, Engine>, epoch: u64) {
    engine.deactivate(epoch);
}

#[tauri::command]
pub fn settings_get(store: State<'_, ConfigStore>) -> Settings {
    settings::settings(&store)
}

#[tauri::command]
pub fn settings_set_last_screen(
    store: State<'_, ConfigStore>,
    last_screen: LastScreen,
) -> AppResult<LastScreen> {
    settings::set_last_screen(&store, last_screen)
}

#[tauri::command]
pub fn settings_set_pause_hotkey(
    store: State<'_, ConfigStore>,
    hotkeys: State<'_, PauseHotkeys>,
    pause_hotkey: KeyCombo,
) -> AppResult<KeyCombo> {
    hotkeys.lock().assign(&store, pause_hotkey)
}

#[tauri::command]
pub fn settings_clear_pause_hotkey(
    store: State<'_, ConfigStore>,
    hotkeys: State<'_, PauseHotkeys>,
) -> AppResult<()> {
    hotkeys.lock().clear(&store)
}

#[tauri::command]
pub fn pause_hotkey_begin(hotkeys: State<'_, PauseHotkeys>) {
    hotkeys.lock().begin_assign();
}

#[tauri::command]
pub fn pause_hotkey_cancel(hotkeys: State<'_, PauseHotkeys>) {
    hotkeys.lock().cancel_assign();
}

#[tauri::command]
pub fn work_pause_toggle(app: AppHandle) -> WorkPause {
    toggle_work_pause(&app)
}

#[tauri::command]
pub fn action_update(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    action: Action,
) -> AppResult<Action> {
    profiles::update_action(&store, profile_id, tracker_id, action)
}

#[tauri::command]
pub fn key_codes_list() -> Vec<KeyCodeInfo> {
    key_codes::list()
}

#[tauri::command]
pub fn action_sounds_list() -> Vec<SoundInfo> {
    action_sound::catalog().to_vec()
}

#[tauri::command]
pub async fn action_sound_preview(
    audio: State<'_, AudioService>,
    id: String,
    volume_percent: u8,
) -> AppResult<()> {
    let audio = audio.inner().clone();
    tauri::async_runtime::spawn_blocking(move || audio.preview(&id, volume_percent))
        .await
        .map_err(|error| crate::error::AppError::Audio(error.to_string()))?
}

#[tauri::command]
pub async fn action_sound_stop_preview(audio: State<'_, AudioService>) -> AppResult<()> {
    let audio = audio.inner().clone();
    tauri::async_runtime::spawn_blocking(move || audio.stop_preview())
        .await
        .map_err(|error| crate::error::AppError::Audio(error.to_string()))?
}

/// Plays the tracker's action 3 seconds later, leaving time to switch to the target window.
#[cfg(debug_assertions)]
#[tauri::command]
pub fn debug_run_action(
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    let action = profiles::get_action(&store, profile_id, tracker_id)?;
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_secs(3));
        if let Err(error) = crate::keyboard::run_action(&action) {
            eprintln!("Действие не выполнено: {error}");
        }
    });
    Ok(())
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
        let _engine_paused = app.state::<Engine>().pause();
        let env = TauriCaptureEnv::new(app.clone());
        let store = app.state::<ConfigStore>();
        let screenshots = app.state::<Screenshots>();
        pixel_capture::run_session(&env, &store, &screenshots, request, &commands);
    })?;
    Ok(())
}

/// Without a running session there is nobody to hide a leftover picker, so it is hidden here.
/// The warmed window stays alive for the next capture.
#[tauri::command]
pub fn pixel_capture_cancel(app: AppHandle, capture: State<'_, PixelCapture>) {
    if capture.send(CaptureCommand::Cancel).is_err() {
        screen_overlay::hide(&app);
    }
}

#[tauri::command]
pub fn pixel_capture_pick(capture: State<'_, PixelCapture>, x: u32, y: u32) -> AppResult<()> {
    capture.send(CaptureCommand::Pick { x, y })
}

// WebView2 window creation deadlocks in synchronous commands on Windows.
#[tauri::command]
pub async fn pixel_view_open(
    app: AppHandle,
    window: tauri::WebviewWindow,
    store: State<'_, ConfigStore>,
    profile_id: Uuid,
    tracker_id: Uuid,
    pixel_id: Uuid,
    origin: Option<crate::screenshot_viewer::ViewOrigin>,
) -> AppResult<()> {
    let pixel = profiles::get_pixel(&store, profile_id, tracker_id, pixel_id)?;
    let origin = origin
        .map(|point| {
            let position = window
                .inner_position()
                .map_err(|error| crate::error::AppError::Capture(error.to_string()))?;
            let scale = window
                .scale_factor()
                .map_err(|error| crate::error::AppError::Capture(error.to_string()))?;
            Ok::<_, crate::error::AppError>(tauri::PhysicalPosition::new(
                f64::from(position.x) + point.x * scale,
                f64::from(position.y) + point.y * scale,
            ))
        })
        .transpose()?;
    crate::screenshot_viewer::open_viewer(&app, profile_id, pixel.id, pixel.x, pixel.y, origin)
}

#[tauri::command]
pub fn screenshots_dir(screenshots: State<'_, Screenshots>) -> String {
    screenshots.root().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn windowed_processes_list() -> Vec<WindowedProcess> {
    processes::windowed_processes()
}
