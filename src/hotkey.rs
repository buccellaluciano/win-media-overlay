use std::sync::mpsc::{channel, Sender};
use std::thread;
use std::time::Duration;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{PeekMessageW, MSG, PM_REMOVE, WM_HOTKEY};

pub struct HotkeyManager {
    update_tx: Option<Sender<(Vec<String>, String)>>,
}

const HOTKEY_ID: i32 = 0x4242;

pub fn parse_modifiers(mods: &[String]) -> HOT_KEY_MODIFIERS {
    let mut result = MOD_NOREPEAT.0;
    for m in mods {
        match m.to_lowercase().as_str() {
            "control" | "ctrl" => result |= MOD_CONTROL.0,
            "shift" => result |= MOD_SHIFT.0,
            "alt" => result |= MOD_ALT.0,
            "win" | "super" | "meta" => result |= MOD_WIN.0,
            _ => {}
        }
    }
    HOT_KEY_MODIFIERS(result)
}

pub fn parse_key(key: &str) -> u32 {
    let upper = key.to_uppercase();
    match upper.as_str() {
        "SPACE" => 0x20,
        "ENTER" | "RETURN" => 0x0D,
        "ESCAPE" | "ESC" => 0x1B,
        "TAB" => 0x09,
        "BACKSPACE" => 0x08,
        "UP" => 0x26,
        "DOWN" => 0x28,
        "LEFT" => 0x25,
        "RIGHT" => 0x27,
        "F1" => 0x70,
        "F2" => 0x71,
        "F3" => 0x72,
        "F4" => 0x73,
        "F5" => 0x74,
        "F6" => 0x75,
        "F7" => 0x76,
        "F8" => 0x77,
        "F9" => 0x78,
        "F10" => 0x79,
        "F11" => 0x7A,
        "F12" => 0x7B,
        "MEDIA_PLAY_PAUSE" => 0xB3,
        "MEDIA_NEXT" => 0xB0,
        "MEDIA_PREV" => 0xB1,
        _ => {
            if upper.len() == 1 {
                let c = upper.chars().next().unwrap();
                if c.is_ascii_alphanumeric() {
                    return c as u32;
                }
            }
            0x20 // default to space
        }
    }
}

impl HotkeyManager {
    pub fn new<F>(initial_mods: Vec<String>, initial_key: String, on_trigger: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        let (tx, rx) = channel::<(Vec<String>, String)>();

        thread::spawn(move || {
            let mut current_mods = parse_modifiers(&initial_mods);
            let mut current_key = parse_key(&initial_key);

            // Register initial hotkey with NULL HWND (thread message queue)
            unsafe {
                let _ = RegisterHotKey(None, HOTKEY_ID, current_mods, current_key);
            }

            loop {
                // Process any dynamic hotkey updates
                while let Ok((new_mods, new_key)) = rx.try_recv() {
                    unsafe {
                        let _ = UnregisterHotKey(None, HOTKEY_ID);
                        current_mods = parse_modifiers(&new_mods);
                        current_key = parse_key(&new_key);
                        let _ = RegisterHotKey(None, HOTKEY_ID, current_mods, current_key);
                    }
                }

                let mut msg = MSG::default();
                unsafe {
                    while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                        if msg.message == WM_HOTKEY && msg.wParam.0 as i32 == HOTKEY_ID {
                            on_trigger();
                        }
                    }
                }

                thread::sleep(Duration::from_millis(35));
            }
        });

        Self {
            update_tx: Some(tx),
        }
    }

    pub fn update_hotkey(&self, mods: Vec<String>, key: String) {
        if let Some(tx) = &self.update_tx {
            let _ = tx.send((mods, key));
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(None, HOTKEY_ID);
        }
    }
}
