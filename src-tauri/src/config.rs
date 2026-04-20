use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

const CONFIG_FILENAME: &str = "export_config.json";
const LEGACY_NLBN_CONFIG_FILENAME: &str = "nlbn_config.txt";

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub nlbn: NlbnConfig,
    #[serde(alias = "syft")]
    pub npnp: NpnpConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            nlbn: NlbnConfig::default(),
            npnp: NpnpConfig::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NlbnConfig {
    pub output_path: String,
    pub show_terminal: bool,
    pub parallel: usize,
}

impl Default for NlbnConfig {
    fn default() -> Self {
        Self {
            output_path: "~/lib".to_string(),
            show_terminal: true,
            parallel: 4,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NpnpConfig {
    pub output_path: String,
    pub mode: String,
    pub merge: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub force: bool,
}

impl Default for NpnpConfig {
    fn default() -> Self {
        Self {
            output_path: "npnp_export".to_string(),
            mode: "full".to_string(),
            merge: false,
            library_name: "SeExMerged".to_string(),
            parallel: 4,
            continue_on_error: true,
            force: false,
        }
    }
}

impl AppConfig {
    fn config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(CONFIG_FILENAME)))
            .unwrap_or_else(|| PathBuf::from(CONFIG_FILENAME))
    }

    fn legacy_nlbn_config_path() -> PathBuf {
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join(LEGACY_NLBN_CONFIG_FILENAME)))
            .unwrap_or_else(|| PathBuf::from(LEGACY_NLBN_CONFIG_FILENAME))
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        match fs::read_to_string(&path) {
            Ok(content) => serde_json::from_str(&content).unwrap_or_else(|_| Self::with_legacy()),
            Err(_) => Self::with_legacy(),
        }
    }

    pub fn save(&self) {
        let path = Self::config_path();
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    fn with_legacy() -> Self {
        let mut cfg = Self::default();
        cfg.nlbn = NlbnConfig::load_legacy();
        cfg
    }
}

impl NlbnConfig {
    fn load_legacy() -> Self {
        let path = AppConfig::legacy_nlbn_config_path();
        let content = match fs::read_to_string(&path) {
            Ok(c) => c,
            Err(_) => return Self::default(),
        };
        let lines: Vec<&str> = content.lines().collect();
        let defaults = Self::default();
        let output_path = if !lines.is_empty() && !lines[0].is_empty() {
            lines[0].to_string()
        } else {
            defaults.output_path
        };
        let show_terminal = if lines.len() >= 2 {
            lines[1] == "true"
        } else {
            defaults.show_terminal
        };
        let parallel = if lines.len() >= 3 {
            lines[2]
                .trim()
                .parse::<usize>()
                .ok()
                .filter(|value| *value >= 1)
                .unwrap_or(defaults.parallel)
        } else {
            defaults.parallel
        };
        Self {
            output_path,
            show_terminal,
            parallel,
        }
    }
}
