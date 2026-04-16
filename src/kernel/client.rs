/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::kernel::{KernelExecutor, KernelOutput};

#[derive(Clone)]
pub struct KernelClient {
    executor: KernelExecutor,
}

impl KernelClient {
    pub fn new() -> Self {
        Self {
            executor: KernelExecutor::new(),
        }
    }

    pub fn with_python(python_path: &str) -> Self {
        Self {
            executor: KernelExecutor::with_python(python_path),
        }
    }

    pub async fn execute(&self, code: &str) -> Result<KernelOutput, anyhow::Error> {
        self.executor.execute(code).await
    }
}

impl Default for KernelClient {
    fn default() -> Self {
        Self::new()
    }
}
