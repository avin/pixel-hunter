use std::collections::HashMap;
use std::path::Path;

use serde::Serialize;
use windows::core::{BOOL, PWSTR};
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
    PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindow, GetWindowLongPtrW, GetWindowTextLengthW, GetWindowTextW,
    GetWindowThreadProcessId, IsWindowVisible, GWL_EXSTYLE, GW_OWNER, WS_EX_TOOLWINDOW,
};

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowedProcess {
    pub exe_name: String,
    pub window_titles: Vec<String>,
}

struct AppWindow {
    pid: u32,
    title: String,
}

/// Processes, except this one, that own a visible top-level window, grouped by exe name.
pub fn windowed_processes() -> Vec<WindowedProcess> {
    let mut windows: Vec<AppWindow> = Vec::new();
    unsafe {
        let _ = EnumWindows(
            Some(collect_app_window),
            LPARAM(&mut windows as *mut Vec<AppWindow> as isize),
        );
    }

    let own_pid = std::process::id();
    let mut exe_names: HashMap<u32, Option<String>> = HashMap::new();
    let named_windows = windows
        .into_iter()
        .filter(|window| window.pid != own_pid)
        .filter_map(|window| {
            let exe_name = exe_names
                .entry(window.pid)
                .or_insert_with(|| exe_name(window.pid))
                .clone()?;
            Some((exe_name, window.title))
        });

    group_by_exe(named_windows)
}

fn group_by_exe(windows: impl IntoIterator<Item = (String, String)>) -> Vec<WindowedProcess> {
    let mut groups: Vec<WindowedProcess> = Vec::new();
    for (exe_name, title) in windows {
        match groups
            .iter_mut()
            .find(|group| group.exe_name.eq_ignore_ascii_case(&exe_name))
        {
            Some(group) if !group.window_titles.contains(&title) => {
                group.window_titles.push(title)
            }
            Some(_) => {}
            None => groups.push(WindowedProcess {
                exe_name,
                window_titles: vec![title],
            }),
        }
    }
    groups.sort_by_key(|group| group.exe_name.to_lowercase());
    groups
}

unsafe extern "system" fn collect_app_window(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<AppWindow>) };
    if let Some(window) = unsafe { app_window(hwnd) } {
        windows.push(window);
    }
    true.into()
}

/// Mirrors what the taskbar shows: visible, unowned, not a tool window, titled and not cloaked.
unsafe fn app_window(hwnd: HWND) -> Option<AppWindow> {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return None;
        }
        if GetWindow(hwnd, GW_OWNER).is_ok_and(|owner| !owner.is_invalid()) {
            return None;
        }
        if GetWindowLongPtrW(hwnd, GWL_EXSTYLE) & WS_EX_TOOLWINDOW.0 as isize != 0 {
            return None;
        }

        let mut cloaked: u32 = 0;
        let is_cloaked = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            size_of::<u32>() as u32,
        )
        .is_ok()
            && cloaked != 0;
        if is_cloaked {
            return None;
        }

        let length = GetWindowTextLengthW(hwnd);
        if length <= 0 {
            return None;
        }
        let mut buffer = vec![0u16; length as usize + 1];
        let copied = GetWindowTextW(hwnd, &mut buffer);
        let title = String::from_utf16_lossy(&buffer[..copied.max(0) as usize])
            .trim()
            .to_string();
        if title.is_empty() {
            return None;
        }

        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        Some(AppWindow { pid, title })
    }
}

fn exe_name(pid: u32) -> Option<String> {
    unsafe {
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buffer = vec![0u16; 32_768];
        let mut size = buffer.len() as u32;
        let queried = QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buffer.as_mut_ptr()),
            &mut size,
        );
        let _ = CloseHandle(process);
        queried.ok()?;

        let path = String::from_utf16_lossy(&buffer[..size as usize]);
        Path::new(&path)
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn window(exe_name: &str, title: &str) -> (String, String) {
        (exe_name.into(), title.into())
    }

    #[test]
    fn groups_titles_by_exe_name_ignoring_case() {
        let groups = group_by_exe([
            window("notepad.exe", "a.txt - Блокнот"),
            window("Game.exe", "Game"),
            window("Notepad.exe", "b.txt - Блокнот"),
            window("notepad.exe", "a.txt - Блокнот"),
        ]);

        assert_eq!(
            groups,
            [
                WindowedProcess {
                    exe_name: "Game.exe".into(),
                    window_titles: vec!["Game".into()],
                },
                WindowedProcess {
                    exe_name: "notepad.exe".into(),
                    window_titles: vec!["a.txt - Блокнот".into(), "b.txt - Блокнот".into()],
                },
            ]
        );
    }

    /// Needs a running Notepad (or a replacement such as Notepad2):
    /// `cargo test -- --ignored lists_running_notepad --nocapture`.
    #[test]
    #[ignore]
    fn lists_running_notepad() {
        let processes = windowed_processes();
        println!("{processes:#?}");

        let notepad = processes
            .iter()
            .find(|process| process.exe_name.to_lowercase().starts_with("notepad"))
            .expect("Notepad must be listed");
        assert!(!notepad.window_titles.is_empty());
        assert!(!processes
            .iter()
            .any(|process| process.exe_name.eq_ignore_ascii_case("svchost.exe")));
    }
}
