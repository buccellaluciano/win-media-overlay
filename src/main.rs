// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_session;
mod config;
mod fullscreen;
mod hotkey;
mod media;
mod position;
mod volume;

use config::{load_config, save_config, AppConfig};
use hotkey::HotkeyManager;
use media::{MediaController, MediaState};
use position::apply_window_position;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{Emitter, LogicalSize, Manager, Size, WebviewWindow, WindowEvent};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{keybd_event, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

pub struct AppState {
    pub config: Mutex<AppConfig>,
    pub hotkey_mgr: Mutex<Option<Arc<HotkeyManager>>>,
    pub last_shown: Mutex<Instant>,
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
fn adjust_volume(delta: i32) -> Result<(), String> {
    let vk = if delta > 0 { 0xAFu8 } else { 0xAEu8 }; // VK_VOLUME_UP = 0xAF, VK_VOLUME_DOWN = 0xAE
    unsafe {
        keybd_event(vk, 0, KEYBD_EVENT_FLAGS(0), 0);
        keybd_event(vk, 0, KEYEVENTF_KEYUP, 0);
    }
    Ok(())
}

#[tauri::command]
async fn get_audio_sources() -> Vec<media::AudioSourceItem> {
    media::MediaController::get_audio_sources()
}

#[tauri::command]
fn select_audio_source(source_id: Option<String>) -> Result<(), String> {
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
) -> Result<(), String> {
    save_config(&new_config)?;

    // Update hotkey
    if let Some(mgr) = state.hotkey_mgr.lock().unwrap_or_else(|e| e.into_inner()).as_ref() {
        mgr.update_hotkey(new_config.hotkey_modifiers.clone(), new_config.hotkey_key.clone());
    }

    // Apply new position
    let _ = apply_window_position(
        &window,
        &new_config.position_preset,
        new_config.custom_x,
        new_config.custom_y,
    );

    // Topmost is managed by the fullscreen watcher; re-assert it on config change
    let _ = window.set_always_on_top(true);

    *state.config.lock().unwrap_or_else(|e| e.into_inner()) = new_config;
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
    window.hide().map_err(|e| e.to_string())
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
    if win.is_visible().unwrap_or(false) {
        let _ = win.hide();
    } else {
        let state = app.state::<AppState>();
        *state.last_shown.lock().unwrap_or_else(|e| e.into_inner()) = Instant::now();
        let conf = state.config.lock().unwrap_or_else(|e| e.into_inner()).clone();
        let _ = apply_window_position(&win, &conf.position_preset, conf.custom_x, conf.custom_y);
        let _ = win.show();
        let _ = win.unminimize();
        let _ = win.set_focus();
        // Ensure the card isn't left in the island "out" state after being shown again
        let _ = app.emit("island-visibility", true);
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let msg = format!("PANIC: {}\nLocation: {:?}\nBacktrace: {:?}", info, info.location(), std::backtrace::Backtrace::capture());
        let _ = std::fs::write("panic.log", &msg);
        eprintln!("{}", msg);
    }));

    let initial_config = load_config();

    let app_state = AppState {
        config: Mutex::new(initial_config.clone()),
        hotkey_mgr: Mutex::new(None),
        last_shown: Mutex::new(Instant::now()),
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
            adjust_volume,
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
                let mut fullscreen_prev = false;
                // Cursor touching the exact top edge of a monitor, within the island's width
                let in_reveal_zone = |win: &WebviewWindow, cursor: POINT| -> bool {
                    let mon = win
                        .current_monitor()
                        .ok()
                        .flatten()
                        .or_else(|| win.primary_monitor().ok().flatten());
                    let Some(mon) = mon else {
                        return false;
                    };
                    let sf = mon.scale_factor();
                    let mp = mon.position();
                    let ms = mon.size();
                    let cx = mp.x + ms.width as i32 / 2;
                    let zone_w = (380.0 * sf) as i32;
                    // Only the exact edge (2px), where the cursor stops when pushed to the limit
                    let edge = (2.0 * sf).max(1.0) as i32;
                    cursor.x >= cx - zone_w / 2
                        && cursor.x <= cx + zone_w / 2
                        && cursor.y >= mp.y
                        && cursor.y <= mp.y + edge
                };
                loop {
                    let (hover_enabled, respect_fullscreen) = {
                        let state = app_handle_hover.state::<AppState>();
                        let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                        (
                            conf.hover_reveal && conf.position_preset == "top-center",
                            conf.respect_fullscreen,
                        )
                    };
                    let fullscreen_active =
                        respect_fullscreen && fullscreen::exclusive_fullscreen_active();

                    if let Some(win) = app_handle_hover.get_webview_window("main") {
                        // Only drop "always on top" while a fullscreen app is in front, so the
                        // overlay can't steal focus/minimize it. Restore it right after.
                        if fullscreen_active != fullscreen_prev {
                            let _ = win.set_always_on_top(!fullscreen_active);
                            fullscreen_prev = fullscreen_active;
                            outside_since = None;
                            pending_hide = None;
                        }

                        if hover_enabled && !fullscreen_active {
                            let mut cursor = POINT::default();
                            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                                let visible = win.is_visible().unwrap_or(false);
                                if !visible {
                                    if in_reveal_zone(&win, cursor) {
                                        let _ = apply_window_position(&win, "top-center", 0, 0);
                                        let _ = win.show();
                                        let _ = app_handle_hover.emit("island-visibility", true);
                                        outside_since = None;
                                        pending_hide = None;
                                    }
                                } else if let Some(t) = pending_hide {
                                    // Animating out: finish hiding once the CSS transition has run
                                    if t.elapsed() > Duration::from_millis(240) {
                                        let _ = win.hide();
                                        pending_hide = None;
                                    }
                                    // Re-enter (window box or the top edge) cancels the hide
                                    let margin = (14.0 * win.scale_factor().unwrap_or(1.0)) as i32;
                                    let inside = in_reveal_zone(&win, cursor)
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
                                    let inside = in_reveal_zone(&win, cursor)
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
                        let auto_hide = state.config.lock().unwrap_or_else(|e| e.into_inner()).auto_hide_on_blur;
                        let last_shown_time = *state.last_shown.lock().unwrap_or_else(|e| e.into_inner());

                        // Guard against premature auto-hide: ignore blur within 350ms of appearing
                        if auto_hide && last_shown_time.elapsed() > Duration::from_millis(350) {
                            let _ = win_for_event.hide();
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
