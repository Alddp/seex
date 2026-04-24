use serde::{Deserialize, Serialize};
use std::fs;

use crate::app_paths::AppPaths;

fn default_true() -> bool {
    true
}

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
            r##"{
  "nlbn": {
    "output_path": "/tmp/nlbn",
    "show_terminal": false,
    "parallel": 8,
    "path_mode": "project_relative",
    "overwrite": true,
    "symbol_fill_color": "#005C8FCC"
  },
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
    "always_on_top": true,
    "history_save_path": "/tmp/history.txt",
    "matched_save_path": "/tmp/matched.txt",
    "imported_parts_save_path": "/tmp/imported-parts.txt"
  }
}"##,
        )
        .unwrap();

        let config = AppConfig::load(&paths);
        assert_eq!(config.nlbn.output_path, "/tmp/nlbn");
        assert!(!config.nlbn.show_terminal);
        assert_eq!(config.nlbn.parallel, 8);
        assert!(config.nlbn.export_symbol);
        assert!(config.nlbn.export_footprint);
        assert!(config.nlbn.export_model_3d);
        assert!(config.nlbn.overwrite_symbol);
        assert!(config.nlbn.overwrite_footprint);
        assert!(config.nlbn.overwrite_model_3d);
        assert_eq!(config.nlbn.symbol_fill_color.as_deref(), Some("#005C8FCC"));
        assert_eq!(config.npnp.output_path, "/tmp/npnp");
        assert_eq!(config.npnp.mode, "pcblib");
        assert!(config.npnp.merge);
        assert!(config.monitor.always_on_top);
        assert_eq!(config.monitor.history_save_path, "/tmp/history.txt");
        assert_eq!(
            config.monitor.imported_parts_save_path,
            "/tmp/imported-parts.txt"
        );

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_omits_unset_symbol_fill_color() {
        let root = test_root("save_without_fill_color");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );

        AppConfig::default().save(&paths);

        let saved = fs::read_to_string(paths.config_file()).unwrap();
        assert!(!saved.contains("symbol_fill_color"));
        assert!(!saved.contains("\"overwrite\":"));
        assert!(saved.contains("\"export_symbol\": true"));
        assert!(saved.contains("\"always_on_top\""));
        assert!(saved.contains("\"imported_parts_save_path\""));

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
    pub always_on_top: bool,
    pub history_save_path: String,
    pub matched_save_path: String,
    pub imported_parts_save_path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_width: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window_height: Option<u32>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct NlbnConfig {
    pub output_path: String,
    pub show_terminal: bool,
    pub parallel: usize,
    pub path_mode: NlbnPathMode,
    #[serde(default = "default_true")]
    pub export_symbol: bool,
    #[serde(default = "default_true")]
    pub export_footprint: bool,
    #[serde(default = "default_true")]
    pub export_model_3d: bool,
    pub overwrite_symbol: bool,
    pub overwrite_footprint: bool,
    pub overwrite_model_3d: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symbol_fill_color: Option<String>,
    #[serde(default, rename = "overwrite", skip_serializing)]
    pub(crate) legacy_overwrite: Option<bool>,
}

impl Default for NlbnConfig {
    fn default() -> Self {
        Self {
            output_path: String::new(),
            show_terminal: true,
            parallel: 4,
            path_mode: NlbnPathMode::Auto,
            export_symbol: true,
            export_footprint: true,
            export_model_3d: true,
            overwrite_symbol: false,
            overwrite_footprint: false,
            overwrite_model_3d: false,
            symbol_fill_color: None,
            legacy_overwrite: None,
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
                serde_json::from_str(&content)
                    .map(Self::normalize)
                    .unwrap_or_else(|_| Self::with_legacy(paths))
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
        serde_json::from_str(&content).ok().map(Self::normalize)
    }

    fn normalize(mut self) -> Self {
        self.nlbn.normalize_legacy();
        self
    }
}

impl NlbnConfig {
    fn normalize_legacy(&mut self) {
        if self.legacy_overwrite == Some(true) {
            self.overwrite_symbol = true;
            self.overwrite_footprint = true;
            self.overwrite_model_3d = true;
        }
        self.legacy_overwrite = None;
    }

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
            export_symbol: defaults.export_symbol,
            export_footprint: defaults.export_footprint,
            export_model_3d: defaults.export_model_3d,
            overwrite_symbol: defaults.overwrite_symbol,
            overwrite_footprint: defaults.overwrite_footprint,
            overwrite_model_3d: defaults.overwrite_model_3d,
            symbol_fill_color: defaults.symbol_fill_color,
            legacy_overwrite: None,
        }
    }
}
