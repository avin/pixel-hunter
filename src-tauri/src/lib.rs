mod action;
mod action_sound;
mod commands;
mod config;
mod config_store;
mod engine;
mod error;
mod foreground;
mod hotkey;
mod icon_search;
mod key_codes;
mod key_sequence;
mod keyboard;
mod network;
mod pixel_capture;
mod printscreen;
mod processes;
mod profiles;
mod screen;
mod screen_overlay;
mod screenshot_viewer;
mod screenshots;
mod settings;
mod shutter;
mod tracker_hotkeys;

use std::path::Path;
use std::sync::Arc;

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
                    let state = event.state;
                    match tracker_hotkeys::dispatch_shortcut(
                        || {
                            app.state::<Arc<hotkey::PauseHotkeys>>()
                                .on_shortcut(shortcut, state)
                        },
                        || {
                            app.state::<tracker_hotkeys::TrackerHotkeyLink>()
                                .take_press(shortcut, state)
                        },
                    ) {
                        tracker_hotkeys::ShortcutDispatch::TogglePause => {
                            commands::toggle_work_pause(app);
                        }
                        tracker_hotkeys::ShortcutDispatch::Tracker(tracker_id) => {
                            app.state::<tracker_hotkeys::TrackerHotkeyLink>()
                                .send(tracker_id);
                        }
                        tracker_hotkeys::ShortcutDispatch::Ignore => {}
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
        .on_page_load(|webview, payload| {
            if webview.label() == "main"
                && payload.event() == tauri::webview::PageLoadEvent::Finished
            {
                // Window state is restored while hidden, before the page finishes loading.
                if let Err(error) = webview.window().show() {
                    eprintln!("failed to show main window: {error}");
                }
            }
        })
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let loaded = ConfigStore::load(&app_data_dir)?;
            if let Some(backup) = loaded.corrupt_backup {
                warn_about_corrupt_config(&backup);
            }
            let saved_hotkey = loaded
                .store
                .read(|config| config.settings.pause_hotkey.clone());
            let pause = Arc::new(hotkey::PauseHotkeys::install(
                saved_hotkey,
                app.handle().clone(),
            ));
            let emitter = app.handle().clone();
            let audio = action_sound::AudioService::start();
            app.manage(audio.clone());
            let icon_emitter = app.handle().clone();
            let (engine, tracker_hotkeys, gate) = Engine::start_with_visuals(
                audio,
                move |status| {
                    let _ = emitter.emit(engine::STATUS_EVENT, status);
                },
                Box::new(hotkey::OsShortcuts::new(app.handle().clone())),
                Arc::clone(&pause) as Arc<dyn hotkey::ShortcutMute>,
                app_data_dir.join("screenshots"),
                move |status| {
                    let _ = icon_emitter.emit(engine::ICON_STATUS_EVENT, status);
                },
            );
            app.manage(engine);
            app.manage(tracker_hotkeys);
            app.manage(gate);
            app.manage(pause);
            let handle = app.handle().clone();
            loaded.store.subscribe(move |previous, config| {
                handle.state::<Engine>().config_changed(previous, config)
            });
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
            commands::tracker_set_trigger,
            commands::tracker_set_hotkey,
            commands::engine_activate,
            commands::engine_deactivate,
            commands::settings_get,
            commands::settings_set_last_screen,
            commands::settings_set_pause_hotkey,
            commands::settings_clear_pause_hotkey,
            commands::pause_hotkey_begin,
            commands::pause_hotkey_cancel,
            commands::key_capture_begin,
            commands::key_capture_end,
            commands::work_pause_toggle,
            commands::action_update,
            commands::key_codes_list,
            commands::action_sounds_list,
            commands::action_sound_preview,
            commands::action_sound_stop_preview,
            #[cfg(debug_assertions)]
            commands::debug_run_action,
            commands::pixel_update,
            commands::icon_update,
            commands::icon_delete,
            commands::icon_capture_start,
            commands::icon_capture_select,
            commands::icon_view_open,
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
            screen_overlay::picker_pixels,
            screen_overlay::picker_frame_drawn,
            screen_overlay::picker_frame_failed,
            commands::screenshots_dir,
            commands::windowed_processes_list,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            if let tauri::RunEvent::ExitRequested { api, code, .. } = &event {
                if code.is_none() {
                    api.prevent_exit();
                    let app = app.clone();
                    std::thread::spawn(move || {
                        app.state::<PixelCapture>().shutdown();
                        app.exit(0);
                    });
                }
            }
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
