#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::lsp::{CompletionItem, LspClient};

#[derive(Clone)]
pub struct CompletionProvider {
    lsp_client: LspClient,
    last_code: String,
    last_position: (usize, usize),
    cached_completions: Vec<CompletionItem>,
}

impl CompletionProvider {
    pub fn new() -> Self {
        Self {
            lsp_client: LspClient::new(),
            last_code: String::new(),
            last_position: (0, 0),
            cached_completions: Vec::new(),
        }
    }

    pub async fn get_completions(
        &mut self,
        code: &str,
        line: usize,
        character: usize,
    ) -> &[CompletionItem] {
        let position = (line, character);
        
        if code == self.last_code && position == self.last_position {
            return &self.cached_completions;
        }

        match self.lsp_client.get_completions(code, line, character).await {
            Ok(completions) => {
                self.last_code = code.to_string();
                self.last_position = position;
                self.cached_completions = completions;
                &self.cached_completions
            }
            Err(_) => {
                self.cached_completions.clear();
                &self.cached_completions
            }
        }
    }

    pub fn clear_cache(&mut self) {
        self.last_code.clear();
        self.last_position = (0, 0);
        self.cached_completions.clear();
    }

    pub fn get_cached(&self) -> &[CompletionItem] {
        &self.cached_completions
    }
}

impl Default for CompletionProvider {
    fn default() -> Self {
        Self::new()
    }
}
