use serde::{Deserialize, Serialize};
use std::fs;

use crate::app_paths::AppPaths;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NlbnPathMode {
    #[default]
    Auto,
    ProjectRelative,
    LibraryRelative,
}

#[cfg(test)]
mod tests {
    use super::AppConfig;
    use crate::app_paths::AppPaths;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "seex_config_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn loads_legacy_export_config_from_executable_dir() {
        let root = test_root("legacy_json");
        let legacy_dir = root.join("legacy");
        fs::create_dir_all(&legacy_dir).unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            Some(legacy_dir.clone()),
        );

        fs::write(
            legacy_dir.join("export_config.json"),
            r#"{
  "nlbn": { "output_path": "/tmp/nlbn", "show_terminal": false, "parallel": 8, "path_mode": "project_relative" },
  "syft": {
    "output_path": "/tmp/npnp",
    "mode": "pcblib",
    "merge": true,
    "append": false,
    "library_name": "LegacyLib",
    "parallel": 2,
    "continue_on_error": false,
    "force": true
  },
  "monitor": {
    "history_save_path": "/tmp/history.txt",
    "matched_save_path": "/tmp/matched.txt"
  }
}"#,
        )
        .unwrap();

        let config = AppConfig::load(&paths);
        assert_eq!(config.nlbn.output_path, "/tmp/nlbn");
        assert!(!config.nlbn.show_terminal);
        assert_eq!(config.nlbn.parallel, 8);
        assert_eq!(config.npnp.output_path, "/tmp/npnp");
        assert_eq!(config.npnp.mode, "pcblib");
        assert!(config.npnp.merge);
        assert_eq!(config.monitor.history_save_path, "/tmp/history.txt");

        let _ = fs::remove_dir_all(root);
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub nlbn: NlbnConfig,
    #[serde(alias = "syft")]
    pub npnp: NpnpConfig,
    pub monitor: MonitorConfig,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            nlbn: NlbnConfig::default(),
            npnp: NpnpConfig::default(),
            monitor: MonitorConfig::default(),
        }
    }
}

#[derive(Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct MonitorConfig {
    pub history_save_path: String,
    pub matched_save_path: String,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NlbnConfig {
    pub output_path: String,
    pub show_terminal: bool,
    pub parallel: usize,
    pub path_mode: NlbnPathMode,
}

impl Default for NlbnConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            show_terminal: true,
            parallel: 4,
            path_mode: NlbnPathMode::Auto,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NpnpConfig {
    pub output_path: String,
    pub mode: String,
    pub merge: bool,
    pub append: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub force: bool,
}

impl Default for NpnpConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            mode: "full".to_string(),
            merge: false,
            append: false,
            library_name: "SeExMerged".to_string(),
            parallel: 4,
            continue_on_error: true,
            force: false,
        }
    }
}

impl AppConfig {
    pub fn load(paths: &AppPaths) -> Self {
        let path = paths.config_file();
        match fs::read_to_string(&path) {
            Ok(content) => {
                serde_json::from_str(&content).unwrap_or_else(|_| Self::with_legacy(paths))
            }
            Err(_) => Self::with_legacy(paths),
        }
    }

    pub fn save(&self, paths: &AppPaths) {
        let path = paths.config_file();
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(content) = serde_json::to_string_pretty(self) {
            let _ = fs::write(path, content);
        }
    }

    fn with_legacy(paths: &AppPaths) -> Self {
        if let Some(config) = Self::load_legacy_export_config(paths) {
            return config;
        }

        let mut cfg = Self::default();
        cfg.nlbn = NlbnConfig::load_legacy(paths);
        cfg
    }

    fn load_legacy_export_config(paths: &AppPaths) -> Option<Self> {
        let path = paths.legacy_config_file()?;
        let content = fs::read_to_string(path).ok()?;
        serde_json::from_str(&content).ok()
    }
}

impl NlbnConfig {
    fn load_legacy(paths: &AppPaths) -> Self {
        let content = paths
            .legacy_nlbn_config_file()
            .and_then(|path| fs::read_to_string(path).ok());

        let Some(content) = content else {
            return Self::default();
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
            path_mode: defaults.path_mode,
        }
    }
}
