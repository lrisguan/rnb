#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cursor {
    pub row: usize,
    pub col: usize,
}

impl Cursor {
    pub fn new(row: usize, col: usize) -> Self {
        Self { row, col }
    }

    pub fn at(row: usize, col: usize) -> Self {
        Self::new(row, col)
    }

    pub fn move_up(&mut self) {
        if self.row > 0 {
            self.row -= 1;
        }
    }

    pub fn move_down(&mut self, max_row: usize) {
        if self.row < max_row {
            self.row += 1;
        }
    }

    pub fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        }
    }

    pub fn move_right(&mut self, max_col: usize) {
        if self.col < max_col {
            self.col += 1;
        }
    }

    pub fn set_row(&mut self, row: usize) {
        self.row = row;
    }

    pub fn set_col(&mut self, col: usize) {
        self.col = col;
    }

    pub fn reset(&mut self) {
        self.row = 0;
        self.col = 0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Direction {
    Up,
    Down,
    Left,
    Right,
}
