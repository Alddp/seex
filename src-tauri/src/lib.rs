mod config;
mod extract;
mod monitor;
mod nlbn;
mod npnp;

use config::{AppConfig, MonitorConfig, NlbnConfig, NpnpConfig};
use monitor::{MonitorHandle, MonitorState};
use serde::Serialize;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Manager, State};

#[derive(Serialize, Clone)]
pub struct AppState {
    pub history: Vec<(String, String)>,
    pub matched: Vec<(String, String)>,
    pub keyword: String,
    pub nlbn_output_path: String,
    pub nlbn_last_result: Option<String>,
    pub nlbn_show_terminal: bool,
    pub nlbn_parallel: usize,
    pub nlbn_running: bool,
    pub npnp_output_path: String,
    pub npnp_last_result: Option<String>,
    pub npnp_running: bool,
    pub npnp_mode: String,
    pub npnp_merge: bool,
    pub npnp_append: bool,
    pub npnp_library_name: String,
    pub npnp_parallel: usize,
    pub npnp_continue_on_error: bool,
    pub npnp_force: bool,
    pub monitoring: bool,
    pub history_count: usize,
    pub matched_count: usize,
    pub history_save_path: String,
    pub matched_save_path: String,
}

pub struct ManagedMonitor {
    pub state: Arc<Mutex<MonitorState>>,
    pub _handle: Mutex<Option<MonitorHandle>>,
}

fn snapshot_config(state: &MonitorState) -> AppConfig {
    AppConfig {
        nlbn: NlbnConfig {
            output_path: state.nlbn_output_path.clone(),
            show_terminal: state.nlbn_show_terminal,
            parallel: state.nlbn_parallel,
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
        },
    }
}

fn save_config(monitor: &State<ManagedMonitor>) {
    if let Ok(state) = monitor.state.lock() {
        snapshot_config(&state).save();
    }
}

#[tauri::command]
fn get_state(monitor: State<ManagedMonitor>) -> AppState {
    let defaults = AppConfig::default();

    if let Ok(m) = monitor.state.lock() {
        AppState {
            history_count: m.history.len(),
            matched_count: m.matched.len(),
            history: m.history.clone(),
            matched: m.matched.clone(),
            keyword: m.keyword.clone(),
            nlbn_output_path: m.nlbn_output_path.clone(),
            nlbn_last_result: m.nlbn_last_result.clone(),
            nlbn_show_terminal: m.nlbn_show_terminal,
            nlbn_parallel: m.nlbn_parallel,
            nlbn_running: m.nlbn_running,
            npnp_output_path: m.npnp_output_path.clone(),
            npnp_last_result: m.npnp_last_result.clone(),
            npnp_running: m.npnp_running,
            npnp_mode: m.npnp_mode.clone(),
            npnp_merge: m.npnp_merge,
            npnp_append: m.npnp_append,
            npnp_library_name: m.npnp_library_name.clone(),
            npnp_parallel: m.npnp_parallel,
            npnp_continue_on_error: m.npnp_continue_on_error,
            npnp_force: m.npnp_force,
            monitoring: m.monitoring,
            history_save_path: m.history_save_path.clone(),
            matched_save_path: m.matched_save_path.clone(),
        }
    } else {
        AppState {
            history: vec![],
            matched: vec![],
            keyword: String::new(),
            nlbn_output_path: defaults.nlbn.output_path,
            nlbn_last_result: None,
            nlbn_show_terminal: defaults.nlbn.show_terminal,
            nlbn_parallel: defaults.nlbn.parallel,
            nlbn_running: false,
            npnp_output_path: defaults.npnp.output_path,
            npnp_last_result: None,
            npnp_running: false,
            npnp_mode: defaults.npnp.mode,
            npnp_merge: defaults.npnp.merge,
            npnp_append: defaults.npnp.append,
            npnp_library_name: defaults.npnp.library_name,
            npnp_parallel: defaults.npnp.parallel,
            npnp_continue_on_error: defaults.npnp.continue_on_error,
            npnp_force: defaults.npnp.force,
            monitoring: true,
            history_count: 0,
            matched_count: 0,
            history_save_path: monitor::default_save_path("history.txt"),
            matched_save_path: monitor::default_save_path("matched.txt"),
        }
    }
}

#[tauri::command]
fn set_keyword(monitor: State<ManagedMonitor>, keyword: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_keyword(keyword);
    }
}

#[tauri::command]
fn toggle_monitoring(monitor: State<ManagedMonitor>) {
    if let Ok(mut m) = monitor.state.lock() {
        m.monitoring = !m.monitoring;
        if m.monitoring {
            m.last_content.clear();
            m.initialized = true;
        }
    }
}

#[tauri::command]
fn delete_history(monitor: State<ManagedMonitor>, index: usize) {
    if let Ok(mut m) = monitor.state.lock() {
        m.delete_history(index);
    }
}

#[tauri::command]
fn delete_matched(monitor: State<ManagedMonitor>, index: usize) {
    if let Ok(mut m) = monitor.state.lock() {
        m.delete_matched(index);
    }
}

#[tauri::command]
fn clear_all(monitor: State<ManagedMonitor>) {
    if let Ok(mut m) = monitor.state.lock() {
        m.history.clear();
        m.matched.clear();
        m.last_content.clear();
        m.initialized = false;
        m.match_debug_log.clear();
        m.nlbn_last_result = None;
        m.nlbn_running = false;
        m.npnp_last_result = None;
        m.npnp_running = false;
    }
}

fn ensure_parent_dir(path: &Path) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            std::fs::create_dir_all(parent)?;
        }
    }
    Ok(())
}

#[tauri::command]
fn save_history(monitor: State<ManagedMonitor>) -> String {
    let (path, entries) = {
        let Ok(m) = monitor.state.lock() else {
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

#[tauri::command]
fn save_matched(monitor: State<ManagedMonitor>) -> String {
    let (path, entries) = {
        let Ok(m) = monitor.state.lock() else {
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

#[tauri::command]
fn set_history_save_path(monitor: State<ManagedMonitor>, path: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_history_save_path(path);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_matched_save_path(monitor: State<ManagedMonitor>, path: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_matched_save_path(path);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_nlbn_path(monitor: State<ManagedMonitor>, path: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_nlbn_output_path(path);
    }
    save_config(&monitor);
}

#[tauri::command]
fn toggle_nlbn_terminal(monitor: State<ManagedMonitor>) {
    if let Ok(mut m) = monitor.state.lock() {
        m.toggle_nlbn_show_terminal();
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_nlbn_parallel(monitor: State<ManagedMonitor>, parallel: usize) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_nlbn_parallel(parallel);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_path(monitor: State<ManagedMonitor>, path: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_output_path(path);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_mode(monitor: State<ManagedMonitor>, mode: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_mode(mode);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_merge(monitor: State<ManagedMonitor>, merge: bool) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_merge(merge);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_append(monitor: State<ManagedMonitor>, append: bool) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_append(append);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_library_name(monitor: State<ManagedMonitor>, library_name: String) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_library_name(library_name);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_parallel(monitor: State<ManagedMonitor>, parallel: usize) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_parallel(parallel);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_continue_on_error(monitor: State<ManagedMonitor>, continue_on_error: bool) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_continue_on_error(continue_on_error);
    }
    save_config(&monitor);
}

#[tauri::command]
fn set_npnp_force(monitor: State<ManagedMonitor>, force: bool) {
    if let Ok(mut m) = monitor.state.lock() {
        m.set_npnp_force(force);
    }
    save_config(&monitor);
}

#[tauri::command]
fn check_nlbn() -> Result<String, String> {
    nlbn::check_installation()
}

#[tauri::command]
fn nlbn_export(monitor: State<ManagedMonitor>, app_handle: AppHandle) -> String {
    let request = if let Ok(m) = monitor.state.lock() {
        let ids = m.get_unique_ids();
        if ids.is_empty() {
            return "No matched results to export".to_string();
        }
        nlbn::ExportRequest {
            ids,
            output_path: m.nlbn_output_path.clone(),
            show_terminal: m.nlbn_show_terminal,
            parallel: m.nlbn_parallel,
        }
    } else {
        return "State lock failed".to_string();
    };

    nlbn::spawn_export(Arc::clone(&monitor.state), request, app_handle);
    "Export started".to_string()
}

#[tauri::command]
fn npnp_export(monitor: State<ManagedMonitor>, app_handle: AppHandle) -> String {
    let request = if let Ok(m) = monitor.state.lock() {
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

    npnp::spawn_export(Arc::clone(&monitor.state), request, app_handle);
    "Export started".to_string()
}

#[tauri::command]
fn get_unique_ids(monitor: State<ManagedMonitor>) -> Vec<String> {
    if let Ok(m) = monitor.state.lock() {
        m.get_unique_ids()
    } else {
        vec![]
    }
}

#[tauri::command]
fn copy_to_clipboard(text: String) -> Result<(), String> {
    let mut clip = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    clip.set_text(&text).map_err(|e| e.to_string())?;
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let config = AppConfig::load();

    let state = Arc::new(Mutex::new(MonitorState::new()));
    if let Ok(mut s) = state.lock() {
        s.set_nlbn_output_path(config.nlbn.output_path.clone());
        s.nlbn_show_terminal = config.nlbn.show_terminal;
        s.set_nlbn_parallel(config.nlbn.parallel);
        s.set_npnp_output_path(config.npnp.output_path.clone());
        s.set_npnp_mode(config.npnp.mode.clone());
        s.set_npnp_merge(config.npnp.merge);
        s.set_npnp_append(config.npnp.append);
        s.set_npnp_library_name(config.npnp.library_name.clone());
        s.set_npnp_parallel(config.npnp.parallel);
        s.set_npnp_continue_on_error(config.npnp.continue_on_error);
        s.set_npnp_force(config.npnp.force);
        s.set_history_save_path(config.monitor.history_save_path.clone());
        s.set_matched_save_path(config.monitor.matched_save_path.clone());
        s.set_keyword(
            "regex:\u{7f16}\u{53f7}[\u{ff1a}:]\\s*(C\\d+)||regex:(?m)^(C\\d{3,})$".to_string(),
        );
    }

    let monitor_state = Arc::clone(&state);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(ManagedMonitor {
            state: monitor_state,
            _handle: Mutex::new(None),
        })
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let handle = MonitorHandle::spawn(Arc::clone(&state), app_handle);
            let managed: State<ManagedMonitor> = app.state();
            *managed._handle.lock().unwrap() = Some(handle);
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_state,
            set_keyword,
            toggle_monitoring,
            delete_history,
            delete_matched,
            clear_all,
            save_history,
            save_matched,
            set_history_save_path,
            set_matched_save_path,
            set_nlbn_path,
            toggle_nlbn_terminal,
            set_nlbn_parallel,
            set_npnp_path,
            set_npnp_mode,
            set_npnp_merge,
            set_npnp_append,
            set_npnp_library_name,
            set_npnp_parallel,
            set_npnp_continue_on_error,
            set_npnp_force,
            check_nlbn,
            nlbn_export,
            npnp_export,
            get_unique_ids,
            copy_to_clipboard,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
