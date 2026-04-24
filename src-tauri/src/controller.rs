use chrono::Local;
use regex::Regex;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use crate::app_paths::AppPaths;
use crate::config::{AppConfig, MonitorConfig, NlbnConfig, NpnpConfig};
use crate::imported_symbols;
use crate::monitor::{MonitorHandle, MonitorState, NotifyFn};
use crate::{nlbn, npnp};

const DEFAULT_KEYWORD: &str =
    "regex:\u{7f16}\u{53f7}[\u{ff1a}:]\\s*(C\\d+)||regex:(?m)^(C\\d{3,})$";

struct ImportedPartsParseResult {
    unique_parts: Vec<String>,
    parsed_count: usize,
    duplicate_count: usize,
    invalid_line_count: usize,
}

pub struct AppController {
    state: Arc<Mutex<MonitorState>>,
    paths: AppPaths,
    _monitor: MonitorHandle,
}

impl AppController {
    pub fn new(paths: AppPaths, notifier: NotifyFn) -> Result<Self, String> {
        let config = AppConfig::load(&paths);
        let state = Arc::new(Mutex::new(MonitorState::new(&paths)));

        if let Ok(mut s) = state.lock() {
            apply_config(&mut s, &config, &paths);
            s.set_keyword(DEFAULT_KEYWORD.to_string());
        }

        if let Ok(s) = state.lock() {
            snapshot_config(&s).save(&paths);
        }

        let monitor = MonitorHandle::spawn_with_callback(Arc::clone(&state), notifier);

        Ok(Self {
            state,
            paths,
            _monitor: monitor,
        })
    }

    pub fn new_native() -> Result<Self, String> {
        Self::new(AppPaths::resolve_native()?, Arc::new(|| {}))
    }

    pub fn state(&self) -> &Arc<Mutex<MonitorState>> {
        &self.state
    }

    pub fn paths(&self) -> &AppPaths {
        &self.paths
    }

    pub fn save_config(&self) {
        if let Ok(state) = self.state.lock() {
            snapshot_config(&state).save(&self.paths);
        }
    }

    pub fn save_history(&self) -> String {
        let (path, entries) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            if m.history.is_empty() {
                return "No history to save".to_string();
            }
            (PathBuf::from(&m.history_save_path), m.history.clone())
        };

        if let Err(err) = ensure_parent_dir(&path) {
            return format!("Save failed: {}", err);
        }
        let file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(err) => return format!("Save failed: {}", err),
        };
        let mut writer = std::io::BufWriter::new(file);
        for (time, content) in &entries {
            if let Err(err) = writeln!(writer, "[{}] {}", time, content) {
                return format!("Save failed: {}", err);
            }
        }
        if let Err(err) = writer.flush() {
            return format!("Save failed: {}", err);
        }
        format!("Saved to {}", path.display())
    }

    pub fn save_matched(&self) -> String {
        let (path, entries) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            if m.matched.is_empty() {
                return "No matched results to export".to_string();
            }
            (PathBuf::from(&m.matched_save_path), m.matched.clone())
        };

        if let Err(err) = ensure_parent_dir(&path) {
            return format!("Export failed: {}", err);
        }
        let file = match std::fs::File::create(&path) {
            Ok(file) => file,
            Err(err) => return format!("Export failed: {}", err),
        };
        let mut writer = std::io::BufWriter::new(file);
        for (_, extracted) in &entries {
            if let Err(err) = writeln!(writer, "{}", extracted) {
                return format!("Export failed: {}", err);
            }
        }
        if let Err(err) = writer.flush() {
            return format!("Export failed: {}", err);
        }
        format!("Exported to {}", path.display())
    }

    pub fn save_imported_parts(&self) -> String {
        let (output_path, save_path) = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            (
                m.nlbn_output_path.clone(),
                PathBuf::from(&m.imported_parts_save_path),
            )
        };

        let imported = match imported_symbols::load_imported_symbols(Path::new(&output_path)) {
            Ok(imported) => imported,
            Err(err) => return format!("Export failed: {}", err),
        };
        let parts = imported_symbols::unique_lcsc_parts(&imported.items);
        if parts.is_empty() {
            return "No imported LCSC Part values to export".to_string();
        }

        if let Err(err) = ensure_parent_dir(&save_path) {
            return format!("Export failed: {}", err);
        }
        let file = match std::fs::File::create(&save_path) {
            Ok(file) => file,
            Err(err) => return format!("Export failed: {}", err),
        };
        let mut writer = std::io::BufWriter::new(file);
        for part in &parts {
            if let Err(err) = writeln!(writer, "{}", part) {
                return format!("Export failed: {}", err);
            }
        }
        if let Err(err) = writer.flush() {
            return format!("Export failed: {}", err);
        }
        format!("Exported to {}", save_path.display())
    }

    pub fn import_imported_parts(&self) -> String {
        let import_path = {
            let Ok(m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            PathBuf::from(&m.imported_parts_save_path)
        };

        let content = match std::fs::read_to_string(&import_path) {
            Ok(content) => content,
            Err(err) => return format!("Import failed: {} ({})", import_path.display(), err),
        };
        let parsed = parse_imported_parts_text(&content);
        if parsed.unique_parts.is_empty() {
            return format!("No LCSC parts found in {}", import_path.display());
        }

        let (added, already_queued, matched_count) = {
            let Ok(mut m) = self.state.lock() else {
                return "State lock failed".to_string();
            };
            let timestamp = Local::now().format("%H:%M:%S").to_string();
            let merge = m.merge_matched_ids(parsed.unique_parts.iter().cloned(), timestamp);
            m.add_debug_log(format!(
                "Imported {} new matched parts from {}",
                merge.added,
                import_path.display()
            ));
            (merge.added, merge.already_present, m.matched.len())
        };

        format!(
            "Imported {} LCSC part(s) from {} ({} match(es), {} unique, {} duplicate hit(s) in file, {} line(s) without a C-code, {} already queued, matched queue now has {} item(s))",
            added,
            import_path.display(),
            parsed.parsed_count,
            parsed.unique_parts.len(),
            parsed.duplicate_count,
            parsed.invalid_line_count,
            already_queued,
            matched_count
        )
    }

    pub fn spawn_nlbn_export(&self, callbacks: nlbn::ExportCallbacks) -> String {
        let request = if let Ok(m) = self.state.lock() {
            let ids = m.get_unique_ids();
            if ids.is_empty() {
                return "No matched results to export".to_string();
            }
            nlbn::ExportRequest {
                ids,
                output_path: m.nlbn_output_path.clone(),
                show_terminal: m.nlbn_show_terminal,
                parallel: m.nlbn_parallel,
                path_mode: m.nlbn_path_mode,
                overwrite: m.nlbn_overwrite,
                symbol_fill_color: m.nlbn_symbol_fill_color.clone(),
            }
        } else {
            return "State lock failed".to_string();
        };

        match nlbn::spawn_export(
            Arc::clone(&self.state),
            request,
            self.paths.clone(),
            callbacks,
        ) {
            Ok(()) => "Export started".to_string(),
            Err(message) => message,
        }
    }

    pub fn spawn_npnp_export(&self, callbacks: npnp::ExportCallbacks) -> String {
        let request = if let Ok(m) = self.state.lock() {
            let ids = m.get_unique_ids();
            if ids.is_empty() {
                return "No matched results to export".to_string();
            }
            npnp::ExportRequest {
                ids,
                output_path: m.npnp_output_path.clone(),
                mode: m.npnp_mode.clone(),
                merge: m.npnp_merge,
                append: m.npnp_append,
                library_name: m.npnp_library_name.clone(),
                parallel: m.npnp_parallel,
                continue_on_error: m.npnp_continue_on_error,
                force: m.npnp_force,
            }
        } else {
            return "State lock failed".to_string();
        };

        npnp::spawn_export(
            Arc::clone(&self.state),
            request,
            self.paths.clone(),
            callbacks,
        );
        "Export started".to_string()
    }
}

pub fn snapshot_config(state: &MonitorState) -> AppConfig {
    AppConfig {
        nlbn: NlbnConfig {
            output_path: state.nlbn_output_path.clone(),
            show_terminal: state.nlbn_show_terminal,
            parallel: state.nlbn_parallel,
            path_mode: state.nlbn_path_mode,
            overwrite: state.nlbn_overwrite,
            symbol_fill_color: state.nlbn_symbol_fill_color.clone(),
        },
        npnp: NpnpConfig {
            output_path: state.npnp_output_path.clone(),
            mode: state.npnp_mode.clone(),
            merge: state.npnp_merge,
            append: state.npnp_append,
            library_name: state.npnp_library_name.clone(),
            parallel: state.npnp_parallel,
            continue_on_error: state.npnp_continue_on_error,
            force: state.npnp_force,
        },
        monitor: MonitorConfig {
            history_save_path: state.history_save_path.clone(),
            matched_save_path: state.matched_save_path.clone(),
            imported_parts_save_path: state.imported_parts_save_path.clone(),
        },
    }
}

fn apply_config(state: &mut MonitorState, config: &AppConfig, paths: &AppPaths) {
    state.set_nlbn_output_path(config.nlbn.output_path.clone());
    state.nlbn_show_terminal = config.nlbn.show_terminal;
    state.set_nlbn_parallel(config.nlbn.parallel);
    state.set_nlbn_path_mode(config.nlbn.path_mode);
    state.set_nlbn_overwrite(config.nlbn.overwrite);
    state.set_nlbn_symbol_fill_color(config.nlbn.symbol_fill_color.clone());
    state.set_npnp_output_path(config.npnp.output_path.clone());
    state.set_npnp_mode(config.npnp.mode.clone());
    state.set_npnp_merge(config.npnp.merge);
    state.set_npnp_append(config.npnp.append);
    state.set_npnp_library_name(config.npnp.library_name.clone());
    state.set_npnp_parallel(config.npnp.parallel);
    state.set_npnp_continue_on_error(config.npnp.continue_on_error);
    state.set_npnp_force(config.npnp.force);
    state.set_history_save_path(config.monitor.history_save_path.clone(), paths);
    state.set_matched_save_path(config.monitor.matched_save_path.clone(), paths);
    state.set_imported_parts_save_path(config.monitor.imported_parts_save_path.clone(), paths);
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

fn parse_imported_parts_text(content: &str) -> ImportedPartsParseResult {
    let regex = Regex::new(r"(?i)c\d+").expect("import regex should compile");
    let mut seen = std::collections::HashSet::new();
    let mut unique_parts = Vec::new();
    let mut parsed_count = 0usize;
    let mut invalid_line_count = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let mut matched = false;
        for capture in regex.find_iter(trimmed) {
            let start = capture.start();
            let end = capture.end();
            let before = trimmed[..start].chars().next_back();
            let after = trimmed[end..].chars().next();

            if before.is_some_and(is_lcsc_boundary_blocker)
                || after.is_some_and(is_lcsc_boundary_blocker)
            {
                continue;
            }

            matched = true;
            parsed_count += 1;
            let normalized = capture.as_str().to_ascii_uppercase();
            if seen.insert(normalized.clone()) {
                unique_parts.push(normalized);
            }
        }

        if !matched {
            invalid_line_count += 1;
        }
    }

    ImportedPartsParseResult {
        duplicate_count: parsed_count.saturating_sub(unique_parts.len()),
        unique_parts,
        parsed_count,
        invalid_line_count,
    }
}

fn is_lcsc_boundary_blocker(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

#[cfg(test)]
mod tests {
    use super::{AppController, parse_imported_parts_text};
    use crate::app_paths::AppPaths;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn test_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        std::env::temp_dir().join(format!(
            "seex_controller_tests_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn native_controller_snapshot_uses_default_keyword() {
        let root = test_root("keyword");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");
        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert!(state.keyword.contains("regex"));
        drop(state);
        let _ = fs::remove_dir_all(root);
    }

    fn write_symbol_library(root: &PathBuf, name: &str, body: &str) {
        fs::create_dir_all(root).unwrap();
        fs::write(root.join(name), body).unwrap();
    }

    #[test]
    fn save_imported_parts_writes_unique_ids_one_per_line() {
        let root = test_root("save_imported_parts");
        let export_dir = root.join("nlbn_export");
        write_symbol_library(
            &export_dir,
            "parts.kicad_sym",
            "(kicad_symbol_lib\n  (version 20211014)\n  (generator seex-test)\n  (symbol \"Alpha\"\n    (property \"LCSC Part\" \"C123\" (id 5) (at 0 0 0))\n    (symbol \"Alpha_0_1\")\n  )\n  (symbol \"Beta\"\n    (property \"LCSC Part\" \"C123\" (id 5) (at 0 0 0))\n    (symbol \"Beta_0_1\")\n  )\n  (symbol \"Gamma\"\n    (property \"LCSC Part\" \"C456\" (id 5) (at 0 0 0))\n    (symbol \"Gamma_0_1\")\n  )\n)\n",
        );

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");
        let save_path = root.join("out").join("imported.txt");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_nlbn_output_path(export_dir.display().to_string());
            state.set_imported_parts_save_path(save_path.display().to_string(), controller.paths());
        }

        let result = controller.save_imported_parts();
        assert_eq!(result, format!("Exported to {}", save_path.display()));
        assert_eq!(fs::read_to_string(&save_path).unwrap(), "C123\nC456\n");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn save_imported_parts_reports_empty_when_scan_has_no_parts() {
        let root = test_root("save_imported_parts_empty");
        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        let result = controller.save_imported_parts();
        assert_eq!(result, "No imported LCSC Part values to export");

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parse_imported_parts_text_reports_unique_duplicates_and_invalid_lines() {
        let parsed = parse_imported_parts_text("C123\njunk\nc123 C456\n\nC456\nXC789\n");

        assert_eq!(parsed.unique_parts, vec!["C123".to_string(), "C456".to_string()]);
        assert_eq!(parsed.parsed_count, 4);
        assert_eq!(parsed.duplicate_count, 2);
        assert_eq!(parsed.invalid_line_count, 2);
    }

    #[test]
    fn import_imported_parts_merges_ids_into_matched_queue() {
        let root = test_root("import_imported_parts");
        let import_path = root.join("incoming").join("parts.txt");
        fs::create_dir_all(import_path.parent().unwrap()).unwrap();
        fs::write(&import_path, "C123\ninvalid\nc123\nC456\n").unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_imported_parts_save_path(import_path.display().to_string(), controller.paths());
            state.matched.push(("10:00:00".to_string(), "C456".to_string()));
        }

        let result = controller.import_imported_parts();
        assert_eq!(
            result,
            format!(
                "Imported 1 LCSC part(s) from {} (3 match(es), 2 unique, 1 duplicate hit(s) in file, 1 line(s) without a C-code, 1 already queued, matched queue now has 2 item(s))",
                import_path.display()
            )
        );

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(state.get_unique_ids(), vec!["C123".to_string(), "C456".to_string()]);

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn import_imported_parts_keeps_large_batches_in_matched_queue() {
        let root = test_root("import_imported_parts_large");
        let import_path = root.join("incoming").join("many-parts.txt");
        fs::create_dir_all(import_path.parent().unwrap()).unwrap();
        let content = (1..=150)
            .map(|value| format!("C{value}"))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(&import_path, content).unwrap();

        let paths = AppPaths::for_test(
            root.join("config"),
            root.join("data"),
            root.join("cache"),
            None,
        );
        let controller =
            AppController::new(paths, Arc::new(|| {})).expect("controller should initialize");

        {
            let mut state = controller
                .state()
                .lock()
                .expect("state lock should succeed");
            state.set_imported_parts_save_path(import_path.display().to_string(), controller.paths());
        }

        let result = controller.import_imported_parts();
        assert!(result.contains("Imported 150 LCSC part(s)"));

        let state = controller
            .state()
            .lock()
            .expect("state lock should succeed");
        assert_eq!(state.matched.len(), 150);
        assert_eq!(
            state.get_unique_ids().first().map(String::as_str),
            Some("C1")
        );

        let _ = fs::remove_dir_all(root);
    }
}
