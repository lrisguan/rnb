#![allow(dead_code)]

/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crossterm::event::{self, poll, Event};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread;
use std::time::Duration;
use tokio::sync::mpsc;

pub type EventSender = mpsc::UnboundedSender<Event>;
pub type EventReceiver = mpsc::UnboundedReceiver<Event>;

pub fn spawn_event_loop(tx: EventSender) {
    tokio::spawn(async move {
        let tick_rate = Duration::from_millis(16); 
        
        loop {
            if poll(tick_rate).unwrap_or(false) {
                if let Ok(event) = event::read() {
                    let _ = tx.send(event);
                }
            }
        }
    });
}

pub struct EventDispatcher {
    tx: EventSender,
    pub rx: EventReceiver,
    running: Arc<AtomicBool>,
    handle: Option<thread::JoinHandle<()>>,
}

impl EventDispatcher {
    pub fn new() -> Self {
        let (tx, rx) = mpsc::unbounded_channel();
        Self {
            tx,
            rx,
            running: Arc::new(AtomicBool::new(false)),
            handle: None,
        }
    }

    pub fn start(&mut self) {
        if self.handle.is_none() {
            let tx = self.tx.clone();
            let running = Arc::clone(&self.running);
            running.store(true, Ordering::SeqCst);
            self.handle = Some(thread::spawn(move || {
                let tick_rate = Duration::from_millis(16);
                while running.load(Ordering::SeqCst) {
                    if poll(tick_rate).unwrap_or(false) {
                        if let Ok(ev) = event::read() {
                            let _ = tx.send(ev);
                        }
                    }
                }
            }));
        }
    }

    pub fn stop(&mut self) {
        self.running.store(false, Ordering::SeqCst);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }

    pub async fn next(&mut self) -> Option<Event> {
        self.rx.recv().await
    }

    pub fn try_next(&mut self) -> Option<Event> {
        self.rx.try_recv().ok()
    }
}

impl Default for EventDispatcher {
    fn default() -> Self {
        Self::new()
    }
}
