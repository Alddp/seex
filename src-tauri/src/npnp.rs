use ::npnp::LcedaClient;
use ::npnp::batch::{BatchOptions, BatchSummary, export_batch};
use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::thread;

use crate::app_paths::AppPaths;
use crate::monitor::MonitorState;

const DEFAULT_LIBRARY_NAME: &str = "SeExMerged";
const DEFAULT_OUTPUT_DIR: &str = "npnp_export";

pub type NotifyFn = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Clone, Serialize)]
pub struct ExportFinishedPayload {
    pub tool: &'static str,
    pub success: bool,
    pub message: String,
}

#[derive(Clone, Serialize)]
pub struct ExportProgressPayload {
    pub tool: &'static str,
    pub message: String,
    pub determinate: bool,
    pub current: Option<usize>,
    pub total: Option<usize>,
}

#[derive(Clone, Default)]
pub struct ExportCallbacks {
    pub on_progress: Option<Arc<dyn Fn(ExportProgressPayload) + Send + Sync + 'static>>,
    pub on_finished: Option<Arc<dyn Fn(ExportFinishedPayload) + Send + Sync + 'static>>,
    pub on_state_changed: Option<NotifyFn>,
}

pub struct ExportRequest {
    pub ids: Vec<String>,
    pub output_path: String,
    pub mode: String,
    pub merge: bool,
    pub append: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub force: bool,
}

pub fn spawn_export(
    state: Arc<Mutex<MonitorState>>,
    req: ExportRequest,
    paths: AppPaths,
    callbacks: ExportCallbacks,
) {
    if let Ok(mut s) = state.lock() {
        s.npnp_running = true;
        s.npnp_last_result = None;
    }

    emit_progress(
        &callbacks,
        "Preparing npnp export...",
        false,
        None,
        Some(req.ids.len()),
    );

    thread::spawn(move || {
        let input_path = paths.cache_file("seex_npnp_ids", "txt");
        let result: Result<String, String> = (|| -> Result<String, String> {
            write_ids_file(&input_path, &req.ids)?;
            let client = LcedaClient::new();
            emit_progress(
                &callbacks,
                running_message(&req),
                false,
                None,
                Some(req.ids.len()),
            );
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| e.to_string())?;
            let summary = runtime
                .block_on(export_batch(
                    &client,
                    build_batch_options(&req, &input_path),
                ))
                .map_err(|e| e.to_string())?;
            Ok(format_summary(&req, &summary))
        })();

        let _ = fs::remove_file(&input_path);

        let (success, message) = match result {
            Ok(message) => (true, message),
            Err(err) => (false, format_error(&req, &err)),
        };

        if let Ok(mut s) = state.lock() {
            s.npnp_running = false;
            s.npnp_last_result = Some(message.clone());
            s.add_debug_log(message.clone());
        }

        notify_state_changed(&callbacks);
        emit_finished(
            &callbacks,
            ExportFinishedPayload {
                tool: "npnp",
                success,
                message,
            },
        );
    });
}

fn emit_progress(
    callbacks: &ExportCallbacks,
    message: impl Into<String>,
    determinate: bool,
    current: Option<usize>,
    total: Option<usize>,
) {
    if let Some(on_progress) = &callbacks.on_progress {
        on_progress(ExportProgressPayload {
            tool: "npnp",
            message: message.into(),
            determinate,
            current,
            total,
        });
    }
}

fn emit_finished(callbacks: &ExportCallbacks, payload: ExportFinishedPayload) {
    if let Some(on_finished) = &callbacks.on_finished {
        on_finished(payload);
    }
}

fn notify_state_changed(callbacks: &ExportCallbacks) {
    if let Some(on_state_changed) = &callbacks.on_state_changed {
        on_state_changed();
    }
}

fn running_message(req: &ExportRequest) -> String {
    let count = req.ids.len();
    if req.merge && req.append {
        format!("Appending into merged npnp library ({} items)...", count)
    } else if req.merge {
        format!("Merging {} npnp items...", count)
    } else {
        format!("Running npnp batch for {} items...", count)
    }
}

fn write_ids_file(path: &PathBuf, ids: &[String]) -> Result<(), String> {
    let mut file = fs::File::create(path).map_err(|e| e.to_string())?;
    for id in ids {
        writeln!(file, "{}", id).map_err(|e| e.to_string())?;
    }
    file.sync_all().map_err(|e| e.to_string())
}

fn build_batch_options(req: &ExportRequest, input_path: &PathBuf) -> BatchOptions {
    let mode = normalize_mode(&req.mode);
    // npnp's validator rejects --append without --merge. The UI also guards
    // this, but clamp on the backend too in case older configs slip through.
    let append = req.append && req.merge;
    BatchOptions {
        input: input_path.clone(),
        output: PathBuf::from(normalize_output_path(&req.output_path)),
        schlib: mode == "schlib",
        pcblib: mode == "pcblib",
        full: mode == "full",
        merge: req.merge,
        append,
        library_name: effective_library_name(req),
        parallel: req.parallel.max(1),
        continue_on_error: req.continue_on_error,
        force: req.force,
    }
}

fn normalize_mode(mode: &str) -> &str {
    match mode.trim().to_ascii_lowercase().as_str() {
        "schlib" => "schlib",
        "pcblib" => "pcblib",
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
    if !req.merge {
        return None;
    }

    Some(resolve_library_name(req))
}

fn resolve_library_name(req: &ExportRequest) -> String {
    let trimmed = req.library_name.trim();
    if trimmed.is_empty() {
        DEFAULT_LIBRARY_NAME.to_string()
    } else {
        trimmed.to_string()
    }
}

fn format_summary(req: &ExportRequest, summary: &BatchSummary) -> String {
    let mut lines = vec![format!(
        "npnp batch complete. Total: {} | Skipped: {} | Success: {} | Failed: {}",
        summary.total, summary.skipped, summary.success, summary.failed,
    )];

    lines.push(format!(
        "Targets: {} | Merge: {} | Parallel: {}",
        mode_label(&req.mode),
        merge_label(req),
        req.parallel.max(1),
    ));

    lines.push(format!(
        "Continue on error: {} | Force: {}",
        yes_no(req.continue_on_error),
        yes_no(req.force),
    ));

    if req.merge {
        lines.push(format!(
            "Library name: {}",
            effective_library_name(req).unwrap_or_else(|| DEFAULT_LIBRARY_NAME.to_string())
        ));
    }

    if !summary.failed_ids.is_empty() {
        lines.push(format!("Failed IDs: {}", summary.failed_ids.join(", ")));
    }

    if summary.generated_files.is_empty() {
        lines.push(format!("Output directory: {}", summary.output.display()));
        if !req.merge {
            lines.push(format!("Folders: {}", target_folders(&req.mode).join(", ")));
        }
    } else {
        for path in &summary.generated_files {
            lines.push(format!("Generated: {}", path.display()));
        }
    }

    lines.join("\n")
}

fn format_error(req: &ExportRequest, error: &str) -> String {
    let mut lines = vec!["npnp export failed".to_string(), error.to_string()];
    lines.push(format!("Targets: {}", mode_label(&req.mode)));
    lines.push(format!(
        "Output directory: {}",
        normalize_output_path(&req.output_path)
    ));
    if req.merge {
        lines.push(format!(
            "Library name: {}",
            effective_library_name(req).unwrap_or_else(|| DEFAULT_LIBRARY_NAME.to_string())
        ));
    }
    lines.join("\n")
}

fn mode_label(mode: &str) -> &'static str {
    match normalize_mode(mode) {
        "schlib" => "SchLib",
        "pcblib" => "PcbLib",
        _ => "Full",
    }
}

fn merge_label(req: &ExportRequest) -> &'static str {
    match (req.merge, req.append) {
        (true, true) => "ON (append)",
        (true, false) => "ON",
        _ => "OFF",
    }
}

fn target_folders(mode: &str) -> Vec<&'static str> {
    match normalize_mode(mode) {
        "schlib" => vec!["schlib"],
        "pcblib" => vec!["pcblib"],
        _ => vec!["schlib", "pcblib"],
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "ON" } else { "OFF" }
}
