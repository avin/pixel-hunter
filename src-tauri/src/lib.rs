mod commands;
mod config;
mod config_store;
mod error;
mod foreground;
mod pixel_capture;
mod processes;
mod profiles;
mod screen;
mod screen_overlay;
mod screenshots;
mod shutter;

use std::path::Path;

use tauri::Manager;

use config_store::ConfigStore;
use pixel_capture::PixelCapture;
use screenshots::Screenshots;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            let loaded = ConfigStore::load(&app_data_dir)?;
            if let Some(backup) = loaded.corrupt_backup {
                warn_about_corrupt_config(&backup);
            }
            app.manage(loaded.store);
            app.manage(Screenshots::new(app_data_dir.join("screenshots")));
            app.manage(PixelCapture::default());
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
            commands::pixel_update,
            commands::pixel_delete,
            commands::pixel_capture_start,
            commands::pixel_capture_cancel,
            commands::pixel_capture_pick,
            commands::pixel_view_open,
            commands::screenshots_dir,
            commands::windowed_processes_list,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
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
