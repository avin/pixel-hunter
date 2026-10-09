mod action;
mod action_sound;
mod commands;
mod config;
mod config_store;
mod engine;
mod error;
mod foreground;
mod hotkey;
mod key_codes;
mod key_sequence;
mod keyboard;
mod pixel_capture;
mod processes;
mod profiles;
mod screen;
mod screen_overlay;
mod screenshot_viewer;
mod screenshots;
mod settings;
mod shutter;

use std::path::Path;

use tauri::{Emitter, Manager};

use config_store::ConfigStore;
use engine::Engine;
use pixel_capture::PixelCapture;
use screenshots::Screenshots;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, shortcut, event| {
                    if app
                        .state::<hotkey::PauseHotkeys>()
                        .on_shortcut(shortcut, event.state)
                    {
                        commands::toggle_work_pause(app);
                    }
                })
                .build(),
        )
        .plugin(
            tauri_plugin_window_state::Builder::default()
                .with_filter(|label| label == "main")
                .with_state_flags(
                    tauri_plugin_window_state::StateFlags::SIZE
                        | tauri_plugin_window_state::StateFlags::POSITION
                        | tauri_plugin_window_state::StateFlags::MAXIMIZED,
                )
                .build(),
        )
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let loaded = ConfigStore::load(&app_data_dir)?;
            if let Some(backup) = loaded.corrupt_backup {
                warn_about_corrupt_config(&backup);
            }
            let emitter = app.handle().clone();
            let audio = action_sound::AudioService::start();
            app.manage(audio.clone());
            app.manage(Engine::start_with_audio(audio, move |status| {
                let _ = emitter.emit(engine::STATUS_EVENT, status);
            }));
            let saved_hotkey = loaded
                .store
                .read(|config| config.settings.pause_hotkey.clone());
            app.manage(hotkey::PauseHotkeys::install(
                saved_hotkey,
                app.handle().clone(),
            ));
            let handle = app.handle().clone();
            loaded
                .store
                .subscribe(move |config| handle.state::<Engine>().config_changed(config));
            app.manage(loaded.store);
            app.manage(Screenshots::new(app_data_dir.join("screenshots")));
            app.manage(PixelCapture::default());
            app.manage(screenshot_viewer::ScreenshotViewer::default());
            app.manage(screen_overlay::PickerOverlay::default());
            screenshot_viewer::register_cleanup(app.handle());
            screen_overlay::register_cleanup(app.handle());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::profiles_list,
            commands::profile_create,
            commands::profile_update,
            commands::profile_delete,
            commands::tracker_create,
            commands::tracker_update,
            commands::tracker_delete,
            commands::tracker_set_running,
            commands::engine_activate,
            commands::engine_deactivate,
            commands::settings_get,
            commands::settings_set_last_screen,
            commands::settings_set_pause_hotkey,
            commands::settings_clear_pause_hotkey,
            commands::pause_hotkey_begin,
            commands::pause_hotkey_cancel,
            commands::work_pause_toggle,
            commands::action_update,
            commands::key_codes_list,
            commands::action_sounds_list,
            commands::action_sound_preview,
            commands::action_sound_stop_preview,
            #[cfg(debug_assertions)]
            commands::debug_run_action,
            commands::pixel_update,
            commands::pixel_delete,
            commands::pixel_capture_start,
            commands::pixel_capture_cancel,
            commands::pixel_capture_pick,
            commands::pixel_view_open,
            screenshot_viewer::screenshot_view_request,
            screenshot_viewer::screenshot_view_ready,
            screenshot_viewer::screenshot_view_close,
            screenshot_viewer::screenshot_view_failed,
            screen_overlay::picker_ready,
            screen_overlay::picker_frame,
            screen_overlay::picker_frame_drawn,
            screen_overlay::picker_frame_failed,
            commands::screenshots_dir,
            commands::windowed_processes_list,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if matches!(event, tauri::RunEvent::Exit) {
                app.state::<action_sound::AudioService>().shutdown();
            }
        });
}

fn warn_about_corrupt_config(backup: &Path) {
    use windows::core::{h, HSTRING};
    use windows::Win32::UI::WindowsAndMessaging::{MessageBoxW, MB_ICONWARNING, MB_OK};

    let text = HSTRING::from(format!(
        "Файл конфигурации повреждён и не может быть прочитан. Приложение запущено с пустой конфигурацией.\n\nПрежний файл сохранён как:\n{}",
        backup.display()
    ));
    std::thread::spawn(move || unsafe {
        MessageBoxW(None, &text, h!("Pixel Hunter"), MB_OK | MB_ICONWARNING);
    });
}
