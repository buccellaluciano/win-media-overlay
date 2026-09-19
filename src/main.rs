// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio_session;
mod config;
mod hotkey;
mod media;
mod position;
mod volume;

use config::{load_config, save_config, AppConfig};
use hotkey::HotkeyManager;
use media::{MediaController, MediaState};
use position::apply_window_position;

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

// ponytail: coarse debounce for drag-save, per-pixel persist if drag storms matter
static LAST_POS_SAVE: Mutex<Option<Instant>> = Mutex::new(None);
// While set and in the future, Moved events are treated as user drags (not programmatic placement)
static DRAG_UNTIL: Mutex<Option<Instant>> = Mutex::new(None);

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
    if app_volume_enabled(&state) {
        let app_id = MediaController::current_source_app_id();
        if !app_id.is_empty() && audio_session::set_volume(&app_id, level).is_ok() {
            return Ok(());
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
                return Ok(m);
            }
        }
    }
    volume::VolumeController::toggle_mute()
}

#[tauri::command]
fn start_drag(window: WebviewWindow, state: tauri::State<'_, AppState>) -> Result<(), String> {
    let draggable = state.config.lock().unwrap_or_else(|e| e.into_inner()).draggable;
    if !draggable {
        return Err("Dragging disabled".to_string());
    }
    // Mark the window as user-dragging for a short window so Moved can tell drags from programmatic moves
    if let Ok(mut g) = DRAG_UNTIL.lock() {
        *g = Some(Instant::now() + Duration::from_millis(400));
    }
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

    *state.config.lock().unwrap_or_else(|e| e.into_inner()) = new_config;
    Ok(())
}

#[tauri::command]
fn set_window_size(width: f64, height: f64, window: WebviewWindow) -> Result<(), String> {
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

            // Dynamic Island hover-to-reveal: hidden at top-center, shows when the cursor hits the top strip
            // ponytail: GetCursorPos polling, no global mouse hook (AV-safe, no deps)
            let app_handle_hover = app.handle().clone();
            thread::spawn(move || {
                let mut outside_since: Option<Instant> = None;
                let mut pending_hide: Option<Instant> = None;
                loop {
                    let enabled = {
                        let state = app_handle_hover.state::<AppState>();
                        let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                        conf.hover_reveal && conf.position_preset == "top-center"
                    };

                    if enabled {
                        if let Some(win) = app_handle_hover.get_webview_window("main") {
                            let mut cursor = POINT::default();
                            if unsafe { GetCursorPos(&mut cursor) }.is_ok() {
                                let visible = win.is_visible().unwrap_or(false);
                                if !visible {
                                    let mon = win
                                        .current_monitor()
                                        .ok()
                                        .flatten()
                                        .or_else(|| win.primary_monitor().ok().flatten());
                                    if let Some(mon) = mon {
                                        let sf = mon.scale_factor();
                                        let mp = mon.position();
                                        let ms = mon.size();
                                        let cx = mp.x + ms.width as i32 / 2;
                                        let zone_w = (380.0 * sf) as i32;
                                        let zone_h = (18.0 * sf) as i32;
                                        if cursor.x >= cx - zone_w / 2
                                            && cursor.x <= cx + zone_w / 2
                                            && cursor.y >= mp.y
                                            && cursor.y <= mp.y + zone_h
                                        {
                                            let _ = apply_window_position(&win, "top-center", 0, 0);
                                            let _ = win.show();
                                            let _ = app_handle_hover.emit("island-visibility", true);
                                            outside_since = None;
                                            pending_hide = None;
                                        }
                                    }
                                } else if let Some(t) = pending_hide {
                                    // Animating out: finish hiding once the CSS transition has run
                                    if t.elapsed() > Duration::from_millis(240) {
                                        let _ = win.hide();
                                        pending_hide = None;
                                    }
                                    // Re-enter cancels the hide and animates back in
                                    let margin = (14.0 * win.scale_factor().unwrap_or(1.0)) as i32;
                                    let inside = match (win.outer_position(), win.outer_size()) {
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
                                    let inside = match (win.outer_position(), win.outer_size()) {
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
                WindowEvent::Moved(physical_pos) => {
                    let state = app_handle_events.state::<AppState>();
                    let now = Instant::now();
                    let user_drag = {
                        let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                        let draggable = conf.draggable;
                        let active = DRAG_UNTIL
                            .lock()
                            .ok()
                            .and_then(|g| *g)
                            .map_or(false, |t| now < t);
                        draggable && active
                    };
                    if user_drag {
                        if let Ok(mut g) = DRAG_UNTIL.lock() {
                            *g = Some(now + Duration::from_millis(400));
                        }
                        let mut conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
                        conf.custom_x = physical_pos.x;
                        conf.custom_y = physical_pos.y;
                        // Dragging switches to the free position so it persists across show/hide
                        conf.position_preset = "custom".to_string();
                        let due = LAST_POS_SAVE
                            .lock()
                            .ok()
                            .map(|g| g.map_or(true, |t| now.duration_since(t) > Duration::from_millis(800)))
                            .unwrap_or(true);
                        if due {
                            if let Ok(mut g) = LAST_POS_SAVE.lock() {
                                *g = Some(now);
                            }
                            let _ = save_config(&conf);
                        }
                    }
                }
                _ => {}
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
