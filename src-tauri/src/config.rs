use serde::{Deserialize, Serialize};
use std::path::PathBuf;
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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            auto_start: false,
            hidden_devices: vec![],
            hidden_groups: vec!["Battery".to_string(), "Monitor".to_string(), "Other".to_string()],
            device_names: std::collections::HashMap::new(),
            device_groups: std::collections::HashMap::new(),
            filter_enabled: true,
            filter_regex: Self::default_filter_regex(),
        }
    }
}

impl Config {
    /// Combined regex for all device exclusion filters (case-insensitive)
    pub fn default_filter_regex() -> String {
        [
            // excluded_audio
            "high definition audio", "amd streaming", "amd high definition",
            r"intel\(r\) display audio", r"realtek\(r\) audio", "virtual", "虚拟",
            "steam streaming", "nahimic", "waves maxxaudio", "dgv1usba",
            "synaptics smartaudio", "相声", "音频端点", "avrcp", "音频网关", "音频服务",
            // excluded_input
            "hid keyboard device", "hid-compliant mouse", "标准 ps/2", "standard ps/2",
            "hid-compliant consumer", "标准键盘", "hid keyboard", "hid mouse",
            // excluded_mon
            "默认监视器", "通用即插即用监视器", "default monitor", "generic pnp monitor",
        ]
        .iter()
        .map(|s| regex::escape(s))
        .collect::<Vec<_>>()
        .join("|")
    }
}

static CONFIG: OnceLock<Mutex<Config>> = OnceLock::new();

fn config_path() -> PathBuf {
    let exe = std::env::current_exe().unwrap_or_default();
    exe.parent()
        .unwrap_or(&PathBuf::from("."))
        .join("config.toml")
}

pub fn load_config() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(content) => toml::from_str(&content).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save_config(config: &Config) {
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
    let guard = CONFIG.get().expect("Config not initialized").lock().unwrap();
    f(&guard)
}

pub fn with_config_mut<F, R>(f: F) -> R
where
    F: FnOnce(&mut Config) -> R,
{
    let mut guard = CONFIG.get().expect("Config not initialized").lock().unwrap();
    let result = f(&mut guard);
    save_config(&guard);
    result
}
