/// rnb
/// Copyright (C) 2026 lrisguan <lrisguan@outlook.com>
///
/// This program is released under the terms of the GNU General Public License version 2(GPLv2).
/// See https://opensource.org/licenses/GPL-2.0 for more information.
///
/// Project homepage: https://github.com/lrisguan/rnb
/// Description: A terminal-first Notebook editor and runner written in Rust.
use crate::app::Action;
use crossterm::event::{
    Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, MouseEventKind,
};

pub fn event_to_action(event: Event, mode: crate::app::Mode, in_cell_mode: bool) -> Option<Action> {
    match event {
        Event::Key(key_event) => key_event_to_action(key_event, mode, in_cell_mode),
        Event::Mouse(mouse_event) => match mouse_event.kind {
            MouseEventKind::ScrollUp => Some(Action::ScrollUp),
            MouseEventKind::ScrollDown => Some(Action::ScrollDown),
            _ => None,
        },
        Event::Resize(_, _) => None,
        _ => None,
    }
}

fn key_event_to_action(
    key: KeyEvent,
    mode: crate::app::Mode,
    in_cell_mode: bool,
) -> Option<Action> {
    // Many terminals emit both Press and Release for one physical key stroke.
    // Ignore Release to avoid handling the same key twice.
    if key.kind == KeyEventKind::Release {
        return None;
    }

    if in_cell_mode && mode != crate::app::Mode::Command {
        return handle_in_cell_mode(key, mode);
    }

    match mode {
        crate::app::Mode::Normal => handle_normal_mode(key, in_cell_mode),
        crate::app::Mode::Insert => handle_insert_mode(key),
        crate::app::Mode::Command => handle_command_mode(key),
    }
}

fn handle_in_cell_mode(key: KeyEvent, mode: crate::app::Mode) -> Option<Action> {
    if key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
    {
        return Some(Action::ExecuteCell);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('b') => return Some(Action::ScrollUp),
            KeyCode::Char('f') => return Some(Action::ScrollDown),
            KeyCode::Char('r') => return Some(Action::ExecuteCell),
            KeyCode::Char('R') => return Some(Action::ExecuteAllCells),
            KeyCode::Char('s') => return Some(Action::SaveFile),
            _ => {}
        }
    }

    if mode == crate::app::Mode::Normal {
        match key.code {
            KeyCode::Esc => return Some(Action::LeaveCell),
            KeyCode::Char(':') => return Some(Action::ChangeMode(crate::app::Mode::Command)),
            KeyCode::Char('?') => return Some(Action::ShowHelp),
            _ => {}
        }
    }

    Some(Action::HandleInCellKey(key))
}

fn handle_normal_mode(key: KeyEvent, in_cell_mode: bool) -> Option<Action> {
    if key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
    {
        return Some(Action::ExecuteCell);
    }

    if in_cell_mode {
        return None;
    }

    match key.code {
        KeyCode::Char('q') => Some(Action::Quit),
        KeyCode::Enter => Some(Action::EnterCell),
        KeyCode::Char(':') => Some(Action::ChangeMode(crate::app::Mode::Command)),
        KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ExecuteCellsBelow)
        }
        KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ExecuteCellsAbove)
        }
        KeyCode::Char('j') | KeyCode::Down => Some(Action::MoveCellDown),
        KeyCode::Char('k') | KeyCode::Up => Some(Action::MoveCellUp),
        KeyCode::Char('a')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(Action::InsertCellAbove)
            } else {
                Some(Action::InsertCellBelow)
            }
        }
        KeyCode::Char('o')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            if key.modifiers.contains(KeyModifiers::SHIFT) {
                Some(Action::InsertCellAbove)
            } else {
                Some(Action::InsertCellBelow)
            }
        }
        KeyCode::Char('A') | KeyCode::Char('O')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(Action::InsertCellAbove)
        }
        KeyCode::Char('b')
            if key.modifiers.contains(KeyModifiers::SHIFT)
                && !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(Action::InsertCellBelow)
        }
        KeyCode::Char('B')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(Action::InsertCellBelow)
        }
        KeyCode::Char('b')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(Action::InsertCellBelow)
        }
        KeyCode::Char('m') | KeyCode::Char('M') => Some(Action::SetCurrentCellMarkdown),
        KeyCode::Char('y') | KeyCode::Char('Y') => Some(Action::SetCurrentCellCode),
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ScrollUp)
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ScrollDown)
        }
        KeyCode::Char('D') => Some(Action::DeleteCell),
        KeyCode::Char('d')
            if !key.modifiers.contains(KeyModifiers::CONTROL)
                && !key.modifiers.contains(KeyModifiers::ALT) =>
        {
            Some(Action::DeleteCell)
        }
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::DeleteCell)
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ExecuteCell)
        }
        KeyCode::Char('r') => Some(Action::ExecuteCell),
        KeyCode::Char('R') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::ExecuteAllCells)
        }
        KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            Some(Action::SaveFile)
        }
        KeyCode::Char('?') => Some(Action::ShowHelp),
        KeyCode::Char('.') => Some(Action::ToggleCompletion),
        KeyCode::Char(']') => Some(Action::NextCompletion),
        KeyCode::Char('[') => Some(Action::PreviousCompletion),
        _ => None,
    }
}

fn handle_insert_mode(key: KeyEvent) -> Option<Action> {
    if key.code == KeyCode::Enter
        && (key.modifiers.contains(KeyModifiers::CONTROL)
            || key.modifiers.contains(KeyModifiers::SHIFT))
    {
        return Some(Action::ExecuteCell);
    }

    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('b') => return Some(Action::ScrollUp),
            KeyCode::Char('f') => return Some(Action::ScrollDown),
            KeyCode::Char('s') => return Some(Action::SaveFile),
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => Some(Action::ChangeMode(crate::app::Mode::Normal)),
        KeyCode::Left => Some(Action::MoveCursor(crate::editor::Direction::Left)),
        KeyCode::Right => Some(Action::MoveCursor(crate::editor::Direction::Right)),
        KeyCode::Up => Some(Action::MoveCursor(crate::editor::Direction::Up)),
        KeyCode::Down => Some(Action::MoveCursor(crate::editor::Direction::Down)),
        KeyCode::Enter => Some(Action::InsertChar('\n')),
        KeyCode::Char('.') => Some(Action::InsertChar('.')),
        KeyCode::Backspace => Some(Action::Backspace),
        KeyCode::Delete => Some(Action::DeleteChar),
        KeyCode::Char(c) => Some(Action::InsertChar(c)),
        _ => None,
    }
}

fn handle_command_mode(key: KeyEvent) -> Option<Action> {
    match key.code {
        KeyCode::Esc => Some(Action::ChangeMode(crate::app::Mode::Normal)),
        KeyCode::Char(c) => Some(Action::CommandInsertChar(c)),
        KeyCode::Backspace => Some(Action::CommandBackspace),
        KeyCode::Enter => Some(Action::ExecuteCommand),
        _ => None,
    }
}
