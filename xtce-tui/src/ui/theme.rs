//! Colour and style palette.
//!
//! All visual styling is defined here. Change colours once and they
//! propagate everywhere.

use ratatui::style::{Color, Modifier, Style};

// ── Border colours ────────────────────────────────────────────────────────────

/// Border colour for the panel that currently has keyboard focus.
pub const BORDER_FOCUSED: Color = Color::Cyan;
/// Border colour for panels that do not have focus.
pub const BORDER_UNFOCUSED: Color = Color::DarkGray;

// ── Text styles ───────────────────────────────────────────────────────────────

/// Application title bar.
pub fn title_bar() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// SpaceSystem name in the tree.
pub fn space_system() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// Section header (TelemetryMetaData, CommandMetaData).
pub fn section_header() -> Style {
    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

/// Sub-section group node (ParameterTypes, Parameters, Containers, …).
pub fn group_node() -> Style {
    Style::default().fg(Color::Yellow)
}

/// Leaf element name (a specific ParameterType, Parameter, Container, etc.).
pub fn leaf_node() -> Style {
    Style::default()
}

/// Type annotation shown after a leaf element name, e.g. " (Integer)".
pub fn type_annotation() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Selected row when the panel has focus.
pub fn selected_focused() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(Color::DarkGray)
        .add_modifier(Modifier::BOLD)
}

/// Selected row when the panel does NOT have focus.
pub fn selected_unfocused() -> Style {
    Style::default().fg(Color::Gray).bg(Color::DarkGray)
}

/// Muted / secondary text (counts, hints).
pub fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

/// Validation error indicator.
pub fn error() -> Style {
    Style::default().fg(Color::Red)
}

/// Warning / advisory indicator.
pub fn warn() -> Style {
    Style::default().fg(Color::Yellow)
}

// ── Help bar styles ───────────────────────────────────────────────────────────

/// Key name in the help bar, e.g. "q".
pub fn key_name() -> Style {
    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
}

/// Action description in the help bar, e.g. "Quit".
pub fn key_desc() -> Style {
    Style::default().fg(Color::DarkGray)
}

// ── Detail panel styles ───────────────────────────────────────────────────────

/// Field label in the detail panel, e.g. "Name:".
pub fn detail_label() -> Style {
    Style::default().fg(Color::Cyan)
}

/// Field value in the detail panel.
pub fn detail_value() -> Style {
    Style::default()
}

/// Section divider line in the detail panel.
pub fn detail_separator() -> Style {
    Style::default().fg(Color::DarkGray)
}

// ── Search styles ─────────────────────────────────────────────────────────────

/// A tree row that matches the current search query (but is not the active match).
pub fn search_match() -> Style {
    Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
}

/// The currently active search match in the tree.
pub fn search_current_match() -> Style {
    Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD)
}
