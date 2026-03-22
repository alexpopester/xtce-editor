//! Application state and action dispatch.
//!
//! [`App`] is the single source of truth for all runtime state:
//! the loaded [`SpaceSystem`], tree expansion state, cursor position,
//! panel focus, and validation errors.

use std::collections::HashSet;
use std::path::PathBuf;

use ratatui::widgets::ListState;
use xtce_core::{ValidationError, SpaceSystem};

use crate::event::{Action, EditField};
use crate::ui::{NodeId, TreeNode, build_tree, enumerate_all_nodes};

// ─────────────────────────────────────────────────────────────────────────────
// EditState
// ─────────────────────────────────────────────────────────────────────────────

/// State for an active inline edit prompt.
pub struct EditState {
    /// Which field is being edited.
    pub field: EditField,
    /// Current contents of the edit buffer.
    pub buffer: String,
    /// The node being edited (used by commit to write back to the model).
    pub node_id: NodeId,
}

// ─────────────────────────────────────────────────────────────────────────────
// Focus
// ─────────────────────────────────────────────────────────────────────────────

/// Which panel currently has keyboard focus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Focus {
    Tree,
    Detail,
}

// ─────────────────────────────────────────────────────────────────────────────
// App
// ─────────────────────────────────────────────────────────────────────────────

/// All runtime state for the TUI application.
pub struct App {
    /// Path of the currently loaded file (used for reload).
    pub path: PathBuf,
    /// Parsed XTCE data model.
    pub space_system: SpaceSystem,
    /// Flattened, visibility-filtered list of tree rows.
    pub tree: Vec<TreeNode>,
    /// Set of [`NodeId`]s whose children are currently visible.
    pub expanded: HashSet<NodeId>,
    /// Index of the currently selected row in `tree`.
    pub cursor: usize,
    /// Ratatui list state (keeps scroll offset in sync with cursor).
    pub list_state: ListState,
    /// Which panel has keyboard focus.
    pub focus: Focus,
    /// Errors from the last validation pass.
    pub validation_errors: Vec<ValidationError>,
    /// Scroll offset for the detail panel (in lines).
    pub detail_scroll: usize,
    /// Whether the validation error overlay is visible.
    pub show_errors: bool,
    /// Whether the help overlay is visible.
    pub show_help: bool,
    /// Whether the search prompt is currently open.
    pub search_mode: bool,
    /// Current search query string.
    pub search_query: String,
    /// All [`NodeId`]s (across the entire SpaceSystem, not just visible rows)
    /// whose label matches `search_query`.
    pub search_matches: Vec<NodeId>,
    /// Which element of `search_matches` is the "active" (jump-to) match.
    pub search_match_cursor: usize,
    /// True when the in-memory model differs from what is on disk.
    pub dirty: bool,
    /// Error message from the last failed save attempt, shown in the status bar.
    pub save_error: Option<String>,
    /// Active inline edit prompt, or `None` when not editing.
    pub edit_state: Option<EditState>,
}

impl App {
    /// Construct an [`App`] from a parsed [`SpaceSystem`].
    ///
    /// Runs an initial validation pass and builds the initial (collapsed) tree.
    pub fn new(path: PathBuf, space_system: SpaceSystem) -> Self {
        let expanded = HashSet::new();
        let validation_errors = xtce_core::validator::validate(&space_system);
        let tree = build_tree(&space_system, &expanded);
        let mut list_state = ListState::default();
        if !tree.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            path,
            space_system,
            tree,
            expanded,
            cursor: 0,
            list_state,
            focus: Focus::Tree,
            validation_errors,
            detail_scroll: 0,
            show_errors: false,
            show_help: false,
            search_mode: false,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_cursor: 0,
            dirty: false,
            save_error: None,
            edit_state: None,
        }
    }

    /// Dispatch an [`Action`] to the appropriate handler.
    pub fn apply_action(&mut self, action: Action) {
        // Edit mode intercepts all input except quit.
        if self.edit_state.is_some() {
            match action {
                Action::EditChar(c) => { self.edit_state.as_mut().unwrap().buffer.push(c); }
                Action::EditBackspace => { self.edit_state.as_mut().unwrap().buffer.pop(); }
                Action::EditCommit => self.commit_edit(),
                Action::EditCancel => { self.edit_state = None; }
                _ => {}
            }
            return;
        }

        // Overlays consume navigation and toggle keys while open.
        if self.show_errors {
            match action {
                Action::ToggleErrors | Action::CloseOverlay => self.show_errors = false,
                Action::MoveUp => self.detail_scroll = self.detail_scroll.saturating_sub(1),
                Action::MoveDown => self.detail_scroll += 1,
                _ => {}
            }
            return;
        }
        if self.show_help {
            match action {
                Action::ToggleHelp | Action::CloseOverlay => self.show_help = false,
                _ => {}
            }
            return;
        }

        match action {
            Action::MoveUp => self.move_cursor(-1),
            Action::MoveDown => self.move_cursor(1),
            Action::PageUp => self.move_cursor(-10),
            Action::PageDown => self.move_cursor(10),
            Action::ToggleExpand => self.toggle_expand(),
            Action::Expand => self.expand_current(),
            Action::Collapse => self.collapse_current(),
            Action::FocusNext => self.cycle_focus(),
            Action::Reload => self.reload(),
            Action::ToggleErrors => self.show_errors = true,
            Action::ToggleHelp => self.show_help = true,
            Action::CloseOverlay => {}
            Action::Quit => {}
            Action::SearchStart => {
                self.search_mode = true;
                self.search_query.clear();
                self.search_matches.clear();
                self.search_match_cursor = 0;
            }
            Action::SearchChar(c) => {
                self.search_query.push(c);
                self.search_match_cursor = 0;
                self.recompute_search();
                if let Some(target) = self.search_matches.first().cloned() {
                    self.jump_to(target);
                }
            }
            Action::SearchBackspace => {
                self.search_query.pop();
                self.search_match_cursor = 0;
                self.recompute_search();
                if let Some(target) = self.search_matches.first().cloned() {
                    self.jump_to(target);
                }
            }
            Action::SearchNext => {
                if !self.search_matches.is_empty() {
                    self.search_match_cursor =
                        (self.search_match_cursor + 1) % self.search_matches.len();
                    let target = self.search_matches[self.search_match_cursor].clone();
                    self.jump_to(target);
                }
            }
            Action::SearchPrev => {
                if !self.search_matches.is_empty() {
                    let len = self.search_matches.len();
                    self.search_match_cursor = (self.search_match_cursor + len - 1) % len;
                    let target = self.search_matches[self.search_match_cursor].clone();
                    self.jump_to(target);
                }
            }
            Action::SearchExit => {
                self.search_mode = false;
                // Matches stay active so n/N can still navigate them.
            }
            Action::Save => self.save(),
            Action::EditStart(field) => self.start_edit(field),
            // These are only dispatched while edit_state.is_some(); ignore otherwise.
            Action::EditChar(_) | Action::EditBackspace | Action::EditCommit | Action::EditCancel => {}
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    fn move_cursor(&mut self, delta: i64) {
        match self.focus {
            Focus::Tree => {
                if self.tree.is_empty() {
                    return;
                }
                let len = self.tree.len() as i64;
                let new = (self.cursor as i64 + delta).clamp(0, len - 1) as usize;
                if new != self.cursor {
                    self.cursor = new;
                    self.list_state.select(Some(new));
                    self.detail_scroll = 0;
                }
            }
            Focus::Detail => {
                let new = (self.detail_scroll as i64 + delta).max(0) as usize;
                self.detail_scroll = new;
            }
        }
    }

    fn toggle_expand(&mut self) {
        let Some(node) = self.tree.get(self.cursor) else {
            return;
        };
        if !node.expandable {
            return;
        }
        let id = node.node_id.clone();
        if self.expanded.contains(&id) {
            self.expanded.remove(&id);
        } else {
            self.expanded.insert(id);
        }
        self.rebuild_tree();
    }

    fn expand_current(&mut self) {
        let Some(node) = self.tree.get(self.cursor) else {
            return;
        };
        if node.expandable && !node.expanded {
            let id = node.node_id.clone();
            self.expanded.insert(id);
            self.rebuild_tree();
        }
    }

    fn collapse_current(&mut self) {
        let Some(node) = self.tree.get(self.cursor) else {
            return;
        };
        if node.expandable && node.expanded {
            let id = node.node_id.clone();
            self.expanded.remove(&id);
            self.rebuild_tree();
        }
    }

    fn cycle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Tree => Focus::Detail,
            Focus::Detail => Focus::Tree,
        };
        self.detail_scroll = 0;
    }

    fn rebuild_tree(&mut self) {
        self.tree = build_tree(&self.space_system, &self.expanded);
        if !self.tree.is_empty() {
            self.cursor = self.cursor.min(self.tree.len() - 1);
        }
        self.list_state.select(Some(self.cursor));
        self.detail_scroll = 0;
        self.recompute_search();
    }

    /// Expand all ancestors of `target`, rebuild the tree, then move the cursor
    /// to the target row.
    pub fn jump_to(&mut self, target: NodeId) {
        for id in ancestors_to_expand(&target) {
            self.expanded.insert(id);
        }
        self.rebuild_tree();
        if let Some(idx) = self.tree.iter().position(|n| n.node_id == target) {
            self.cursor = idx;
            self.list_state.select(Some(idx));
            self.detail_scroll = 0;
        }
    }

    /// Recompute `search_matches` by scanning the entire SpaceSystem hierarchy,
    /// regardless of which nodes are currently expanded.
    ///
    /// This means collapsed nodes are always findable; navigating to a match
    /// automatically expands its ancestors via [`Self::jump_to`].
    pub fn recompute_search(&mut self) {
        self.search_matches.clear();
        if self.search_query.is_empty() {
            return;
        }
        let q = self.search_query.to_lowercase();
        self.search_matches = enumerate_all_nodes(&self.space_system)
            .into_iter()
            .filter(|(_, label)| label.to_lowercase().contains(&q))
            .map(|(id, _)| id)
            .collect();
        // Clamp cursor without resetting it so position is preserved on rebuild.
        if !self.search_matches.is_empty() {
            self.search_match_cursor =
                self.search_match_cursor.min(self.search_matches.len() - 1);
        } else {
            self.search_match_cursor = 0;
        }
    }

    // ── Edit prompt ───────────────────────────────────────────────────────────

    fn start_edit(&mut self, field: EditField) {
        // Only trigger from the tree panel on a leaf or SpaceSystem node.
        if self.focus != Focus::Tree {
            return;
        }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();
        let Some(initial) = self.initial_value(&node_id, &field) else { return };
        self.edit_state = Some(EditState { field, buffer: initial, node_id });
    }

    fn commit_edit(&mut self) {
        let Some(edit) = self.edit_state.take() else { return };
        let new_value = edit.buffer.trim().to_string();
        if new_value.is_empty() {
            return;
        }
        let new_node_id = self.apply_field_edit(&edit.node_id, &edit.field, new_value);
        self.dirty = true;
        self.save_error = None;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
        if let Some(id) = new_node_id {
            self.jump_to(id);
        }
    }

    /// Return the current value of `field` for `node_id`, or `None` if not editable.
    fn initial_value(&self, node_id: &NodeId, field: &EditField) -> Option<String> {
        match field {
            EditField::Name => match node_id {
                NodeId::SpaceSystem(path) => {
                    if path.is_empty() {
                        Some(self.space_system.name.clone())
                    } else {
                        path.last().cloned()
                    }
                }
                NodeId::TmParameterType(_, name)
                | NodeId::TmParameter(_, name)
                | NodeId::TmContainer(_, name)
                | NodeId::CmdArgumentType(_, name)
                | NodeId::CmdMetaCommand(_, name) => Some(name.clone()),
                _ => None,
            },
            EditField::ShortDescription => match node_id {
                NodeId::SpaceSystem(path) => get_ss(&self.space_system, path)
                    .map(|ss| ss.short_description.clone().unwrap_or_default()),
                NodeId::TmParameterType(path, name) => get_ss(&self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_ref())
                    .and_then(|tm| tm.parameter_types.get(name.as_str()))
                    .map(|pt| pt.short_description().unwrap_or("").to_string()),
                NodeId::TmParameter(path, name) => get_ss(&self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_ref())
                    .and_then(|tm| tm.parameters.get(name.as_str()))
                    .map(|p| p.short_description.clone().unwrap_or_default()),
                NodeId::TmContainer(path, name) => get_ss(&self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_ref())
                    .and_then(|tm| tm.containers.get(name.as_str()))
                    .map(|c| c.short_description.clone().unwrap_or_default()),
                NodeId::CmdArgumentType(path, name) => get_ss(&self.space_system, path)
                    .and_then(|ss| ss.command.as_ref())
                    .and_then(|cmd| cmd.argument_types.get(name.as_str()))
                    .map(|at| at.short_description().unwrap_or("").to_string()),
                NodeId::CmdMetaCommand(path, name) => get_ss(&self.space_system, path)
                    .and_then(|ss| ss.command.as_ref())
                    .and_then(|cmd| cmd.meta_commands.get(name.as_str()))
                    .map(|mc| mc.short_description.clone().unwrap_or_default()),
                _ => None,
            },
        }
    }

    /// Apply an edit to the model.  Returns the new [`NodeId`] after a rename.
    fn apply_field_edit(
        &mut self,
        node_id: &NodeId,
        field: &EditField,
        value: String,
    ) -> Option<NodeId> {
        match field {
            EditField::Name => self.apply_rename(node_id, value),
            EditField::ShortDescription => {
                self.apply_short_description(node_id, value);
                None
            }
        }
    }

    fn apply_rename(&mut self, node_id: &NodeId, new_name: String) -> Option<NodeId> {
        match node_id {
            NodeId::SpaceSystem(path) => {
                let ss = get_ss_mut(&mut self.space_system, path)?;
                ss.name = new_name.clone();
                let mut new_path = path.clone();
                if let Some(last) = new_path.last_mut() {
                    *last = new_name;
                }
                Some(NodeId::SpaceSystem(new_path))
            }
            NodeId::TmParameterType(path, old_name) => {
                let tm = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())?;
                let mut entry = tm.parameter_types.shift_remove(old_name.as_str())?;
                entry.set_name(new_name.clone());
                tm.parameter_types.insert(new_name.clone(), entry);
                Some(NodeId::TmParameterType(path.clone(), new_name))
            }
            NodeId::TmParameter(path, old_name) => {
                let tm = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())?;
                let mut param = tm.parameters.shift_remove(old_name.as_str())?;
                param.name = new_name.clone();
                tm.parameters.insert(new_name.clone(), param);
                Some(NodeId::TmParameter(path.clone(), new_name))
            }
            NodeId::TmContainer(path, old_name) => {
                let tm = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())?;
                let mut c = tm.containers.shift_remove(old_name.as_str())?;
                c.name = new_name.clone();
                tm.containers.insert(new_name.clone(), c);
                Some(NodeId::TmContainer(path.clone(), new_name))
            }
            NodeId::CmdArgumentType(path, old_name) => {
                let cmd = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())?;
                let mut at = cmd.argument_types.shift_remove(old_name.as_str())?;
                at.set_name(new_name.clone());
                cmd.argument_types.insert(new_name.clone(), at);
                Some(NodeId::CmdArgumentType(path.clone(), new_name))
            }
            NodeId::CmdMetaCommand(path, old_name) => {
                let cmd = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())?;
                let mut mc = cmd.meta_commands.shift_remove(old_name.as_str())?;
                mc.name = new_name.clone();
                cmd.meta_commands.insert(new_name.clone(), mc);
                Some(NodeId::CmdMetaCommand(path.clone(), new_name))
            }
            _ => None,
        }
    }

    fn apply_short_description(&mut self, node_id: &NodeId, desc: String) {
        let opt = if desc.is_empty() { None } else { Some(desc) };
        match node_id {
            NodeId::SpaceSystem(path) => {
                if let Some(ss) = get_ss_mut(&mut self.space_system, path) {
                    ss.short_description = opt;
                }
            }
            NodeId::TmParameterType(path, name) => {
                if let Some(pt) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    pt.set_short_description(opt);
                }
            }
            NodeId::TmParameter(path, name) => {
                if let Some(p) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameters.get_mut(name.as_str()))
                {
                    p.short_description = opt;
                }
            }
            NodeId::TmContainer(path, name) => {
                if let Some(c) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.containers.get_mut(name.as_str()))
                {
                    c.short_description = opt;
                }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(at) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    at.set_short_description(opt);
                }
            }
            NodeId::CmdMetaCommand(path, name) => {
                if let Some(mc) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.meta_commands.get_mut(name.as_str()))
                {
                    mc.short_description = opt;
                }
            }
            _ => {}
        }
    }

    fn reload(&mut self) {
        match xtce_core::parser::parse_file(&self.path) {
            Ok(ss) => {
                self.space_system = ss;
                self.expanded.clear();
                self.cursor = 0;
                self.detail_scroll = 0;
                self.search_mode = false;
                self.search_query.clear();
                self.dirty = false;
                self.save_error = None;
                self.validation_errors = xtce_core::validator::validate(&self.space_system);
                self.rebuild_tree();
                self.list_state.select(Some(0));
            }
            Err(_) => {} // keep current state on parse error
        }
    }

    fn save(&mut self) {
        match xtce_core::serializer::serialize(&self.space_system) {
            Err(e) => {
                self.save_error = Some(format!("Serialize error: {e}"));
            }
            Ok(bytes) => match std::fs::write(&self.path, &bytes) {
                Ok(()) => {
                    self.dirty = false;
                    self.save_error = None;
                }
                Err(e) => {
                    self.save_error = Some(format!("Write error: {e}"));
                }
            },
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

use crate::ui::tree::{get_ss, SsPath};

/// Mutable counterpart to [`get_ss`]: navigate to the SpaceSystem at `path`.
fn get_ss_mut<'a>(
    root: &'a mut xtce_core::SpaceSystem,
    path: &[String],
) -> Option<&'a mut xtce_core::SpaceSystem> {
    let mut current = root;
    for name in path {
        current = current.sub_systems.iter_mut().find(|ss| &ss.name == name)?;
    }
    Some(current)
}

/// Return all [`NodeId`]s that must be in `expanded` for `node_id` to be visible.
///
/// SpaceSystem ancestry (every prefix of the path) is always included.
/// Section and group nodes for the immediate parent chain are added for leaf nodes.
fn ancestors_to_expand(node_id: &NodeId) -> Vec<NodeId> {
    /// Every SpaceSystem from root down to and including the given path.
    fn ss_chain(path: &SsPath) -> Vec<NodeId> {
        (0..=path.len())
            .map(|i| NodeId::SpaceSystem(path[..i].to_vec()))
            .collect()
    }

    match node_id {
        NodeId::SpaceSystem(path) => {
            // Only the ancestors of this SS need to be expanded, not the SS itself.
            (0..path.len())
                .map(|i| NodeId::SpaceSystem(path[..i].to_vec()))
                .collect()
        }
        NodeId::TmSection(path) | NodeId::CmdSection(path) => ss_chain(path),
        NodeId::TmParameterTypes(path)
        | NodeId::TmParameters(path)
        | NodeId::TmContainers(path) => {
            let mut v = ss_chain(path);
            v.push(NodeId::TmSection(path.clone()));
            v
        }
        NodeId::CmdArgumentTypes(path) | NodeId::CmdMetaCommands(path) => {
            let mut v = ss_chain(path);
            v.push(NodeId::CmdSection(path.clone()));
            v
        }
        NodeId::TmParameterType(path, _) => {
            let mut v = ss_chain(path);
            v.push(NodeId::TmSection(path.clone()));
            v.push(NodeId::TmParameterTypes(path.clone()));
            v
        }
        NodeId::TmParameter(path, _) => {
            let mut v = ss_chain(path);
            v.push(NodeId::TmSection(path.clone()));
            v.push(NodeId::TmParameters(path.clone()));
            v
        }
        NodeId::TmContainer(path, _) => {
            let mut v = ss_chain(path);
            v.push(NodeId::TmSection(path.clone()));
            v.push(NodeId::TmContainers(path.clone()));
            v
        }
        NodeId::CmdArgumentType(path, _) => {
            let mut v = ss_chain(path);
            v.push(NodeId::CmdSection(path.clone()));
            v.push(NodeId::CmdArgumentTypes(path.clone()));
            v
        }
        NodeId::CmdMetaCommand(path, _) => {
            let mut v = ss_chain(path);
            v.push(NodeId::CmdSection(path.clone()));
            v.push(NodeId::CmdMetaCommands(path.clone()));
            v
        }
    }
}
