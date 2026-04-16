//! Integration tests for the xtce-tui App state machine.
//!
//! Tests exercise `App::apply_action` directly — no terminal I/O.  A few
//! smoke tests also call `ui::render` with ratatui's `TestBackend` to verify
//! that rendering does not panic under various states.

use std::path::PathBuf;

use ratatui::{Terminal, backend::TestBackend};
use xtce_core::model::{
    command::{ArgumentType, CommandMetaData, IntegerArgumentType, MetaCommand},
    space_system::SpaceSystem,
    telemetry::{IntegerParameterType, Parameter, ParameterType, TelemetryMetaData},
};
use xtce_tui::{
    app::App,
    event::{Action, EditField},
    ui::{self, NodeId},
};

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

fn test_path() -> PathBuf {
    PathBuf::from("/tmp/xtce_test.xml")
}

/// Build a SpaceSystem with enough content to exercise most tree nodes.
fn make_ss() -> SpaceSystem {
    let mut ss = SpaceSystem::new("Root");
    let mut tm = TelemetryMetaData::default();

    let mut int_type = IntegerParameterType::new("UINT8");
    int_type.short_description = Some("8-bit unsigned".to_string());
    tm.parameter_types
        .insert("UINT8".to_string(), ParameterType::Integer(int_type));

    let mut p = Parameter::new("Voltage", "UINT8");
    p.short_description = Some("Bus voltage".to_string());
    tm.parameters.insert("Voltage".to_string(), p);

    ss.telemetry = Some(tm);
    // Add a child SpaceSystem so the tree has at least two top-level visible nodes
    // (root + child appear together when collapsed).
    ss.sub_systems.push(SpaceSystem::new("Payload"));
    ss
}

/// Return an App whose tree has been expanded one level so navigation tests
/// can move the cursor (collapsed single-root trees have only one visible row).
fn make_expanded_app() -> App {
    let mut app = make_app();
    app.apply_action(Action::Expand);
    app
}

fn make_app() -> App {
    App::new(test_path(), make_ss())
}

// ─────────────────────────────────────────────────────────────────────────────
// Initialisation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn initial_cursor_at_zero() {
    let app = make_app();
    assert_eq!(app.cursor, 0);
}

#[test]
fn initial_tree_not_empty() {
    let app = make_app();
    assert!(!app.tree.is_empty());
}

#[test]
fn initial_not_dirty() {
    let app = make_app();
    assert!(!app.dirty);
}

#[test]
fn initial_no_validation_errors_for_valid_model() {
    let app = make_app();
    assert!(app.validation_errors.is_empty());
}

#[test]
fn initial_undo_stack_empty() {
    let app = make_app();
    assert!(app.undo_stack.is_empty());
    assert!(app.redo_stack.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Navigation
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn move_down_increments_cursor() {
    let mut app = make_expanded_app();
    app.apply_action(Action::MoveDown);
    assert_eq!(app.cursor, 1);
}

#[test]
fn move_up_at_top_clamps_to_zero() {
    let mut app = make_expanded_app();
    app.apply_action(Action::MoveUp);
    assert_eq!(app.cursor, 0);
}

#[test]
fn move_down_past_end_clamps() {
    let mut app = make_expanded_app();
    let tree_len = app.tree.len();
    for _ in 0..tree_len + 5 {
        app.apply_action(Action::MoveDown);
    }
    assert_eq!(app.cursor, tree_len - 1);
}

#[test]
fn move_up_after_moving_down() {
    let mut app = make_expanded_app();
    app.apply_action(Action::MoveDown);
    app.apply_action(Action::MoveDown);
    app.apply_action(Action::MoveUp);
    assert_eq!(app.cursor, 1);
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree expand / collapse
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn toggle_expand_adds_to_expanded_set() {
    let mut app = make_app();
    let node_id = app.tree[app.cursor].node_id.clone();
    assert!(!app.expanded.contains(&node_id));
    app.apply_action(Action::ToggleExpand);
    assert!(app.expanded.contains(&node_id));
}

#[test]
fn toggle_expand_twice_collapses() {
    let mut app = make_app();
    let node_id = app.tree[app.cursor].node_id.clone();
    app.apply_action(Action::ToggleExpand);
    app.apply_action(Action::ToggleExpand);
    assert!(!app.expanded.contains(&node_id));
}

#[test]
fn expand_reveals_child_nodes() {
    let mut app = make_app();
    let collapsed_len = app.tree.len();
    app.apply_action(Action::Expand);
    // After expanding the root, more nodes should be visible.
    assert!(app.tree.len() > collapsed_len);
}

#[test]
fn collapse_hides_child_nodes() {
    let mut app = make_app();
    app.apply_action(Action::Expand);
    let expanded_len = app.tree.len();
    app.apply_action(Action::Collapse);
    assert!(app.tree.len() <= expanded_len);
}

// ─────────────────────────────────────────────────────────────────────────────
// Search
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn search_start_enters_search_mode() {
    let mut app = make_app();
    assert!(!app.search_mode);
    app.apply_action(Action::SearchStart);
    assert!(app.search_mode);
}

#[test]
fn search_char_appends_to_query() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    app.apply_action(Action::SearchChar('V'));
    app.apply_action(Action::SearchChar('o'));
    assert_eq!(app.search_query, "Vo");
}

#[test]
fn search_finds_parameter_by_name() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    for c in "Voltage".chars() {
        app.apply_action(Action::SearchChar(c));
    }
    // Search runs on commit (Enter), not live on each keystroke.
    app.apply_action(Action::SearchCommit);
    assert!(!app.search_matches.is_empty());
}

#[test]
fn search_backspace_trims_query() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    app.apply_action(Action::SearchChar('V'));
    app.apply_action(Action::SearchChar('x'));
    app.apply_action(Action::SearchBackspace);
    assert_eq!(app.search_query, "V");
}

#[test]
fn search_exit_clears_mode() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    app.apply_action(Action::SearchChar('V'));
    app.apply_action(Action::SearchExit);
    // SearchExit leaves the mode but keeps the query visible in the status bar.
    assert!(!app.search_mode);
}

#[test]
fn search_no_match_for_absent_name() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    for c in "zzz_nonexistent".chars() {
        app.apply_action(Action::SearchChar(c));
    }
    assert!(app.search_matches.is_empty());
}

// ─────────────────────────────────────────────────────────────────────────────
// Rename (edit flow)
// ─────────────────────────────────────────────────────────────────────────────

/// Navigate to the root SpaceSystem node and rename it.
#[test]
fn rename_space_system_updates_model() {
    let mut app = make_app();
    // Cursor starts at the root SpaceSystem node.
    assert!(matches!(app.tree[0].node_id, NodeId::SpaceSystem(_)));

    app.apply_action(Action::EditStart(EditField::Name));
    assert!(app.edit_state.is_some());

    // Buffer is pre-filled with the current name ("Root") — backspace to clear it.
    for _ in 0.."Root".len() {
        app.apply_action(Action::EditBackspace);
    }
    for c in "NewRoot".chars() {
        app.apply_action(Action::EditChar(c));
    }
    app.apply_action(Action::EditCommit);

    assert!(app.edit_state.is_none());
    assert_eq!(app.space_system.name, "NewRoot");
    assert!(app.dirty);
}

#[test]
fn rename_sets_dirty_flag() {
    let mut app = make_app();
    rename_root(&mut app, "X");
    assert!(app.dirty);
}

#[test]
fn edit_cancel_leaves_model_unchanged() {
    let mut app = make_app();
    let original = app.space_system.name.clone();
    app.apply_action(Action::EditStart(EditField::Name));
    for c in "ShouldNotApply".chars() {
        app.apply_action(Action::EditChar(c));
    }
    app.apply_action(Action::EditCancel);
    assert_eq!(app.space_system.name, original);
    assert!(!app.dirty);
}

// ─────────────────────────────────────────────────────────────────────────────
// Undo / Redo
// ─────────────────────────────────────────────────────────────────────────────

fn rename_root(app: &mut App, new_name: &str) {
    app.apply_action(Action::EditStart(EditField::Name));
    let current_len = app.edit_state.as_ref().map(|e| e.buffer.len()).unwrap_or(0);
    for _ in 0..current_len {
        app.apply_action(Action::EditBackspace);
    }
    for c in new_name.chars() {
        app.apply_action(Action::EditChar(c));
    }
    app.apply_action(Action::EditCommit);
}

#[test]
fn undo_reverts_rename() {
    let mut app = make_app();
    let original = app.space_system.name.clone();
    rename_root(&mut app, "Renamed");
    assert_eq!(app.space_system.name, "Renamed");

    app.apply_action(Action::Undo);
    assert_eq!(app.space_system.name, original);
}

#[test]
fn redo_reapplies_after_undo() {
    let mut app = make_app();
    rename_root(&mut app, "Renamed");
    app.apply_action(Action::Undo);
    app.apply_action(Action::Redo);
    assert_eq!(app.space_system.name, "Renamed");
}

#[test]
fn new_edit_clears_redo_stack() {
    let mut app = make_app();
    rename_root(&mut app, "A");
    app.apply_action(Action::Undo);
    assert!(!app.redo_stack.is_empty());

    // Second rename clears redo.
    rename_root(&mut app, "B");
    assert!(app.redo_stack.is_empty());
}

#[test]
fn undo_on_empty_stack_is_no_op() {
    let mut app = make_app();
    let name_before = app.space_system.name.clone();
    app.apply_action(Action::Undo);
    assert_eq!(app.space_system.name, name_before);
}

// ─────────────────────────────────────────────────────────────────────────────
// Error overlay toggle
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn toggle_errors_shows_overlay() {
    let mut app = make_app();
    assert!(!app.show_errors);
    app.apply_action(Action::ToggleErrors);
    assert!(app.show_errors);
}

#[test]
fn toggle_errors_twice_hides_overlay() {
    let mut app = make_app();
    app.apply_action(Action::ToggleErrors);
    app.apply_action(Action::ToggleErrors);
    assert!(!app.show_errors);
}

#[test]
fn close_overlay_hides_errors() {
    let mut app = make_app();
    app.apply_action(Action::ToggleErrors);
    app.apply_action(Action::CloseOverlay);
    assert!(!app.show_errors);
}

// ─────────────────────────────────────────────────────────────────────────────
// Render smoke tests (TestBackend)
// ─────────────────────────────────────────────────────────────────────────────

fn render_once(app: &mut App) {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| ui::render(app, frame))
        .expect("render should not error");
}

#[test]
fn render_initial_state_does_not_panic() {
    let mut app = make_app();
    render_once(&mut app);
}

#[test]
fn render_with_errors_overlay_open() {
    let mut app = make_app();
    app.apply_action(Action::ToggleErrors);
    render_once(&mut app);
}

#[test]
fn render_with_help_overlay_open() {
    let mut app = make_app();
    app.apply_action(Action::ToggleHelp);
    render_once(&mut app);
}

#[test]
fn render_with_edit_state_active() {
    let mut app = make_app();
    app.apply_action(Action::EditStart(EditField::Name));
    render_once(&mut app);
}

#[test]
fn render_with_search_mode_active() {
    let mut app = make_app();
    app.apply_action(Action::SearchStart);
    app.apply_action(Action::SearchChar('V'));
    render_once(&mut app);
}

#[test]
fn render_expanded_tree_does_not_panic() {
    let mut app = make_app();
    app.apply_action(Action::Expand);
    render_once(&mut app);
}

#[test]
fn render_minimal_empty_space_system() {
    let mut app = App::new(test_path(), SpaceSystem::new("Empty"));
    render_once(&mut app);
}

// ─────────────────────────────────────────────────────────────────────────────
// Task #8: build_tree for command-side nodes and nested SpaceSystem
// ─────────────────────────────────────────────────────────────────────────────

/// Build a SpaceSystem that has both TelemetryMetaData and CommandMetaData,
/// plus a child SpaceSystem (itself with only command data).
fn make_cmd_ss() -> SpaceSystem {
    let mut ss = SpaceSystem::new("Root");

    // Telemetry side
    let mut tm = TelemetryMetaData::default();
    let pt = IntegerParameterType::new("UINT8");
    tm.parameter_types.insert("UINT8".to_string(), ParameterType::Integer(pt));
    let p = Parameter::new("Voltage", "UINT8");
    tm.parameters.insert("Voltage".to_string(), p);
    ss.telemetry = Some(tm);

    // Command side
    let mut cmd = CommandMetaData::default();
    let at = IntegerArgumentType::new("IntArg");
    cmd.argument_types.insert("IntArg".to_string(), ArgumentType::Integer(at));
    let mc = MetaCommand::new("CmdA");
    cmd.meta_commands.insert("CmdA".to_string(), mc);
    ss.command = Some(cmd);

    // Nested SpaceSystem with only commands
    let mut child = SpaceSystem::new("Payload");
    let mut child_cmd = CommandMetaData::default();
    let mc2 = MetaCommand::new("ChildCmd");
    child_cmd.meta_commands.insert("ChildCmd".to_string(), mc2);
    child.command = Some(child_cmd);
    ss.sub_systems.push(child);

    ss
}

fn make_cmd_app() -> App {
    App::new(test_path(), make_cmd_ss())
}

/// Expand a node at the given tree index and return the new tree.
fn expand_at(app: &mut App, idx: usize) {
    app.cursor = idx;
    app.apply_action(Action::Expand);
}

#[test]
fn build_tree_cmd_section_visible_when_root_expanded() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand root
    // Should have: Root, Telemetry, Commands, Payload
    let labels: Vec<_> = app.tree.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Commands"), "expected Commands node; got: {labels:?}");
}

#[test]
fn build_tree_cmd_argument_types_visible_when_cmd_section_expanded() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand Root
    // find Commands node index
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx); // expand CmdSection
    let labels: Vec<_> = app.tree.iter().map(|n| n.label.as_str()).collect();
    assert!(
        labels.contains(&"Argument Types"),
        "expected Argument Types node; got: {labels:?}"
    );
    assert!(
        labels.contains(&"MetaCommands"),
        "expected MetaCommands node; got: {labels:?}"
    );
}

#[test]
fn build_tree_cmd_argument_types_children_visible_when_expanded() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand Root
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx); // expand CmdSection
    let at_idx = app.tree.iter().position(|n| n.label == "Argument Types").unwrap();
    expand_at(&mut app, at_idx); // expand CmdArgumentTypes
    let labels: Vec<_> = app.tree.iter().map(|n| n.label.as_str()).collect();
    assert!(
        labels.contains(&"IntArg"),
        "expected IntArg node; got: {labels:?}"
    );
}

#[test]
fn build_tree_meta_commands_children_visible_when_expanded() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand Root
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx); // expand CmdSection
    let mc_idx = app.tree.iter().position(|n| n.label == "MetaCommands").unwrap();
    expand_at(&mut app, mc_idx); // expand CmdMetaCommands
    let labels: Vec<_> = app.tree.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"CmdA"), "expected CmdA node; got: {labels:?}");
}

#[test]
fn build_tree_nested_spacesystem_visible_when_root_expanded() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand Root
    let labels: Vec<_> = app.tree.iter().map(|n| n.label.as_str()).collect();
    assert!(labels.contains(&"Payload"), "expected Payload child ss; got: {labels:?}");
}

#[test]
fn build_tree_argument_type_annotation_shows_kind() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand);
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx);
    let at_idx = app.tree.iter().position(|n| n.label == "Argument Types").unwrap();
    expand_at(&mut app, at_idx);
    let intarg = app.tree.iter().find(|n| n.label == "IntArg").unwrap();
    assert_eq!(intarg.annotation.as_deref(), Some("(Integer)"));
}

// ─────────────────────────────────────────────────────────────────────────────
// Task #9: render paths for command-side nodes (smoke tests)
// ─────────────────────────────────────────────────────────────────────────────

#[test]
fn render_cmd_section_node_does_not_panic() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand); // expand Root
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    app.cursor = cmd_idx;
    render_once(&mut app);
}

#[test]
fn render_cmd_argument_types_node_does_not_panic() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand);
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx);
    let at_idx = app.tree.iter().position(|n| n.label == "Argument Types").unwrap();
    app.cursor = at_idx;
    render_once(&mut app);
}

#[test]
fn render_cmd_argument_type_leaf_does_not_panic() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand);
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx);
    let at_idx = app.tree.iter().position(|n| n.label == "Argument Types").unwrap();
    expand_at(&mut app, at_idx);
    let leaf_idx = app.tree.iter().position(|n| n.label == "IntArg").unwrap();
    app.cursor = leaf_idx;
    render_once(&mut app);
}

#[test]
fn render_cmd_meta_commands_node_does_not_panic() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand);
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx);
    let mc_idx = app.tree.iter().position(|n| n.label == "MetaCommands").unwrap();
    app.cursor = mc_idx;
    render_once(&mut app);
}

#[test]
fn render_cmd_meta_command_leaf_does_not_panic() {
    let mut app = make_cmd_app();
    app.apply_action(Action::Expand);
    let cmd_idx = app.tree.iter().position(|n| n.label == "Commands").unwrap();
    expand_at(&mut app, cmd_idx);
    let mc_idx = app.tree.iter().position(|n| n.label == "MetaCommands").unwrap();
    expand_at(&mut app, mc_idx);
    let leaf_idx = app.tree.iter().position(|n| n.label == "CmdA").unwrap();
    app.cursor = leaf_idx;
    render_once(&mut app);
}
