#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use ropey::Rope;

pub struct Buffer {
    rope: Rope,
}

impl Buffer {
    pub fn new() -> Self {
        Self { rope: Rope::new() }
    }

    pub fn from_str(text: &str) -> Self {
        Self {
            rope: Rope::from_str(text),
        }
    }

    pub fn from_rope(rope: Rope) -> Self {
        Self { rope }
    }

    pub fn insert_char(&mut self, pos: usize, ch: char) {
        self.rope.insert_char(pos, ch);
    }

    pub fn insert_text(&mut self, pos: usize, text: &str) {
        self.rope.insert(pos, text);
    }

    pub fn delete(&mut self, range: std::ops::Range<usize>) {
        if range.start < self.rope.len_chars() && range.end <= self.rope.len_chars() {
            self.rope.remove(range);
        }
    }

    pub fn len_chars(&self) -> usize {
        self.rope.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.rope.len_lines()
    }

    pub fn is_empty(&self) -> bool {
        self.rope.len_chars() == 0
    }

    pub fn to_string(&self) -> String {
        self.rope.to_string()
    }

    pub fn line(&self, line_idx: usize) -> String {
        self.rope.line(line_idx).to_string()
    }

    pub fn char(&self, pos: usize) -> Option<char> {
        if pos < self.rope.len_chars() {
            Some(self.rope.char(pos))
        } else {
            None
        }
    }

    pub fn rope(&self) -> &Rope {
        &self.rope
    }

    pub fn rope_mut(&mut self) -> &mut Rope {
        &mut self.rope
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for Buffer {
    fn clone(&self) -> Self {
        Self {
            rope: self.rope.clone(),
        }
    }
}
