#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use serde::{Deserialize, Serialize};

pub struct KernelMessage {
    pub message_type: MessageType,
    pub content: MessageContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageType {
    Execute,
    Complete,
    Inspect,
    Shutdown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageContent {
    Execute {
        code: String,
        silent: bool,
        store_history: bool,
    },
    Complete {
        code: String,
        cursor_pos: usize,
    },
    Inspect {
        code: String,
        cursor_pos: usize,
        detail_level: usize,
    },
    Shutdown {
        restart: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KernelResponse {
    pub status: String,
    pub content: ResponseContent,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResponseContent {
    Execute {
        execution_count: u32,
        payload: Vec<serde_json::Value>,
        user_expressions: serde_json::Value,
    },
    Complete {
        matches: Vec<String>,
        cursor_start: usize,
        cursor_end: usize,
        metadata: serde_json::Value,
    },
    Inspect {
        found: bool,
        data: serde_json::Value,
        metadata: serde_json::Value,
    },
    Error {
        ename: String,
        evalue: String,
        traceback: Vec<String>,
    },
}
