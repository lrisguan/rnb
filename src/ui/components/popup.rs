/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
};

pub struct PopupItem {
    pub label: String,
    pub detail: Option<String>,
}

pub fn render_completion_popup<'a>(
    items: &'a [PopupItem],
    selected_index: usize,
    title: &'a str,
) -> Paragraph<'a> {
    let lines: Vec<Line> = items
        .iter()
        .enumerate()
        .map(|(i, item)| {
            let is_selected = i == selected_index;
            let base_style = if is_selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let mut spans = vec![Span::styled(&item.label, base_style)];

            if let Some(detail) = &item.detail {
                spans.push(Span::styled(
                    format!(" - {}", detail),
                    Style::default().fg(Color::DarkGray),
                ));
            }

            Line::from(spans)
        })
        .collect();

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    Paragraph::new(lines).block(block).wrap(Wrap { trim: true })
}

pub struct HelpItem {
    pub key: String,
    pub description: String,
}

pub fn render_help_popup(items: &[HelpItem]) -> Paragraph<'_> {
    let lines: Vec<Line> = items
        .iter()
        .map(|item| {
            let padded_key = format!("{:<18}", item.key);
            Line::from(vec![
                Span::styled(
                    padded_key,
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::BOLD),
                ),
                Span::styled("  ", Style::default().fg(Color::DarkGray)),
                Span::styled(&item.description, Style::default().fg(Color::Black)),
            ])
        })
        .collect();

    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Blue))
        .style(Style::default().bg(Color::White));

    Paragraph::new(lines)
        .block(block)
        .style(Style::default().bg(Color::White).fg(Color::Black))
        .wrap(Wrap { trim: false })
}
