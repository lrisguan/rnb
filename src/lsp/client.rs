#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::process::Stdio;
use tokio::io::AsyncReadExt;
use tokio::process::Command as AsyncCommand;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionItem {
    pub label: String,
    pub kind: Option<String>,
    pub detail: Option<String>,
    pub documentation: Option<String>,
}

#[derive(Clone)]
pub struct LspClient {
    language_server_path: String,
}

impl LspClient {
    pub fn new() -> Self {
        Self {
            language_server_path: "pylsp".to_string(),
        }
    }

    pub fn with_server(path: &str) -> Self {
        Self {
            language_server_path: path.to_string(),
        }
    }

    pub async fn get_completions(
        &self,
        code: &str,
        line: usize,
        character: usize,
    ) -> Result<Vec<CompletionItem>> {
        let code_json = serde_json::to_string(code)?;
        let safe_line = line.max(1);

        let python_code = format!(
            r#"
import json
import re

code = json.loads({})
line = {}
character = {}

def fallback_completion(src: str, line_no: int, col: int):
    lines = src.splitlines() or [""]
    current_idx = max(0, min(line_no - 1, len(lines) - 1))
    current_line = lines[current_idx]
    col = max(0, min(col, len(current_line)))
    prefix_match = re.search(r'[a-zA-Z_][a-zA-Z0-9_]*$', current_line[:col])
    if not prefix_match:
        return []

    prefix = prefix_match.group()
    variables = set()
    for src_line in lines[: current_idx + 1]:
        m = re.match(r'\s*([a-zA-Z_][a-zA-Z0-9_]*)\s*=', src_line)
        if m:
            variables.add(m.group(1))
    return [{{"label": v, "kind": "Variable"}} for v in sorted(variables) if v.startswith(prefix)]

try:
    import jedi
    script = jedi.Script(code=code)
    completions = script.complete(line=line, column=character)
    result = []
    for c in completions:
        result.append({{
            "label": c.name,
            "kind": c.type,
            "detail": c.description,
        }})
    print(json.dumps(result))
except Exception:
    print(json.dumps(fallback_completion(code, line, character)))
"#,
            code_json, safe_line, character,
        );

        let mut child = AsyncCommand::new("python3")
            .arg("-c")
            .arg(&python_code)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut output = String::new();
        if let Some(mut stdout) = child.stdout.take() {
            stdout.read_to_string(&mut output).await?;
        }

        let _ = child.wait().await;

        if let Ok(items) = serde_json::from_str::<Vec<serde_json::Value>>(&output) {
            Ok(items
                .into_iter()
                .filter_map(|v| {
                    let label = v.get("label").and_then(|s| s.as_str())?;
                    let kind = v
                        .get("kind")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    let detail = v
                        .get("detail")
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string());
                    Some(CompletionItem {
                        label: label.to_string(),
                        kind,
                        detail,
                        documentation: None,
                    })
                })
                .collect())
        } else {
            Ok(Vec::new())
        }
    }
}

impl Default for LspClient {
    fn default() -> Self {
        Self::new()
    }
}
