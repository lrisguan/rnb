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
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone)]
pub struct Notebook {
    pub cells: Vec<Cell>,
    pub metadata: Metadata,
}

#[derive(Debug, Clone)]
pub enum Cell {
    Code(CodeCell),
    Markdown(MarkdownCell),
}

#[derive(Clone, Debug)]
pub struct CodeCell {
    pub source: Rope,
    pub outputs: Vec<Output>,
    pub execution_count: Option<u32>,
}

#[derive(Clone, Debug)]
pub struct MarkdownCell {
    pub source: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Metadata {
    pub language_info: LanguageInfo,
    #[serde(default)]
    pub kernelspec: Option<KernelSpec>,
    #[serde(default)]
    pub title: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LanguageInfo {
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelSpec {
    pub name: String,
    pub language: String,
    pub display_name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Output {
    Stream(StreamOutput),
    ExecuteResult(ExecuteResult),
    DisplayData(DisplayData),
    Error(ErrorOutput),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StreamOutput {
    pub name: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ExecuteResult {
    pub data: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DisplayData {
    pub data: serde_json::Value,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ErrorOutput {
    pub ename: String,
    pub evalue: String,
    pub traceback: Vec<String>,
}

impl Notebook {
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            metadata: Metadata {
                language_info: LanguageInfo {
                    name: "python".to_string(),
                },
                kernelspec: None,
                title: None,
            },
        }
    }

    pub fn add_cell(&mut self, cell: Cell) {
        self.cells.push(cell);
    }

    pub fn insert_cell(&mut self, index: usize, cell: Cell) {
        self.cells.insert(index, cell);
    }

    pub fn remove_cell(&mut self, index: usize) -> Option<Cell> {
        if index < self.cells.len() {
            Some(self.cells.remove(index))
        } else {
            None
        }
    }

    pub fn get_cell(&self, index: usize) -> Option<&Cell> {
        self.cells.get(index)
    }

    pub fn get_cell_mut(&mut self, index: usize) -> Option<&mut Cell> {
        self.cells.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.cells.len()
    }

    pub fn is_empty(&self) -> bool {
        self.cells.is_empty()
    }
}

impl CodeCell {
    pub fn new() -> Self {
        Self {
            source: Rope::new(),
            outputs: Vec::new(),
            execution_count: None,
        }
    }

    pub fn with_source(source: &str) -> Self {
        Self {
            source: Rope::from_str(source),
            outputs: Vec::new(),
            execution_count: None,
        }
    }

    pub fn get_source(&self) -> String {
        self.source.to_string()
    }
}

impl MarkdownCell {
    pub fn new() -> Self {
        Self {
            source: String::new(),
        }
    }

    pub fn with_source(source: &str) -> Self {
        Self {
            source: source.to_string(),
        }
    }
}
