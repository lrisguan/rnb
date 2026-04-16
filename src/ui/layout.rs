/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use ratatui::layout::{Constraint, Direction, Layout, Rect};

pub struct AppLayout {
    pub main_area: Rect,
    pub status_bar: Rect,
    pub command_line: Rect,
    pub completion_popup: Rect,
}

impl AppLayout {
    pub fn calculate(area: Rect, show_command: bool, show_popup: bool) -> Self {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Min(0),
                Constraint::Length(if show_command { 2 } else { 1 }),
            ])
            .split(area);

        let main_area = chunks[0];
        let status_bar = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };

        let command_line = if show_command {
            Rect {
                x: area.x,
                y: area.y + area.height - 2,
                width: area.width,
                height: 1,
            }
        } else {
            Rect::default()
        };

        let completion_popup = if show_popup {
            let popup_width = area.width.saturating_mul(3).saturating_div(5).clamp(36, 96);
            let popup_height = area.height.saturating_mul(3).saturating_div(5).clamp(8, 20);
            let width = popup_width.min(main_area.width);
            let height = popup_height.min(main_area.height);
            Rect {
                x: main_area.x + (main_area.width.saturating_sub(width)) / 2,
                y: main_area.y + (main_area.height.saturating_sub(height)) / 2,
                width,
                height,
            }
        } else {
            Rect::default()
        };

        Self {
            main_area,
            status_bar,
            command_line,
            completion_popup,
        }
    }
}
