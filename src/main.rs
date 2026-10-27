// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_session;
mod config;
mod fullscreen;
mod hotkey;
mod media;
mod position;
mod sysstats;
mod volume;

use config::{load_config, save_config, AppConfig};
use hotkey::HotkeyManager;
use media::{MediaController, MediaState};
use position::{apply_top_center_on_monitor, apply_top_center_size, apply_window_position};

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, LogicalSize, Manager, Size, WebviewWindow, WindowEvent};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::POINT;
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, ShowWindow, SW_SHOWNOACTIVATE};

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub hotkey_mgr: Mutex<Option<Arc<HotkeyManager>>>,
    pub last_shown: Mutex<Instant>,
    // Set by the fullscreen watcher so config changes can't re-assert topmost over a game
    pub fullscreen_active: AtomicBool,
    // Dynamic Island state: who the window is, owned by the backend
    pub island_mode: Mutex<IslandMode>,
    pub island_prev_mode: Mutex<IslandMode>,
    pub island_pinned: AtomicBool, // settings/popover open: don't collapse on hover-out
    pub island_ignore: AtomicBool, // click-through active in compact mode
    pub has_media: AtomicBool,     // updated by the polling loop, read by the hover watcher
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IslandMode {
    Hidden,
    Compact,
    Activity,
    Expanded,
}

impl IslandMode {
    fn as_str(self) -> &'static str {
        match self {
            IslandMode::Hidden => "hidden",
            IslandMode::Compact => "compact",
            IslandMode::Activity => "activity",
            IslandMode::Expanded => "expanded",
        }
    }

    fn size(self) -> Option<(f64, f64)> {
        match self {
            IslandMode::Hidden => None,
            IslandMode::Compact => Some((240.0, 36.0)),
            IslandMode::Activity => Some((340.0, 56.0)),
            IslandMode::Expanded => Some((480.0, 188.0)),
        }
    }

    fn rank(self) -> u8 {
        match self {
            IslandMode::Hidden => 0,
            IslandMode::Compact => 1,
            IslandMode::Activity => 2,
            IslandMode::Expanded => 3,
        }
    }
}

#[derive(Clone, serde::Serialize)]
struct IslandModePayload {
    mode: &'static str,
    width: f64,
    height: f64,
}

// Per-app desired volume/mute (in-memory): lets us re-apply after a player recreates its audio session
static DESIRED_VOLUME: Mutex<Option<HashMap<String, (f32, bool)>>> = Mutex::new(None);

fn desired_volume_set(app_id: &str, level: f32, muted: bool) {
    if let Ok(mut g) = DESIRED_VOLUME.lock() {
        g.get_or_insert_with(HashMap::new)
            .insert(app_id.to_string(), (level, muted));
    }
}

fn desired_volume_get(app_id: &str) -> Option<(f32, bool)> {
    DESIRED_VOLUME
        .lock()
        .ok()
        .and_then(|g| g.as_ref().and_then(|m| m.get(app_id).copied()))
}

// ponytail: async keeps blocking WinRT .get() off the main thread (sync commands run there and deadlock)
#[tauri::command]
async fn get_media_state() -> MediaState {
    MediaController::get_state()
}

#[tauri::command]
async fn get_media_thumbnail() -> Option<String> {
    MediaController::get_media_thumbnail()
}

#[tauri::command]
async fn media_play_pause() -> Result<bool, String> {
    MediaController::toggle_play_pause()
}

#[tauri::command]
async fn media_next() -> Result<bool, String> {
    MediaController::skip_next()
}

#[tauri::command]
async fn media_previous() -> Result<bool, String> {
    MediaController::skip_previous()
}

#[tauri::command]
async fn media_seek(seconds: f64) -> Result<bool, String> {
    MediaController::seek_to(seconds)
}

#[tauri::command]
async fn get_audio_sources() -> Vec<media::AudioSourceItem> {
    media::MediaController::get_audio_sources()
}

#[tauri::command]
async fn select_audio_source(source_id: Option<String>) -> Result<(), String> {
    media::MediaController::select_source(source_id);
    Ok(())
}

fn app_volume_enabled(state: &tauri::State<'_, AppState>) -> bool {
    state.config.lock().unwrap_or_else(|e| e.into_inner()).app_volume
}

// ponytail: per-app volume first, system volume as fallback (session may not exist yet)
#[tauri::command]
async fn get_master_volume(state: tauri::State<'_, AppState>) -> Result<f32, String> {
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() {
            if let Ok(v) = audio_session::get_volume(&app_id) {
                return Ok(v);
            }
        }
    }
    volume::VolumeController::get_volume()
}

#[tauri::command]
async fn set_master_volume(level: f32, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if !level.is_finite() {
        return Err("Invalid volume".to_string());
    }
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() {
            let muted = desired_volume_get(&app_id).map(|(_, m)| m).unwrap_or(false);
            desired_volume_set(&app_id, level, muted);
            if audio_session::set_volume(&app_id, level).is_ok() {
                return Ok(());
            }
        }
    }
    volume::VolumeController::set_volume(level)
}

#[tauri::command]
async fn is_master_muted(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() {
            if let Ok(m) = audio_session::is_muted(&app_id) {
                return Ok(m);
            }
        }
    }
    volume::VolumeController::is_muted()
}

#[tauri::command]
async fn toggle_master_mute(state: tauri::State<'_, AppState>) -> Result<bool, String> {
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() {
            if let Ok(m) = audio_session::toggle_mute(&app_id) {
                let level = desired_volume_get(&app_id)
                    .map(|(l, _)| l)
                    .or_else(|| audio_session::get_volume(&app_id).ok())
                    .unwrap_or(1.0);
                desired_volume_set(&app_id, level, m);
                return Ok(m);
            }
        }
    }
    volume::VolumeController::toggle_mute()
}

// Explicit mute set (used when re-applying after a session re-created)
#[tauri::command]
async fn set_master_muted(muted: bool, state: tauri::State<'_, AppState>) -> Result<(), String> {
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() {
            let level = desired_volume_get(&app_id).map(|(l, _)| l).unwrap_or(1.0);
            desired_volume_set(&app_id, level, muted);
            if audio_session::set_muted(&app_id, muted).is_ok() {
                return Ok(());
            }
        }
    }
    volume::VolumeController::set_mute(muted)
}

// Re-apply the user's value for the current app after it recreates its audio session
#[tauri::command]
async fn reapply_app_volume(state: tauri::State<'_, AppState>) -> Result<(), String> {
    if !app_volume_enabled(&state) {
        return Ok(());
    }
    let app_id = MediaController::current_source_app_id();
    if app_id.is_empty() {
        return Ok(());
    }
    if let Some((level, muted)) = desired_volume_get(&app_id) {
        let _ = audio_session::set_volume(&app_id, level);
        let _ = audio_session::set_muted(&app_id, muted);
    }
    Ok(())
}

#[tauri::command]
fn start_drag(window: WebviewWindow, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let draggable = state.config.lock().unwrap_or_else(|e| e.into_inner()).draggable;
    if !draggable {
        return Err("Dragging disabled".to_string());
    }
    // The window is always the Dynamic Island; a drag only moves it temporarily (recenters on next show)
    window.start_dragging().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_config(state: tauri::State<'_, AppState>) -> AppConfig {
    state.config.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

#[tauri::command]
fn update_app_config(
    new_config: AppConfig,
    state: tauri::State<'_, AppState>,
    window: WebviewWindow,
    app: tauri::AppHandle,
) -> Result<(), String> {
    // Register the hotkey first: if another app owns it, nothing is persisted and the UI can revert
    if let Some(mgr) = state.hotkey_mgr.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        mgr.update_hotkey(new_config.hotkey_modifiers.clone(), new_config.hotkey_key.clone())?;
    }

    let island_was_enabled = state.config.lock().unwrap_or_else(|e| e.into_inner()).island_enabled;

    save_config(&new_config)?;

    // Apply new position
    let _ = apply_window_position(
        &window,
        &new_config.position_preset,
        new_config.custom_x,
        new_config.custom_y,
    );

    // Topmost is managed by the fullscreen watcher; re-assert it unless a game is fullscreen
    let fullscreen = state.fullscreen_active.load(Ordering::Relaxed);
    let _ = window.set_always_on_top(!fullscreen);

    *state.config.lock().unwrap_or_else(|e| e.into_inner()) = new_config.clone();
    sysstats::wake();

    // Switching the island on/off takes effect now, not on the next hover tick
    if new_config.island_enabled != island_was_enabled {
        if new_config.island_enabled {
            if !state.island_pinned.load(Ordering::Relaxed) {
                set_island_mode_internal(&app, IslandMode::Compact, false);
            }
        } else {
            set_island_mode_internal(&app, IslandMode::Expanded, true);
        }
    }
    Ok(())
}

#[tauri::command]
fn set_window_size(width: f64, height: f64, window: WebviewWindow) -> Result<(), String> {
    if !width.is_finite() || !height.is_finite() {
        return Err("Invalid window size".to_string());
    }
    let width = width.clamp(100.0, 4000.0);
    let height = height.clamp(100.0, 4000.0);
    window
        .set_size(Size::Logical(LogicalSize { width, height }))
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn hide_window(window: WebviewWindow) -> Result<(), String> {
    let res = window.hide().map_err(|e| e.to_string());
    sysstats::wake();
    res
}

#[tauri::command]
fn toggle_window(window: WebviewWindow) -> Result<(), String> {
    toggle_main_window(window.app_handle());
    Ok(())
}

// Shared show/hide used by the hotkey, the tray and the IPC command
fn toggle_main_window(app: &tauri::AppHandle) {
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    let state = app.state::<AppState>();
    let conf = state.config.lock().unwrap_or_else(|e| e.into_inner()).clone();

    if conf.island_enabled {
        let current = *state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
        let pinned = state.island_pinned.load(Ordering::Relaxed);
        if current == IslandMode::Expanded {
            if pinned {
                return;
            }
            let has_media = state.has_media.load(Ordering::Relaxed);
            let target = if conf.island_always_visible || has_media {
                IslandMode::Compact
            } else {
                IslandMode::Hidden
            };
            set_island_mode_internal(app, target, false);
        } else {
            *state.last_shown.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
            let fullscreen = state.fullscreen_active.load(Ordering::Relaxed);
            // Activating the overlay over an exclusive-fullscreen game minimizes it
            let no_activate = conf.respect_fullscreen && fullscreen;
            set_island_mode_internal(app, IslandMode::Expanded, !no_activate);
            if no_activate {
                let _ = win.set_always_on_top(false);
            }
        }
        let _ = app.emit("island-visibility", true);
        return;
    }

    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        *state.last_shown.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        let fullscreen = state.fullscreen_active.load(Ordering::Relaxed);
        // Activating the overlay over an exclusive-fullscreen game minimizes it
        let no_activate = conf.respect_fullscreen && fullscreen;

        let _ = apply_window_position(&win, &conf.position_preset, conf.custom_x, conf.custom_y);
        if no_activate {
            // Show without stealing focus and without topmost (Windows won't draw it over D3D)
            let _ = win.set_always_on_top(false);
            if let Ok(hwnd) = win.hwnd() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
        } else {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        }
        // Ensure the card isn't left in the island "out" state after being shown again
        let _ = app.emit("island-visibility", true);
    }
    sysstats::wake();
}

fn logs_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    let dir = base.join("WMO").join("logs");
    let _ = std::fs::create_dir_all(&dir);
    dir
}

// Logs live in %APPDATA%\WMO\logs, never in the CWD (which can be System32 at autostart)
fn write_panic_log(msg: &str) {
    use std::io::Write;

    let path = logs_dir().join("panic.log");
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let entry = format!("--- panic at {} ---\n{}\n", stamp, msg);

    // Cap the file at ~256 KB so it can't grow forever
    let too_big = std::fs::metadata(&path)
        .map(|m| m.len() > 262_144)
        .unwrap_or(false);
    let result = if too_big {
        std::fs::write(&path, &entry)
    } else {
        std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .and_then(|mut f| f.write_all(entry.as_bytes()))
    };
    let _ = result;
}

// Monitor under the cursor (physical rect) plus its effective DPI scale
fn monitor_at_cursor(cursor: POINT) -> Option<(MONITORINFO, f64)> {
    unsafe {
        let hmon = MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST);
        if hmon.0.is_null() {
            return None;
        }
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(hmon, &mut info).as_bool() {
            return None;
        }
        let mut dpi_x = 96u32;
        let mut dpi_y = 96u32;
        let _ = GetDpiForMonitor(hmon, MDT_EFFECTIVE_DPI, &mut dpi_x, &mut dpi_y);
        let scale = if dpi_x > 0 { dpi_x as f64 / 96.0 } else { 1.0 };
        Some((info, scale))
    }
}

fn place_island(win: &WebviewWindow, logical_width: f64) {
    if let Some(mon) = win.current_monitor().ok().flatten() {
        let p = mon.position();
        let s = mon.size();
        let rect = (p.x, p.y, s.width as i32, s.height as i32);
        let _ = apply_top_center_size(win, rect, mon.scale_factor(), logical_width);
    }
}

fn cursor_inside_window(win: &WebviewWindow, cursor: POINT, margin: i32) -> bool {
    match (win.outer_position(), win.outer_size()) {
        (Ok(pos), Ok(size)) => {
            cursor.x >= pos.x - margin
                && cursor.x <= pos.x + size.width as i32 + margin
                && cursor.y >= pos.y - margin
                && cursor.y <= pos.y + size.height as i32 + margin
        }
        _ => false,
    }
}

// Single point that changes island state: native size, position and frontend event.
fn set_island_mode_internal(app: &tauri::AppHandle, mode: IslandMode, activate: bool) {
    let state = app.state::<AppState>();
    let prev = {
        let mut current = state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
        if *current == mode {
            return;
        }
        let prev = *current;
        *current = mode;
        prev
    };
    let Some(win) = app.get_webview_window("main") else {
        return;
    };
    sysstats::wake();

    if mode == IslandMode::Hidden {
        if prev != IslandMode::Hidden {
            *state.island_prev_mode.lock().unwrap_or_else(|e| e.into_inner()) = prev;
        }
        let _ = win.set_ignore_cursor_events(false);
        state.island_ignore.store(false, Ordering::Relaxed);
        let _ = win.hide();
        let _ = app.emit(
            "island-mode",
            IslandModePayload {
                mode: mode.as_str(),
                width: 0.0,
                height: 0.0,
            },
        );
        return;
    }

    let Some((width, height)) = mode.size() else {
        return;
    };
    let shrink = prev.rank() > 0 && mode.rank() < prev.rank();

    if mode != IslandMode::Compact {
        let _ = win.set_ignore_cursor_events(false);
        state.island_ignore.store(false, Ordering::Relaxed);
    }

    if shrink {
        // Animate the CSS collapse first; the native window follows (no transparent gap)
        let _ = app.emit(
            "island-mode",
            IslandModePayload {
                mode: mode.as_str(),
                width,
                height,
            },
        );
        let app_delayed = app.clone();
        thread::spawn(move || {
            thread::sleep(Duration::from_millis(220));
            let state = app_delayed.state::<AppState>();
            if *state.island_mode.lock().unwrap_or_else(|e| e.into_inner()) != mode {
                return;
            }
            if let Some(win) = app_delayed.get_webview_window("main") {
                let _ = win.set_size(Size::Logical(LogicalSize { width, height }));
                place_island(&win, width);
            }
        });
    } else {
        // Grow the native window first, then let the CSS animate
        let _ = win.set_size(Size::Logical(LogicalSize { width, height }));
        place_island(&win, width);
        if activate {
            let _ = win.show();
            let _ = win.unminimize();
            let _ = win.set_focus();
        } else if !win.is_visible().unwrap_or(false) {
            // Appearing for a session change must never steal focus from a game/app
            if let Ok(hwnd) = win.hwnd() {
                unsafe {
                    let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                }
            }
        }
        let _ = app.emit(
            "island-mode",
            IslandModePayload {
                mode: mode.as_str(),
                width,
                height,
            },
        );
    }
}

#[tauri::command]
fn get_island_mode(state: tauri::State<'_, AppState>) -> IslandModePayload {
    let mode = *state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
    let (width, height) = mode.size().unwrap_or((0.0, 0.0));
    IslandModePayload {
        mode: mode.as_str(),
        width,
        height,
    }
}

#[tauri::command]
fn request_island_mode(mode: String, app: tauri::AppHandle) -> Result<(), String> {
    let mode = match mode.as_str() {
        "compact" => IslandMode::Compact,
        "activity" => IslandMode::Activity,
        "expanded" => IslandMode::Expanded,
        _ => return Err("Modo de isla no válido".to_string()),
    };
    // A late activity must not shrink an island the user already expanded
    let state = app.state::<AppState>();
    let current = *state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
    if mode == IslandMode::Activity && current == IslandMode::Expanded {
        return Ok(());
    }
    set_island_mode_internal(&app, mode, false);
    Ok(())
}

#[tauri::command]
fn set_island_pinned(pinned: bool, state: tauri::State<'_, AppState>) {
    state.island_pinned.store(pinned, Ordering::Relaxed);
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}\nLocation: {:?}\nBacktrace: {:?}", info, info.location(), std::backtrace::Backtrace::capture());
        write_panic_log(&msg);
        eprintln!("{}", msg);
    }));

    let initial_config = load_config();

    let app_state = AppState {
        config: Mutex::new(initial_config.clone()),
        hotkey_mgr: Mutex::new(None),
        last_shown: Mutex::new(Instant::now()),
        fullscreen_active: AtomicBool::new(false),
        island_mode: Mutex::new(IslandMode::Hidden),
        island_prev_mode: Mutex::new(IslandMode::Compact),
        island_pinned: AtomicBool::new(false),
        island_ignore: AtomicBool::new(false),
        has_media: AtomicBool::new(false),
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            get_media_state,
            get_media_thumbnail,
            media_play_pause,
            media_next,
            media_previous,
            media_seek,
            get_audio_sources,
            select_audio_source,
            get_master_volume,
            set_master_volume,
            is_master_muted,
            toggle_master_mute,
            set_master_muted,
            reapply_app_volume,
            start_drag,
            get_app_config,
            update_app_config,
            get_island_mode,
            request_island_mode,
            set_island_pinned,
            set_window_size,
            hide_window,
            toggle_window,
        ])
        .setup(move |app| {
            let window = app.get_webview_window("main").unwrap();

            // Initial window size: 480 x 188
            let _ = window.set_size(Size::Logical(LogicalSize {
                width: 480.0,
                height: 188.0,
            }));

            // Set initial position
            let _ = apply_window_position(
                &window,
                &initial_config.position_preset,
                initial_config.custom_x,
                initial_config.custom_y,
            );

            // Keep the overlay on top; it only steps aside while a fullscreen app is in front
            let _ = window.set_always_on_top(true);

            // Initial island state (the frontend also asks for it on startup)
            let initial_mode = if initial_config.island_enabled {
                if initial_config.island_always_visible {
                    IslandMode::Compact
                } else {
                    IslandMode::Hidden
                }
            } else {
                IslandMode::Expanded
            };
            if initial_mode == IslandMode::Hidden {
                let _ = window.hide();
            } else {
                set_island_mode_internal(app.handle(), initial_mode, false);
            }

            // Register global hotkey
            let app_handle = app.handle().clone();
            let initial_mods = initial_config.hotkey_modifiers.clone();
            let initial_key = initial_config.hotkey_key.clone();

            let hotkey_manager = Arc::new(HotkeyManager::new(initial_mods, initial_key, move || {
                toggle_main_window(&app_handle);
            }));

            let state = app.state::<AppState>();
            *state.hotkey_mgr.lock().unwrap_or_else(|e| e.into_inner()) = Some(hotkey_manager);

            // Tray icon: show/hide + a real "Salir" (the window only hides otherwise)
            {
                let toggle_item =
                    MenuItem::with_id(app, "toggle", "Mostrar / Ocultar", true, None::<&str>)?;
                let quit_item = MenuItem::with_id(app, "quit", "Salir", true, None::<&str>)?;
                let menu = Menu::with_items(app, &[&toggle_item, &quit_item])?;
                let mut tray = TrayIconBuilder::new()
                    .menu(&menu)
                    .show_menu_on_left_click(false)
                    .tooltip("WMO")
                    .on_menu_event(|app, event| match event.id.as_ref() {
                        "toggle" => toggle_main_window(app),
                        "quit" => app.exit(0),
                        _ => {}
                    })
                    .on_tray_icon_event(|tray, event| {
                        if let TrayIconEvent::Click {
                            button: MouseButton::Left,
                            button_state: MouseButtonState::Up,
                            ..
                        } = event
                        {
                            toggle_main_window(tray.app_handle());
                        }
                    });
                if let Some(icon) = app.handle().default_window_icon().cloned() {
                    tray = tray.icon(icon);
                }
                app.manage(tray.build(app)?);
            }

            // System stats sampler (sleeps while disabled or hidden)
            sysstats::spawn(app.handle().clone());

            // Background media polling loop
            let app_handle_polling = app.handle().clone();
            thread::spawn(move || {
                let mut last_title = String::new();
                let mut last_artist = String::new();
                let mut last_status = false;
                let mut last_source = String::new();
                let mut last_selected: Option<String> = None;

                loop {
                    let state = MediaController::get_state();
                    app_handle_polling
                        .state::<AppState>()
                        .has_media
                        .store(state.has_session, Ordering::Relaxed);
                    let selected = MediaController::selected_source_id();
                    let mut visible = true;

                    if let Some(win) = app_handle_polling.get_webview_window("main") {
                        let is_visible = win.is_visible().unwrap_or(true);
                        visible = is_visible;
                        // ponytail: also flag source/selection changes so switching a source forces a refresh
                        let changed = state.title != last_title
                            || state.artist != last_artist
                            || state.is_playing != last_status
                            || state.source_app_id != last_source
                            || selected != last_selected;

                        if is_visible || changed {
                            let _ = app_handle_polling.emit("media-state", &state);
                            last_title = state.title.clone();
                            last_artist = state.artist.clone();
                            last_status = state.is_playing;
                            last_source = state.source_app_id.clone();
                            last_selected = selected;
                        }
                    }

                    // Hidden window doesn't need a fast poll
                    thread::sleep(Duration::from_millis(if visible { 350 } else { 900 }));
                }
            });

            // Fullscreen-aware topmost + Dynamic Island hover-to-reveal (single cursor poll, no hooks)
            // ponytail: GetCursorPos polling, no global mouse hook (AV-safe, no deps)
            let app_handle_hover = app.handle().clone();
            thread::spawn(move || {
                let mut outside_since: Option<Instant> = None;
                let mut pending_hide: Option<Instant> = None;
                let mut hover_since: Option<Instant> = None;
                let mut fullscreen_prev = false;
                // Cursor touching the exact top edge of a monitor, within the island's width
                let in_reveal_zone = |mon: &MONITORINFO, scale: f64, cursor: POINT| -> bool {
                    let rect = mon.rcMonitor;
                    let cx = rect.left + (rect.right - rect.left) / 2;
                    let zone_w = (380.0 * scale) as i32;
                    // Only the exact edge (2px), where the cursor stops when pushed to the limit
                    let edge = (2.0 * scale).max(1.0) as i32;
                    cursor.x >= cx - zone_w / 2
                        && cursor.x <= cx + zone_w / 2
                        && cursor.y >= rect.top
                        && cursor.y <= rect.top + edge
                };
                loop {
                    let (
                        hover_enabled,
                        respect_fullscreen,
                        island_enabled,
                        island_always_visible,
                        click_through,
                        expand_delay,
                    ) = {
                        let state = app_handle_hover.state::<AppState>();
                        let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            conf.hover_reveal && conf.position_preset == "top-center",
                            conf.respect_fullscreen,
                            conf.island_enabled,
                            conf.island_always_visible,
                            conf.island_click_through_compact,
                            conf.hover_expand_delay_ms,
                        )
                    };
                    let fullscreen_active =
                        respect_fullscreen && fullscreen::exclusive_fullscreen_active();
                    // Share the state with update_app_config / the hotkey (other threads)
                    app_handle_hover
                        .state::<AppState>()
                        .fullscreen_active
                        .store(fullscreen_active, Ordering::Relaxed);

                    if let Some(win) = app_handle_hover.get_webview_window("main") {
                        // Only drop "always on top" while a fullscreen app is in front, so the
                        // overlay can't steal focus/minimize it. Restore it right after.
                        if fullscreen_active != fullscreen_prev {
                            let _ = win.set_always_on_top(!fullscreen_active);
                            fullscreen_prev = fullscreen_active;
                            outside_since = None;
                            pending_hide = None;
                            hover_since = None;
                        }

                        if island_enabled {
                            let state = app_handle_hover.state::<AppState>();
                            let mode = *state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
                            let has_media = state.has_media.load(Ordering::Relaxed);
                            let pinned = state.island_pinned.load(Ordering::Relaxed);

                            if fullscreen_active {
                                // The island can't draw over an exclusive-fullscreen game
                                if mode != IslandMode::Hidden {
                                    set_island_mode_internal(&app_handle_hover, IslandMode::Hidden, false);
                                }
                            } else {
                                match mode {
                                    IslandMode::Hidden => {
                                        if has_media || island_always_visible {
                                            let prev = *state
                                                .island_prev_mode
                                                .lock()
                                                .unwrap_or_else(|e| e.into_inner());
                                            let target = if prev == IslandMode::Expanded && !pinned {
                                                IslandMode::Expanded
                                            } else {
                                                IslandMode::Compact
                                            };
                                            set_island_mode_internal(&app_handle_hover, target, false);
                                        }
                                    }
                                    IslandMode::Compact => {
                                        if !has_media && !island_always_visible {
                                            set_island_mode_internal(
                                                &app_handle_hover,
                                                IslandMode::Hidden,
                                                false,
                                            );
                                        } else {
                                            let mut cursor = POINT::default();
                                            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                                                let inside = cursor_inside_window(&win, cursor, 2);
                                                let want_ignore = click_through && !inside;
                                                let ignore_now =
                                                    state.island_ignore.load(Ordering::Relaxed);
                                                if want_ignore != ignore_now {
                                                    let _ = win.set_ignore_cursor_events(want_ignore);
                                                    state.island_ignore.store(
                                                        want_ignore,
                                                        Ordering::Relaxed,
                                                    );
                                                }
                                                if inside {
                                                    match hover_since {
                                                        Some(t)
                                                            if t.elapsed()
                                                                >= Duration::from_millis(
                                                                    expand_delay as u64,
                                                                ) =>
                                                        {
                                                            hover_since = None;
                                                            set_island_mode_internal(
                                                                &app_handle_hover,
                                                                IslandMode::Expanded,
                                                                false,
                                                            );
                                                        }
                                                        Some(_) => {}
                                                        None => hover_since = Some(Instant::now()),
                                                    }
                                                } else {
                                                    hover_since = None;
                                                }
                                            }
                                        }
                                    }
                                    IslandMode::Activity => {
                                        // The frontend owns the 2.5 s activity timer
                                        hover_since = None;
                                    }
                                    IslandMode::Expanded => {
                                        hover_since = None;
                                        if !pinned {
                                            let mut cursor = POINT::default();
                                            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                                                if cursor_inside_window(&win, cursor, 14) {
                                                    outside_since = None;
                                                } else {
                                                    let now = Instant::now();
                                                    match outside_since {
                                                        Some(t)
                                                            if now.duration_since(t)
                                                                > Duration::from_millis(450) =>
                                                        {
                                                            outside_since = None;
                                                            let target =
                                                                if has_media || island_always_visible {
                                                                    IslandMode::Compact
                                                                } else {
                                                                    IslandMode::Hidden
                                                                };
                                                            set_island_mode_internal(
                                                                &app_handle_hover,
                                                                target,
                                                                false,
                                                            );
                                                        }
                                                        Some(_) => {}
                                                        None => outside_since = Some(now),
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else if hover_enabled && !fullscreen_active {
                            let mut cursor = POINT::default();
                            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                                // Follow the cursor: with several monitors the window may be elsewhere
                                let mon = monitor_at_cursor(cursor);
                                let in_zone = |c: POINT| match &mon {
                                    Some((info, scale)) => in_reveal_zone(info, *scale, c),
                                    None => false,
                                };
                                let visible = win.is_visible().unwrap_or(false);
                                if !visible {
                                    if let Some((info, scale)) = &mon {
                                        if in_reveal_zone(info, *scale, cursor) {
                                            let rect = info.rcMonitor;
                                            let _ = apply_top_center_on_monitor(
                                                &win,
                                                (
                                                    rect.left,
                                                    rect.top,
                                                    rect.right - rect.left,
                                                    rect.bottom - rect.top,
                                                ),
                                                *scale,
                                            );
                                            let _ = win.show();
                                            let _ = app_handle_hover.emit("island-visibility", true);
                                            sysstats::wake();
                                            outside_since = None;
                                            pending_hide = None;
                                        }
                                    }
                                } else if let Some(t) = pending_hide {
                                    // Animating out: finish hiding once the CSS transition has run
                                    if t.elapsed() > Duration::from_millis(240) {
                                        let _ = win.hide();
                                        pending_hide = None;
                                        sysstats::wake();
                                    }
                                    // Re-enter (window box or the top edge) cancels the hide
                                    let margin = (14.0 * win.scale_factor().unwrap_or(1.0)) as i32;
                                    let inside = in_zone(cursor)
                                        || match (win.outer_position(), win.outer_size()) {
                                            (Ok(pos), Ok(size)) => {
                                                cursor.x >= pos.x - margin
                                                    && cursor.x <= pos.x + size.width as i32 + margin
                                                    && cursor.y >= pos.y - margin
                                                    && cursor.y <= pos.y + size.height as i32 + margin
                                            }
                                            _ => true,
                                        };
                                    if inside {
                                        pending_hide = None;
                                        outside_since = None;
                                        let _ = app_handle_hover.emit("island-visibility", true);
                                    }
                                } else if !win.is_focused().unwrap_or(false) {
                                    let margin = (14.0 * win.scale_factor().unwrap_or(1.0)) as i32;
                                    let inside = in_zone(cursor)
                                        || match (win.outer_position(), win.outer_size()) {
                                            (Ok(pos), Ok(size)) => {
                                                cursor.x >= pos.x - margin
                                                    && cursor.x <= pos.x + size.width as i32 + margin
                                                    && cursor.y >= pos.y - margin
                                                    && cursor.y <= pos.y + size.height as i32 + margin
                                            }
                                            _ => true,
                                        };
                                    if inside {
                                        outside_since = None;
                                    } else {
                                        let now = Instant::now();
                                        match outside_since {
                                            Some(t) if now.duration_since(t) > Duration::from_millis(450) => {
                                                let _ = app_handle_hover.emit("island-visibility", false);
                                                pending_hide = Some(now);
                                            }
                                            Some(_) => {}
                                            None => outside_since = Some(now),
                                        }
                                    }
                                } else {
                                    outside_since = None;
                                }
                            }
                        }
                    }

                    thread::sleep(Duration::from_millis(120));
                }
            });

            // Handle native window events: movement tracking and safe auto-hide
            let app_handle_events = app.handle().clone();
            let win_for_event = window.clone();
            window.on_window_event(move |event| match event {
                WindowEvent::Focused(focused) => {
                    if !focused {
                        let state = app_handle_events.state::<AppState>();
                        let (auto_hide, island_enabled) = {
                            let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                            (conf.auto_hide_on_blur, conf.island_enabled)
                        };
                        let last_shown_time = *state.last_shown.lock().unwrap_or_else(|e| e.into_inner());

                        // Island mode collapses on hover-out instead of hiding on blur
                        if auto_hide && !island_enabled && last_shown_time.elapsed() > Duration::from_millis(350) {
                            let _ = win_for_event.hide();
                            sysstats::wake();
                        }
                    }
                }
                WindowEvent::Moved(_) => {
                    // The island is always top-center: a drag just moves it temporarily and it
                    // recenters on the current monitor on the next show/reveal. Nothing to persist.
                }
                _ => {}
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
