use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::monitor::MonitorState;

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
    pub show_terminal: bool,
    pub parallel: usize,
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

    thread::spawn(move || {
        let exe_dir = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.to_path_buf()))
            .unwrap_or_else(|| PathBuf::from("."));

        let temp_file = exe_dir.join("nlbn_ids.txt");

        let write_result = (|| -> std::io::Result<()> {
            let mut file = fs::File::create(&temp_file)?;
            for id in &req.ids {
                writeln!(file, "{}", id)?;
            }
            file.sync_all()?;
            Ok(())
        })();

        match write_result {
            Ok(_) => {
                let temp_str = temp_file.display().to_string();
                let work_dir = temp_file.parent().unwrap_or(Path::new(".")).to_path_buf();

                emit_progress(
                    &app_handle,
                    if req.show_terminal {
                        "Launching nlbn terminal..."
                    } else {
                        "Running nlbn export..."
                    },
                    false,
                    None,
                    Some(req.ids.len()),
                );

                let result = if req.show_terminal {
                    run_in_terminal(&temp_str, &req.output_path, req.parallel, &work_dir)
                } else {
                    run_in_background(&temp_str, &req.output_path, req.parallel, &work_dir)
                };

                let mut notify_payload = None;
                if let Ok(mut s) = state.lock() {
                    s.nlbn_running = false;
                    match result {
                        Ok(msg) => {
                            let full = format!(
                                "{} ({} items -> {})\n{}",
                                if req.show_terminal {
                                    "Terminal launched"
                                } else {
                                    "nlbn completed"
                                },
                                req.ids.len(),
                                req.output_path,
                                msg,
                            );
                            s.nlbn_last_result = Some(full.clone());
                            s.add_debug_log(full.clone());
                            if !req.show_terminal {
                                notify_payload = Some(ExportFinishedPayload {
                                    tool: "nlbn",
                                    success: true,
                                    message: full,
                                });
                            }
                        }
                        Err(msg) => {
                            s.nlbn_last_result = Some(msg.clone());
                            s.add_debug_log(msg.clone());
                            notify_payload = Some(ExportFinishedPayload {
                                tool: "nlbn",
                                success: false,
                                message: msg,
                            });
                        }
                    }
                }

                let _ = app_handle.emit("clipboard-changed", ());
                if let Some(payload) = notify_payload {
                    let _ = app_handle.emit("export-finished", payload);
                }

                if req.show_terminal {
                    thread::sleep(Duration::from_secs(30));
                } else {
                    thread::sleep(Duration::from_secs(2));
                }
                let _ = fs::remove_file(&temp_file);
                let _ = fs::remove_file(work_dir.join("nlbn_export.bat"));
            }
            Err(e) => {
                emit_progress(
                    &app_handle,
                    "nlbn export failed before start",
                    false,
                    None,
                    Some(req.ids.len()),
                );
                let msg = format!(
                    "Failed to create temp file: {}\nPath: {}",
                    e,
                    temp_file.display()
                );
                if let Ok(mut s) = state.lock() {
                    s.nlbn_running = false;
                    s.nlbn_last_result = Some(msg.clone());
                    s.add_debug_log(msg.clone());
                }
                let _ = app_handle.emit("clipboard-changed", ());
                let _ = app_handle.emit(
                    "export-finished",
                    ExportFinishedPayload {
                        tool: "nlbn",
                        success: false,
                        message: msg,
                    },
                );
            }
        }
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

fn run_in_terminal(
    temp_path: &str,
    output_path: &str,
    parallel: usize,
    work_dir: &Path,
) -> Result<String, String> {
    #[cfg(target_os = "windows")]
    {
        let bat_file = work_dir.join("nlbn_export.bat");
        let bat_content = format!(
            "@echo on\r\ncd /D \"{}\"\r\nnlbn --full --batch \"{}\" -o \"{}\" --parallel {}\r\necho.\r\necho === Done ===\r\npause\r\n",
            work_dir.display(),
            temp_path,
            output_path,
            parallel,
        );
        fs::write(&bat_file, &bat_content)
            .map_err(|e| format!("Failed to write batch file: {}", e))?;

        Command::new("cmd")
            .raw_arg(format!(
                "/C start \"nlbn export\" \"{}\"",
                bat_file.display()
            ))
            .spawn()
            .map(|_| format!("Temp: {}\nBatch: {}", temp_path, bat_file.display()))
            .map_err(|e| format!("Execution failed: {}", e))
    }

    #[cfg(not(target_os = "windows"))]
    {
        let script = format!(
            "cd \"{}\" && nlbn --full --batch \"{}\" -o \"{}\" --parallel {}; echo Press Enter to exit; read",
            work_dir.display(),
            temp_path,
            output_path,
            parallel,
        );
        Command::new("gnome-terminal")
            .args(["--", "bash", "-c", &script])
            .spawn()
            .map(|_| format!("Temp: {}", temp_path))
            .map_err(|e| {
                format!(
                    "Execution failed: {}\nMake sure nlbn is installed and in PATH",
                    e
                )
            })
    }
}

fn run_in_background(
    temp_path: &str,
    output_path: &str,
    parallel: usize,
    work_dir: &Path,
) -> Result<String, String> {
    let parallel_str = parallel.to_string();

    #[cfg(target_os = "windows")]
    let result = Command::new("cmd")
        .args([
            "/C",
            "nlbn",
            "--full",
            "--batch",
            temp_path,
            "-o",
            output_path,
            "--parallel",
            &parallel_str,
        ])
        .current_dir(work_dir)
        .output();

    #[cfg(not(target_os = "windows"))]
    let result = Command::new("nlbn")
        .args([
            "--full",
            "--batch",
            temp_path,
            "-o",
            output_path,
            "--parallel",
            &parallel_str,
        ])
        .current_dir(work_dir)
        .output();

    match result {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            if output.status.success() {
                Ok(format!("Output: {}", stdout))
            } else {
                Err(format!(
                    "nlbn failed\nstdout: {}\nstderr: {}",
                    stdout, stderr
                ))
            }
        }
        Err(e) => Err(format!(
            "Execution failed: {}\nMake sure nlbn is installed and in PATH",
            e
        )),
    }
}
