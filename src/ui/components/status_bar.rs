#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::app::Mode;
use ratatui::{
    layout::Alignment,
    style::{Color, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

pub fn render_status_bar<'a>(
    mode: Mode,
    status_message: &'a Option<String>,
    cell_info: &'a str,
) -> Paragraph<'a> {
    let mode_color = match mode {
        Mode::Normal => Color::Green,
        Mode::Insert => Color::Cyan,
        Mode::Command => Color::Yellow,
    };

    let mode_text = format!("{:?}", mode);

    let spans = vec![
        Span::styled(mode_text, Style::default().fg(Color::Black).bg(mode_color)),
        Span::raw(" "),
        Span::styled(cell_info, Style::default().fg(Color::White)),
        Span::raw(" "),
        Span::styled(
            status_message.as_deref().unwrap_or(""),
            Style::default().fg(Color::Gray),
        ),
    ];

    Paragraph::new(Line::from(spans))
        .alignment(Alignment::Left)
        .style(Style::default().bg(Color::DarkGray))
}
