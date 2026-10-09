//! Real keyboard input: `SendInput` with scan codes, which DirectInput and Raw Input games read.

use std::time::Duration;

use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_EXTENDEDKEY, KEYEVENTF_KEYUP,
    KEYEVENTF_SCANCODE, VIRTUAL_KEY,
};

use crate::action::{Action, ActionKind};
use crate::error::AppResult;
use crate::key_sequence::{self, InputSink, KeyEvent, Sleeper};

pub struct SendInputSink;

impl InputSink for SendInputSink {
    fn send(&mut self, event: KeyEvent) {
        let mut flags = KEYEVENTF_SCANCODE;
        if event.is_extended {
            flags |= KEYEVENTF_EXTENDEDKEY;
        }
        if event.is_up {
            flags |= KEYEVENTF_KEYUP;
        }
        let input = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(0),
                    wScan: event.scan_code,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let sent = unsafe { SendInput(&[input], size_of::<INPUT>() as i32) };
        if sent == 0 {
            eprintln!(
                "SendInput не отправил {event:?}: {}",
                windows::core::Error::from_thread()
            );
        }
    }
}

pub struct ThreadSleeper;

impl Sleeper for ThreadSleeper {
    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }
}

/// 1 ms system timer resolution while alive; the default 15.6 ms would swallow the hold spread.
pub struct TimerResolution;

impl TimerResolution {
    pub fn begin() -> Self {
        unsafe { timeBeginPeriod(1) };
        Self
    }
}

impl Drop for TimerResolution {
    fn drop(&mut self) {
        unsafe { timeEndPeriod(1) };
    }
}

/// Blocks the calling thread until the whole action has been played.
pub fn run_action(action: &Action) -> AppResult<()> {
    let _resolution = TimerResolution::begin();
    match &action.kind {
        ActionKind::KeySequence(sequence) => key_sequence::execute_key_sequence(
            sequence,
            &mut SendInputSink,
            &ThreadSleeper,
            &mut rand::rng(),
        ),
    }
}
