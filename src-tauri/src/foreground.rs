//! The active (foreground) window and activation of our own windows.

use windows::Win32::Foundation::HWND;
use windows::Win32::System::Threading::{AttachThreadInput, GetCurrentThreadId};
use windows::Win32::UI::WindowsAndMessaging::{
    BringWindowToTop, GetForegroundWindow, GetWindowThreadProcessId, SetForegroundWindow,
    ShowWindow, SW_MINIMIZE,
};

use crate::processes;

/// Top-level window handle that can cross threads.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowHandle(pub isize);

impl WindowHandle {
    fn hwnd(self) -> HWND {
        HWND(self.0 as *mut _)
    }
}

impl From<HWND> for WindowHandle {
    fn from(hwnd: HWND) -> Self {
        Self(hwnd.0 as isize)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ForegroundWindow {
    pub window: WindowHandle,
    pub exe_name: String,
}

pub fn foreground_window() -> Option<ForegroundWindow> {
    unsafe {
        let hwnd = GetForegroundWindow();
        if hwnd.is_invalid() {
            return None;
        }
        let mut pid = 0;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        Some(ForegroundWindow {
            window: hwnd.into(),
            exe_name: processes::exe_name(pid)?,
        })
    }
}

/// Profiles store the exe name as listed; Windows file names are case-insensitive.
pub fn is_process(exe_name: &str, process_name: &str) -> bool {
    exe_name.eq_ignore_ascii_case(process_name)
}

pub fn minimize(window: WindowHandle) {
    unsafe {
        let _ = ShowWindow(window.hwnd(), SW_MINIMIZE);
    }
}

/// Windows refuses `SetForegroundWindow` from a background process unless its input is
/// attached to the thread that owns the current foreground window.
pub fn bring_to_front(window: WindowHandle) {
    let hwnd = window.hwnd();
    unsafe {
        let foreground = GetForegroundWindow();
        if foreground == hwnd {
            return;
        }
        let current_thread = GetCurrentThreadId();
        let foreground_thread = if foreground.is_invalid() {
            0
        } else {
            GetWindowThreadProcessId(foreground, None)
        };
        let is_attached = foreground_thread != 0
            && foreground_thread != current_thread
            && AttachThreadInput(current_thread, foreground_thread, true).as_bool();

        let _ = BringWindowToTop(hwnd);
        let _ = SetForegroundWindow(hwnd);

        if is_attached {
            let _ = AttachThreadInput(current_thread, foreground_thread, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_process_name_ignoring_case() {
        assert!(is_process("Game.EXE", "game.exe"));
        assert!(!is_process("game.exe", "game2.exe"));
    }

    /// Activate the window to check right after starting the test, e.g. Notepad:
    /// `cargo test -- --ignored prints_foreground_exe --nocapture`.
    #[test]
    #[ignore]
    fn prints_foreground_exe() {
        std::thread::sleep(std::time::Duration::from_secs(3));
        let foreground = foreground_window().expect("some window must be active");
        println!("foreground exe: {}", foreground.exe_name);
    }
}
