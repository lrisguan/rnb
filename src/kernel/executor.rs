/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use serde_json::json;
use std::process::Stdio;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdin, ChildStdout, Command as AsyncCommand};
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct KernelExecutor {
    python_path: String,
    session: Arc<Mutex<Option<KernelSession>>>,
}

struct KernelSession {
    child: Child,
    stdin: ChildStdin,
    stdout_lines: Lines<BufReader<ChildStdout>>,
}

#[derive(Debug, Deserialize)]
struct PythonKernelResponse {
    ok: bool,
    execution_count: Option<u32>,
    #[serde(default)]
    stdout: String,
    #[serde(default)]
    stderr: String,
    #[serde(default)]
    result: String,
    #[serde(default)]
    ename: String,
    #[serde(default)]
    evalue: String,
    #[serde(default)]
    traceback: Vec<String>,
    #[serde(default)]
    message: String,
}

impl KernelExecutor {
    pub fn new() -> Self {
        Self {
            python_path: "python3".to_string(),
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_python(python_path: &str) -> Self {
        Self {
            python_path: python_path.to_string(),
            session: Arc::new(Mutex::new(None)),
        }
    }

    pub async fn execute(&self, code: &str) -> Result<KernelOutput> {
        let mut guard = self.session.lock().await;

        if guard.is_none() {
            *guard = Some(self.start_session().await?);
        }

        let session = guard
            .as_mut()
            .ok_or_else(|| anyhow!("failed to initialize kernel session"))?;

        let request_line = json!({
            "cmd": "execute",
            "code": code,
        })
        .to_string();

        if let Err(e) = session
            .stdin
            .write_all(format!("{}\n", request_line).as_bytes())
            .await
        {
            *guard = None;
            return Err(anyhow!("failed to write execute request: {e}"));
        }

        if let Err(e) = session.stdin.flush().await {
            *guard = None;
            return Err(anyhow!("failed to flush execute request: {e}"));
        }

        let response_line = match session.stdout_lines.next_line().await {
            Ok(Some(line)) => line,
            Ok(None) => {
                let status = session
                    .child
                    .try_wait()
                    .ok()
                    .flatten()
                    .map(|s| s.to_string())
                    .unwrap_or_else(|| "unknown status".to_string());
                *guard = None;
                return Err(anyhow!("kernel helper terminated unexpectedly ({status})"));
            }
            Err(e) => {
                *guard = None;
                return Err(anyhow!("failed to read kernel response: {e}"));
            }
        };

        let response: PythonKernelResponse = match serde_json::from_str(&response_line) {
            Ok(v) => v,
            Err(e) => {
                *guard = None;
                return Err(anyhow!(
                    "invalid kernel response: {e}; raw={}",
                    response_line
                ));
            }
        };

        if response.ok {
            let mut parts = Vec::new();
            if !response.stdout.trim().is_empty() {
                parts.push(response.stdout);
            }
            if !response.result.trim().is_empty() {
                parts.push(response.result);
            }
            if !response.stderr.trim().is_empty() {
                parts.push(format!("[stderr]\n{}", response.stderr));
            }

            Ok(KernelOutput::Success {
                output: parts.join("\n"),
                execution_count: response.execution_count,
            })
        } else {
            let mut parts = Vec::new();
            if !response.traceback.is_empty() {
                parts.push(response.traceback.join("\n"));
            } else if !response.ename.is_empty() || !response.evalue.is_empty() {
                parts.push(format!("{}: {}", response.ename, response.evalue));
            }

            if !response.stdout.trim().is_empty() {
                parts.push(format!("[stdout]\n{}", response.stdout));
            }
            if !response.stderr.trim().is_empty() {
                parts.push(format!("[stderr]\n{}", response.stderr));
            }
            if !response.message.trim().is_empty() {
                parts.push(response.message);
            }

            Ok(KernelOutput::Error {
                error: parts.join("\n"),
                execution_count: response.execution_count,
                traceback: response.traceback,
            })
        }
    }

    async fn start_session(&self) -> Result<KernelSession> {
        let helper_code = r#"
import atexit
import contextlib
import io
import json
import sys
import traceback


def _as_text(v):
    if isinstance(v, list):
        return "".join(str(x) for x in v)
    if v is None:
        return ""
    return str(v)

use_jupyter = False
km = None
client = None
fallback_ns = {}
fallback_count = 0

try:
    from jupyter_client import KernelManager

    km = KernelManager(kernel_name="python3")
    km.start_kernel()
    client = km.client()
    client.start_channels()
    client.wait_for_ready(timeout=60)
    use_jupyter = True
except Exception:
    # Fall back to stateful in-process exec mode when Jupyter kernel cannot start.
    use_jupyter = False


def _shutdown_kernel():
    if client is not None:
        try:
            client.stop_channels()
        except Exception:
            pass
    if km is not None:
        try:
            km.shutdown_kernel(now=True)
        except Exception:
            pass


atexit.register(_shutdown_kernel)


def _shell_reply_for(msg_id, timeout=120):
    while True:
        msg = client.get_shell_msg(timeout=timeout)
        parent_id = msg.get("parent_header", {}).get("msg_id")
        if parent_id == msg_id:
            return msg


def _execute_fallback(code):
    global fallback_count
    fallback_count += 1

    stdout_buf = io.StringIO()
    stderr_buf = io.StringIO()
    try:
        with contextlib.redirect_stdout(stdout_buf), contextlib.redirect_stderr(stderr_buf):
            exec(code, fallback_ns, fallback_ns)
        return {
            "ok": True,
            "execution_count": fallback_count,
            "stdout": stdout_buf.getvalue(),
            "stderr": stderr_buf.getvalue(),
            "result": "",
        }
    except Exception as e:
        tb_lines = traceback.format_exception(type(e), e, e.__traceback__)
        return {
            "ok": False,
            "execution_count": fallback_count,
            "stdout": stdout_buf.getvalue(),
            "stderr": stderr_buf.getvalue(),
            "result": "",
            "ename": type(e).__name__,
            "evalue": str(e),
            "traceback": [line.rstrip("\n") for line in tb_lines],
        }


for raw in sys.stdin:
    raw = raw.strip()
    if not raw:
        continue

    try:
        req = json.loads(raw)
        cmd = req.get("cmd")

        if cmd == "shutdown":
            print(json.dumps({"ok": True}), flush=True)
            break

        if cmd != "execute":
            print(json.dumps({"ok": False, "message": f"unknown command: {cmd}"}), flush=True)
            continue

        code = req.get("code", "")
        if not use_jupyter:
            print(json.dumps(_execute_fallback(code)), flush=True)
            continue

        msg_id = client.execute(
            code=code,
            store_history=True,
            allow_stdin=False,
            stop_on_error=False,
        )

        stdout_parts = []
        stderr_parts = []
        result_parts = []
        ename = ""
        evalue = ""
        traceback_lines = []

        while True:
            msg = client.get_iopub_msg(timeout=120)
            parent_id = msg.get("parent_header", {}).get("msg_id")
            if parent_id != msg_id:
                continue

            mtype = msg.get("header", {}).get("msg_type")
            content = msg.get("content", {})

            if mtype == "status" and content.get("execution_state") == "idle":
                break

            if mtype == "stream":
                text = _as_text(content.get("text", ""))
                if content.get("name") == "stderr":
                    stderr_parts.append(text)
                else:
                    stdout_parts.append(text)
            elif mtype in ("execute_result", "display_data"):
                data = content.get("data", {})
                result_parts.append(_as_text(data.get("text/plain", "")))
            elif mtype == "error":
                ename = _as_text(content.get("ename", ""))
                evalue = _as_text(content.get("evalue", ""))
                tb = content.get("traceback", [])
                traceback_lines = [str(x) for x in tb]

        shell = _shell_reply_for(msg_id, timeout=120)
        shell_content = shell.get("content", {})
        status = shell_content.get("status", "ok")
        execution_count = shell_content.get("execution_count")

        if status == "ok" and not ename:
            print(
                json.dumps(
                    {
                        "ok": True,
                        "execution_count": execution_count,
                        "stdout": "".join(stdout_parts),
                        "stderr": "".join(stderr_parts),
                        "result": "\n".join([p for p in result_parts if p]),
                    }
                ),
                flush=True,
            )
        else:
            print(
                json.dumps(
                    {
                        "ok": False,
                        "execution_count": execution_count,
                        "stdout": "".join(stdout_parts),
                        "stderr": "".join(stderr_parts),
                        "result": "\n".join([p for p in result_parts if p]),
                        "ename": ename,
                        "evalue": evalue,
                        "traceback": traceback_lines,
                    }
                ),
                flush=True,
            )

    except Exception:
        print(
            json.dumps(
                {
                    "ok": False,
                    "message": "".join(traceback.format_exc()),
                    "traceback": traceback.format_exc().splitlines(),
                }
            ),
            flush=True,
        )
"#;

        let mut child = AsyncCommand::new(&self.python_path)
            .arg("-s")
            .arg("-u")
            .arg("-c")
            .arg(helper_code)
            .env("PYTHONNOUSERSITE", "1")
            .env_remove("PYTHONPATH")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| anyhow!("failed to open kernel helper stdin"))?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| anyhow!("failed to open kernel helper stdout"))?;

        Ok(KernelSession {
            child,
            stdin,
            stdout_lines: BufReader::new(stdout).lines(),
        })
    }
}

impl Default for KernelExecutor {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone)]
pub enum KernelOutput {
    Success {
        output: String,
        execution_count: Option<u32>,
    },
    Error {
        error: String,
        execution_count: Option<u32>,
        traceback: Vec<String>,
    },
}

impl KernelOutput {
    #[allow(dead_code)]
    pub fn is_success(&self) -> bool {
        matches!(self, KernelOutput::Success { .. })
    }

    #[allow(dead_code)]
    pub fn get_output(&self) -> Option<&str> {
        match self {
            KernelOutput::Success { output, .. } => Some(output),
            KernelOutput::Error { .. } => None,
        }
    }

    #[allow(dead_code)]
    pub fn get_error(&self) -> Option<&str> {
        match self {
            KernelOutput::Error { error, .. } => Some(error),
            KernelOutput::Success { .. } => None,
        }
    }
}
