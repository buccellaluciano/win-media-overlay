use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    #[serde(default = "d_mods")]
    pub hotkey_modifiers: Vec<String>, // e.g. ["Control", "Shift"]
    #[serde(default = "d_key")]
    pub hotkey_key: String,            // e.g. "Space"
    #[serde(default = "d_preset")]
    pub position_preset: String,       // "top-center", "bottom-right", "center", "custom"
    #[serde(default = "d_custom")]
    pub custom_x: i32,
    #[serde(default = "d_custom")]
    pub custom_y: i32,
    #[serde(default)]
    pub auto_hide_on_blur: bool,
    #[serde(default = "d_true")]
    pub ambient_glow: bool,
    #[serde(default = "d_step")]
    pub volume_step: f64,
    #[serde(default = "d_true")]
    pub hover_reveal: bool, // Dynamic Island: hidden at top-center, reveal on mouse hover
    #[serde(default = "d_true")]
    pub app_volume: bool, // Slider controls the app's audio session (mixer); falls back to system
    #[serde(default = "d_true")]
    pub draggable: bool, // Allow dragging the window in any position mode
    #[serde(default = "d_true")]
    pub respect_fullscreen: bool, // true: stay on top, but step aside while a fullscreen app is in front
    #[serde(default)]
    pub lyrics_enabled: bool, // opt-in: fetch synced lyrics from LRCLIB and show them under the player
    #[serde(default = "d_true")]
    pub island_enabled: bool, // real Dynamic Island: compact/activity/expanded states
    #[serde(default = "d_true")]
    pub island_always_visible: bool, // compact visible even without a media session
    #[serde(default = "d_true")]
    pub island_activities: bool, // transient notifications on track/source changes
    #[serde(default = "d_expand")]
    pub hover_expand_delay_ms: u32, // hover time before compact expands, 0..=1500
    #[serde(default)]
    pub stats_enabled: bool, // opt-in system stats sampler
    #[serde(default = "d_stats_ms")]
    pub stats_interval_ms: u32,
    #[serde(default = "d_stats_items")]
    pub stats_items: Vec<String>,
    #[serde(default)]
    pub stats_in_island: bool,
    #[serde(default)]
    pub stats_alerts: bool,
    #[serde(default = "d_theme")]
    pub theme: String,
    #[serde(default = "d_true")]
    pub accent_from_art: bool,
    #[serde(default = "d_opacity")]
    pub card_opacity: f32,
}

pub const STAT_ITEMS: [&str; 7] = ["cpu", "gpu", "ram", "vram", "net", "disk", "gpu_temp"];
pub const THEMES: [&str; 9] = [
    "liquid",
    "frost",
    "oled",
    "mica",
    "neon",
    "sunset",
    "minimal",
    "contrast",
    "auto",
];

fn d_mods() -> Vec<String> { vec!["Control".to_string(), "Shift".to_string()] }
fn d_key() -> String { "Space".to_string() }
fn d_preset() -> String { "top-center".to_string() }
fn d_custom() -> i32 { 100 }
fn d_true() -> bool { true }
fn d_step() -> f64 { 0.05 }
fn d_expand() -> u32 { 250 }
fn d_stats_ms() -> u32 { 1000 }
fn d_stats_items() -> Vec<String> {
    STAT_ITEMS.iter().map(|s| s.to_string()).collect()
}
fn d_theme() -> String { "liquid".to_string() }
fn d_opacity() -> f32 { 0.74 }

impl AppConfig {
    // Clamp values from old or hand-edited configs so nothing downstream gets garbage
    fn normalize(&mut self) {
        if !self.volume_step.is_finite() {
            self.volume_step = d_step();
        }
        self.volume_step = self.volume_step.clamp(0.01, 0.2);
        self.hover_expand_delay_ms = self.hover_expand_delay_ms.min(1500);
        self.stats_interval_ms = self.stats_interval_ms.clamp(500, 5000);
        let mut items = Vec::new();
        for item in &self.stats_items {
            if STAT_ITEMS.contains(&item.as_str()) && !items.contains(item) {
                items.push(item.clone());
            }
        }
        self.stats_items = items;
        if !THEMES.contains(&self.theme.as_str()) {
            self.theme = d_theme();
        }
        if !self.card_opacity.is_finite() {
            self.card_opacity = d_opacity();
        }
        self.card_opacity = self.card_opacity.clamp(0.4, 1.0);
    }
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            hotkey_modifiers: vec!["Control".to_string(), "Shift".to_string()],
            hotkey_key: "Space".to_string(),
            position_preset: "top-center".to_string(),
            custom_x: 100,
            custom_y: 100,
            auto_hide_on_blur: false,
            ambient_glow: true,
            volume_step: 0.05,
            hover_reveal: true,
            app_volume: true,
            draggable: true,
            respect_fullscreen: true,
            lyrics_enabled: false,
            island_enabled: true,
            island_always_visible: true,
            island_activities: true,
            hover_expand_delay_ms: 250,
            stats_enabled: false,
            stats_interval_ms: 1000,
            stats_items: d_stats_items(),
            stats_in_island: false,
            stats_alerts: false,
            theme: d_theme(),
            accent_from_art: true,
            card_opacity: d_opacity(),
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(&appdata).join("WMO");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("config.json");
        // ponytail: migrate settings from the pre-rename folder once so nothing is lost
        if !path.exists() {
            let old = PathBuf::from(&appdata)
                .join("MediaLiquidGlass")
                .join("config.json");
            if old.exists() {
                let _ = fs::copy(&old, &path);
            }
        }
        path
    } else {
        PathBuf::from("config.json")
    }
}

// Never overwrite a config we failed to parse: keep it aside so the user can recover it
fn backup_corrupt_config(path: &PathBuf) {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut backup = path.clone().into_os_string();
    backup.push(format!(".bak-{}", stamp));
    let _ = fs::rename(path, PathBuf::from(backup));
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            match serde_json::from_str::<AppConfig>(&content) {
                Ok(mut config) => {
                    // Position presets were removed: the app is always the Dynamic Island (top-center).
                    let mut changed = false;
                    if config.position_preset != "top-center" {
                        // Legacy "custom" only came from the old drag logic, which broke hover-hide.
                        // Restore the island behavior when migrating away from it.
                        if config.position_preset == "custom" {
                            config.hover_reveal = true;
                        }
                        config.position_preset = "top-center".to_string();
                        changed = true;
                    }
                    let before = config.volume_step;
                    config.normalize();
                    if config.volume_step != before {
                        changed = true;
                    }
                    if changed {
                        let _ = save_config(&config);
                    }
                    return config;
                }
                Err(_) => {
                    // Broken JSON: keep a copy instead of silently replacing user settings
                    backup_corrupt_config(&path);
                }
            }
        }
    }
    let default_config = AppConfig::default();
    let _ = save_config(&default_config);
    default_config
}

pub fn save_config(config: &AppConfig) -> Result<(), String> {
    let path = get_config_path();
    let json = serde_json::to_string_pretty(config).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, json).map_err(|e| e.to_string())?;
    fs::rename(&tmp, &path).map_err(|e| e.to_string())?;
    Ok(())
}
