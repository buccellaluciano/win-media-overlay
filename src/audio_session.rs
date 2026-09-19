// Per-application volume via the Windows audio session API (WASAPI mixer).
// ponytail: matches GSMTC app -> WASAPI session by process image name; heuristic, falls back to system volume upstream
use std::cell::RefCell;
use std::ffi::OsString;
use std::os::windows::ffi::OsStringExt;
use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::CloseHandle;
use windows::Win32::Media::Audio::{
    eMultimedia, eRender, IAudioSessionControl2, IAudioSessionManager2, IMMDeviceEnumerator,
    ISimpleAudioVolume, MMDeviceEnumerator,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_ALL, COINIT_MULTITHREADED,
};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};

thread_local! {
    static SESSION_CACHE: RefCell<Option<(String, Vec<ISimpleAudioVolume>)>> = const { RefCell::new(None) };
}

fn expected_stems(app_id: &str) -> Vec<String> {
    let lower = app_id.to_lowercase();
    let mut v = Vec::new();
    if lower.contains("spotify") {
        v.push("spotify".to_string());
    }
    if lower.contains("chrome") {
        v.push("chrome".to_string());
    }
    if lower.contains("msedge") || lower.contains("edge") {
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

// Full image path -> lowercased exe stem (e.g. "...\brave.exe" -> "brave")
unsafe fn image_stem(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    let handle = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
    let mut buf = [0u16; 1024];
    let mut size = buf.len() as u32;
    let res = QueryFullProcessImageNameW(
        handle,
        PROCESS_NAME_WIN32,
        PWSTR(buf.as_mut_ptr()),
        &mut size,
    );
    let _ = CloseHandle(handle);
    res.ok()?;
    let path = OsString::from_wide(&buf[..size as usize]);
    let name = std::path::Path::new(&path)
        .file_name()?
        .to_string_lossy()
        .to_lowercase();
    Some(name.trim_end_matches(".exe").to_string())
}

fn enumerate_sessions() -> Vec<(String, ISimpleAudioVolume)> {
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
            let Ok(pid) = control2.GetProcessId() else {
                continue;
            };
            let Some(stem) = image_stem(pid) else {
                continue;
            };
            let Ok(volume): std::result::Result<ISimpleAudioVolume, _> = control2.cast() else {
                continue;
            };
            out.push((stem, volume));
        }
        out
    }
}

fn sessions_for_app(app_id: &str) -> Vec<ISimpleAudioVolume> {
    let expected = expected_stems(app_id);
    enumerate_sessions()
        .into_iter()
        .filter(|(stem, _)| {
            expected
                .iter()
                .any(|e| stem == e || stem.contains(e.as_str()))
        })
        .map(|(_, v)| v)
        .collect()
}

fn sessions_cached(app_id: &str) -> Vec<ISimpleAudioVolume> {
    SESSION_CACHE.with(|cell| {
        if let Some((id, list)) = cell.borrow().as_ref() {
            if id == app_id && !list.is_empty() {
                return list.clone();
            }
        }
        let list = sessions_for_app(app_id);
        // Don't cache an empty match: re-enumerate next call (app may have just started playing)
        *cell.borrow_mut() = Some((app_id.to_string(), list.clone()));
        list
    })
}

pub fn get_volume(app_id: &str) -> Result<f32, String> {
    let sessions = sessions_cached(app_id);
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
    let sessions = sessions_cached(app_id);
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

pub fn is_muted(app_id: &str) -> Result<bool, String> {
    let sessions = sessions_cached(app_id);
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
    let sessions = sessions_cached(app_id);
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
