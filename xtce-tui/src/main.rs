mod app;
mod event;
mod tui;
mod ui;

use std::{env, path::PathBuf, process};

use crossterm::event::{Event, KeyEventKind};

use app::App;
use event::Action;

fn main() {
    let path = env::args().nth(1).unwrap_or_else(|| {
        eprintln!("Usage: xtce-tui <file.xml>");
        process::exit(1);
    });
    let path = PathBuf::from(path);

    let space_system = match xtce_core::parser::parse_file(&path) {
        Ok(ss) => ss,
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    };

    // Restore the terminal before printing any panic message.
    let original_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = tui::restore_raw();
        original_hook(info);
    }));

    let mut terminal = tui::init().expect("failed to initialize terminal");
    let mut app = App::new(path, space_system);

    let result = run(&mut app, &mut terminal);

    tui::restore(&mut terminal).expect("failed to restore terminal");

    if let Err(e) = result {
        eprintln!("Error: {e}");
        process::exit(1);
    }
}

fn run(app: &mut App, terminal: &mut tui::Ratatui) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| ui::render(app, frame))?;

        if let Event::Key(key) = crossterm::event::read()? {
            if key.kind != KeyEventKind::Press {
                continue;
            }
            let action = if app.picker_state.is_some() {
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
            } else {
                event::key_to_action(key)
            };
            match action {
                Some(Action::Quit) => break,
                Some(action) => app.apply_action(action),
                None => {}
            }
        }
    }
    Ok(())
}
