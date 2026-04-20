use ::npnp::LcedaClient;
use ::npnp::batch::{BatchOptions, BatchSummary, export_batch};
use ::npnp::pcblib::{PcbLibrary, write_pcblib};
use ::npnp::schlib::{Component, write_schlib_library};
use ::npnp::util::sanitize_filename;
use ::npnp::workflow::{build_pcblib_library_for_item, build_schlib_component_for_item};
use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter};
use tokio::sync::{Mutex as AsyncMutex, Semaphore};
use tokio::task::JoinSet;

use crate::monitor::MonitorState;

const DEFAULT_LIBRARY_NAME: &str = "SeExMerged";
const DEFAULT_OUTPUT_DIR: &str = "npnp_export";

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

pub struct ExportRequest {
    pub ids: Vec<String>,
    pub output_path: String,
    pub mode: String,
    pub merge: bool,
    pub library_name: String,
    pub parallel: usize,
    pub continue_on_error: bool,
    pub force: bool,
}

pub fn spawn_export(state: Arc<Mutex<MonitorState>>, req: ExportRequest, app_handle: AppHandle) {
    if let Ok(mut s) = state.lock() {
        s.npnp_running = true;
        s.npnp_last_result = None;
    }

    emit_progress(
        &app_handle,
        "Preparing npnp export...",
        false,
        None,
        Some(req.ids.len()),
    );

    tauri::async_runtime::spawn(async move {
        let input_path = create_temp_input_path();
        let result: Result<String, String> = async {
            write_ids_file(&input_path, &req.ids)?;
            let client = LcedaClient::new();
            let summary = if req.merge {
                emit_progress(
                    &app_handle,
                    format!("Merging {} npnp items...", req.ids.len()),
                    true,
                    Some(0),
                    Some(req.ids.len()),
                );
                export_batch_merged(&client, &req, &input_path, &app_handle).await?
            } else {
                emit_progress(
                    &app_handle,
                    format!("Running npnp batch for {} items...", req.ids.len()),
                    false,
                    None,
                    Some(req.ids.len()),
                );
                export_batch(&client, build_batch_options(&req, &input_path))
                    .await
                    .map_err(|e| e.to_string())?
            };
            Ok(format_summary(&req, &summary))
        }
        .await;

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

        let _ = app_handle.emit("clipboard-changed", ());
        let _ = app_handle.emit(
            "export-finished",
            ExportFinishedPayload {
                tool: "npnp",
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
            tool: "npnp",
            message: message.into(),
            determinate,
            current,
            total,
        },
    );
}

fn create_temp_input_path() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or(0);
    std::env::temp_dir().join(format!(
        "seex_npnp_ids_{}_{}.txt",
        std::process::id(),
        stamp
    ))
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
    BatchOptions {
        input: input_path.clone(),
        output: PathBuf::from(normalize_output_path(&req.output_path)),
        schlib: mode == "schlib",
        pcblib: mode == "pcblib",
        full: mode == "full",
        merge: req.merge,
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

#[derive(Default)]
struct MergedArtifacts {
    schlib_components: Vec<Component>,
    pcblib_library: PcbLibrary,
}

struct MergedComponentResult {
    schlib_component: Option<Component>,
    pcblib_library: Option<PcbLibrary>,
}

async fn export_batch_merged(
    client: &LcedaClient,
    req: &ExportRequest,
    input_path: &Path,
    app_handle: &AppHandle,
) -> Result<BatchSummary, String> {
    let output = PathBuf::from(normalize_output_path(&req.output_path));
    fs::create_dir_all(&output).map_err(|e| e.to_string())?;

    let ids = parse_lcsc_ids(&fs::read_to_string(input_path).map_err(|e| e.to_string())?);
    if ids.is_empty() {
        return Err("no valid LCSC IDs found in batch input".to_string());
    }

    let targets = ExportTargets::resolve(&req.mode)?;
    let library_name = resolve_library_name(req);
    let merged_pcblib_file = format!("{}.PcbLib", sanitize_filename(&library_name));
    let parallel = req.parallel.max(1);
    let used_names = Arc::new(AsyncMutex::new(HashSet::new()));
    let semaphore = Arc::new(Semaphore::new(parallel));
    let mut join_set = JoinSet::new();

    for id in ids.iter().cloned() {
        let client = client.clone();
        let used_names = Arc::clone(&used_names);
        let semaphore = Arc::clone(&semaphore);
        let merged_pcblib_file = merged_pcblib_file.clone();
        join_set.spawn(async move {
            let _permit = semaphore
                .acquire_owned()
                .await
                .expect("merged export semaphore should remain open");
            export_merged_component(&client, targets, id, used_names, merged_pcblib_file).await
        });
    }

    let mut summary = BatchSummary {
        total: ids.len(),
        skipped: 0,
        success: 0,
        failed: 0,
        failed_ids: Vec::new(),
        output: output.clone(),
        generated_files: Vec::new(),
    };
    let mut artifacts = MergedArtifacts::default();
    let mut first_error = None;

    while let Some(joined) = join_set.join_next().await {
        match joined {
            Ok(Ok(result)) => {
                if let Some(component) = result.schlib_component {
                    artifacts.schlib_components.push(component);
                }
                if let Some(library) = result.pcblib_library {
                    append_pcblib_library(&mut artifacts.pcblib_library, library);
                }
                summary.success += 1;
            }
            Ok(Err((id, err))) => {
                summary.failed += 1;
                summary.failed_ids.push(id);
                if first_error.is_none() {
                    first_error = Some(err);
                }
            }
            Err(err) => {
                summary.failed += 1;
                let msg = format!("merged export task join failed: {err}");
                if first_error.is_none() {
                    first_error = Some(msg);
                }
            }
        }

        let completed = summary.success + summary.failed;
        emit_progress(
            app_handle,
            format!(
                "Merged npnp export: {} of {} components processed",
                completed, summary.total
            ),
            true,
            Some(completed),
            Some(summary.total),
        );
    }

    if summary.failed > 0 && !req.continue_on_error {
        return Err(first_error.unwrap_or_else(|| "merged export failed".to_string()));
    }

    let mut wrote_any = false;
    if targets.schlib && !artifacts.schlib_components.is_empty() {
        let path = output.join(format!("{}.SchLib", sanitize_filename(&library_name)));
        write_schlib_library(&artifacts.schlib_components, &path).map_err(|e| e.to_string())?;
        summary.generated_files.push(path);
        wrote_any = true;
    }
    if targets.pcblib && !artifacts.pcblib_library.components.is_empty() {
        let path = output.join(format!("{}.PcbLib", sanitize_filename(&library_name)));
        write_pcblib(&artifacts.pcblib_library, &path).map_err(|e| e.to_string())?;
        summary.generated_files.push(path);
        wrote_any = true;
    }

    if !wrote_any {
        return Err(first_error.unwrap_or_else(|| {
            "no components exported successfully for merged batch".to_string()
        }));
    }

    Ok(summary)
}

#[derive(Debug, Clone, Copy)]
struct ExportTargets {
    schlib: bool,
    pcblib: bool,
}

impl ExportTargets {
    fn resolve(mode: &str) -> Result<Self, String> {
        match normalize_mode(mode) {
            "schlib" => Ok(Self {
                schlib: true,
                pcblib: false,
            }),
            "pcblib" => Ok(Self {
                schlib: false,
                pcblib: true,
            }),
            _ => Ok(Self {
                schlib: true,
                pcblib: true,
            }),
        }
    }
}

async fn export_merged_component(
    client: &LcedaClient,
    targets: ExportTargets,
    id: String,
    used_names: Arc<AsyncMutex<HashSet<String>>>,
    merged_pcblib_file: String,
) -> Result<MergedComponentResult, (String, String)> {
    let result: Result<MergedComponentResult, String> = async {
        let item = client
            .select_item(&id, 1)
            .await
            .map_err(|e| e.to_string())?;
        let component_name = {
            let mut used_names = used_names.lock().await;
            merged_component_name(item.display_name(), &id, &mut used_names)
        };

        let schlib_component = if targets.schlib {
            Some(
                build_schlib_component_for_item(
                    client,
                    &item,
                    &component_name,
                    Some(&merged_pcblib_file),
                )
                .await
                .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };

        let pcblib_library = if targets.pcblib {
            Some(
                build_pcblib_library_for_item(client, &item, &component_name)
                    .await
                    .map_err(|e| e.to_string())?,
            )
        } else {
            None
        };

        Ok(MergedComponentResult {
            schlib_component,
            pcblib_library,
        })
    }
    .await;

    result.map_err(|err| (id, err))
}

fn merged_component_name(
    display_name: &str,
    lcsc_id: &str,
    used_names: &mut HashSet<String>,
) -> String {
    let base = display_name.trim();
    let base = if base.is_empty() { lcsc_id } else { base };
    if used_names.insert(base.to_ascii_lowercase()) {
        return base.to_string();
    }

    let with_id = format!("{base}_{lcsc_id}");
    if used_names.insert(with_id.to_ascii_lowercase()) {
        return with_id;
    }

    let mut index = 2usize;
    loop {
        let candidate = format!("{base}_{lcsc_id}_{index}");
        if used_names.insert(candidate.to_ascii_lowercase()) {
            return candidate;
        }
        index += 1;
    }
}

fn append_pcblib_library(target: &mut PcbLibrary, source: PcbLibrary) {
    target.components.extend(source.components);
    target.models.extend(source.models);
}

fn parse_lcsc_ids(text: &str) -> Vec<String> {
    let bytes = text.as_bytes();
    let mut ids = Vec::new();
    let mut seen = HashSet::new();
    let mut index = 0usize;

    while index < bytes.len() {
        let byte = bytes[index];
        if byte == b'C' || byte == b'c' {
            let start = index + 1;
            let mut end = start;
            while end < bytes.len() && bytes[end].is_ascii_digit() {
                end += 1;
            }

            if end > start {
                let digits = std::str::from_utf8(&bytes[start..end])
                    .expect("ASCII digits must be valid UTF-8");
                let id = format!("C{digits}");
                if seen.insert(id.clone()) {
                    ids.push(id);
                }
                index = end;
                continue;
            }
        }

        index += 1;
    }

    ids
}

fn format_summary(req: &ExportRequest, summary: &BatchSummary) -> String {
    let mut lines = vec![format!(
        "npnp batch complete. Total: {} | Skipped: {} | Success: {} | Failed: {}",
        summary.total, summary.skipped, summary.success, summary.failed,
    )];

    if req.merge {
        lines.push(format!("Targets: {} | Merge: ON", mode_label(&req.mode)));
        lines.push(format!("Parallel: {}", req.parallel.max(1)));
        lines.push("Checkpoint/force behavior: not used for merged exports".to_string());
    } else {
        lines.push(format!(
            "Targets: {} | Merge: OFF | Parallel: {}",
            mode_label(&req.mode),
            req.parallel.max(1),
        ));
    }

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
