/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;

pub static LOGGER: Mutex<Option<Logger>> = Mutex::new(None);

pub struct Logger {
    file: std::fs::File,
}

impl Logger {
    pub fn init(path: &str) -> std::io::Result<()> {
        let file = OpenOptions::new().create(true).append(true).open(path)?;

        let mut guard = LOGGER.lock().unwrap();
        *guard = Some(Logger { file });
        Ok(())
    }

    pub fn log(&mut self, message: &str) {
        let _ = writeln!(self.file, "{}", message);
    }
}

pub fn log(message: &str) {
    if let Some(ref mut logger) = *LOGGER.lock().unwrap() {
        logger.log(message);
    }
}

pub fn log_error(message: &str) {
    log(&format!("[ERROR] {}", message));
}

pub fn log_info(message: &str) {
    log(&format!("[INFO] {}", message));
}

pub fn log_debug(message: &str) {
    log(&format!("[DEBUG] {}", message));
}
