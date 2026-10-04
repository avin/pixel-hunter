use tauri::State;
use uuid::Uuid;

use crate::config::{Profile, Tracker};
use crate::config_store::ConfigStore;
use crate::error::AppResult;
use crate::processes::{self, WindowedProcess};
use crate::profiles::{self, ProfileInput, TrackerInput};

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
pub fn profile_delete(store: State<'_, ConfigStore>, id: Uuid) -> AppResult<()> {
    profiles::delete_profile(&store, id)
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
    profile_id: Uuid,
    tracker_id: Uuid,
) -> AppResult<()> {
    profiles::delete_tracker(&store, profile_id, tracker_id)
}

#[tauri::command]
pub fn windowed_processes_list() -> Vec<WindowedProcess> {
    processes::windowed_processes()
}
