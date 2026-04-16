#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::app::AppState;
use crate::notebook::Cell;
use crate::ui::components::{
    render_completion_popup as build_completion_popup, render_help_popup, HelpItem, PopupItem,
};
use base64::Engine;
use image::{imageops::FilterType, DynamicImage, GenericImageView, ImageBuffer, Rgba};
use mathlex::parse_latex;
use pulldown_cmark::{Alignment as MdAlignment, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::{
    layout::Alignment,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol, Resize, StatefulImage};
use resvg::{tiny_skia, usvg};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

const ASCII_IMAGE_WIDTH_DEFAULT: u32 = 56;
const ASCII_IMAGE_HEIGHT_MAX: u32 = 20;
const ASCII_IMAGE_HEIGHT_MIN: u32 = 1;
const MAX_SVG_INPUT_CHARS: usize = 300_000;
const MAX_IMAGE_BASE64_CHARS: usize = 2_000_000;
const MAX_IMAGE_DECODED_BYTES: usize = 6_000_000;

static IMAGE_RENDER_CHAR_WIDTH: AtomicU32 = AtomicU32::new(ASCII_IMAGE_WIDTH_DEFAULT);
static DISPLAY_IMAGE_PICKER: OnceLock<Picker> = OnceLock::new();

const PYTHON_KEYWORDS: &[&str] = &[
    "and", "as", "assert", "async", "await", "break", "class", "continue", "def", "del", "elif",
    "else", "except", "False", "finally", "for", "from", "global", "if", "import", "in", "is",
    "lambda", "None", "nonlocal", "not", "or", "pass", "raise", "return", "True", "try", "while",
    "with", "yield",
];

pub fn render(frame: &mut Frame, state: &AppState) {
    let area = frame.area();

    let layout = crate::ui::layout::AppLayout::calculate(
        area,
        state.mode == crate::app::Mode::Command,
        state.show_completion || state.show_target_picker,
    );

    // Keep previews compact; terminal images are only previews, not a viewer.
    let adaptive_width = layout.main_area.width.saturating_sub(20).clamp(16, 64) as u32;
    IMAGE_RENDER_CHAR_WIDTH.store(adaptive_width, Ordering::Relaxed);

    // 先清除整个主区域，避免任何残留
    frame.render_widget(Clear, layout.main_area);

    // 然后再渲染所有 cells...
    render_cells(frame, state, layout.main_area);

    render_status_bar(frame, state, layout.status_bar);

    if state.mode == crate::app::Mode::Command {
        render_command_line(frame, state, layout.command_line);
    }

    if state.show_completion && !state.completion_items.is_empty() {
        render_completion_popup(frame, state, layout.completion_popup);
    }

    if state.show_target_picker && !state.target_picker_items.is_empty() {
        let items: Vec<PopupItem> = state
            .target_picker_items
            .iter()
            .map(|item| PopupItem {
                label: item.label.clone(),
                detail: item.detail.clone(),
            })
            .collect();
        let popup = build_completion_popup(
            &items,
            state.target_picker_selected,
            &state.target_picker_title,
        );
        frame.render_widget(Clear, layout.completion_popup);
        frame.render_widget(popup, layout.completion_popup);
    }

    if state.show_kernel_selector && !state.kernel_items.is_empty() {
        render_kernel_selector_overlay(frame, area, state);
    }

    if state.show_help {
        render_help_overlay(frame, area);
    }
}

fn display_image_picker() -> &'static Picker {
    DISPLAY_IMAGE_PICKER
        .get_or_init(|| Picker::from_query_stdio().unwrap_or_else(|_| Picker::halfblocks()))
}

fn render_kernel_selector_overlay(
    frame: &mut Frame,
    area: ratatui::layout::Rect,
    state: &AppState,
) {
    let popup_width = (area.width.saturating_mul(4) / 5).clamp(52, 110);
    let popup_height = (area.height.saturating_mul(3) / 5).clamp(10, 22);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = ratatui::layout::Rect {
        x,
        y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    let items: Vec<Line> = state
        .kernel_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == state.kernel_selected;
            let style = if is_selected {
                Style::default()
                    .add_modifier(Modifier::REVERSED)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(Span::styled(item.clone(), style))
        })
        .collect();

    let block = Block::default()
        .title("Kernel Selector (Enter apply, Esc cancel)")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    frame.render_widget(Clear, popup_area);
    frame.render_widget(
        Paragraph::new(items)
            .block(block)
            .wrap(Wrap { trim: false }),
        popup_area,
    );
}

fn render_help_overlay(frame: &mut Frame, area: ratatui::layout::Rect) {
    let popup_width = (area.width.saturating_mul(3) / 4).clamp(48, 96);
    let popup_height = (area.height.saturating_mul(3) / 4).clamp(12, 24);
    let x = area.x + (area.width.saturating_sub(popup_width)) / 2;
    let y = area.y + (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = ratatui::layout::Rect {
        x,
        y,
        width: popup_width.min(area.width),
        height: popup_height.min(area.height),
    };

    let items = [
        HelpItem {
            key: ":h".to_string(),
            description: "show this help".to_string(),
        },
        HelpItem {
            key: "Esc".to_string(),
            description: "close help".to_string(),
        },
        HelpItem {
            key: "Enter".to_string(),
            description: "enter current cell (vim normal)".to_string(),
        },
        HelpItem {
            key: "Esc (in cell)".to_string(),
            description: "leave cell back to cell mode".to_string(),
        },
        HelpItem {
            key: ":kernel".to_string(),
            description: "open kernel selector".to_string(),
        },
        HelpItem {
            key: "Ctrl-r / r".to_string(),
            description: "run current cell".to_string(),
        },
        HelpItem {
            key: "Ctrl-Enter / Shift-Enter".to_string(),
            description: "run current cell".to_string(),
        },
        HelpItem {
            key: "i / I / a / A".to_string(),
            description: "vim insert start/start-of-line/append/end-of-line".to_string(),
        },
        HelpItem {
            key: "Ctrl-R".to_string(),
            description: "run all code cells".to_string(),
        },
        HelpItem {
            key: ":ra".to_string(),
            description: "run all code cells".to_string(),
        },
        HelpItem {
            key: ":ro".to_string(),
            description: "run current and above".to_string(),
        },
        HelpItem {
            key: ":rb".to_string(),
            description: "run current and below".to_string(),
        },
        HelpItem {
            key: "Ctrl-j".to_string(),
            description: "run current and below".to_string(),
        },
        HelpItem {
            key: "Ctrl-k".to_string(),
            description: "run current and above".to_string(),
        },
        HelpItem {
            key: "Ctrl-s / :w".to_string(),
            description: "save notebook".to_string(),
        },
        HelpItem {
            key: "Ctrl-b / Ctrl-f".to_string(),
            description: "scroll up / down and focus the visible cell".to_string(),
        },
        HelpItem {
            key: ":open".to_string(),
            description: "open notebook file (:open <path>)".to_string(),
        },
        HelpItem {
            key: ":ln".to_string(),
            description: "open markdown links in system viewer".to_string(),
        },
        HelpItem {
            key: ":img".to_string(),
            description: "open markdown or output images in system viewer".to_string(),
        },
        HelpItem {
            key: "i".to_string(),
            description: "enter insert mode".to_string(),
        },
        HelpItem {
            key: ":".to_string(),
            description: "enter command mode".to_string(),
        },
    ];

    frame.render_widget(Clear, popup_area);
    frame.render_widget(render_help_popup(&items), popup_area);
}

fn cursor_offset_to_line_col(text: &str, offset_chars: usize) -> (usize, usize) {
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

fn render_status_bar(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let mode_color = match state.mode {
        crate::app::Mode::Normal => Color::Green,
        crate::app::Mode::Insert => Color::Cyan,
        crate::app::Mode::Command => Color::Yellow,
    };

    let mode_text = format!("{:?}", state.mode);
    let status_text = state.status_message.as_deref().unwrap_or("");
    let cell_info = format!(
        "Cell {}/{}  ViewRow {}",
        state.current_cell + 1,
        state.total_cells(),
        state.scroll_offset
    );
    let hint = "  mouse wheel/Ctrl-b/Ctrl-f scroll, A/B insert, M markdown, Y code, D delete, :kernel select, :e <file> open, :q quit";

    let spans = vec![
        Span::styled(mode_text, Style::default().fg(Color::Black).bg(mode_color)),
        Span::raw(" "),
        Span::styled(cell_info, Style::default().fg(Color::White)),
        Span::raw(" "),
        Span::styled(status_text, Style::default().fg(Color::Gray)),
        Span::raw(hint),
    ];

    let paragraph = Paragraph::new(Line::from(spans))
        .alignment(Alignment::Left)
        .style(Style::default().bg(Color::DarkGray));

    frame.render_widget(paragraph, area);
}

fn render_command_line(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let text = format!(":{}", state.command_buffer);
    let paragraph = Paragraph::new(text).style(Style::default().fg(Color::White).bg(Color::Black));

    frame.render_widget(paragraph, area);

    let cursor_x = area
        .x
        .saturating_add(1 + state.command_buffer.chars().count() as u16);
    frame.set_cursor_position((cursor_x, area.y));
}

fn render_cells(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let viewport_start = state.scroll_offset;
    let viewport_end = viewport_start.saturating_add(area.height as usize);
    let content_width = area.width.saturating_sub(2).max(1);
    let mut document_row = 0usize;

    for (idx, cell) in state.notebook.cells.iter().enumerate() {
        let is_current = idx == state.current_cell;
        let cell_height = calculate_cell_height(cell, content_width, is_current, state.mode);
        let cell_with_gap = cell_height.saturating_add(1);
        let cell_start = document_row;
        let cell_end = cell_start.saturating_add(cell_height);

        if cell_end <= viewport_start {
            document_row = document_row.saturating_add(cell_with_gap);
            continue;
        }

        if cell_start >= viewport_end {
            break;
        }

        let visible_start_in_cell = viewport_start.saturating_sub(cell_start).min(cell_height);
        let visible_end_in_cell = viewport_end.saturating_sub(cell_start).min(cell_height);
        let visible_height = visible_end_in_cell.saturating_sub(visible_start_in_cell);

        if visible_height > 0 {
            let render_y = area
                .y
                .saturating_add(cell_start.saturating_sub(viewport_start) as u16);
            let cell_area = ratatui::layout::Rect {
                x: area.x,
                y: render_y,
                width: area.width,
                height: visible_height as u16,
            };
            frame.render_widget(Clear, cell_area);
            let is_clipped = visible_start_in_cell > 0 || visible_end_in_cell < cell_height;
            render_cell(
                frame,
                state,
                cell,
                is_current,
                is_clipped,
                visible_start_in_cell,
                cell_area,
            );
        }

        document_row = document_row.saturating_add(cell_with_gap);
    }
}

fn calculate_cell_height(
    cell: &Cell,
    max_width: u16,
    is_current: bool,
    mode: crate::app::Mode,
) -> usize {
    match cell {
        Cell::Code(code_cell) => {
            let lines = build_code_cell_lines(
                &code_cell.source.to_string(),
                code_cell.execution_count,
                &code_cell.outputs,
            );
            wrapped_rendered_lines_height(&lines, max_width).max(1) + 2
        }
        Cell::Markdown(markdown_cell) => {
            if is_current && mode == crate::app::Mode::Insert {
                let md_source_text = build_markdown_source_prefixed_text(&markdown_cell.source);
                wrapped_text_height(&md_source_text, max_width).max(1) + 2
            } else {
                let rendered = render_markdown_to_lines(&markdown_cell.source);
                let content_height = wrapped_rendered_lines_height(&rendered, max_width).max(1);
                if is_current {
                    content_height + 2
                } else {
                    content_height
                }
            }
        }
    }
}

fn wrapped_text_height(text: &str, width: u16) -> usize {
    let width = width.max(1) as usize;
    let mut total = 0usize;

    if text.is_empty() {
        return 1;
    }

    for line in text.lines() {
        let chars = line.chars().count().max(1);
        total += (chars + width - 1) / width;
    }

    total.max(1)
}

fn wrapped_rendered_lines_height(lines: &[Line<'_>], width: u16) -> usize {
    let width = width.max(1) as usize;
    if lines.is_empty() {
        return 1;
    }

    let mut total = 0usize;
    for line in lines {
        let chars = line
            .spans
            .iter()
            .map(|span| span.content.chars().count())
            .sum::<usize>()
            .max(1);
        total += (chars + width - 1) / width;
    }

    total.max(1)
}

fn build_code_prefixed_text(source: &str, execution_count: Option<u32>) -> String {
    let source_lines: Vec<&str> = if source.is_empty() {
        vec![""]
    } else {
        source.split('\n').collect()
    };

    let prompt = format!(
        "In [{}]: ",
        execution_count
            .map(|n| n.to_string())
            .unwrap_or_else(|| " ".to_string())
    );
    let prompt_pad = " ".repeat(prompt.chars().count());

    let mut out = String::new();
    for (idx, line) in source_lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if idx == 0 {
            out.push_str(&prompt);
        } else {
            out.push_str(&prompt_pad);
        }
        out.push_str(line);
    }
    out
}

fn build_code_cell_lines(
    source: &str,
    execution_count: Option<u32>,
    outputs: &[crate::notebook::Output],
) -> Vec<Line<'static>> {
    let source_lines = sanitize_and_split_lines(source);
    let prompt = format!(
        "In [{}]: ",
        execution_count
            .map(|n| n.to_string())
            .unwrap_or_else(|| " ".to_string())
    );
    let prompt_pad = " ".repeat(prompt.chars().count());

    let mut lines: Vec<Line> = source_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let prefix = if idx == 0 { &prompt } else { &prompt_pad };
            let mut spans = vec![Span::styled(
                prefix.to_string(),
                Style::default().fg(Color::Blue),
            )];
            spans.extend(highlight_python_line(line));
            Line::from(spans)
        })
        .collect();

    if !outputs.is_empty() {
        lines.push(Line::from(String::new()));
        for output in outputs {
            lines.extend(output_to_prefixed_lines(output, execution_count));
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![
            Span::styled(prompt, Style::default().fg(Color::Blue)),
            Span::styled(String::new(), Style::default().fg(Color::Reset)),
        ]));
    }

    lines
}

fn highlight_python_line(line: &str) -> Vec<Span<'static>> {
    if line.is_empty() {
        return vec![Span::styled(
            String::new(),
            Style::default().fg(Color::Reset),
        )];
    }

    let mut spans = Vec::new();
    let chars: Vec<char> = line.chars().collect();
    let mut i = 0usize;

    while i < chars.len() {
        let ch = chars[i];
        if ch == '#' {
            let text: String = chars[i..].iter().collect();
            spans.push(Span::styled(text, Style::default().fg(Color::DarkGray)));
            break;
        }

        if ch == '\'' || ch == '"' {
            let quote = ch;
            let start = i;
            i += 1;
            while i < chars.len() {
                if chars[i] == '\\' {
                    i = (i + 2).min(chars.len());
                    continue;
                }
                if chars[i] == quote {
                    i += 1;
                    break;
                }
                i += 1;
            }
            let text: String = chars[start..i.min(chars.len())].iter().collect();
            spans.push(Span::styled(text, Style::default().fg(Color::Yellow)));
            continue;
        }

        if ch.is_ascii_digit() {
            let start = i;
            i += 1;
            while i < chars.len()
                && (chars[i].is_ascii_alphanumeric() || matches!(chars[i], '.' | '_' | 'x' | 'X'))
            {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            spans.push(Span::styled(text, Style::default().fg(Color::Cyan)));
            continue;
        }

        if ch.is_ascii_alphabetic() || ch == '_' {
            let start = i;
            i += 1;
            while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '_') {
                i += 1;
            }
            let text: String = chars[start..i].iter().collect();
            if PYTHON_KEYWORDS.iter().any(|kw| *kw == text) {
                spans.push(Span::styled(
                    text,
                    Style::default()
                        .fg(Color::Magenta)
                        .add_modifier(Modifier::BOLD),
                ));
            } else {
                spans.push(Span::styled(text, Style::default().fg(Color::Reset)));
            }
            continue;
        }

        if "+-*/%=<>!&|^~:".contains(ch) {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(Color::LightBlue),
            ));
            i += 1;
            continue;
        }

        spans.push(Span::styled(
            ch.to_string(),
            Style::default().fg(Color::Reset),
        ));
        i += 1;
    }

    spans
}

fn build_markdown_source_prefixed_text(source: &str) -> String {
    let source_lines: Vec<&str> = if source.is_empty() {
        vec![""]
    } else {
        source.split('\n').collect()
    };
    let prompt = "Md: ";
    let prompt_pad = " ".repeat(prompt.chars().count());

    let mut out = String::new();
    for (idx, line) in source_lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        if idx == 0 {
            out.push_str(prompt);
        } else {
            out.push_str(&prompt_pad);
        }
        out.push_str(line);
    }
    out
}

fn sanitize_and_split_lines(text: &str) -> Vec<String> {
    let cleaned = sanitize_output_text(text);
    if cleaned.is_empty() {
        vec![String::new()]
    } else {
        cleaned.lines().map(|line| line.to_string()).collect()
    }
}

fn sanitize_output_text(text: &str) -> String {
    let normalized = text.replace("\r\n", "\n");
    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut chars = normalized.chars().peekable();

    while let Some(ch) = chars.next() {
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
            '\u{1b}' => {
                skip_ansi_sequence(&mut chars);
            }
            _ => current_line.push(ch),
        }
    }

    lines.push(current_line);
    strip_ansi_sequences(&lines.join("\n"))
}

fn skip_ansi_sequence(chars: &mut std::iter::Peekable<std::str::Chars<'_>>) {
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

fn prefixed_output_lines_from_vec(
    lines: &[String],
    first_prefix: &str,
    continuation_prefix: &str,
    prefix_style: Style,
    text_style: Style,
) -> Vec<Line<'static>> {
    if lines.is_empty() {
        return vec![Line::from(vec![
            Span::styled(first_prefix.to_string(), prefix_style),
            Span::styled(String::new(), text_style),
        ])];
    }

    lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let prefix = if idx == 0 {
                first_prefix
            } else {
                continuation_prefix
            };
            Line::from(vec![
                Span::styled(prefix.to_string(), prefix_style),
                Span::styled(line.clone(), text_style),
            ])
        })
        .collect()
}

fn display_data_text_lines(data: &crate::notebook::DisplayData) -> Vec<String> {
    let cache_key = display_data_cache_key(data);
    if let Ok(cache) = display_data_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let computed = display_data_text_lines_uncached(data);
    if let Ok(mut cache) = display_data_cache().lock() {
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(cache_key, computed.clone());
    }
    computed
}

fn display_data_image_lines(data: &crate::notebook::DisplayData) -> Option<Vec<Line<'static>>> {
    let cache_key = display_data_cache_key(data);
    if let Ok(cache) = display_image_cache().lock() {
        if let Some(cached) = cache.get(&cache_key) {
            return cached.clone();
        }
    }

    let computed = display_data_image_lines_uncached(data);
    if let Ok(mut cache) = display_image_cache().lock() {
        if cache.len() > 256 {
            cache.clear();
        }
        cache.insert(cache_key, computed.clone());
    }

    computed
}

fn display_data_image_lines_uncached(
    data: &crate::notebook::DisplayData,
) -> Option<Vec<Line<'static>>> {
    if data
        .data
        .get("application/pdf")
        .and_then(|v| v.as_str())
        .is_some()
    {
        return None;
    }

    if let Some(svg) = data.data.get("image/svg+xml").and_then(|v| v.as_str()) {
        if svg.len() > MAX_SVG_INPUT_CHARS {
            return Some(vec![Line::from(format!(
                "[svg too large: {} chars, skipped]",
                svg.len()
            ))]);
        }
        return svg_data_to_color_lines(svg);
    }

    for key in ["image/png", "image/jpeg", "image/jpg", "image/webp"] {
        if let Some(encoded) = data.data.get(key).and_then(|v| v.as_str()) {
            if encoded.len() > MAX_IMAGE_BASE64_CHARS {
                return Some(vec![Line::from(format!(
                    "[{} too large: {} chars, skipped]",
                    key,
                    encoded.len()
                ))]);
            }
            return encoded_image_to_color_lines(encoded, key);
        }
    }

    None
}

fn display_image_cache() -> &'static Mutex<HashMap<u64, Option<Vec<Line<'static>>>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Option<Vec<Line<'static>>>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn display_data_text_lines_uncached(data: &crate::notebook::DisplayData) -> Vec<String> {
    if let Some(pdf_payload) = data.data.get("application/pdf").and_then(|v| v.as_str()) {
        return vec![format!(
            "[application/pdf not supported yet, payload {} chars]",
            pdf_payload.len()
        )];
    }

    if let Some(svg) = data.data.get("image/svg+xml").and_then(|v| v.as_str()) {
        if svg.len() > MAX_SVG_INPUT_CHARS {
            return vec![format!("[svg too large: {} chars, skipped]", svg.len())];
        }
        return vec!["[svg render failed]".to_string()];
    }

    for key in ["image/png", "image/jpeg", "image/jpg", "image/webp"] {
        if let Some(encoded) = data.data.get(key).and_then(|v| v.as_str()) {
            if encoded.len() > MAX_IMAGE_BASE64_CHARS {
                return vec![format!(
                    "[{} too large: {} chars, skipped]",
                    key,
                    encoded.len()
                )];
            }
            return vec![format!("[{} render failed]", key)];
        }
    }

    data.data
        .get("text/plain")
        .and_then(|v| v.as_str())
        .map(sanitize_output_text)
        .map(|text| {
            if text.is_empty() {
                vec!["[Display Data]".to_string()]
            } else {
                text.lines().map(|line| line.to_string()).collect()
            }
        })
        .unwrap_or_else(|| vec!["[Display Data]".to_string()])
}

fn display_data_cache() -> &'static Mutex<HashMap<u64, Vec<String>>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, Vec<String>>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn display_data_cache_key(data: &crate::notebook::DisplayData) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();

    if let Some(pdf) = data.data.get("application/pdf").and_then(|v| v.as_str()) {
        "application/pdf".hash(&mut hasher);
        pdf.len().hash(&mut hasher);
        pdf.hash(&mut hasher);
        return hasher.finish();
    }

    if let Some(svg) = data.data.get("image/svg+xml").and_then(|v| v.as_str()) {
        "image/svg+xml".hash(&mut hasher);
        svg.len().hash(&mut hasher);
        svg.hash(&mut hasher);
        return hasher.finish();
    }

    for key in ["image/png", "image/jpeg", "image/jpg", "image/webp"] {
        if let Some(encoded) = data.data.get(key).and_then(|v| v.as_str()) {
            key.hash(&mut hasher);
            encoded.len().hash(&mut hasher);
            encoded.hash(&mut hasher);
            return hasher.finish();
        }
    }

    if let Some(text) = data.data.get("text/plain").and_then(|v| v.as_str()) {
        "text/plain".hash(&mut hasher);
        text.len().hash(&mut hasher);
        text.hash(&mut hasher);
    }

    hasher.finish()
}

fn decode_data_payload(input: &str) -> Option<Vec<u8>> {
    if input.len() > MAX_IMAGE_BASE64_CHARS {
        return None;
    }

    let payload = if let Some(idx) = input.find("base64,") {
        &input[idx + 7..]
    } else {
        input
    };
    let compact: String = payload.chars().filter(|c| !c.is_whitespace()).collect();
    if compact.len() > MAX_IMAGE_BASE64_CHARS {
        return None;
    }
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(compact)
        .ok()?;
    if bytes.len() > MAX_IMAGE_DECODED_BYTES {
        return None;
    }
    Some(bytes)
}

fn encoded_image_to_color_lines(encoded: &str, label: &str) -> Option<Vec<Line<'static>>> {
    let bytes = decode_data_payload(encoded)?;
    let image = image::load_from_memory(&bytes).ok()?;
    Some(dynamic_image_to_color_lines(image, label))
}

fn svg_data_to_color_lines(svg_data: &str) -> Option<Vec<Line<'static>>> {
    let svg_source = if svg_data.trim_start().starts_with('<') {
        svg_data.to_string()
    } else {
        let bytes = decode_data_payload(svg_data)?;
        String::from_utf8(bytes).ok()?
    };

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg_source, &options).ok()?;
    let size = tree.size();
    let src_w = size.width().max(1.0) as u32;
    let src_h = size.height().max(1.0) as u32;

    let target_w = src_w
        .max(1)
        .min(IMAGE_RENDER_CHAR_WIDTH.load(Ordering::Relaxed).max(1));
    let ratio = src_h as f32 / src_w as f32;
    let target_h = ((target_w as f32 * ratio * 0.5).round() as u32)
        .clamp(ASCII_IMAGE_HEIGHT_MIN, ASCII_IMAGE_HEIGHT_MAX);

    let mut pixmap = tiny_skia::Pixmap::new(target_w, target_h)?;
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap_mut);

    let rgba =
        ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(target_w, target_h, pixmap.data().to_vec())?;

    let image = DynamicImage::ImageRgba8(rgba);
    Some(dynamic_image_to_color_lines_with_source(
        image, "svg", src_w, src_h,
    ))
}

fn dynamic_image_to_color_lines(image: DynamicImage, label: &str) -> Vec<Line<'static>> {
    let (src_w, src_h) = image.dimensions();
    dynamic_image_to_color_lines_with_source(image, label, src_w.max(1), src_h.max(1))
}

fn dynamic_image_to_color_lines_with_source(
    image: DynamicImage,
    label: &str,
    src_w: u32,
    src_h: u32,
) -> Vec<Line<'static>> {
    let target_w = src_w
        .max(1)
        .min(IMAGE_RENDER_CHAR_WIDTH.load(Ordering::Relaxed).max(1));
    let ratio = src_h as f32 / src_w.max(1) as f32;
    let target_h = ((target_w as f32 * ratio).round() as u32)
        .clamp(ASCII_IMAGE_HEIGHT_MIN, ASCII_IMAGE_HEIGHT_MAX);
    let pixel_h = target_h.saturating_mul(2);

    let resized = image
        .resize_exact(target_w, pixel_h.max(2), FilterType::Nearest)
        .to_rgba8();

    let mut lines = Vec::with_capacity(target_h as usize + 1);
    lines.push(Line::from(Span::styled(
        format!("[{} {}x{}]", label, src_w, src_h),
        Style::default().fg(Color::Cyan),
    )));

    for y in 0..target_h {
        let upper_y = y.saturating_mul(2);
        let lower_y = (upper_y + 1).min(pixel_h.saturating_sub(1));
        let mut spans = Vec::with_capacity(target_w as usize);
        for x in 0..target_w {
            let up = resized.get_pixel(x, upper_y);
            let down = resized.get_pixel(x, lower_y);

            spans.push(Span::styled(
                "▀",
                Style::default()
                    .fg(rgba_to_color(*up))
                    .bg(rgba_to_color(*down)),
            ));
        }
        lines.push(Line::from(spans));
    }

    lines
}

fn rgba_to_color(px: Rgba<u8>) -> Color {
    let [r, g, b, a] = px.0;
    if a == 255 {
        return Color::Rgb(r, g, b);
    }

    // Blend alpha over black background for terminals without transparency.
    let alpha = a as f32 / 255.0;
    let br = (r as f32 * alpha).round() as u8;
    let bg = (g as f32 * alpha).round() as u8;
    let bb = (b as f32 * alpha).round() as u8;
    Color::Rgb(br, bg, bb)
}

fn prefixed_output_lines_from_rendered_lines(
    lines: &[Line<'static>],
    first_prefix: &str,
    continuation_prefix: &str,
    prefix_style: Style,
) -> Vec<Line<'static>> {
    if lines.is_empty() {
        return vec![Line::from(vec![
            Span::styled(first_prefix.to_string(), prefix_style),
            Span::raw(String::new()),
        ])];
    }

    lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let prefix = if idx == 0 {
                first_prefix
            } else {
                continuation_prefix
            };

            let mut spans = Vec::with_capacity(line.spans.len() + 1);
            spans.push(Span::styled(prefix.to_string(), prefix_style));
            spans.extend(line.spans.clone());
            Line::from(spans)
        })
        .collect()
}

fn decode_display_data_image(
    output: &crate::notebook::Output,
) -> Option<(&'static str, DynamicImage)> {
    let crate::notebook::Output::DisplayData(data) = output else {
        return None;
    };

    if let Some(svg) = data.data.get("image/svg+xml").and_then(|v| v.as_str()) {
        if svg.len() > MAX_SVG_INPUT_CHARS {
            return None;
        }
        return decode_svg_to_image(svg).map(|image| ("svg", image));
    }

    for key in ["image/png", "image/jpeg", "image/jpg", "image/webp"] {
        if let Some(encoded) = data.data.get(key).and_then(|v| v.as_str()) {
            if encoded.len() > MAX_IMAGE_BASE64_CHARS {
                return None;
            }
            return decode_encoded_image_to_image(encoded).map(|image| (key, image));
        }
    }

    None
}

fn decode_encoded_image_to_image(encoded: &str) -> Option<DynamicImage> {
    let bytes = decode_data_payload(encoded)?;
    image::load_from_memory(&bytes).ok()
}

fn decode_svg_to_image(svg_data: &str) -> Option<DynamicImage> {
    let svg_source = if svg_data.trim_start().starts_with('<') {
        svg_data.to_string()
    } else {
        let bytes = decode_data_payload(svg_data)?;
        String::from_utf8(bytes).ok()?
    };

    let options = usvg::Options::default();
    let tree = usvg::Tree::from_str(&svg_source, &options).ok()?;
    let size = tree.size();
    let src_w = size.width().max(1.0) as u32;
    let src_h = size.height().max(1.0) as u32;

    let mut pixmap = tiny_skia::Pixmap::new(src_w, src_h)?;
    let mut pixmap_mut = pixmap.as_mut();
    resvg::render(&tree, tiny_skia::Transform::identity(), &mut pixmap_mut);

    let rgba = ImageBuffer::<Rgba<u8>, Vec<u8>>::from_raw(src_w, src_h, pixmap.data().to_vec())?;
    Some(DynamicImage::ImageRgba8(rgba))
}

fn render_display_image_widget(
    frame: &mut Frame,
    mime: &'static str,
    image: DynamicImage,
    area: ratatui::layout::Rect,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }

    let cache_key = image_cache_key(mime, &image);
    if let Ok(mut cache) = display_image_protocol_cache().lock() {
        let protocol = cache.entry(cache_key).or_insert_with(|| {
            let picker = display_image_picker();
            picker.new_resize_protocol(image)
        });

        let widget = StatefulImage::<StatefulProtocol>::default()
            .resize(Resize::Scale(Some(FilterType::Nearest)));
        frame.render_stateful_widget(widget, area, protocol);
    }
}

fn display_image_protocol_cache() -> &'static Mutex<HashMap<u64, StatefulProtocol>> {
    static CACHE: OnceLock<Mutex<HashMap<u64, StatefulProtocol>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn image_cache_key(mime: &str, image: &DynamicImage) -> u64 {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    mime.hash(&mut hasher);
    image.width().hash(&mut hasher);
    image.height().hash(&mut hasher);
    image.as_bytes().hash(&mut hasher);
    hasher.finish()
}

fn render_cell(
    frame: &mut Frame,
    state: &AppState,
    cell: &Cell,
    is_current: bool,
    is_clipped: bool,
    scroll_lines: usize,
    area: ratatui::layout::Rect,
) {
    match cell {
        Cell::Code(code_cell) => render_code_cell(
            frame,
            code_cell,
            is_current,
            state.in_cell_mode,
            state.mode,
            state.cursor_char,
            is_clipped,
            scroll_lines,
            area,
        ),
        Cell::Markdown(markdown_cell) => render_markdown_cell(
            frame,
            markdown_cell,
            area,
            is_current,
            state.in_cell_mode,
            state.mode,
            state.cursor_char,
            is_clipped,
            scroll_lines,
        ),
    }
}

fn render_code_cell(
    frame: &mut Frame,
    cell: &crate::notebook::CodeCell,
    is_current: bool,
    in_cell_mode: bool,
    mode: crate::app::Mode,
    cursor_char: usize,
    is_clipped: bool,
    scroll_lines: usize,
    area: ratatui::layout::Rect,
) {
    let source = sanitize_output_text(&cell.source.to_string());
    let source_lines: Vec<String> = sanitize_and_split_lines(&source);
    let prompt = format!(
        "In [{}]: ",
        cell.execution_count
            .map(|n| n.to_string())
            .unwrap_or_else(|| " ".to_string())
    );
    let prompt_pad = " ".repeat(prompt.chars().count());
    let source_code_lines: Vec<Line> = source_lines
        .iter()
        .enumerate()
        .map(|(idx, line)| {
            let prefix = if idx == 0 { &prompt } else { &prompt_pad };
            let mut spans = vec![Span::styled(
                prefix.to_string(),
                Style::default().fg(Color::Blue),
            )];
            spans.extend(highlight_python_line(line));
            Line::from(spans)
        })
        .collect();

    let mut all_lines = source_code_lines.clone();
    if !cell.outputs.is_empty() {
        all_lines.push(Line::from(String::new()));
        for output in &cell.outputs {
            all_lines.extend(output_to_prefixed_lines(output, cell.execution_count));
        }
    }

    if is_clipped {
        let border_style = if is_current {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Reset)
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let content_height = wrapped_rendered_lines_height(&all_lines, inner.width).max(1) as u16;
        // scroll_lines counts full-cell rows including the cell frame.
        let content_scroll = scroll_lines.saturating_sub(1);

        if inner.width > 0 && inner.height > 0 {
            frame.render_widget(
                Paragraph::new(all_lines)
                    .wrap(Wrap { trim: false })
                    .scroll((
                        content_scroll.min(content_height.saturating_sub(inner.height) as usize)
                            as u16,
                        0,
                    )),
                inner,
            );
        }

        if is_current
            && in_cell_mode
            && (mode == crate::app::Mode::Insert || mode == crate::app::Mode::Normal)
            && inner.height > 0
        {
            let clamped_cursor = cursor_char.min(source.chars().count());
            let (line, col) = cursor_offset_to_line_col(&source, clamped_cursor);
            let line_idx = line.min(source_lines.len().saturating_sub(1));
            let max_col = source_lines[line_idx].chars().count();
            let visible_y = line_idx.saturating_sub(content_scroll);
            let cursor_x = inner
                .x
                .saturating_add(prompt.chars().count() as u16)
                .saturating_add(col.min(max_col) as u16);
            let cursor_y = inner.y.saturating_add(visible_y as u16);
            if cursor_y < inner.y + inner.height {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
        }

        return;
    }

    let border_style = if is_current {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Reset)
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style);

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let content_height = wrapped_rendered_lines_height(&all_lines, inner.width).max(1) as u16;
    let code_area_height = inner.height.max(1);
    let code_scroll =
        scroll_lines.min((content_height as usize).saturating_sub(code_area_height as usize));
    let code_area = ratatui::layout::Rect {
        x: inner.x,
        y: inner.y,
        width: inner.width,
        height: code_area_height,
    };

    frame.render_widget(
        Paragraph::new(all_lines)
            .wrap(Wrap { trim: false })
            .scroll((code_scroll as u16, 0)),
        code_area,
    );

    let source_rows = wrapped_rendered_lines_height(&source_code_lines, inner.width).max(1);
    let mut output_row = source_rows + if cell.outputs.is_empty() { 0 } else { 1 };
    for output in &cell.outputs {
        let output_lines = output_to_prefixed_lines(output, cell.execution_count);
        let output_height = wrapped_rendered_lines_height(&output_lines, inner.width).max(1);

        if let Some((mime, image)) = decode_display_data_image(output) {
            let visible_start = output_row.saturating_sub(code_scroll);
            let visible_end = output_row
                .saturating_add(output_height)
                .saturating_sub(code_scroll)
                .min(code_area.height as usize);

            if visible_end > visible_start {
                let overlay_area = ratatui::layout::Rect {
                    x: code_area.x,
                    y: code_area.y.saturating_add(visible_start as u16),
                    width: code_area.width,
                    height: (visible_end - visible_start) as u16,
                };
                render_display_image_widget(frame, mime, image, overlay_area);
            }
        }

        output_row = output_row.saturating_add(output_height);
    }

    if is_current
        && in_cell_mode
        && (mode == crate::app::Mode::Insert || mode == crate::app::Mode::Normal)
        && code_area.height > 0
    {
        let clamped_cursor = cursor_char.min(source.chars().count());
        let (line, col) = cursor_offset_to_line_col(&source, clamped_cursor);
        let line_idx = line.min(source_lines.len().saturating_sub(1));
        let max_col = source_lines[line_idx].chars().count();
        let visible_y = line_idx.saturating_sub(code_scroll);
        let cursor_x = code_area
            .x
            .saturating_add(prompt.chars().count() as u16)
            .saturating_add(col.min(max_col) as u16);
        let cursor_y = code_area.y.saturating_add(visible_y as u16);
        if cursor_y < code_area.y + code_area.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }
}

fn render_markdown_cell(
    frame: &mut Frame,
    cell: &crate::notebook::MarkdownCell,
    area: ratatui::layout::Rect,
    is_current: bool,
    in_cell_mode: bool,
    mode: crate::app::Mode,
    cursor_char: usize,
    is_clipped: bool,
    scroll_lines: usize,
) {
    if is_clipped {
        let base_scroll = if is_current {
            scroll_lines.saturating_sub(1)
        } else {
            scroll_lines
        };

        if is_current
            && in_cell_mode
            && (mode == crate::app::Mode::Normal || mode == crate::app::Mode::Insert)
        {
            let source = sanitize_output_text(&cell.source);
            let source_lines = sanitize_and_split_lines(&source);
            let prompt = "Md: ";
            let prompt_pad = " ".repeat(prompt.chars().count());
            let raw_lines: Vec<Line> = source_lines
                .iter()
                .enumerate()
                .map(|(idx, line)| {
                    let prefix = if idx == 0 { prompt } else { &prompt_pad };
                    Line::from(vec![
                        Span::styled(prefix.to_string(), Style::default().fg(Color::Blue)),
                        Span::styled((*line).to_string(), Style::default().fg(Color::Reset)),
                    ])
                })
                .collect();

            frame.render_widget(
                Paragraph::new(raw_lines)
                    .wrap(Wrap { trim: false })
                    .scroll((base_scroll as u16, 0)),
                area,
            );

            let clamped_cursor = cursor_char.min(source.chars().count());
            let (line, col) = cursor_offset_to_line_col(&source, clamped_cursor);
            let line_idx = line.min(source_lines.len().saturating_sub(1));
            let max_col = source_lines[line_idx].chars().count();
            let visible_y = line_idx.saturating_sub(base_scroll);
            let cursor_x = area
                .x
                .saturating_add(prompt.chars().count() as u16)
                .saturating_add(col.min(max_col) as u16);
            let cursor_y = area.y.saturating_add(visible_y as u16);
            if cursor_y < area.y + area.height {
                frame.set_cursor_position((cursor_x, cursor_y));
            }
            return;
        }

        let lines = render_markdown_to_lines(&sanitize_output_text(&cell.source));
        frame.render_widget(
            Paragraph::new(lines)
                .wrap(Wrap { trim: false })
                .scroll((base_scroll as u16, 0))
                .style(Style::default().fg(Color::Reset)),
            area,
        );
        return;
    }

    if is_current && in_cell_mode && mode == crate::app::Mode::Normal {
        let border_style = Style::default().fg(Color::Yellow);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let source = sanitize_output_text(&cell.source);
        let source_lines = sanitize_and_split_lines(&source);

        let prompt = "Md: ";
        let prompt_pad = " ".repeat(prompt.chars().count());
        let raw_lines: Vec<Line> = source_lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let prefix = if idx == 0 { prompt } else { &prompt_pad };
                Line::from(vec![
                    Span::styled(prefix.to_string(), Style::default().fg(Color::Blue)),
                    Span::styled((*line).to_string(), Style::default().fg(Color::Reset)),
                ])
            })
            .collect();

        let prompt_height =
            wrapped_text_height(&build_markdown_source_prefixed_text(&source), inner.width).max(1)
                as u16;
        let normal_scroll = scroll_lines.min(prompt_height.saturating_sub(inner.height) as usize);

        frame.render_widget(
            Paragraph::new(raw_lines)
                .wrap(Wrap { trim: false })
                .scroll((normal_scroll as u16, 0)),
            inner,
        );

        let clamped_cursor = cursor_char.min(source.chars().count());
        let (line, col) = cursor_offset_to_line_col(&source, clamped_cursor);
        let line_idx = line.min(source_lines.len().saturating_sub(1));
        let max_col = source_lines[line_idx].chars().count();
        let visible_y = line_idx.saturating_sub(normal_scroll);
        let cursor_x = inner
            .x
            .saturating_add(prompt.chars().count() as u16)
            .saturating_add(col.min(max_col) as u16);
        let cursor_y = inner.y.saturating_add(visible_y as u16);
        if cursor_y < inner.y + inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        return;
    }

    if is_current && mode != crate::app::Mode::Insert {
        let border_style = Style::default().fg(Color::Yellow);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let lines = render_markdown_to_lines(&cell.source);
        let visible_scroll = scroll_lines.min(
            wrapped_rendered_lines_height(&lines, inner.width)
                .saturating_sub(inner.height as usize),
        );
        let paragraph = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((visible_scroll as u16, 0))
            .style(Style::default().fg(Color::Reset));

        frame.render_widget(paragraph, inner);
        return;
    }

    if is_current && mode == crate::app::Mode::Insert {
        let border_style = Style::default().fg(Color::Yellow);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style);
        let inner = block.inner(area);
        frame.render_widget(block, area);

        let source = sanitize_output_text(&cell.source);
        let source_lines = sanitize_and_split_lines(&source);

        let prompt = "Md: ";
        let prompt_pad = " ".repeat(prompt.chars().count());
        let raw_lines: Vec<Line> = source_lines
            .iter()
            .enumerate()
            .map(|(idx, line)| {
                let prefix = if idx == 0 { prompt } else { &prompt_pad };
                Line::from(vec![
                    Span::styled(prefix.to_string(), Style::default().fg(Color::Blue)),
                    Span::styled((*line).to_string(), Style::default().fg(Color::Reset)),
                ])
            })
            .collect();

        let prompt_height =
            wrapped_text_height(&build_markdown_source_prefixed_text(&source), inner.width).max(1)
                as u16;
        let insert_scroll = scroll_lines.min(prompt_height.saturating_sub(inner.height) as usize);

        let paragraph = Paragraph::new(raw_lines)
            .wrap(Wrap { trim: false })
            .scroll((insert_scroll as u16, 0));
        frame.render_widget(paragraph, inner);

        let clamped_cursor = cursor_char.min(source.chars().count());
        let (line, col) = cursor_offset_to_line_col(&source, clamped_cursor);
        let line_idx = line.min(source_lines.len().saturating_sub(1));
        let max_col = source_lines[line_idx].chars().count();
        let visible_y = line_idx.saturating_sub(insert_scroll);
        let cursor_x = inner
            .x
            .saturating_add(prompt.chars().count() as u16)
            .saturating_add(col.min(max_col) as u16);
        let cursor_y = inner.y.saturating_add(visible_y as u16);
        if cursor_y < inner.y + inner.height {
            frame.set_cursor_position((cursor_x, cursor_y));
        }
        return;
    }

    let lines = render_markdown_to_lines(&sanitize_output_text(&cell.source));
    let visible_scroll = scroll_lines.min(
        wrapped_rendered_lines_height(&lines, area.width).saturating_sub(area.height as usize),
    );
    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((visible_scroll as u16, 0))
        .style(Style::default().fg(Color::Reset));

    frame.render_widget(paragraph, area);
}

fn render_markdown_to_lines(source: &str) -> Vec<Line<'static>> {
    let mut renderer = MarkdownRenderer::new();
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_MATH;
    let parser = Parser::new_ext(source, options);

    for event in parser {
        renderer.push_event(event);
    }

    renderer.finish()
}

struct MarkdownRenderer {
    lines: Vec<Line<'static>>,
    current_line: Vec<Span<'static>>,
    style_stack: Vec<MarkdownStyleKind>,
    quote_depth: usize,
    list_stack: Vec<ListContext>,
    code_block_depth: usize,
    table: Option<TableState>,
}

#[derive(Clone, Debug)]
struct ListContext {
    ordered: bool,
    next_number: u64,
    current_marker: Option<String>,
}

#[derive(Clone, Debug)]
struct TableState {
    alignments: Vec<MdAlignment>,
    header: Option<Vec<Vec<Span<'static>>>>,
    body_rows: Vec<Vec<Vec<Span<'static>>>>,
    current_row: Vec<Vec<Span<'static>>>,
    current_cell: Vec<Span<'static>>,
    in_head: bool,
}

#[derive(Clone, Copy, Debug)]
enum MarkdownStyleKind {
    Heading(HeadingLevel),
    Emphasis,
    Strong,
    Strikethrough,
    Link,
    InlineCode,
    InlineMath,
    HtmlBold,
    HtmlItalic,
    HtmlUnderline,
    HtmlColor(Color),
}

impl MarkdownRenderer {
    fn new() -> Self {
        Self {
            lines: Vec::new(),
            current_line: Vec::new(),
            style_stack: Vec::new(),
            quote_depth: 0,
            list_stack: Vec::new(),
            code_block_depth: 0,
            table: None,
        }
    }

    fn push_event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start_tag(tag),
            Event::End(tag) => self.end_tag(tag),
            Event::Text(text) => self.push_text(text.as_ref()),
            Event::Code(text) => {
                self.push_styled_text(text.as_ref(), MarkdownStyleKind::InlineCode)
            }
            Event::InlineMath(text) => {
                self.push_styled_text(text.as_ref(), MarkdownStyleKind::InlineMath)
            }
            Event::DisplayMath(text) => self.push_display_math(text.as_ref()),
            Event::Html(html) | Event::InlineHtml(html) => self.push_html(html.as_ref()),
            Event::SoftBreak => self.soft_break(),
            Event::HardBreak => self.hard_break(),
            Event::Rule => self.render_rule(),
            Event::TaskListMarker(checked) => {
                self.push_text(if checked { "[x]" } else { "[ ]" });
            }
            Event::FootnoteReference(label) => {
                self.push_text(&format!("[^{}]", label));
            }
        }
    }

    fn start_tag(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => self.flush_current_line(),
            Tag::Heading { level, .. } => {
                self.flush_current_line();
                self.style_stack.push(MarkdownStyleKind::Heading(level));
            }
            Tag::BlockQuote(_) => {
                self.flush_current_line();
                self.quote_depth += 1;
            }
            Tag::CodeBlock(_) => {
                self.flush_current_line();
                self.code_block_depth += 1;
            }
            Tag::List(start) => {
                self.flush_current_line();
                self.list_stack.push(ListContext {
                    ordered: start.is_some(),
                    next_number: start.unwrap_or(1),
                    current_marker: None,
                });
            }
            Tag::Item => {
                self.flush_current_line();
                if let Some(list) = self.list_stack.last_mut() {
                    let marker = if list.ordered {
                        let marker = format!("{}.", list.next_number);
                        list.next_number += 1;
                        marker
                    } else {
                        "•".to_string()
                    };
                    list.current_marker = Some(marker);
                }
            }
            Tag::Table(alignments) => {
                self.flush_current_line();
                self.table = Some(TableState {
                    alignments,
                    header: None,
                    body_rows: Vec::new(),
                    current_row: Vec::new(),
                    current_cell: Vec::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = true;
                }
            }
            Tag::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    table.current_row.clear();
                }
            }
            Tag::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    table.current_cell.clear();
                }
            }
            Tag::Emphasis => self.style_stack.push(MarkdownStyleKind::Emphasis),
            Tag::Strong => self.style_stack.push(MarkdownStyleKind::Strong),
            Tag::Strikethrough => self.style_stack.push(MarkdownStyleKind::Strikethrough),
            Tag::Link { .. } | Tag::Image { .. } => self.style_stack.push(MarkdownStyleKind::Link),
            Tag::MetadataBlock(_) => {}
            Tag::FootnoteDefinition(_) => {}
            Tag::DefinitionList | Tag::DefinitionListTitle | Tag::DefinitionListDefinition => {}
            Tag::HtmlBlock => self.flush_current_line(),
        }
    }

    fn end_tag(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => self.flush_current_line(),
            TagEnd::Heading(_) => {
                self.flush_current_line();
                self.pop_style(MarkdownStyleKind::Heading(HeadingLevel::H1));
            }
            TagEnd::BlockQuote(_) => {
                self.flush_current_line();
                self.quote_depth = self.quote_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.flush_current_line();
                self.code_block_depth = self.code_block_depth.saturating_sub(1);
            }
            TagEnd::List(_) => {
                self.flush_current_line();
                self.list_stack.pop();
            }
            TagEnd::Item => {
                self.flush_current_line();
                if let Some(list) = self.list_stack.last_mut() {
                    list.current_marker = None;
                }
            }
            TagEnd::Table => {
                self.flush_current_line();
                if let Some(table) = self.table.take() {
                    self.lines.extend(render_table_state(table));
                }
            }
            TagEnd::TableHead => {
                if let Some(table) = self.table.as_mut() {
                    table.in_head = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(table) = self.table.as_mut() {
                    if !table.current_cell.is_empty() || !table.current_row.is_empty() {
                        table
                            .current_row
                            .push(std::mem::take(&mut table.current_cell));
                    }
                    let row = std::mem::take(&mut table.current_row);
                    if table.in_head {
                        table.header = Some(row);
                    } else {
                        table.body_rows.push(row);
                    }
                }
            }
            TagEnd::TableCell => {
                if let Some(table) = self.table.as_mut() {
                    table
                        .current_row
                        .push(std::mem::take(&mut table.current_cell));
                }
            }
            TagEnd::Emphasis => self.pop_style(MarkdownStyleKind::Emphasis),
            TagEnd::Strong => self.pop_style(MarkdownStyleKind::Strong),
            TagEnd::Strikethrough => self.pop_style(MarkdownStyleKind::Strikethrough),
            TagEnd::Link | TagEnd::Image => self.pop_style(MarkdownStyleKind::Link),
            TagEnd::MetadataBlock(_) => {}
            TagEnd::HtmlBlock => {}
            TagEnd::FootnoteDefinition => {}
            TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition => {}
        }
    }

    fn push_text(&mut self, text: &str) {
        let style = self.current_style();
        if let Some(table) = self.table.as_mut() {
            Self::push_cell_text(&mut table.current_cell, text, style);
            return;
        }

        if self.code_block_depth > 0 {
            self.push_spans_text(text, Style::default().fg(Color::Reset));
            return;
        }

        self.push_spans_text(text, style);
    }

    fn push_styled_text(&mut self, text: &str, kind: MarkdownStyleKind) {
        let style = self.style_for(kind);
        if let Some(table) = self.table.as_mut() {
            let rendered = if matches!(kind, MarkdownStyleKind::InlineMath) {
                render_terminal_math(text)
            } else {
                text.to_string()
            };
            Self::push_cell_text(&mut table.current_cell, &rendered, style);
            return;
        }

        let rendered = if matches!(kind, MarkdownStyleKind::InlineMath) {
            render_terminal_math(text)
        } else {
            text.to_string()
        };
        self.push_spans_text(&rendered, style);
    }

    fn push_display_math(&mut self, text: &str) {
        let style = Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD);
        let rendered = render_terminal_math(text);
        if let Some(table) = self.table.as_mut() {
            Self::push_cell_text(&mut table.current_cell, &rendered, style);
            return;
        }

        self.flush_current_line();
        let mut spans = self.current_prefix_spans();
        spans.push(Span::styled(rendered, style));
        self.lines.push(Line::from(spans));
    }

    fn push_html(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let mut rest = text;
        while let Some(start) = rest.find('<') {
            let plain = &rest[..start];
            if !plain.is_empty() {
                self.push_html_text(plain);
            }

            let tail = &rest[start + 1..];
            if let Some(close) = tail.find('>') {
                let tag_content = &tail[..close];
                self.apply_html_tag(tag_content);
                rest = &tail[close + 1..];
            } else {
                self.push_html_text(rest);
                return;
            }
        }

        if !rest.is_empty() {
            self.push_html_text(rest);
        }
    }

    fn push_html_text(&mut self, text: &str) {
        let style = self.current_style();
        if let Some(table) = self.table.as_mut() {
            Self::push_cell_text(&mut table.current_cell, text, style);
            return;
        }
        self.push_spans_text(text, style);
    }

    fn apply_html_tag(&mut self, raw_tag: &str) {
        let tag = raw_tag.trim();
        if tag.is_empty() {
            return;
        }

        let is_end = tag.starts_with('/');
        let content = if is_end { &tag[1..] } else { tag };
        let content = content.trim();
        let self_closing = content.ends_with('/');
        let tag_name = content
            .trim_end_matches('/')
            .split_whitespace()
            .next()
            .unwrap_or("")
            .to_ascii_lowercase();

        if is_end {
            match tag_name.as_str() {
                "b" | "strong" => self.pop_style(MarkdownStyleKind::HtmlBold),
                "i" | "em" => self.pop_style(MarkdownStyleKind::HtmlItalic),
                "u" => self.pop_style(MarkdownStyleKind::HtmlUnderline),
                "font" | "span" => self.pop_style(MarkdownStyleKind::HtmlColor(Color::White)),
                _ => {}
            }
            return;
        }

        if tag_name == "br" {
            self.flush_current_line();
            return;
        }

        if self_closing {
            return;
        }

        match tag_name.as_str() {
            "b" | "strong" => self.style_stack.push(MarkdownStyleKind::HtmlBold),
            "i" | "em" => self.style_stack.push(MarkdownStyleKind::HtmlItalic),
            "u" => self.style_stack.push(MarkdownStyleKind::HtmlUnderline),
            "font" | "span" => {
                if let Some(color) = extract_html_color(content) {
                    self.style_stack.push(MarkdownStyleKind::HtmlColor(color));
                }
            }
            _ => {}
        }
    }

    fn soft_break(&mut self) {
        let style = self.current_style();
        if let Some(table) = self.table.as_mut() {
            Self::push_cell_text(&mut table.current_cell, " ", style);
            return;
        }

        self.flush_current_line();
    }

    fn hard_break(&mut self) {
        let style = self.current_style();
        if let Some(table) = self.table.as_mut() {
            Self::push_cell_text(&mut table.current_cell, " ", style);
            return;
        }

        self.flush_current_line();
    }

    fn render_rule(&mut self) {
        self.flush_current_line();
        self.lines.push(Line::from(Span::styled(
            "────────────────────────".to_string(),
            Style::default().fg(Color::Reset),
        )));
    }

    fn push_spans_text(&mut self, text: &str, style: Style) {
        for (idx, segment) in text.split('\n').enumerate() {
            if idx > 0 {
                self.flush_current_line();
            }
            if segment.is_empty() && self.current_line.is_empty() {
                self.ensure_prefix();
                continue;
            }
            self.ensure_prefix();
            self.current_line
                .push(Span::styled(segment.to_string(), style));
        }
    }

    fn flush_current_line(&mut self) {
        if self.current_line.is_empty() {
            return;
        }

        self.lines
            .push(Line::from(std::mem::take(&mut self.current_line)));
    }

    fn ensure_prefix(&mut self) {
        if !self.current_line.is_empty() {
            return;
        }

        self.current_line.extend(self.current_prefix_spans());
    }

    fn current_prefix_spans(&self) -> Vec<Span<'static>> {
        let mut spans = Vec::new();

        for _ in 0..self.quote_depth {
            spans.push(Span::styled(
                "│ ".to_string(),
                Style::default().fg(Color::Blue),
            ));
        }

        for ctx in &self.list_stack {
            let marker = ctx.current_marker.as_deref().unwrap_or("  ");
            let color = if ctx.ordered {
                Color::Yellow
            } else {
                Color::Green
            };
            spans.push(Span::styled(
                format!("{} ", marker),
                Style::default().fg(color),
            ));
        }

        spans
    }

    fn current_style(&self) -> Style {
        let mut style = Style::default().fg(Color::Reset);
        for kind in &self.style_stack {
            style = style.patch(self.style_for(*kind));
        }
        style
    }

    fn style_for(&self, kind: MarkdownStyleKind) -> Style {
        match kind {
            MarkdownStyleKind::Heading(level) => match level {
                HeadingLevel::H1 => Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
                HeadingLevel::H2 => Style::default()
                    .fg(Color::Magenta)
                    .add_modifier(Modifier::BOLD),
                HeadingLevel::H3 => Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
                HeadingLevel::H4 => Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
                HeadingLevel::H5 => Style::default()
                    .fg(Color::Blue)
                    .add_modifier(Modifier::BOLD),
                HeadingLevel::H6 => Style::default()
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            },
            MarkdownStyleKind::Emphasis => Style::default().add_modifier(Modifier::ITALIC),
            MarkdownStyleKind::Strong => Style::default().add_modifier(Modifier::BOLD),
            MarkdownStyleKind::Strikethrough => Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::CROSSED_OUT),
            MarkdownStyleKind::Link => Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::UNDERLINED),
            MarkdownStyleKind::InlineCode => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            MarkdownStyleKind::InlineMath => Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD),
            MarkdownStyleKind::HtmlBold => Style::default().add_modifier(Modifier::BOLD),
            MarkdownStyleKind::HtmlItalic => Style::default().add_modifier(Modifier::ITALIC),
            MarkdownStyleKind::HtmlUnderline => Style::default().add_modifier(Modifier::UNDERLINED),
            MarkdownStyleKind::HtmlColor(color) => Style::default().fg(color),
        }
    }

    fn pop_style(&mut self, kind: MarkdownStyleKind) {
        if let Some(idx) = self
            .style_stack
            .iter()
            .rposition(|existing| same_style_kind(*existing, kind))
        {
            self.style_stack.remove(idx);
        }
    }

    fn finish(mut self) -> Vec<Line<'static>> {
        self.flush_current_line();
        if self.lines.is_empty() {
            self.lines.push(Line::from(String::new()));
        }
        self.lines
    }

    fn push_cell_text(cell: &mut Vec<Span<'static>>, text: &str, style: Style) {
        for segment in text.split('\n') {
            if !cell.is_empty() && segment.is_empty() {
                cell.push(Span::styled(" ".to_string(), style));
            } else if !segment.is_empty() {
                cell.push(Span::styled(segment.to_string(), style));
            }
        }
    }
}

fn same_style_kind(a: MarkdownStyleKind, b: MarkdownStyleKind) -> bool {
    use MarkdownStyleKind::*;

    match (a, b) {
        (Heading(lhs), Heading(rhs)) => lhs == rhs,
        (Emphasis, Emphasis)
        | (Strong, Strong)
        | (Strikethrough, Strikethrough)
        | (Link, Link)
        | (InlineCode, InlineCode)
        | (InlineMath, InlineMath)
        | (HtmlBold, HtmlBold)
        | (HtmlItalic, HtmlItalic)
        | (HtmlUnderline, HtmlUnderline)
        | (HtmlColor(_), HtmlColor(_)) => true,
        _ => false,
    }
}

fn render_table_state(table: TableState) -> Vec<Line<'static>> {
    let mut rows = Vec::new();
    if let Some(header) = table.header {
        rows.push(header);
    }
    rows.extend(table.body_rows);

    if rows.is_empty() {
        return vec![Line::from(String::new())];
    }

    let cols = rows.iter().map(|row| row.len()).max().unwrap_or(0);
    if cols == 0 {
        return vec![Line::from(String::new())];
    }

    let mut normalized_rows: Vec<Vec<Vec<Span<'static>>>> = Vec::new();
    let mut widths = vec![0usize; cols];

    for mut row in rows {
        while row.len() < cols {
            row.push(Vec::new());
        }
        for (idx, cell) in row.iter().enumerate() {
            widths[idx] = widths[idx].max(cell_plain_text_width(cell));
        }
        normalized_rows.push(row);
    }

    let mut lines = Vec::new();
    lines.push(Line::from(Span::styled(
        format_table_border('┌', '┬', '┐', &widths),
        Style::default().fg(Color::Blue),
    )));

    if let Some(header) = normalized_rows.first() {
        lines.push(render_table_row(header, &widths, &table.alignments, true));
        lines.push(Line::from(Span::styled(
            format_table_border('├', '┼', '┤', &widths),
            Style::default().fg(Color::Blue),
        )));

        for row in normalized_rows.iter().skip(1) {
            lines.push(render_table_row(row, &widths, &table.alignments, false));
        }
    }

    lines.push(Line::from(Span::styled(
        format_table_border('└', '┴', '┘', &widths),
        Style::default().fg(Color::Blue),
    )));

    lines
}

fn render_table_row(
    row: &[Vec<Span<'static>>],
    widths: &[usize],
    alignments: &[MdAlignment],
    is_header: bool,
) -> Line<'static> {
    let mut spans = Vec::new();
    let border_style = Style::default().fg(Color::Blue);
    spans.push(Span::styled("│".to_string(), border_style));

    for idx in 0..widths.len() {
        let cell = row.get(idx).cloned().unwrap_or_default();
        let cell_width = cell_plain_text_width(&cell);
        let alignment = alignments.get(idx).copied().unwrap_or(MdAlignment::None);
        let padding = widths[idx].saturating_sub(cell_width);
        let (left_pad, right_pad) = match alignment {
            MdAlignment::Right => (padding, 0),
            MdAlignment::Center => (padding / 2, padding - (padding / 2)),
            MdAlignment::Left | MdAlignment::None => (0, padding),
        };

        spans.push(Span::raw(" ".repeat(1 + left_pad)));

        for span in cell {
            let style = if is_header {
                span.style
                    .patch(Style::default().add_modifier(Modifier::BOLD))
            } else {
                span.style
            };
            spans.push(Span::styled(span.content.to_string(), style));
        }

        spans.push(Span::raw(" ".repeat(1 + right_pad)));
        spans.push(Span::styled("│".to_string(), border_style));
    }

    Line::from(spans)
}

fn cell_plain_text_width(cell: &[Span<'static>]) -> usize {
    cell.iter().map(|span| span.content.chars().count()).sum()
}

fn extract_quoted_or_unquoted_attr(tag_content: &str, attr: &str) -> Option<String> {
    let lower = tag_content.to_ascii_lowercase();
    let pattern = format!("{}=", attr);
    let idx = lower.find(&pattern)?;
    let value_raw = tag_content[idx + pattern.len()..].trim_start();

    if let Some(rest) = value_raw.strip_prefix('"') {
        let end = rest.find('"')?;
        return Some(rest[..end].to_string());
    }

    if let Some(rest) = value_raw.strip_prefix('\'') {
        let end = rest.find('\'')?;
        return Some(rest[..end].to_string());
    }

    let end = value_raw
        .find(|c: char| c.is_whitespace() || c == '>' || c == '/')
        .unwrap_or(value_raw.len());
    Some(value_raw[..end].to_string())
}

fn parse_html_color_value(value: &str) -> Option<Color> {
    let v = value.trim();
    if let Some(hex) = v.strip_prefix('#') {
        if hex.len() == 6 {
            if let Ok(rgb) = u32::from_str_radix(hex, 16) {
                let r = ((rgb >> 16) & 0xff) as u8;
                let g = ((rgb >> 8) & 0xff) as u8;
                let b = (rgb & 0xff) as u8;
                return Some(Color::Rgb(r, g, b));
            }
        }
    }

    match v.to_ascii_lowercase().as_str() {
        "black" => Some(Color::Black),
        "white" => Some(Color::White),
        "red" => Some(Color::Red),
        "green" => Some(Color::Green),
        "blue" => Some(Color::Blue),
        "yellow" => Some(Color::Yellow),
        "cyan" => Some(Color::Cyan),
        "magenta" => Some(Color::Magenta),
        "gray" | "grey" => Some(Color::Gray),
        "darkgray" | "darkgrey" => Some(Color::DarkGray),
        "lightred" => Some(Color::LightRed),
        "lightgreen" => Some(Color::LightGreen),
        "lightblue" => Some(Color::LightBlue),
        "lightyellow" => Some(Color::LightYellow),
        "lightcyan" => Some(Color::LightCyan),
        "lightmagenta" => Some(Color::LightMagenta),
        _ => None,
    }
}

fn extract_html_color(tag_content: &str) -> Option<Color> {
    if let Some(v) = extract_quoted_or_unquoted_attr(tag_content, "color") {
        if let Some(c) = parse_html_color_value(&v) {
            return Some(c);
        }
    }

    if let Some(style_attr) = extract_quoted_or_unquoted_attr(tag_content, "style") {
        let lower = style_attr.to_ascii_lowercase();
        if let Some(idx) = lower.find("color:") {
            let rest = style_attr[idx + 6..].trim_start();
            let end = rest.find(';').unwrap_or(rest.len());
            return parse_html_color_value(rest[..end].trim());
        }
    }

    None
}

fn sanitize_html_fragment(text: &str) -> String {
    let mut sanitized = text
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    sanitized = strip_html_tags(&sanitized);
    sanitized
}

fn render_terminal_math(source: &str) -> String {
    let rendered = parse_latex(source)
        .map(|expression| expression.to_string())
        .unwrap_or_else(|_| latex_fallback_to_terminal(source));

    normalize_terminal_math(&rendered)
}

fn latex_fallback_to_terminal(source: &str) -> String {
    let mut out = source.to_string();

    // Strip sizing/grouping helpers that are not meaningful in plain terminal text.
    for cmd in ["\\left", "\\right", "\\!", "\\,", "\\;", "\\:"] {
        out = out.replace(cmd, "");
    }

    // Common symbolic commands.
    let replacements = [
        ("\\alpha", "α"),
        ("\\beta", "β"),
        ("\\gamma", "γ"),
        ("\\delta", "δ"),
        ("\\epsilon", "ϵ"),
        ("\\theta", "θ"),
        ("\\lambda", "λ"),
        ("\\mu", "μ"),
        ("\\pi", "π"),
        ("\\sigma", "σ"),
        ("\\phi", "φ"),
        ("\\omega", "ω"),
        ("\\Gamma", "Γ"),
        ("\\Delta", "Δ"),
        ("\\Theta", "Θ"),
        ("\\Lambda", "Λ"),
        ("\\Pi", "Π"),
        ("\\Sigma", "Σ"),
        ("\\Phi", "Φ"),
        ("\\Omega", "Ω"),
        ("\\times", "×"),
        ("\\cdot", "·"),
        ("\\pm", "±"),
        ("\\neq", "≠"),
        ("\\ne", "≠"),
        ("\\leq", "≤"),
        ("\\le", "≤"),
        ("\\geq", "≥"),
        ("\\ge", "≥"),
        ("\\to", "→"),
        ("\\rightarrow", "→"),
        ("\\leftarrow", "←"),
        ("\\infty", "∞"),
        ("\\sum", "∑"),
        ("\\prod", "∏"),
        ("\\int", "∫"),
        ("\\sqrt", "sqrt"),
    ];
    for (k, v) in replacements {
        out = out.replace(k, v);
    }

    // Keep variable-heavy formulas readable in plain terminal output.
    out = out.replace('{', "(").replace('}', ")");
    out = out.replace("\\", "");

    out
}

fn normalize_terminal_math(text: &str) -> String {
    text.replace(" <= ", " ≤ ")
        .replace(" >= ", " ≥ ")
        .replace(" != ", " ≠ ")
        .replace(" * ", " × ")
        .replace(" pi", " π")
        .replace("pi ", "π ")
        .replace(" inf", " ∞")
        .replace("inf ", "∞ ")
}

fn format_table_border(left: char, mid: char, right: char, widths: &[usize]) -> String {
    let mut s = String::new();
    s.push(left);
    for (idx, width) in widths.iter().enumerate() {
        s.push_str(&"─".repeat(width + 2));
        if idx + 1 < widths.len() {
            s.push(mid);
        }
    }
    s.push(right);
    s
}

fn strip_html_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        match ch {
            '<' => in_tag = true,
            '>' => in_tag = false,
            _ if !in_tag => result.push(ch),
            _ => {}
        }
    }
    result
}

fn render_output(
    frame: &mut Frame,
    output: &crate::notebook::Output,
    area: ratatui::layout::Rect,
    execution_count: Option<u32>,
) {
    let out_prefix = execution_count
        .map(|n| format!("Out[{}]: ", n))
        .unwrap_or_else(|| "Out: ".to_string());
    let out_pad = " ".repeat(out_prefix.chars().count());

    match output {
        crate::notebook::Output::Stream(stream) => {
            let lines = prefixed_output_lines(
                &stream.text,
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Green),
            );
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
        crate::notebook::Output::Error(err) => {
            let mut text = format!("{}: {}", err.ename, err.evalue);
            if !err.traceback.is_empty() {
                text.push('\n');
                text.push_str(&err.traceback.join("\n"));
            }

            let lines = prefixed_output_lines(
                &text,
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Red),
            );
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
        crate::notebook::Output::ExecuteResult(result) => {
            let text = result
                .data
                .get("text/plain")
                .and_then(|v| v.as_str())
                .unwrap_or("[Execute Result]");
            let lines = prefixed_output_lines(
                text,
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Cyan),
            );
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
        crate::notebook::Output::DisplayData(data) => {
            if let Some((mime, image)) = decode_display_data_image(output) {
                render_display_image_widget(frame, mime, image, area);
                return;
            }

            let text = data
                .data
                .get("text/plain")
                .and_then(|v| v.as_str())
                .unwrap_or("[Display Data]");
            let lines = prefixed_output_lines(
                text,
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Cyan),
            );
            let paragraph = Paragraph::new(lines).wrap(Wrap { trim: false });
            frame.render_widget(paragraph, area);
        }
    }
}

fn prefixed_output_lines(
    text: &str,
    first_prefix: &str,
    continuation_prefix: &str,
    prefix_style: Style,
    text_style: Style,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let mut iter = text.lines();

    if let Some(first) = iter.next() {
        lines.push(Line::from(vec![
            Span::styled(first_prefix.to_string(), prefix_style),
            Span::styled(first.to_string(), text_style),
        ]));
    } else {
        lines.push(Line::from(vec![
            Span::styled(first_prefix.to_string(), prefix_style),
            Span::styled(String::new(), text_style),
        ]));
        return lines;
    }

    for line in iter {
        lines.push(Line::from(vec![
            Span::styled(continuation_prefix.to_string(), prefix_style),
            Span::styled(line.to_string(), text_style),
        ]));
    }

    lines
}

fn output_to_prefixed_lines(
    output: &crate::notebook::Output,
    execution_count: Option<u32>,
) -> Vec<Line<'static>> {
    let out_prefix = execution_count
        .map(|n| format!("Out[{}]: ", n))
        .unwrap_or_else(|| "Out: ".to_string());
    let out_pad = " ".repeat(out_prefix.chars().count());

    match output {
        crate::notebook::Output::Stream(stream) => prefixed_output_lines(
            &sanitize_output_text(&stream.text),
            &out_prefix,
            &out_pad,
            Style::default().fg(Color::Blue),
            Style::default().fg(Color::Green),
        ),
        crate::notebook::Output::Error(err) => {
            let mut text = format!("{}: {}", err.ename, err.evalue);
            if !err.traceback.is_empty() {
                text.push('\n');
                text.push_str(&err.traceback.join("\n"));
            }
            prefixed_output_lines(
                &sanitize_output_text(&text),
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Red),
            )
        }
        crate::notebook::Output::ExecuteResult(result) => {
            let text = result
                .data
                .get("text/plain")
                .and_then(|v| v.as_str())
                .unwrap_or("[Execute Result]");
            prefixed_output_lines(
                &sanitize_output_text(text),
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Cyan),
            )
        }
        crate::notebook::Output::DisplayData(data) => {
            if let Some(image_lines) = display_data_image_lines(data) {
                return prefixed_output_lines_from_rendered_lines(
                    &image_lines,
                    &out_prefix,
                    &out_pad,
                    Style::default().fg(Color::Blue),
                );
            }

            let lines = display_data_text_lines(data);
            prefixed_output_lines_from_vec(
                &lines,
                &out_prefix,
                &out_pad,
                Style::default().fg(Color::Blue),
                Style::default().fg(Color::Cyan),
            )
        }
    }
}

fn output_to_prefixed_text(
    output: &crate::notebook::Output,
    execution_count: Option<u32>,
) -> String {
    let lines = output_to_prefixed_lines(output, execution_count);
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        if idx > 0 {
            out.push('\n');
        }
        for span in &line.spans {
            out.push_str(span.content.as_ref());
        }
    }
    out
}

fn render_completion_popup(frame: &mut Frame, state: &AppState, area: ratatui::layout::Rect) {
    let items: Vec<Line> = state
        .completion_items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == state.completion_selected;
            Line::from(Span::styled(
                item.clone(),
                if is_selected {
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(Color::White)
                },
            ))
        })
        .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let paragraph = Paragraph::new(items).block(block).wrap(Wrap { trim: true });

    frame.render_widget(Clear, area);
    frame.render_widget(paragraph, area);
}
