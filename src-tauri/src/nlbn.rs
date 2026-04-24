use nlbn::checkpoint::{append_checkpoint, load_checkpoint};
use nlbn::{Cli, EasyedaApi, LibraryManager};
use serde::Serialize;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tauri::{AppHandle, Emitter};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;

use crate::monitor::MonitorState;

const DEFAULT_OUTPUT_DIR: &str = "~/lib";

#[derive(Clone, Serialize)]
struct ExportFinishedPayload {
    tool: &'static str,
    success: bool,
    message: String,
}

#[derive(Clone, Serialize)]
struct ExportProgressPayload {
    tool: &'static str,
    message: String,
    determinate: bool,
    current: Option<usize>,
    total: Option<usize>,
}

struct ItemOutcome {
    id: String,
    result: Result<(), String>,
}

pub struct ExportRequest {
    pub ids: Vec<String>,
    pub output_path: String,
    pub mode: String,
    pub append: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub overwrite: bool,
    pub project_relative: bool,
}

pub fn spawn_export(state: Arc<Mutex<MonitorState>>, req: ExportRequest, app_handle: AppHandle) {
    if let Ok(mut s) = state.lock() {
        s.nlbn_running = true;
        s.nlbn_last_result = None;
    }

    emit_progress(
        &app_handle,
        "Preparing nlbn export...",
        false,
        None,
        Some(req.ids.len()),
    );

    tauri::async_runtime::spawn(async move {
        let result = run_export(&req, &app_handle).await;
        let (success, message) = match result {
            Ok(message) => (true, message),
            Err(error) => (false, error),
        };

        if let Ok(mut s) = state.lock() {
            s.nlbn_running = false;
            s.nlbn_last_result = Some(message.clone());
            s.add_debug_log(message.clone());
        }

        let _ = app_handle.emit("clipboard-changed", ());
        let _ = app_handle.emit(
            "export-finished",
            ExportFinishedPayload {
                tool: "nlbn",
                success,
                message,
            },
        );
    });
}

fn emit_progress(
    app_handle: &AppHandle,
    message: impl Into<String>,
    determinate: bool,
    current: Option<usize>,
    total: Option<usize>,
) {
    let _ = app_handle.emit(
        "export-progress",
        ExportProgressPayload {
            tool: "nlbn",
            message: message.into(),
            determinate,
            current,
            total,
        },
    );
}

async fn run_export(req: &ExportRequest, app_handle: &AppHandle) -> Result<String, String> {
    let args = Arc::new(build_cli(req));
    let lib_manager = Arc::new(LibraryManager::from_cli(&args).map_err(|e| e.to_string())?);
    lib_manager
        .create_directories()
        .map_err(|e| e.to_string())?;

    let checkpoint_path = args.output.join(".checkpoint");
    let completed_ids = load_checkpoint(&checkpoint_path);
    let total_requested = req.ids.len();
    let pending_ids: Vec<String> = req
        .ids
        .iter()
        .filter(|id| !completed_ids.contains(id.as_str()))
        .cloned()
        .collect();
    let skipped_by_checkpoint = total_requested.saturating_sub(pending_ids.len());

    if pending_ids.is_empty() {
        return Ok(format_summary(
            req,
            total_requested,
            skipped_by_checkpoint,
            0,
            0,
            &[],
        ));
    }

    let total = pending_ids.len();
    emit_progress(
        app_handle,
        running_message(req, total),
        false,
        None,
        Some(total),
    );

    let api = Arc::new(EasyedaApi::new());
    let success_count = Arc::new(AtomicUsize::new(0));
    let failed_count = Arc::new(AtomicUsize::new(0));
    let processed_count = Arc::new(AtomicUsize::new(0));
    let failed_ids = Arc::new(Mutex::new(Vec::new()));

    if total > 1 && req.parallel.max(1) > 1 {
        let semaphore = Arc::new(Semaphore::new(req.parallel.max(1)));
        let mut join_set = JoinSet::new();

        for id in pending_ids {
            let semaphore = semaphore.clone();
            let args = args.clone();
            let api = api.clone();
            let lib_manager = lib_manager.clone();

            join_set.spawn(async move {
                let _permit = semaphore.acquire_owned().await.map_err(|e| e.to_string())?;
                let result = process_component(&args, &api, &lib_manager, &id)
                    .await
                    .map_err(|e| e.to_string());
                Ok::<ItemOutcome, String>(ItemOutcome { id, result })
            });
        }

        while let Some(joined) = join_set.join_next().await {
            let outcome = match joined {
                Ok(Ok(outcome)) => outcome,
                Ok(Err(error)) => ItemOutcome {
                    id: "<task>".to_string(),
                    result: Err(error),
                },
                Err(error) => ItemOutcome {
                    id: "<task>".to_string(),
                    result: Err(error.to_string()),
                },
            };

            register_outcome(
                app_handle,
                &outcome,
                total,
                &processed_count,
                &success_count,
                &failed_count,
                &failed_ids,
            );
        }
    } else {
        for id in pending_ids {
            let outcome = ItemOutcome {
                id: id.clone(),
                result: process_component(&args, &api, &lib_manager, &id)
                    .await
                    .map_err(|e| e.to_string()),
            };

            register_outcome(
                app_handle,
                &outcome,
                total,
                &processed_count,
                &success_count,
                &failed_count,
                &failed_ids,
            );

            if outcome.result.is_err() && !req.continue_on_error {
                return Err(format_stopped_error(
                    req,
                    &outcome.id,
                    outcome
                        .result
                        .as_ref()
                        .err()
                        .map(|message| message.as_str())
                        .unwrap_or("unknown error"),
                    total_requested,
                    skipped_by_checkpoint,
                    success_count.load(Ordering::Relaxed),
                    failed_count.load(Ordering::Relaxed),
                    &failed_ids.lock().map(|ids| ids.clone()).unwrap_or_default(),
                ));
            }
        }
    }

    let failed_ids = failed_ids.lock().map(|ids| ids.clone()).unwrap_or_default();
    Ok(format_summary(
        req,
        total_requested,
        skipped_by_checkpoint,
        success_count.load(Ordering::Relaxed),
        failed_count.load(Ordering::Relaxed),
        &failed_ids,
    ))
}

fn register_outcome(
    app_handle: &AppHandle,
    outcome: &ItemOutcome,
    total: usize,
    processed_count: &Arc<AtomicUsize>,
    success_count: &Arc<AtomicUsize>,
    failed_count: &Arc<AtomicUsize>,
    failed_ids: &Arc<Mutex<Vec<String>>>,
) {
    match &outcome.result {
        Ok(()) => {
            success_count.fetch_add(1, Ordering::Relaxed);
        }
        Err(_) => {
            failed_count.fetch_add(1, Ordering::Relaxed);
            if let Ok(mut ids) = failed_ids.lock() {
                ids.push(outcome.id.clone());
            }
        }
    }

    let current = processed_count.fetch_add(1, Ordering::Relaxed) + 1;
    let message = match &outcome.result {
        Ok(()) => format!("Processed {} successfully", outcome.id),
        Err(error) => format!("Failed {}: {}", outcome.id, error),
    };
    emit_progress(app_handle, message, true, Some(current), Some(total));
}

async fn process_component(
    args: &Cli,
    api: &EasyedaApi,
    lib_manager: &LibraryManager,
    lcsc_id: &str,
) -> nlbn::Result<()> {
    let component_data = api.get_component_data(lcsc_id).await?;

    if args.symbol || args.full {
        nlbn::symbol_converter::convert_symbol(args, &component_data, lib_manager, lcsc_id)?;
    }

    if args.footprint || args.full {
        nlbn::footprint_converter::convert_footprint(args, &component_data, lib_manager, lcsc_id)?;
    }

    if args.model_3d || args.full {
        nlbn::model_converter::convert_3d_model(args, api, &component_data, lib_manager, lcsc_id)
            .await?;
    }

    append_checkpoint(&args.output.join(".checkpoint"), lcsc_id);
    Ok(())
}

fn build_cli(req: &ExportRequest) -> Cli {
    let mode = normalize_mode(&req.mode);
    let output = PathBuf::from(normalize_output_path(&req.output_path));

    Cli {
        lcsc_id: None,
        batch: None,
        symbol: mode == "symbol",
        footprint: mode == "footprint",
        model_3d: mode == "3d",
        full: mode == "full",
        output,
        lib_name: effective_library_name(req),
        symbol_lib: None,
        footprint_lib: None,
        model_lib: None,
        prompt: false,
        overwrite: req.overwrite,
        project_relative: req.project_relative,
        debug: false,
        continue_on_error: req.continue_on_error,
        parallel: req.parallel.max(1),
    }
}

fn normalize_mode(mode: &str) -> &str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "symbol" => "symbol",
        "footprint" => "footprint",
        "3d" => "3d",
        _ => "full",
    }
}

fn normalize_output_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        DEFAULT_OUTPUT_DIR.to_string()
    } else {
        trimmed.to_string()
    }
}

fn effective_library_name(req: &ExportRequest) -> Option<String> {
    if !req.append {
        None
    } else {
        let trimmed = req.library_name.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }
}

fn running_message(req: &ExportRequest, total: usize) -> String {
    format!(
        "Running nlbn {} batch for {} items...",
        mode_label(&req.mode).to_ascii_lowercase(),
        total
    )
}

fn format_summary(
    req: &ExportRequest,
    total_requested: usize,
    skipped_by_checkpoint: usize,
    success_count: usize,
    failed_count: usize,
    failed_ids: &[String],
) -> String {
    let mut lines = vec![format!(
        "nlbn batch complete. Total: {} | Checkpoint skipped: {} | Success: {} | Failed: {}",
        total_requested, skipped_by_checkpoint, success_count, failed_count
    )];

    lines.push(format!(
        "Targets: {} | Parallel: {}",
        mode_label(&req.mode),
        req.parallel.max(1),
    ));

    lines.push(format!(
        "Append: {} | Continue on error: {} | Overwrite: {} | Project relative: {}",
        yes_no(req.append),
        yes_no(req.continue_on_error),
        yes_no(req.overwrite),
        yes_no(req.project_relative),
    ));

    lines.push(format!("Library name: {}", resolved_library_name(req)));
    lines.push(format!(
        "Output directory: {}",
        normalize_output_path(&req.output_path)
    ));

    if !failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", failed_ids.join(", ")));
    }

    lines.join("\n")
}

fn format_stopped_error(
    req: &ExportRequest,
    failed_id: &str,
    error: &str,
    total_requested: usize,
    skipped_by_checkpoint: usize,
    success_count: usize,
    failed_count: usize,
    failed_ids: &[String],
) -> String {
    let mut lines = vec![
        "nlbn export failed".to_string(),
        format!("Stopped on {}: {}", failed_id, error),
        format!(
            "Total: {} | Checkpoint skipped: {} | Success: {} | Failed: {}",
            total_requested, skipped_by_checkpoint, success_count, failed_count
        ),
        format!(
            "Targets: {} | Parallel: {}",
            mode_label(&req.mode),
            req.parallel.max(1),
        ),
        format!(
            "Append: {} | Continue on error: {} | Overwrite: {} | Project relative: {}",
            yes_no(req.append),
            yes_no(req.continue_on_error),
            yes_no(req.overwrite),
            yes_no(req.project_relative),
        ),
        format!("Library name: {}", resolved_library_name(req)),
        format!(
            "Output directory: {}",
            normalize_output_path(&req.output_path)
        ),
    ];

    if !failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", failed_ids.join(", ")));
    }

    lines.join("\n")
}

fn mode_label(mode: &str) -> &'static str {
    match normalize_mode(mode) {
        "symbol" => "Symbol",
        "footprint" => "Footprint",
        "3d" => "3D",
        _ => "Full",
    }
}

fn resolved_library_name(req: &ExportRequest) -> String {
    effective_library_name(req).unwrap_or_else(|| {
        let output = PathBuf::from(normalize_output_path(&req.output_path));
        output
            .file_name()
            .and_then(|name| name.to_str())
            .filter(|name| !name.is_empty())
            .unwrap_or("nlbn")
            .to_string()
    })
}

fn yes_no(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
