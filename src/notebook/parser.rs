#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
///
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
///
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::notebook::{
    Cell, CodeCell, DisplayData, ErrorOutput, ExecuteResult, MarkdownCell, Metadata, Notebook,
    Output, StreamOutput,
};

use anyhow::Result;
use serde::Deserialize;
use serde_json::Value;

#[derive(Debug, Deserialize)]
pub struct Ipynb {
    #[serde(default)]
    pub metadata: serde_json::Value,
    pub cells: Vec<IpynbCell>,
    #[serde(default)]
    pub nbformat: u32,
    #[serde(default)]
    pub nbformat_minor: u32,
}

#[derive(Debug, Deserialize)]
pub struct IpynbCell {
    pub cell_type: String,
    #[serde(default)]
    pub source: IpynbSource,
    #[serde(default)]
    pub outputs: Vec<serde_json::Value>,
    #[serde(default)]
    pub execution_count: Option<u32>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug)]
pub enum IpynbSource {
    String(String),
    Array(Vec<String>),
}

impl Default for IpynbSource {
    fn default() -> Self {
        IpynbSource::String(String::new())
    }
}

impl<'de> Deserialize<'de> for IpynbSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = Value::deserialize(deserializer)?;
        match value {
            Value::String(s) => Ok(IpynbSource::String(s)),
            Value::Array(arr) => {
                let strings: Result<Vec<String>, _> = arr
                    .into_iter()
                    .map(|v| match v {
                        Value::String(s) => Ok(s),
                        _ => Err(serde::de::Error::custom("Expected string in array")),
                    })
                    .collect();
                Ok(IpynbSource::Array(strings?))
            }
            _ => Err(serde::de::Error::custom("Expected string or array")),
        }
    }
}

impl IpynbSource {
    fn to_string(&self) -> String {
        match self {
            IpynbSource::String(s) => s.clone(),
            IpynbSource::Array(arr) => arr.join(""),
        }
    }
}

pub fn parse_ipynb(json: &str) -> Result<Notebook> {
    let ipynb: Ipynb = serde_json::from_str(json)?;
    let cells = ipynb
        .cells
        .into_iter()
        .map(parse_cell)
        .collect::<Result<Vec<_>>>()?;
    let metadata = parse_metadata(&ipynb.metadata)?;
    Ok(Notebook { cells, metadata })
}

fn parse_cell(cell: IpynbCell) -> Result<Cell> {
    match cell.cell_type.as_str() {
        "code" => {
            let source = cell.source.to_string();
            let outputs = cell
                .outputs
                .into_iter()
                .map(parse_output)
                .collect::<Result<Vec<_>>>()?;
            Ok(Cell::Code(CodeCell {
                source: ropey::Rope::from_str(&source),
                outputs,
                execution_count: cell.execution_count,
            }))
        }
        "markdown" => {
            let source = cell.source.to_string();
            Ok(Cell::Markdown(MarkdownCell { source }))
        }
        other => Err(anyhow::anyhow!("Unknown cell type: {}", other)),
    }
}

fn parse_output(output: serde_json::Value) -> Result<Output> {
    let output_type = output["output_type"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing output_type"))?;

    match output_type {
        "stream" => {
            let name = output["name"].as_str().unwrap_or("stdout").to_string();
            let text = extract_text(&output["text"]);
            Ok(Output::Stream(StreamOutput { name, text }))
        }
        "execute_result" => {
            let data = output["data"].clone();
            let metadata = output["metadata"].clone();
            Ok(Output::ExecuteResult(ExecuteResult { data, metadata }))
        }
        "display_data" => {
            let data = output["data"].clone();
            let metadata = output["metadata"].clone();
            Ok(Output::DisplayData(DisplayData { data, metadata }))
        }
        "error" => {
            let ename = output["ename"].as_str().unwrap_or("").to_string();
            let evalue = output["evalue"].as_str().unwrap_or("").to_string();
            let traceback = extract_traceback(&output["traceback"]);
            Ok(Output::Error(ErrorOutput {
                ename,
                evalue,
                traceback,
            }))
        }
        other => Err(anyhow::anyhow!("Unknown output type: {}", other)),
    }
}

fn extract_text(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(s) => s.clone(),
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn extract_traceback(value: &serde_json::Value) -> Vec<String> {
    match value {
        serde_json::Value::Array(arr) => arr
            .iter()
            .filter_map(|v| v.as_str())
            .map(|s| s.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn parse_metadata(value: &serde_json::Value) -> Result<Metadata> {
    let language_info = parse_language_info(&value["language_info"])?;
    let kernelspec = value["kernelspec"]
        .as_object()
        .map(|ks| {
            Ok::<_, anyhow::Error>(super::model::KernelSpec {
                name: ks["name"].as_str().unwrap_or("python3").to_string(),
                language: ks["language"].as_str().unwrap_or("python").to_string(),
                display_name: ks["display_name"]
                    .as_str()
                    .unwrap_or("Python 3")
                    .to_string(),
            })
        })
        .transpose()?;
    let title = value["title"].as_str().map(|s| s.to_string());

    Ok(Metadata {
        language_info,
        kernelspec,
        title,
    })
}

fn parse_language_info(value: &serde_json::Value) -> Result<super::model::LanguageInfo> {
    Ok(super::model::LanguageInfo {
        name: value["name"].as_str().unwrap_or("python").to_string(),
    })
}
