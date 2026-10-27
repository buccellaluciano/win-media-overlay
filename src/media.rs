use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::Interface;
use windows::Foundation::TimeSpan;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession,
    GlobalSystemMediaTransportControlsSessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaState {
    pub has_session: bool,
    pub source_app_id: String,
    pub source_name: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub is_playing: bool,
    pub can_play_pause: bool,
    pub can_skip_next: bool,
    pub can_skip_previous: bool,
    pub position_sec: f64,
    pub duration_sec: f64,
    pub thumbnail_base64: Option<String>,
}

impl Default for MediaState {
    fn default() -> Self {
        Self {
            has_session: false,
            source_app_id: String::new(),
            source_name: "Windows Media".to_string(),
            title: "Sin reproducción".to_string(),
            artist: "Inicia tu reproductor de música".to_string(),
            album: String::new(),
            is_playing: false,
            can_play_pause: false,
            can_skip_next: false,
            can_skip_previous: false,
            position_sec: 0.0,
            duration_sec: 0.0,
            thumbnail_base64: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioSourceItem {
    pub id: String,
    pub name: String,
    pub title: String,
    pub artist: String,
    pub is_current: bool,
    pub is_playing: bool,
}

// Global cache: (Track Title, Track Artist, Source App ID, Cached Base64)
type ThumbnailCache = Mutex<Option<(String, String, String, Option<String>)>>;
static THUMBNAIL_CACHE: ThumbnailCache = Mutex::new(None);
// Manual selection: id (with ##idx tiebreaker) + title at the time of picking
#[derive(Clone)]
struct SelectedSource {
    id: String,
    title: String,
}
static SELECTED_SOURCE: Mutex<Option<SelectedSource>> = Mutex::new(None);
// Exact AUMID of the session we are currently showing (auto mode).
// Exact (not app-level) because one browser exposes several sessions with different AUMIDs.
// Only `stabilize` commits it, so a transient pick can never latch.
type ShownState = Mutex<Option<(String, MediaState, Option<(String, Instant)>)>>;
static SHOWN_STATE: ShownState = Mutex::new(None);
// AUMID -> instant it started playing (penalizes just-appeared ads/previews)
static PLAYING_SINCE: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);
// AUMID -> (title, duration, cached at): scoring must stay cheap on every poll
type SessionMetaCache = Mutex<Option<HashMap<String, (String, f64, Instant)>>>;
static SESSION_META: SessionMetaCache = Mutex::new(None);

const SESSION_SWITCH_HOLD: Duration = Duration::from_millis(1500);
const SESSION_META_TTL: Duration = Duration::from_secs(2);
const JUST_STARTED: Duration = Duration::from_millis(1500);
const SHORT_TRACK_SECS: f64 = 8.0;

// The AUMID currently committed for display (used as sticky preference when picking a session)
fn shown_aumid() -> Option<String> {
    SHOWN_STATE.lock().ok().and_then(|g| g.as_ref().map(|(k, _, _)| k.clone()))
}

// Hold the previously shown state while a different session is only transiently reported.
// Any key change (including "no session") must persist for the hold window to commit.
fn stabilize(state: MediaState) -> MediaState {
    let key = state.source_app_id.clone();
    let Ok(mut g) = SHOWN_STATE.lock() else {
        return state;
    };
    let Some((shown_key, shown_state, pending)) = g.as_mut() else {
        *g = Some((key, state.clone(), None));
        return state;
    };
    if *shown_key == key {
        *shown_state = state.clone();
        *pending = None;
        return state;
    }
    match pending {
        Some((k, since)) if *k == key => {
            if since.elapsed() >= SESSION_SWITCH_HOLD {
                *shown_key = key;
                *shown_state = state.clone();
                *pending = None;
                state
            } else {
                shown_state.clone()
            }
        }
        _ => {
            *pending = Some((key, Instant::now()));
            shown_state.clone()
        }
    }
}

// Thumbnail format must match the real bytes: wrong MIME makes WebView2 reject the image
fn image_mime(data: &[u8]) -> &'static str {
    if data.starts_with(&[0x89, 0x50, 0x4E, 0x47]) {
        "image/png"
    } else if data.starts_with(&[0xFF, 0xD8]) {
        "image/jpeg"
    } else if data.len() >= 12 && &data[0..4] == b"RIFF" && &data[8..12] == b"WEBP" {
        "image/webp"
    } else {
        "image/jpeg"
    }
}

fn to_base64(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let b0 = chunk[0] as usize;
        let b1 = chunk.get(1).copied().unwrap_or(0) as usize;
        let b2 = chunk.get(2).copied().unwrap_or(0) as usize;
        let triple = (b0 << 16) | (b1 << 8) | b2;
        out.push(CHARSET[(triple >> 18) & 63] as char);
        out.push(CHARSET[(triple >> 12) & 63] as char);
        if chunk.len() > 1 {
            out.push(CHARSET[(triple >> 6) & 63] as char);
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(CHARSET[triple & 63] as char);
        } else {
            out.push('=');
        }
    }
    out
}

pub struct MediaController;

// ponytail: one manager per thread (thread_local keeps COM apartment-safe); shared globals caused the cross-thread hang
thread_local! {
    static MANAGER: RefCell<Option<GlobalSystemMediaTransportControlsSessionManager>> =
        const { RefCell::new(None) };
}

impl MediaController {
    // Drop the cached manager so the next call recreates it (shell restarts, RPC errors)
    fn invalidate_manager() {
        MANAGER.with(|cell| *cell.borrow_mut() = None);
    }

    fn get_manager() -> Option<GlobalSystemMediaTransportControlsSessionManager> {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        MANAGER.with(|cell| {
            if let Some(mgr) = cell.borrow().as_ref() {
                return Some(mgr.clone());
            }
            let mgr = GlobalSystemMediaTransportControlsSessionManager::RequestAsync()
                .ok()?
                .get()
                .ok()?;
            *cell.borrow_mut() = Some(mgr.clone());
            Some(mgr)
        })
    }

    fn tie_id(raw: &str, idx: usize, dup: bool) -> String {
        if dup {
            format!("{}##{}", raw, idx)
        } else {
            raw.to_string()
        }
    }

    fn session_is_playing(session: &GlobalSystemMediaTransportControlsSession) -> bool {
        session
            .GetPlaybackInfo()
            .ok()
            .filter(|info| !info.as_raw().is_null())
            .and_then(|info| info.PlaybackStatus().ok())
            .map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing)
            .unwrap_or(false)
    }

    fn session_title(session: &GlobalSystemMediaTransportControlsSession) -> String {
        if let Ok(op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(props) = op.get() {
                if !props.as_raw().is_null() {
                    return props.Title().map(|h| h.to_string()).unwrap_or_default();
                }
            }
        }
        String::new()
    }

    // Title + duration for scoring, cached briefly so the 3 Hz poll stays cheap
    fn session_meta(session: &GlobalSystemMediaTransportControlsSession, aumid: &str) -> (String, f64) {
        if let Ok(guard) = SESSION_META.lock() {
            if let Some(map) = guard.as_ref() {
                if let Some((title, duration, at)) = map.get(aumid) {
                    if at.elapsed() < SESSION_META_TTL {
                        return (title.clone(), *duration);
                    }
                }
            }
        }
        let title = Self::session_title(session);
        let mut duration = 0.0;
        if let Ok(timeline) = session.GetTimelineProperties() {
            if !timeline.as_raw().is_null() {
                if let Ok(end) = timeline.EndTime() {
                    let secs = end.Duration as f64 / 10_000_000.0;
                    if secs.is_finite() && secs > 0.0 {
                        duration = secs;
                    }
                }
            }
        }
        if let Ok(mut guard) = SESSION_META.lock() {
            guard
                .get_or_insert_with(HashMap::new)
                .insert(aumid.to_string(), (title.clone(), duration, Instant::now()));
        }
        (title, duration)
    }

    fn track_playing(aumid: &str, playing: bool) {
        if let Ok(mut guard) = PLAYING_SINCE.lock() {
            let map = guard.get_or_insert_with(HashMap::new);
            if playing {
                map.entry(aumid.to_string()).or_insert_with(Instant::now);
            } else {
                map.remove(aumid);
            }
        }
    }

    fn playing_for(aumid: &str) -> Duration {
        PLAYING_SINCE
            .lock()
            .ok()
            .and_then(|g| g.as_ref().and_then(|m| m.get(aumid).map(|t| t.elapsed())))
            .unwrap_or(Duration::MAX)
    }

    // Auto-mode score: playing and sticky first, tiny/new flashes last
    fn score_session(
        playing: bool,
        is_shown: bool,
        has_title: bool,
        duration: f64,
        is_windows_current: bool,
        playing_for: Duration,
    ) -> i32 {
        let mut score = 0i32;
        if playing {
            score += 100;
        }
        if is_shown {
            score += 40;
        }
        if has_title && duration > 0.0 {
            score += 20;
        }
        if is_windows_current {
            score += 10;
        }
        if playing && playing_for < JUST_STARTED {
            score -= 50;
        }
        if duration > 0.0 && duration < SHORT_TRACK_SECS {
            score -= 30;
        }
        score
    }

    // Title of the session a manual id points at (stored so duplicates survive reorders)
    fn title_for_id(target: &str) -> Option<String> {
        let manager = Self::get_manager()?;
        let sessions = manager.GetSessions().ok()?;
        let all: Vec<_> = sessions.into_iter().collect();
        let aumids: Vec<String> = all
            .iter()
            .map(|s| s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default())
            .collect();
        let mut counts: HashMap<String, usize> = HashMap::new();
        for aumid in &aumids {
            if !aumid.is_empty() {
                *counts.entry(aumid.clone()).or_insert(0) += 1;
            }
        }
        for (idx, aumid) in aumids.iter().enumerate() {
            let dup = counts.get(aumid).copied().unwrap_or(0) > 1;
            if Self::tie_id(aumid, idx, dup) == target {
                return Some(Self::session_title(&all[idx]));
            }
        }
        let base = target.split("##").next().unwrap_or(target);
        let idx = (0..all.len()).find(|&i| aumids[i] == base)?;
        Some(Self::session_title(&all[idx]))
    }

    fn get_current_session() -> Option<GlobalSystemMediaTransportControlsSession> {
        let manager = Self::get_manager()?;

        // Manual selection: exact id first, then title (duplicates), then playing, then the app
        let manual = SELECTED_SOURCE.lock().ok().and_then(|g| g.clone());
        if let Some(sel) = manual {
            let target = sel.id.as_str();
            match manager.GetSessions() {
                Ok(sessions) => {
                    let all: Vec<_> = sessions.into_iter().collect();
                    let aumids: Vec<String> = all
                        .iter()
                        .map(|s| s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default())
                        .collect();
                    let mut counts: HashMap<String, usize> = HashMap::new();
                    for aumid in &aumids {
                        if !aumid.is_empty() {
                            *counts.entry(aumid.clone()).or_insert(0) += 1;
                        }
                    }
                    for (idx, aumid) in aumids.iter().enumerate() {
                        let dup = counts.get(aumid).copied().unwrap_or(0) > 1;
                        if Self::tie_id(aumid, idx, dup) == target {
                            return Some(all[idx].clone());
                        }
                    }
                    // The index suffix depends on GetSessions() order: match the saved title first
                    let base = target.split("##").next().unwrap_or(target);
                    let same_app: Vec<usize> = (0..all.len()).filter(|&i| aumids[i] == base).collect();
                    if same_app.len() > 1 && !sel.title.is_empty() {
                        if let Some(&idx) = same_app
                            .iter()
                            .find(|&&i| Self::session_title(&all[i]) == sel.title)
                        {
                            return Some(all[idx].clone());
                        }
                    }
                    if let Some(&idx) = same_app.iter().find(|&&i| Self::session_is_playing(&all[i])) {
                        return Some(all[idx].clone());
                    }
                    if let Some(&idx) = same_app.first() {
                        return Some(all[idx].clone());
                    }
                }
                // A broken manager would keep failing forever: recreate it on the next call
                Err(_) => Self::invalidate_manager(),
            }
        }

        // Auto mode: score every session and prefer what we are already showing (sticky).
        // Transient flashes are still absorbed later by `stabilize`'s debounce.
        let windows_current = manager
            .GetCurrentSession()
            .ok()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string())
            .unwrap_or_default();

        match manager.GetSessions() {
            Ok(sessions) => {
                let all: Vec<_> = sessions.into_iter().collect();
                if all.is_empty() {
                    return manager.GetCurrentSession().ok();
                }
                let aumids: Vec<String> = all
                    .iter()
                    .map(|s| s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default())
                    .collect();
                let shown = shown_aumid();
                let mut best: Option<(i32, usize)> = None;

                // ponytail: browser tabs share one AUMID. Track "playing since" per app (any tab
                // playing) and cache meta per tab, or a paused tab resets a playing tab every poll.
                let playing: Vec<bool> = all.iter().map(Self::session_is_playing).collect();
                let mut counts: HashMap<&str, usize> = HashMap::new();
                let mut app_playing: HashMap<&str, bool> = HashMap::new();
                for (idx, aumid) in aumids.iter().enumerate() {
                    if aumid.is_empty() {
                        continue;
                    }
                    *counts.entry(aumid.as_str()).or_insert(0) += 1;
                    *app_playing.entry(aumid.as_str()).or_insert(false) |= playing[idx];
                }
                for (aumid, any_playing) in &app_playing {
                    Self::track_playing(aumid, *any_playing);
                }

                for idx in 0..all.len() {
                    let aumid = &aumids[idx];
                    if aumid.is_empty() {
                        continue;
                    }
                    let playing = playing[idx];
                    let dup = counts.get(aumid.as_str()).copied().unwrap_or(0) > 1;
                    let meta_key = Self::tie_id(aumid, idx, dup);
                    let (title, duration) = Self::session_meta(&all[idx], &meta_key);
                    let is_shown = shown.as_deref() == Some(aumid.as_str());
                    let score = Self::score_session(
                        playing,
                        is_shown,
                        !title.is_empty(),
                        duration,
                        !windows_current.is_empty() && aumid == &windows_current,
                        Self::playing_for(aumid),
                    );

                    match best {
                        Some((best_score, _)) if score > best_score => best = Some((score, idx)),
                        Some((best_score, _)) if score == best_score && is_shown => {
                            best = Some((score, idx))
                        }
                        None => best = Some((score, idx)),
                        _ => {}
                    }
                }

                if let Some((_, idx)) = best {
                    return Some(all[idx].clone());
                }
            }
            Err(_) => Self::invalidate_manager(),
        }

        // Default fallback to system's current session
        manager.GetCurrentSession().ok()
    }

    pub fn select_source(source_id: Option<String>) {
        let stored = source_id.map(|id| {
            let title = Self::title_for_id(&id).unwrap_or_default();
            SelectedSource { id, title }
        });
        if let Ok(mut guard) = SELECTED_SOURCE.lock() {
            *guard = stored;
        }
        // Manual choice must show immediately: drop the auto-selection debounce
        if let Ok(mut g) = SHOWN_STATE.lock() {
            *g = None;
        }
        // Invalidate thumbnail cache so new source fetches its own art
        if let Ok(mut cache) = THUMBNAIL_CACHE.lock() {
            *cache = None;
        }
    }

    pub fn get_audio_sources() -> Vec<AudioSourceItem> {
        let manager = match Self::get_manager() {
            Some(m) => m,
            None => return Vec::new(),
        };

        let current_app_id = match manager.GetCurrentSession().ok() {
            Some(s) => s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default(),
            None => String::new(),
        };

        let selected_id = Self::selected_source_id();

        let mut list = Vec::new();
        if let Ok(sessions) = manager.GetSessions() {
            let all: Vec<_> = sessions.into_iter().collect();
            let mut counts: HashMap<String, usize> = HashMap::new();
            for s in &all {
                let r = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                if !r.is_empty() {
                    *counts.entry(r).or_insert(0) += 1;
                }
            }
            for (idx, session) in all.iter().enumerate() {
                let raw_id = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                if raw_id.is_empty() {
                    continue;
                }

                let mut title = "Sin título".to_string();
                let mut artist = String::new();

                if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
                    if let Ok(props) = props_op.get() {
                        if !props.as_raw().is_null() {
                            if let Ok(t) = props.Title() {
                                let s = t.to_string();
                                if !s.is_empty() {
                                    title = s;
                                }
                            }
                            if let Ok(a) = props.Artist() {
                                artist = a.to_string();
                            }
                        }
                    }
                }

                let is_playing = Self::session_is_playing(session);

                let session_unique_id = Self::tie_id(&raw_id, idx, counts.get(&raw_id).copied().unwrap_or(0) > 1);
                // Auto mode must mark the session we actually show, not Windows' current one
                let is_current = if let Some(ref sel) = selected_id {
                    sel == &session_unique_id || sel == &raw_id
                } else {
                    match shown_aumid() {
                        Some(shown) => raw_id == shown,
                        None => raw_id == current_app_id,
                    }
                };

                let display_name = Self::clean_app_name(&raw_id);

                list.push(AudioSourceItem {
                    id: session_unique_id,
                    name: display_name,
                    title,
                    artist,
                    is_current,
                    is_playing,
                });
            }
        } else {
            Self::invalidate_manager();
        }
        // Playing sessions first so the live source is easy to pick
        list.sort_by_key(|a| std::cmp::Reverse(a.is_playing));
        list
    }

    pub fn selected_source_id() -> Option<String> {
        SELECTED_SOURCE
            .lock()
            .ok()
            .and_then(|g| g.as_ref().map(|s| s.id.clone()))
    }

    pub fn current_source_app_id() -> String {
        // The poll already committed what we show: reuse it instead of re-enumerating GSMTC
        if let Some(aumid) = shown_aumid() {
            return aumid;
        }
        Self::get_current_session()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string())
            .unwrap_or_default()
    }

    fn clean_app_name(app_id: &str) -> String {
        if app_id.is_empty() {
            return "Windows Media".to_string();
        }
        let lower = app_id.to_lowercase();
        // Firefox reports an opaque hash as AUMID, not its name
        if lower.contains("308046b0af4a39cb") {
            "Mozilla Firefox".to_string()
        } else if lower.contains("spotify") {
            "Spotify".to_string()
        } else if lower.contains("chrome") {
            "Google Chrome".to_string()
        } else if lower.contains("msedge") || lower.starts_with("msedge") {
            "Microsoft Edge".to_string()
        } else if lower.contains("firefox") {
            "Mozilla Firefox".to_string()
        } else if lower.contains("brave") {
            "Brave".to_string()
        } else if lower.contains("vlc") {
            "VLC Media Player".to_string()
        } else if lower.contains("apple") || lower.contains("itunes") {
            "Apple Music".to_string()
        } else if lower.contains("tidal") {
            "TIDAL".to_string()
        } else if lower.contains("foobar") {
            "foobar2000".to_string()
        } else {
            // Strip .exe or path if present
            let name = app_id.split('\\').next_back().unwrap_or(app_id);
            let name = name.strip_suffix(".exe").unwrap_or(name);
            name.to_string()
        }
    }

    pub fn get_state() -> MediaState {
        let session = match Self::get_current_session() {
            Some(s) => s,
            None => {
                if let Ok(mut cache) = THUMBNAIL_CACHE.lock() {
                    *cache = None;
                }
                return MediaState::default();
            }
        };

        let app_id = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        let mut title = "Desconocido".to_string();
        let mut artist = "Desconocido".to_string();
        let mut album = String::new();

        if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(props) = props_op.get() {
                if !props.as_raw().is_null() {
                    if let Ok(t) = props.Title() {
                        let s = t.to_string();
                        if !s.is_empty() {
                            title = s;
                        }
                    }
                    if let Ok(a) = props.Artist() {
                        let s = a.to_string();
                        if !s.is_empty() {
                            artist = s;
                        }
                    }
                    if let Ok(al) = props.AlbumTitle() {
                        album = al.to_string();
                    }
                }
            }
        }

        // ponytail: art is fetched on demand via get_media_thumbnail; never ship base64 on the 3Hz poll
        let thumbnail_base64 = None;

        let mut is_playing = false;
        let mut can_play_pause = true;
        let mut can_skip_next = true;
        let mut can_skip_previous = true;

        if let Ok(info) = session.GetPlaybackInfo() {
            if !info.as_raw().is_null() {
                if let Ok(status) = info.PlaybackStatus() {
                    is_playing = status == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing;
                }
                if let Ok(controls) = info.Controls() {
                    if !controls.as_raw().is_null() {
                        can_play_pause = controls.IsPlayPauseToggleEnabled().unwrap_or(true)
                            || controls.IsPlayEnabled().unwrap_or(true)
                            || controls.IsPauseEnabled().unwrap_or(true);
                        can_skip_next = controls.IsNextEnabled().unwrap_or(true);
                        can_skip_previous = controls.IsPreviousEnabled().unwrap_or(true);
                    }
                }
            }
        }

        let mut position_sec = 0.0;
        let mut duration_sec = 0.0;

        if let Ok(timeline) = session.GetTimelineProperties() {
            if !timeline.as_raw().is_null() {
                if let Ok(pos) = timeline.Position() {
                    let s = (pos.Duration as f64) / 10_000_000.0;
                    if s.is_finite() && s >= 0.0 {
                        position_sec = s;
                    }
                }
                if let Ok(end) = timeline.EndTime() {
                    let s = (end.Duration as f64) / 10_000_000.0;
                    if s.is_finite() && s >= 0.0 {
                        duration_sec = s;
                    }
                }
                // GSMTC Position is the value at LastUpdatedTime; while playing add elapsed time
                if is_playing && duration_sec > 0.0 {
                    if let Ok(last) = timeline.LastUpdatedTime() {
                        if last.UniversalTime > 0 {
                            const EPOCH_DIFF_100NS: i128 = 116_444_736_000_000_000;
                            let now_100ns = std::time::SystemTime::now()
                                .duration_since(std::time::UNIX_EPOCH)
                                .map(|d| (d.as_nanos() / 100) as i128)
                                .unwrap_or(0)
                                + EPOCH_DIFF_100NS;
                            let elapsed = (now_100ns - last.UniversalTime as i128) as f64 / 10_000_000.0;
                            if elapsed > 0.0 && elapsed < 86_400.0 {
                                position_sec = (position_sec + elapsed).min(duration_sec);
                            }
                        }
                    }
                }
            }
        }

        let source_name = Self::clean_app_name(&app_id);
        let state = MediaState {
            has_session: true,
            source_app_id: app_id,
            source_name,
            title,
            artist,
            album,
            is_playing,
            can_play_pause,
            can_skip_next,
            can_skip_previous,
            position_sec,
            duration_sec,
            thumbnail_base64,
        };
        stabilize(state)
    }

    // ponytail: art fetched on demand (async from UI), never on the title/position path; 2MB cap, full-res covers later if needed
    pub fn get_media_thumbnail() -> Option<String> {
        let session = Self::get_current_session()?;
        let app_id = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
        if let Ok(props_op) = session.TryGetMediaPropertiesAsync() {
            if let Ok(props) = props_op.get() {
                if props.as_raw().is_null() {
                    return None;
                }
                let title = props.Title().map(|h| h.to_string()).unwrap_or_default();
                let artist = props.Artist().map(|h| h.to_string()).unwrap_or_default();
                if title.is_empty() && artist.is_empty() {
                    return None;
                }
                if let Ok(cache) = THUMBNAIL_CACHE.lock() {
                    if let Some((ct, ca, cs, cb)) = cache.as_ref() {
                        if ct == &title && ca == &artist && cs == &app_id {
                            return cb.clone();
                        }
                    }
                }
                let mut fetched = None;
                if let Ok(thumb_ref) = props.Thumbnail() {
                    if !thumb_ref.as_raw().is_null() {
                        if let Ok(stream) = thumb_ref.OpenReadAsync().ok()?.get() {
                            if !stream.as_raw().is_null() {
                                if let Ok(size) = stream.Size() {
                                    if size > 0 && size < 2_000_000 {
                                        if let Ok(reader) = DataReader::CreateDataReader(&stream) {
                                            if !reader.as_raw().is_null() {
                                                if let Ok(load_op) = reader.LoadAsync(size as u32) {
                                                    if load_op.get().is_ok() {
                                                        let mut buf = vec![0u8; size as usize];
                                                        if reader.ReadBytes(&mut buf).is_ok() {
                                                            fetched = Some(format!(
                                                                "data:{};base64,{}",
                                                                image_mime(&buf),
                                                                to_base64(&buf)
                                                            ));
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                if let Ok(mut cache) = THUMBNAIL_CACHE.lock() {
                    *cache = Some((title, artist, app_id, fetched.clone()));
                }
                return fetched;
            }
        }
        None
    }

    pub fn toggle_play_pause() -> Result<bool, String> {
        let session = Self::get_current_session().ok_or("No active media session")?;
        let op = session.TryTogglePlayPauseAsync().map_err(|e| e.to_string())?;
        let res = op.get().map_err(|e| e.to_string())?;
        Ok(res)
    }

    pub fn skip_next() -> Result<bool, String> {
        let session = Self::get_current_session().ok_or("No active media session")?;
        let op = session.TrySkipNextAsync().map_err(|e| e.to_string())?;
        let res = op.get().map_err(|e| e.to_string())?;
        Ok(res)
    }

    pub fn skip_previous() -> Result<bool, String> {
        let session = Self::get_current_session().ok_or("No active media session")?;
        let op = session.TrySkipPreviousAsync().map_err(|e| e.to_string())?;
        let res = op.get().map_err(|e| e.to_string())?;
        Ok(res)
    }

    pub fn seek_to(seconds: f64) -> Result<bool, String> {
        if !seconds.is_finite() || seconds < 0.0 || seconds > 86_400.0 {
            return Err("Invalid position".to_string());
        }
        let session = Self::get_current_session().ok_or("No active media session")?;
        let ticks = (seconds * 10_000_000.0) as i64;
        let timespan = TimeSpan { Duration: ticks };
        let op = session.TryChangePlaybackPositionAsync(timespan.Duration).map_err(|e| e.to_string())?;
        let res = op.get().map_err(|e| e.to_string())?;
        Ok(res)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn score(
        playing: bool,
        shown: bool,
        titled: bool,
        duration: f64,
        windows_current: bool,
        playing_for: Duration,
    ) -> i32 {
        MediaController::score_session(
            playing,
            shown,
            titled,
            duration,
            windows_current,
            playing_for,
        )
    }

    #[test]
    fn playing_current_source_beats_a_fresh_preview() {
        let spotify = score(true, true, true, 210.0, false, Duration::from_secs(30));
        let preview = score(true, false, true, 120.0, false, Duration::from_millis(300));
        assert_eq!(spotify, 160);
        assert_eq!(preview, 70);
        assert!(preview < spotify);
    }

    #[test]
    fn switches_when_the_shown_source_pauses() {
        let paused_show = score(false, true, true, 210.0, false, Duration::MAX);
        let youtube = score(true, false, true, 200.0, false, Duration::from_secs(2));
        assert_eq!(paused_show, 60);
        assert_eq!(youtube, 120);
        assert!(youtube > paused_show);
    }

    #[test]
    fn ignores_short_just_started_sounds() {
        let paused_show = score(false, true, true, 210.0, false, Duration::MAX);
        let sound = score(true, false, true, 2.0, false, Duration::from_millis(200));
        assert_eq!(sound, 40);
        assert!(sound < paused_show);
    }
}
