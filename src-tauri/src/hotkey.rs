//! Global pause hotkey: translates a [`KeyCombo`] and registers it with the OS.

use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use tauri::AppHandle;
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

use crate::action::{KeyCombo, Modifier};
use crate::config_store::ConfigStore;
use crate::error::{AppError, AppResult};
use crate::settings::{self, Accelerator};

pub trait ShortcutHost {
    fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()>;
    fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()>;
}

/// Registers and removes shortcuts through `tauri-plugin-global-shortcut`.
pub struct OsShortcuts {
    app: AppHandle,
}

impl ShortcutHost for OsShortcuts {
    fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
        let shortcut = to_shortcut(accelerator).ok_or(())?;
        self.app
            .global_shortcut()
            .register(shortcut)
            .map_err(|_| ())
    }

    fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
        let shortcut = to_shortcut(accelerator).ok_or(())?;
        self.app
            .global_shortcut()
            .unregister(shortcut)
            .map_err(|_| ())
    }
}

pub fn to_shortcut(accelerator: &Accelerator) -> Option<Shortcut> {
    let key = accelerator.key.parse::<Code>().ok()?;
    let mut mods = Modifiers::empty();
    for modifier in &accelerator.modifiers {
        mods |= match modifier {
            Modifier::Ctrl => Modifiers::CONTROL,
            Modifier::Shift => Modifiers::SHIFT,
            Modifier::Alt => Modifiers::ALT,
        };
    }
    Some(Shortcut::new((!mods.is_empty()).then_some(mods), key))
}

/// The plugin invokes its handler while holding its shortcut table lock. Keep event handling
/// separate from the binding lock, and never hold this lock across plugin calls.
#[derive(Default)]
struct ShortcutEvents {
    active: Option<u32>,
    held: bool,
}

impl ShortcutEvents {
    fn on_shortcut(&mut self, shortcut: &Shortcut, state: ShortcutState) -> bool {
        if self.active != Some(shortcut.id()) {
            return false;
        }
        match state {
            ShortcutState::Pressed if !self.held => {
                self.held = true;
                true
            }
            ShortcutState::Released => {
                self.held = false;
                false
            }
            _ => false,
        }
    }
}

/// Saved combo plus what is currently registered. Assignment mode drops the registration so the
/// window can see the keys, then either commits a new combo or puts the previous one back.
pub struct HotkeyBinding<H> {
    host: H,
    saved: Option<Accelerator>,
    registered: Option<Accelerator>,
    assigning: bool,
    events: Arc<Mutex<ShortcutEvents>>,
}

impl<H: ShortcutHost> HotkeyBinding<H> {
    pub fn install(saved: Option<KeyCombo>, host: H) -> Self {
        let saved = saved.as_ref().and_then(settings::accelerator);
        let mut binding = Self {
            host,
            saved: saved.clone(),
            registered: None,
            assigning: false,
            events: Arc::default(),
        };
        if let Some(accelerator) = saved {
            binding.register(accelerator);
        }
        binding
    }

    pub fn begin_assign(&mut self) {
        self.assigning = true;
        self.unregister_current();
    }

    pub fn cancel_assign(&mut self) {
        if !self.assigning {
            return;
        }
        self.assigning = false;
        self.restore_saved();
    }

    /// Registers `combo` and stores it. An unsupported code or an OS refusal leaves the previous
    /// combo registered and saved.
    pub fn assign(&mut self, store: &ConfigStore, combo: KeyCombo) -> AppResult<KeyCombo> {
        let Some(accelerator) = settings::accelerator(&combo) else {
            return Err(AppError::Validation("Эта клавиша не поддерживается".into()));
        };
        let normalized = KeyCombo {
            modifiers: accelerator.modifiers.clone(),
            key: accelerator.key.clone(),
        };
        self.unregister_current();
        if self.host.register(&accelerator).is_err() {
            self.assigning = false;
            self.restore_saved();
            return Err(AppError::Validation(
                "Система не может занять это сочетание".into(),
            ));
        }
        self.registered = Some(accelerator);
        if let Err(error) = settings::set_pause_hotkey(store, Some(normalized.clone())) {
            self.unregister_current();
            self.assigning = false;
            self.restore_saved();
            return Err(error);
        }
        self.saved = self.registered.clone();
        self.assigning = false;
        self.enable_events();
        Ok(normalized)
    }

    pub fn clear(&mut self, store: &ConfigStore) -> AppResult<()> {
        self.unregister_current();
        if let Err(error) = settings::set_pause_hotkey(store, None) {
            self.restore_saved();
            return Err(error);
        }
        self.saved = None;
        self.assigning = false;
        Ok(())
    }

    fn enable_events(&self) {
        let active = self
            .registered
            .as_ref()
            .and_then(to_shortcut)
            .map(|shortcut| shortcut.id());
        *self.events.lock().unwrap_or_else(PoisonError::into_inner) = ShortcutEvents {
            active,
            held: false,
        };
    }

    fn register(&mut self, accelerator: Accelerator) {
        if self.host.register(&accelerator).is_ok() {
            self.registered = Some(accelerator);
            if !self.assigning {
                self.enable_events();
            }
        }
    }

    fn unregister_current(&mut self) {
        *self.events.lock().unwrap_or_else(PoisonError::into_inner) = ShortcutEvents::default();
        if let Some(accelerator) = self.registered.take() {
            let _ = self.host.unregister(&accelerator);
        }
    }

    fn restore_saved(&mut self) {
        if self.registered.is_some() {
            return;
        }
        if let Some(accelerator) = self.saved.clone() {
            self.register(accelerator);
        }
    }
}

pub struct PauseHotkeys {
    binding: Mutex<HotkeyBinding<OsShortcuts>>,
    events: Arc<Mutex<ShortcutEvents>>,
}

impl PauseHotkeys {
    pub fn install(saved: Option<KeyCombo>, app: AppHandle) -> Self {
        let binding = HotkeyBinding::install(saved, OsShortcuts { app });
        Self {
            events: Arc::clone(&binding.events),
            binding: Mutex::new(binding),
        }
    }

    pub fn lock(&self) -> MutexGuard<'_, HotkeyBinding<OsShortcuts>> {
        self.binding.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Handles one plugin event. `true` when the pause should toggle.
    pub fn on_shortcut(&self, shortcut: &Shortcut, state: ShortcutState) -> bool {
        self.events
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .on_shortcut(shortcut, state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Modifier;
    use std::sync::mpsc;
    use std::time::Duration;

    #[derive(Default)]
    struct FakeHost {
        fail: bool,
        registered: Option<Accelerator>,
        log: Vec<&'static str>,
    }

    impl ShortcutHost for FakeHost {
        fn register(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            self.log.push("register");
            if self.fail {
                self.fail = false;
                return Err(());
            }
            self.registered = Some(accelerator.clone());
            Ok(())
        }

        fn unregister(&mut self, accelerator: &Accelerator) -> Result<(), ()> {
            self.log.push("unregister");
            if self.registered.as_ref() == Some(accelerator) {
                self.registered = None;
            }
            Ok(())
        }
    }

    fn combo(modifiers: Vec<Modifier>, key: &str) -> KeyCombo {
        KeyCombo {
            modifiers,
            key: key.into(),
        }
    }

    fn saved_store(hotkey: Option<KeyCombo>) -> (tempfile::TempDir, ConfigStore) {
        let dir = tempfile::tempdir().unwrap();
        let store = ConfigStore::load(dir.path()).unwrap().store;
        if let Some(hotkey) = hotkey {
            settings::set_pause_hotkey(&store, Some(hotkey)).unwrap();
        }
        (dir, store)
    }

    #[test]
    fn every_catalog_key_is_a_global_shortcut() {
        for info in crate::key_codes::list() {
            let accelerator = settings::accelerator(&combo(vec![], info.code))
                .unwrap_or_else(|| panic!("catalog rejected {}", info.code));
            assert!(
                to_shortcut(&accelerator).is_some(),
                "global shortcut cannot use {}",
                info.code
            );
        }
    }

    #[test]
    fn translates_supported_combos_and_rejects_an_unsupported_code() {
        let ctrl_shift_q =
            settings::accelerator(&combo(vec![Modifier::Ctrl, Modifier::Shift], "KeyQ")).unwrap();
        let shortcut = to_shortcut(&ctrl_shift_q).unwrap();
        assert!(shortcut.matches(Modifiers::CONTROL | Modifiers::SHIFT, Code::KeyQ));

        let f5 = settings::accelerator(&combo(vec![], "F5")).unwrap();
        assert!(to_shortcut(&f5)
            .unwrap()
            .matches(Modifiers::empty(), Code::F5));

        let alone = settings::accelerator(&combo(vec![], "ControlLeft")).unwrap();
        assert!(to_shortcut(&alone)
            .unwrap()
            .matches(Modifiers::empty(), Code::ControlLeft));

        assert!(settings::accelerator(&combo(vec![], "PrintScreen")).is_none());
        assert!(settings::accelerator(&combo(vec![Modifier::Alt], "MetaLeft")).is_none());
    }

    #[test]
    fn os_refusal_keeps_the_previous_combo() {
        let (_dir, store) = saved_store(Some(combo(vec![Modifier::Ctrl], "KeyQ")));
        let mut binding = HotkeyBinding::install(
            Some(combo(vec![Modifier::Ctrl], "KeyQ")),
            FakeHost::default(),
        );
        binding.host.fail = true;

        let result = binding.assign(&store, combo(vec![], "F5"));

        assert!(matches!(result, Err(AppError::Validation(_))));
        assert_eq!(
            settings::settings(&store).pause_hotkey,
            Some(combo(vec![Modifier::Ctrl], "KeyQ"))
        );
        assert_eq!(
            binding
                .host
                .registered
                .as_ref()
                .map(|item| item.key.as_str()),
            Some("KeyQ")
        );
    }

    #[test]
    fn leaving_assignment_restores_the_previous_registration() {
        let mut binding = HotkeyBinding::install(Some(combo(vec![], "F5")), FakeHost::default());
        assert_eq!(
            binding
                .host
                .registered
                .as_ref()
                .map(|item| item.key.as_str()),
            Some("F5")
        );

        binding.begin_assign();
        assert!(binding.host.registered.is_none());

        binding.cancel_assign();
        assert_eq!(
            binding
                .host
                .registered
                .as_ref()
                .map(|item| item.key.as_str()),
            Some("F5")
        );
    }

    #[test]
    fn a_repeated_hold_toggles_once() {
        let binding = HotkeyBinding::install(Some(combo(vec![], "F5")), FakeHost::default());
        let shortcut = to_shortcut(binding.registered.as_ref().unwrap()).unwrap();
        let mut toggles = 0;
        let press = |toggles: &mut i32| {
            if binding
                .events
                .lock()
                .unwrap()
                .on_shortcut(&shortcut, ShortcutState::Pressed)
            {
                *toggles += 1;
            }
        };
        press(&mut toggles);
        press(&mut toggles);
        assert_eq!(toggles, 1);
        binding
            .events
            .lock()
            .unwrap()
            .on_shortcut(&shortcut, ShortcutState::Released);
        press(&mut toggles);
        assert_eq!(toggles, 2);
    }

    #[test]
    fn clear_unregisters_and_forgets_the_combo() {
        let (_dir, store) = saved_store(Some(combo(vec![], "F5")));
        let mut binding = HotkeyBinding::install(Some(combo(vec![], "F5")), FakeHost::default());

        binding.clear(&store).unwrap();

        assert!(binding.host.registered.is_none());
        assert_eq!(settings::settings(&store).pause_hotkey, None);
        let shortcut = to_shortcut(&settings::accelerator(&combo(vec![], "F5")).unwrap()).unwrap();
        assert!(!binding
            .events
            .lock()
            .unwrap()
            .on_shortcut(&shortcut, ShortcutState::Pressed));
    }

    #[test]
    fn plugin_callback_finishes_while_assignment_waits_for_plugin_lock() {
        struct LockedHost {
            plugin_lock: Arc<Mutex<()>>,
            unregistering: mpsc::Sender<()>,
        }

        impl ShortcutHost for LockedHost {
            fn register(&mut self, _: &Accelerator) -> Result<(), ()> {
                let _guard = self.plugin_lock.lock().unwrap();
                Ok(())
            }

            fn unregister(&mut self, _: &Accelerator) -> Result<(), ()> {
                self.unregistering.send(()).unwrap();
                let _guard = self.plugin_lock.lock().unwrap();
                Ok(())
            }
        }

        let plugin_lock = Arc::new(Mutex::new(()));
        let (unregistering, unregister_started) = mpsc::channel();
        let binding = HotkeyBinding::install(
            Some(combo(vec![], "F5")),
            LockedHost {
                plugin_lock: Arc::clone(&plugin_lock),
                unregistering,
            },
        );
        let shortcut = to_shortcut(binding.registered.as_ref().unwrap()).unwrap();
        let events = Arc::clone(&binding.events);
        let binding = Arc::new(Mutex::new(binding));
        let plugin_guard = plugin_lock.lock().unwrap();
        let assigning = std::thread::spawn(move || binding.lock().unwrap().begin_assign());
        unregister_started
            .recv_timeout(Duration::from_secs(2))
            .unwrap();

        // Like the plugin's handler, the callback runs while its table is locked.
        let (finished, callback_finished) = mpsc::channel();
        let callback = std::thread::spawn(move || {
            let mut events = events.lock().unwrap();
            assert!(!events.on_shortcut(&shortcut, ShortcutState::Released));
            assert!(!events.on_shortcut(&shortcut, ShortcutState::Pressed));
            finished.send(()).unwrap();
        });
        let result = callback_finished.recv_timeout(Duration::from_secs(2));
        drop(plugin_guard);
        assigning.join().unwrap();
        callback.join().unwrap();
        assert!(
            result.is_ok(),
            "callback waited for the binding during assignment"
        );
    }

    #[test]
    fn events_from_the_previous_shortcut_do_not_toggle_the_new_binding() {
        let (_dir, store) = saved_store(Some(combo(vec![], "F5")));
        let mut binding = HotkeyBinding::install(Some(combo(vec![], "F5")), FakeHost::default());
        let old = to_shortcut(binding.registered.as_ref().unwrap()).unwrap();
        binding.begin_assign();
        binding.assign(&store, combo(vec![], "F6")).unwrap();
        let new = to_shortcut(binding.registered.as_ref().unwrap()).unwrap();
        let mut events = binding.events.lock().unwrap();
        assert!(!events.on_shortcut(&old, ShortcutState::Pressed));
        assert!(events.on_shortcut(&new, ShortcutState::Pressed));
        assert!(!events.on_shortcut(&old, ShortcutState::Released));
        assert!(!events.on_shortcut(&new, ShortcutState::Pressed));
    }
}
