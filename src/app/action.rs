/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
/// 
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
/// 
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.

use crate::editor::Direction;
use crossterm::event::KeyEvent;

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub enum Action {
    Quit,

    InsertChar(char),
    DeleteChar,
    Backspace,

    MoveCursor(Direction),
    MoveCellUp,
    MoveCellDown,
    EnterCell,
    LeaveCell,
    HandleInCellKey(KeyEvent),

    InsertCellAbove,
    InsertCellBelow,
    DeleteCell,
    SetCurrentCellMarkdown,
    SetCurrentCellCode,

    ChangeMode(crate::app::Mode),

    ExecuteCell,
    ExecuteAllCells,
    ExecuteCellsAbove,
    ExecuteCellsBelow,

    ToggleCompletion,
    NextCompletion,
    PreviousCompletion,
    AcceptCompletion,

    CommandInsertChar(char),
    CommandBackspace,
    ExecuteCommand,

    ScrollUp,
    ScrollDown,

    SaveFile,
    OpenFile(String),

    ShowHelp,
    HideHelp,
    ClearStatus,
}
