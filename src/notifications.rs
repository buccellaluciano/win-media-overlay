// Windows notifications -> quiet WMO toasts (top-right), so Do Not Disturb can stay on.
// ponytail: unpackaged apps can't subscribe to NotificationChanged (0x80070490 without package
// identity), so we poll GetNotificationsAsync and diff ids. Public WinRT API with the user's
// permission: no hooks, no other-process access.
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::Duration;
use tauri::{Emitter, LogicalSize, Manager, Size, WebviewWindow};
use windows::core::Interface;
use windows::Foundation::Size as WinSize;
use windows::Storage::Streams::DataReader;
use windows::UI::Notifications::Management::{
    UserNotificationListener, UserNotificationListenerAccessStatus,
};
use windows::UI::Notifications::{KnownNotificationBindings, NotificationKinds, UserNotification};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, SetWindowLongPtrW, ShowWindow, GWL_EXSTYLE, SW_SHOWNOACTIVATE,
    WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
};

use crate::app_icon;
use crate::media::{image_mime, to_base64};
use crate::AppState;

pub const NOTIFY_LABEL: &str = "notify";
pub const NOTIFY_WIDTH: f64 = 340.0;
const POLL: Duration = Duration::from_millis(1500);
const MAX_PER_POLL: usize = 3; // a burst (e.g. after waking up) must not flood the screen
const LOGO_MAX_BYTES: u64 = 200_000;
const LOGO_PX: i32 = 64; // shown at 32 CSS px: sharp up to 200 % scaling

#[derive(Debug, Clone, Serialize)]
pub struct NotifyToast {
    pub id: u32,
    pub app: String,
    pub title: String,
    pub body: String,
    pub logo: Option<String>,
}

static WAKE: Mutex<Option<Sender<()>>> = Mutex::new(None);
// "allowed" | "denied" | "unspecified" | "unavailable" (read by the settings sheet)
static ACCESS: Mutex<&'static str> = Mutex::new("unspecified");

pub fn wake() {
    if let Ok(guard) = WAKE.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(());
        }
    }
}

pub fn access_status() -> &'static str {
    ACCESS.lock().map(|g| *g).unwrap_or("unavailable")
}

pub fn spawn(app: tauri::AppHandle) {
    let (tx, rx) = channel::<()>();
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    std::thread::spawn(move || run(app, rx));
}

fn set_access(value: &'static str) {
    if let Ok(mut g) = ACCESS.lock() {
        *g = value;
    }
}

fn run(app: tauri::AppHandle, rx: Receiver<()>) {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
    let mut listener: Option<UserNotificationListener> = None;
    // None until the first successful read: what is already in the center is never announced
    let mut seen: Option<HashSet<u32>> = None;
    let mut logos: HashMap<String, Option<String>> = HashMap::new();

    loop {
        let (enabled, show_body, ignore, respect_fullscreen) = {
            let state = app.state::<AppState>();
            let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
            (
                conf.notif_enabled,
                conf.notif_show_body,
                conf.notif_ignore.clone(),
                conf.respect_fullscreen,
            )
        };

        if !enabled {
            // Feature off: forget state and block (zero wakeups) until the config changes
            listener = None;
            seen = None;
            let _ = rx.recv();
            continue;
        }

        if listener.is_none() {
            listener = UserNotificationListener::Current().ok();
        }
        let Some(l) = listener.as_ref() else {
            set_access("unavailable");
            let _ = rx.recv_timeout(Duration::from_secs(10));
            continue;
        };

        match l.GetAccessStatus() {
            Ok(s) if s == UserNotificationListenerAccessStatus::Allowed => set_access("allowed"),
            Ok(s) if s == UserNotificationListenerAccessStatus::Denied => {
                set_access("denied");
                let _ = rx.recv_timeout(Duration::from_secs(10));
                continue;
            }
            Ok(_) => set_access("unspecified"),
            Err(_) => {
                set_access("unavailable");
                listener = None;
                let _ = rx.recv_timeout(Duration::from_secs(10));
                continue;
            }
        }

        let list: Vec<UserNotification> = match l
            .GetNotificationsAsync(NotificationKinds::Toast)
            .and_then(|op| op.get())
        {
            Ok(v) => v.into_iter().collect(),
            Err(_) => {
                listener = None; // recreate next round (shell restart, RPC error)
                let _ = rx.recv_timeout(POLL);
                continue;
            }
        };

        let ids: HashSet<u32> = list.iter().filter_map(|n| n.Id().ok()).collect();
        if let Some(prev) = seen.as_ref() {
            let fullscreen = respect_fullscreen
                && app
                    .state::<AppState>()
                    .fullscreen_active
                    .load(std::sync::atomic::Ordering::Relaxed);
            let mut fresh: Vec<&UserNotification> = list
                .iter()
                .filter(|n| n.Id().map(|id| !prev.contains(&id)).unwrap_or(false))
                .collect();
            // Newest last, and only the latest few of a burst
            fresh.sort_by_key(|n| n.CreationTime().map(|t| t.UniversalTime).unwrap_or(0));
            let skip = fresh.len().saturating_sub(MAX_PER_POLL);
            if !fullscreen {
                for n in fresh.into_iter().skip(skip) {
                    if let Some(toast) = read_toast(n, show_body, &mut logos) {
                        let lower = toast.app.to_lowercase();
                        let ignored = ignore
                            .iter()
                            .any(|i| !i.trim().is_empty() && lower.contains(&i.trim().to_lowercase()));
                        if !ignored {
                            announce(&app, toast);
                        }
                    }
                }
            }
        }
        seen = Some(ids);
        let _ = rx.recv_timeout(POLL);
    }
}

// App name, title (first text), body (rest) and a small app logo. Any failure skips the field.
fn read_toast(
    n: &UserNotification,
    show_body: bool,
    logos: &mut HashMap<String, Option<String>>,
) -> Option<NotifyToast> {
    let id = n.Id().ok()?;
    let info = n.AppInfo().ok();
    let display = info.as_ref().and_then(|i| i.DisplayInfo().ok());
    let app = display
        .as_ref()
        .and_then(|d| d.DisplayName().ok())
        .map(|h| h.to_string())
        .unwrap_or_else(|| "Notificación".to_string());

    let texts: Vec<String> = n
        .Notification()
        .and_then(|x| x.Visual())
        .and_then(|v| v.GetBinding(&KnownNotificationBindings::ToastGeneric()?))
        .ok()
        .filter(|b| !b.as_raw().is_null())
        .and_then(|b| b.GetTextElements().ok())
        .map(|els| {
            els.into_iter()
                .filter_map(|t| t.Text().ok())
                .map(|h| h.to_string())
                .filter(|s| !s.trim().is_empty())
                .collect()
        })
        .unwrap_or_default();

    let title = texts.first().cloned().unwrap_or_default();
    let body = if show_body { texts.iter().skip(1).cloned().collect::<Vec<_>>().join("\n") } else { String::new() };

    // Packaged apps expose a logo; unpackaged Win32 apps (Discord, PowerShell…) return null there,
    // so fall back to the Start-menu icon resolved by AUMID. Cached per app.
    let aumid = info
        .as_ref()
        .and_then(|i| i.AppUserModelId().ok())
        .map(|h| h.to_string())
        .unwrap_or_default();
    let cache_key = if aumid.is_empty() { app.clone() } else { aumid.clone() };
    let logo = logos
        .entry(cache_key)
        .or_insert_with(|| {
            display
                .as_ref()
                .and_then(read_logo)
                .or_else(|| app_icon::icon_data_url(&aumid, LOGO_PX))
        })
        .clone();

    Some(NotifyToast {
        id,
        app,
        title: truncate(&title, 160),
        body: truncate(&body, 400),
        logo,
    })
}

fn read_logo(display: &windows::ApplicationModel::AppDisplayInfo) -> Option<String> {
    let reference = display
        .GetLogo(WinSize {
            Width: 32.0,
            Height: 32.0,
        })
        .ok()?;
    let stream = reference.OpenReadAsync().ok()?.get().ok()?;
    let size = stream.Size().ok()?;
    if size == 0 || size > LOGO_MAX_BYTES {
        return None;
    }
    let reader = DataReader::CreateDataReader(&stream).ok()?;
    reader.LoadAsync(size as u32).ok()?.get().ok()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf).ok()?;
    Some(format!("data:{};base64,{}", image_mime(&buf), to_base64(&buf)))
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max).collect();
        out.push('…');
        out
    }
}

pub fn announce(app: &tauri::AppHandle, toast: NotifyToast) {
    if let Some(win) = app.get_webview_window(NOTIFY_LABEL) {
        if !win.is_visible().unwrap_or(false) {
            place_notify(app, &win);
            // Never activate: no focus change, no sound, the game/app keeps the keyboard
            if let Ok(hwnd) = win.hwnd() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
        }
        let _ = win.emit("notification", toast);
    }
}

// Top-right of the work area of the monitor where the island lives
fn place_notify(app: &tauri::AppHandle, win: &WebviewWindow) {
    let Some(main) = app.get_webview_window("main") else {
        return;
    };
    let Ok(main_hwnd) = main.hwnd() else {
        return;
    };
    unsafe {
        let hmon = MonitorFromWindow(main_hwnd, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmon, &mut info).as_bool() {
            return;
        }
        let scale = main.scale_factor().unwrap_or(1.0);
        let w = (NOTIFY_WIDTH * scale) as i32;
        let margin = (14.0 * scale) as i32;
        let x = info.rcWork.right - w - margin;
        let y = info.rcWork.top + margin;
        let _ = win.set_position(tauri::Position::Physical(tauri::PhysicalPosition { x, y }));
    }
}

// The toast window must never take focus, even when clicked, and stays out of Alt+Tab
pub fn setup_window(win: &WebviewWindow) {
    if let Ok(hwnd) = win.hwnd() {
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let ex = ex | WS_EX_NOACTIVATE.0 as isize | WS_EX_TOOLWINDOW.0 as isize;
            let _ = SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex);
        }
    }
}

// Height follows the stack. Width and top-left never change, so a resize can't show a shifted
// frame (content is top-aligned); empty stack hides the window.
#[tauri::command]
pub fn notify_layout(height: f64, app: tauri::AppHandle) -> Result<(), String> {
    let Some(win) = app.get_webview_window(NOTIFY_LABEL) else {
        return Ok(());
    };
    if !height.is_finite() || height <= 1.0 {
        let _ = win.hide();
        return Ok(());
    }
    let height = height.clamp(40.0, 900.0);
    win.set_size(Size::Logical(LogicalSize {
        width: NOTIFY_WIDTH,
        height,
    }))
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn notify_access() -> &'static str {
    access_status()
}

// Settings "Probar aviso": a local sample, no real notification involved
#[tauri::command]
pub fn notify_test(app: tauri::AppHandle) {
    announce(
        &app,
        NotifyToast {
            id: 0,
            app: "WMO".to_string(),
            title: "Así se ven los avisos".to_string(),
            body: "Llegan sin sonido y sin quitarle el foco a la app que estés usando.".to_string(),
            logo: None,
        },
    );
}
