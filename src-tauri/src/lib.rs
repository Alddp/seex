mod config;
mod extract;
mod monitor;
mod nlbn;
mod npnp;

use config::{AppConfig, NlbnConfig, NpnpConfig};
use monitor::{MonitorHandle, MonitorState};
use serde::Serialize;
use std::io::Write;
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
    pub npnp_library_name: String,
    pub npnp_parallel: usize,
    pub npnp_continue_on_error: bool,
    pub npnp_force: bool,
    pub monitoring: bool,
    pub history_count: usize,
    pub matched_count: usize,
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
            library_name: state.npnp_library_name.clone(),
            parallel: state.npnp_parallel,
            continue_on_error: state.npnp_continue_on_error,
            force: state.npnp_force,
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
            npnp_library_name: m.npnp_library_name.clone(),
            npnp_parallel: m.npnp_parallel,
            npnp_continue_on_error: m.npnp_continue_on_error,
            npnp_force: m.npnp_force,
            monitoring: m.monitoring,
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
            npnp_library_name: defaults.npnp.library_name,
            npnp_parallel: defaults.npnp.parallel,
            npnp_continue_on_error: defaults.npnp.continue_on_error,
            npnp_force: defaults.npnp.force,
            monitoring: true,
            history_count: 0,
            matched_count: 0,
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

#[tauri::command]
fn save_history(monitor: State<ManagedMonitor>) -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    if let Ok(m) = monitor.state.lock() {
        if m.history.is_empty() {
            return "No history to save".to_string();
        }
        let path = exe_dir.join("history.txt");
        if let Ok(mut file) = std::fs::File::create(&path) {
            for (time, content) in &m.history {
                let _ = writeln!(file, "[{}] {}", time, content);
            }
            return format!("Saved to {}", path.display());
        }
    }
    "Save failed".to_string()
}

#[tauri::command]
fn save_matched(monitor: State<ManagedMonitor>) -> String {
    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|d| d.to_path_buf()))
        .unwrap_or_else(|| std::path::PathBuf::from("."));

    if let Ok(m) = monitor.state.lock() {
        if m.matched.is_empty() {
            return "No matched results to export".to_string();
        }
        let path = exe_dir.join("matched.txt");
        if let Ok(mut file) = std::fs::File::create(&path) {
            for (_, extracted) in &m.matched {
                let _ = writeln!(file, "{}", extracted);
            }
            return format!("Exported to {}", path.display());
        }
    }
    "Export failed".to_string()
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
    #[cfg(target_os = "windows")]
    let result = std::process::Command::new("cmd")
        .args(["/C", "nlbn", "--version"])
        .output();

    #[cfg(not(target_os = "windows"))]
    let result = std::process::Command::new("nlbn").arg("--version").output();

    match result {
        Ok(output) if output.status.success() => {
            let ver = String::from_utf8_lossy(&output.stdout).trim().to_string();
            Ok(ver)
        }
        _ => Err("nlbn not found".to_string()),
    }
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
        s.set_npnp_library_name(config.npnp.library_name.clone());
        s.set_npnp_parallel(config.npnp.parallel);
        s.set_npnp_continue_on_error(config.npnp.continue_on_error);
        s.set_npnp_force(config.npnp.force);
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
            set_nlbn_path,
            toggle_nlbn_terminal,
            set_nlbn_parallel,
            set_npnp_path,
            set_npnp_mode,
            set_npnp_merge,
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
