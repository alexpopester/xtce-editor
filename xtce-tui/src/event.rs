//! Application-level actions and key → action mapping.
//!
//! Raw crossterm key events are mapped to [`Action`] values before being
//! dispatched to [`crate::app::App`]. This indirection means:
//!
//! - Key bindings are defined in one place and easy to remap.
//! - Business logic never needs to know about specific key codes.
//! - Actions can eventually be triggered by mouse, gamepad, or IPC too.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// All actions the application understands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    /// Exit the application.
    Quit,
    /// Move the selection cursor one row up (or scroll detail up when focused).
    MoveUp,
    /// Move the selection cursor one row down (or scroll detail down when focused).
    MoveDown,
    /// Scroll up by a page.
    PageUp,
    /// Scroll down by a page.
    PageDown,
    /// If collapsed, expand the selected node; if expanded, collapse it.
    ToggleExpand,
    /// Expand the selected node (no-op if already expanded or not expandable).
    Expand,
    /// Collapse the selected node (no-op if already collapsed or not expandable).
    Collapse,
    /// Cycle keyboard focus to the next panel.
    FocusNext,
    /// Reload the current file from disk.
    Reload,
    /// Show or hide the validation error overlay.
    ToggleErrors,
    /// Show or hide the keybinding help overlay.
    ToggleHelp,
    /// Close any open overlay (Escape).
    CloseOverlay,
    /// Enter search mode (opens the search prompt).
    SearchStart,
    /// Append a character to the search query (only dispatched in search mode).
    SearchChar(char),
    /// Delete the last character from the search query.
    SearchBackspace,
    /// Advance to the next search match.
    SearchNext,
    /// Go back to the previous search match.
    SearchPrev,
    /// Exit search mode (keeps matches highlighted for navigation).
    SearchExit,
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] in normal mode.
///
/// Returns `None` for unbound keys, which are silently ignored by the
/// event loop.
pub fn key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        // Quit
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => {
            Some(Action::Quit)
        }
        // Navigation — arrow keys and vim-style hjkl
        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => Some(Action::MoveUp),
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => Some(Action::MoveDown),
        (KeyCode::PageUp, _) | (KeyCode::Char('u'), KeyModifiers::CONTROL) => {
            Some(Action::PageUp)
        }
        (KeyCode::PageDown, _) | (KeyCode::Char('d'), KeyModifiers::CONTROL) => {
            Some(Action::PageDown)
        }
        // Expand / collapse
        (KeyCode::Enter, _) | (KeyCode::Char(' '), _) => Some(Action::ToggleExpand),
        (KeyCode::Right, _) | (KeyCode::Char('l'), _) => Some(Action::Expand),
        (KeyCode::Left, _) | (KeyCode::Char('h'), _) => Some(Action::Collapse),
        // Panel management
        (KeyCode::Tab, _) => Some(Action::FocusNext),
        // File operations
        (KeyCode::Char('r'), _) => Some(Action::Reload),
        // Search
        (KeyCode::Char('/'), _) => Some(Action::SearchStart),
        (KeyCode::Char('n'), _) => Some(Action::SearchNext),
        (KeyCode::Char('N'), _) => Some(Action::SearchPrev),
        // Overlays
        (KeyCode::Char('e'), _) => Some(Action::ToggleErrors),
        (KeyCode::Char('?'), _) => Some(Action::ToggleHelp),
        (KeyCode::Esc, _) => Some(Action::CloseOverlay),
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while search mode is active.
///
/// Printable characters are routed to [`Action::SearchChar`]; navigation keys
/// still work so the user can preview matches while typing.
pub fn search_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        // Always allow quit via Ctrl+C.
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        // Exit search mode.
        (KeyCode::Esc, _) | (KeyCode::Enter, _) => Some(Action::SearchExit),
        // Edit query.
        (KeyCode::Backspace, _) => Some(Action::SearchBackspace),
        // Navigation still works so the user can see match context.
        (KeyCode::Up, _) => Some(Action::MoveUp),
        (KeyCode::Down, _) => Some(Action::MoveDown),
        // Any printable character (including uppercase via Shift) feeds the query.
        (KeyCode::Char(c), m) if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) => {
            Some(Action::SearchChar(c))
        }
        _ => None,
    }
}
