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
static THUMBNAIL_CACHE: Mutex<Option<(String, String, String, Option<String>)>> = Mutex::new(None);
// Selected source app ID (if manually selected by user)
static SELECTED_SOURCE_ID: Mutex<Option<String>> = Mutex::new(None);
// Exact AUMID of the session we are currently showing (auto mode).
// Exact (not app-level) because one browser exposes several sessions with different AUMIDs.
// Only `stabilize` commits it, so a transient pick can never latch.
static SHOWN_STATE: Mutex<Option<(String, MediaState, Option<(String, Instant)>)>> = Mutex::new(None);

const SESSION_SWITCH_HOLD: Duration = Duration::from_millis(1500);

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

fn to_base64(data: &[u8]) -> String {
    const CHARSET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity((data.len() + 2) / 3 * 4);
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

    fn get_current_session() -> Option<GlobalSystemMediaTransportControlsSession> {
        let manager = Self::get_manager()?;

        // If user manually chose a source, try finding that session
        if let Ok(guard) = SELECTED_SOURCE_ID.lock() {
            if let Some(target) = guard.as_ref() {
                if let Ok(sessions) = manager.GetSessions() {
                    // ponytail: exact id; ##idx tiebreaker only among same-app dupes; else first playing of that app
                    let all: Vec<_> = sessions.into_iter().collect();
                    let mut counts: HashMap<String, usize> = HashMap::new();
                    for s in &all {
                        let r = s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                        *counts.entry(r).or_insert(0) += 1;
                    }
                    for (idx, session) in all.iter().enumerate() {
                        let raw = session.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default();
                        let tid = Self::tie_id(&raw, idx, counts.get(&raw).copied().unwrap_or(0) > 1);
                        if &tid == target {
                            return Some(session.clone());
                        }
                    }
                    // Base id (no ## tie): if several same-app sessions, prefer the one actually playing
                    let base = target.split("##").next().unwrap_or(target);
                    if counts.get(base).copied().unwrap_or(0) > 1 {
                        if let Some(s) = all.iter().find(|s| {
                            s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default() == base
                                && Self::session_is_playing(s)
                        }) {
                            return Some(s.clone());
                        }
                    }
                    if let Some(s) = all.iter().find(|s| {
                        s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default() == base
                    }) {
                        return Some(s.clone());
                    }
                }
            }
        }

        // Auto mode. Keep the exact session we're showing (a browser exposes several sessions
        // with different AUMIDs, e.g. Brave vs Brave._crx_...); only switch when it's gone.
        // Transient flashes are absorbed later by `stabilize`'s debounce.
        if let Ok(sessions) = manager.GetSessions() {
            let all: Vec<_> = sessions.into_iter().collect();
            let aumid_of = |s: &GlobalSystemMediaTransportControlsSession| {
                s.SourceAppUserModelId().map(|h| h.to_string()).unwrap_or_default()
            };

            // 1. Exact session we were showing, if it still exists
            if let Some(shown) = shown_aumid() {
                if let Some(s) = all.iter().find(|s| aumid_of(s) == shown) {
                    return Some(s.clone());
                }
            }
            // 2. Otherwise a playing session, else the first available
            if let Some(s) = all.iter().find(|s| Self::session_is_playing(s)) {
                return Some(s.clone());
            }
            if let Some(s) = all.first() {
                return Some(s.clone());
            }
        }

        // Default fallback to system's current session
        manager.GetCurrentSession().ok()
    }

    pub fn select_source(source_id: Option<String>) {
        if let Ok(mut guard) = SELECTED_SOURCE_ID.lock() {
            *guard = source_id;
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

        let selected_id = SELECTED_SOURCE_ID.lock().ok().and_then(|g| g.clone());

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
                let is_current = if let Some(ref sel) = selected_id {
                    sel == &session_unique_id || sel == &raw_id
                } else {
                    raw_id == current_app_id
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
        }
        // Playing sessions first so the live source is easy to pick
        list.sort_by(|a, b| b.is_playing.cmp(&a.is_playing));
        list
    }

    pub fn selected_source_id() -> Option<String> {
        SELECTED_SOURCE_ID.lock().ok().and_then(|g| g.clone())
    }

    pub fn current_source_app_id() -> String {
        Self::get_current_session()
            .and_then(|s| s.SourceAppUserModelId().ok())
            .map(|h| h.to_string())
            .unwrap_or_default()
    }

    fn clean_app_name(app_id: &str) -> String {
        let lower = app_id.to_lowercase();
        if lower.contains("spotify") {
            "Spotify".to_string()
        } else if lower.contains("chrome") {
            "Google Chrome".to_string()
        } else if lower.contains("msedge") || lower.contains("edge") {
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
            let name = app_id.split('\\').last().unwrap_or(app_id);
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

        let state = MediaState {
            has_session: true,
            source_app_id: app_id,
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
                                                            fetched = Some(format!("data:image/jpeg;base64,{}", to_base64(&buf)));
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
