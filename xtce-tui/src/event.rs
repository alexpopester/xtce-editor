//! Application-level actions and key → action mapping.
//!
//! Raw crossterm key events are mapped to [`Action`] values before being
//! dispatched to [`crate::app::App`]. This indirection means:
//!
//! - Key bindings are defined in one place and easy to remap.
//! - Business logic never needs to know about specific key codes.
//! - Actions can eventually be triggered by mouse, gamepad, or IPC too.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

/// The field being edited in an active edit prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EditField {
    /// The item's name (also its map key — renames update references).
    Name,
    /// The item's short, one-line description.
    ShortDescription,
}

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
    /// Save the current SpaceSystem to disk.
    Save,
    /// Open an inline edit prompt for the given field on the selected node.
    EditStart(EditField),
    /// Append a character to the edit buffer (only dispatched in edit mode).
    EditChar(char),
    /// Delete the last character from the edit buffer.
    EditBackspace,
    /// Commit the edit buffer to the model.
    EditCommit,
    /// Discard the edit buffer and close the prompt.
    EditCancel,
    /// Open the entry-add picker for the selected container or MetaCommand.
    EntryAddStart,
    /// Move the entry-add picker cursor up.
    EntryAddMoveUp,
    /// Move the entry-add picker cursor down.
    EntryAddMoveDown,
    /// Confirm the selected entry-add picker item.
    EntryAddConfirm,
    /// Append a character to the entry-add filter.
    EntryAddChar(char),
    /// Delete the last character from the entry-add filter.
    EntryAddBackspace,
    /// Cancel and close the entry-add picker.
    EntryAddCancel,
    /// Remove the last entry from the selected container or MetaCommand.
    EntryRemoveLast,
    /// Confirm reload despite unsaved changes.
    ReloadConfirm,
    /// Cancel a pending reload confirmation.
    ReloadCancel,
    /// Begin the add-item flow for the selected node.
    CreateStart,
    /// Move the selector or picker cursor up.
    CreateMoveUp,
    /// Move the selector or picker cursor down.
    CreateMoveDown,
    /// Advance through create steps (TypeVariantSelect → NamePrompt → commit).
    CreateConfirm,
    /// Append a character in a create text field.
    CreateChar(char),
    /// Delete the last character in a create text field.
    CreateBackspace,
    /// Cancel and close the create flow.
    CreateCancel,
    /// Begin the delete-confirmation prompt for the selected node.
    DeleteStart,
    /// Confirm deletion.
    DeleteConfirm,
    /// Cancel deletion.
    DeleteCancel,
    // Scalar field toggles
    ToggleSigned,
    ToggleAbstract,
    CycleDataSource,
    // Generic picker (ChangeTypeRef and SetBase)
    ChangeTypeRefStart,
    SetBaseStart,
    PickerMoveUp,
    PickerMoveDown,
    PickerConfirm,
    PickerChar(char),
    PickerBackspace,
    PickerCancel,
    // Encoding wizard
    EncodingStart,
    EncodingMoveUp,
    EncodingMoveDown,
    EncodingConfirm,
    EncodingChar(char),
    EncodingBackspace,
    EncodingCancel,
    // MetaCommand argument management
    ArgAddStart,
    ArgRemoveLast,
    // Enumeration entry editing
    EnumEntryConfirm,
    EnumEntryChar(char),
    EnumEntryBackspace,
    EnumEntryCancel,
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
        // Editing
        (KeyCode::Char('i'), _) => Some(Action::EditStart(EditField::Name)),
        (KeyCode::Char('C'), _) => Some(Action::EditStart(EditField::ShortDescription)),
        // File operations
        (KeyCode::Char('r'), _) => Some(Action::Reload),
        (KeyCode::Char('s'), _) | (KeyCode::Char('w'), KeyModifiers::CONTROL) => {
            Some(Action::Save)
        }
        // Search
        (KeyCode::Char('/'), _) => Some(Action::SearchStart),
        (KeyCode::Char('n'), _) => Some(Action::SearchNext),
        (KeyCode::Char('N'), _) => Some(Action::SearchPrev),
        // Create / delete / entry editing
        (KeyCode::Char('a'), _) => Some(Action::CreateStart),
        (KeyCode::Char('d'), _) => Some(Action::DeleteStart),
        (KeyCode::Char('A'), _) => Some(Action::EntryAddStart),
        (KeyCode::Char('x'), _) => Some(Action::EntryRemoveLast),
        (KeyCode::Char('t'), _) => Some(Action::ChangeTypeRefStart),
        (KeyCode::Char('b'), _) => Some(Action::SetBaseStart),
        (KeyCode::Char('E'), _) => Some(Action::EncodingStart),
        (KeyCode::Char('S'), _) => Some(Action::ToggleSigned),
        (KeyCode::Char('B'), _) => Some(Action::ToggleAbstract),
        (KeyCode::Char('D'), _) => Some(Action::CycleDataSource),
        (KeyCode::Char('g'), _) => Some(Action::ArgAddStart),
        (KeyCode::Char('G'), _) => Some(Action::ArgRemoveLast),
        // Overlays
        (KeyCode::Char('e'), _) => Some(Action::ToggleErrors),
        (KeyCode::Char('?'), _) => Some(Action::ToggleHelp),
        (KeyCode::Esc, _) => Some(Action::CloseOverlay),
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while an edit prompt is open.
pub fn edit_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::EditCancel),
        (KeyCode::Enter, _) => Some(Action::EditCommit),
        (KeyCode::Backspace, _) => Some(Action::EditBackspace),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::EditChar(c))
        }
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while the entry-add picker is open.
pub fn entry_add_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::EntryAddCancel),
        (KeyCode::Enter, _) => Some(Action::EntryAddConfirm),
        (KeyCode::Backspace, _) => Some(Action::EntryAddBackspace),
        (KeyCode::Up, _) => Some(Action::EntryAddMoveUp),
        (KeyCode::Down, _) => Some(Action::EntryAddMoveDown),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::EntryAddChar(c))
        }
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while a create flow is active.
///
/// Arrow keys drive the selector/picker; printable characters feed the text
/// buffer (in NamePrompt) or, for `j`/`k`, also navigate selectors/pickers
/// (routing is resolved in [`crate::app::App::apply_action`]).
pub fn create_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::CreateCancel),
        (KeyCode::Enter, _) => Some(Action::CreateConfirm),
        (KeyCode::Backspace, _) => Some(Action::CreateBackspace),
        (KeyCode::Up, _) => Some(Action::CreateMoveUp),
        (KeyCode::Down, _) => Some(Action::CreateMoveDown),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::CreateChar(c))
        }
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while a delete confirmation is pending.
pub fn delete_confirm_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Char('y'), _) => Some(Action::DeleteConfirm),
        (KeyCode::Char('n'), _) | (KeyCode::Esc, _) => Some(Action::DeleteCancel),
        _ => None,
    }
}

/// Map a raw crossterm [`KeyEvent`] to an [`Action`] while a reload confirmation is pending.
pub fn reload_confirm_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Char('y'), _) => Some(Action::ReloadConfirm),
        (KeyCode::Char('n'), _) | (KeyCode::Esc, _) => Some(Action::ReloadCancel),
        _ => None,
    }
}

pub fn picker_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::PickerCancel),
        (KeyCode::Enter, _) => Some(Action::PickerConfirm),
        (KeyCode::Backspace, _) => Some(Action::PickerBackspace),
        (KeyCode::Up, _) => Some(Action::PickerMoveUp),
        (KeyCode::Down, _) => Some(Action::PickerMoveDown),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::PickerChar(c))
        }
        _ => None,
    }
}

pub fn encoding_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::EncodingCancel),
        (KeyCode::Enter, _) => Some(Action::EncodingConfirm),
        (KeyCode::Backspace, _) => Some(Action::EncodingBackspace),
        (KeyCode::Up, _) => Some(Action::EncodingMoveUp),
        (KeyCode::Down, _) => Some(Action::EncodingMoveDown),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::EncodingChar(c))
        }
        _ => None,
    }
}

pub fn enum_entry_key_to_action(key: KeyEvent) -> Option<Action> {
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Some(Action::Quit),
        (KeyCode::Esc, _) => Some(Action::EnumEntryCancel),
        (KeyCode::Enter, _) => Some(Action::EnumEntryConfirm),
        (KeyCode::Backspace, _) => Some(Action::EnumEntryBackspace),
        (KeyCode::Char(c), m)
            if !m.contains(KeyModifiers::CONTROL) && !m.contains(KeyModifiers::ALT) =>
        {
            Some(Action::EnumEntryChar(c))
        }
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
