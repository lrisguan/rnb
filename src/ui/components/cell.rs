#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub fn render_cell_widget(
    _is_code: bool,
    is_current: bool,
    execution_count: Option<u32>,
    content: &str,
) -> Paragraph<'_> {
    let border_style = if is_current {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Gray)
    };

    let prefix = if let Some(count) = execution_count {
        format!("[{}] ", count)
    } else {
        String::new()
    };

    let lines: Vec<Line> = content
        .lines()
        .map(|line| {
            Line::from(vec![
                Span::styled(prefix.clone(), Style::default().fg(Color::Blue)),
                Span::styled(line, Style::default().fg(Color::White)),
            ])
        })
        .collect();

    Paragraph::new(lines).wrap(Wrap { trim: false }).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(border_style),
    )
}
