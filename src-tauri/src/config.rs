use serde::{Deserialize, Serialize};
use std::sync::OnceLock;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub auto_start: bool,
    pub hidden_devices: Vec<String>,
    pub hidden_groups: Vec<String>,
    pub device_names: std::collections::HashMap<String, String>,
    pub device_groups: std::collections::HashMap<String, String>,
    pub filter_enabled: bool,
    pub filter_regex: String,
    pub dedup_devices: bool,
    pub show_unnamed_bt: bool,
    pub use_system_bt: bool,
    #[serde(default)]
    pub tray_devices: Vec<String>,
    #[serde(default)]
    pub hidden_audio_devices: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_start: false,
            hidden_devices: vec![],
            hidden_groups: vec!["Battery".to_string(), "Monitor".to_string()],
            device_names: std::collections::HashMap::new(),
            device_groups: std::collections::HashMap::new(),
            filter_enabled: true,
            filter_regex: Self::default_filter_regex(),
            dedup_devices: true,
            show_unnamed_bt: false,
            use_system_bt: false,
            tray_devices: vec![],
            hidden_audio_devices: vec![],
        }
    }
}

impl Config {
    /// Combined regex for all device exclusion filters (case-insensitive)
    pub fn default_filter_regex() -> String {
        "Virtual|虚拟|^HID|Audio Device|Audio 设备|Hands-Free|A2DP|gvinput Device|英特尔\\(R\\)".to_string()
    }
}

static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

fn config_path() -> std::path::PathBuf {
    crate::process::exe_dir().join("config.toml")
}

fn load_config() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

fn save_config(config: &Config) {
    let path = config_path();
    if let Ok(content) = toml::to_string_pretty(config) {
        use std::io::Write;
        if let Ok(mut f) = std::fs::File::create(&path) {
            let _ = f.write_all(content.as_bytes());
        }
    }
}

pub fn init_config() {
    let config = load_config();
    CONFIG.set(Mutex::new(config)).ok();
}

pub fn with_config<F, R>(f: F) -> R
where
    F: FnOnce(&Config) -> R,
{
    let guard = CONFIG.get().expect("Config not initialized").lock().unwrap_or_else(|e| e.into_inner());
    f(&guard)
}

pub fn with_config_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Config) -> R,
{
    let mut guard = CONFIG.get().expect("Config not initialized").lock().unwrap_or_else(|e| e.into_inner());
    let result = f(&mut guard);
    save_config(&guard);
    result
}
