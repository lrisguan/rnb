/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

pub mod dispatcher;
pub mod input;

#[allow(unused_imports)]
pub use dispatcher::{spawn_event_loop, EventDispatcher, EventReceiver, EventSender};
#[allow(unused_imports)]
pub use input::event_to_action;
