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
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

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
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return 0;
    }

    // Find the start offset of target_line. If the line does not exist, clamp to EOF.
    let mut line = 0usize;
    let mut line_start = 0usize;
    while line < target_line {
        if line_start >= chars.len() {
            return chars.len();
        }

        match chars[line_start..].iter().position(|ch| *ch == '\n') {
            Some(rel) => {
                line_start += rel + 1;
                line += 1;
            }
            None => {
                return chars.len();
            }
        }
    }

    // Clamp target column inside this line (including empty lines).
    let line_end = chars[line_start..]
        .iter()
        .position(|ch| *ch == '\n')
        .map(|rel| line_start + rel)
        .unwrap_or(chars.len());
    let line_len = line_end.saturating_sub(line_start);
    line_start + target_col.min(line_len)
}

fn total_lines(text: &str) -> usize {
    text.chars().filter(|c| *c == '\n').count() + 1
}

fn char_display_width(ch: char) -> usize {
    if ch == '\t' {
        4
    } else {
        UnicodeWidthChar::width(ch).unwrap_or(0)
    }
}

fn text_display_width(text: &str) -> usize {
    if text.contains('\t') {
        text.chars().map(char_display_width).sum()
    } else {
        UnicodeWidthStr::width(text)
    }
}

fn wrapped_row_count(line_cells: usize, prefix_cells: usize, width: usize) -> usize {
    let cells = (prefix_cells + line_cells).max(1);
    (cells + width - 1) / width
}

fn line_number_width(total_lines: usize) -> usize {
    total_lines.max(1).to_string().len().max(2)
}

fn line_number_gutter_chars(width: usize) -> usize {
    width + 3
}

fn wrapped_line_height(text: &str, width: usize) -> usize {
    let width = width.max(1);
    if text.is_empty() {
        return 1;
    }

    let mut total = 0usize;
    for line in text.split('\n') {
        let cells = text_display_width(line).max(1);
        total += (cells + width - 1) / width;
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
            let show_in_cell_line_numbers =
                is_current
                    && state.in_cell_mode
                    && (state.mode == crate::app::Mode::Insert
                        || state.mode == crate::app::Mode::Normal);
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

            let line_no_chars = if show_in_cell_line_numbers {
                line_number_gutter_chars(line_number_width(source_lines.len()))
            } else {
                0
            };

            let mut source_rows = 0usize;
            for (line_idx, line) in source_lines.iter().enumerate() {
                let prefix_chars = if line_idx == 0 {
                    line_no_chars + prompt.chars().count()
                } else {
                    line_no_chars + prompt_pad.chars().count()
                };
                source_rows += wrapped_row_count(text_display_width(line), prefix_chars, width);
            }

            let mut output_rows = 0usize;
            if !code.outputs.is_empty() {
                output_rows += 1;
            }
            for output in &code.outputs {
                output_rows += wrapped_line_height(&output_text(output), width);
            }

            source_rows.max(1) + output_rows + 2
        }
        Some(Cell::Markdown(markdown)) => crate::ui::render::markdown_cell_content_height(
            &markdown.source,
            width as u16,
            is_current,
            state.in_cell_mode,
            state.mode,
        ),
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

fn cell_index_for_document_row(state: &AppState, document_row: usize) -> Option<usize> {
    if state.notebook.is_empty() {
        return None;
    }

    let mut row = 0usize;
    for idx in 0..state.notebook.len() {
        let cell_rows = cell_visual_rows(state, idx);
        if document_row < row.saturating_add(cell_rows) {
            return Some(idx);
        }
        row = row.saturating_add(cell_rows);
    }

    Some(state.notebook.len().saturating_sub(1))
}

fn centered_document_row(state: &AppState) -> usize {
    let center_offset = viewport_visible_rows(state) / 2;
    state.scroll_offset.saturating_add(center_offset)
}

fn current_cell_text(state: &AppState) -> Option<String> {
    state.current_cell().map(|cell| match cell {
        Cell::Code(code) => code.source.to_string(),
        Cell::Markdown(markdown) => markdown.source.clone(),
    })
}

fn ensure_in_cell_cursor_visible(state: &mut AppState) {
    if !state.in_cell_mode || state.mode == crate::app::Mode::Command {
        return;
    }

    let Some(text) = current_cell_text(state) else {
        return;
    };

    let cursor = state.cursor_char.min(text.chars().count());
    let (line, col) = offset_to_line_col(&text, cursor);
    let width = state.viewport_width.saturating_sub(4).max(20) as usize;

    // Cell visual rows include the top border; content starts one row below it.
    let source_visual_row = match state.current_cell() {
        Some(Cell::Code(code)) => {
            let source_lines: Vec<&str> = if text.is_empty() {
                vec![""]
            } else {
                text.split('\n').collect()
            };
            let safe_line = line.min(source_lines.len().saturating_sub(1));
            let safe_col = col.min(source_lines[safe_line].chars().count());
            let prompt = format!(
                "In [{}]: ",
                code.execution_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| " ".to_string())
            );
            let prompt_pad = " ".repeat(prompt.chars().count());
            let line_no_chars = line_number_gutter_chars(line_number_width(source_lines.len()));

            let mut row = 0usize;
            for (idx, line_text) in source_lines.iter().enumerate().take(safe_line) {
                let prefix_chars = if idx == 0 {
                    line_no_chars + prompt.chars().count()
                } else {
                    line_no_chars + prompt_pad.chars().count()
                };
                row += wrapped_row_count(text_display_width(line_text), prefix_chars, width);
            }

            let current_prefix = if safe_line == 0 {
                line_no_chars + prompt.chars().count()
            } else {
                line_no_chars + prompt_pad.chars().count()
            };
            let col_cells: usize = source_lines[safe_line]
                .chars()
                .take(safe_col)
                .map(char_display_width)
                .sum();
            row + (current_prefix + col_cells) / width
        }
        Some(Cell::Markdown(_)) => {
            let source_lines: Vec<&str> = if text.is_empty() {
                vec![""]
            } else {
                text.split('\n').collect()
            };
            let safe_line = line.min(source_lines.len().saturating_sub(1));
            let safe_col = col.min(source_lines[safe_line].chars().count());
            let prompt = "Md: ";
            let prompt_pad = " ".repeat(prompt.chars().count());
            let line_no_chars = line_number_gutter_chars(line_number_width(source_lines.len()));

            let mut row = 0usize;
            for (idx, line_text) in source_lines.iter().enumerate().take(safe_line) {
                let prefix_chars = if idx == 0 {
                    line_no_chars + prompt.chars().count()
                } else {
                    line_no_chars + prompt_pad.chars().count()
                };
                row += wrapped_row_count(text_display_width(line_text), prefix_chars, width);
            }

            let current_prefix = if safe_line == 0 {
                line_no_chars + prompt.chars().count()
            } else {
                line_no_chars + prompt_pad.chars().count()
            };
            let col_cells: usize = source_lines[safe_line]
                .chars()
                .take(safe_col)
                .map(char_display_width)
                .sum();
            row + (current_prefix + col_cells) / width
        }
        None => 0,
    };

    let cursor_row = cell_start_row(state, state.current_cell)
        .saturating_add(1)
        .saturating_add(source_visual_row);

    let visible = viewport_visible_rows(state).max(1);
    // Keep two safety rows at the bottom to avoid cursor clamping on viewport edge.
    let usable_visible = visible.saturating_sub(2).max(1);
    let view_top = state.scroll_offset;
    let view_bottom = view_top.saturating_add(usable_visible.saturating_sub(1));

    if cursor_row < view_top {
        state.scroll_offset = cursor_row;
    } else if cursor_row >= view_bottom {
        state.scroll_offset = cursor_row.saturating_sub(usable_visible.saturating_sub(1));
    }

    clamp_scroll_offset(state);
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

fn delete_current_line(text: &str, cursor: usize) -> (String, usize) {
    let (_, start, end) = current_line_bounds(text, cursor);
    let mut chars: Vec<char> = text.chars().collect();

    let remove_end = if end < chars.len() && chars[end] == '\n' {
        end + 1
    } else {
        end
    };

    if start < remove_end {
        chars.drain(start..remove_end);
    }

    if chars.is_empty() {
        return (String::new(), 0);
    }

    let new_cursor = start.min(chars.len().saturating_sub(1));
    (chars.into_iter().collect(), new_cursor)
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
                state.vim_pending_d = false;
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
                let insert_pos = end;
                chars.insert(insert_pos, '\n');
                text = chars.into_iter().collect();
                set_current_cell_text(state, &text);
                state.cursor_char = insert_pos + 1;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('g') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let last_line = total_lines(&text).saturating_sub(1);
                    state.cursor_char = line_col_to_offset(&text, last_line, 0);
                    state.vim_pending_g = false;
                    state.vim_pending_d = false;
                } else if state.vim_pending_g {
                    state.cursor_char = 0;
                    state.vim_pending_g = false;
                    state.vim_pending_d = false;
                } else {
                    state.vim_pending_g = true;
                    state.vim_pending_d = false;
                }
            }
            KeyCode::Char('G') => {
                let last_line = total_lines(&text).saturating_sub(1);
                state.cursor_char = line_col_to_offset(&text, last_line, 0);
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('h') | KeyCode::Left => {
                state.cursor_char = state.cursor_char.saturating_sub(1);
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('l') | KeyCode::Right => {
                state.cursor_char = (state.cursor_char + 1).min(text.chars().count());
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('k') | KeyCode::Up => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                if line > 0 {
                    state.cursor_char = line_col_to_offset(&text, line - 1, col);
                }
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                let (line, col) = offset_to_line_col(&text, state.cursor_char);
                let line_count = total_lines(&text);
                if line + 1 < line_count {
                    state.cursor_char = line_col_to_offset(&text, line + 1, col);
                }
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('i') => {
                if key_event.modifiers.contains(KeyModifiers::SHIFT) {
                    let (_, start, _) = current_line_bounds(&text, state.cursor_char);
                    state.cursor_char = start;
                }
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('I') => {
                let (_, start, _) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = start;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
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
                state.vim_pending_d = false;
            }
            KeyCode::Char('A') => {
                let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = end;
                state.mode = crate::app::Mode::Insert;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
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
                state.vim_pending_d = false;
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
                state.vim_pending_d = false;
            }
            KeyCode::Char('d') | KeyCode::Char('D') => {
                if state.vim_pending_d {
                    let (new_text, new_cursor) = delete_current_line(&text, state.cursor_char);
                    set_current_cell_text(state, &new_text);
                    state.cursor_char = new_cursor;
                    state.vim_pending_d = false;
                } else {
                    state.vim_pending_d = true;
                }
                state.vim_pending_g = false;
            }
            KeyCode::Char('^') | KeyCode::Char('6')
                if key_event.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                let (_, start, end) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = first_non_ws_offset(&text, start, end);
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('$') | KeyCode::Char('4')
                if key_event.modifiers.contains(KeyModifiers::SHIFT) =>
            {
                let (_, _, end) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = end;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('0') => {
                let (_, start, _) = current_line_bounds(&text, state.cursor_char);
                state.cursor_char = start;
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Char('x') => {
                let mut chars: Vec<char> = text.chars().collect();
                if state.cursor_char < chars.len() {
                    chars.remove(state.cursor_char);
                    text = chars.into_iter().collect();
                    set_current_cell_text(state, &text);
                    state.cursor_char = state.cursor_char.min(text.chars().count());
                }
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            KeyCode::Esc => {
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
            _ => {
                state.vim_pending_g = false;
                state.vim_pending_d = false;
            }
        },
        crate::app::Mode::Command => {}
    }

    state.clamp_cursor();
    ensure_in_cell_cursor_visible(state);
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
                ensure_in_cell_cursor_visible(state);
            }
        }

        Action::EnterCell => {
            state.in_cell_mode = true;
            state.mode = crate::app::Mode::Normal;
            state.vim_pending_g = false;
            state.vim_pending_d = false;
            state.show_completion = false;
            state.clamp_cursor();
            ensure_in_cell_cursor_visible(state);
        }

        Action::LeaveCell => {
            state.in_cell_mode = false;
            state.mode = crate::app::Mode::Normal;
            state.vim_pending_g = false;
            state.vim_pending_d = false;
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
                state.vim_pending_d = false;
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
                if let Some(idx) = cell_index_for_document_row(state, centered_document_row(state))
                {
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
                if let Some(idx) = cell_index_for_document_row(state, centered_document_row(state))
                {
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
