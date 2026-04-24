use serde::Serialize;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

#[cfg(target_os = "macos")]
use std::os::unix::fs::PermissionsExt;
#[cfg(target_os = "windows")]
use std::os::windows::process::CommandExt;

use crate::app_paths::AppPaths;
use crate::config::NlbnPathMode;
use crate::monitor::MonitorState;

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
    pub show_terminal: bool,
    pub parallel: usize,
    pub path_mode: NlbnPathMode,
    pub overwrite: bool,
    pub symbol_fill_color: Option<String>,
}

pub fn check_installation() -> Result<String, String> {
    let installation = inspect_installation()?;
    Ok(format!(
        "{} ({})",
        installation.version,
        installation.executable.display()
    ))
}

pub fn spawn_export(
    state: Arc<Mutex<MonitorState>>,
    req: ExportRequest,
    paths: AppPaths,
    callbacks: ExportCallbacks,
) -> Result<(), String> {
    let installation = match inspect_installation() {
        Ok(installation) => installation,
        Err(message) => {
            if let Ok(mut s) = state.lock() {
                s.nlbn_running = false;
                s.nlbn_last_result = Some(message.clone());
                s.add_debug_log(message.clone());
            }
            notify_state_changed(&callbacks);
            emit_finished(
                &callbacks,
                ExportFinishedPayload {
                    tool: "nlbn",
                    success: false,
                    message: message.clone(),
                },
            );
            return Err(message);
        }
    };

    if let Ok(mut s) = state.lock() {
        s.nlbn_running = true;
        s.nlbn_last_result = None;
    }

    emit_progress(
        &callbacks,
        "Preparing nlbn export...",
        false,
        None,
        Some(req.ids.len()),
    );

    thread::spawn(move || {
        let executable = installation.executable;
        let temp_file = paths.cache_file("nlbn_ids", "txt");

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
                let work_dir = paths.cache_dir().to_path_buf();
                let script_base = paths.cache_file("nlbn_export", "command");
                let windows_script = script_base.with_extension("bat");
                let unix_script = script_base.with_extension("command");

                emit_progress(
                    &callbacks,
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
                    run_in_terminal(
                        &executable,
                        &temp_str,
                        &req.output_path,
                        req.parallel,
                        req.path_mode,
                        req.overwrite,
                        req.symbol_fill_color.as_deref(),
                        &work_dir,
                        &windows_script,
                        &unix_script,
                    )
                } else {
                    run_in_background(
                        &executable,
                        &temp_str,
                        &req.output_path,
                        req.parallel,
                        req.path_mode,
                        req.overwrite,
                        req.symbol_fill_color.as_deref(),
                        &work_dir,
                    )
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
                            notify_payload = Some(ExportFinishedPayload {
                                tool: "nlbn",
                                success: true,
                                message: full,
                            });
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

                notify_state_changed(&callbacks);
                if let Some(payload) = notify_payload {
                    emit_finished(&callbacks, payload);
                }

                if req.show_terminal {
                    thread::sleep(Duration::from_secs(30));
                } else {
                    thread::sleep(Duration::from_secs(2));
                }
                let _ = fs::remove_file(&temp_file);
                let _ = fs::remove_file(&windows_script);
                let _ = fs::remove_file(&unix_script);
            }
            Err(e) => {
                emit_progress(
                    &callbacks,
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
                notify_state_changed(&callbacks);
                emit_finished(
                    &callbacks,
                    ExportFinishedPayload {
                        tool: "nlbn",
                        success: false,
                        message: msg,
                    },
                );
            }
        }
    });

    Ok(())
}

#[derive(Debug, Clone)]
struct NlbnInstallation {
    executable: PathBuf,
    version: String,
    capabilities: NlbnCapabilities,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct NlbnCapabilities {
    overwrite: bool,
    symbol_fill_color: bool,
}

fn inspect_installation() -> Result<NlbnInstallation, String> {
    let executable = resolve_nlbn_executable()?;
    let help = run_nlbn_text_command(&executable, "--help")
        .map_err(|e| format!("nlbn help failed: {}", e))?;
    let capabilities = parse_capabilities(&help);
    let version = run_nlbn_text_command(&executable, "--version")
        .unwrap_or_else(|_| "nlbn (version unavailable)".to_string());

    let installation = NlbnInstallation {
        executable,
        version,
        capabilities,
    };
    validate_installation(&installation)?;
    Ok(installation)
}

fn validate_installation(installation: &NlbnInstallation) -> Result<(), String> {
    if !installation.capabilities.symbol_fill_color {
        return Err(format!(
            "Installed nlbn is too old: {} at {}.\nseex now requires a newer nlbn with --symbol-fill-color support.\nPlease upgrade nlbn and retry.",
            installation.version,
            installation.executable.display()
        ));
    }

    if !installation.capabilities.overwrite {
        return Err(format!(
            "Installed nlbn is incomplete: {} at {}.\nseex requires nlbn with --overwrite support.\nPlease upgrade nlbn and retry.",
            installation.version,
            installation.executable.display()
        ));
    }

    Ok(())
}

fn run_nlbn_text_command(executable: &Path, arg: &str) -> Result<String, String> {
    let output = Command::new(executable)
        .arg(arg)
        .output()
        .map_err(|e| e.to_string())?;

    if output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if stdout.is_empty() {
            Err(format!("{arg} returned no output"))
        } else {
            Ok(stdout)
        }
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if stderr.is_empty() {
            Err(format!("{arg} exited with status {}", output.status))
        } else {
            Err(stderr)
        }
    }
}

fn parse_capabilities(help: &str) -> NlbnCapabilities {
    NlbnCapabilities {
        overwrite: help.contains("--overwrite"),
        symbol_fill_color: help.contains("--symbol-fill-color"),
    }
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
            tool: "nlbn",
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

fn run_in_terminal(
    executable: &Path,
    temp_path: &str,
    output_path: &str,
    parallel: usize,
    path_mode: NlbnPathMode,
    overwrite: bool,
    symbol_fill_color: Option<&str>,
    work_dir: &Path,
    #[cfg(target_os = "windows")] windows_script_path: &Path,
    #[cfg(not(target_os = "windows"))] _windows_script_path: &Path,
    unix_script_path: &Path,
) -> Result<String, String> {
    let resolved_output_path = expand_user_path(output_path);
    let use_project_relative = resolve_project_relative_mode(path_mode, &resolved_output_path);

    #[cfg(target_os = "windows")]
    {
        let project_relative_arg = if use_project_relative {
            " --project-relative"
        } else {
            ""
        };
        let overwrite_arg = if overwrite { " --overwrite" } else { "" };
        let symbol_fill_color_arg = normalized_symbol_fill_color_arg(symbol_fill_color)
            .map(|value| format!(" --symbol-fill-color \"{}\"", value))
            .unwrap_or_default();
        let bat_content = format!(
            "@echo on\r\ncd /D \"{}\"\r\n\"{}\" --full --batch \"{}\" -o \"{}\" --parallel {}{}{}{}\r\necho.\r\necho === Done ===\r\npause\r\n",
            work_dir.display(),
            executable.display(),
            temp_path,
            resolved_output_path.display(),
            parallel,
            project_relative_arg,
            overwrite_arg,
            symbol_fill_color_arg,
        );
        fs::write(windows_script_path, &bat_content)
            .map_err(|e| format!("Failed to write batch file: {}", e))?;

        Command::new("cmd")
            .raw_arg(format!(
                "/C start \"nlbn export\" \"{}\"",
                windows_script_path.display()
            ))
            .spawn()
            .map(|_| {
                format!(
                    "Temp: {}\nBatch: {}",
                    temp_path,
                    windows_script_path.display()
                )
            })
            .map_err(|e| format!("Execution failed: {}", e))
    }

    #[cfg(target_os = "macos")]
    {
        let project_relative_arg = if use_project_relative {
            " --project-relative"
        } else {
            ""
        };
        let overwrite_arg = if overwrite { " --overwrite" } else { "" };
        let symbol_fill_color_arg = normalized_symbol_fill_color_arg(symbol_fill_color)
            .map(|value| format!(" --symbol-fill-color {}", shell_quote(value)))
            .unwrap_or_default();
        let script_path = unix_script_path.display().to_string();
        let script_content = format!(
            "#!/bin/zsh\ncd {}\n{} --full --batch {} -o {} --parallel {}{}{}{}\necho\necho '=== Done ==='\necho 'Press Enter to exit'\nread\n",
            shell_quote(&work_dir.display().to_string()),
            shell_quote(&executable.display().to_string()),
            shell_quote(temp_path),
            shell_quote(&resolved_output_path.display().to_string()),
            parallel,
            project_relative_arg,
            overwrite_arg,
            symbol_fill_color_arg,
        );

        fs::write(unix_script_path, script_content)
            .map_err(|e| format!("Failed to write command file: {}", e))?;
        let mut permissions = fs::metadata(unix_script_path)
            .map_err(|e| format!("Failed to read command file metadata: {}", e))?
            .permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(unix_script_path, permissions)
            .map_err(|e| format!("Failed to make command file executable: {}", e))?;

        Command::new("open")
            .args(["-a", "Terminal", &script_path])
            .spawn()
            .map(|_| {
                format!(
                    "Temp: {}\nCommand: {}",
                    temp_path,
                    unix_script_path.display()
                )
            })
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
        let overwrite_arg = if overwrite { " --overwrite" } else { "" };
        let symbol_fill_color_arg = normalized_symbol_fill_color_arg(symbol_fill_color)
            .map(|value| format!(" --symbol-fill-color \"{}\"", value))
            .unwrap_or_default();
        let script = format!(
            "cd \"{}\" && \"{}\" --full --batch \"{}\" -o \"{}\" --parallel {}{}{}{}; echo Press Enter to exit; read",
            work_dir.display(),
            executable.display(),
            temp_path,
            resolved_output_path.display(),
            parallel,
            project_relative_arg,
            overwrite_arg,
            symbol_fill_color_arg,
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
    executable: &Path,
    temp_path: &str,
    output_path: &str,
    parallel: usize,
    path_mode: NlbnPathMode,
    overwrite: bool,
    symbol_fill_color: Option<&str>,
    work_dir: &Path,
) -> Result<String, String> {
    let resolved_output_path = expand_user_path(output_path);
    let parallel_str = parallel.to_string();
    let resolved_output_str = resolved_output_path.display().to_string();
    let use_project_relative = resolve_project_relative_mode(path_mode, &resolved_output_path);

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
        if overwrite {
            command.arg("--overwrite");
        }
        if let Some(color) = normalized_symbol_fill_color_arg(symbol_fill_color) {
            command.args(["--symbol-fill-color", color]);
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
        if overwrite {
            command.arg("--overwrite");
        }
        if let Some(color) = normalized_symbol_fill_color_arg(symbol_fill_color) {
            command.args(["--symbol-fill-color", color]);
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
        if let Some(path) = find_with_shell("command -v nlbn") {
            return Ok(path);
        }

        if let Some(path) = find_in_path("nlbn") {
            return Ok(path);
        }

        for candidate in unix_nlbn_candidates() {
            if candidate.is_file() {
                return Ok(candidate);
            }
        }

        Err("nlbn not found".to_string())
    }
}

#[cfg(not(target_os = "windows"))]
fn find_in_path(program: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;

    std::env::split_paths(&path)
        .map(|dir| dir.join(program))
        .find(|candidate| candidate.is_file())
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

fn resolve_project_relative_mode(path_mode: NlbnPathMode, output_path: &Path) -> bool {
    match path_mode {
        NlbnPathMode::Auto => should_use_project_relative(output_path),
        NlbnPathMode::ProjectRelative => true,
        NlbnPathMode::LibraryRelative => false,
    }
}

fn normalized_symbol_fill_color_arg(symbol_fill_color: Option<&str>) -> Option<&str> {
    symbol_fill_color
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::{NlbnCapabilities, NlbnInstallation, parse_capabilities, validate_installation};
    use std::path::PathBuf;

    #[test]
    fn detects_required_capabilities_from_help_output() {
        let caps = parse_capabilities(
            "\
Options:\n\
      --overwrite            Overwrite existing components\n\
      --symbol-fill-color    Override filled symbol rectangle color\n",
        );

        assert_eq!(
            caps,
            NlbnCapabilities {
                overwrite: true,
                symbol_fill_color: true,
            }
        );
    }

    #[test]
    fn rejects_old_nlbn_without_symbol_fill_color_support() {
        let installation = NlbnInstallation {
            executable: PathBuf::from("/opt/nlbn"),
            version: "nlbn 1.0.25".to_string(),
            capabilities: NlbnCapabilities {
                overwrite: true,
                symbol_fill_color: false,
            },
        };

        let err = validate_installation(&installation).expect_err("should reject old nlbn");
        assert!(err.contains("too old"));
        assert!(err.contains("--symbol-fill-color"));
    }
}
