//! Terminal lifecycle management.
//!
//! Call [`init`] once at startup and [`restore`] before exit (or on panic).
//! The [`Ratatui`] type alias avoids repeating the backend type throughout.

use std::io::{self, Stdout};

use crossterm::{
    execute,
    terminal::{
        disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
    },
};
use ratatui::{backend::CrosstermBackend, Terminal};

/// Convenience alias for the concrete terminal type used throughout the app.
pub type Ratatui = Terminal<CrosstermBackend<Stdout>>;

/// Set up raw mode and the alternate screen, then create a [`Ratatui`] handle.
pub fn init() -> io::Result<Ratatui> {
    enable_raw_mode()?;
    execute!(io::stdout(), EnterAlternateScreen)?;
    Terminal::new(CrosstermBackend::new(io::stdout()))
}

/// Restore the terminal to its original state.
///
/// Disables raw mode and leaves the alternate screen. Always call this,
/// even on error paths.
pub fn restore(terminal: &mut Ratatui) -> io::Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()
}

/// Restore the terminal without needing the [`Ratatui`] handle.
///
/// Used in the panic hook, where only stdout is available.
pub fn restore_raw() -> io::Result<()> {
    disable_raw_mode()?;
    execute!(io::stdout(), LeaveAlternateScreen)
}
