use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
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

pub fn check_installation() -> Result<String, String> {
    let executable = resolve_nlbn_executable()?;
    let output = Command::new(&executable)
        .arg("--version")
        .output()
        .map_err(|e| format!("nlbn found but version check failed: {}", e))?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err("nlbn found but version check failed".to_string())
        } else {
            Err(format!("nlbn found but version check failed: {}", stderr))
        }
    }
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
                let _ = fs::remove_file(work_dir.join("nlbn_export.command"));
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
    let executable = resolve_nlbn_executable()?;
    let resolved_output_path = expand_user_path(output_path);
    let use_project_relative = should_use_project_relative(&resolved_output_path);

    #[cfg(target_os = "windows")]
    {
        let project_relative_arg = if use_project_relative {
            " --project-relative"
        } else {
            ""
        };
        let bat_file = work_dir.join("nlbn_export.bat");
        let bat_content = format!(
            "@echo on\r\ncd /D \"{}\"\r\n\"{}\" --full --batch \"{}\" -o \"{}\" --parallel {}{}\r\necho.\r\necho === Done ===\r\npause\r\n",
            work_dir.display(),
            executable.display(),
            temp_path,
            resolved_output_path.display(),
            parallel,
            project_relative_arg,
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

    #[cfg(target_os = "macos")]
    {
        let project_relative_arg = if use_project_relative {
            " --project-relative"
        } else {
            ""
        };
        let script_file = work_dir.join("nlbn_export.command");
        let script_path = script_file.display().to_string();
        let script_content = format!(
            "#!/bin/zsh\ncd {}\n{} --full --batch {} -o {} --parallel {}{}\necho\necho '=== Done ==='\necho 'Press Enter to exit'\nread\n",
            shell_quote(&work_dir.display().to_string()),
            shell_quote(&executable.display().to_string()),
            shell_quote(temp_path),
            shell_quote(&resolved_output_path.display().to_string()),
            parallel,
            project_relative_arg,
        );

        fs::write(&script_file, script_content)
            .map_err(|e| format!("Failed to write command file: {}", e))?;
        let mut permissions = fs::metadata(&script_file)
            .map_err(|e| format!("Failed to read command file metadata: {}", e))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&script_file, permissions)
            .map_err(|e| format!("Failed to make command file executable: {}", e))?;

        Command::new("open")
            .args(["-a", "Terminal", &script_path])
            .spawn()
            .map(|_| format!("Temp: {}\nCommand: {}", temp_path, script_file.display()))
            .map_err(|e| format!("Execution failed: {}", e))
    }

    #[cfg(not(target_os = "windows"))]
    #[cfg(not(target_os = "macos"))]
    {
        let project_relative_arg = if use_project_relative {
            " --project-relative"
        } else {
            ""
        };
        let script = format!(
            "cd \"{}\" && \"{}\" --full --batch \"{}\" -o \"{}\" --parallel {}{}; echo Press Enter to exit; read",
            work_dir.display(),
            executable.display(),
            temp_path,
            resolved_output_path.display(),
            parallel,
            project_relative_arg,
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
    let executable = resolve_nlbn_executable()?;
    let resolved_output_path = expand_user_path(output_path);
    let parallel_str = parallel.to_string();
    let resolved_output_str = resolved_output_path.display().to_string();
    let use_project_relative = should_use_project_relative(&resolved_output_path);

    #[cfg(target_os = "windows")]
    let executable_str = executable.display().to_string();

    #[cfg(target_os = "windows")]
    let result = {
        let mut command = Command::new("cmd");
        command
            .args([
                "/C",
                &executable_str,
                "--full",
                "--batch",
                temp_path,
                "-o",
                &resolved_output_str,
                "--parallel",
                &parallel_str,
            ])
            .current_dir(work_dir);
        if use_project_relative {
            command.arg("--project-relative");
        }
        command.output()
    };

    #[cfg(not(target_os = "windows"))]
    let result = {
        let mut command = Command::new(&executable);
        command
            .args([
                "--full",
                "--batch",
                temp_path,
                "-o",
                &resolved_output_str,
                "--parallel",
                &parallel_str,
            ])
            .current_dir(work_dir);
        if use_project_relative {
            command.arg("--project-relative");
        }
        command.output()
    };

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

fn resolve_nlbn_executable() -> Result<PathBuf, String> {
    #[cfg(target_os = "windows")]
    {
        return Ok(PathBuf::from("nlbn"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        if command_available("nlbn") {
            return Ok(PathBuf::from("nlbn"));
        }

        for candidate in unix_nlbn_candidates() {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        #[cfg(target_os = "macos")]
        if let Some(path) = find_with_shell("command -v nlbn") {
            return Ok(path);
        }

        #[cfg(not(target_os = "macos"))]
        if let Some(path) = find_with_shell("command -v nlbn") {
            return Ok(path);
        }

        Err("nlbn not found".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn command_available(program: &str) -> bool {
    Command::new(program)
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

#[cfg(not(target_os = "windows"))]
fn unix_nlbn_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();

    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(&home).join(".cargo/bin/nlbn"));
        candidates.push(PathBuf::from(&home).join(".local/bin/nlbn"));
    }

    candidates.push(PathBuf::from("/opt/homebrew/bin/nlbn"));
    candidates.push(PathBuf::from("/usr/local/bin/nlbn"));
    candidates.push(PathBuf::from("/opt/local/bin/nlbn"));
    candidates
}

#[cfg(target_os = "macos")]
fn find_with_shell(script: &str) -> Option<PathBuf> {
    let output = Command::new("/bin/zsh")
        .args(["-ilc", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if resolved.is_empty() {
        return None;
    }

    let path = PathBuf::from(resolved);
    path.is_file().then_some(path)
}

#[cfg(all(unix, not(target_os = "windows"), not(target_os = "macos")))]
fn find_with_shell(script: &str) -> Option<PathBuf> {
    let output = Command::new("/bin/sh")
        .args(["-lc", script])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let resolved = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if resolved.is_empty() {
        return None;
    }

    let path = PathBuf::from(resolved);
    path.is_file().then_some(path)
}

fn expand_user_path(path: &str) -> PathBuf {
    let trimmed = path.trim();
    if trimmed == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(trimmed));
    }

    if let Some(stripped) = trimmed.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }

    PathBuf::from(trimmed)
}

fn should_use_project_relative(output_path: &Path) -> bool {
    let Ok(entries) = fs::read_dir(output_path) else {
        return false;
    };

    entries.filter_map(|entry| entry.ok()).any(|entry| {
        entry
            .path()
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| matches!(ext, "kicad_pro" | "kicad_pcb" | "pro"))
            .unwrap_or(false)
    })
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
