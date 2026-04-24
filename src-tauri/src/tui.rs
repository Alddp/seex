use std::cmp::min;
use std::error::Error;
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Tabs, Wrap};

use crate::config::NlbnPathMode;
use crate::controller::AppController;

const TICK_RATE: Duration = Duration::from_millis(180);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FocusPane {
    Queue,
    Trace,
    Run,
    Clipboard,
}

impl FocusPane {
    fn next(self) -> Self {
        match self {
            Self::Queue => Self::Trace,
            Self::Trace => Self::Run,
            Self::Run => Self::Clipboard,
            Self::Clipboard => Self::Queue,
        }
    }

    fn prev(self) -> Self {
        match self {
            Self::Queue => Self::Clipboard,
            Self::Trace => Self::Queue,
            Self::Run => Self::Trace,
            Self::Clipboard => Self::Run,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TraceTab {
    History,
    Log,
    Result,
}

impl TraceTab {
    fn next(self) -> Self {
        match self {
            Self::History => Self::Log,
            Self::Log => Self::Result,
            Self::Result => Self::History,
        }
    }

    fn titles() -> [&'static str; 3] {
        ["History", "Log", "Result"]
    }

    fn index(self) -> usize {
        match self {
            Self::History => 0,
            Self::Log => 1,
            Self::Result => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Exporter {
    Nlbn,
    Npnp,
}

impl Exporter {
    fn toggle(self) -> Self {
        match self {
            Self::Nlbn => Self::Npnp,
            Self::Npnp => Self::Nlbn,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Nlbn => "nlbn",
            Self::Npnp => "npnp",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RunField {
    Export,
    OutputPath,
    Terminal,
    Parallel,
    PathMode,
    Overwrite,
    Mode,
    Merge,
    Append,
    LibraryName,
    ContinueOnError,
    Force,
}

#[derive(Clone, Debug)]
struct InputMode {
    field: RunField,
    buffer: String,
}

#[derive(Clone, Debug)]
struct UiState {
    focus: FocusPane,
    trace_tab: TraceTab,
    exporter: Exporter,
    queue_index: usize,
    trace_index: usize,
    run_index: usize,
    input: Option<InputMode>,
    status: String,
    last_result_cache: Option<String>,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            focus: FocusPane::Queue,
            trace_tab: TraceTab::History,
            exporter: Exporter::Nlbn,
            queue_index: 0,
            trace_index: 0,
            run_index: 0,
            input: None,
            status: "Ready".to_string(),
            last_result_cache: None,
        }
    }
}

#[derive(Clone)]
struct Snapshot {
    monitoring: bool,
    keyword: String,
    matched: Vec<(String, String)>,
    history: Vec<(String, String)>,
    logs: Vec<String>,
    clipboard: String,
    nlbn_output_path: String,
    nlbn_show_terminal: bool,
    nlbn_parallel: usize,
    nlbn_path_mode: NlbnPathMode,
    nlbn_overwrite: bool,
    nlbn_last_result: Option<String>,
    nlbn_running: bool,
    npnp_output_path: String,
    npnp_mode: String,
    npnp_merge: bool,
    npnp_append: bool,
    npnp_library_name: String,
    npnp_parallel: usize,
    npnp_continue_on_error: bool,
    npnp_force: bool,
    npnp_last_result: Option<String>,
    npnp_running: bool,
}

pub fn run() -> Result<(), Box<dyn Error>> {
    let controller = AppController::new_native().map_err(io::Error::other)?;
    let mut terminal = setup_terminal()?;
    let result = run_loop(&controller, &mut terminal);
    restore_terminal(&mut terminal)?;
    result
}

fn run_loop(
    controller: &AppController,
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    let mut ui = UiState::default();
    let mut last_tick = Instant::now();

    loop {
        let snapshot = snapshot(controller);
        refresh_status_from_result(&mut ui, &snapshot);
        clamp_indices(&mut ui, &snapshot);

        terminal.draw(|frame| render(frame, &snapshot, &ui))?;

        let timeout = TICK_RATE.saturating_sub(last_tick.elapsed());
        if event::poll(timeout)?
            && let Event::Key(key) = event::read()?
        {
            if handle_key(controller, &snapshot, &mut ui, key)? {
                break;
            }
        }

        if last_tick.elapsed() >= TICK_RATE {
            last_tick = Instant::now();
        }
    }

    Ok(())
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<Stdout>>, Box<dyn Error>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    Ok(Terminal::new(CrosstermBackend::new(stdout))?)
}

fn restore_terminal(
    terminal: &mut Terminal<CrosstermBackend<Stdout>>,
) -> Result<(), Box<dyn Error>> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

fn snapshot(controller: &AppController) -> Snapshot {
    let state = controller
        .state()
        .lock()
        .expect("state lock should succeed");
    Snapshot {
        monitoring: state.monitoring,
        keyword: state.keyword.clone(),
        matched: state.matched.clone(),
        history: state.history.clone(),
        logs: state.match_debug_log.clone(),
        clipboard: state.last_content.clone(),
        nlbn_output_path: state.nlbn_output_path.clone(),
        nlbn_show_terminal: state.nlbn_show_terminal,
        nlbn_parallel: state.nlbn_parallel,
        nlbn_path_mode: state.nlbn_path_mode,
        nlbn_overwrite: state.nlbn_overwrite,
        nlbn_last_result: state.nlbn_last_result.clone(),
        nlbn_running: state.nlbn_running,
        npnp_output_path: state.npnp_output_path.clone(),
        npnp_mode: state.npnp_mode.clone(),
        npnp_merge: state.npnp_merge,
        npnp_append: state.npnp_append,
        npnp_library_name: state.npnp_library_name.clone(),
        npnp_parallel: state.npnp_parallel,
        npnp_continue_on_error: state.npnp_continue_on_error,
        npnp_force: state.npnp_force,
        npnp_last_result: state.npnp_last_result.clone(),
        npnp_running: state.npnp_running,
    }
}

fn handle_key(
    controller: &AppController,
    snapshot: &Snapshot,
    ui: &mut UiState,
    key: KeyEvent,
) -> Result<bool, Box<dyn Error>> {
    if ui.input.is_some() {
        return handle_input_mode(controller, ui, key);
    }

    match key {
        KeyEvent {
            code: KeyCode::Char('q'),
            modifiers: KeyModifiers::NONE,
            ..
        } => return Ok(true),
        KeyEvent {
            code: KeyCode::BackTab,
            ..
        } => ui.focus = ui.focus.prev(),
        KeyEvent {
            code: KeyCode::Tab, ..
        } => ui.focus = ui.focus.next(),
        KeyEvent {
            code: KeyCode::Char('m'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            toggle_monitoring(controller);
            ui.status = if snapshot.monitoring {
                "Monitoring paused".to_string()
            } else {
                "Monitoring resumed".to_string()
            };
        }
        KeyEvent {
            code: KeyCode::Char('c'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            clear_all(controller);
            ui.status = "Cleared history, matches, and exporter state".to_string();
        }
        KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            ui.status = if ui.focus == FocusPane::Trace {
                controller.save_history()
            } else {
                controller.save_matched()
            };
        }
        KeyEvent {
            code: KeyCode::Char('t'),
            modifiers: KeyModifiers::NONE,
            ..
        } => {
            ui.trace_tab = ui.trace_tab.next();
            ui.trace_index = 0;
        }
        KeyEvent {
            code: KeyCode::Char(' '),
            modifiers: KeyModifiers::NONE,
            ..
        } if ui.focus == FocusPane::Run => {
            ui.exporter = ui.exporter.toggle();
            ui.run_index = 0;
            ui.status = format!("Active exporter: {}", ui.exporter.label());
        }
        KeyEvent {
            code: KeyCode::Char('e'),
            modifiers: KeyModifiers::NONE,
            ..
        } => trigger_export(controller, ui),
        KeyEvent {
            code: KeyCode::Up | KeyCode::Char('k'),
            ..
        } => move_selection(ui, snapshot, -1),
        KeyEvent {
            code: KeyCode::Down | KeyCode::Char('j'),
            ..
        } => move_selection(ui, snapshot, 1),
        KeyEvent {
            code: KeyCode::Left | KeyCode::Char('h'),
            ..
        } if ui.focus == FocusPane::Run => adjust_run_field(controller, ui, -1),
        KeyEvent {
            code: KeyCode::Right | KeyCode::Char('l'),
            ..
        } if ui.focus == FocusPane::Run => adjust_run_field(controller, ui, 1),
        KeyEvent {
            code: KeyCode::Enter,
            ..
        } if ui.focus == FocusPane::Run => activate_run_field(controller, ui, snapshot),
        _ => {}
    }

    Ok(false)
}

fn handle_input_mode(
    controller: &AppController,
    ui: &mut UiState,
    key: KeyEvent,
) -> Result<bool, Box<dyn Error>> {
    let Some(mut input) = ui.input.take() else {
        return Ok(false);
    };

    match key.code {
        KeyCode::Esc => {
            ui.status = "Edit cancelled".to_string();
        }
        KeyCode::Enter => {
            let value = input.buffer.trim().to_string();
            apply_input_value(controller, ui.exporter, input.field, value.clone());
            ui.status = format!("Updated {}", run_field_label(ui.exporter, input.field));
        }
        KeyCode::Backspace => {
            input.buffer.pop();
            ui.input = Some(input);
        }
        KeyCode::Char(ch) => {
            input.buffer.push(ch);
            ui.input = Some(input);
        }
        _ => {
            ui.input = Some(input);
        }
    }
    Ok(false)
}

fn trigger_export(controller: &AppController, ui: &mut UiState) {
    ui.status = match ui.exporter {
        Exporter::Nlbn => controller.spawn_nlbn_export(Default::default()),
        Exporter::Npnp => controller.spawn_npnp_export(Default::default()),
    };
}

fn move_selection(ui: &mut UiState, snapshot: &Snapshot, delta: isize) {
    match ui.focus {
        FocusPane::Queue => move_index(&mut ui.queue_index, snapshot.matched.len(), delta),
        FocusPane::Trace => move_index(
            &mut ui.trace_index,
            trace_items(snapshot, ui.trace_tab).len(),
            delta,
        ),
        FocusPane::Run => move_index(&mut ui.run_index, run_fields(ui.exporter).len(), delta),
        FocusPane::Clipboard => {}
    }
}

fn move_index(index: &mut usize, len: usize, delta: isize) {
    if len == 0 {
        *index = 0;
        return;
    }
    let current = *index as isize;
    let next = (current + delta).clamp(0, len.saturating_sub(1) as isize);
    *index = next as usize;
}

fn activate_run_field(controller: &AppController, ui: &mut UiState, snapshot: &Snapshot) {
    let field = run_fields(ui.exporter)[ui.run_index];
    match field {
        RunField::Export => trigger_export(controller, ui),
        RunField::OutputPath => {
            let buffer = match ui.exporter {
                Exporter::Nlbn => snapshot.nlbn_output_path.clone(),
                Exporter::Npnp => snapshot.npnp_output_path.clone(),
            };
            ui.input = Some(InputMode { field, buffer });
            ui.status = "Editing output path".to_string();
        }
        RunField::LibraryName => {
            ui.input = Some(InputMode {
                field,
                buffer: snapshot.npnp_library_name.clone(),
            });
            ui.status = "Editing library name".to_string();
        }
        _ => adjust_run_field(controller, ui, 1),
    }
}

fn adjust_run_field(controller: &AppController, ui: &mut UiState, delta: isize) {
    let field = run_fields(ui.exporter)[ui.run_index];
    match (ui.exporter, field) {
        (_, RunField::Export) => {}
        (Exporter::Nlbn, RunField::Terminal) => {
            with_state(controller, |m| m.toggle_nlbn_show_terminal())
        }
        (Exporter::Nlbn, RunField::Parallel) => with_state(controller, |m| {
            let next = if delta < 0 {
                m.nlbn_parallel.saturating_sub(1).max(1)
            } else {
                m.nlbn_parallel.saturating_add(1)
            };
            m.set_nlbn_parallel(next);
        }),
        (Exporter::Nlbn, RunField::PathMode) => with_state(controller, |m| {
            let next = match (m.nlbn_path_mode, delta.signum()) {
                (NlbnPathMode::Auto, -1) => NlbnPathMode::LibraryRelative,
                (NlbnPathMode::Auto, _) => NlbnPathMode::ProjectRelative,
                (NlbnPathMode::ProjectRelative, -1) => NlbnPathMode::Auto,
                (NlbnPathMode::ProjectRelative, _) => NlbnPathMode::LibraryRelative,
                (NlbnPathMode::LibraryRelative, -1) => NlbnPathMode::ProjectRelative,
                (NlbnPathMode::LibraryRelative, _) => NlbnPathMode::Auto,
            };
            m.set_nlbn_path_mode(next);
        }),
        (Exporter::Nlbn, RunField::Overwrite) => {
            with_state(controller, |m| m.set_nlbn_overwrite(!m.nlbn_overwrite))
        }
        (Exporter::Npnp, RunField::Mode) => with_state(controller, |m| {
            let next = match (m.npnp_mode.as_str(), delta.signum()) {
                ("full", -1) => "pcblib",
                ("full", _) => "schlib",
                ("schlib", -1) => "full",
                ("schlib", _) => "pcblib",
                ("pcblib", -1) => "schlib",
                _ => "full",
            };
            m.set_npnp_mode(next.to_string());
        }),
        (Exporter::Npnp, RunField::Merge) => {
            with_state(controller, |m| m.set_npnp_merge(!m.npnp_merge))
        }
        (Exporter::Npnp, RunField::Append) => {
            with_state(controller, |m| m.set_npnp_append(!m.npnp_append))
        }
        (Exporter::Npnp, RunField::Parallel) => with_state(controller, |m| {
            let next = if delta < 0 {
                m.npnp_parallel.saturating_sub(1).max(1)
            } else {
                m.npnp_parallel.saturating_add(1)
            };
            m.set_npnp_parallel(next);
        }),
        (Exporter::Npnp, RunField::ContinueOnError) => with_state(controller, |m| {
            m.set_npnp_continue_on_error(!m.npnp_continue_on_error)
        }),
        (Exporter::Npnp, RunField::Force) => {
            with_state(controller, |m| m.set_npnp_force(!m.npnp_force))
        }
        _ => {}
    }
    controller.save_config();
}

fn apply_input_value(
    controller: &AppController,
    exporter: Exporter,
    field: RunField,
    value: String,
) {
    match (exporter, field) {
        (Exporter::Nlbn, RunField::OutputPath) => {
            with_state(controller, |m| m.set_nlbn_output_path(value))
        }
        (Exporter::Npnp, RunField::OutputPath) => {
            with_state(controller, |m| m.set_npnp_output_path(value))
        }
        (Exporter::Npnp, RunField::LibraryName) => {
            with_state(controller, |m| m.set_npnp_library_name(value))
        }
        _ => {}
    }
    controller.save_config();
}

fn with_state(controller: &AppController, f: impl FnOnce(&mut crate::monitor::MonitorState)) {
    if let Ok(mut state) = controller.state().lock() {
        f(&mut state);
    }
}

fn toggle_monitoring(controller: &AppController) {
    with_state(controller, |m| {
        m.monitoring = !m.monitoring;
        if m.monitoring {
            m.last_content.clear();
            m.initialized = true;
        }
    });
}

fn clear_all(controller: &AppController) {
    with_state(controller, |m| {
        m.history.clear();
        m.matched.clear();
        m.last_content.clear();
        m.initialized = false;
        m.match_debug_log.clear();
        m.nlbn_last_result = None;
        m.nlbn_running = false;
        m.npnp_last_result = None;
        m.npnp_running = false;
    });
}

fn refresh_status_from_result(ui: &mut UiState, snapshot: &Snapshot) {
    let latest = snapshot
        .nlbn_last_result
        .clone()
        .or_else(|| snapshot.npnp_last_result.clone());
    if latest.is_some() && latest != ui.last_result_cache {
        ui.status = latest.clone().unwrap_or_default();
        ui.last_result_cache = latest;
    }
}

fn clamp_indices(ui: &mut UiState, snapshot: &Snapshot) {
    ui.queue_index = clamp_index(ui.queue_index, snapshot.matched.len());
    ui.trace_index = clamp_index(ui.trace_index, trace_items(snapshot, ui.trace_tab).len());
    ui.run_index = clamp_index(ui.run_index, run_fields(ui.exporter).len());
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { min(index, len - 1) }
}

fn render(frame: &mut ratatui::Frame<'_>, snapshot: &Snapshot, ui: &UiState) {
    let areas = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(10),
            Constraint::Length(2),
        ])
        .split(frame.area());

    render_topbar(frame, areas[0], snapshot);

    let body = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(56), Constraint::Percentage(44)])
        .split(areas[1]);

    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(58), Constraint::Percentage(42)])
        .split(body[0]);
    let right = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(62), Constraint::Percentage(38)])
        .split(body[1]);

    render_queue(frame, left[0], snapshot, ui);
    render_trace(frame, left[1], snapshot, ui);
    render_run(frame, right[0], snapshot, ui);
    render_clipboard(frame, right[1], snapshot, ui);
    render_footer(frame, areas[2], ui);
}

fn render_topbar(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, snapshot: &Snapshot) {
    let status = if snapshot.monitoring {
        "MONITORING ON"
    } else {
        "MONITORING OFF"
    };
    let line = Line::from(vec![
        Span::styled(
            " SeEx ",
            Style::default()
                .fg(Color::Black)
                .bg(Color::Rgb(255, 143, 104))
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            status,
            Style::default()
                .fg(if snapshot.monitoring {
                    Color::Green
                } else {
                    Color::Red
                })
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(format!(
            "   matched {}   history {}",
            snapshot.matched.len(),
            snapshot.history.len()
        )),
        Span::raw("   "),
        Span::styled(
            trim_line(&snapshot.keyword, 48),
            Style::default().fg(Color::Gray),
        ),
    ]);

    frame.render_widget(
        Paragraph::new(line).block(Block::default().borders(Borders::ALL).title("Command Deck")),
        area,
    );
}

fn render_queue(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    snapshot: &Snapshot,
    ui: &UiState,
) {
    let items: Vec<ListItem<'_>> = if snapshot.matched.is_empty() {
        vec![ListItem::new(Line::from("No matched IDs yet"))]
    } else {
        snapshot
            .matched
            .iter()
            .map(|(time, value)| {
                ListItem::new(Line::from(vec![
                    Span::styled(format!("{time:>8} "), Style::default().fg(Color::DarkGray)),
                    Span::styled(value.clone(), Style::default().fg(Color::White)),
                ]))
            })
            .collect()
    };

    let mut state = ListState::default();
    if !snapshot.matched.is_empty() {
        state.select(Some(ui.queue_index));
    }

    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block("Active Queue", ui.focus == FocusPane::Queue))
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(58, 38, 31))
                    .fg(Color::Rgb(255, 229, 221))
                    .add_modifier(Modifier::BOLD),
            )
            .highlight_symbol(">> "),
        area,
        &mut state,
    );
}

fn render_trace(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    snapshot: &Snapshot,
    ui: &UiState,
) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(4)])
        .split(area);

    let titles = TraceTab::titles()
        .into_iter()
        .map(Line::from)
        .collect::<Vec<_>>();
    frame.render_widget(
        Tabs::new(titles)
            .block(pane_block("Trace", ui.focus == FocusPane::Trace))
            .select(ui.trace_tab.index())
            .highlight_style(
                Style::default()
                    .fg(Color::Rgb(255, 143, 104))
                    .add_modifier(Modifier::BOLD),
            ),
        chunks[0],
    );

    let items = trace_items(snapshot, ui.trace_tab);
    let list_items: Vec<ListItem<'_>> = if items.is_empty() {
        vec![ListItem::new(Line::from("No entries"))]
    } else {
        items
            .into_iter()
            .map(|line| ListItem::new(Line::from(line)))
            .collect()
    };
    let mut state = ListState::default();
    if !list_items.is_empty() {
        state.select(Some(ui.trace_index));
    }
    frame.render_stateful_widget(
        List::new(list_items)
            .highlight_style(Style::default().bg(Color::Rgb(36, 42, 54)))
            .highlight_symbol("> "),
        chunks[1],
        &mut state,
    );
}

fn render_run(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    snapshot: &Snapshot,
    ui: &UiState,
) {
    let title = format!("Run [{}]", ui.exporter.label());
    let fields = run_fields(ui.exporter);
    let items = fields
        .iter()
        .map(|field| {
            let label = run_field_label(ui.exporter, *field);
            let value = run_field_value(snapshot, ui, *field);
            ListItem::new(Line::from(vec![
                Span::styled(format!("{label:<14}"), Style::default().fg(Color::Gray)),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::White)),
            ]))
        })
        .collect::<Vec<_>>();

    let mut state = ListState::default();
    state.select(Some(ui.run_index));
    frame.render_stateful_widget(
        List::new(items)
            .block(pane_block(&title, ui.focus == FocusPane::Run))
            .highlight_style(
                Style::default()
                    .bg(Color::Rgb(19, 36, 56))
                    .fg(Color::Rgb(232, 243, 255)),
            )
            .highlight_symbol(">> "),
        area,
        &mut state,
    );
}

fn render_clipboard(
    frame: &mut ratatui::Frame<'_>,
    area: ratatui::layout::Rect,
    snapshot: &Snapshot,
    ui: &UiState,
) {
    let text = if snapshot.clipboard.trim().is_empty() {
        "Waiting for clipboard changes...".to_string()
    } else {
        trim_block(&snapshot.clipboard, 14, 72)
    };

    frame.render_widget(
        Paragraph::new(text)
            .block(pane_block("Clipboard", ui.focus == FocusPane::Clipboard))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn render_footer(frame: &mut ratatui::Frame<'_>, area: ratatui::layout::Rect, ui: &UiState) {
    let help = match &ui.input {
        Some(input) => format!(
            "Editing {} | type text | Enter apply | Esc cancel | {}",
            run_field_label(ui.exporter, input.field),
            trim_line(&ui.status, 80)
        ),
        None => format!(
            "Tab pane | Shift+Tab back | t trace tab | space switch tool | Enter edit/toggle | e export | m monitor | s save | c clear | q quit | {}",
            trim_line(&ui.status, 90)
        ),
    };
    frame.render_widget(Paragraph::new(help), area);
}

fn pane_block(title: &str, focused: bool) -> Block<'_> {
    let border_style = if focused {
        Style::default().fg(Color::Rgb(255, 143, 104))
    } else {
        Style::default().fg(Color::DarkGray)
    };
    Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(Span::styled(
            title.to_string(),
            if focused {
                Style::default()
                    .fg(Color::Rgb(255, 213, 199))
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().add_modifier(Modifier::BOLD)
            },
        ))
}

fn trace_items(snapshot: &Snapshot, tab: TraceTab) -> Vec<String> {
    match tab {
        TraceTab::History => snapshot
            .history
            .iter()
            .map(|(time, content)| format!("{time:>8}  {}", trim_line(content, 80)))
            .collect(),
        TraceTab::Log => snapshot.logs.clone(),
        TraceTab::Result => latest_result(snapshot)
            .map(|result| result.lines().map(|line| line.to_string()).collect())
            .unwrap_or_default(),
    }
}

fn latest_result(snapshot: &Snapshot) -> Option<String> {
    if let Some(result) = &snapshot.nlbn_last_result {
        Some(result.clone())
    } else {
        snapshot.npnp_last_result.clone()
    }
}

fn run_fields(exporter: Exporter) -> &'static [RunField] {
    match exporter {
        Exporter::Nlbn => &[
            RunField::Export,
            RunField::OutputPath,
            RunField::Terminal,
            RunField::Parallel,
            RunField::PathMode,
            RunField::Overwrite,
        ],
        Exporter::Npnp => &[
            RunField::Export,
            RunField::OutputPath,
            RunField::Mode,
            RunField::Merge,
            RunField::Append,
            RunField::LibraryName,
            RunField::Parallel,
            RunField::ContinueOnError,
            RunField::Force,
        ],
    }
}

fn run_field_label(exporter: Exporter, field: RunField) -> &'static str {
    match (exporter, field) {
        (_, RunField::Export) => "action",
        (_, RunField::OutputPath) => "output",
        (Exporter::Nlbn, RunField::Terminal) => "terminal",
        (_, RunField::Parallel) => "parallel",
        (Exporter::Nlbn, RunField::PathMode) => "3d mode",
        (Exporter::Nlbn, RunField::Overwrite) => "overwrite",
        (Exporter::Npnp, RunField::Mode) => "mode",
        (Exporter::Npnp, RunField::Merge) => "merge",
        (Exporter::Npnp, RunField::Append) => "append",
        (Exporter::Npnp, RunField::LibraryName) => "library",
        (Exporter::Npnp, RunField::ContinueOnError) => "cont err",
        (Exporter::Npnp, RunField::Force) => "force",
        _ => "",
    }
}

fn run_field_value(snapshot: &Snapshot, ui: &UiState, field: RunField) -> String {
    if let Some(input) = &ui.input
        && input.field == field
    {
        return format!("{}_", input.buffer);
    }

    match (ui.exporter, field) {
        (Exporter::Nlbn, RunField::Export) => {
            if snapshot.nlbn_running {
                "running...".to_string()
            } else {
                "export matched ids".to_string()
            }
        }
        (Exporter::Npnp, RunField::Export) => {
            if snapshot.npnp_running {
                "running...".to_string()
            } else {
                "export matched ids".to_string()
            }
        }
        (Exporter::Nlbn, RunField::OutputPath) => trim_line(&snapshot.nlbn_output_path, 30),
        (Exporter::Npnp, RunField::OutputPath) => trim_line(&snapshot.npnp_output_path, 30),
        (Exporter::Nlbn, RunField::Terminal) => yes_no(snapshot.nlbn_show_terminal).to_string(),
        (Exporter::Nlbn, RunField::Parallel) => snapshot.nlbn_parallel.to_string(),
        (Exporter::Npnp, RunField::Parallel) => snapshot.npnp_parallel.to_string(),
        (Exporter::Nlbn, RunField::PathMode) => match snapshot.nlbn_path_mode {
            NlbnPathMode::Auto => "auto".to_string(),
            NlbnPathMode::ProjectRelative => "project_relative".to_string(),
            NlbnPathMode::LibraryRelative => "library_relative".to_string(),
        },
        (Exporter::Nlbn, RunField::Overwrite) => yes_no(snapshot.nlbn_overwrite).to_string(),
        (Exporter::Npnp, RunField::Mode) => snapshot.npnp_mode.clone(),
        (Exporter::Npnp, RunField::Merge) => yes_no(snapshot.npnp_merge).to_string(),
        (Exporter::Npnp, RunField::Append) => yes_no(snapshot.npnp_append).to_string(),
        (Exporter::Npnp, RunField::LibraryName) => trim_line(&snapshot.npnp_library_name, 30),
        (Exporter::Npnp, RunField::ContinueOnError) => {
            yes_no(snapshot.npnp_continue_on_error).to_string()
        }
        (Exporter::Npnp, RunField::Force) => yes_no(snapshot.npnp_force).to_string(),
        _ => String::new(),
    }
}

fn trim_block(input: &str, max_lines: usize, max_cols: usize) -> String {
    input
        .lines()
        .take(max_lines)
        .map(|line| trim_line(line, max_cols))
        .collect::<Vec<_>>()
        .join("\n")
}

fn trim_line(input: &str, max: usize) -> String {
    let mut chars = input.chars();
    let clipped = chars.by_ref().take(max).collect::<String>();
    if chars.next().is_some() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

fn yes_no(value: bool) -> &'static str {
    if value { "on" } else { "off" }
}

#[cfg(test)]
mod tests {
    use super::{Exporter, FocusPane, TraceTab, UiState, clamp_index, run_fields};

    #[test]
    fn focus_cycles_forward_and_backward() {
        assert_eq!(FocusPane::Queue.next(), FocusPane::Trace);
        assert_eq!(FocusPane::Queue.prev(), FocusPane::Clipboard);
    }

    #[test]
    fn trace_tabs_cycle() {
        assert_eq!(TraceTab::History.next(), TraceTab::Log);
        assert_eq!(TraceTab::Result.next(), TraceTab::History);
    }

    #[test]
    fn run_field_sets_exist_for_each_exporter() {
        assert!(!run_fields(Exporter::Nlbn).is_empty());
        assert!(!run_fields(Exporter::Npnp).is_empty());
    }

    #[test]
    fn clamp_index_handles_empty_and_short_lists() {
        assert_eq!(clamp_index(3, 0), 0);
        assert_eq!(clamp_index(3, 2), 1);
    }

    #[test]
    fn ui_state_defaults_to_queue_focus() {
        let ui = UiState::default();
        assert_eq!(ui.focus, FocusPane::Queue);
    }
}
