//! Supported physical keys: `KeyboardEvent.code` → set 1 scan code and UI label.

use serde::Serialize;

use crate::action::Modifier;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyDef {
    pub code: &'static str,
    pub scan_code: u16,
    /// Sent with `KEYEVENTF_EXTENDEDKEY`, the `E0` prefix of the scan code.
    pub is_extended: bool,
    pub label: &'static str,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct KeyCodeInfo {
    pub code: &'static str,
    pub label: &'static str,
}

const fn key(code: &'static str, scan_code: u16, label: &'static str) -> KeyDef {
    KeyDef {
        code,
        scan_code,
        is_extended: false,
        label,
    }
}

const fn extended(code: &'static str, scan_code: u16, label: &'static str) -> KeyDef {
    KeyDef {
        code,
        scan_code,
        is_extended: true,
        label,
    }
}

/// Win, PrintScreen, Pause and the lock keys are left out on purpose.
const KEYS: &[KeyDef] = &[
    key("Escape", 0x01, "Esc"),
    key("Digit1", 0x02, "1"),
    key("Digit2", 0x03, "2"),
    key("Digit3", 0x04, "3"),
    key("Digit4", 0x05, "4"),
    key("Digit5", 0x06, "5"),
    key("Digit6", 0x07, "6"),
    key("Digit7", 0x08, "7"),
    key("Digit8", 0x09, "8"),
    key("Digit9", 0x0A, "9"),
    key("Digit0", 0x0B, "0"),
    key("Minus", 0x0C, "-"),
    key("Equal", 0x0D, "="),
    key("Backspace", 0x0E, "Backspace"),
    key("Tab", 0x0F, "Tab"),
    key("KeyQ", 0x10, "Q"),
    key("KeyW", 0x11, "W"),
    key("KeyE", 0x12, "E"),
    key("KeyR", 0x13, "R"),
    key("KeyT", 0x14, "T"),
    key("KeyY", 0x15, "Y"),
    key("KeyU", 0x16, "U"),
    key("KeyI", 0x17, "I"),
    key("KeyO", 0x18, "O"),
    key("KeyP", 0x19, "P"),
    key("BracketLeft", 0x1A, "["),
    key("BracketRight", 0x1B, "]"),
    key("Enter", 0x1C, "Enter"),
    key("ControlLeft", 0x1D, "Ctrl"),
    key("KeyA", 0x1E, "A"),
    key("KeyS", 0x1F, "S"),
    key("KeyD", 0x20, "D"),
    key("KeyF", 0x21, "F"),
    key("KeyG", 0x22, "G"),
    key("KeyH", 0x23, "H"),
    key("KeyJ", 0x24, "J"),
    key("KeyK", 0x25, "K"),
    key("KeyL", 0x26, "L"),
    key("Semicolon", 0x27, ";"),
    key("Quote", 0x28, "'"),
    key("Backquote", 0x29, "`"),
    key("ShiftLeft", 0x2A, "Shift"),
    key("Backslash", 0x2B, "\\"),
    key("KeyZ", 0x2C, "Z"),
    key("KeyX", 0x2D, "X"),
    key("KeyC", 0x2E, "C"),
    key("KeyV", 0x2F, "V"),
    key("KeyB", 0x30, "B"),
    key("KeyN", 0x31, "N"),
    key("KeyM", 0x32, "M"),
    key("Comma", 0x33, ","),
    key("Period", 0x34, "."),
    key("Slash", 0x35, "/"),
    key("ShiftRight", 0x36, "Правый Shift"),
    key("NumpadMultiply", 0x37, "Num *"),
    key("AltLeft", 0x38, "Alt"),
    key("Space", 0x39, "Пробел"),
    key("F1", 0x3B, "F1"),
    key("F2", 0x3C, "F2"),
    key("F3", 0x3D, "F3"),
    key("F4", 0x3E, "F4"),
    key("F5", 0x3F, "F5"),
    key("F6", 0x40, "F6"),
    key("F7", 0x41, "F7"),
    key("F8", 0x42, "F8"),
    key("F9", 0x43, "F9"),
    key("F10", 0x44, "F10"),
    key("Numpad7", 0x47, "Num 7"),
    key("Numpad8", 0x48, "Num 8"),
    key("Numpad9", 0x49, "Num 9"),
    key("NumpadSubtract", 0x4A, "Num -"),
    key("Numpad4", 0x4B, "Num 4"),
    key("Numpad5", 0x4C, "Num 5"),
    key("Numpad6", 0x4D, "Num 6"),
    key("NumpadAdd", 0x4E, "Num +"),
    key("Numpad1", 0x4F, "Num 1"),
    key("Numpad2", 0x50, "Num 2"),
    key("Numpad3", 0x51, "Num 3"),
    key("Numpad0", 0x52, "Num 0"),
    key("NumpadDecimal", 0x53, "Num ."),
    key("F11", 0x57, "F11"),
    key("F12", 0x58, "F12"),
    key("F13", 0x64, "F13"),
    key("F14", 0x65, "F14"),
    key("F15", 0x66, "F15"),
    key("F16", 0x67, "F16"),
    key("F17", 0x68, "F17"),
    key("F18", 0x69, "F18"),
    key("F19", 0x6A, "F19"),
    key("F20", 0x6B, "F20"),
    key("F21", 0x6C, "F21"),
    key("F22", 0x6D, "F22"),
    key("F23", 0x6E, "F23"),
    key("F24", 0x76, "F24"),
    extended("NumpadEnter", 0x1C, "Num Enter"),
    extended("ControlRight", 0x1D, "Правый Ctrl"),
    extended("NumpadDivide", 0x35, "Num /"),
    extended("AltRight", 0x38, "Правый Alt"),
    extended("Home", 0x47, "Home"),
    extended("ArrowUp", 0x48, "↑"),
    extended("PageUp", 0x49, "Page Up"),
    extended("ArrowLeft", 0x4B, "←"),
    extended("ArrowRight", 0x4D, "→"),
    extended("End", 0x4F, "End"),
    extended("ArrowDown", 0x50, "↓"),
    extended("PageDown", 0x51, "Page Down"),
    extended("Insert", 0x52, "Insert"),
    extended("Delete", 0x53, "Delete"),
];

pub fn lookup(code: &str) -> Option<&'static KeyDef> {
    KEYS.iter().find(|key| key.code == code)
}

/// Left keys send the modifiers of a combo.
pub fn modifier_key(modifier: Modifier) -> &'static KeyDef {
    let code = match modifier {
        Modifier::Ctrl => "ControlLeft",
        Modifier::Shift => "ShiftLeft",
        Modifier::Alt => "AltLeft",
    };
    lookup(code).expect("modifier keys are in the table")
}

/// The modifier a key acts as when it is pressed together with another key.
pub fn modifier_of(code: &str) -> Option<Modifier> {
    match code {
        "ControlLeft" | "ControlRight" => Some(Modifier::Ctrl),
        "ShiftLeft" | "ShiftRight" => Some(Modifier::Shift),
        "AltLeft" | "AltRight" => Some(Modifier::Alt),
        _ => None,
    }
}

pub fn list() -> Vec<KeyCodeInfo> {
    KEYS.iter()
        .map(|key| KeyCodeInfo {
            code: key.code,
            label: key.label,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn maps_codes_to_scan_codes() {
        let cases = [
            ("KeyA", 0x1E, false),
            ("ArrowUp", 0x48, true),
            ("NumpadEnter", 0x1C, true),
            ("Enter", 0x1C, false),
            ("F13", 0x64, false),
        ];
        for (code, scan_code, is_extended) in cases {
            let key = lookup(code).unwrap();
            assert_eq!(
                (key.scan_code, key.is_extended),
                (scan_code, is_extended),
                "{code}"
            );
        }
    }

    #[test]
    fn leaves_out_win_and_print_screen() {
        for code in ["MetaLeft", "MetaRight", "PrintScreen"] {
            assert!(lookup(code).is_none(), "{code}");
        }
    }

    #[test]
    fn covers_every_letter_digit_and_function_key() {
        for letter in 'A'..='Z' {
            assert!(lookup(&format!("Key{letter}")).is_some(), "{letter}");
        }
        for digit in 0..=9 {
            assert!(lookup(&format!("Digit{digit}")).is_some(), "{digit}");
            assert!(lookup(&format!("Numpad{digit}")).is_some(), "Numpad{digit}");
        }
        for number in 1..=24 {
            assert!(lookup(&format!("F{number}")).is_some(), "F{number}");
        }
    }

    #[test]
    fn has_unique_codes_and_scan_codes() {
        let codes: HashSet<_> = KEYS.iter().map(|key| key.code).collect();
        let scan_codes: HashSet<_> = KEYS
            .iter()
            .map(|key| (key.scan_code, key.is_extended))
            .collect();

        assert_eq!(codes.len(), KEYS.len());
        assert_eq!(scan_codes.len(), KEYS.len());
    }

    #[test]
    fn sends_combo_modifiers_with_left_keys() {
        assert_eq!(modifier_key(Modifier::Ctrl).scan_code, 0x1D);
        assert_eq!(modifier_key(Modifier::Shift).scan_code, 0x2A);
        assert_eq!(modifier_key(Modifier::Alt).scan_code, 0x38);
        assert!(!modifier_key(Modifier::Alt).is_extended);
    }
}
