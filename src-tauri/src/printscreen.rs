//! A capture session owns this guard until cleanup. Only the hook thread touches
//! keyboard state; the callback must only signal work to the capture worker.
//! Accepted down/repeats/up are swallowed, then the hook removes itself. Drop
//! immediately disables input handling, unhooks, and joins before a new session.

use std::cell::RefCell;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc, Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_SNAPSHOT};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PeekMessageW, PostQuitMessage,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, LLKHF_ALTDOWN, MSG, PM_NOREMOVE, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP,
    WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

const MODIFIERS: [u32; 8] = [0xA0, 0xA1, 0xA2, 0xA3, 0xA4, 0xA5, 0x5B, 0x5C];

#[derive(Default)]
enum PressState {
    #[default]
    Waiting,
    Passing,
    Accepted,
    Finished,
}

#[derive(Debug, PartialEq)]
enum Decision {
    Pass,
    Trigger,
    Suppress,
    Release,
}

impl PressState {
    fn input(&mut self, down: bool, modified: bool) -> Decision {
        match (&*self, down) {
            (Self::Waiting, true) if !modified => {
                *self = Self::Accepted;
                Decision::Trigger
            }
            (Self::Waiting, true) => {
                *self = Self::Passing;
                Decision::Pass
            }
            (Self::Passing, false) => {
                *self = Self::Waiting;
                Decision::Pass
            }
            (Self::Accepted, true) => Decision::Suppress,
            (Self::Accepted, false) => {
                *self = Self::Finished;
                Decision::Release
            }
            _ => Decision::Pass,
        }
    }
}

struct CallbackState {
    press: PressState,
    modifiers: [bool; 8],
    cancelled: Arc<AtomicBool>,
    on_press: Option<Box<dyn FnOnce() + Send>>,
}

impl CallbackState {
    fn input(&mut self, vk: u32, down: bool, alt_down: bool) -> Decision {
        if self.cancelled.load(Ordering::Acquire) {
            return Decision::Pass;
        }
        if let Some(index) = MODIFIERS.iter().position(|modifier| *modifier == vk) {
            self.modifiers[index] = down;
        }
        if vk != u32::from(VK_SNAPSHOT.0) {
            return Decision::Pass;
        }
        self.press.input(
            down,
            self.modifiers.iter().any(|pressed| *pressed) || alt_down,
        )
    }
}

thread_local! {
    static CALLBACK: RefCell<Option<CallbackState>> = const { RefCell::new(None) };
}

unsafe extern "system" fn keyboard_hook(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code != 0 {
        return CallNextHookEx(None, code, wparam, lparam);
    }
    let down = match wparam.0 as u32 {
        WM_KEYDOWN | WM_SYSKEYDOWN => true,
        WM_KEYUP | WM_SYSKEYUP => false,
        _ => return CallNextHookEx(None, code, wparam, lparam),
    };
    let key = &*(lparam.0 as *const KBDLLHOOKSTRUCT);
    let decision = CALLBACK.with(|cell| {
        let mut state = cell.borrow_mut();
        let Some(state) = state.as_mut() else {
            return Decision::Pass;
        };
        let decision = state.input(key.vkCode, down, key.flags.contains(LLKHF_ALTDOWN));
        if decision == Decision::Trigger {
            if let Some(on_press) = state.on_press.take() {
                // Never allow a caller panic to unwind across the Windows ABI.
                let _ = std::panic::catch_unwind(std::panic::AssertUnwindSafe(on_press));
            }
        }
        decision
    });
    match decision {
        Decision::Pass => CallNextHookEx(None, code, wparam, lparam),
        Decision::Release => {
            PostQuitMessage(0);
            LRESULT(1)
        }
        Decision::Trigger | Decision::Suppress => LRESULT(1),
    }
}

// Store the handle as an integer because Windows handles are not Send. Access
// is serialized, and each successful installation is removed exactly once.
fn remove_hook(handle: &Mutex<Option<usize>>) {
    if let Some(handle) = handle.lock().unwrap().take() {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(handle as *mut _));
        }
    }
}

pub struct PrintScreenHook {
    cancelled: Arc<AtomicBool>,
    handle: Arc<Mutex<Option<usize>>>,
    thread_id: u32,
    worker: Option<JoinHandle<()>>,
}

impl PrintScreenHook {
    /// `on_press` executes on the hook thread and must only enqueue a command.
    pub fn install(on_press: impl FnOnce() + Send + 'static) -> Result<Self, String> {
        Self::install_with(on_press, || unsafe {
            SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook), None, 0)
                .map_err(|error| format!("Не удалось включить перехват PrintScreen: {error}"))
        })
    }

    fn install_with(
        on_press: impl FnOnce() + Send + 'static,
        install: impl FnOnce() -> Result<HHOOK, String> + Send + 'static,
    ) -> Result<Self, String> {
        let cancelled = Arc::new(AtomicBool::new(false));
        let handle = Arc::new(Mutex::new(None));
        let (ready_tx, ready_rx) = mpsc::sync_channel(1);
        let thread_cancelled = cancelled.clone();
        let thread_handle = handle.clone();
        let worker = thread::Builder::new()
            .name("capture-printscreen".into())
            .spawn(move || unsafe {
                let mut message = MSG::default();
                // Create the queue before exposing the id to Drop/PostThreadMessage.
                let _ = PeekMessageW(&mut message, None, 0, 0, PM_NOREMOVE);
                let modifiers = MODIFIERS.map(|vk| GetAsyncKeyState(vk as i32) < 0);
                let press = if GetAsyncKeyState(i32::from(VK_SNAPSHOT.0)) < 0 {
                    PressState::Passing
                } else {
                    PressState::Waiting
                };
                CALLBACK.with(|cell| {
                    *cell.borrow_mut() = Some(CallbackState {
                        press,
                        modifiers,
                        cancelled: thread_cancelled,
                        on_press: Some(Box::new(on_press)),
                    })
                });
                match install() {
                    Ok(hook) => {
                        *thread_handle.lock().unwrap() = Some(hook.0 as usize);
                        // Sample after installation so changes between initial
                        // sampling and install cannot leave a stale modifier.
                        // Later changes are delivered to this thread's hook.
                        CALLBACK.with(|cell| {
                            cell.borrow_mut().as_mut().unwrap().modifiers =
                                MODIFIERS.map(|vk| GetAsyncKeyState(vk as i32) < 0);
                        });
                        let _ = ready_tx.send(Ok(GetCurrentThreadId()));
                        while GetMessageW(&mut message, None, 0, 0).0 > 0 {
                            let _ = TranslateMessage(&message);
                            DispatchMessageW(&message);
                        }
                        remove_hook(&thread_handle);
                    }
                    Err(error) => {
                        let _ = ready_tx.send(Err(error));
                    }
                }
                CALLBACK.with(|cell| *cell.borrow_mut() = None);
            })
            .map_err(|error| format!("Не удалось запустить перехват PrintScreen: {error}"))?;
        match ready_rx.recv() {
            Ok(Ok(thread_id)) => Ok(Self {
                cancelled,
                handle,
                thread_id,
                worker: Some(worker),
            }),
            result => {
                let _ = worker.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    _ => "Поток перехвата PrintScreen завершился при запуске".into(),
                })
            }
        }
    }
}

impl Drop for PrintScreenHook {
    fn drop(&mut self) {
        self.cancelled.store(true, Ordering::Release);
        remove_hook(&self.handle);
        if let Some(worker) = self.worker.take() {
            if !worker.is_finished() {
                unsafe {
                    let _ = PostThreadMessageW(self.thread_id, WM_QUIT, WPARAM(0), LPARAM(0));
                }
            }
            let _ = worker.join();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn callback_state() -> CallbackState {
        CallbackState {
            press: PressState::default(),
            modifiers: [false; 8],
            cancelled: Arc::new(AtomicBool::new(false)),
            on_press: None,
        }
    }

    #[test]
    fn all_modifiers_and_other_keys_pass_through() {
        for modifier in MODIFIERS {
            let mut state = callback_state();
            assert_eq!(state.input(modifier, true, false), Decision::Pass);
            assert_eq!(
                state.input(u32::from(VK_SNAPSHOT.0), true, false),
                Decision::Pass
            );
            assert_eq!(
                state.input(u32::from(VK_SNAPSHOT.0), false, false),
                Decision::Pass
            );
            assert_eq!(state.input(modifier, false, false), Decision::Pass);
            assert_eq!(state.input(0x1B, true, false), Decision::Pass);
            assert_eq!(
                state.input(u32::from(VK_SNAPSHOT.0), true, false),
                Decision::Trigger
            );
            assert_eq!(state.input(0x41, true, false), Decision::Pass);
        }
    }

    #[test]
    fn cancellation_immediately_passes_even_an_accepted_key_release() {
        let mut state = callback_state();
        assert_eq!(
            state.input(u32::from(VK_SNAPSHOT.0), true, false),
            Decision::Trigger
        );
        state.cancelled.store(true, Ordering::Release);
        assert_eq!(
            state.input(u32::from(VK_SNAPSHOT.0), true, false),
            Decision::Pass
        );
        assert_eq!(
            state.input(u32::from(VK_SNAPSHOT.0), false, false),
            Decision::Pass
        );
    }

    #[test]
    fn plain_press_suppresses_the_whole_pair_and_triggers_only_once() {
        let mut state = PressState::default();
        assert_eq!(state.input(false, false), Decision::Pass);
        assert_eq!(state.input(true, false), Decision::Trigger);
        assert_eq!(state.input(true, false), Decision::Suppress);
        assert_eq!(state.input(true, true), Decision::Suppress);
        assert_eq!(state.input(false, true), Decision::Release);
        assert_eq!(state.input(true, false), Decision::Pass);
        assert_eq!(state.input(false, false), Decision::Pass);
    }

    #[test]
    fn modified_press_passes_even_if_modifier_is_released_during_repeat() {
        let mut state = PressState::default();
        assert_eq!(state.input(true, true), Decision::Pass);
        assert_eq!(state.input(true, false), Decision::Pass);
        assert_eq!(state.input(false, false), Decision::Pass);
        assert_eq!(state.input(true, false), Decision::Trigger);
    }

    #[test]
    fn installation_failure_returns_the_reason_without_triggering() {
        let result = PrintScreenHook::install_with(
            || panic!("unexpected trigger"),
            || Err("hook unavailable".into()),
        );
        assert!(matches!(result, Err(error) if error == "hook unavailable"));
    }

    #[test]
    fn drop_removes_an_installed_hook_and_joins_its_thread() {
        for _ in 0..3 {
            let hook = PrintScreenHook::install(|| panic!("unexpected trigger")).unwrap();
            let handle = hook.handle.clone();
            assert!(handle.lock().unwrap().is_some());
            drop(hook);
            assert!(handle.lock().unwrap().is_none());
        }
    }
}
