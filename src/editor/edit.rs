#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::editor::{Buffer, Cursor, Direction};

pub struct Editor {
    buffer: Buffer,
    cursor: Cursor,
}

impl Editor {
    pub fn new() -> Self {
        Self {
            buffer: Buffer::new(),
            cursor: Cursor::new(0, 0),
        }
    }

    pub fn from_str(text: &str) -> Self {
        let buffer = Buffer::from_str(text);
        let cursor = Cursor::new(0, 0);
        Self { buffer, cursor }
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn cursor(&self) -> Cursor {
        self.cursor
    }

    pub fn set_cursor(&mut self, cursor: Cursor) {
        self.cursor = cursor;
    }

    pub fn insert_char(&mut self, ch: char) {
        let pos = self.cursor_to_pos();
        self.buffer.insert_char(pos, ch);
        self.move_cursor(Direction::Right);
    }

    pub fn insert_text(&mut self, text: &str) {
        let pos = self.cursor_to_pos();
        self.buffer.insert_text(pos, text);
    }

    pub fn delete_char(&mut self) {
        let pos = self.cursor_to_pos();
        if pos < self.buffer.len_chars() {
            self.buffer.delete(pos..pos + 1);
        }
    }

    pub fn backspace(&mut self) {
        let pos = self.cursor_to_pos();
        if pos > 0 {
            self.buffer.delete(pos - 1..pos);
            self.move_cursor(Direction::Left);
        }
    }

    pub fn move_cursor(&mut self, direction: Direction) {
        match direction {
            Direction::Up => {
                self.cursor.move_up();
                self.clamp_cursor();
            }
            Direction::Down => {
                let max_row = self.buffer.len_lines().saturating_sub(1);
                self.cursor.move_down(max_row);
                self.clamp_cursor();
            }
            Direction::Left => {
                self.cursor.move_left();
                self.clamp_cursor();
            }
            Direction::Right => {
                let max_col = self.current_line_len();
                self.cursor.move_right(max_col);
                self.clamp_cursor();
            }
        }
    }

    fn current_line_len(&self) -> usize {
        if self.cursor.row < self.buffer.len_lines() {
            self.buffer.line(self.cursor.row).chars().count()
        } else {
            0
        }
    }

    fn clamp_cursor(&mut self) {
        let max_row = self.buffer.len_lines().saturating_sub(1);
        if self.cursor.row > max_row {
            self.cursor.row = max_row;
        }

        let max_col = self.current_line_len();
        if self.cursor.col > max_col {
            self.cursor.col = max_col;
        }
    }

    pub fn cursor_to_pos(&self) -> usize {
        let mut pos = 0;
        for i in 0..self.cursor.row {
            pos += self.buffer.line(i).chars().count() + 1;
        }
        pos += self.cursor.col;
        pos
    }

    pub fn pos_to_cursor(&self, mut pos: usize) -> Cursor {
        let mut row = 0;
        loop {
            let line_len = self.buffer.line(row).chars().count();
            if pos <= line_len {
                return Cursor::new(row, pos);
            }
            pos -= line_len + 1;
            row += 1;
            if row >= self.buffer.len_lines() {
                return Cursor::new(self.buffer.len_lines().saturating_sub(1), 0);
            }
        }
    }

    pub fn to_string(&self) -> String {
        self.buffer.to_string()
    }
}

impl Default for Editor {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Editor {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            cursor: self.cursor,
        }
    }
}
