use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use windows::Win32::Foundation::{LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
};

const HOTKEY_ID: i32 = 0x4242;
const WM_APP_UPDATE: u32 = WM_APP + 1;
const WM_APP_QUIT: u32 = WM_APP + 2;

enum Command {
    Update {
        mods: Vec<String>,
        key: String,
        reply: Sender<Result<(), String>>,
    },
    Quit,
}

pub struct HotkeyManager {
    tx: Option<Sender<Command>>,
    thread_id: Arc<Mutex<Option<u32>>>,
}

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

// Returns None for unknown keys instead of silently falling back to Space
pub fn parse_key(key: &str) -> Option<u32> {
    let upper = key.to_uppercase();
    let code = match upper.as_str() {
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
            let mut chars = upper.chars();
            let (Some(c), None) = (chars.next(), chars.next()) else {
                return None;
            };
            if !c.is_ascii_alphanumeric() {
                return None;
            }
            c as u32
        }
    };
    Some(code)
}

// Swap the registration while keeping the previous bind usable if the new one is taken
fn apply_update(
    current_mods: &mut HOT_KEY_MODIFIERS,
    current_key: &mut u32,
    mods: Vec<String>,
    key: String,
) -> Result<(), String> {
    let Some(new_key) = parse_key(&key) else {
        return Err("Tecla de atajo no válida".to_string());
    };
    let new_mods = parse_modifiers(&mods);
    unsafe {
        let _ = UnregisterHotKey(None, HOTKEY_ID);
        if RegisterHotKey(None, HOTKEY_ID, new_mods, new_key).is_err() {
            // Someone else owns the combination: restore ours and report it
            let _ = RegisterHotKey(None, HOTKEY_ID, *current_mods, *current_key);
            return Err("El atajo ya está en uso por otra aplicación".to_string());
        }
    }
    *current_mods = new_mods;
    *current_key = new_key;
    Ok(())
}

impl HotkeyManager {
    pub fn new<F>(initial_mods: Vec<String>, initial_key: String, on_trigger: F) -> Self
    where
        F: Fn() + Send + 'static,
    {
        let (tx, rx) = channel::<Command>();
        let thread_id = Arc::new(Mutex::new(None));

        let thread_id_clone = thread_id.clone();
        thread::spawn(move || {
            let tid = unsafe { GetCurrentThreadId() };
            // Force the thread message queue to exist before anyone posts to it
            let mut msg = MSG::default();
            unsafe {
                let _ = PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE);
            }
            *thread_id_clone.lock().unwrap_or_else(|e| e.into_inner()) = Some(tid);

            let mut current_mods = parse_modifiers(&initial_mods);
            let mut current_key = parse_key(&initial_key).unwrap_or(0x20);

            unsafe {
                // A conflict here is surfaced when the user changes the bind in settings
                let _ = RegisterHotKey(None, HOTKEY_ID, current_mods, current_key);
            }

            loop {
                // Blocking wait: no busy loop, and updates wake us with a posted message
                let ret = unsafe { GetMessageW(&mut msg, None, 0, 0) };
                if ret.0 <= 0 {
                    break; // WM_QUIT or error
                }
                match msg.message {
                    WM_HOTKEY if msg.wParam.0 as i32 == HOTKEY_ID => on_trigger(),
                    WM_APP_UPDATE => {
                        while let Ok(cmd) = rx.try_recv() {
                            match cmd {
                                Command::Update { mods, key, reply } => {
                                    let result = apply_update(
                                        &mut current_mods,
                                        &mut current_key,
                                        mods,
                                        key,
                                    );
                                    let _ = reply.send(result);
                                }
                                Command::Quit => {
                                    unsafe {
                                        let _ = UnregisterHotKey(None, HOTKEY_ID);
                                    }
                                    return;
                                }
                            }
                        }
                    }
                    WM_APP_QUIT => {
                        unsafe {
                            let _ = UnregisterHotKey(None, HOTKEY_ID);
                        }
                        return;
                    }
                    _ => {}
                }
            }

            unsafe {
                let _ = UnregisterHotKey(None, HOTKEY_ID);
            }
        });

        Self {
            tx: Some(tx),
            thread_id,
        }
    }

    // Applies the bind and waits for the owning thread to confirm (or reject) it
    pub fn update_hotkey(&self, mods: Vec<String>, key: String) -> Result<(), String> {
        let Some(tx) = &self.tx else {
            return Err("Gestor de atajos no disponible".to_string());
        };
        let (reply_tx, reply_rx) = channel();
        tx.send(Command::Update {
            mods,
            key,
            reply: reply_tx,
        })
        .map_err(|_| "Gestor de atajos no disponible".to_string())?;

        if let Some(tid) = *self.thread_id.lock().unwrap_or_else(|e| e.into_inner()) {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_APP_UPDATE, WPARAM(0), LPARAM(0));
            }
        }

        match reply_rx.recv_timeout(Duration::from_millis(1500)) {
            Ok(result) => result,
            Err(_) => Err("No se pudo aplicar el atajo".to_string()),
        }
    }
}

impl Drop for HotkeyManager {
    fn drop(&mut self) {
        // The hotkey belongs to its own thread, so it must be released there
        if let Some(tx) = &self.tx {
            let _ = tx.send(Command::Quit);
        }
        if let Some(tid) = *self.thread_id.lock().unwrap_or_else(|e| e.into_inner()) {
            unsafe {
                let _ = PostThreadMessageW(tid, WM_APP_QUIT, WPARAM(0), LPARAM(0));
            }
        }
    }
}
