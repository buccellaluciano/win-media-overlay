// Docked browser video: the user drops the browser's Picture-in-Picture window on the card and
// WMO moves/resizes that real window into a slot of the card (the browser keeps drawing it, so
// it costs nothing and DRM works). Only public user-mode window APIs: EnumWindows enumerates
// windows (not processes), no hooks, no OpenProcess, no input simulation.
//
// ponytail: every Win32 call here is either a read of window state or an async request
// (SWP_ASYNCWINDOWPOS / ShowWindowAsync), so a hung browser can never block WMO. Geometry of our
// own window is read with Win32 too (not Tauri getters, which hop to the main thread): sync()
// runs on the main thread from WindowEvent::Moved and must never wait for it.

use crate::{AppState, IslandMode};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::{Emitter, Manager};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::UI::HiDpi::GetDpiForWindow;
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetClassNameW, GetCursorPos, GetGUIThreadInfo, GetWindow, GetWindowLongPtrW,
    GetWindowRect, GetWindowTextW, GetWindowThreadProcessId, IsIconic, IsWindow, IsWindowVisible,
    SetWindowPos, ShowWindowAsync, GUITHREADINFO, GWL_EXSTYLE, GW_HWNDPREV, HWND_TOPMOST,
    SWP_ASYNCWINDOWPOS, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE,
    SW_SHOWNOACTIVATE, WS_EX_TOPMOST,
};

const PIP_CLASSES: [&str; 3] = ["Chrome_WidgetWin_1", "MozillaDialogClass", "MozillaWindowClass"];
const PIP_TITLES: [&str; 5] = [
    "picture in picture",
    "picture-in-picture",
    "imagen en imagen",
    "imagen con imagen",
    "pantalla en pantalla",
];
// Floating size while the island is collapsed ("float"), logical px
const FLOAT_W: f64 = 320.0;
const FLOAT_H: f64 = 180.0;

#[derive(Clone, Copy, Default)]
struct Slot {
    x: f64,
    y: f64,
    w: f64,
    h: f64,
}

#[derive(Default)]
struct Pip {
    // Docked window (HWND isn't Send, so it's kept as isize; 0 = nothing docked)
    hwnd: isize,
    orig: RECT,
    slot: Option<Slot>,
    hidden_by_us: bool,
    // Last rect we asked for and when: tells our own moves apart from the user's
    target: Option<RECT>,
    target_at: Option<Instant>,
    // The user is dragging the docked window: stop forcing its position meanwhile
    user_moving: bool,
    still_ticks: u32,
    last_rect: RECT,
    // Real size the browser kept (its PiP minimum is bigger than what we ask for), physical px
    reported_size: Option<(i32, i32)>,
    // Undocked candidate being dragged towards the card
    cand: isize,
    cand_rect: RECT,
    cand_moving: bool,
    cand_still: u32,
    hover: bool,
}

static STATE: Mutex<Option<Pip>> = Mutex::new(None);
static WAKE: Mutex<Option<Sender<()>>> = Mutex::new(None);
static MAIN_HWND: Mutex<isize> = Mutex::new(0);

#[derive(Clone, serde::Serialize)]
struct DockedPayload {
    docked: bool,
}

#[derive(Clone, serde::Serialize)]
struct HoverPayload {
    over: bool,
}

// Size the docked window really has, CSS px
#[derive(Clone, serde::Serialize)]
struct SizePayload {
    width: f64,
    height: f64,
}

enum Event {
    Docked(bool),
    Hover(bool),
    Size(f64, f64),
    Expand,
}

fn h(v: isize) -> HWND {
    HWND(v as *mut core::ffi::c_void)
}

fn with_state<R>(f: impl FnOnce(&mut Pip) -> R) -> R {
    let mut g = STATE.lock().unwrap_or_else(|e| e.into_inner());
    f(g.get_or_insert_with(Pip::default))
}

pub fn wake() {
    if let Ok(g) = WAKE.lock() {
        if let Some(tx) = g.as_ref() {
            let _ = tx.send(());
        }
    }
}

pub fn spawn(app: tauri::AppHandle) {
    if let Some(win) = app.get_webview_window("main") {
        if let Ok(hwnd) = win.hwnd() {
            *MAIN_HWND.lock().unwrap_or_else(|e| e.into_inner()) = hwnd.0 as isize;
        }
    }
    let (tx, rx) = channel::<()>();
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    std::thread::spawn(move || run(app, rx));
}

// Everything sync() needs, read without holding STATE
struct Ctx {
    enabled: bool,
    float: bool,
    art: bool,
    mode: IslandMode,
    visible: bool,
    origin: POINT, // client origin of the main window, physical px
    scale: f64,
    hit: Option<(f64, f64, f64, f64)>, // card box, CSS px
    main: isize,
}

fn read_ctx(app: &tauri::AppHandle) -> Ctx {
    let state = app.state::<AppState>();
    let (enabled, float, art) = {
        let c = state.config.lock().unwrap_or_else(|e| e.into_inner());
        (c.pip_dock_enabled, c.pip_island == "float", c.pip_layout == "art")
    };
    let mode = *state.island_mode.lock().unwrap_or_else(|e| e.into_inner());
    let hit = state
        .hit_rect
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .map(|r| (r.x, r.y, r.w, r.h));
    let main = *MAIN_HWND.lock().unwrap_or_else(|e| e.into_inner());
    let mut origin = POINT::default();
    let (mut visible, mut scale) = (false, 1.0);
    if main != 0 {
        unsafe {
            visible = IsWindowVisible(h(main)).as_bool() && !IsIconic(h(main)).as_bool();
            let _ = ClientToScreen(h(main), &mut origin);
            let dpi = GetDpiForWindow(h(main));
            if dpi > 0 {
                scale = dpi as f64 / 96.0;
            }
        }
    }
    Ctx { enabled, float, art, mode, visible, origin, scale, hit, main }
}

impl Ctx {
    fn to_screen(&self, x: f64, y: f64, w: f64, hgt: f64) -> RECT {
        let left = self.origin.x + (x * self.scale).round() as i32;
        let top = self.origin.y + (y * self.scale).round() as i32;
        RECT {
            left,
            top,
            right: left + (w * self.scale).round() as i32,
            bottom: top + (hgt * self.scale).round() as i32,
        }
    }

    fn cursor_on_card(&self) -> bool {
        let Some((x, y, w, hgt)) = self.hit else {
            return false;
        };
        if !self.visible || self.mode == IslandMode::Hidden {
            return false;
        }
        let r = self.to_screen(x, y, w, hgt);
        let mut c = POINT::default();
        if unsafe { GetCursorPos(&mut c) }.is_err() {
            return false;
        }
        c.x >= r.left && c.x <= r.right && c.y >= r.top && c.y <= r.bottom
    }
}

fn window_rect(hwnd: isize) -> Option<RECT> {
    let mut r = RECT::default();
    unsafe { GetWindowRect(h(hwnd), &mut r) }.ok().map(|_| r)
}

fn rect_eq(a: RECT, b: RECT, tol: i32) -> bool {
    (a.left - b.left).abs() <= tol
        && (a.top - b.top).abs() <= tol
        && (a.right - b.right).abs() <= tol
        && (a.bottom - b.bottom).abs() <= tol
}

// The browser is in its own move/size loop on this window (the user is dragging it)
fn in_move_size(hwnd: isize) -> bool {
    unsafe {
        let tid = GetWindowThreadProcessId(h(hwnd), None);
        if tid == 0 {
            return false;
        }
        let mut info = GUITHREADINFO {
            cbSize: std::mem::size_of::<GUITHREADINFO>() as u32,
            ..Default::default()
        };
        if GetGUIThreadInfo(tid, &mut info).is_err() {
            return false;
        }
        info.flags.0 & windows::Win32::UI::WindowsAndMessaging::GUI_INMOVESIZE.0 != 0
            && (info.hwndMoveSize.0.is_null() || info.hwndMoveSize.0 as isize == hwnd)
    }
}

fn wide_to_lower(buf: &[u16], len: i32) -> String {
    String::from_utf16_lossy(&buf[..len.max(0) as usize]).to_lowercase()
}

fn is_pip_window(hwnd: HWND) -> bool {
    unsafe {
        if !IsWindowVisible(hwnd).as_bool() {
            return false;
        }
        let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE) as u32;
        if ex & WS_EX_TOPMOST.0 == 0 {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        if pid == 0 || pid == std::process::id() {
            return false;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        if !PIP_CLASSES.contains(&class.as_str()) {
            return false;
        }
        let mut title = [0u16; 128];
        let n = GetWindowTextW(hwnd, &mut title);
        let title = wide_to_lower(&title, n);
        PIP_TITLES.iter().any(|t| title.contains(t))
    }
}

unsafe extern "system" fn enum_cb(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let out = unsafe { &mut *(lparam.0 as *mut Vec<isize>) };
    if is_pip_window(hwnd) {
        out.push(hwnd.0 as isize);
    }
    BOOL(1)
}

fn find_candidates() -> Vec<isize> {
    let mut out: Vec<isize> = Vec::new();
    unsafe {
        let _ = EnumWindows(Some(enum_cb), LPARAM(&mut out as *mut Vec<isize> as isize));
    }
    out
}

fn show(st: &mut Pip) {
    if st.hidden_by_us {
        unsafe {
            let _ = ShowWindowAsync(h(st.hwnd), SW_SHOWNOACTIVATE);
        }
        st.hidden_by_us = false;
    }
}

fn hide(st: &mut Pip) {
    if !st.hidden_by_us {
        unsafe {
            let _ = ShowWindowAsync(h(st.hwnd), SW_HIDE);
        }
        st.hidden_by_us = true;
        st.target = None;
    }
}

fn place(st: &mut Pip, r: RECT, main: isize) {
    let unchanged_target = st.target.map(|t| rect_eq(t, r, 0)).unwrap_or(false);
    let cur = window_rect(st.hwnd);
    // The browser may clamp the size (Chrome's PiP minimum): only the position must match then,
    // or we'd re-send the same request every tick
    let in_place = match cur {
        Some(c) => {
            if unchanged_target {
                (c.left - r.left).abs() <= 1 && (c.top - r.top).abs() <= 1
            } else {
                rect_eq(c, r, 1)
            }
        }
        None => false,
    };
    if !in_place {
        unsafe {
            let _ = SetWindowPos(
                h(st.hwnd),
                Some(HWND_TOPMOST),
                r.left,
                r.top,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
        }
        st.target = Some(r);
        st.target_at = Some(Instant::now());
    }
    show(st);
    keep_above(st.hwnd, main);
}

// Both windows are topmost: if ours got above the video (focus, show), lift the video again
fn keep_above(pip: isize, main: isize) {
    if main == 0 {
        return;
    }
    unsafe {
        let mut cur = h(pip);
        for _ in 0..64 {
            match GetWindow(cur, GW_HWNDPREV) {
                Ok(prev) => {
                    if prev.0 as isize == main {
                        let _ = SetWindowPos(
                            h(pip),
                            Some(HWND_TOPMOST),
                            0,
                            0,
                            0,
                            0,
                            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                        );
                        return;
                    }
                    cur = prev;
                }
                Err(_) => return,
            }
        }
    }
}

// Put the docked window back as the user had it (size and position) and forget it
fn restore(st: &mut Pip) {
    if st.hwnd == 0 {
        return;
    }
    if unsafe { IsWindow(Some(h(st.hwnd))) }.as_bool() {
        let o = st.orig;
        unsafe {
            let _ = SetWindowPos(
                h(st.hwnd),
                None,
                o.left,
                o.top,
                o.right - o.left,
                o.bottom - o.top,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
            let _ = ShowWindowAsync(h(st.hwnd), SW_SHOWNOACTIVATE);
        }
    }
    clear_dock(st);
}

fn clear_dock(st: &mut Pip) {
    st.hwnd = 0;
    st.hidden_by_us = false;
    st.target = None;
    st.target_at = None;
    st.user_moving = false;
    st.still_ticks = 0;
    st.reported_size = None;
}

// Apply the docked window's place for the current mode. Cheap; safe from any thread.
fn apply(ctx: &Ctx, st: &mut Pip) -> Option<Event> {
    if st.hwnd == 0 {
        return None;
    }
    if !unsafe { IsWindow(Some(h(st.hwnd))) }.as_bool() {
        // Closed from the browser
        clear_dock(st);
        return Some(Event::Docked(false));
    }
    if st.user_moving {
        return None;
    }
    let expanded = ctx.visible && ctx.mode == IslandMode::Expanded;
    let collapsed =
        ctx.visible && matches!(ctx.mode, IslandMode::Compact | IslandMode::Activity);
    if expanded {
        match st.slot {
            Some(s) => {
                let r = ctx.to_screen(s.x, s.y, s.w, s.h);
                place(st, r, ctx.main);
            }
            // Settings/popover over the player, or mid-morph: the video would cover them
            None => hide(st),
        }
    } else if ctx.float && collapsed {
        // Fixed island geometry (style.css: 6 px top margin, 36 px compact / 56 px activity,
        // centered), not the reported card box: right after collapsing that box is still the
        // expanded player's until the CSS morph ends, and the video flashed mid-screen
        let pill_h = if ctx.mode == IslandMode::Activity { 56.0 } else { 36.0 };
        let (cx, below) = (crate::WINDOW_W / 2.0, 6.0 + pill_h + 8.0);
        let r = ctx.to_screen(cx - FLOAT_W / 2.0, below, FLOAT_W, FLOAT_H);
        place(st, r, ctx.main);
    } else if ctx.float {
        // Main window hidden: the floating video stays where it is
        show(st);
    } else {
        hide(st);
    }
    None
}

fn emit(app: &tauri::AppHandle, ev: Option<Event>) {
    match ev {
        Some(Event::Docked(d)) => {
            let _ = app.emit("pip-docked", DockedPayload { docked: d });
        }
        Some(Event::Hover(o)) => {
            let _ = app.emit("pip-drop-hover", HoverPayload { over: o });
        }
        Some(Event::Size(w, hgt)) => {
            let _ = app.emit("pip-size", SizePayload { width: w, height: hgt });
        }
        Some(Event::Expand) => crate::set_island_mode_internal(app, IslandMode::Expanded, false),
        None => {}
    }
}

// Re-apply the docked window's position now (window moved, mode or slot changed)
pub fn sync(app: &tauri::AppHandle) {
    wake();
    let ctx = read_ctx(app);
    let ev = with_state(|st| apply(&ctx, st));
    emit(app, ev);
}

// Restore the docked window (option turned off, quitting)
pub fn release(app: &tauri::AppHandle) {
    let was = with_state(|st| {
        let was = st.hwnd != 0;
        restore(st);
        was
    });
    if was {
        emit(app, Some(Event::Docked(false)));
    }
}

fn dock(st: &mut Pip, hwnd: isize) {
    if st.hwnd != 0 && st.hwnd != hwnd {
        restore(st); // a new video replaces the old one
    }
    st.hwnd = hwnd;
    st.orig = window_rect(hwnd).unwrap_or_default();
    st.hidden_by_us = false;
    st.target = None;
    st.target_at = None;
    st.user_moving = false;
    st.still_ticks = 0;
    st.reported_size = None;
}

// One 100 ms step. Returns events to emit and whether something needs fast ticks.
fn tick(ctx: &Ctx, st: &mut Pip) -> (Vec<Event>, bool) {
    let mut events = Vec::new();
    let mut busy = false;

    // Docked window: is the user dragging it away?
    if st.hwnd != 0 && unsafe { IsWindow(Some(h(st.hwnd))) }.as_bool() && !st.hidden_by_us {
        busy = true;
        let cur = window_rect(st.hwnd).unwrap_or_default();
        let ours_settled = st
            .target_at
            .map(|t| t.elapsed() > Duration::from_millis(400))
            .unwrap_or(true);
        let moved_by_user = in_move_size(st.hwnd)
            || (ours_settled
                && st
                    .target
                    .map(|t| (cur.left - t.left).abs() > 2 || (cur.top - t.top).abs() > 2)
                    .unwrap_or(false));
        if st.user_moving {
            let still = rect_eq(cur, st.last_rect, 0) && !in_move_size(st.hwnd);
            st.still_ticks = if still { st.still_ticks + 1 } else { 0 };
            if st.still_ticks >= 2 {
                st.user_moving = false;
                st.still_ticks = 0;
                if ctx.cursor_on_card() {
                    st.target = None; // snap back into the slot
                } else {
                    // Undocked where it was dropped, with its original size back
                    let (w, hgt) = (st.orig.right - st.orig.left, st.orig.bottom - st.orig.top);
                    unsafe {
                        let _ = SetWindowPos(
                            h(st.hwnd),
                            None,
                            cur.left,
                            cur.top,
                            w,
                            hgt,
                            SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
                        );
                    }
                    clear_dock(st);
                    events.push(Event::Docked(false));
                }
            }
        } else if moved_by_user {
            st.user_moving = true;
            st.still_ticks = 0;
        } else if ours_settled && ctx.art && ctx.visible && ctx.mode == IslandMode::Expanded {
            // The art slot follows the size the browser really kept (its PiP minimum): tell
            // the UI once, it resizes the slot and the next target matches the window
            if let Some(t) = st.target {
                let (tw, th) = (t.right - t.left, t.bottom - t.top);
                let (cw, ch) = (cur.right - cur.left, cur.bottom - cur.top);
                let clamped = (cw - tw).abs() > 2 || (ch - th).abs() > 2;
                if clamped && cw > 0 && ch > 0 && st.reported_size != Some((cw, ch)) {
                    st.reported_size = Some((cw, ch));
                    events.push(Event::Size(cw as f64 / ctx.scale, ch as f64 / ctx.scale));
                }
            }
        }
        st.last_rect = cur;
    }

    if let Some(ev) = apply(ctx, st) {
        events.push(ev);
    }

    // Candidates: a PiP window being dragged over the card
    let can_drop = ctx.visible && ctx.mode != IslandMode::Hidden;
    let cands: Vec<isize> = if can_drop {
        find_candidates().into_iter().filter(|c| *c != st.hwnd).collect()
    } else {
        Vec::new()
    };
    if !cands.contains(&st.cand) {
        st.cand = 0;
        st.cand_moving = false;
        st.cand_still = 0;
    }
    // Follow the one that is moving (or the first one)
    for c in &cands {
        let r = window_rect(*c).unwrap_or_default();
        let moving = in_move_size(*c) || (*c == st.cand && !rect_eq(r, st.cand_rect, 0));
        if *c != st.cand && (moving || st.cand == 0) {
            st.cand = *c;
            st.cand_rect = r;
            st.cand_moving = moving;
            st.cand_still = 0;
        }
    }
    let mut over = false;
    if st.cand != 0 {
        let r = window_rect(st.cand).unwrap_or_default();
        let moving = in_move_size(st.cand) || !rect_eq(r, st.cand_rect, 0);
        st.cand_rect = r;
        if moving {
            st.cand_moving = true;
            st.cand_still = 0;
            busy = true;
            over = ctx.cursor_on_card();
        } else if st.cand_moving {
            st.cand_still += 1;
            busy = true;
            if st.cand_still >= 2 {
                st.cand_moving = false;
                st.cand_still = 0;
                if ctx.cursor_on_card() {
                    let hwnd = st.cand;
                    st.cand = 0;
                    dock(st, hwnd);
                    events.push(Event::Docked(true));
                    if ctx.mode != IslandMode::Expanded {
                        events.push(Event::Expand);
                    }
                }
            } else {
                over = ctx.cursor_on_card();
            }
        }
    }
    if over != st.hover {
        st.hover = over;
        events.push(Event::Hover(over));
    }
    (events, busy || !cands.is_empty())
}

fn run(app: tauri::AppHandle, rx: Receiver<()>) {
    loop {
        let ctx = read_ctx(&app);
        if !ctx.enabled {
            release(&app);
            let hover_was = with_state(|st| {
                st.cand = 0;
                std::mem::replace(&mut st.hover, false)
            });
            if hover_was {
                emit(&app, Some(Event::Hover(false)));
            }
            let _ = rx.recv(); // zero wakeups while the option is off
            continue;
        }
        let docked = with_state(|st| st.hwnd != 0);
        if !docked && (!ctx.visible || ctx.mode == IslandMode::Hidden) {
            // Nothing to drop on and nothing to follow: block until a show/mode change
            let _ = rx.recv();
            continue;
        }

        let (events, busy) = with_state(|st| tick(&ctx, st));
        for ev in events {
            emit(&app, Some(ev));
        }

        // Fast ticks only while a video is docked or a PiP window exists
        let wait = Duration::from_millis(if busy { 100 } else { 500 });
        match rx.recv_timeout(wait) {
            Ok(()) | Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }
    }
}

#[tauri::command]
pub fn pip_set_slot(x: f64, y: f64, width: f64, height: f64, app: tauri::AppHandle) -> Result<(), String> {
    if ![x, y, width, height].iter().all(|v| v.is_finite()) {
        return Err("Invalid slot".to_string());
    }
    let slot = if width <= 0.0 || height <= 0.0 {
        None
    } else {
        Some(Slot {
            x: x.clamp(0.0, crate::WINDOW_W),
            y: y.clamp(0.0, crate::WINDOW_H),
            w: width.clamp(0.0, crate::WINDOW_W),
            h: height.clamp(0.0, crate::WINDOW_H),
        })
    };
    with_state(|st| st.slot = slot);
    sync(&app);
    Ok(())
}

#[tauri::command]
pub fn pip_undock(app: tauri::AppHandle) {
    release(&app);
}

#[tauri::command]
pub fn pip_status() -> bool {
    with_state(|st| st.hwnd != 0)
}
