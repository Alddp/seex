use arboard::Clipboard;
use chrono::Local;
use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::app_paths::AppPaths;
use crate::config::NlbnPathMode;
use crate::extract::extract_by_keyword;

pub type NotifyFn = Arc<dyn Fn() + Send + Sync + 'static>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchedMergeStats {
    pub added: usize,
    pub already_present: usize,
}

pub struct MonitorState {
    pub last_content: String,
    pub history: Vec<(String, String)>,
    pub matched: Vec<(String, String)>,
    pub keyword: String,
    pub initialized: bool,
    pub monitoring: bool,
    pub always_on_top: bool,
    pub match_debug_log: Vec<String>,
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
    pub history_save_path: String,
    pub matched_save_path: String,
    pub imported_parts_save_path: String,
    pub window_width: Option<u32>,
    pub window_height: Option<u32>,
    default_nlbn_output_path: String,
    default_npnp_output_path: String,
}

impl MonitorState {
    pub fn new(paths: &AppPaths) -> Self {
        let default_nlbn_output_path = paths.default_nlbn_output_path_string();
        let default_npnp_output_path = paths.default_npnp_output_path_string();
        Self {
            last_content: String::new(),
            history: Vec::new(),
            matched: Vec::new(),
            keyword: String::new(),
            initialized: false,
            monitoring: true,
            always_on_top: false,
            match_debug_log: Vec::new(),
            nlbn_output_path: default_nlbn_output_path.clone(),
            nlbn_last_result: None,
            nlbn_show_terminal: true,
            nlbn_parallel: 4,
            nlbn_path_mode: NlbnPathMode::Auto,
            nlbn_export_symbol: true,
            nlbn_export_footprint: true,
            nlbn_export_model_3d: true,
            nlbn_overwrite_symbol: false,
            nlbn_overwrite_footprint: false,
            nlbn_overwrite_model_3d: false,
            nlbn_symbol_fill_color: None,
            nlbn_running: false,
            npnp_output_path: default_npnp_output_path.clone(),
            npnp_last_result: None,
            npnp_running: false,
            npnp_mode: "full".to_string(),
            npnp_merge: false,
            npnp_append: false,
            npnp_library_name: "SeExMerged".to_string(),
            npnp_parallel: 4,
            npnp_continue_on_error: true,
            npnp_force: false,
            history_save_path: paths.default_history_save_path_string(),
            matched_save_path: paths.default_matched_save_path_string(),
            imported_parts_save_path: paths.default_imported_parts_save_path_string(),
            window_width: None,
            window_height: None,
            default_nlbn_output_path,
            default_npnp_output_path,
        }
    }

    pub fn add_debug_log(&mut self, msg: String) {
        self.match_debug_log.insert(0, msg);
        if self.match_debug_log.len() > 50 {
            self.match_debug_log.pop();
        }
    }

    pub fn set_keyword(&mut self, keyword: String) {
        self.add_debug_log(format!("Keyword set: [{}]", &keyword));
        self.keyword = keyword;
        self.rematch_history();
    }

    fn rematch_history(&mut self) {
        if self.keyword.is_empty() {
            return;
        }
        let existing: HashSet<String> = self.matched.iter().map(|(_, id)| id.clone()).collect();
        let mut new_matches: Vec<(String, String)> = Vec::new();
        for (time, content) in &self.history {
            if let Some(extracted) = extract_by_keyword(content, &self.keyword) {
                if !existing.contains(&extracted)
                    && !new_matches.iter().any(|(_, id)| id == &extracted)
                {
                    new_matches.push((time.clone(), extracted));
                }
            }
        }
        if !new_matches.is_empty() {
            let count = new_matches.len();
            for item in new_matches.into_iter().rev() {
                self.matched.insert(0, item);
            }
            self.add_debug_log(format!("Rematch: found {} new results from history", count));
        }
    }

    pub fn process_clipboard_change(&mut self, content: String) -> bool {
        if !self.monitoring {
            return false;
        }

        let trimmed = content.trim().to_string();
        if trimmed.is_empty() {
            return false;
        }

        if !self.initialized {
            self.last_content = trimmed;
            self.initialized = true;
            self.add_debug_log("Listener initialized".to_string());
            return false;
        }

        if trimmed == self.last_content {
            return false;
        }

        self.last_content = trimmed.clone();
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.add_debug_log(format!(
            "New content: {}",
            trimmed.chars().take(50).collect::<String>()
        ));

        self.history.insert(0, (timestamp.clone(), trimmed.clone()));
        if self.history.len() > 50 {
            self.history.pop();
        }

        if !self.keyword.is_empty() {
            if let Some(extracted) = extract_by_keyword(&trimmed, &self.keyword) {
                if !self.matched.iter().any(|(_, id)| id == &extracted) {
                    self.matched.insert(0, (timestamp, extracted.clone()));
                    self.add_debug_log(format!("Matched: {}", extracted));
                }
            } else {
                self.add_debug_log("No match found".to_string());
            }
        }
        true
    }

    pub fn set_nlbn_output_path(&mut self, path: String) {
        let trimmed = path.trim();
        self.nlbn_output_path = if trimmed.is_empty() {
            self.default_nlbn_output_path.clone()
        } else {
            resolve_user_path(trimmed)
        };
    }

    pub fn toggle_nlbn_show_terminal(&mut self) {
        self.nlbn_show_terminal = !self.nlbn_show_terminal;
    }

    pub fn set_nlbn_parallel(&mut self, parallel: usize) {
        self.nlbn_parallel = parallel.max(1);
    }

    pub fn set_nlbn_path_mode(&mut self, path_mode: NlbnPathMode) {
        self.nlbn_path_mode = path_mode;
    }

    pub fn set_nlbn_export_symbol(&mut self, enabled: bool) {
        self.nlbn_export_symbol = enabled;
        if !enabled {
            self.nlbn_overwrite_symbol = false;
        }
    }

    pub fn set_nlbn_export_footprint(&mut self, enabled: bool) {
        self.nlbn_export_footprint = enabled;
        if !enabled {
            self.nlbn_overwrite_footprint = false;
        }
    }

    pub fn set_nlbn_export_model_3d(&mut self, enabled: bool) {
        self.nlbn_export_model_3d = enabled;
        if !enabled {
            self.nlbn_overwrite_model_3d = false;
        }
    }

    pub fn set_nlbn_overwrite_symbol(&mut self, overwrite: bool) {
        self.nlbn_overwrite_symbol = self.nlbn_export_symbol && overwrite;
    }

    pub fn set_nlbn_overwrite_footprint(&mut self, overwrite: bool) {
        self.nlbn_overwrite_footprint = self.nlbn_export_footprint && overwrite;
    }

    pub fn set_nlbn_overwrite_model_3d(&mut self, overwrite: bool) {
        self.nlbn_overwrite_model_3d = self.nlbn_export_model_3d && overwrite;
    }

    pub fn set_nlbn_symbol_fill_color(&mut self, color: Option<String>) {
        self.nlbn_symbol_fill_color = color.and_then(|value| {
            let trimmed = value.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        });
    }

    pub fn set_npnp_output_path(&mut self, path: String) {
        let trimmed = path.trim();
        self.npnp_output_path = if trimmed.is_empty() {
            self.default_npnp_output_path.clone()
        } else {
            resolve_user_path(trimmed)
        };
    }

    pub fn set_npnp_mode(&mut self, mode: String) {
        self.npnp_mode = match mode.trim().to_ascii_lowercase().as_str() {
            "schlib" => "schlib".to_string(),
            "pcblib" => "pcblib".to_string(),
            _ => "full".to_string(),
        };
    }

    pub fn set_npnp_merge(&mut self, merge: bool) {
        self.npnp_merge = merge;
        if !merge {
            self.npnp_append = false;
        }
    }

    pub fn set_npnp_append(&mut self, append: bool) {
        self.npnp_append = append;
        if append {
            self.npnp_merge = true;
        }
    }

    pub fn set_npnp_library_name(&mut self, library_name: String) {
        self.npnp_library_name = library_name;
    }

    pub fn set_npnp_parallel(&mut self, parallel: usize) {
        self.npnp_parallel = parallel.max(1);
    }

    pub fn set_npnp_continue_on_error(&mut self, continue_on_error: bool) {
        self.npnp_continue_on_error = continue_on_error;
    }

    pub fn set_npnp_force(&mut self, force: bool) {
        self.npnp_force = force;
    }

    pub fn set_history_save_path(&mut self, path: String, paths: &AppPaths) {
        self.history_save_path = paths.resolve_history_save_path(&path);
    }

    pub fn set_matched_save_path(&mut self, path: String, paths: &AppPaths) {
        self.matched_save_path = paths.resolve_matched_save_path(&path);
    }

    pub fn set_imported_parts_save_path(&mut self, path: String, paths: &AppPaths) {
        self.imported_parts_save_path = paths.resolve_imported_parts_save_path(&path);
    }

    pub fn set_window_size(&mut self, width: u32, height: u32) {
        self.window_width = Some(width);
        self.window_height = Some(height);
    }

    pub fn nlbn_has_export_targets(&self) -> bool {
        self.nlbn_export_symbol || self.nlbn_export_footprint || self.nlbn_export_model_3d
    }

    pub fn delete_history(&mut self, index: usize) {
        if index < self.history.len() {
            self.history.remove(index);
        }
    }

    pub fn delete_matched(&mut self, index: usize) {
        if index < self.matched.len() {
            self.matched.remove(index);
        }
    }

    pub fn merge_matched_ids<I>(&mut self, ids: I, timestamp: String) -> MatchedMergeStats
    where
        I: IntoIterator<Item = String>,
    {
        let mut seen: HashSet<String> = self.matched.iter().map(|(_, id)| id.clone()).collect();
        let mut additions = Vec::new();
        let mut already_present = 0usize;

        for id in ids {
            if seen.insert(id.clone()) {
                additions.push((timestamp.clone(), id));
            } else {
                already_present += 1;
            }
        }

        let added = additions.len();
        for item in additions.into_iter().rev() {
            self.matched.insert(0, item);
        }

        MatchedMergeStats {
            added,
            already_present,
        }
    }

    pub fn get_unique_ids(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.matched
            .iter()
            .filter(|(_, id)| seen.insert(id.clone()))
            .map(|(_, id)| id.clone())
            .collect()
    }
}

fn resolve_user_path(value: &str) -> String {
    let expanded = expand_home_path(value);
    if expanded.is_absolute() {
        expanded.display().to_string()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(expanded)
            .display()
            .to_string()
    }
}

fn expand_home_path(value: &str) -> PathBuf {
    if value == "~" {
        return std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(value));
    }

    if let Some(stripped) = value.strip_prefix("~/") {
        if let Some(home) = std::env::var_os("HOME") {
            return PathBuf::from(home).join(stripped);
        }
    }

    PathBuf::from(value)
}

struct Handler {
    state: Arc<Mutex<MonitorState>>,
    notifier: NotifyFn,
}

impl ClipboardHandler for Handler {
    fn on_clipboard_change(&mut self) -> CallbackResult {
        if let Ok(mut clip) = Clipboard::new() {
            match clip.get_text() {
                Ok(content) => {
                    if let Ok(mut s) = self.state.lock() {
                        s.process_clipboard_change(content);
                    }
                }
                Err(e) => {
                    let msg = e.to_string();
                    if !msg.to_lowercase().contains("empty") && !msg.contains("format") {
                        if let Ok(mut s) = self.state.lock() {
                            s.add_debug_log(format!("Clipboard read error: {}", msg));
                        }
                    }
                }
            }
        }
        (self.notifier)();
        CallbackResult::Next
    }

    fn on_clipboard_error(&mut self, error: std::io::Error) -> CallbackResult {
        if let Ok(mut s) = self.state.lock() {
            s.add_debug_log(format!("Clipboard listener error: {}", error));
        }
        CallbackResult::Next
    }
}

pub struct MonitorHandle {
    _shutdown: Option<Shutdown>,
    stop: Arc<AtomicBool>,
    _event_thread: Option<JoinHandle<()>>,
    _poll_thread: Option<JoinHandle<()>>,
}

impl MonitorHandle {
    pub fn spawn_with_callback(state: Arc<Mutex<MonitorState>>, notifier: NotifyFn) -> Self {
        if let Ok(mut s) = state.lock() {
            s.initialized = true;
        }

        let stop = Arc::new(AtomicBool::new(false));

        let (tx, rx) = mpsc::channel::<Shutdown>();
        let state_ev = Arc::clone(&state);
        let notify_ev = Arc::clone(&notifier);
        let event_thread = thread::spawn(move || {
            let handler = Handler {
                state: state_ev,
                notifier: notify_ev,
            };
            let mut master = match Master::new(handler) {
                Ok(m) => m,
                Err(e) => {
                    eprintln!("clipboard-master init failed: {}", e);
                    return;
                }
            };
            let shutdown = master.shutdown_channel();
            let _ = tx.send(shutdown);
            let _ = master.run();
        });
        let shutdown = rx.recv().ok();

        let state_poll = Arc::clone(&state);
        let notify_poll = Arc::clone(&notifier);
        let stop_poll = Arc::clone(&stop);
        let poll_thread = thread::spawn(move || {
            thread::sleep(Duration::from_millis(500));
            let mut clipboard: Option<Clipboard> = None;

            while !stop_poll.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(300));

                if clipboard.is_none() {
                    clipboard = Clipboard::new().ok();
                }

                if let Some(ref mut clip) = clipboard {
                    match clip.get_text() {
                        Ok(content) => {
                            let changed = if let Ok(mut s) = state_poll.lock() {
                                s.process_clipboard_change(content)
                            } else {
                                false
                            };
                            if changed {
                                (notify_poll)();
                            }
                        }
                        Err(_) => {
                            clipboard = None;
                        }
                    }
                }
            }
        });

        Self {
            _shutdown: shutdown,
            stop,
            _event_thread: Some(event_thread),
            _poll_thread: Some(poll_thread),
        }
    }

    pub fn spawn(state: Arc<Mutex<MonitorState>>) -> Self {
        Self::spawn_with_callback(state, Arc::new(|| {}))
    }
}

impl Drop for MonitorHandle {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        drop(self._shutdown.take());
        if let Some(t) = self._event_thread.take() {
            let _ = t.join();
        }
        if let Some(t) = self._poll_thread.take() {
            let _ = t.join();
        }
    }
}
