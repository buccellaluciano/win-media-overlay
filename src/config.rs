use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

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
    #[serde(default)]
    pub hover_reveal: bool, // Dynamic Island: hidden at top-center, reveal on mouse hover
    #[serde(default = "d_true")]
    pub app_volume: bool, // Slider controls the app's audio session (mixer); falls back to system
    #[serde(default = "d_true")]
    pub draggable: bool, // Allow dragging the window in any position mode
}

fn d_mods() -> Vec<String> { vec!["Control".to_string(), "Shift".to_string()] }
fn d_key() -> String { "Space".to_string() }
fn d_preset() -> String { "top-center".to_string() }
fn d_custom() -> i32 { 100 }
fn d_true() -> bool { true }
fn d_step() -> f64 { 0.05 }

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
            hover_reveal: false,
            app_volume: true,
            draggable: true,
        }
    }
}

pub fn get_config_path() -> PathBuf {
    if let Ok(appdata) = std::env::var("APPDATA") {
        let dir = PathBuf::from(appdata).join("MediaLiquidGlass");
        let _ = fs::create_dir_all(&dir);
        dir.join("config.json")
    } else {
        PathBuf::from("config.json")
    }
}

pub fn load_config() -> AppConfig {
    let path = get_config_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(config) = serde_json::from_str::<AppConfig>(&content) {
                return config;
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
