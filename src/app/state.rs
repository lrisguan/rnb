/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
///
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
///
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

#[derive(Debug, Clone)]
pub struct TargetPickerItem {
    pub label: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Insert,
    Command,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub notebook: crate::notebook::Notebook,
    pub current_cell: usize,
    pub cursor_char: usize,
    pub vim_pending_g: bool,
    pub vim_pending_d: bool,
    pub in_cell_mode: bool,
    pub mode: Mode,
    pub scroll_offset: usize,
    pub viewport_width: u16,
    pub viewport_height: u16,
    pub command_buffer: String,
    pub show_help: bool,
    pub show_kernel_selector: bool,
    pub kernel_items: Vec<String>,
    pub kernel_selected: usize,
    pub show_completion: bool,
    pub completion_items: Vec<String>,
    pub completion_selected: usize,
    pub show_target_picker: bool,
    pub target_picker_title: String,
    pub target_picker_items: Vec<TargetPickerItem>,
    pub target_picker_selected: usize,
    pub status_message: Option<String>,
    pub should_quit: bool,
    pub file_path: Option<String>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            notebook: crate::notebook::Notebook::new(),
            current_cell: 0,
            cursor_char: 0,
            vim_pending_g: false,
            vim_pending_d: false,
            in_cell_mode: false,
            mode: Mode::Normal,
            scroll_offset: 0,
            viewport_width: 80,
            viewport_height: 24,
            command_buffer: String::new(),
            show_help: false,
            show_kernel_selector: false,
            kernel_items: Vec::new(),
            kernel_selected: 0,
            show_completion: false,
            completion_items: Vec::new(),
            completion_selected: 0,
            show_target_picker: false,
            target_picker_title: String::new(),
            target_picker_items: Vec::new(),
            target_picker_selected: 0,
            status_message: None,
            should_quit: false,
            file_path: None,
        }
    }

    pub fn with_notebook(notebook: crate::notebook::Notebook) -> Self {
        let current_cell = if notebook.is_empty() { 0 } else { 0 };
        Self {
            notebook,
            current_cell,
            cursor_char: 0,
            vim_pending_g: false,
            vim_pending_d: false,
            in_cell_mode: false,
            mode: Mode::Normal,
            scroll_offset: 0,
            viewport_width: 80,
            viewport_height: 24,
            command_buffer: String::new(),
            show_help: false,
            show_kernel_selector: false,
            kernel_items: Vec::new(),
            kernel_selected: 0,
            show_completion: false,
            completion_items: Vec::new(),
            completion_selected: 0,
            show_target_picker: false,
            target_picker_title: String::new(),
            target_picker_items: Vec::new(),
            target_picker_selected: 0,
            status_message: None,
            should_quit: false,
            file_path: None,
        }
    }

    pub fn current_cell(&self) -> Option<&crate::notebook::Cell> {
        self.notebook.get_cell(self.current_cell)
    }

    pub fn current_cell_mut(&mut self) -> Option<&mut crate::notebook::Cell> {
        self.notebook.get_cell_mut(self.current_cell)
    }

    pub fn total_cells(&self) -> usize {
        self.notebook.len()
    }

    pub fn can_move_down(&self) -> bool {
        self.current_cell + 1 < self.total_cells()
    }

    pub fn can_move_up(&self) -> bool {
        self.current_cell > 0
    }

    pub fn set_status(&mut self, message: String) {
        self.status_message = Some(message);
    }

    pub fn clear_status(&mut self) {
        self.status_message = None;
    }

    pub fn current_cell_len_chars(&self) -> Option<usize> {
        match self.current_cell() {
            Some(crate::notebook::Cell::Code(cell)) => Some(cell.source.len_chars()),
            Some(crate::notebook::Cell::Markdown(cell)) => Some(cell.source.chars().count()),
            _ => None,
        }
    }

    pub fn clamp_cursor(&mut self) {
        if let Some(max) = self.current_cell_len_chars() {
            self.cursor_char = self.cursor_char.min(max);
        } else {
            self.cursor_char = 0;
        }
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
