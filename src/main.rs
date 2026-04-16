/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

mod app;
mod editor;
mod event;
mod kernel;
mod lsp;
mod notebook;
mod ui;

use app::state::TargetPickerItem;
use app::{Action, AppState, Mode};
use base64::Engine;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use event::{event_to_action, EventDispatcher};
use kernel::{KernelClient, KernelOutput};
use lsp::CompletionProvider;
use pulldown_cmark::{Event as MdEvent, Parser as MdParser, Tag as MdTag, TagEnd as MdTagEnd};
use ratatui::{
    backend::{Backend, CrosstermBackend},
    Terminal,
};
use serde_json::Value;
use std::collections::HashSet;
use std::io;
use std::path::Path;
use std::process::Command as StdCommand;
use std::process::Stdio;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tokio::sync::mpsc;

enum Command {
    ExecuteCell(usize),
    RequestCompletion(usize),
}

#[derive(Clone, Copy)]
enum ExecutionScope {
    All,
    Above,
    Below,
}

enum WorkerResult {
    ExecuteDone {
        cell_idx: usize,
        result: Result<KernelOutput, String>,
    },
    CompletionDone {
        items: Vec<String>,
    },
}

#[derive(Clone, Debug)]
struct KernelOption {
    name: String,
    display_name: String,
    language: String,
    python_path: String,
}

#[derive(Clone, Debug)]
enum OpenResource {
    Url {
        label: String,
        detail: Option<String>,
        url: String,
    },
    Bytes {
        label: String,
        detail: Option<String>,
        ext: String,
        bytes: Vec<u8>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OpenResourceKind {
    Link,
    Image,
}

fn parse_colorless_line(s: &[u8]) -> String {
    String::from_utf8_lossy(s).trim().to_string()
}

fn parse_python_from_argv(spec: &Value) -> Option<String> {
    spec.get("argv")
        .and_then(|v| v.as_array())
        .and_then(|arr| arr.first())
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
}

fn parse_named_python_from_path(cmd: &str) -> Option<String> {
    let out = StdCommand::new("which").arg(cmd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let p = parse_colorless_line(&out.stdout);
    if p.is_empty() {
        None
    } else {
        Some(p)
    }
}

fn push_kernel_option(
    options: &mut Vec<KernelOption>,
    seen: &mut HashSet<String>,
    option: KernelOption,
) {
    let key = option.python_path.clone();
    if seen.insert(key) {
        options.push(option);
    }
}

fn discover_kernel_options() -> Vec<KernelOption> {
    let mut options = Vec::new();
    let mut seen = HashSet::new();

    if let Ok(out) = StdCommand::new("jupyter")
        .args(["kernelspec", "list", "--json"])
        .output()
    {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(specs) = json.get("kernelspecs").and_then(|v| v.as_object()) {
                    for (name, info) in specs {
                        if let Some(spec) = info.get("spec") {
                            let python_path = parse_python_from_argv(spec)
                                .unwrap_or_else(|| "python3".to_string());
                            let display_name = spec
                                .get("display_name")
                                .and_then(|v| v.as_str())
                                .unwrap_or(name)
                                .to_string();
                            let language = spec
                                .get("language")
                                .and_then(|v| v.as_str())
                                .unwrap_or("python")
                                .to_string();

                            push_kernel_option(
                                &mut options,
                                &mut seen,
                                KernelOption {
                                    name: name.to_string(),
                                    display_name,
                                    language,
                                    python_path,
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    if let Ok(out) = StdCommand::new("conda")
        .args(["env", "list", "--json"])
        .output()
    {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<Value>(&out.stdout) {
                if let Some(envs) = json.get("envs").and_then(|v| v.as_array()) {
                    for env in envs.iter().filter_map(|v| v.as_str()) {
                        let python_path = Path::new(env).join("bin/python");
                        if python_path.exists() {
                            let env_name = Path::new(env)
                                .file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("conda")
                                .to_string();
                            push_kernel_option(
                                &mut options,
                                &mut seen,
                                KernelOption {
                                    name: format!("conda-{env_name}"),
                                    display_name: format!("Conda: {env_name}"),
                                    language: "python".to_string(),
                                    python_path: python_path.to_string_lossy().to_string(),
                                },
                            );
                        }
                    }
                }
            }
        }
    }

    for cmd in ["python3", "python"] {
        if let Some(path) = parse_named_python_from_path(cmd) {
            let display_name = if cmd == "python3" {
                "System Python3"
            } else {
                "System Python"
            };
            push_kernel_option(
                &mut options,
                &mut seen,
                KernelOption {
                    name: cmd.to_string(),
                    display_name: display_name.to_string(),
                    language: "python".to_string(),
                    python_path: path,
                },
            );
        }
    }

    options.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    options
}

fn cursor_to_lsp_position(code: &str, cursor_char: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (idx, ch) in code.chars().enumerate() {
        if idx >= cursor_char {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line + 1, col)
}

struct TuiApp {
    state: AppState,
    kernel_client: KernelClient,
    completion_provider: CompletionProvider,
    event_dispatcher: EventDispatcher,
    cmd_tx: mpsc::UnboundedSender<Command>,
    cmd_rx: mpsc::UnboundedReceiver<Command>,
    worker_tx: mpsc::UnboundedSender<WorkerResult>,
    worker_rx: mpsc::UnboundedReceiver<WorkerResult>,
    kernel_options: Vec<KernelOption>,
    active_python_path: String,
    open_picker_resources: Vec<OpenResource>,
}

impl TuiApp {
    fn new() -> Self {
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel();
        let (worker_tx, worker_rx) = mpsc::unbounded_channel();
        Self {
            state: AppState::new(),
            kernel_client: KernelClient::new(),
            completion_provider: CompletionProvider::new(),
            event_dispatcher: EventDispatcher::new(),
            cmd_tx,
            cmd_rx,
            worker_tx,
            worker_rx,
            kernel_options: Vec::new(),
            active_python_path: "python3".to_string(),
            open_picker_resources: Vec::new(),
        }
    }

    async fn run<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> io::Result<()> {
        self.event_dispatcher.start();

        let mut last_tick = Instant::now();
        let tick_rate = Duration::from_millis(16);

        loop {
            if let Ok(size) = terminal.size() {
                self.state.viewport_width = size.width;
                self.state.viewport_height = size.height;
            }

            if let Err(err) = terminal.draw(|f| ui::render(f, &self.state)) {
                self.event_dispatcher.stop();
                return Err(err);
            }

            let timeout = tick_rate
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_secs(0));

            if let Ok(Some(event)) =
                tokio::time::timeout(timeout, self.event_dispatcher.rx.recv()).await
            {
                if self.state.show_help {
                    if matches!(event, Event::Key(key) if key.code == KeyCode::Esc) {
                        self.handle_action(Action::HideHelp);
                    }
                } else if self.state.show_target_picker {
                    self.handle_target_picker_event(event);
                } else if self.state.show_kernel_selector {
                    self.handle_kernel_selector_event(event);
                } else if let Some(action) =
                    event_to_action(event, self.state.mode, self.state.in_cell_mode)
                {
                    self.handle_action(action);
                }
            }

            while let Ok(cmd) = self.cmd_rx.try_recv() {
                self.handle_command(cmd);
            }

            while let Ok(result) = self.worker_rx.try_recv() {
                self.handle_worker_result(result);
            }

            if last_tick.elapsed() >= tick_rate {
                last_tick = Instant::now();
            }

            if self.should_quit() {
                break;
            }
        }

        self.event_dispatcher.stop();

        Ok(())
    }

    fn handle_action(&mut self, action: Action) {
        let command_preview = if matches!(action, Action::ExecuteCommand) {
            Some(self.state.command_buffer.clone())
        } else {
            None
        };

        match &action {
            Action::Quit => {
                self.state.should_quit = true;
                return;
            }
            Action::ExecuteCell => {
                if let Some(cell_idx) = Some(self.state.current_cell) {
                    let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                }
            }
            Action::ExecuteAllCells => {
                for cell_idx in self.code_cell_indices(ExecutionScope::All) {
                    let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                }
            }
            Action::ExecuteCellsBelow => {
                for cell_idx in self.code_cell_indices(ExecutionScope::Below) {
                    let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                }
            }
            Action::ExecuteCellsAbove => {
                for cell_idx in self.code_cell_indices(ExecutionScope::Above) {
                    let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                }
            }
            Action::ToggleCompletion => {
                if self.state.mode == Mode::Insert {
                    let cell_idx = self.state.current_cell;
                    let _ = self.cmd_tx.send(Command::RequestCompletion(cell_idx));
                }
            }
            Action::SaveFile => {
                if self.state.file_path.is_some() {
                    app::reduce(&mut self.state, action);
                    let _ = self.save_notebook(None);
                } else {
                    self.state.mode = Mode::Command;
                    self.state.command_buffer = "w ".to_string();
                    self.state
                        .set_status("Please input save path after :w".to_string());
                }
                return;
            }
            Action::InsertChar('.') => {
                if self.state.mode == Mode::Insert {
                    let cell_idx = self.state.current_cell;
                    app::reduce(&mut self.state, action);
                    let _ = self.cmd_tx.send(Command::RequestCompletion(cell_idx));
                    return;
                }
            }
            Action::HandleInCellKey(key)
                if self.state.in_cell_mode
                    && self.state.mode == Mode::Insert
                    && key.code == KeyCode::Char('.')
                    && !key.modifiers.contains(KeyModifiers::CONTROL)
                    && !key.modifiers.contains(KeyModifiers::ALT) =>
            {
                let cell_idx = self.state.current_cell;
                app::reduce(&mut self.state, action);
                let _ = self.cmd_tx.send(Command::RequestCompletion(cell_idx));
                return;
            }
            _ => {}
        }

        app::reduce(&mut self.state, action);

        if let Some(cmd) = command_preview {
            let parts: Vec<&str> = cmd.split_whitespace().collect();
            match parts.first().copied() {
                Some("open") => {
                    if let Some(path) = parts.get(1) {
                        self.open_notebook_file(path);
                    } else {
                        self.state
                            .set_status("Usage: :open <path-to-notebook.ipynb>".to_string());
                    }
                }
                Some("e") | Some("edit") => {
                    if let Some(path) = parts.get(1) {
                        self.open_notebook_file(path);
                    } else {
                        self.state
                            .set_status("Usage: :e <path-to-notebook.ipynb>".to_string());
                    }
                }
                Some("w") | Some("write") => {
                    let path = parts.get(1).copied();
                    if path.is_none() && self.state.file_path.is_none() {
                        self.state.mode = Mode::Command;
                        self.state.command_buffer = "w ".to_string();
                        self.state
                            .set_status("Please input save path after :w".to_string());
                    } else {
                        let _ = self.save_notebook(path);
                    }
                }
                Some("wq") | Some("x") => {
                    let path = parts.get(1).copied();
                    if path.is_none() && self.state.file_path.is_none() {
                        self.state.mode = Mode::Command;
                        self.state.command_buffer = "w ".to_string();
                        self.state
                            .set_status("Please input save path after :w".to_string());
                    } else if self.save_notebook(path) {
                        self.state.should_quit = true;
                    }
                }
                Some("ra") => {
                    for cell_idx in self.code_cell_indices(ExecutionScope::All) {
                        let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                    }
                    self.state
                        .set_status("Executing all code cells...".to_string());
                }
                Some("ro") => {
                    for cell_idx in self.code_cell_indices(ExecutionScope::Above) {
                        let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                    }
                    self.state
                        .set_status("Executing code cells from top to current...".to_string());
                }
                Some("rb") => {
                    for cell_idx in self.code_cell_indices(ExecutionScope::Below) {
                        let _ = self.cmd_tx.send(Command::ExecuteCell(cell_idx));
                    }
                    self.state
                        .set_status("Executing code cells from current to bottom...".to_string());
                }
                Some("h") | Some("help") => {
                    self.state.show_help = true;
                    self.state.set_status("Press Esc to close help".to_string());
                }
                Some("kernel") | Some("kernels") | Some("k") => {
                    self.open_kernel_selector();
                }
                Some("img") => {
                    self.open_current_cell_resources(OpenResourceKind::Image);
                }
                Some("ln") | Some("link") => {
                    self.open_current_cell_resources(OpenResourceKind::Link);
                }
                _ => {}
            }
        }
    }

    fn open_notebook_file(&mut self, path: &str) {
        match std::fs::read_to_string(path) {
            Ok(content) => match notebook::parse_ipynb(&content) {
                Ok(notebook) => {
                    self.state = AppState::with_notebook(notebook);
                    self.state.file_path = Some(path.to_string());
                    self.state.in_cell_mode = false;
                    self.state.mode = Mode::Normal;
                    self.state.set_status(format!("Loaded: {}", path));
                }
                Err(e) => {
                    self.state
                        .set_status(format!("Error parsing notebook: {}", e));
                }
            },
            Err(e) => {
                self.state.set_status(format!("Error reading file: {}", e));
            }
        }
    }

    fn open_kernel_selector(&mut self) {
        let options = discover_kernel_options();
        if options.is_empty() {
            self.state
                .set_status("No compatible Python kernels found".to_string());
            return;
        }

        self.kernel_options = options;
        self.state.kernel_items = self
            .kernel_options
            .iter()
            .map(|k| format!("{}  [{}]", k.display_name, k.python_path))
            .collect();
        self.state.kernel_selected = self
            .kernel_options
            .iter()
            .position(|k| k.python_path == self.active_python_path)
            .unwrap_or(0);
        self.state.show_kernel_selector = true;
        self.state
            .set_status("Select kernel: Enter apply, Esc cancel".to_string());
    }

    fn handle_kernel_selector_event(&mut self, event: Event) {
        let Event::Key(key) = event else {
            return;
        };

        match key.code {
            KeyCode::Esc => {
                self.state.show_kernel_selector = false;
                self.state.clear_status();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.kernel_items.is_empty() {
                    return;
                }
                self.state.kernel_selected = if self.state.kernel_selected == 0 {
                    self.state.kernel_items.len() - 1
                } else {
                    self.state.kernel_selected - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.kernel_items.is_empty() {
                    return;
                }
                self.state.kernel_selected =
                    (self.state.kernel_selected + 1) % self.state.kernel_items.len();
            }
            KeyCode::Enter => {
                if let Some(selected) = self.kernel_options.get(self.state.kernel_selected).cloned()
                {
                    self.kernel_client = KernelClient::with_python(&selected.python_path);
                    self.active_python_path = selected.python_path.clone();
                    self.state.notebook.metadata.kernelspec = Some(notebook::KernelSpec {
                        name: selected.name,
                        language: selected.language,
                        display_name: selected.display_name.clone(),
                    });
                    self.state.show_kernel_selector = false;
                    self.state
                        .set_status(format!("Kernel switched: {}", selected.display_name));
                }
            }
            _ => {}
        }
    }

    fn save_notebook(&mut self, path_override: Option<&str>) -> bool {
        let target_path = path_override
            .map(|p| p.to_string())
            .or_else(|| self.state.file_path.clone())
            .unwrap_or_else(|| "notebook.ipynb".to_string());

        match notebook::serialize_to_ipynb(&self.state.notebook) {
            Ok(content) => match std::fs::write(&target_path, content) {
                Ok(_) => {
                    if path_override.is_some() {
                        self.state.file_path = Some(target_path.clone());
                    }
                    self.state.set_status(format!("Saved: {}", target_path));
                    true
                }
                Err(e) => {
                    self.state.set_status(format!("Save failed: {}", e));
                    false
                }
            },
            Err(e) => {
                self.state.set_status(format!("Serialize failed: {}", e));
                false
            }
        }
    }

    fn open_current_cell_resources(&mut self, kind: OpenResourceKind) {
        let resources = self.collect_current_cell_resources(kind);
        if resources.is_empty() {
            let message = match kind {
                OpenResourceKind::Link => "Current cell has no markdown links",
                OpenResourceKind::Image => "Current cell has no images",
            };
            self.state.set_status(message.to_string());
            return;
        }

        self.open_resource_selection(kind, resources);
    }

    fn open_resource_selection(&mut self, kind: OpenResourceKind, resources: Vec<OpenResource>) {
        if resources.len() == 1 {
            let _ = self.open_resource(&resources[0]);
            return;
        }

        let title = match kind {
            OpenResourceKind::Link => "Open Link",
            OpenResourceKind::Image => "Open Image",
        };

        self.open_picker_resources = resources;
        self.state.show_target_picker = true;
        self.state.target_picker_title = title.to_string();
        self.state.target_picker_items = self
            .open_picker_resources
            .iter()
            .map(open_resource_to_picker_item)
            .collect();
        self.state.target_picker_selected = 0;
        self.state
            .set_status("Select item: Enter open, Esc cancel".to_string());
    }

    fn handle_target_picker_event(&mut self, event: Event) {
        let Event::Key(key) = event else {
            return;
        };

        match key.code {
            KeyCode::Esc => {
                self.state.show_target_picker = false;
                self.state.clear_status();
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.state.target_picker_items.is_empty() {
                    return;
                }
                self.state.target_picker_selected = if self.state.target_picker_selected == 0 {
                    self.state.target_picker_items.len() - 1
                } else {
                    self.state.target_picker_selected - 1
                };
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.state.target_picker_items.is_empty() {
                    return;
                }
                self.state.target_picker_selected =
                    (self.state.target_picker_selected + 1) % self.state.target_picker_items.len();
            }
            KeyCode::Enter => {
                if let Some(resource) = self
                    .open_picker_resources
                    .get(self.state.target_picker_selected)
                    .cloned()
                {
                    let _ = self.open_resource(&resource);
                    self.state.show_target_picker = false;
                }
            }
            _ => {}
        }
    }

    fn open_resource(&mut self, resource: &OpenResource) -> bool {
        match resource {
            OpenResource::Url { url, .. } => {
                let resolved = self.resolve_open_url(url);
                match StdCommand::new("xdg-open")
                    .arg(&resolved)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(_) => {
                        self.state.set_status(format!("Opened: {}", resolved));
                        true
                    }
                    Err(e) => {
                        self.state
                            .set_status(format!("Failed to open {}: {}", resolved, e));
                        false
                    }
                }
            }
            OpenResource::Bytes { ext, bytes, .. } => {
                let ts = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_millis())
                    .unwrap_or(0);
                let pid = std::process::id();
                let file_name = format!("nbtui-image-{}-{}.{}", pid, ts, ext);
                let path = std::env::temp_dir().join(file_name);

                if let Err(e) = std::fs::write(&path, bytes) {
                    self.state
                        .set_status(format!("Failed to write temp image: {}", e));
                    return false;
                }

                match StdCommand::new("xdg-open")
                    .arg(&path)
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn()
                {
                    Ok(_) => {
                        self.state.set_status(format!(
                            "Opened image in system viewer: {}",
                            path.display()
                        ));
                        true
                    }
                    Err(e) => {
                        self.state
                            .set_status(format!("Failed to launch viewer (xdg-open): {}", e));
                        false
                    }
                }
            }
        }
    }

    fn resolve_open_url(&self, url: &str) -> String {
        if url.contains("://") || url.starts_with("data:") || Path::new(url).is_absolute() {
            return url.to_string();
        }

        if let Some(base) = self
            .state
            .file_path
            .as_ref()
            .and_then(|path| Path::new(path).parent().map(|parent| parent.to_path_buf()))
        {
            return base.join(url).to_string_lossy().to_string();
        }

        url.to_string()
    }

    fn collect_current_cell_resources(&self, kind: OpenResourceKind) -> Vec<OpenResource> {
        match self.state.current_cell() {
            Some(notebook::Cell::Code(cell)) if kind == OpenResourceKind::Image => {
                self.collect_code_cell_images(cell)
            }
            Some(notebook::Cell::Markdown(cell)) if kind == OpenResourceKind::Link => {
                collect_markdown_resources(&cell.source, MarkdownResourceKind::Link)
            }
            Some(notebook::Cell::Markdown(cell)) if kind == OpenResourceKind::Image => {
                collect_markdown_resources(&cell.source, MarkdownResourceKind::Image)
            }
            _ => Vec::new(),
        }
    }

    fn collect_code_cell_images(&self, cell: &notebook::CodeCell) -> Vec<OpenResource> {
        let mut resources = Vec::new();

        for (output_idx, output) in cell.outputs.iter().enumerate() {
            let notebook::Output::DisplayData(display) = output else {
                continue;
            };

            if let Some(svg) = display.data.get("image/svg+xml").and_then(|v| v.as_str()) {
                if svg.trim_start().starts_with('<') {
                    resources.push(OpenResource::Bytes {
                        label: format!("svg output {}", output_idx + 1),
                        detail: Some(format!("{} chars", svg.len())),
                        ext: "svg".to_string(),
                        bytes: svg.as_bytes().to_vec(),
                    });
                } else if let Some(bytes) = decode_markdown_or_image_payload(svg) {
                    resources.push(OpenResource::Bytes {
                        label: format!("svg output {}", output_idx + 1),
                        detail: Some(format!("{} chars", bytes.len())),
                        ext: "svg".to_string(),
                        bytes,
                    });
                }
            }

            for (mime, ext) in [
                ("image/png", "png"),
                ("image/jpeg", "jpg"),
                ("image/jpg", "jpg"),
                ("image/webp", "webp"),
            ] {
                if let Some(encoded) = display.data.get(mime).and_then(|v| v.as_str()) {
                    if let Some(bytes) = decode_markdown_or_image_payload(encoded) {
                        resources.push(OpenResource::Bytes {
                            label: format!("{} output {}", mime, output_idx + 1),
                            detail: Some(format!("{} bytes", bytes.len())),
                            ext: ext.to_string(),
                            bytes,
                        });
                    }
                }
            }
        }

        resources
    }

    fn code_cell_indices(&self, scope: ExecutionScope) -> Vec<usize> {
        let len = self.state.notebook.cells.len();
        if len == 0 {
            return Vec::new();
        }

        let current = self.state.current_cell.min(len.saturating_sub(1));
        self.state
            .notebook
            .cells
            .iter()
            .enumerate()
            .filter(|(idx, cell)| {
                let in_scope = match scope {
                    ExecutionScope::All => true,
                    ExecutionScope::Above => *idx <= current,
                    ExecutionScope::Below => *idx >= current,
                };
                in_scope && matches!(cell, notebook::Cell::Code(_))
            })
            .map(|(idx, _)| idx)
            .collect()
    }

    fn handle_command(&mut self, cmd: Command) {
        match cmd {
            Command::ExecuteCell(cell_idx) => {
                if let Some(notebook::Cell::Code(cell)) = self.state.notebook.get_cell(cell_idx) {
                    // Run only the current cell to avoid replaying earlier cell output.
                    let code = cell.source.to_string();
                    let kernel_client = self.kernel_client.clone();
                    let worker_tx = self.worker_tx.clone();
                    tokio::spawn(async move {
                        let result = kernel_client
                            .execute(&code)
                            .await
                            .map_err(|e| e.to_string());
                        let _ = worker_tx.send(WorkerResult::ExecuteDone { cell_idx, result });
                    });
                }
            }
            Command::RequestCompletion(cell_idx) => {
                if let Some(notebook::Cell::Code(cell)) = self.state.notebook.get_cell(cell_idx) {
                    let code = cell.source.to_string();
                    let (line, character) = cursor_to_lsp_position(&code, self.state.cursor_char);

                    let mut completion_provider = self.completion_provider.clone();
                    let worker_tx = self.worker_tx.clone();

                    tokio::spawn(async move {
                        let completions = completion_provider
                            .get_completions(&code, line, character)
                            .await;
                        let items = completions.iter().map(|item| item.label.clone()).collect();
                        let _ = worker_tx.send(WorkerResult::CompletionDone { items });
                    });
                }
            }
        }
    }

    fn handle_worker_result(&mut self, result: WorkerResult) {
        match result {
            WorkerResult::ExecuteDone { cell_idx, result } => {
                if let Some(notebook::Cell::Code(cell)) = self.state.notebook.get_cell_mut(cell_idx)
                {
                    cell.outputs.clear();
                    match result {
                        Ok(KernelOutput::Success {
                            output,
                            execution_count,
                        }) => {
                            if let Some(count) = execution_count {
                                cell.execution_count = Some(count);
                            }
                            if !output.trim().is_empty() {
                                cell.outputs.push(notebook::Output::Stream(
                                    notebook::StreamOutput {
                                        name: "stdout".to_string(),
                                        text: output,
                                    },
                                ));
                            }
                            self.state.set_status("Execution finished".to_string());
                        }
                        Ok(KernelOutput::Error {
                            error,
                            execution_count,
                            traceback,
                        }) => {
                            if let Some(count) = execution_count {
                                cell.execution_count = Some(count);
                            }
                            cell.outputs
                                .push(notebook::Output::Error(notebook::ErrorOutput {
                                    ename: "ExecutionError".to_string(),
                                    evalue: error,
                                    traceback,
                                }));
                            self.state.set_status("Execution failed".to_string());
                        }
                        Err(error) => {
                            cell.outputs
                                .push(notebook::Output::Error(notebook::ErrorOutput {
                                    ename: "ExecutionError".to_string(),
                                    evalue: error,
                                    traceback: Vec::new(),
                                }));
                            self.state.set_status("Execution failed".to_string());
                        }
                    }
                }
            }
            WorkerResult::CompletionDone { items } => {
                self.state.completion_items = items;
                self.state.completion_selected = 0;
                self.state.show_completion = !self.state.completion_items.is_empty();
            }
        }
    }

    fn should_quit(&self) -> bool {
        self.state.should_quit
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum MarkdownResourceKind {
    Link,
    Image,
}

#[derive(Clone, Debug)]
struct MarkdownResourceDraft {
    label: String,
    destination: String,
    title: Option<String>,
}

fn open_resource_to_picker_item(resource: &OpenResource) -> TargetPickerItem {
    match resource {
        OpenResource::Url { label, detail, .. } | OpenResource::Bytes { label, detail, .. } => {
            TargetPickerItem {
                label: label.clone(),
                detail: detail.clone(),
            }
        }
    }
}

fn collect_markdown_resources(source: &str, kind: MarkdownResourceKind) -> Vec<OpenResource> {
    let mut resources = Vec::new();
    let parser = MdParser::new(source);
    let mut stack: Vec<MarkdownResourceDraft> = Vec::new();

    for event in parser {
        match event {
            MdEvent::Start(tag) => match tag {
                MdTag::Link {
                    dest_url, title, ..
                } if kind == MarkdownResourceKind::Link => {
                    stack.push(MarkdownResourceDraft {
                        label: String::new(),
                        destination: dest_url.to_string(),
                        title: if title.is_empty() {
                            None
                        } else {
                            Some(title.to_string())
                        },
                    });
                }
                MdTag::Image {
                    dest_url, title, ..
                } if kind == MarkdownResourceKind::Image => {
                    stack.push(MarkdownResourceDraft {
                        label: String::new(),
                        destination: dest_url.to_string(),
                        title: if title.is_empty() {
                            None
                        } else {
                            Some(title.to_string())
                        },
                    });
                }
                _ => {}
            },
            MdEvent::End(tag) => match tag {
                MdTagEnd::Link if kind == MarkdownResourceKind::Link => {
                    if let Some(draft) = stack.pop() {
                        let label = if draft.label.trim().is_empty() {
                            draft.destination.clone()
                        } else {
                            draft.label.trim().to_string()
                        };
                        resources.push(OpenResource::Url {
                            label,
                            detail: draft.title.filter(|value| !value.trim().is_empty()),
                            url: draft.destination,
                        });
                    }
                }
                MdTagEnd::Image if kind == MarkdownResourceKind::Image => {
                    if let Some(draft) = stack.pop() {
                        let label = if draft.label.trim().is_empty() {
                            draft.destination.clone()
                        } else {
                            draft.label.trim().to_string()
                        };
                        resources.push(OpenResource::Url {
                            label,
                            detail: draft.title.filter(|value| !value.trim().is_empty()),
                            url: draft.destination,
                        });
                    }
                }
                _ => {}
            },
            MdEvent::Text(text) | MdEvent::Code(text) | MdEvent::Html(text) => {
                if let Some(active) = stack.last_mut() {
                    active.label.push_str(&text);
                }
            }
            MdEvent::SoftBreak | MdEvent::HardBreak => {
                if let Some(active) = stack.last_mut() {
                    active.label.push(' ');
                }
            }
            _ => {}
        }
    }

    resources
}

fn decode_markdown_or_image_payload(input: &str) -> Option<Vec<u8>> {
    let payload = if let Some(idx) = input.find("base64,") {
        &input[idx + 7..]
    } else {
        input
    };
    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()
}

#[tokio::main]
async fn main() -> io::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|arg| arg == "-h" || arg == "--help") {
        println!(
            "rnb {}\n\nUsage:\n  rnb [notebook.ipynb]\n  rnb -h | --help\n  rnb --version\n\nIn-app commands:\n  :open <file>   Open notebook file\n  :w [file]      Save notebook\n  :img           Open images from current cell\n  :ln            Open links from current markdown cell\n  :kernel        Select kernel\n  :h             Show help\n",
            env!("CARGO_PKG_VERSION")
        );
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = TuiApp::new();

    if args.len() > 1 {
        let path = &args[1];
        match std::fs::read_to_string(path) {
            Ok(content) => match notebook::parse_ipynb(&content) {
                Ok(notebook) => {
                    app.state = AppState::with_notebook(notebook);
                    app.state.file_path = Some(path.to_string());
                    app.state.set_status(format!("Loaded: {}", path));
                }
                Err(e) => {
                    app.state
                        .set_status(format!("Error parsing notebook: {}", e));
                }
            },
            Err(e) => {
                app.state.set_status(format!("Error reading file: {}", e));
            }
        }
    } else {
        app.state
            .set_status("type :h for help, and :w <path> to save".to_string());
    }

    let res = app.run(&mut terminal).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        println!("{}", err);
    }

    Ok(())
}
