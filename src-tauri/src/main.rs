// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    #[cfg(not(debug_assertions))]
    elevation::relaunch_as_admin_if_needed();

    pixel_hunter_lib::run()
}

#[cfg(not(debug_assertions))]
mod elevation {
    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::HWND;
    use windows::Win32::Security::{
        AllocateAndInitializeSid, CheckTokenMembership, FreeSid, PSID, SECURITY_NT_AUTHORITY,
    };
    use windows::Win32::System::SystemServices::{
        DOMAIN_ALIAS_RID_ADMINS, SECURITY_BUILTIN_DOMAIN_RID,
    };
    use windows::Win32::UI::Shell::{ShellExecuteExW, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_NORMAL;

    /// Returns only when the current process already runs as administrator.
    pub fn relaunch_as_admin_if_needed() {
        if is_admin() {
            return;
        }
        let code = if relaunch_elevated() { 0 } else { 1 };
        std::process::exit(code);
    }

    fn is_admin() -> bool {
        let mut admins = PSID::default();
        let mut is_member = false.into();
        unsafe {
            if AllocateAndInitializeSid(
                &SECURITY_NT_AUTHORITY,
                2,
                SECURITY_BUILTIN_DOMAIN_RID as u32,
                DOMAIN_ALIAS_RID_ADMINS as u32,
                0,
                0,
                0,
                0,
                0,
                0,
                &mut admins,
            )
            .is_err()
            {
                return false;
            }
            let checked = CheckTokenMembership(None, admins, &mut is_member);
            FreeSid(admins);
            checked.is_ok() && is_member.as_bool()
        }
    }

    fn relaunch_elevated() -> bool {
        let Ok(exe) = std::env::current_exe() else {
            return false;
        };
        let exe = HSTRING::from(exe.as_os_str());
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            hwnd: HWND::default(),
            lpVerb: w!("runas"),
            lpFile: PCWSTR(exe.as_ptr()),
            nShow: SW_NORMAL.0,
            ..Default::default()
        };
        unsafe { ShellExecuteExW(&mut info).is_ok() }
    }
}
