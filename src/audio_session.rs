// Per-application volume via the Windows audio session API (WASAPI mixer).
// ponytail: app -> session matching uses the audio session identifier (which embeds the
// exe path), so we never query other processes (anticheat/AV pattern).
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{Interface, PWSTR};
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, AudioSessionStateActive, IAudioSessionControl2, IAudioSessionManager2,
    IMMDeviceEnumerator, ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CLSCTX_ALL, COINIT_MULTITHREADED,
};

struct SessionEntry {
    stem: String,
    volume: ISimpleAudioVolume,
    active: bool,
}

// pid -> (exe stem, cached at). Short TTL: players recreate their session on track change.
static STEM_CACHE: Mutex<Option<HashMap<u32, (String, Instant)>>> = Mutex::new(None);
const STEM_CACHE_TTL: Duration = Duration::from_secs(5);

fn expected_stems(app_id: &str) -> Vec<String> {
    let lower = app_id.to_lowercase();
    let mut v = Vec::new();
    // Firefox's AUMID is a hash, not a name: map it before the generic heuristics
    if lower.contains("308046b0af4a39cb") {
        v.push("firefox".to_string());
    }
    if lower.contains("spotify") {
        v.push("spotify".to_string());
    }
    if lower.contains("chrome") {
        v.push("chrome".to_string());
    }
    if lower.contains("msedge") || lower.starts_with("msedge") {
        v.push("msedge".to_string());
    }
    if lower.contains("firefox") {
        v.push("firefox".to_string());
    }
    if lower.contains("brave") {
        v.push("brave".to_string());
    }
    if lower.contains("vlc") {
        v.push("vlc".to_string());
    }
    if lower.contains("apple") || lower.contains("itunes") {
        v.push("applemusic".to_string());
        v.push("itunes".to_string());
    }
    if lower.contains("tidal") {
        v.push("tidal".to_string());
    }
    if lower.contains("foobar") {
        v.push("foobar".to_string());
    }
    if v.is_empty() {
        if let Some(token) = lower.split(['.', '!', '_']).next() {
            let token = token.trim();
            if !token.is_empty() {
                v.push(token.to_string());
            }
        }
    }
    v
}

// "{...}|\\Device\\HarddiskVolume3\\...\\Spotify.exe%b{...}|1%b12345" -> "spotify"
fn stem_from_instance_identifier(raw: &str) -> Option<String> {
    for part in raw.split('|') {
        // The instance part appends "%b{guid}" style metadata after the path
        let path = part.split('%').next().unwrap_or(part).trim();
        if path.is_empty() {
            continue;
        }
        let name = path.rsplit('\\').next()?.trim();
        let lower = name.to_lowercase();
        let Some(stem) = lower.strip_suffix(".exe") else {
            continue;
        };
        let stem = stem.trim();
        if !stem.is_empty() {
            return Some(stem.to_string());
        }
    }
    None
}

unsafe fn take_pwstr(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    let value = p.to_string().ok();
    CoTaskMemFree(Some(p.0 as *const core::ffi::c_void));
    value
}

// Exe stem for a session, cached per pid so a slider drag doesn't reparse every tick
unsafe fn session_stem(control: &IAudioSessionControl2) -> Option<String> {
    let pid = control.GetProcessId().ok()?;
    if pid == 0 {
        return None;
    }
    if let Ok(mut guard) = STEM_CACHE.lock() {
        if let Some(map) = guard.as_mut() {
            if let Some((stem, at)) = map.get(&pid) {
                if at.elapsed() < STEM_CACHE_TTL {
                    return Some(stem.clone());
                }
            }
        }
    }
    let raw = control
        .GetSessionInstanceIdentifier()
        .ok()
        .and_then(|p| take_pwstr(p))
        .or_else(|| {
            control
                .GetSessionIdentifier()
                .ok()
                .and_then(|p| take_pwstr(p))
        })?;
    let stem = stem_from_instance_identifier(&raw)?;
    if let Ok(mut guard) = STEM_CACHE.lock() {
        guard
            .get_or_insert_with(HashMap::new)
            .insert(pid, (stem.clone(), Instant::now()));
    }
    Some(stem)
}

fn enumerate_sessions() -> Vec<SessionEntry> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let enumerator: IMMDeviceEnumerator =
            match CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL) {
                Ok(e) => e,
                Err(_) => return Vec::new(),
            };
        let device = match enumerator.GetDefaultAudioEndpoint(eRender, eMultimedia) {
            Ok(d) => d,
            Err(_) => return Vec::new(),
        };
        let manager: IAudioSessionManager2 = match device.Activate(CLSCTX_ALL, None) {
            Ok(m) => m,
            Err(_) => return Vec::new(),
        };
        let sessions = match manager.GetSessionEnumerator() {
            Ok(s) => s,
            Err(_) => return Vec::new(),
        };
        let count = sessions.GetCount().unwrap_or(0);
        let mut out = Vec::new();
        for i in 0..count {
            let Ok(control) = sessions.GetSession(i) else {
                continue;
            };
            let Ok(control2): std::result::Result<IAudioSessionControl2, _> = control.cast()
            else {
                continue;
            };
            // ponytail: S_FALSE (not system sounds) is >= 0, so must compare to S_OK (0) explicitly
            if control2.IsSystemSoundsSession().0 == 0 {
                continue;
            }
            let Some(stem) = session_stem(&control2) else {
                continue;
            };
            let Ok(volume): std::result::Result<ISimpleAudioVolume, _> = control2.cast() else {
                continue;
            };
            let active = matches!(control.GetState(), Ok(state) if state == AudioSessionStateActive);
            out.push(SessionEntry {
                stem,
                volume,
                active,
            });
        }
        out
    }
}

// ponytail: always re-enumerate — players recreate their audio session on track change/seek,
// so a cached session writes the wrong one and the new session keeps the default volume.
fn sessions_for_app(app_id: &str) -> Vec<ISimpleAudioVolume> {
    let expected = expected_stems(app_id);
    let mut matched = Vec::new();
    let mut active = Vec::new();
    for entry in enumerate_sessions() {
        if expected
            .iter()
            .any(|e| entry.stem == *e || entry.stem.contains(e.as_str()))
        {
            matched.push(entry.volume.clone());
        }
        if entry.active {
            active.push(entry.volume);
        }
    }
    if !matched.is_empty() {
        return matched;
    }
    // Opaque AUMID (e.g. Firefox): if exactly one app session is active, that's the one playing
    if active.len() == 1 {
        return active;
    }
    Vec::new()
}

pub fn get_volume(app_id: &str) -> Result<f32, String> {
    let sessions = sessions_for_app(app_id);
    if sessions.is_empty() {
        return Err("No audio session for app".to_string());
    }
    let mut max = 0.0f32;
    let mut found = false;
    for s in &sessions {
        if let Ok(v) = unsafe { s.GetMasterVolume() } {
            found = true;
            if v > max {
                max = v;
            }
        }
    }
    if found {
        Ok(max)
    } else {
        Err("No volume".to_string())
    }
}

pub fn set_volume(app_id: &str, level: f32) -> Result<(), String> {
    if !level.is_finite() {
        return Err("Invalid volume".to_string());
    }
    let sessions = sessions_for_app(app_id);
    if sessions.is_empty() {
        return Err("No audio session for app".to_string());
    }
    let clamped = level.clamp(0.0, 1.0);
    let mut ok = false;
    for s in &sessions {
        if unsafe { s.SetMasterVolume(clamped, std::ptr::null()) }.is_ok() {
            ok = true;
        }
    }
    if ok {
        Ok(())
    } else {
        Err("Set volume failed".to_string())
    }
}

pub fn set_muted(app_id: &str, muted: bool) -> Result<(), String> {
    let sessions = sessions_for_app(app_id);
    if sessions.is_empty() {
        return Err("No audio session for app".to_string());
    }
    let mut ok = false;
    for s in &sessions {
        if unsafe { s.SetMute(muted, std::ptr::null()) }.is_ok() {
            ok = true;
        }
    }
    if ok {
        Ok(())
    } else {
        Err("Set mute failed".to_string())
    }
}

pub fn is_muted(app_id: &str) -> Result<bool, String> {
    let sessions = sessions_for_app(app_id);
    if sessions.is_empty() {
        return Err("No audio session for app".to_string());
    }
    let mut any_unmuted = false;
    for s in &sessions {
        if let Ok(m) = unsafe { s.GetMute() } {
            if !m.as_bool() {
                any_unmuted = true;
            }
        }
    }
    Ok(!any_unmuted)
}

pub fn toggle_mute(app_id: &str) -> Result<bool, String> {
    let sessions = sessions_for_app(app_id);
    if sessions.is_empty() {
        return Err("No audio session for app".to_string());
    }
    let target = !is_muted(app_id)?;
    let mut ok = false;
    for s in &sessions {
        if unsafe { s.SetMute(target, std::ptr::null()) }.is_ok() {
            ok = true;
        }
    }
    if ok {
        Ok(target)
    } else {
        Err("Mute failed".to_string())
    }
}
