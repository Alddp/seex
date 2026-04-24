use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CONFIG_FILENAME: &str = "export_config.json";
const LEGACY_NLBN_CONFIG_FILENAME: &str = "nlbn_config.txt";

#[derive(Clone, Debug)]
pub struct AppPaths {
    config_dir: PathBuf,
    data_dir: PathBuf,
    cache_dir: PathBuf,
    legacy_exe_dir: Option<PathBuf>,
}

impl AppPaths {
    pub fn resolve(app_handle: &AppHandle) -> Result<Self, String> {
        let resolver = app_handle.path();
        let config_dir = resolver
            .app_config_dir()
            .map_err(|err| format!("failed to resolve app config dir: {err}"))?;
        let data_dir = resolver
            .app_data_dir()
            .map_err(|err| format!("failed to resolve app data dir: {err}"))?;
        let cache_dir = resolver
            .app_cache_dir()
            .map_err(|err| format!("failed to resolve app cache dir: {err}"))?;

        for dir in [&config_dir, &data_dir, &cache_dir] {
            fs::create_dir_all(dir)
                .map_err(|err| format!("failed to create {}: {err}", dir.display()))?;
        }

        Ok(Self {
            config_dir,
            data_dir,
            cache_dir,
            legacy_exe_dir: std::env::current_exe()
                .ok()
                .and_then(|path| path.parent().map(Path::to_path_buf)),
        })
    }

    pub fn config_file(&self) -> PathBuf {
        self.config_dir.join(CONFIG_FILENAME)
    }

    pub fn legacy_config_file(&self) -> Option<PathBuf> {
        self.legacy_exe_dir
            .as_ref()
            .map(|dir| dir.join(CONFIG_FILENAME))
    }

    pub fn legacy_nlbn_config_file(&self) -> Option<PathBuf> {
        self.legacy_exe_dir
            .as_ref()
            .map(|dir| dir.join(LEGACY_NLBN_CONFIG_FILENAME))
    }

    pub fn default_history_file(&self) -> PathBuf {
        self.data_dir.join("history.txt")
    }

    pub fn default_matched_file(&self) -> PathBuf {
        self.data_dir.join("matched.txt")
    }

    pub fn default_nlbn_output_dir(&self) -> PathBuf {
        self.data_dir.join("nlbn_export")
    }

    pub fn default_npnp_output_dir(&self) -> PathBuf {
        self.data_dir.join("npnp_export")
    }

    pub fn default_history_save_path_string(&self) -> String {
        self.default_history_file().display().to_string()
    }

    pub fn default_matched_save_path_string(&self) -> String {
        self.default_matched_file().display().to_string()
    }

    pub fn default_nlbn_output_dir_string(&self) -> String {
        self.default_nlbn_output_dir().display().to_string()
    }

    pub fn default_npnp_output_dir_string(&self) -> String {
        self.default_npnp_output_dir().display().to_string()
    }

    pub fn default_nlbn_output_path_string(&self) -> String {
        self.default_nlbn_output_dir_string()
    }

    pub fn default_npnp_output_path_string(&self) -> String {
        self.default_npnp_output_dir_string()
    }

    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }

    pub fn cache_file(&self, prefix: &str, extension: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        self.cache_dir.join(format!(
            "{prefix}_{}_{}.{}",
            std::process::id(),
            stamp,
            extension
        ))
    }

    pub fn resolve_history_save_path(&self, path: &str) -> String {
        self.resolve_monitor_save_path(path, "history.txt", self.default_history_file())
    }

    pub fn resolve_matched_save_path(&self, path: &str) -> String {
        self.resolve_monitor_save_path(path, "matched.txt", self.default_matched_file())
    }

    pub fn resolve_nlbn_output_path(&self, path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            self.default_nlbn_output_dir_string()
        } else {
            trimmed.to_string()
        }
    }

    pub fn resolve_npnp_output_path(&self, path: &str) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() {
            self.default_npnp_output_dir_string()
        } else {
            trimmed.to_string()
        }
    }

    fn resolve_monitor_save_path(&self, path: &str, legacy_name: &str, default: PathBuf) -> String {
        let trimmed = path.trim();
        if trimmed.is_empty() || self.is_legacy_monitor_default(trimmed, legacy_name) {
            default.display().to_string()
        } else {
            trimmed.to_string()
        }
    }

    fn is_legacy_monitor_default(&self, path: &str, legacy_name: &str) -> bool {
        if path == legacy_name {
            return true;
        }

        self.legacy_exe_dir
            .as_ref()
            .map(|dir| dir.join(legacy_name))
            .is_some_and(|legacy_path| legacy_path == PathBuf::from(path))
    }

    #[cfg(test)]
    pub fn for_test(
        config_dir: PathBuf,
        data_dir: PathBuf,
        cache_dir: PathBuf,
        legacy_exe_dir: Option<PathBuf>,
    ) -> Self {
        Self {
            config_dir,
            data_dir,
            cache_dir,
            legacy_exe_dir,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::AppPaths;
    use std::path::PathBuf;

    #[test]
    fn migrates_legacy_monitor_defaults_to_app_data() {
        let paths = AppPaths::for_test(
            PathBuf::from("/tmp/seex-config"),
            PathBuf::from("/tmp/seex-data"),
            PathBuf::from("/tmp/seex-cache"),
            Some(PathBuf::from("/Applications/SeEx.app/Contents/MacOS")),
        );

        assert_eq!(
            paths.resolve_history_save_path("history.txt"),
            "/tmp/seex-data/history.txt"
        );
        assert_eq!(
            paths.resolve_matched_save_path("/Applications/SeEx.app/Contents/MacOS/matched.txt"),
            "/tmp/seex-data/matched.txt"
        );
        assert_eq!(
            paths.resolve_history_save_path("/tmp/custom-history.txt"),
            "/tmp/custom-history.txt"
        );
    }
}
