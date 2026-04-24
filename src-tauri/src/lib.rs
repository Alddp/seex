pub mod app_paths;
pub mod config;
pub mod controller;
pub mod extract;
pub mod imported_symbols;
pub mod monitor;
pub mod nlbn;
pub mod npnp;
pub mod tui;

use controller::AppController;
use serde::Serialize;
use std::sync::Arc;
use tauri::{AppHandle, Emitter, Manager, PhysicalSize, State, WindowEvent};

use crate::config::{AppConfig, NlbnPathMode};

#[derive(Serialize, Clone)]
pub struct AppState {
    pub history: Vec<(String, String)>,
    pub matched: Vec<(String, String)>,
    pub keyword: String,
    pub nlbn_output_path: String,
    pub nlbn_last_result: Option<String>,
    pub nlbn_show_terminal: bool,
    pub nlbn_parallel: usize,
    pub nlbn_path_mode: NlbnPathMode,
    pub nlbn_export_symbol: bool,
    pub nlbn_export_footprint: bool,
    pub nlbn_export_model_3d: bool,
    pub nlbn_overwrite_symbol: bool,
    pub nlbn_overwrite_footprint: bool,
    pub nlbn_overwrite_model_3d: bool,
    pub nlbn_symbol_fill_color: Option<String>,
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
    pub always_on_top: bool,
    pub history_count: usize,
    pub matched_count: usize,
    pub history_save_path: String,
    pub matched_save_path: String,
    pub imported_parts_save_path: String,
}

pub struct ManagedApp {
    pub controller: AppController,
}

fn build_app_state(controller: &AppController) -> AppState {
    let defaults = AppConfig::default();

    if let Ok(m) = controller.state().lock() {
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
            nlbn_path_mode: m.nlbn_path_mode,
            nlbn_export_symbol: m.nlbn_export_symbol,
            nlbn_export_footprint: m.nlbn_export_footprint,
            nlbn_export_model_3d: m.nlbn_export_model_3d,
            nlbn_overwrite_symbol: m.nlbn_overwrite_symbol,
            nlbn_overwrite_footprint: m.nlbn_overwrite_footprint,
            nlbn_overwrite_model_3d: m.nlbn_overwrite_model_3d,
            nlbn_symbol_fill_color: m.nlbn_symbol_fill_color.clone(),
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
            always_on_top: m.always_on_top,
            history_save_path: m.history_save_path.clone(),
            matched_save_path: m.matched_save_path.clone(),
            imported_parts_save_path: m.imported_parts_save_path.clone(),
        }
    } else {
        AppState {
            history: vec![],
            matched: vec![],
            keyword: String::new(),
            nlbn_output_path: controller.paths().default_nlbn_output_path_string(),
            nlbn_last_result: None,
            nlbn_show_terminal: defaults.nlbn.show_terminal,
            nlbn_parallel: defaults.nlbn.parallel,
            nlbn_path_mode: defaults.nlbn.path_mode,
            nlbn_export_symbol: defaults.nlbn.export_symbol,
            nlbn_export_footprint: defaults.nlbn.export_footprint,
            nlbn_export_model_3d: defaults.nlbn.export_model_3d,
            nlbn_overwrite_symbol: defaults.nlbn.overwrite_symbol,
            nlbn_overwrite_footprint: defaults.nlbn.overwrite_footprint,
            nlbn_overwrite_model_3d: defaults.nlbn.overwrite_model_3d,
            nlbn_symbol_fill_color: defaults.nlbn.symbol_fill_color,
            nlbn_running: false,
            npnp_output_path: controller.paths().default_npnp_output_path_string(),
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
            always_on_top: defaults.monitor.always_on_top,
            history_count: 0,
            matched_count: 0,
            history_save_path: controller.paths().default_history_save_path_string(),
            matched_save_path: controller.paths().default_matched_save_path_string(),
            imported_parts_save_path: controller.paths().default_imported_parts_save_path_string(),
        }
    }
}

fn update_state<T, F>(app: &State<ManagedApp>, update: F) -> Result<T, String>
where
    F: FnOnce(&mut crate::monitor::MonitorState) -> Result<T, String>,
{
    app.controller.update_state_and_save(update)
}

fn update_state_and_ignore_lock_error<F>(app: &State<ManagedApp>, update: F)
where
    F: FnOnce(&mut crate::monitor::MonitorState),
{
    let _ = update_state(app, |state| {
        update(state);
        Ok(())
    });
}

#[tauri::command]
fn get_state(app: State<ManagedApp>) -> AppState {
    build_app_state(&app.controller)
}

#[tauri::command]
fn set_keyword(app: State<ManagedApp>, keyword: String) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.set_keyword(keyword);
    }
}

#[tauri::command]
fn toggle_monitoring(app: State<ManagedApp>) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.monitoring = !m.monitoring;
        if m.monitoring {
            m.last_content.clear();
            m.initialized = true;
        }
    }
}

#[tauri::command]
fn delete_history(app: State<ManagedApp>, index: usize) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.delete_history(index);
    }
}

#[tauri::command]
fn delete_matched(app: State<ManagedApp>, index: usize) {
    if let Ok(mut m) = app.controller.state().lock() {
        m.delete_matched(index);
    }
}

#[tauri::command]
fn clear_all(app: State<ManagedApp>) {
    if let Ok(mut m) = app.controller.state().lock() {
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
fn save_history(app: State<ManagedApp>) -> String {
    app.controller.save_history()
}

#[tauri::command]
fn save_matched(app: State<ManagedApp>) -> String {
    app.controller.save_matched()
}

#[tauri::command]
fn save_imported_parts(app: State<ManagedApp>) -> String {
    app.controller.save_imported_parts()
}

#[tauri::command]
fn save_lcsc_parts(app: State<ManagedApp>, parts: Vec<String>) -> String {
    app.controller.save_lcsc_parts(parts)
}

#[tauri::command]
fn import_imported_parts(app: State<ManagedApp>) -> String {
    app.controller.import_imported_parts()
}

#[tauri::command]
fn queue_lcsc_parts(app: State<ManagedApp>, parts: Vec<String>) -> String {
    app.controller.queue_lcsc_parts(parts)
}

#[tauri::command]
fn set_history_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_history_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_matched_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_matched_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_imported_parts_save_path(app: State<ManagedApp>, path: String) {
    let paths = app.controller.paths().clone();
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_imported_parts_save_path(path, &paths);
    });
}

#[tauri::command]
fn set_nlbn_path(app: State<ManagedApp>, path: String) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_nlbn_output_path(path);
    });
}

#[tauri::command]
fn toggle_nlbn_terminal(app: State<ManagedApp>) {
    update_state_and_ignore_lock_error(&app, |state| {
        state.toggle_nlbn_show_terminal();
    });
}

#[tauri::command]
fn set_nlbn_parallel(app: State<ManagedApp>, parallel: usize) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_nlbn_parallel(parallel);
    });
}

#[tauri::command]
fn set_nlbn_path_mode(app: State<ManagedApp>, path_mode: NlbnPathMode) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_nlbn_path_mode(path_mode);
    });
}

fn parse_nlbn_asset_kind(target: &str) -> Result<&str, String> {
    match target.trim().to_ascii_lowercase().as_str() {
        "symbol" => Ok("symbol"),
        "footprint" => Ok("footprint"),
        "3d" | "model_3d" | "model-3d" => Ok("model_3d"),
        _ => Err(format!("Unknown nlbn asset type: {}", target)),
    }
}

#[tauri::command]
fn set_nlbn_export_enabled(
    app: State<ManagedApp>,
    target: String,
    enabled: bool,
) -> Result<(), String> {
    update_state(&app, move |state| {
        match parse_nlbn_asset_kind(&target)? {
            "symbol" => state.set_nlbn_export_symbol(enabled),
            "footprint" => state.set_nlbn_export_footprint(enabled),
            "model_3d" => state.set_nlbn_export_model_3d(enabled),
            _ => unreachable!(),
        }
        Ok(())
    })
}

#[tauri::command]
fn set_nlbn_export_symbol(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_nlbn_export_enabled(app, "symbol".to_string(), enabled)
}

#[tauri::command]
fn set_nlbn_export_footprint(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_nlbn_export_enabled(app, "footprint".to_string(), enabled)
}

#[tauri::command]
fn set_nlbn_export_model_3d(app: State<ManagedApp>, enabled: bool) -> Result<(), String> {
    set_nlbn_export_enabled(app, "model_3d".to_string(), enabled)
}

#[tauri::command]
fn set_nlbn_overwrite_enabled(
    app: State<ManagedApp>,
    target: String,
    enabled: bool,
) -> Result<(), String> {
    update_state(&app, move |state| {
        match parse_nlbn_asset_kind(&target)? {
            "symbol" => state.set_nlbn_overwrite_symbol(enabled),
            "footprint" => state.set_nlbn_overwrite_footprint(enabled),
            "model_3d" => state.set_nlbn_overwrite_model_3d(enabled),
            _ => unreachable!(),
        }
        Ok(())
    })
}

#[tauri::command]
fn set_nlbn_overwrite_symbol(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_nlbn_overwrite_enabled(app, "symbol".to_string(), overwrite)
}

#[tauri::command]
fn set_nlbn_overwrite_footprint(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_nlbn_overwrite_enabled(app, "footprint".to_string(), overwrite)
}

#[tauri::command]
fn set_nlbn_overwrite_model_3d(app: State<ManagedApp>, overwrite: bool) -> Result<(), String> {
    set_nlbn_overwrite_enabled(app, "model_3d".to_string(), overwrite)
}

#[tauri::command]
fn set_nlbn_symbol_fill_color(app: State<ManagedApp>, color: Option<String>) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_nlbn_symbol_fill_color(color);
    });
}

#[tauri::command]
fn set_npnp_path(app: State<ManagedApp>, path: String) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_output_path(path);
    });
}

#[tauri::command]
fn set_npnp_mode(app: State<ManagedApp>, mode: String) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_mode(mode);
    });
}

#[tauri::command]
fn set_npnp_merge(app: State<ManagedApp>, merge: bool) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_merge(merge);
    });
}

#[tauri::command]
fn set_npnp_append(app: State<ManagedApp>, append: bool) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_append(append);
    });
}

#[tauri::command]
fn set_npnp_library_name(app: State<ManagedApp>, library_name: String) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_library_name(library_name);
    });
}

#[tauri::command]
fn set_npnp_parallel(app: State<ManagedApp>, parallel: usize) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_parallel(parallel);
    });
}

#[tauri::command]
fn set_npnp_continue_on_error(app: State<ManagedApp>, continue_on_error: bool) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_continue_on_error(continue_on_error);
    });
}

#[tauri::command]
fn set_npnp_force(app: State<ManagedApp>, force: bool) {
    update_state_and_ignore_lock_error(&app, move |state| {
        state.set_npnp_force(force);
    });
}

#[tauri::command]
fn set_window_always_on_top(
    app: State<ManagedApp>,
    app_handle: AppHandle,
    always_on_top: bool,
) -> Result<(), String> {
    let window = app_handle
        .get_webview_window("main")
        .ok_or_else(|| "Main window not found".to_string())?;
    window
        .set_always_on_top(always_on_top)
        .map_err(|err| err.to_string())?;

    update_state(&app, move |state| {
        state.always_on_top = always_on_top;
        Ok(())
    })
}

#[tauri::command]
fn check_nlbn() -> Result<String, String> {
    nlbn::check_installation()
}

#[tauri::command]
fn nlbn_export(app: State<ManagedApp>, app_handle: AppHandle) -> String {
    let emit_handle = app_handle.clone();
    let state_handle = app_handle.clone();
    let finished_handle = app_handle;

    app.controller.spawn_nlbn_export(nlbn::ExportCallbacks {
        on_progress: Some(Arc::new(move |payload| {
            let _ = emit_handle.emit("export-progress", payload);
        })),
        on_finished: Some(Arc::new(move |payload| {
            let _ = finished_handle.emit("export-finished", payload);
        })),
        on_state_changed: Some(Arc::new(move || {
            let _ = state_handle.emit("clipboard-changed", ());
        })),
    })
}

#[tauri::command]
fn npnp_export(app: State<ManagedApp>, app_handle: AppHandle) -> String {
    let emit_handle = app_handle.clone();
    let state_handle = app_handle.clone();
    let finished_handle = app_handle;

    app.controller.spawn_npnp_export(npnp::ExportCallbacks {
        on_progress: Some(Arc::new(move |payload| {
            let _ = emit_handle.emit("export-progress", payload);
        })),
        on_finished: Some(Arc::new(move |payload| {
            let _ = finished_handle.emit("export-finished", payload);
        })),
        on_state_changed: Some(Arc::new(move || {
            let _ = state_handle.emit("clipboard-changed", ());
        })),
    })
}

#[tauri::command]
fn get_unique_ids(app: State<ManagedApp>) -> Vec<String> {
    if let Ok(m) = app.controller.state().lock() {
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

#[tauri::command]
fn get_imported_symbols(
    app: State<ManagedApp>,
) -> Result<imported_symbols::ImportedSymbolsResponse, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.nlbn_output_path.clone()
    };

    imported_symbols::load_imported_symbols(std::path::Path::new(&output_path))
}

#[tauri::command]
fn update_imported_symbol(
    app: State<ManagedApp>,
    request: imported_symbols::ImportedSymbolUpdateRequest,
) -> Result<String, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.nlbn_output_path.clone()
    };

    imported_symbols::update_imported_symbol(std::path::Path::new(&output_path), request)
}

#[tauri::command]
fn delete_imported_symbol(
    app: State<ManagedApp>,
    request: imported_symbols::ImportedSymbolDeleteRequest,
) -> Result<String, String> {
    let output_path = {
        let state = app
            .controller
            .state()
            .lock()
            .map_err(|_| "State lock failed".to_string())?;
        state.nlbn_output_path.clone()
    };

    imported_symbols::delete_imported_symbol(std::path::Path::new(&output_path), request)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(move |app| {
            let app_handle = app.handle().clone();
            let paths = app_paths::AppPaths::resolve(&app_handle).map_err(std::io::Error::other)?;
            let emit_handle = app_handle;
            let controller = AppController::new(
                paths,
                Arc::new(move || {
                    let _ = emit_handle.emit("clipboard-changed", ());
                }),
            )
            .map_err(std::io::Error::other)?;
            let (always_on_top, window_width, window_height) = controller
                .state()
                .lock()
                .map(|state| (state.always_on_top, state.window_width, state.window_height))
                .unwrap_or((false, None, None));
            app.manage(ManagedApp { controller });
            if let Some(window) = app.get_webview_window("main") {
                if let (Some(width), Some(height)) = (window_width, window_height) {
                    let _ = window.set_size(PhysicalSize::new(width, height));
                }
                let _ = window.set_always_on_top(always_on_top);
                let app_handle = app.handle().clone();
                let event_window = window.clone();
                window.on_window_event(move |event| match event {
                    WindowEvent::Resized(size) => {
                        let is_maximized = event_window.is_maximized().unwrap_or(false);
                        let is_minimized = event_window.is_minimized().unwrap_or(false);
                        if !is_maximized
                            && !is_minimized
                            && let Ok(mut state) =
                                app_handle.state::<ManagedApp>().controller.state().lock()
                        {
                            state.set_window_size(size.width, size.height);
                        }
                    }
                    WindowEvent::CloseRequested { .. } | WindowEvent::Destroyed => {
                        app_handle.state::<ManagedApp>().controller.save_config();
                    }
                    _ => {}
                });
            }
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
            save_imported_parts,
            save_lcsc_parts,
            import_imported_parts,
            queue_lcsc_parts,
            set_history_save_path,
            set_matched_save_path,
            set_imported_parts_save_path,
            set_nlbn_path,
            toggle_nlbn_terminal,
            set_nlbn_parallel,
            set_nlbn_path_mode,
            set_nlbn_export_enabled,
            set_nlbn_export_symbol,
            set_nlbn_export_footprint,
            set_nlbn_export_model_3d,
            set_nlbn_overwrite_enabled,
            set_nlbn_overwrite_symbol,
            set_nlbn_overwrite_footprint,
            set_nlbn_overwrite_model_3d,
            set_nlbn_symbol_fill_color,
            set_npnp_path,
            set_npnp_mode,
            set_npnp_merge,
            set_npnp_append,
            set_npnp_library_name,
            set_npnp_parallel,
            set_npnp_continue_on_error,
            set_npnp_force,
            set_window_always_on_top,
            check_nlbn,
            nlbn_export,
            npnp_export,
            get_unique_ids,
            copy_to_clipboard,
            get_imported_symbols,
            update_imported_symbol,
            delete_imported_symbol,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
