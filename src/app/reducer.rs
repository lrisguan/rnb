/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::app::{Action, AppState};
use crate::editor::Direction;
use crate::notebook::{Cell, CodeCell, MarkdownCell};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

fn offset_to_line_col(text: &str, offset_chars: usize) -> (usize, usize) {
    let mut line = 0usize;
    let mut col = 0usize;
    for (idx, ch) in text.chars().enumerate() {
        if idx >= offset_chars {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn line_col_to_offset(text: &str, target_line: usize, target_col: usize) -> usize {
    let mut line = 0usize;
    let mut col = 0usize;
    let mut offset = 0usize;

    for ch in text.chars() {
        if line == target_line && col >= target_col {
            break;
        }
        offset += 1;
        if ch == '\n' {
            line += 1;
            col = 0;
            if line > target_line {
                break;
            }
        } else {
            col += 1;
        }
    }

    offset
}

fn total_lines(text: &str) -> usize {
    text.chars().filter(|c| *c == '\n').count() + 1
}

fn wrapped_line_height(text: &str, width: usize) -> usize {
    let width = width.max(1);
    if text.is_empty() {
        return 1;
    }

    let mut total = 0usize;
    for line in text.lines() {
        let chars = line.chars().count().max(1);
        total += (chars + width - 1) / width;
    }
    total.max(1)
}

fn viewport_visible_rows(state: &AppState) -> usize {
    state.viewport_height.saturating_sub(8).max(4) as usize
}

fn scroll_step_rows(state: &AppState) -> usize {
    // Keep wheel responsive on very long cells.
    (viewport_visible_rows(state) / 4).max(3)
}

fn output_text(output: &crate::notebook::Output) -> String {
    const IMAGE_ESTIMATE_ROWS: usize = 19;

    fn strip_ansi_sequences(text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut chars = text.chars().peekable();

        while let Some(ch) = chars.next() {
            if ch != '\u{1b}' {
                out.push(ch);
                continue;
            }

            match chars.peek().copied() {
                Some('[') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if ('@'..='~').contains(&next) {
                            break;
                        }
                    }
                }
                Some(']') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{7}' {
                            break;
                        }
                        if next == '\u{1b}' && matches!(chars.peek(), Some('\\')) {
                            chars.next();
                            break;
                        }
                    }
                }
                Some('P') | Some('X') | Some('^') | Some('_') => {
                    chars.next();
                    while let Some(next) = chars.next() {
                        if next == '\u{1b}' && matches!(chars.peek(), Some('\\')) {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => {}
            }
        }

        out
    }

    fn normalize_output_text(text: &str) -> String {
        let normalized = text.replace("\r\n", "\n");
        let mut lines = Vec::new();
        let mut current_line = String::new();

        for ch in normalized.chars() {
            match ch {
                '\n' => {
                    lines.push(std::mem::take(&mut current_line));
                }
                '\r' => {
                    current_line.clear();
                }
                '\u{8}' => {
                    current_line.pop();
                }
                _ => current_line.push(ch),
            }
        }

        lines.push(current_line);
        strip_ansi_sequences(&lines.join("\n"))
    }

    let rendered = match output {
        crate::notebook::Output::Stream(stream) => stream.text.clone(),
        crate::notebook::Output::Error(err) => {
            let mut text = format!("{}: {}", err.ename, err.evalue);
            if !err.traceback.is_empty() {
                text.push('\n');
                text.push_str(&err.traceback.join("\n"));
            }
            text
        }
        crate::notebook::Output::ExecuteResult(result) => result
            .data
            .get("text/plain")
            .and_then(|v| v.as_str())
            .unwrap_or("[Execute Result]")
            .to_string(),
        crate::notebook::Output::DisplayData(data) => data
            .data
            .get("application/pdf")
            .and_then(|v| v.as_str())
            .map(|v| {
                format!(
                    "[application/pdf not supported yet, payload {} chars]",
                    v.len()
                )
            })
            .or_else(|| {
                data.data
                    .get("image/svg+xml")
                    .and_then(|v| v.as_str())
                    .map(|_| {
                        let mut lines = Vec::with_capacity(IMAGE_ESTIMATE_ROWS);
                        lines.push("[svg image]".to_string());
                        for _ in 0..IMAGE_ESTIMATE_ROWS.saturating_sub(1) {
                            lines.push(
                                "................................................".to_string(),
                            );
                        }
                        lines.join("\n")
                    })
            })
            .or_else(|| {
                ["image/png", "image/jpeg", "image/jpg", "image/webp"]
                    .iter()
                    .find_map(|key| {
                        data.data.get(*key).and_then(|v| v.as_str()).map(|_| {
                            let mut lines = Vec::with_capacity(IMAGE_ESTIMATE_ROWS);
                            lines.push(format!("[{}]", key));
                            for _ in 0..IMAGE_ESTIMATE_ROWS.saturating_sub(1) {
                                lines.push(
                                    "................................................".to_string(),
                                );
                            }
                            lines.join("\n")
                        })
                    })
            })
            .or_else(|| {
                data.data
                    .get("text/plain")
                    .and_then(|v| v.as_str())
                    .map(|v| v.to_string())
            })
            .unwrap_or_else(|| "[Display Data]".to_string()),
    };

    normalize_output_text(&rendered)
}

fn cell_visual_rows(state: &AppState, idx: usize) -> usize {
    let width = state.viewport_width.saturating_sub(4).max(20) as usize;
    let is_current = idx == state.current_cell;

    let base = match state.notebook.get_cell(idx) {
        Some(Cell::Code(code)) => {
            let source_text = code.source.to_string();
            let prompt = format!(
                "In [{}]: ",
                code.execution_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| " ".to_string())
            );
            let prompt_pad = " ".repeat(prompt.chars().count());
            let source_lines: Vec<&str> = if source_text.is_empty() {
                vec![""]
            } else {
                source_text.split('\n').collect()
            };

            let mut rendered = String::new();
            for (line_idx, line) in source_lines.iter().enumerate() {
                if line_idx > 0 {
                    rendered.push('\n');
                }
                if line_idx == 0 {
                    rendered.push_str(&prompt);
                } else {
                    rendered.push_str(&prompt_pad);
                }
                rendered.push_str(line);
            }

            if !code.outputs.is_empty() {
                rendered.push('\n');
            }
            for output in &code.outputs {
                rendered.push_str(&output_text(output));
                rendered.push('\n');
            }

            wrapped_line_height(&rendered, width).max(1) + 2
        }
        Some(Cell::Markdown(markdown)) => {
            let content_rows = wrapped_line_height(&markdown.source, width).max(1);
            if is_current {
                content_rows + 2
            } else {
                content_rows
            }
        }
        None => 1,
    };

    base + 1
}

fn cell_start_row(state: &AppState, target_idx: usize) -> usize {
    let mut row = 0usize;
    for idx in 0..target_idx.min(state.notebook.len()) {
        row = row.saturating_add(cell_visual_rows(state, idx));
    }
    row
}

fn notebook_total_rows(state: &AppState) -> usize {
    let mut total = 0usize;
    for idx in 0..state.notebook.len() {
        total = total.saturating_add(cell_visual_rows(state, idx));
    }
    total
}

fn max_notebook_scroll_offset(state: &AppState) -> usize {
    notebook_total_rows(state).saturating_sub(viewport_visible_rows(state))
}

fn clamp_scroll_offset(state: &mut AppState) {
    state.scroll_offset = state.scroll_offset.min(max_notebook_scroll_offset(state));
}

fn cell_index_for_scroll_offset(state: &AppState) -> Option<usize> {
    if state.notebook.is_empty() {
        return None;
    }

    let mut row = 0usize;
    for idx in 0..state.notebook.len() {
        let cell_rows = cell_visual_rows(state, idx);
        if state.scroll_offset < row.saturating_add(cell_rows) {
            return Some(idx);
        }
        row = row.saturating_add(cell_rows);
    }

    Some(state.notebook.len().saturating_sub(1))
}

fn current_cell_text(state: &AppState) -> Option<String> {
    state.current_cell().map(|cell| match cell {
        Cell::Code(code) => code.source.to_string(),
        Cell::Markdown(markdown) => markdown.source.clone(),
    })
}

fn set_current_cell_text(state: &mut AppState, text: &str) {
    if let Some(cell) = state.current_cell_mut() {
        match cell {
            Cell::Code(code) => {
                code.source = ropey::Rope::from_str(text);
            }
            Cell::Markdown(markdown) => {
                markdown.source = text.to_string();
            }
        }
    }
}

fn current_line_bounds(text: &str, cursor: usize) -> (usize, usize, usize) {
    let clamped = cursor.min(text.chars().count());
    let (line, _) = offset_to_line_col(text, clamped);
    let start = line_col_to_offset(text, line, 0);
    let mut end = start;
    let chars: Vec<char> = text.chars().collect();
    while end < chars.len() && chars[end] != '\n' {
        end += 1;
    }
    (line, start, end)
}

fn first_non_ws_offset(text: &str, start: usize, end: usize) -> usize {
    for idx in start..end {
        if let Some(ch) = text.chars().nth(idx) {
            if !ch.is_whitespace() {
                return idx;
            }
        }
    }
    start
}

fn apply_in_cell_key(state: &mut AppState, key_event: KeyEvent) {
    if !(state.in_cell_mode && state.mode != crate::app::Mode::Command) {
        return;
    }

    let Some(mut text) = current_cell_text(state) else {
        return;
    };

    state.cursor_char = state.cursor_char.min(text.chars().count());

    match state.mode {
        crate::app::Mode::Insert => match key_event.code {
            KeyCode::Esc => {
                state.mode = crate::app::Mode::Normal;
                state.vim_pending_g = false;
            }
            KeyCode::Left => {
                state.cursor_char = state.cursor_char.saturating_sub(1);
            }
            KeyCode::Right => {
                state.cursor_char = (state.cursor_char + 1).min(text.chars().count());
            }
            KeyCode::Up => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                if line > 0 {
                    state.cursor_char = line_col_to_offset(&text, line - 1, col);
                }
            }
            KeyCode::Down => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                let line_count = total_lines(&text);
                if line + 1 < line_count {
                    state.cursor_char = line_col_to_offset(&text, line + 1, col);
                }
            }
            KeyCode::Backspace => {
                if state.cursor_char > 0 {
                    let remove_at = state.cursor_char - 1;
                    let mut chars: Vec<char> = text.chars().collect();
                    chars.remove(remove_at);
                    text = chars.into_iter().collect();
                    state.cursor_char = remove_at;
                    set_current_cell_text(state, &text);
                }
            }
            KeyCode::Delete => {
                if state.cursor_char < text.chars().count() {
                    let mut chars: Vec<char> = text.chars().collect();
                    chars.remove(state.cursor_char);
                    text = chars.into_iter().collect();
                    set_current_cell_text(state, &text);
                }
            }
            KeyCode::Enter => {
                let mut chars: Vec<char> = text.chars().collect();
                chars.insert(state.cursor_char, '\n');
                text = chars.into_iter().collect();
                state.cursor_char += 1;
                set_current_cell_text(state, &text);
            }
            KeyCode::Tab => {
                let mut chars: Vec<char> = text.chars().collect();
                chars.insert(state.cursor_char, '\t');
                text = chars.into_iter().collect();
                state.cursor_char += 1;
                set_current_cell_text(state, &text);
            }
            KeyCode::Char(c)
                if !key_event.modifiers.contains(KeyModifiers::CONTROL)
                    && !key_event.modifiers.contains(KeyModifiers::ALT) =>
            {
                let mut chars: Vec<char> = text.chars().collect();
                chars.insert(state.cursor_char, c);
                text = chars.into_iter().collect();
                state.cursor_char += 1;
                set_current_cell_text(state, &text);
            }
            _ => {}
        },
        crate::app::Mode::Normal => match key_event.code {
            KeyCode::Enter => {
                let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                let mut chars: Vec<char> = text.chars().collect();
                let insert_pos = if end < chars.len() { end + 1 } else { end };
                chars.insert(insert_pos, '\n');
                text = chars.into_iter().collect();
                set_current_cell_text(state, &text);
                state.cursor_char = insert_pos + 1;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('g') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let last_line = total_lines(&text).saturating_sub(1);
                    state.cursor_char = line_col_to_offset(&text, last_line, 0);
                    state.vim_pending_g = false;
                } else if state.vim_pending_g {
                    state.cursor_char = 0;
                    state.vim_pending_g = false;
                } else {
                    state.vim_pending_g = true;
                }
            }
            KeyCode::Char('G') => {
                let last_line = total_lines(&text).saturating_sub(1);
                state.cursor_char = line_col_to_offset(&text, last_line, 0);
                state.vim_pending_g = false;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                state.cursor_char = state.cursor_char.saturating_sub(1);
                state.vim_pending_g = false;
            }
            KeyCode::Char('l') | KeyCode::Right => {
                state.cursor_char = (state.cursor_char + 1).min(text.chars().count());
                state.vim_pending_g = false;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                if line > 0 {
                    state.cursor_char = line_col_to_offset(&text, line - 1, col);
                }
                state.vim_pending_g = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                let line_count = total_lines(&text);
                if line + 1 < line_count {
                    state.cursor_char = line_col_to_offset(&text, line + 1, col);
                }
                state.vim_pending_g = false;
            }
            KeyCode::Char('i') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let (_, start, end) = current_line_bounds(&text, state.cursor_char);
                    state.cursor_char = first_non_ws_offset(&text, start, end);
                }
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('I') => {
                let (_, start, end) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = first_non_ws_offset(&text, start, end);
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('a') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                    state.cursor_char = end;
                } else {
                    state.cursor_char = (state.cursor_char + 1).min(text.chars().count());
                }
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('A') => {
                let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = end;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('o') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let (_, start, _) = current_line_bounds(&text, state.cursor_char);
                    let mut chars: Vec<char> = text.chars().collect();
                    chars.insert(start, '\n');
                    text = chars.into_iter().collect();
                    set_current_cell_text(state, &text);
                    state.cursor_char = start;
                } else {
                    let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                    let mut chars: Vec<char> = text.chars().collect();
                    let insert_pos = if end < chars.len() { end + 1 } else { end };
                    chars.insert(insert_pos, '\n');
                    text = chars.into_iter().collect();
                    set_current_cell_text(state, &text);
                    state.cursor_char = insert_pos + 1;
                }
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Char('O') => {
                let (_, start, _) = current_line_bounds(&text, state.cursor_char);
                let mut chars: Vec<char> = text.chars().collect();
                chars.insert(start, '\n');
                text = chars.into_iter().collect();
                set_current_cell_text(state, &text);
                state.cursor_char = start;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
            }
            KeyCode::Esc => {
                state.vim_pending_g = false;
            }
            _ => {
                state.vim_pending_g = false;
            }
        },
        crate::app::Mode::Command => {}
    }

    state.clamp_cursor();
}

pub fn reduce(state: &mut AppState, action: Action) {
    match action {
        Action::Quit => {
            state.should_quit = true;
        }

        Action::InsertChar(c) => {
            if state.mode == crate::app::Mode::Insert {
                let cursor = state.cursor_char;
                if let Some(cell) = state.current_cell_mut() {
                    match cell {
                        Cell::Code(code) => {
                            let pos = cursor.min(code.source.len_chars());
                            code.source.insert_char(pos, c);
                            state.cursor_char = pos + 1;
                        }
                        Cell::Markdown(markdown) => {
                            let mut chars: Vec<char> = markdown.source.chars().collect();
                            let pos = cursor.min(chars.len());
                            chars.insert(pos, c);
                            markdown.source = chars.into_iter().collect();
                            state.cursor_char = pos + 1;
                        }
                    }
                }
            }
        }

        Action::DeleteChar => {
            if state.mode == crate::app::Mode::Insert {
                let cursor = state.cursor_char;
                if let Some(cell) = state.current_cell_mut() {
                    match cell {
                        Cell::Code(code) => {
                            let pos = cursor.min(code.source.len_chars());
                            if pos < code.source.len_chars() {
                                code.source.remove(pos..=pos);
                            }
                        }
                        Cell::Markdown(markdown) => {
                            let mut chars: Vec<char> = markdown.source.chars().collect();
                            let pos = cursor.min(chars.len());
                            if pos < chars.len() {
                                chars.remove(pos);
                                markdown.source = chars.into_iter().collect();
                            }
                        }
                    }
                }
            }
        }

        Action::Backspace => {
            if state.mode == crate::app::Mode::Insert {
                let cursor = state.cursor_char;
                if let Some(cell) = state.current_cell_mut() {
                    match cell {
                        Cell::Code(code) => {
                            let pos = cursor.min(code.source.len_chars());
                            if pos > 0 {
                                code.source.remove(pos - 1..pos);
                                state.cursor_char = pos - 1;
                            }
                        }
                        Cell::Markdown(markdown) => {
                            let mut chars: Vec<char> = markdown.source.chars().collect();
                            let pos = cursor.min(chars.len());
                            if pos > 0 {
                                chars.remove(pos - 1);
                                markdown.source = chars.into_iter().collect();
                                state.cursor_char = pos - 1;
                            }
                        }
                    }
                }
            }
        }

        Action::MoveCursor(dir) => {
            if state.in_cell_mode && state.mode != crate::app::Mode::Command {
                match dir {
                    Direction::Left => {
                        state.cursor_char = state.cursor_char.saturating_sub(1);
                    }
                    Direction::Right => {
                        if let Some(max) = state.current_cell_len_chars() {
                            state.cursor_char = (state.cursor_char + 1).min(max);
                        }
                    }
                    Direction::Up | Direction::Down => {
                        if let Some(text) = state.current_cell().map(|cell| match cell {
                            Cell::Code(code) => code.source.to_string(),
                            Cell::Markdown(markdown) => markdown.source.clone(),
                        }) {
                            let current = state.cursor_char.min(text.chars().count());
                            let (line, col) = offset_to_line_col(&text, current);
                            let line_count = total_lines(&text);

                            let target_line = match dir {
                                Direction::Up if line > 0 => line - 1,
                                Direction::Down if line + 1 < line_count => line + 1,
                                _ => line,
                            };
                            state.cursor_char = line_col_to_offset(&text, target_line, col);
                        }
                    }
                }
                state.clamp_cursor();
            }
        }

        Action::EnterCell => {
            state.in_cell_mode = true;
            state.mode = crate::app::Mode::Normal;
            state.vim_pending_g = false;
            state.show_completion = false;
            state.clamp_cursor();
        }

        Action::LeaveCell => {
            state.in_cell_mode = false;
            state.mode = crate::app::Mode::Normal;
            state.vim_pending_g = false;
            state.show_completion = false;
        }

        Action::HandleInCellKey(key_event) => {
            apply_in_cell_key(state, key_event);
        }

        Action::MoveCellUp => {
            if !state.in_cell_mode && state.can_move_up() {
                state.current_cell -= 1;
                state.clamp_cursor();
                state.scroll_offset = cell_start_row(state, state.current_cell);
                clamp_scroll_offset(state);
            }
        }

        Action::MoveCellDown => {
            if !state.in_cell_mode && state.can_move_down() {
                state.current_cell += 1;
                state.clamp_cursor();
                state.scroll_offset = cell_start_row(state, state.current_cell);
                clamp_scroll_offset(state);
            }
        }

        Action::InsertCellAbove => {
            let new_cell = Cell::Code(CodeCell::new());
            state.notebook.insert_cell(state.current_cell, new_cell);
            state.current_cell = state
                .current_cell
                .min(state.notebook.len().saturating_sub(1));
            state.cursor_char = 0;
            state.scroll_offset = cell_start_row(state, state.current_cell);
            clamp_scroll_offset(state);
            state.clamp_cursor();
            state.set_status("Inserted cell above".to_string());
        }

        Action::InsertCellBelow => {
            let new_cell = Cell::Code(CodeCell::new());
            let insert_pos = if state.current_cell + 1 <= state.notebook.len() {
                state.current_cell + 1
            } else {
                state.notebook.len()
            };
            state.notebook.insert_cell(insert_pos, new_cell);
            state.current_cell = insert_pos;
            state.cursor_char = 0;
            state.scroll_offset = cell_start_row(state, state.current_cell);
            clamp_scroll_offset(state);
            state.clamp_cursor();
            state.set_status("Inserted cell below".to_string());
        }

        Action::DeleteCell => {
            if state.notebook.len() > 0 {
                state.notebook.remove_cell(state.current_cell);
                if state.notebook.is_empty() {
                    state.current_cell = 0;
                } else if state.current_cell >= state.notebook.len() {
                    state.current_cell = state.notebook.len() - 1;
                }
                state.scroll_offset = cell_start_row(state, state.current_cell);
                clamp_scroll_offset(state);
                state.clamp_cursor();
                state.set_status("Deleted cell".to_string());
            }
        }

        Action::SetCurrentCellMarkdown => {
            let cursor = state.cursor_char;
            if let Some(cell) = state.current_cell_mut() {
                let source = match cell {
                    Cell::Code(code) => code.source.to_string(),
                    Cell::Markdown(markdown) => markdown.source.clone(),
                };
                *cell = Cell::Markdown(MarkdownCell::with_source(&source));
                state.cursor_char = cursor.min(source.chars().count());
                state.set_status("Converted cell to markdown".to_string());
            }
        }

        Action::SetCurrentCellCode => {
            let cursor = state.cursor_char;
            if let Some(cell) = state.current_cell_mut() {
                let source = match cell {
                    Cell::Code(code) => code.source.to_string(),
                    Cell::Markdown(markdown) => markdown.source.clone(),
                };
                *cell = Cell::Code(CodeCell::with_source(&source));
                state.cursor_char = cursor.min(source.chars().count());
                state.set_status("Converted cell to code".to_string());
            }
        }

        Action::ChangeMode(mode) => {
            state.mode = mode;
            if mode != crate::app::Mode::Normal {
                state.vim_pending_g = false;
            }
            state.show_completion = false;
        }

        Action::ExecuteCell => {
            if let Some(Cell::Code(cell)) = state.current_cell_mut() {
                cell.outputs.clear();
                state.set_status("Executing cell...".to_string());
            }
        }

        Action::ExecuteAllCells => {
            for cell in &mut state.notebook.cells {
                if let Cell::Code(code_cell) = cell {
                    code_cell.outputs.clear();
                }
            }
            state.set_status("Executed all cells".to_string());
        }

        Action::ExecuteCellsBelow => {
            let start = state.current_cell;
            for cell in state.notebook.cells.iter_mut().skip(start) {
                if let Cell::Code(code_cell) = cell {
                    code_cell.outputs.clear();
                }
            }
            state.set_status("Executing code cells from current to bottom...".to_string());
        }

        Action::ExecuteCellsAbove => {
            let end = state
                .current_cell
                .min(state.notebook.cells.len().saturating_sub(1));
            for cell in state.notebook.cells.iter_mut().take(end + 1) {
                if let Cell::Code(code_cell) = cell {
                    code_cell.outputs.clear();
                }
            }
            state.set_status("Executing code cells from top to current...".to_string());
        }

        Action::ToggleCompletion => {
            state.show_completion = !state.show_completion;
            state.completion_selected = 0;
        }

        Action::NextCompletion => {
            if state.show_completion && !state.completion_items.is_empty() {
                state.completion_selected =
                    (state.completion_selected + 1) % state.completion_items.len();
            }
        }

        Action::PreviousCompletion => {
            if state.show_completion && !state.completion_items.is_empty() {
                state.completion_selected = if state.completion_selected == 0 {
                    state.completion_items.len() - 1
                } else {
                    state.completion_selected - 1
                };
            }
        }

        Action::AcceptCompletion => {
            if state.show_completion && !state.completion_items.is_empty() {
                let selected = state.completion_items[state.completion_selected].clone();
                let cursor = state.cursor_char;
                if let Some(Cell::Code(cell)) = state.current_cell_mut() {
                    let pos = cursor.min(cell.source.len_chars());
                    cell.source.insert(pos, &selected);
                    state.cursor_char = pos + selected.chars().count();
                }
                state.show_completion = false;
                state.completion_items.clear();
                state.completion_selected = 0;
            }
        }

        Action::CommandInsertChar(c) => {
            if state.mode == crate::app::Mode::Command {
                state.command_buffer.push(c);
            }
        }

        Action::CommandBackspace => {
            if state.mode == crate::app::Mode::Command {
                state.command_buffer.pop();
            }
        }

        Action::ExecuteCommand => {
            if state.mode == crate::app::Mode::Command {
                let cmd = state.command_buffer.clone();
                state.command_buffer.clear();
                state.mode = crate::app::Mode::Normal;

                let parts: Vec<&str> = cmd.trim().split_whitespace().collect();
                match parts.get(0).copied() {
                    Some("q") | Some("quit") => {
                        state.should_quit = true;
                    }
                    Some("w") | Some("write") => {
                        state.set_status("Saving notebook...".to_string());
                    }
                    Some("e") | Some("edit") => {
                        if let Some(path) = parts.get(1) {
                            state.set_status(format!("Opening: {}", path));
                        }
                    }
                    Some("h") | Some("help") => {
                        state.set_status(
                            "Commands: q(uit), w(rite) [file], wq, x, open <file>, ln, img, kernel, h(elp)"
                                .to_string(),
                        );
                    }
                    _ => {
                        state.set_status(format!("Unknown command: {}", cmd));
                    }
                }
            }
        }

        Action::ScrollUp => {
            let step = scroll_step_rows(state);
            state.scroll_offset = state.scroll_offset.saturating_sub(step);
            clamp_scroll_offset(state);
            if !state.in_cell_mode {
                if let Some(idx) = cell_index_for_scroll_offset(state) {
                    state.current_cell = idx;
                }
            }
        }

        Action::ScrollDown => {
            let step = scroll_step_rows(state);
            let max_scroll = max_notebook_scroll_offset(state);
            state.scroll_offset = state.scroll_offset.saturating_add(step).min(max_scroll);
            clamp_scroll_offset(state);
            if !state.in_cell_mode {
                if let Some(idx) = cell_index_for_scroll_offset(state) {
                    state.current_cell = idx;
                }
            }
        }

        Action::SaveFile => {
            state.set_status("Saving notebook...".to_string());
        }

        Action::OpenFile(path) => {
            state.set_status(format!("Opening: {}", path));
        }

        Action::ShowHelp => {
            state.show_help = true;
            state.set_status("Press Esc to close help".to_string());
        }

        Action::HideHelp => {
            state.show_help = false;
            state.clear_status();
        }

        Action::ClearStatus => {
            state.clear_status();
        }
    }
}
