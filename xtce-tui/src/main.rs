mod tui;

use std::{env, path::PathBuf, process};

use crossterm::event::{Event, KeyEventKind};
use ratatui::{
    layout::Alignment,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

use xtce_tui::app::{App, AppMode};
use xtce_tui::event::{self, Action};
use xtce_tui::ui;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: xtce-tui <file.xml>");
        process::exit(1);
    });
    let path = PathBuf::from(path);

    // Set up the panic hook before terminal init so raw mode is always restored.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = tui::restore_raw();
        original_hook(info);
    }));

    // Init the terminal first, then show a loading screen while parsing.
    // Large XTCE files can take several seconds — without this the user
    // sees nothing until parsing finishes.
    let mut terminal = tui::init().expect("failed to initialize terminal");

    let filename = path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("?")
        .to_string();
    terminal
        .draw(|frame| {
            let area = frame.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" XTCE Editor ");
            let inner = block.inner(area);
            frame.render_widget(block, area);
            let text = vec![
                Line::from(""),
                Line::from(Span::raw(format!("  Loading {}…", filename))),
                Line::from(""),
                Line::from(Span::raw("  Parsing XTCE file, please wait.")),
            ];
            frame.render_widget(
                Paragraph::new(text).alignment(Alignment::Left),
                inner,
            );
        })
        .expect("failed to draw loading screen");

    let space_system = match xtce_core::parser::parse_file(&path) {
        Ok(ss) => ss,
        Err(e) => {
            tui::restore(&mut terminal).expect("failed to restore terminal");
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    let mut app = App::new(path, space_system);

    let result = run(&mut app, &mut terminal);

    tui::restore(&mut terminal).expect("failed to restore terminal");

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn map_key(app: &App, key: crossterm::event::KeyEvent) -> Option<Action> {
    if app.picker_state.is_some() {
        event::picker_key_to_action(key)
    } else if app.encoding_state.is_some() {
        event::encoding_key_to_action(key)
    } else if app.enum_entry_state.is_some() {
        event::enum_entry_key_to_action(key)
    } else if app.entry_location_state.is_some() {
        event::entry_location_key_to_action(key)
    } else if app.restriction_edit_state.is_some() {
        event::restriction_edit_key_to_action(key)
    } else if app.calibrator_state.is_some() {
        event::calibrator_key_to_action(key)
    } else if app.unit_edit_state.is_some() {
        event::unit_edit_key_to_action(key)
    } else if app.create_state.is_some() {
        event::create_key_to_action(key)
    } else if app.entry_add_state.is_some() {
        event::entry_add_key_to_action(key)
    } else if app.delete_confirm.is_some() {
        event::delete_confirm_key_to_action(key)
    } else if app.reload_confirm {
        event::reload_confirm_key_to_action(key)
    } else if app.edit_state.is_some() {
        event::edit_key_to_action(key)
    } else if app.search_mode {
        event::search_key_to_action(key)
    } else if app.mode == AppMode::Edit {
        event::edit_mode_key_to_action(key)
    } else {
        event::key_to_action(key)
    }
}

fn run(app: &mut App, terminal: &mut tui::Ratatui) -> std::io::Result<()> {
    use std::time::Duration;

    loop {
        terminal.draw(|frame| ui::render(app, frame))?;

        // Block until at least one key press arrives.
        let first_key = loop {
            match crossterm::event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => break k,
                _ => {}
            }
        };

        // Collect all further events that are already queued (non-blocking).
        // For pure navigation actions we only keep the last one (coalescing),
        // which stops the cursor from continuing to drift after a key is released.
        // Non-navigation actions are appended as-is so nothing is silently dropped.
        let first_action = map_key(app, first_key);
        let mut actions: Vec<Action> = Vec::new();
        if let Some(a) = first_action {
            actions.push(a);
        }

        while crossterm::event::poll(Duration::ZERO)? {
            match crossterm::event::read()? {
                Event::Key(k) if k.kind == KeyEventKind::Press => {
                    if let Some(a) = map_key(app, k) {
                        // For pure navigation, replace the last queued navigation
                        // action instead of stacking infinitely — this coalesces
                        // a burst of key-repeat events into a single move.
                        if is_navigation(&a) {
                            if let Some(last) = actions.last_mut() {
                                if is_navigation(last) {
                                    *last = a;
                                    continue;
                                }
                            }
                        }
                        actions.push(a);
                    }
                }
                _ => {}
            }
        }

        for action in actions {
            match action {
                Action::Quit => return Ok(()),
                a => app.apply_action(a),
            }
        }
    }
}

/// Returns true for pure cursor-movement actions that can be coalesced when
/// the OS key-repeat buffer fills up.
fn is_navigation(action: &Action) -> bool {
    matches!(
        action,
        Action::MoveUp | Action::MoveDown | Action::PageUp | Action::PageDown
    )
}
