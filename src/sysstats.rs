// System stats sampler: CPU, RAM, GPU, VRAM, network, disk and (optional) NVIDIA temperature.
// ponytail: single sleeping thread; no WMI and no process enumeration (AV/anticheat-safe).
use serde::Serialize;
use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use windows::core::{PCSTR, PCWSTR};
use windows::Win32::Foundation::{FreeLibrary, FILETIME, HMODULE};
use windows::Win32::Graphics::Dxgi::{
    CreateDXGIFactory1, IDXGIFactory1, DXGI_ADAPTER_FLAG_SOFTWARE,
};
use windows::Win32::NetworkManagement::IpHelper::{
    FreeMibTable, GetIfTable2, MIB_IF_TABLE2, IF_TYPE_SOFTWARE_LOOPBACK,
};
use windows::Win32::NetworkManagement::Ndis::IfOperStatusUp;
use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows::Win32::System::Performance::{
    PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryData, PdhGetFormattedCounterArrayW,
    PdhOpenQueryW, PDH_FMT_COUNTERVALUE_ITEM_W, PDH_FMT_DOUBLE, PDH_HCOUNTER, PDH_HQUERY,
    PDH_MORE_DATA,
};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

use crate::AppState;

#[derive(Debug, Clone, Default, Serialize)]
pub struct SystemStats {
    pub cpu_pct: f32,
    pub ram_used_mb: u64,
    pub ram_total_mb: u64,
    pub ram_pct: f32,
    pub gpu_pct: Option<f32>,
    pub vram_used_mb: Option<u64>,
    pub vram_total_mb: Option<u64>,
    pub gpu_temp_c: Option<f32>,
    pub net_down_kbps: Option<f32>, // KB/s
    pub net_up_kbps: Option<f32>,   // KB/s
    pub disk_pct: Option<f32>,
}

static WAKE: Mutex<Option<Sender<()>>> = Mutex::new(None);

pub fn wake() {
    if let Ok(guard) = WAKE.lock() {
        if let Some(tx) = guard.as_ref() {
            let _ = tx.send(());
        }
    }
}

pub fn spawn(app: tauri::AppHandle) {
    let (tx, rx) = channel::<()>();
    *WAKE.lock().unwrap_or_else(|e| e.into_inner()) = Some(tx);
    std::thread::spawn(move || run(app, rx));
}

fn run(app: tauri::AppHandle, rx: Receiver<()>) {
    use tauri::Manager;

    // Created lazily: stats disabled means no PDH query and no nvml.dll load
    let mut sampler: Option<Sampler> = None;
    let mut last_sample: Option<Instant> = None;

    loop {
        let (enabled, interval_ms) = {
            let state = app.state::<AppState>();
            let conf = state.config.lock().unwrap_or_else(|e| e.into_inner());
            (conf.stats_enabled, conf.stats_interval_ms.clamp(500, 5000))
        };
        let visible = app
            .get_webview_window("main")
            .map(|w| w.is_visible().unwrap_or(false))
            .unwrap_or(false);

        if !enabled {
            // Release the query/NVML while the feature is off, then block: zero wakeups
            sampler = None;
            last_sample = None;
            let _ = rx.recv();
            continue;
        }

        if !visible {
            // Block instead of polling; a wake on show/hide or config change resumes us
            if let Some(s) = sampler.as_mut() {
                s.reset();
            }
            last_sample = None;
            let _ = rx.recv();
            continue;
        }

        if sampler.is_none() {
            sampler = Some(Sampler::new());
        }
        let Some(s) = sampler.as_mut() else {
            continue;
        };

        if !s.is_active() {
            // Prime deltas/counters, then wait one interval before the first real reading
            s.begin();
            let _ = rx.recv_timeout(Duration::from_millis(interval_ms as u64));
            continue;
        }

        // A config-save wake can land mid-interval; don't measure over a tiny window
        if let Some(t) = last_sample {
            if t.elapsed() < Duration::from_millis(300) {
                let _ = rx.recv_timeout(Duration::from_millis(300).saturating_sub(t.elapsed()));
                continue;
            }
        }

        if let Some(stats) = s.sample() {
            use tauri::Emitter;
            let _ = app.emit("system-stats", &stats);
            last_sample = Some(Instant::now());
        }
        let _ = rx.recv_timeout(Duration::from_millis(interval_ms as u64));
    }
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

fn filetime_ticks(t: FILETIME) -> u64 {
    ((t.dwHighDateTime as u64) << 32) | t.dwLowDateTime as u64
}

fn cpu_times() -> Option<(u64, u64, u64)> {
    let mut idle = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).ok()?;
    }
    Some((
        filetime_ticks(idle),
        filetime_ticks(kernel),
        filetime_ticks(user),
    ))
}

fn net_totals() -> Option<(u64, u64)> {
    unsafe {
        let mut table: *mut MIB_IF_TABLE2 = std::ptr::null_mut();
        if GetIfTable2(&mut table).0 != 0 || table.is_null() {
            return None;
        }
        let t = &*table;
        let rows = std::slice::from_raw_parts(t.Table.as_ptr(), t.NumEntries as usize);
        let mut down = 0u64;
        let mut up = 0u64;
        for row in rows {
            if row.OperStatus != IfOperStatusUp || row.Type == IF_TYPE_SOFTWARE_LOOPBACK {
                continue;
            }
            down = down.saturating_add(row.InOctets);
            up = up.saturating_add(row.OutOctets);
        }
        FreeMibTable(table as *const c_void);
        Some((down, up))
    }
}

fn total_dedicated_vram_mb() -> Option<u64> {
    unsafe {
        let factory: IDXGIFactory1 = CreateDXGIFactory1().ok()?;
        let mut total = 0u64;
        let mut index = 0;
        loop {
            let Ok(adapter) = factory.EnumAdapters1(index) else {
                break;
            };
            index += 1;
            let Ok(desc) = adapter.GetDesc1() else {
                continue;
            };
            let software = (desc.Flags & DXGI_ADAPTER_FLAG_SOFTWARE.0 as u32) != 0;
            if !software {
                total = total.saturating_add(desc.DedicatedVideoMemory as u64);
            }
        }
        if total > 0 {
            Some(total / (1024 * 1024))
        } else {
            None
        }
    }
}

// PDH counter instances are per adapter; sum engines per LUID and keep the busiest adapter
fn gpu_percent(items: &[(String, f64)]) -> Option<f32> {
    let mut per_adapter: HashMap<String, f64> = HashMap::new();
    for (name, value) in items {
        let key = match name.find("luid_") {
            Some(i) => {
                let rest = &name[i..];
                rest.split("_phys").next().unwrap_or(rest).to_string()
            }
            None => name.clone(),
        };
        *per_adapter.entry(key).or_insert(0.0) += *value;
    }
    if per_adapter.is_empty() {
        return None;
    }
    let max = per_adapter.values().copied().fold(0.0f64, f64::max);
    Some(max.clamp(0.0, 100.0) as f32)
}

type FnInit = unsafe extern "C" fn() -> i32;
type FnShutdown = unsafe extern "C" fn() -> i32;
type FnGetHandle = unsafe extern "C" fn(u32, *mut *mut c_void) -> i32;
type FnGetTemp = unsafe extern "C" fn(*mut c_void, i32, *mut u32) -> i32;

struct Nvml {
    lib: HMODULE,
    shutdown: FnShutdown,
    handle: *mut c_void,
    get_temp: FnGetTemp,
}

impl Nvml {
    // Only used if the NVIDIA driver already installed nvml.dll; nothing is downloaded
    fn load() -> Option<Self> {
        unsafe {
            let name = wide("nvml.dll");
            let lib = LoadLibraryW(PCWSTR(name.as_ptr())).ok()?;

            let p_init = GetProcAddress(lib, PCSTR(c"nvmlInit_v2".as_ptr() as *const u8))?;
            let init: FnInit = std::mem::transmute(p_init);
            let p_shutdown = GetProcAddress(lib, PCSTR(c"nvmlShutdown".as_ptr() as *const u8))?;
            let shutdown: FnShutdown = std::mem::transmute(p_shutdown);
            let p_handle = GetProcAddress(
                lib,
                PCSTR(c"nvmlDeviceGetHandleByIndex_v2".as_ptr() as *const u8),
            )?;
            let get_handle: FnGetHandle = std::mem::transmute(p_handle);
            let p_temp =
                GetProcAddress(lib, PCSTR(c"nvmlDeviceGetTemperature".as_ptr() as *const u8))?;
            let get_temp: FnGetTemp = std::mem::transmute(p_temp);

            if init() != 0 {
                let _ = FreeLibrary(lib);
                return None;
            }
            let mut handle: *mut c_void = std::ptr::null_mut();
            if get_handle(0, &mut handle) != 0 || handle.is_null() {
                let _ = shutdown();
                let _ = FreeLibrary(lib);
                return None;
            }
            Some(Self {
                lib,
                shutdown,
                handle,
                get_temp,
            })
        }
    }

    fn temperature(&self) -> Option<f32> {
        let mut temp = 0u32;
        let status = unsafe { (self.get_temp)(self.handle, 0, &mut temp) }; // NVML_TEMPERATURE_GPU
        if status == 0 {
            Some(temp as f32)
        } else {
            None
        }
    }
}

impl Drop for Nvml {
    fn drop(&mut self) {
        unsafe {
            let _ = (self.shutdown)();
            let _ = FreeLibrary(self.lib);
        }
    }
}

struct Sampler {
    query: Option<PDH_HQUERY>,
    gpu_counter: Option<PDH_HCOUNTER>,
    vram_counter: Option<PDH_HCOUNTER>,
    disk_counter: Option<PDH_HCOUNTER>,
    prev_cpu: Option<(u64, u64, u64)>,
    prev_net: Option<(u64, u64, Instant)>,
    active: bool,
    nvml: Option<Nvml>,
    vram_total_mb: Option<u64>,
}

impl Sampler {
    fn new() -> Self {
        let mut query = PDH_HQUERY::default();
        let opened = unsafe { PdhOpenQueryW(PCWSTR::null(), 0, &mut query) == 0 };
        let query_handle = query;

        let add = |path: &str| -> Option<PDH_HCOUNTER> {
            if !opened {
                return None;
            }
            let w = wide(path);
            let mut counter = PDH_HCOUNTER::default();
            let status =
                unsafe { PdhAddEnglishCounterW(query_handle, PCWSTR(w.as_ptr()), 0, &mut counter) };
            if status == 0 {
                Some(counter)
            } else {
                None
            }
        };

        let gpu_counter = add(r"\GPU Engine(*engtype_3D)\Utilization Percentage");
        let vram_counter = add(r"\GPU Adapter Memory(*)\Dedicated Usage");
        let disk_counter = add(r"\PhysicalDisk(_Total)\% Idle Time");

        Self {
            query: if opened { Some(query) } else { None },
            gpu_counter,
            vram_counter,
            disk_counter,
            prev_cpu: None,
            prev_net: None,
            active: false,
            nvml: Nvml::load(),
            vram_total_mb: total_dedicated_vram_mb(),
        }
    }

    fn is_active(&self) -> bool {
        self.active
    }

    // Baseline readings so the first emitted sample covers a real time window
    fn begin(&mut self) {
        let _ = self.collect();
        self.prev_cpu = cpu_times();
        self.prev_net = net_totals().map(|(d, u)| (d, u, Instant::now()));
        self.active = true;
    }

    fn reset(&mut self) {
        self.active = false;
        self.prev_cpu = None;
        self.prev_net = None;
    }

    fn collect(&self) -> bool {
        match self.query {
            Some(q) => unsafe { PdhCollectQueryData(q) == 0 },
            None => false,
        }
    }

    fn counter_array(counter: PDH_HCOUNTER) -> Vec<(String, f64)> {
        unsafe {
            let mut size = 0u32;
            let mut count = 0u32;
            let status =
                PdhGetFormattedCounterArrayW(counter, PDH_FMT_DOUBLE, &mut size, &mut count, None);
            if status != PDH_MORE_DATA || size == 0 {
                return Vec::new();
            }
            let mut buffer = vec![0u8; size as usize];
            let status = PdhGetFormattedCounterArrayW(
                counter,
                PDH_FMT_DOUBLE,
                &mut size,
                &mut count,
                Some(buffer.as_mut_ptr() as *mut PDH_FMT_COUNTERVALUE_ITEM_W),
            );
            if status != 0 || count == 0 {
                return Vec::new();
            }
            let items = std::slice::from_raw_parts(
                buffer.as_ptr() as *const PDH_FMT_COUNTERVALUE_ITEM_W,
                count as usize,
            );
            let mut out = Vec::with_capacity(items.len());
            for item in items {
                if item.FmtValue.CStatus != 0 {
                    continue; // PDH_CSTATUS_VALID_DATA
                }
                let name = item.szName.to_string().unwrap_or_default();
                let value = item.FmtValue.Anonymous.doubleValue;
                if value.is_finite() {
                    out.push((name, value));
                }
            }
            out
        }
    }

    fn sample(&mut self) -> Option<SystemStats> {
        let mut stats = SystemStats::default();

        // CPU: kernel time already includes idle
        if let Some((idle, kernel, user)) = cpu_times() {
            if let Some((p_idle, p_kernel, p_user)) = self.prev_cpu {
                let d_idle = idle.saturating_sub(p_idle);
                let d_total = kernel
                    .saturating_sub(p_kernel)
                    .saturating_add(user.saturating_sub(p_user));
                if d_total > 0 {
                    let busy = d_total.saturating_sub(d_idle);
                    stats.cpu_pct = (busy as f64 / d_total as f64 * 100.0).clamp(0.0, 100.0) as f32;
                }
            }
            self.prev_cpu = Some((idle, kernel, user));
        }

        // RAM
        let mut mem = MEMORYSTATUSEX {
            dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32,
            ..Default::default()
        };
        if unsafe { GlobalMemoryStatusEx(&mut mem) }.is_ok() {
            stats.ram_total_mb = mem.ullTotalPhys / (1024 * 1024);
            stats.ram_used_mb = mem.ullTotalPhys.saturating_sub(mem.ullAvailPhys) / (1024 * 1024);
            stats.ram_pct = mem.dwMemoryLoad as f32;
        }

        // Network (KB/s over the elapsed window)
        if let Some((down, up)) = net_totals() {
            if let Some((p_down, p_up, at)) = self.prev_net {
                let secs = at.elapsed().as_secs_f64();
                if secs > 0.05 {
                    let down_bps = down.saturating_sub(p_down) as f64 / secs;
                    let up_bps = up.saturating_sub(p_up) as f64 / secs;
                    stats.net_down_kbps = Some((down_bps / 1024.0) as f32);
                    stats.net_up_kbps = Some((up_bps / 1024.0) as f32);
                }
            }
            self.prev_net = Some((down, up, Instant::now()));
        }

        // GPU / VRAM / disk through the same PDH query
        if self.collect() {
            if let Some(counter) = self.gpu_counter {
                let items = Self::counter_array(counter);
                stats.gpu_pct = gpu_percent(&items).or(Some(0.0));
            }
            if let Some(counter) = self.vram_counter {
                let items = Self::counter_array(counter);
                if !items.is_empty() {
                    let bytes: f64 = items.iter().map(|(_, v)| v.max(0.0)).sum();
                    stats.vram_used_mb = Some((bytes / (1024.0 * 1024.0)) as u64);
                    stats.vram_total_mb = self.vram_total_mb;
                }
            }
            if let Some(counter) = self.disk_counter {
                let items = Self::counter_array(counter);
                if let Some((_, idle)) = items.first() {
                    stats.disk_pct = Some((100.0 - idle).clamp(0.0, 100.0) as f32);
                }
            }
        }

        if let Some(nvml) = &self.nvml {
            stats.gpu_temp_c = nvml.temperature();
        }

        Some(stats)
    }
}

impl Drop for Sampler {
    fn drop(&mut self) {
        if let Some(query) = self.query.take() {
            unsafe {
                PdhCloseQuery(query);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Manual probe: cargo test --offline samples -- --nocapture
    #[test]
    fn samples() {
        let mut sampler = Sampler::new();
        sampler.begin();
        std::thread::sleep(Duration::from_millis(700));
        let stats = sampler.sample().expect("sample");
        println!("{:?}", stats);
    }
}
