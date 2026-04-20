use arboard::Clipboard;
use chrono::Local;
use clipboard_master::{CallbackResult, ClipboardHandler, Master, Shutdown};
use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;
use tauri::{AppHandle, Emitter};

use crate::extract::extract_by_keyword;

pub struct MonitorState {
    pub last_content: String,
    pub history: Vec<(String, String)>,
    pub matched: Vec<(String, String)>,
    pub keyword: String,
    pub initialized: bool,
    pub monitoring: bool,
    pub match_debug_log: Vec<String>,
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
}

impl MonitorState {
    pub fn new() -> Self {
        Self {
            last_content: String::new(),
            history: Vec::new(),
            matched: Vec::new(),
            keyword: String::new(),
            initialized: false,
            monitoring: true,
            match_debug_log: Vec::new(),
            nlbn_output_path: "~/lib".to_string(),
            nlbn_last_result: None,
            nlbn_show_terminal: true,
            nlbn_parallel: 4,
            nlbn_running: false,
            npnp_output_path: "npnp_export".to_string(),
            npnp_last_result: None,
            npnp_running: false,
            npnp_mode: "full".to_string(),
            npnp_merge: false,
            npnp_library_name: "SeExMerged".to_string(),
            npnp_parallel: 4,
            npnp_continue_on_error: true,
            npnp_force: false,
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
            if self.matched.len() > 100 {
                self.matched.truncate(100);
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
                    if self.matched.len() > 100 {
                        self.matched.pop();
                    }
                    self.add_debug_log(format!("Matched: {}", extracted));
                }
            } else {
                self.add_debug_log("No match found".to_string());
            }
        }
        true
    }

    pub fn set_nlbn_output_path(&mut self, path: String) {
        self.nlbn_output_path = path;
    }

    pub fn toggle_nlbn_show_terminal(&mut self) {
        self.nlbn_show_terminal = !self.nlbn_show_terminal;
    }

    pub fn set_nlbn_parallel(&mut self, parallel: usize) {
        self.nlbn_parallel = parallel.max(1);
    }

    pub fn set_npnp_output_path(&mut self, path: String) {
        self.npnp_output_path = path;
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

    pub fn get_unique_ids(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        self.matched
            .iter()
            .filter(|(_, id)| seen.insert(id.clone()))
            .map(|(_, id)| id.clone())
            .collect()
    }
}

struct Handler {
    state: Arc<Mutex<MonitorState>>,
    app_handle: AppHandle,
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
        let _ = self.app_handle.emit("clipboard-changed", ());
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
    pub fn spawn(state: Arc<Mutex<MonitorState>>, app_handle: AppHandle) -> Self {
        if let Ok(mut s) = state.lock() {
            s.initialized = true;
        }

        let stop = Arc::new(AtomicBool::new(false));

        let (tx, rx) = mpsc::channel::<Shutdown>();
        let state_ev = Arc::clone(&state);
        let app_ev = app_handle.clone();
        let event_thread = thread::spawn(move || {
            let handler = Handler {
                state: state_ev,
                app_handle: app_ev,
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
        let app_poll = app_handle.clone();
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
                                let _ = app_poll.emit("clipboard-changed", ());
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
