/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::notebook::{Cell, Notebook, Output};
use anyhow::Result;
use serde_json::json;

pub fn serialize_to_ipynb(notebook: &Notebook) -> Result<String> {
    let cells: Vec<serde_json::Value> = notebook
        .cells
        .iter()
        .map(serialize_cell)
        .collect::<Result<Vec<_>>>()?;

    let mut metadata = json!({
        "language_info": notebook.metadata.language_info,
    });

    if let Some(ref kernelspec) = notebook.metadata.kernelspec {
        metadata["kernelspec"] = json!(kernelspec);
    }

    if let Some(ref title) = notebook.metadata.title {
        metadata["title"] = json!(title);
    }

    let ipynb = json!({
        "cells": cells,
        "metadata": metadata,
        "nbformat": 4,
        "nbformat_minor": 4,
    });

    Ok(serde_json::to_string_pretty(&ipynb)?)
}

fn serialize_cell(cell: &Cell) -> Result<serde_json::Value> {
    match cell {
        Cell::Code(code_cell) => {
            let source = code_cell.source.to_string();
            let outputs: Vec<serde_json::Value> = code_cell
                .outputs
                .iter()
                .map(serialize_output)
                .collect::<Result<Vec<_>>>()?;

            Ok(json!({
                "cell_type": "code",
                "execution_count": code_cell.execution_count,
                "metadata": {},
                "outputs": outputs,
                "source": source,
            }))
        }
        Cell::Markdown(markdown_cell) => Ok(json!({
            "cell_type": "markdown",
            "metadata": {},
            "source": markdown_cell.source,
        })),
    }
}

fn serialize_output(output: &Output) -> Result<serde_json::Value> {
    match output {
        Output::Stream(stream) => Ok(json!({
            "name": stream.name,
            "output_type": "stream",
            "text": stream.text,
        })),
        Output::ExecuteResult(result) => Ok(json!({
            "data": result.data,
            "execution_count": null,
            "metadata": result.metadata,
            "output_type": "execute_result",
        })),
        Output::DisplayData(data) => Ok(json!({
            "data": data.data,
            "metadata": data.metadata,
            "output_type": "display_data",
        })),
        Output::Error(err) => Ok(json!({
            "ename": err.ename,
            "evalue": err.evalue,
            "output_type": "error",
            "traceback": err.traceback,
        })),
    }
}
