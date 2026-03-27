//! Application state and action dispatch.
//!
//! [`App`] is the single source of truth for all runtime state:
//! the loaded [`SpaceSystem`], tree expansion state, cursor position,
//! panel focus, and validation errors.

use std::collections::{HashSet, VecDeque};
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
// Create / Delete state types
// ─────────────────────────────────────────────────────────────────────────────

/// The eight XTCE data-type variants, shared between ParameterType and ArgumentType.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TypeVariant {
    Integer,
    Float,
    Enumerated,
    Boolean,
    String,
    Binary,
    Aggregate,
    Array,
}

impl TypeVariant {
    pub fn label(self) -> &'static str {
        match self {
            TypeVariant::Integer   => "Integer",
            TypeVariant::Float     => "Float",
            TypeVariant::Enumerated => "Enumerated",
            TypeVariant::Boolean   => "Boolean",
            TypeVariant::String    => "String",
            TypeVariant::Binary    => "Binary",
            TypeVariant::Aggregate => "Aggregate",
            TypeVariant::Array     => "Array",
        }
    }

    pub fn all() -> &'static [TypeVariant] {
        &[
            TypeVariant::Integer,
            TypeVariant::Float,
            TypeVariant::Enumerated,
            TypeVariant::Boolean,
            TypeVariant::String,
            TypeVariant::Binary,
            TypeVariant::Aggregate,
            TypeVariant::Array,
        ]
    }
}

/// What kind of entity is being created.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CreateKind {
    ParameterType,
    Parameter,
    Container,
    ArgumentType,
    MetaCommand,
    SpaceSystem,
    Argument,
}

impl CreateKind {
    pub fn label(&self) -> &'static str {
        match self {
            CreateKind::ParameterType => "ParameterType",
            CreateKind::Parameter     => "Parameter",
            CreateKind::Container     => "Container",
            CreateKind::ArgumentType  => "ArgumentType",
            CreateKind::MetaCommand   => "MetaCommand",
            CreateKind::SpaceSystem   => "SpaceSystem",
            CreateKind::Argument      => "Argument",
        }
    }
}

/// Which step of the multi-step create flow is active.
#[derive(Debug, Clone)]
pub enum CreateStep {
    /// Choose one of the 8 type variants (ParameterType / ArgumentType only).
    TypeVariantSelect { selector_cursor: usize },
    /// Enter the name for the new entity.
    NamePrompt { buffer: String, variant: Option<TypeVariant> },
    /// Pick a type reference from a filterable list (Parameter → type, Array → element type).
    PickerPrompt {
        name: String,
        variant: Option<TypeVariant>,
        filter: String,
        /// `(display_label, value_string)` — display includes type annotation, value is the bare name.
        items: Vec<(String, String)>,
        picker_cursor: usize,
    },
}

/// Which step of the entry-add flow is active.
#[derive(Debug, Clone)]
pub enum EntryAddStep {
    /// Choose entry type: ParameterRef / ContainerRef / FixedValue (containers only).
    ContainerTypeSelect { cursor: usize },
    /// Pick a parameter to add as a ParameterRefEntry.
    ParameterPicker { filter: String, items: Vec<(String, String)>, cursor: usize },
    /// Pick a container to add as a ContainerRefEntry.
    ContainerPicker { filter: String, items: Vec<(String, String)>, cursor: usize },
    /// Enter size in bits for a FixedValueEntry (digits only).
    FixedValueSizePrompt { buffer: String },
    /// Pick an argument to add as an ArgumentRefEntry (MetaCommand only).
    ArgumentPicker { filter: String, items: Vec<(String, String)>, cursor: usize },
}

/// State for the entry-add flow.
#[derive(Debug, Clone)]
pub struct EntryAddState {
    /// The container or MetaCommand whose entry list is being edited.
    pub node_id: NodeId,
    pub step: EntryAddStep,
}

/// All state needed to drive the add-item wizard.
#[derive(Debug, Clone)]
pub struct CreateState {
    pub kind: CreateKind,
    pub target_path: SsPath,
    pub target_name: Option<String>,
    pub step: CreateStep,
}

/// Pending single-key delete confirmation.
#[derive(Debug, Clone)]
pub struct DeleteConfirmState {
    pub node_id: NodeId,
    pub name: String,
}

// ── Picker state (ChangeTypeRef / SetBase) ────────────────────────────────

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerPurpose {
    ChangeTypeRef,
    SetBaseType,
    SetBaseContainer,
    SetBaseMetaCommand,
}

pub struct PickerState {
    pub purpose: PickerPurpose,
    pub node_id: NodeId,
    pub filter: String,
    pub items: Vec<(String, String)>,
    pub cursor: usize,
}

// ── Encoding wizard state ─────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EncodingStep {
    IntegerFormatSelect { cursor: usize },
    IntegerSizePrompt { format_cursor: usize, buffer: String },
    FloatSizeSelect { cursor: usize },
}

#[derive(Debug, Clone)]
pub struct EncodingState {
    pub node_id: NodeId,
    pub step: EncodingStep,
}

// ── Enumeration entry editing state ───────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EnumEntryStep {
    ValuePrompt { buffer: String },
    LabelPrompt { value: i64, buffer: String },
}

#[derive(Debug, Clone)]
pub struct EnumEntryState {
    pub node_id: NodeId,
    pub step: EnumEntryStep,
}

// ── Entry location editing state ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EntryLocationStep {
    /// Pick which entry in the list to set a location on.
    /// `items` is `(display_label, entry_index_string)`.
    PickEntry { items: Vec<(String, String)>, cursor: usize },
    /// Enter the bit offset (integer, may be negative).
    EnterOffset { entry_index: usize, entry_name: String, buffer: String },
}

#[derive(Debug, Clone)]
pub struct EntryLocationState {
    pub node_id: NodeId,
    pub step: EntryLocationStep,
}

// ── Restriction criteria editing state ────────────────────────────────────────

pub const RESTRICTION_OPERATOR_LABELS: &[&str] = &[
    "== (Equal)",
    "!= (Not equal)",
    "<  (Less than)",
    "<= (Less or equal)",
    ">  (Greater than)",
    ">= (Greater or equal)",
];

#[derive(Debug, Clone)]
pub enum RestrictionEditStep {
    /// Pick the parameter to compare against.
    PickParameter { filter: String, items: Vec<(String, String)>, cursor: usize },
    /// Pick the comparison operator.
    PickOperator { parameter_ref: String, cursor: usize },
    /// Enter the comparison value (free text).
    EnterValue { parameter_ref: String, operator_cursor: usize, buffer: String },
}

#[derive(Debug, Clone)]
pub struct RestrictionEditState {
    /// The SequenceContainer being edited.
    pub node_id: NodeId,
    pub step: RestrictionEditStep,
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
    /// Active add-item wizard state, or `None` when not creating.
    pub create_state: Option<CreateState>,
    /// Pending delete confirmation, or `None`.
    pub delete_confirm: Option<DeleteConfirmState>,
    /// Error message from a failed create attempt (e.g. duplicate name).
    pub create_error: Option<String>,
    /// Whether a "discard changes and reload?" confirmation is pending.
    pub reload_confirm: bool,
    /// Active entry-add picker, or `None`.
    pub entry_add_state: Option<EntryAddState>,
    /// Active generic picker overlay (ChangeTypeRef / SetBase), or `None`.
    pub picker_state: Option<PickerState>,
    /// Active encoding wizard, or `None`.
    pub encoding_state: Option<EncodingState>,
    /// Active enumeration entry editing state, or `None`.
    pub enum_entry_state: Option<EnumEntryState>,
    /// Active restriction criteria editor, or `None`.
    pub restriction_edit_state: Option<RestrictionEditState>,
    /// Active entry location editor, or `None`.
    pub entry_location_state: Option<EntryLocationState>,
    /// Undo history: snapshots of `space_system` taken before each mutation.
    /// Most recent snapshot is at the back. Capped at 50 entries.
    pub undo_stack: VecDeque<SpaceSystem>,
    /// Redo stack: snapshots pushed by `undo`. Cleared on any new mutation.
    pub redo_stack: VecDeque<SpaceSystem>,
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
            create_state: None,
            delete_confirm: None,
            create_error: None,
            reload_confirm: false,
            entry_add_state: None,
            picker_state: None,
            encoding_state: None,
            enum_entry_state: None,
            restriction_edit_state: None,
            entry_location_state: None,
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    /// Dispatch an [`Action`] to the appropriate handler.
    pub fn apply_action(&mut self, action: Action) {
        // Picker (ChangeTypeRef, SetBase)
        if self.picker_state.is_some() {
            match action {
                Action::PickerCancel   => { self.picker_state = None; }
                Action::PickerConfirm  => self.commit_picker(),
                Action::PickerMoveUp   => self.picker_move(-1),
                Action::PickerMoveDown => self.picker_move(1),
                Action::PickerChar(c)  => self.picker_push_char(c),
                Action::PickerBackspace => self.picker_pop_char(),
                _ => {}
            }
            return;
        }

        // Encoding wizard
        if self.encoding_state.is_some() {
            match action {
                Action::EncodingCancel   => { self.encoding_state = None; }
                Action::EncodingConfirm  => self.encoding_confirm_step(),
                Action::EncodingMoveUp   => self.encoding_move(-1),
                Action::EncodingMoveDown => self.encoding_move(1),
                Action::EncodingChar(c)  => self.encoding_push_char(c),
                Action::EncodingBackspace => self.encoding_pop_char(),
                _ => {}
            }
            return;
        }

        // Enumeration entry editing
        if self.enum_entry_state.is_some() {
            match action {
                Action::EnumEntryCancel   => { self.enum_entry_state = None; }
                Action::EnumEntryConfirm  => self.enum_entry_confirm_step(),
                Action::EnumEntryChar(c)  => self.enum_entry_push_char(c),
                Action::EnumEntryBackspace => self.enum_entry_pop_char(),
                _ => {}
            }
            return;
        }

        // Entry location editor intercepts all input.
        if self.entry_location_state.is_some() {
            match action {
                Action::EntryLocationCancel    => { self.entry_location_state = None; }
                Action::EntryLocationConfirm   => self.entry_location_confirm_step(),
                Action::EntryLocationMoveUp    => self.entry_location_move(-1),
                Action::EntryLocationMoveDown  => self.entry_location_move(1),
                Action::EntryLocationChar(c)   => self.entry_location_push_char(c),
                Action::EntryLocationBackspace => self.entry_location_pop_char(),
                _ => {}
            }
            return;
        }

        // Restriction criteria editor intercepts all input.
        if self.restriction_edit_state.is_some() {
            match action {
                Action::RestrictionEditCancel    => { self.restriction_edit_state = None; }
                Action::RestrictionEditConfirm   => self.restriction_edit_confirm_step(),
                Action::RestrictionEditMoveUp    => self.restriction_edit_move(-1),
                Action::RestrictionEditMoveDown  => self.restriction_edit_move(1),
                Action::RestrictionEditChar(c)   => self.restriction_edit_push_char(c),
                Action::RestrictionEditBackspace => self.restriction_edit_pop_char(),
                _ => {}
            }
            return;
        }

        // Create flow intercepts all input.
        if self.create_state.is_some() {
            match action {
                Action::CreateCancel => {
                    self.create_state = None;
                    self.create_error = None;
                }
                Action::CreateConfirm  => self.create_confirm_step(),
                Action::CreateMoveUp   => self.create_move(-1),
                Action::CreateMoveDown => self.create_move(1),
                Action::CreateChar(c)  => self.create_push_char(c),
                Action::CreateBackspace => self.create_pop_char(),
                _ => {}
            }
            return;
        }

        // Delete confirmation intercepts all input.
        if self.delete_confirm.is_some() {
            match action {
                Action::DeleteConfirm => self.commit_delete(),
                Action::DeleteCancel  => { self.delete_confirm = None; }
                _ => {}
            }
            return;
        }

        // Reload confirmation intercepts all input.
        if self.reload_confirm {
            match action {
                Action::ReloadConfirm => { self.reload_confirm = false; self.reload(); }
                Action::ReloadCancel  => { self.reload_confirm = false; }
                _ => {}
            }
            return;
        }

        // Entry-add picker intercepts all input.
        if self.entry_add_state.is_some() {
            match action {
                Action::EntryAddCancel   => { self.entry_add_state = None; }
                Action::EntryAddConfirm  => self.commit_entry_add(),
                Action::EntryAddMoveUp   => self.entry_add_move(-1),
                Action::EntryAddMoveDown => self.entry_add_move(1),
                Action::EntryAddChar(c)  => self.entry_add_push_char(c),
                Action::EntryAddBackspace => self.entry_add_pop_char(),
                _ => {}
            }
            return;
        }

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
            Action::Reload => {
                if self.dirty {
                    self.reload_confirm = true;
                } else {
                    self.reload();
                }
            }
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
            Action::Undo => self.undo(),
            Action::Redo => self.redo(),
            Action::Save => self.save(),
            Action::EditStart(field) => self.start_edit(field),
            Action::ChangeTypeRefStart => self.start_change_type_ref(),
            Action::SetBaseStart       => self.start_set_base(),
            Action::EncodingStart      => self.start_encoding(),
            Action::ToggleSigned       => self.toggle_signed(),
            Action::ToggleAbstract     => self.toggle_abstract(),
            Action::CycleDataSource    => self.cycle_data_source(),
            Action::ArgAddStart        => self.start_arg_add(),
            Action::ArgRemoveLast      => self.remove_last_argument(),
            Action::ToggleReadOnly     => self.toggle_read_only(),
            Action::RestrictionEditStart  => self.start_restriction_edit(),
            Action::EntryLocationStart    => self.start_entry_location(),
            Action::CreateStart  => self.start_create(),
            Action::DeleteStart  => self.start_delete(),
            Action::EntryAddStart   => self.start_entry_add(),
            Action::EntryRemoveLast => self.remove_last_entry(),
            // These are only dispatched in their respective modes; ignore otherwise.
            Action::EditChar(_) | Action::EditBackspace | Action::EditCommit | Action::EditCancel => {}
            Action::CreateMoveUp | Action::CreateMoveDown | Action::CreateConfirm
            | Action::CreateChar(_) | Action::CreateBackspace | Action::CreateCancel => {}
            Action::DeleteConfirm | Action::DeleteCancel => {}
            Action::ReloadConfirm | Action::ReloadCancel => {}
            Action::EntryAddMoveUp | Action::EntryAddMoveDown | Action::EntryAddConfirm
            | Action::EntryAddChar(_) | Action::EntryAddBackspace | Action::EntryAddCancel => {}
            Action::PickerMoveUp | Action::PickerMoveDown | Action::PickerConfirm
            | Action::PickerChar(_) | Action::PickerBackspace | Action::PickerCancel => {}
            Action::EncodingMoveUp | Action::EncodingMoveDown | Action::EncodingConfirm
            | Action::EncodingChar(_) | Action::EncodingBackspace | Action::EncodingCancel => {}
            Action::EnumEntryConfirm | Action::EnumEntryChar(_) | Action::EnumEntryBackspace
            | Action::EnumEntryCancel => {}
            Action::RestrictionEditMoveUp | Action::RestrictionEditMoveDown
            | Action::RestrictionEditConfirm | Action::RestrictionEditChar(_)
            | Action::RestrictionEditBackspace | Action::RestrictionEditCancel => {}
            Action::EntryLocationMoveUp | Action::EntryLocationMoveDown
            | Action::EntryLocationConfirm | Action::EntryLocationChar(_)
            | Action::EntryLocationBackspace | Action::EntryLocationCancel => {}
        }
    }

    // ── Private helpers ───────────────────────────────────────────────────────

    /// Snapshot the current `space_system` onto the undo stack before a mutation.
    ///
    /// Clears the redo stack (new branch), and caps history at 50 entries.
    fn push_undo_snapshot(&mut self) {
        if self.undo_stack.len() >= 50 {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(self.space_system.clone());
        self.redo_stack.clear();
    }

    fn undo(&mut self) {
        let Some(prev) = self.undo_stack.pop_back() else { return };
        let current = std::mem::replace(&mut self.space_system, prev);
        if self.redo_stack.len() >= 50 {
            self.redo_stack.pop_front();
        }
        self.redo_stack.push_back(current);
        self.dirty = !self.undo_stack.is_empty() || self.dirty;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    fn redo(&mut self) {
        let Some(next) = self.redo_stack.pop_back() else { return };
        let current = std::mem::replace(&mut self.space_system, next);
        if self.undo_stack.len() >= 50 {
            self.undo_stack.pop_front();
        }
        self.undo_stack.push_back(current);
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

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
        // For name edits, check for collision before applying.
        if matches!(edit.field, EditField::Name) && self.rename_would_conflict(&edit.node_id, &new_value) {
            self.save_error = Some(format!("'{}' already exists", new_value));
            self.edit_state = Some(edit);
            return;
        }
        self.push_undo_snapshot();
        let new_node_id = self.apply_field_edit(&edit.node_id, &edit.field, new_value);
        self.dirty = true;
        self.save_error = None;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
        if let Some(id) = new_node_id {
            self.jump_to(id);
        }
    }

    /// Returns `true` if renaming `node_id` to `new_name` would collide with
    /// an existing sibling.
    fn rename_would_conflict(&self, node_id: &NodeId, new_name: &str) -> bool {
        match node_id {
            NodeId::SpaceSystem(path) => {
                let parent_path = &path[..path.len().saturating_sub(1)];
                get_ss(&self.space_system, parent_path)
                    .map(|p| p.sub_systems.iter().any(|s| s.name == new_name))
                    .unwrap_or(false)
            }
            NodeId::TmParameterType(path, old_name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .map(|tm| tm.parameter_types.contains_key(new_name) && new_name != old_name.as_str())
                .unwrap_or(false),
            NodeId::TmParameter(path, old_name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .map(|tm| tm.parameters.contains_key(new_name) && new_name != old_name.as_str())
                .unwrap_or(false),
            NodeId::TmContainer(path, old_name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .map(|tm| tm.containers.contains_key(new_name) && new_name != old_name.as_str())
                .unwrap_or(false),
            NodeId::CmdArgumentType(path, old_name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.command.as_ref())
                .map(|cmd| cmd.argument_types.contains_key(new_name) && new_name != old_name.as_str())
                .unwrap_or(false),
            NodeId::CmdMetaCommand(path, old_name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.command.as_ref())
                .map(|cmd| cmd.meta_commands.contains_key(new_name) && new_name != old_name.as_str())
                .unwrap_or(false),
            _ => false,
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
                // Guard: sibling name collision.
                let parent_path = &path[..path.len().saturating_sub(1)];
                if let Some(parent) = get_ss(&self.space_system, parent_path) {
                    if parent.sub_systems.iter().any(|s| s.name == new_name) {
                        return None;
                    }
                }
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
                if tm.parameter_types.contains_key(new_name.as_str()) {
                    return None;
                }
                let mut entry = tm.parameter_types.shift_remove(old_name.as_str())?;
                entry.set_name(new_name.clone());
                tm.parameter_types.insert(new_name.clone(), entry);
                Some(NodeId::TmParameterType(path.clone(), new_name))
            }
            NodeId::TmParameter(path, old_name) => {
                let tm = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())?;
                if tm.parameters.contains_key(new_name.as_str()) {
                    return None;
                }
                let mut param = tm.parameters.shift_remove(old_name.as_str())?;
                param.name = new_name.clone();
                tm.parameters.insert(new_name.clone(), param);
                Some(NodeId::TmParameter(path.clone(), new_name))
            }
            NodeId::TmContainer(path, old_name) => {
                let tm = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())?;
                if tm.containers.contains_key(new_name.as_str()) {
                    return None;
                }
                let mut c = tm.containers.shift_remove(old_name.as_str())?;
                c.name = new_name.clone();
                tm.containers.insert(new_name.clone(), c);
                Some(NodeId::TmContainer(path.clone(), new_name))
            }
            NodeId::CmdArgumentType(path, old_name) => {
                let cmd = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())?;
                if cmd.argument_types.contains_key(new_name.as_str()) {
                    return None;
                }
                let mut at = cmd.argument_types.shift_remove(old_name.as_str())?;
                at.set_name(new_name.clone());
                cmd.argument_types.insert(new_name.clone(), at);
                Some(NodeId::CmdArgumentType(path.clone(), new_name))
            }
            NodeId::CmdMetaCommand(path, old_name) => {
                let cmd = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())?;
                if cmd.meta_commands.contains_key(new_name.as_str()) {
                    return None;
                }
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
                self.undo_stack.clear();
                self.redo_stack.clear();
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

    // ── Create flow ───────────────────────────────────────────────────────────

    fn start_create(&mut self) {
        if self.focus != Focus::Tree {
            return;
        }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();

        let (kind, target_path) = match &node_id {
            NodeId::SpaceSystem(path) => (CreateKind::SpaceSystem, path.clone()),
            NodeId::TmSection(path)
            | NodeId::TmParameterTypes(path)
            | NodeId::TmParameterType(path, _) => (CreateKind::ParameterType, path.clone()),
            NodeId::TmParameters(path) | NodeId::TmParameter(path, _) => {
                (CreateKind::Parameter, path.clone())
            }
            NodeId::TmContainers(path) | NodeId::TmContainer(path, _) => {
                (CreateKind::Container, path.clone())
            }
            NodeId::CmdSection(path)
            | NodeId::CmdArgumentTypes(path)
            | NodeId::CmdArgumentType(path, _) => (CreateKind::ArgumentType, path.clone()),
            NodeId::CmdMetaCommands(path) | NodeId::CmdMetaCommand(path, _) => {
                (CreateKind::MetaCommand, path.clone())
            }
        };

        let first_step = match kind {
            CreateKind::ParameterType | CreateKind::ArgumentType => {
                CreateStep::TypeVariantSelect { selector_cursor: 0 }
            }
            _ => CreateStep::NamePrompt { buffer: String::new(), variant: None },
        };

        self.create_state = Some(CreateState { kind, target_path, target_name: None, step: first_step });
        self.create_error = None;
    }

    fn create_move(&mut self, delta: i64) {
        let Some(cs) = self.create_state.as_mut() else { return };
        match &mut cs.step {
            CreateStep::TypeVariantSelect { selector_cursor } => {
                let max = TypeVariant::all().len() - 1;
                let new = (*selector_cursor as i64 + delta).clamp(0, max as i64) as usize;
                *selector_cursor = new;
            }
            CreateStep::PickerPrompt { items, picker_cursor, filter, .. } => {
                let count = filtered_count(items, filter);
                if count > 0 {
                    let new = (*picker_cursor as i64 + delta).clamp(0, count as i64 - 1) as usize;
                    *picker_cursor = new;
                }
            }
            CreateStep::NamePrompt { .. } => {}
        }
    }

    fn create_push_char(&mut self, c: char) {
        let Some(cs) = self.create_state.as_mut() else { return };
        match &mut cs.step {
            CreateStep::TypeVariantSelect { selector_cursor } => {
                let max = TypeVariant::all().len() - 1;
                if c == 'j' {
                    *selector_cursor = (*selector_cursor + 1).min(max);
                } else if c == 'k' {
                    *selector_cursor = selector_cursor.saturating_sub(1);
                }
            }
            CreateStep::NamePrompt { buffer, .. } => {
                buffer.push(c);
                self.create_error = None;
            }
            CreateStep::PickerPrompt { filter, picker_cursor, items, .. } => {
                if c == 'j' {
                    let count = filtered_count(items, filter);
                    if count > 0 {
                        *picker_cursor = (*picker_cursor + 1).min(count - 1);
                    }
                } else if c == 'k' {
                    *picker_cursor = picker_cursor.saturating_sub(1);
                } else {
                    filter.push(c);
                    *picker_cursor = 0;
                }
            }
        }
    }

    fn create_pop_char(&mut self) {
        let Some(cs) = self.create_state.as_mut() else { return };
        match &mut cs.step {
            CreateStep::NamePrompt { buffer, .. } => { buffer.pop(); }
            CreateStep::PickerPrompt { filter, picker_cursor, .. } => {
                filter.pop();
                *picker_cursor = 0;
            }
            CreateStep::TypeVariantSelect { .. } => {}
        }
    }

    fn create_confirm_step(&mut self) {
        // Take ownership so we can call other &self methods without borrow conflicts.
        let Some(cs) = self.create_state.take() else { return };
        let kind = cs.kind;
        let path = cs.target_path;
        let target_name = cs.target_name;

        match cs.step {
            CreateStep::TypeVariantSelect { selector_cursor } => {
                let variant = TypeVariant::all()[selector_cursor];
                self.create_state = Some(CreateState {
                    kind,
                    target_path: path,
                    target_name,
                    step: CreateStep::NamePrompt { buffer: String::new(), variant: Some(variant) },
                });
            }
            CreateStep::NamePrompt { buffer, variant } => {
                let name = buffer.trim().to_string();
                if name.is_empty() {
                    self.create_state = Some(CreateState {
                        kind,
                        target_path: path,
                        target_name,
                        step: CreateStep::NamePrompt { buffer, variant },
                    });
                    return;
                }
                if self.name_exists(&kind, &path, &name, target_name.as_deref()) {
                    self.create_error = Some(format!("'{}' already exists", name));
                    self.create_state = Some(CreateState {
                        kind,
                        target_path: path,
                        target_name,
                        step: CreateStep::NamePrompt { buffer: name, variant },
                    });
                    return;
                }
                self.create_error = None;
                let needs_picker = match &kind {
                    CreateKind::Parameter | CreateKind::Argument => true,
                    CreateKind::ParameterType | CreateKind::ArgumentType => {
                        matches!(variant, Some(TypeVariant::Array))
                    }
                    _ => false,
                };
                let picker_kind = match &kind {
                    CreateKind::Argument => CreateKind::ArgumentType,
                    other => other.clone(),
                };
                if needs_picker {
                    let items = self.build_picker_items(&picker_kind, &path);
                    self.create_state = Some(CreateState {
                        kind,
                        target_path: path,
                        target_name,
                        step: CreateStep::PickerPrompt {
                            name,
                            variant,
                            filter: String::new(),
                            items,
                            picker_cursor: 0,
                        },
                    });
                } else {
                    self.commit_create(kind, path, name, variant, None, target_name);
                }
            }
            CreateStep::PickerPrompt { name, variant, filter, items, picker_cursor } => {
                let filtered: Vec<_> = items
                    .iter()
                    .filter(|(label, _)| {
                        filter.is_empty()
                            || label.to_lowercase().contains(&filter.to_lowercase())
                    })
                    .collect();
                if filtered.is_empty() {
                    self.create_state = Some(CreateState {
                        kind,
                        target_path: path,
                        target_name,
                        step: CreateStep::PickerPrompt { name, variant, filter, items, picker_cursor },
                    });
                    return;
                }
                let idx = picker_cursor.min(filtered.len() - 1);
                let type_ref = filtered[idx].1.clone();
                self.create_error = None;
                self.commit_create(kind, path, name, variant, Some(type_ref), target_name);
            }
        }
    }

    fn name_exists(&self, kind: &CreateKind, path: &SsPath, name: &str, target_name: Option<&str>) -> bool {
        let Some(ss) = get_ss(&self.space_system, path) else { return false };
        match kind {
            CreateKind::SpaceSystem => ss.sub_systems.iter().any(|s| s.name == name),
            CreateKind::ParameterType => ss
                .telemetry
                .as_ref()
                .map(|tm| tm.parameter_types.contains_key(name))
                .unwrap_or(false),
            CreateKind::Parameter => ss
                .telemetry
                .as_ref()
                .map(|tm| tm.parameters.contains_key(name))
                .unwrap_or(false),
            CreateKind::Container => ss
                .telemetry
                .as_ref()
                .map(|tm| tm.containers.contains_key(name))
                .unwrap_or(false),
            CreateKind::ArgumentType => ss
                .command
                .as_ref()
                .map(|cmd| cmd.argument_types.contains_key(name))
                .unwrap_or(false),
            CreateKind::MetaCommand => ss
                .command
                .as_ref()
                .map(|cmd| cmd.meta_commands.contains_key(name))
                .unwrap_or(false),
            CreateKind::Argument => target_name
                .and_then(|mc_name| {
                    get_ss(&self.space_system, path)
                        .and_then(|ss| ss.command.as_ref())
                        .and_then(|cmd| cmd.meta_commands.get(mc_name))
                        .map(|mc| mc.argument_list.iter().any(|a| a.name == name))
                })
                .unwrap_or(false),
        }
    }

    fn build_picker_items(&self, kind: &CreateKind, path: &SsPath) -> Vec<(String, String)> {
        // Collect types from root down to the target SpaceSystem.
        let mut sources: Vec<&SpaceSystem> = vec![&self.space_system];
        for i in 1..=path.len() {
            if let Some(ss) = get_ss(&self.space_system, &path[..i]) {
                sources.push(ss);
            }
        }
        match kind {
            CreateKind::Parameter | CreateKind::ParameterType => {
                let mut items = Vec::new();
                for ss in &sources {
                    if let Some(tm) = &ss.telemetry {
                        for (name, pt) in &tm.parameter_types {
                            let ann = parameter_type_variant_label(pt);
                            items.push((format!("{} ({})", name, ann), name.clone()));
                        }
                    }
                }
                items
            }
            CreateKind::ArgumentType => {
                let mut items = Vec::new();
                for ss in &sources {
                    if let Some(cmd) = &ss.command {
                        for (name, at) in &cmd.argument_types {
                            let ann = argument_type_variant_label(at);
                            items.push((format!("{} ({})", name, ann), name.clone()));
                        }
                    }
                }
                items
            }
            _ => Vec::new(),
        }
    }

    fn commit_create(
        &mut self,
        kind: CreateKind,
        path: SsPath,
        name: String,
        variant: Option<TypeVariant>,
        type_ref: Option<String>,
        target_name: Option<String>,
    ) {
        self.push_undo_snapshot();
        let new_id: NodeId = match kind {
            CreateKind::SpaceSystem => {
                if let Some(parent) = get_ss_mut(&mut self.space_system, &path) {
                    parent.sub_systems.push(SpaceSystem::new(name.clone()));
                }
                let mut new_path = path.clone();
                new_path.push(name);
                NodeId::SpaceSystem(new_path)
            }
            CreateKind::ParameterType => {
                let v = variant.unwrap_or(TypeVariant::Integer);
                let pt = make_parameter_type(v, &name, type_ref.as_deref());
                if let Some(ss) = get_ss_mut(&mut self.space_system, &path) {
                    ss.telemetry
                        .get_or_insert_with(Default::default)
                        .parameter_types
                        .insert(name.clone(), pt);
                }
                NodeId::TmParameterType(path, name)
            }
            CreateKind::Parameter => {
                let type_ref = type_ref.unwrap_or_default();
                let param = xtce_core::model::telemetry::Parameter::new(name.clone(), type_ref);
                if let Some(ss) = get_ss_mut(&mut self.space_system, &path) {
                    ss.telemetry
                        .get_or_insert_with(Default::default)
                        .parameters
                        .insert(name.clone(), param);
                }
                NodeId::TmParameter(path, name)
            }
            CreateKind::Container => {
                let container = xtce_core::model::container::SequenceContainer::new(name.clone());
                if let Some(ss) = get_ss_mut(&mut self.space_system, &path) {
                    ss.telemetry
                        .get_or_insert_with(Default::default)
                        .containers
                        .insert(name.clone(), container);
                }
                NodeId::TmContainer(path, name)
            }
            CreateKind::ArgumentType => {
                let v = variant.unwrap_or(TypeVariant::Integer);
                let at = make_argument_type(v, &name, type_ref.as_deref());
                if let Some(ss) = get_ss_mut(&mut self.space_system, &path) {
                    ss.command
                        .get_or_insert_with(Default::default)
                        .argument_types
                        .insert(name.clone(), at);
                }
                NodeId::CmdArgumentType(path, name)
            }
            CreateKind::MetaCommand => {
                let mc = xtce_core::model::command::MetaCommand::new(name.clone());
                if let Some(ss) = get_ss_mut(&mut self.space_system, &path) {
                    ss.command
                        .get_or_insert_with(Default::default)
                        .meta_commands
                        .insert(name.clone(), mc);
                }
                NodeId::CmdMetaCommand(path, name)
            }
            CreateKind::Argument => {
                let type_ref = type_ref.unwrap_or_default();
                let mc_name = target_name.unwrap_or_default();
                if let Some(mc) = get_ss_mut(&mut self.space_system, &path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.meta_commands.get_mut(mc_name.as_str()))
                {
                    mc.argument_list.push(xtce_core::model::command::Argument::new(name.clone(), type_ref));
                }
                NodeId::CmdMetaCommand(path, mc_name)
            }
        };

        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
        self.jump_to(new_id);
    }

    // ── Delete flow ───────────────────────────────────────────────────────────

    fn start_delete(&mut self) {
        if self.focus != Focus::Tree {
            return;
        }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let name = match &node.node_id {
            NodeId::SpaceSystem(path) if !path.is_empty() => path.last().unwrap().clone(),
            NodeId::TmParameterType(_, name)
            | NodeId::TmParameter(_, name)
            | NodeId::TmContainer(_, name)
            | NodeId::CmdArgumentType(_, name)
            | NodeId::CmdMetaCommand(_, name) => name.clone(),
            _ => return,
        };
        self.delete_confirm = Some(DeleteConfirmState { node_id: node.node_id.clone(), name });
    }

    fn commit_delete(&mut self) {
        let Some(dc) = self.delete_confirm.take() else { return };
        self.push_undo_snapshot();
        match &dc.node_id {
            NodeId::SpaceSystem(path) if !path.is_empty() => {
                let name = path.last().unwrap().clone();
                let parent_path = path[..path.len() - 1].to_vec();
                if let Some(parent) = get_ss_mut(&mut self.space_system, &parent_path) {
                    parent.sub_systems.retain(|ss| ss.name != name);
                }
            }
            NodeId::TmParameterType(path, name) => {
                if let Some(tm) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                {
                    tm.parameter_types.shift_remove(name.as_str());
                }
            }
            NodeId::TmParameter(path, name) => {
                if let Some(tm) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                {
                    tm.parameters.shift_remove(name.as_str());
                }
            }
            NodeId::TmContainer(path, name) => {
                if let Some(tm) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                {
                    tm.containers.shift_remove(name.as_str());
                }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(cmd) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                {
                    cmd.argument_types.shift_remove(name.as_str());
                }
            }
            NodeId::CmdMetaCommand(path, name) => {
                if let Some(cmd) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                {
                    cmd.meta_commands.shift_remove(name.as_str());
                }
            }
            _ => return,
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    // ── Entry list editing ────────────────────────────────────────────────────

    fn start_entry_add(&mut self) {
        if self.focus != Focus::Tree {
            return;
        }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();

        // Enumerated parameter/argument type → enum entry flow
        use xtce_core::model::telemetry::ParameterType as PT;
        use xtce_core::model::command::ArgumentType as AT;
        let is_enum = match &node_id {
            NodeId::TmParameterType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .and_then(|tm| tm.parameter_types.get(name.as_str()))
                .map(|pt| matches!(pt, PT::Enumerated(_)))
                .unwrap_or(false),
            NodeId::CmdArgumentType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.command.as_ref())
                .and_then(|cmd| cmd.argument_types.get(name.as_str()))
                .map(|at| matches!(at, AT::Enumerated(_)))
                .unwrap_or(false),
            _ => false,
        };
        if is_enum {
            self.enum_entry_state = Some(EnumEntryState {
                node_id,
                step: EnumEntryStep::ValuePrompt { buffer: String::new() },
            });
            return;
        }

        match &node_id {
            NodeId::TmContainer(_, _) => {
                // Show entry type selector first (ParameterRef / ContainerRef / FixedValue)
                self.entry_add_state = Some(EntryAddState {
                    node_id,
                    step: EntryAddStep::ContainerTypeSelect { cursor: 0 },
                });
            }
            NodeId::CmdMetaCommand(path, mc_name) => {
                // MetaCommand: go straight to ArgumentRef picker
                let items = self.build_argument_picker_items(path, mc_name);
                self.entry_add_state = Some(EntryAddState {
                    node_id,
                    step: EntryAddStep::ArgumentPicker { filter: String::new(), items, cursor: 0 },
                });
            }
            _ => {} // not an editable entry-list node
        }
    }

    fn build_parameter_picker_items(&self, path: &SsPath) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(ss) = get_ss(&self.space_system, path) {
            if let Some(tm) = &ss.telemetry {
                for name in tm.parameters.keys() {
                    out.push((name.clone(), name.clone()));
                }
            }
        }
        out
    }

    fn build_container_picker_items(&self, path: &SsPath, exclude: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(ss) = get_ss(&self.space_system, path) {
            if let Some(tm) = &ss.telemetry {
                for name in tm.containers.keys() {
                    if name != exclude {
                        out.push((name.clone(), name.clone()));
                    }
                }
            }
        }
        out
    }

    fn build_argument_picker_items(&self, path: &SsPath, mc_name: &str) -> Vec<(String, String)> {
        let mut out = Vec::new();
        if let Some(mc) = get_ss(&self.space_system, path)
            .and_then(|ss| ss.command.as_ref())
            .and_then(|cmd| cmd.meta_commands.get(mc_name))
        {
            for arg in &mc.argument_list {
                let label = if let Some(d) = &arg.short_description {
                    format!("{} — {}", arg.name, d)
                } else {
                    format!("{} ({})", arg.name, arg.argument_type_ref)
                };
                out.push((label, arg.name.clone()));
            }
        }
        out
    }

    fn entry_add_move(&mut self, delta: i64) {
        let Some(ea) = self.entry_add_state.as_mut() else { return };
        match &mut ea.step {
            EntryAddStep::ContainerTypeSelect { cursor } => {
                *cursor = (*cursor as i64 + delta).clamp(0, 2) as usize;
            }
            EntryAddStep::ParameterPicker { filter, items, cursor }
            | EntryAddStep::ContainerPicker { filter, items, cursor }
            | EntryAddStep::ArgumentPicker { filter, items, cursor } => {
                let count = filtered_count(items, filter);
                if count > 0 {
                    *cursor = (*cursor as i64 + delta).clamp(0, count as i64 - 1) as usize;
                }
            }
            EntryAddStep::FixedValueSizePrompt { .. } => {}
        }
    }

    fn entry_add_push_char(&mut self, c: char) {
        let Some(ea) = self.entry_add_state.as_mut() else { return };
        match &mut ea.step {
            EntryAddStep::ContainerTypeSelect { cursor } => {
                if c == 'j' { *cursor = (*cursor + 1).min(2); }
                else if c == 'k' { *cursor = cursor.saturating_sub(1); }
            }
            EntryAddStep::ParameterPicker { filter, items, cursor }
            | EntryAddStep::ContainerPicker { filter, items, cursor }
            | EntryAddStep::ArgumentPicker { filter, items, cursor } => {
                if c == 'j' {
                    let count = filtered_count(items, filter);
                    if count > 0 { *cursor = (*cursor + 1).min(count - 1); }
                } else if c == 'k' {
                    *cursor = cursor.saturating_sub(1);
                } else {
                    filter.push(c);
                    *cursor = 0;
                }
            }
            EntryAddStep::FixedValueSizePrompt { buffer } => {
                if c.is_ascii_digit() { buffer.push(c); }
            }
        }
    }

    fn entry_add_pop_char(&mut self) {
        let Some(ea) = self.entry_add_state.as_mut() else { return };
        match &mut ea.step {
            EntryAddStep::ContainerTypeSelect { .. } => {}
            EntryAddStep::ParameterPicker { filter, cursor, .. }
            | EntryAddStep::ContainerPicker { filter, cursor, .. }
            | EntryAddStep::ArgumentPicker { filter, cursor, .. } => {
                filter.pop();
                *cursor = 0;
            }
            EntryAddStep::FixedValueSizePrompt { buffer } => { buffer.pop(); }
        }
    }

    fn commit_entry_add(&mut self) {
        let Some(ea) = self.entry_add_state.take() else { return };
        let node_id = ea.node_id;

        match ea.step {
            EntryAddStep::ContainerTypeSelect { cursor } => {
                // Advance to the appropriate next step.
                match cursor {
                    0 => {
                        // ParameterRef
                        let items = if let NodeId::TmContainer(path, _) = &node_id {
                            self.build_parameter_picker_items(path)
                        } else { Vec::new() };
                        self.entry_add_state = Some(EntryAddState {
                            node_id,
                            step: EntryAddStep::ParameterPicker { filter: String::new(), items, cursor: 0 },
                        });
                    }
                    1 => {
                        // ContainerRef
                        let items = if let NodeId::TmContainer(path, name) = &node_id {
                            self.build_container_picker_items(path, name)
                        } else { Vec::new() };
                        self.entry_add_state = Some(EntryAddState {
                            node_id,
                            step: EntryAddStep::ContainerPicker { filter: String::new(), items, cursor: 0 },
                        });
                    }
                    _ => {
                        // FixedValue
                        self.entry_add_state = Some(EntryAddState {
                            node_id,
                            step: EntryAddStep::FixedValueSizePrompt { buffer: String::new() },
                        });
                    }
                }
            }
            EntryAddStep::ParameterPicker { filter, items, cursor } => {
                let value = pick_from_filtered(&items, &filter, cursor);
                if let Some(value) = value {
                    self.do_add_parameter_ref(&node_id, value);
                } else {
                    self.entry_add_state = Some(EntryAddState {
                        node_id,
                        step: EntryAddStep::ParameterPicker { filter, items, cursor },
                    });
                }
            }
            EntryAddStep::ContainerPicker { filter, items, cursor } => {
                let value = pick_from_filtered(&items, &filter, cursor);
                if let Some(value) = value {
                    self.do_add_container_ref(&node_id, value);
                } else {
                    self.entry_add_state = Some(EntryAddState {
                        node_id,
                        step: EntryAddStep::ContainerPicker { filter, items, cursor },
                    });
                }
            }
            EntryAddStep::FixedValueSizePrompt { buffer } => {
                match buffer.trim().parse::<u32>() {
                    Ok(size) if size > 0 => self.do_add_fixed_value(&node_id, size),
                    _ => {
                        self.entry_add_state = Some(EntryAddState {
                            node_id,
                            step: EntryAddStep::FixedValueSizePrompt { buffer },
                        });
                    }
                }
            }
            EntryAddStep::ArgumentPicker { filter, items, cursor } => {
                let value = pick_from_filtered(&items, &filter, cursor);
                if let Some(value) = value {
                    self.do_add_argument_ref(&node_id, value);
                } else {
                    self.entry_add_state = Some(EntryAddState {
                        node_id,
                        step: EntryAddStep::ArgumentPicker { filter, items, cursor },
                    });
                }
            }
        }
    }

    fn do_add_parameter_ref(&mut self, node_id: &NodeId, param_ref: String) {
        self.push_undo_snapshot();
        use xtce_core::model::container::{ParameterRefEntry, SequenceEntry};
        if let NodeId::TmContainer(path, name) = node_id {
            if let Some(c) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.telemetry.as_mut())
                .and_then(|tm| tm.containers.get_mut(name.as_str()))
            {
                c.entry_list.push(SequenceEntry::ParameterRef(ParameterRefEntry {
                    parameter_ref: param_ref,
                    location: None,
                    include_condition: None,
                }));
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    fn do_add_container_ref(&mut self, node_id: &NodeId, container_ref: String) {
        self.push_undo_snapshot();
        use xtce_core::model::container::{ContainerRefEntry, SequenceEntry};
        if let NodeId::TmContainer(path, name) = node_id {
            if let Some(c) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.telemetry.as_mut())
                .and_then(|tm| tm.containers.get_mut(name.as_str()))
            {
                c.entry_list.push(SequenceEntry::ContainerRef(ContainerRefEntry {
                    container_ref,
                    location: None,
                    include_condition: None,
                }));
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    fn do_add_fixed_value(&mut self, node_id: &NodeId, size_in_bits: u32) {
        self.push_undo_snapshot();
        use xtce_core::model::container::{FixedValueEntry, SequenceEntry};
        if let NodeId::TmContainer(path, name) = node_id {
            if let Some(c) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.telemetry.as_mut())
                .and_then(|tm| tm.containers.get_mut(name.as_str()))
            {
                c.entry_list.push(SequenceEntry::FixedValue(FixedValueEntry {
                    size_in_bits,
                    binary_value: None,
                    location: None,
                }));
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    fn do_add_argument_ref(&mut self, node_id: &NodeId, arg_ref: String) {
        self.push_undo_snapshot();
        use xtce_core::model::command::{ArgumentRefEntry, CommandContainer, CommandEntry};
        if let NodeId::CmdMetaCommand(path, mc_name) = node_id {
            if let Some(mc) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.command.as_mut())
                .and_then(|cmd| cmd.meta_commands.get_mut(mc_name.as_str()))
            {
                let cc = mc.command_container.get_or_insert_with(|| CommandContainer {
                    name: format!("{}Container", mc_name),
                    base_container: None,
                    entry_list: Vec::new(),
                });
                cc.entry_list.push(CommandEntry::ArgumentRef(ArgumentRefEntry {
                    argument_ref: arg_ref,
                    location: None,
                }));
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    fn remove_last_entry(&mut self) {
        if self.focus != Focus::Tree {
            return;
        }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();

        // Snapshot before the pop (mutation happens inside the match arms).
        let pre_snapshot = self.space_system.clone();
        let removed = match &node_id {
            NodeId::TmContainer(path, name) => {
                get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.containers.get_mut(name.as_str()))
                    .and_then(|c| c.entry_list.pop())
                    .is_some()
            }
            NodeId::CmdMetaCommand(path, mc_name) => {
                get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.meta_commands.get_mut(mc_name.as_str()))
                    .and_then(|mc| mc.command_container.as_mut())
                    .and_then(|cc| cc.entry_list.pop())
                    .is_some()
            }
            NodeId::TmParameterType(path, name) => {
                use xtce_core::model::telemetry::ParameterType;
                if let Some(ParameterType::Enumerated(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    t.enumeration_list.pop().is_some()
                } else { false }
            }
            NodeId::CmdArgumentType(path, name) => {
                use xtce_core::model::command::ArgumentType;
                if let Some(ArgumentType::Enumerated(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    t.enumeration_list.pop().is_some()
                } else { false }
            }
            _ => false,
        };

        if removed {
            if self.undo_stack.len() >= 50 {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(pre_snapshot);
            self.redo_stack.clear();
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    // ── Picker (ChangeTypeRef / SetBase) ──────────────────────────────────────

    fn start_change_type_ref(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::TmParameter(path, _) = &node.node_id else { return };
        let path = path.clone();
        let items = self.build_picker_items(&CreateKind::Parameter, &path);
        let node_id = node.node_id.clone();
        self.picker_state = Some(PickerState {
            purpose: PickerPurpose::ChangeTypeRef,
            node_id,
            filter: String::new(),
            items,
            cursor: 0,
        });
    }

    fn start_set_base(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();

        let (purpose, items) = match &node_id {
            NodeId::TmParameterType(path, self_name) => {
                let self_name = self_name.clone();
                let mut items = self.build_picker_items(&CreateKind::ParameterType, path);
                items.retain(|(_, v)| *v != self_name);
                (PickerPurpose::SetBaseType, items)
            }
            NodeId::CmdArgumentType(path, self_name) => {
                let self_name = self_name.clone();
                let mut items = self.build_picker_items(&CreateKind::ArgumentType, path);
                items.retain(|(_, v)| *v != self_name);
                (PickerPurpose::SetBaseType, items)
            }
            NodeId::TmContainer(path, self_name) => {
                let self_name = self_name.clone();
                let mut items = Vec::new();
                if let Some(ss) = get_ss(&self.space_system, path) {
                    if let Some(tm) = &ss.telemetry {
                        for (n, _) in &tm.containers {
                            if *n != self_name {
                                items.push((n.clone(), n.clone()));
                            }
                        }
                    }
                }
                (PickerPurpose::SetBaseContainer, items)
            }
            NodeId::CmdMetaCommand(path, self_name) => {
                let self_name = self_name.clone();
                let mut items = Vec::new();
                if let Some(ss) = get_ss(&self.space_system, path) {
                    if let Some(cmd) = &ss.command {
                        for (n, _) in &cmd.meta_commands {
                            if *n != self_name {
                                items.push((n.clone(), n.clone()));
                            }
                        }
                    }
                }
                (PickerPurpose::SetBaseMetaCommand, items)
            }
            _ => return,
        };

        self.picker_state = Some(PickerState {
            purpose,
            node_id,
            filter: String::new(),
            items,
            cursor: 0,
        });
    }

    fn picker_move(&mut self, delta: i64) {
        let Some(ps) = self.picker_state.as_mut() else { return };
        let count = filtered_count(&ps.items, &ps.filter);
        if count > 0 {
            let new = (ps.cursor as i64 + delta).clamp(0, count as i64 - 1) as usize;
            ps.cursor = new;
        }
    }

    fn picker_push_char(&mut self, c: char) {
        let Some(ps) = self.picker_state.as_mut() else { return };
        if c == 'j' {
            let count = filtered_count(&ps.items, &ps.filter);
            if count > 0 { ps.cursor = (ps.cursor + 1).min(count - 1); }
        } else if c == 'k' {
            ps.cursor = ps.cursor.saturating_sub(1);
        } else {
            ps.filter.push(c);
            ps.cursor = 0;
        }
    }

    fn picker_pop_char(&mut self) {
        let Some(ps) = self.picker_state.as_mut() else { return };
        ps.filter.pop();
        ps.cursor = 0;
    }

    fn commit_picker(&mut self) {
        let Some(ps) = self.picker_state.take() else { return };
        let q = ps.filter.to_lowercase();
        let filtered: Vec<_> = ps.items.iter()
            .filter(|(l, _)| q.is_empty() || l.to_lowercase().contains(&q))
            .collect();
        if filtered.is_empty() { return; }
        let idx = ps.cursor.min(filtered.len() - 1);
        let value = filtered[idx].1.clone();
        self.push_undo_snapshot();

        match ps.purpose {
            PickerPurpose::ChangeTypeRef => {
                if let NodeId::TmParameter(ref path, ref name) = ps.node_id {
                    if let Some(p) = get_ss_mut(&mut self.space_system, path)
                        .and_then(|ss| ss.telemetry.as_mut())
                        .and_then(|tm| tm.parameters.get_mut(name.as_str()))
                    {
                        p.parameter_type_ref = value;
                    }
                }
            }
            PickerPurpose::SetBaseType => {
                match &ps.node_id {
                    NodeId::TmParameterType(path, name) => {
                        if let Some(pt) = get_ss_mut(&mut self.space_system, path)
                            .and_then(|ss| ss.telemetry.as_mut())
                            .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                        {
                            pt.set_base_type(Some(value));
                        }
                    }
                    NodeId::CmdArgumentType(path, name) => {
                        if let Some(at) = get_ss_mut(&mut self.space_system, path)
                            .and_then(|ss| ss.command.as_mut())
                            .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                        {
                            at.set_base_type(Some(value));
                        }
                    }
                    _ => return,
                }
            }
            PickerPurpose::SetBaseContainer => {
                if let NodeId::TmContainer(ref path, ref name) = ps.node_id {
                    if let Some(c) = get_ss_mut(&mut self.space_system, path)
                        .and_then(|ss| ss.telemetry.as_mut())
                        .and_then(|tm| tm.containers.get_mut(name.as_str()))
                    {
                        c.base_container = Some(xtce_core::model::container::BaseContainer {
                            container_ref: value,
                            restriction_criteria: None,
                        });
                    }
                }
            }
            PickerPurpose::SetBaseMetaCommand => {
                if let NodeId::CmdMetaCommand(ref path, ref name) = ps.node_id {
                    if let Some(mc) = get_ss_mut(&mut self.space_system, path)
                        .and_then(|ss| ss.command.as_mut())
                        .and_then(|cmd| cmd.meta_commands.get_mut(name.as_str()))
                    {
                        mc.base_meta_command = Some(value);
                    }
                }
            }
        }

        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    // ── Scalar toggles ────────────────────────────────────────────────────────

    /// Commit a pre-snapshot to the undo stack only if a mutation actually occurred.
    fn commit_snapshot_if_changed(&mut self, pre: SpaceSystem, changed: bool) {
        if changed {
            if self.undo_stack.len() >= 50 {
                self.undo_stack.pop_front();
            }
            self.undo_stack.push_back(pre);
            self.redo_stack.clear();
        }
    }

    fn toggle_signed(&mut self) {
        use xtce_core::model::telemetry::ParameterType;
        use xtce_core::model::command::ArgumentType;
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let pre = self.space_system.clone();
        let changed = match &node.node_id.clone() {
            NodeId::TmParameterType(path, name) => {
                if let Some(ParameterType::Integer(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    t.signed = !t.signed;
                    true
                } else { false }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(ArgumentType::Integer(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    t.signed = !t.signed;
                    true
                } else { false }
            }
            _ => false,
        };
        self.commit_snapshot_if_changed(pre, changed);
        if changed {
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    fn toggle_abstract(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let pre = self.space_system.clone();
        let changed = match &node.node_id.clone() {
            NodeId::TmContainer(path, name) => {
                if let Some(c) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.containers.get_mut(name.as_str()))
                {
                    c.r#abstract = !c.r#abstract;
                    true
                } else { false }
            }
            NodeId::CmdMetaCommand(path, name) => {
                if let Some(mc) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.meta_commands.get_mut(name.as_str()))
                {
                    mc.r#abstract = !mc.r#abstract;
                    true
                } else { false }
            }
            _ => false,
        };
        self.commit_snapshot_if_changed(pre, changed);
        if changed {
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    fn cycle_data_source(&mut self) {
        use xtce_core::model::telemetry::{DataSource, ParameterProperties};
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::TmParameter(path, name) = node.node_id.clone() else { return };
        let pre = self.space_system.clone();
        let changed = if let Some(p) = get_ss_mut(&mut self.space_system, &path)
            .and_then(|ss| ss.telemetry.as_mut())
            .and_then(|tm| tm.parameters.get_mut(name.as_str()))
        {
            let props = p.parameter_properties.get_or_insert_with(ParameterProperties::default);
            props.data_source = Some(match &props.data_source {
                None | Some(DataSource::Telemetered) => DataSource::Derived,
                Some(DataSource::Derived)  => DataSource::Constant,
                Some(DataSource::Constant) => DataSource::Local,
                Some(DataSource::Local)    => DataSource::Ground,
                Some(DataSource::Ground)   => DataSource::Telemetered,
            });
            true
        } else { false };
        self.commit_snapshot_if_changed(pre, changed);
        if changed {
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    fn toggle_read_only(&mut self) {
        use xtce_core::model::telemetry::ParameterProperties;
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::TmParameter(path, name) = node.node_id.clone() else { return };
        let pre = self.space_system.clone();
        let changed = if let Some(p) = get_ss_mut(&mut self.space_system, &path)
            .and_then(|ss| ss.telemetry.as_mut())
            .and_then(|tm| tm.parameters.get_mut(name.as_str()))
        {
            let props = p.parameter_properties.get_or_insert_with(ParameterProperties::default);
            props.read_only = !props.read_only;
            true
        } else { false };
        self.commit_snapshot_if_changed(pre, changed);
        if changed {
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    // ── Restriction criteria editor ───────────────────────────────────────────

    fn start_restriction_edit(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::TmContainer(path, _) = node.node_id.clone() else { return };
        // Only offer the editor when a base container is already set.
        let has_base = get_ss(&self.space_system, &path)
            .and_then(|ss| ss.telemetry.as_ref())
            .and_then(|tm| {
                if let NodeId::TmContainer(_, name) = &node.node_id {
                    tm.containers.get(name.as_str())
                } else { None }
            })
            .map(|c| c.base_container.is_some())
            .unwrap_or(false);
        if !has_base { return; }

        // Build parameter picker items (parameters from this SpaceSystem).
        let items = self.build_parameter_picker_items(&path);
        self.restriction_edit_state = Some(RestrictionEditState {
            node_id: node.node_id.clone(),
            step: RestrictionEditStep::PickParameter { filter: String::new(), items, cursor: 0 },
        });
    }

    fn restriction_edit_move(&mut self, delta: i64) {
        let Some(res) = self.restriction_edit_state.as_mut() else { return };
        match &mut res.step {
            RestrictionEditStep::PickParameter { filter, items, cursor } => {
                let count = filtered_count(items, filter);
                if count > 0 {
                    *cursor = (*cursor as i64 + delta).clamp(0, count as i64 - 1) as usize;
                }
            }
            RestrictionEditStep::PickOperator { cursor, .. } => {
                let max = RESTRICTION_OPERATOR_LABELS.len() - 1;
                *cursor = (*cursor as i64 + delta).clamp(0, max as i64) as usize;
            }
            RestrictionEditStep::EnterValue { .. } => {}
        }
    }

    fn restriction_edit_push_char(&mut self, c: char) {
        let Some(res) = self.restriction_edit_state.as_mut() else { return };
        match &mut res.step {
            RestrictionEditStep::PickParameter { filter, items, cursor } => {
                if c == 'j' {
                    let count = filtered_count(items, filter);
                    if count > 0 { *cursor = (*cursor + 1).min(count - 1); }
                } else if c == 'k' {
                    *cursor = cursor.saturating_sub(1);
                } else {
                    filter.push(c);
                    *cursor = 0;
                }
            }
            RestrictionEditStep::PickOperator { cursor, .. } => {
                let max = RESTRICTION_OPERATOR_LABELS.len() - 1;
                if c == 'j' { *cursor = (*cursor + 1).min(max); }
                else if c == 'k' { *cursor = cursor.saturating_sub(1); }
            }
            RestrictionEditStep::EnterValue { buffer, .. } => { buffer.push(c); }
        }
    }

    fn restriction_edit_pop_char(&mut self) {
        let Some(res) = self.restriction_edit_state.as_mut() else { return };
        match &mut res.step {
            RestrictionEditStep::PickParameter { filter, cursor, .. } => {
                filter.pop();
                *cursor = 0;
            }
            RestrictionEditStep::PickOperator { .. } => {}
            RestrictionEditStep::EnterValue { buffer, .. } => { buffer.pop(); }
        }
    }

    fn restriction_edit_confirm_step(&mut self) {
        let Some(res) = self.restriction_edit_state.take() else { return };
        let node_id = res.node_id;
        match res.step {
            RestrictionEditStep::PickParameter { filter, items, cursor } => {
                let value = pick_from_filtered(&items, &filter, cursor);
                if let Some(param_ref) = value {
                    self.restriction_edit_state = Some(RestrictionEditState {
                        node_id,
                        step: RestrictionEditStep::PickOperator { parameter_ref: param_ref, cursor: 0 },
                    });
                } else {
                    self.restriction_edit_state = Some(RestrictionEditState {
                        node_id,
                        step: RestrictionEditStep::PickParameter { filter, items, cursor },
                    });
                }
            }
            RestrictionEditStep::PickOperator { parameter_ref, cursor } => {
                self.restriction_edit_state = Some(RestrictionEditState {
                    node_id,
                    step: RestrictionEditStep::EnterValue {
                        parameter_ref,
                        operator_cursor: cursor,
                        buffer: String::new(),
                    },
                });
            }
            RestrictionEditStep::EnterValue { parameter_ref, operator_cursor, buffer } => {
                let value = buffer.trim().to_string();
                if value.is_empty() {
                    self.restriction_edit_state = Some(RestrictionEditState {
                        node_id,
                        step: RestrictionEditStep::EnterValue { parameter_ref, operator_cursor, buffer },
                    });
                    return;
                }
                self.commit_restriction_criteria(&node_id, parameter_ref, operator_cursor, value);
            }
        }
    }

    fn commit_restriction_criteria(
        &mut self,
        node_id: &NodeId,
        parameter_ref: String,
        operator_cursor: usize,
        value: String,
    ) {
        self.push_undo_snapshot();
        use xtce_core::model::container::{Comparison, ComparisonOperator, RestrictionCriteria};
        let operator = match operator_cursor {
            0 => ComparisonOperator::Equality,
            1 => ComparisonOperator::Inequality,
            2 => ComparisonOperator::LessThan,
            3 => ComparisonOperator::LessThanOrEqual,
            4 => ComparisonOperator::GreaterThan,
            _ => ComparisonOperator::GreaterThanOrEqual,
        };
        if let NodeId::TmContainer(path, name) = node_id {
            if let Some(c) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.telemetry.as_mut())
                .and_then(|tm| tm.containers.get_mut(name.as_str()))
            {
                if let Some(base) = c.base_container.as_mut() {
                    base.restriction_criteria = Some(RestrictionCriteria::Comparison(Comparison {
                        parameter_ref,
                        value,
                        comparison_operator: operator,
                        use_calibrated_value: false,
                    }));
                }
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    // ── Entry location editor ─────────────────────────────────────────────────

    fn start_entry_location(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::TmContainer(path, name) = node.node_id.clone() else { return };
        let entry_list = get_ss(&self.space_system, &path)
            .and_then(|ss| ss.telemetry.as_ref())
            .and_then(|tm| tm.containers.get(name.as_str()))
            .map(|c| &c.entry_list);
        let Some(entry_list) = entry_list else { return };
        if entry_list.is_empty() { return; }

        let items: Vec<(String, String)> = entry_list
            .iter()
            .enumerate()
            .map(|(i, e)| {
                use xtce_core::model::container::SequenceEntry;
                let label = match e {
                    SequenceEntry::ParameterRef(r) => r.parameter_ref.clone(),
                    SequenceEntry::ContainerRef(r) => format!("[{}]", r.container_ref),
                    SequenceEntry::FixedValue(r) => format!("<fixed {}b>", r.size_in_bits),
                    SequenceEntry::ArrayParameterRef(r) => format!("{}[]", r.parameter_ref),
                };
                (format!("{}: {}", i, label), i.to_string())
            })
            .collect();

        self.entry_location_state = Some(EntryLocationState {
            node_id: node.node_id.clone(),
            step: EntryLocationStep::PickEntry { items, cursor: 0 },
        });
    }

    fn entry_location_move(&mut self, delta: i64) {
        let Some(els) = self.entry_location_state.as_mut() else { return };
        if let EntryLocationStep::PickEntry { items, cursor } = &mut els.step {
            let count = items.len();
            if count > 0 {
                *cursor = (*cursor as i64 + delta).clamp(0, count as i64 - 1) as usize;
            }
        }
    }

    fn entry_location_push_char(&mut self, c: char) {
        let Some(els) = self.entry_location_state.as_mut() else { return };
        match &mut els.step {
            EntryLocationStep::PickEntry { items, cursor } => {
                if c == 'j' { let n = items.len(); if n > 0 { *cursor = (*cursor + 1).min(n - 1); } }
                else if c == 'k' { *cursor = cursor.saturating_sub(1); }
            }
            EntryLocationStep::EnterOffset { buffer, .. } => {
                // Allow digits, '-' sign at start, and delete via backspace.
                if c == '-' && buffer.is_empty() {
                    buffer.push(c);
                } else if c.is_ascii_digit() {
                    buffer.push(c);
                }
            }
        }
    }

    fn entry_location_pop_char(&mut self) {
        let Some(els) = self.entry_location_state.as_mut() else { return };
        if let EntryLocationStep::EnterOffset { buffer, .. } = &mut els.step {
            buffer.pop();
        }
    }

    fn entry_location_confirm_step(&mut self) {
        let Some(els) = self.entry_location_state.take() else { return };
        let node_id = els.node_id;
        match els.step {
            EntryLocationStep::PickEntry { items, cursor } => {
                let idx = cursor.min(items.len().saturating_sub(1));
                let entry_index: usize = items[idx].1.parse().unwrap_or(0);
                let entry_name = items[idx].0.trim_start_matches(|c: char| c.is_ascii_digit() || c == ':' || c == ' ').to_string();
                self.entry_location_state = Some(EntryLocationState {
                    node_id,
                    step: EntryLocationStep::EnterOffset {
                        entry_index,
                        entry_name,
                        buffer: String::new(),
                    },
                });
            }
            EntryLocationStep::EnterOffset { entry_index, buffer, .. } => {
                let trimmed = buffer.trim();
                if trimmed.is_empty() || trimmed == "-" { return; }
                let Ok(bit_offset) = trimmed.parse::<i64>() else { return };
                self.commit_entry_location(&node_id, entry_index, bit_offset);
            }
        }
    }

    fn commit_entry_location(&mut self, node_id: &NodeId, entry_index: usize, bit_offset: i64) {
        use xtce_core::model::container::{EntryLocation, ReferenceLocation, SequenceEntry};
        self.push_undo_snapshot();
        let loc = EntryLocation { reference_location: ReferenceLocation::ContainerStart, bit_offset };
        if let NodeId::TmContainer(path, name) = node_id {
            if let Some(c) = get_ss_mut(&mut self.space_system, path)
                .and_then(|ss| ss.telemetry.as_mut())
                .and_then(|tm| tm.containers.get_mut(name.as_str()))
            {
                if let Some(entry) = c.entry_list.get_mut(entry_index) {
                    match entry {
                        SequenceEntry::ParameterRef(e)     => e.location = Some(loc),
                        SequenceEntry::ContainerRef(e)     => e.location = Some(loc),
                        SequenceEntry::FixedValue(e)       => e.location = Some(loc),
                        SequenceEntry::ArrayParameterRef(e) => e.location = Some(loc),
                    }
                }
            }
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
        self.rebuild_tree();
    }

    // ── Encoding wizard ───────────────────────────────────────────────────────

    fn start_encoding(&mut self) {
        use xtce_core::model::telemetry::ParameterType;
        use xtce_core::model::command::ArgumentType;
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let node_id = node.node_id.clone();

        let is_float = match &node_id {
            NodeId::TmParameterType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .and_then(|tm| tm.parameter_types.get(name.as_str()))
                .map(|pt| matches!(pt, ParameterType::Float(_)))
                .unwrap_or(false),
            NodeId::CmdArgumentType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.command.as_ref())
                .and_then(|cmd| cmd.argument_types.get(name.as_str()))
                .map(|at| matches!(at, ArgumentType::Float(_)))
                .unwrap_or(false),
            _ => return,
        };

        let is_encodable = match &node_id {
            NodeId::TmParameterType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.telemetry.as_ref())
                .and_then(|tm| tm.parameter_types.get(name.as_str()))
                .map(|pt| !matches!(pt, ParameterType::Aggregate(_) | ParameterType::Array(_)))
                .unwrap_or(false),
            NodeId::CmdArgumentType(path, name) => get_ss(&self.space_system, path)
                .and_then(|ss| ss.command.as_ref())
                .and_then(|cmd| cmd.argument_types.get(name.as_str()))
                .map(|at| !matches!(at, ArgumentType::Aggregate(_) | ArgumentType::Array(_)))
                .unwrap_or(false),
            _ => false,
        };
        if !is_encodable { return; }

        let first_step = if is_float {
            EncodingStep::FloatSizeSelect { cursor: 0 }
        } else {
            EncodingStep::IntegerFormatSelect { cursor: 0 }
        };
        self.encoding_state = Some(EncodingState { node_id, step: first_step });
    }

    fn encoding_move(&mut self, delta: i64) {
        let Some(es) = self.encoding_state.as_mut() else { return };
        match &mut es.step {
            EncodingStep::IntegerFormatSelect { cursor } => {
                let max = integer_encoding_labels().len() - 1;
                *cursor = (*cursor as i64 + delta).clamp(0, max as i64) as usize;
            }
            EncodingStep::FloatSizeSelect { cursor } => {
                let max = float_size_labels().len() - 1;
                *cursor = (*cursor as i64 + delta).clamp(0, max as i64) as usize;
            }
            EncodingStep::IntegerSizePrompt { .. } => {}
        }
    }

    fn encoding_push_char(&mut self, c: char) {
        let Some(es) = self.encoding_state.as_mut() else { return };
        match &mut es.step {
            EncodingStep::IntegerFormatSelect { cursor } => {
                let max = integer_encoding_labels().len() - 1;
                if c == 'j' { *cursor = (*cursor + 1).min(max); }
                else if c == 'k' { *cursor = cursor.saturating_sub(1); }
            }
            EncodingStep::FloatSizeSelect { cursor } => {
                let max = float_size_labels().len() - 1;
                if c == 'j' { *cursor = (*cursor + 1).min(max); }
                else if c == 'k' { *cursor = cursor.saturating_sub(1); }
            }
            EncodingStep::IntegerSizePrompt { buffer, .. } => {
                if c.is_ascii_digit() { buffer.push(c); }
            }
        }
    }

    fn encoding_pop_char(&mut self) {
        let Some(es) = self.encoding_state.as_mut() else { return };
        if let EncodingStep::IntegerSizePrompt { buffer, .. } = &mut es.step {
            buffer.pop();
        }
    }

    fn encoding_confirm_step(&mut self) {
        let Some(es) = self.encoding_state.take() else { return };
        let node_id = es.node_id;
        match es.step {
            EncodingStep::IntegerFormatSelect { cursor } => {
                self.encoding_state = Some(EncodingState {
                    node_id,
                    step: EncodingStep::IntegerSizePrompt { format_cursor: cursor, buffer: String::new() },
                });
            }
            EncodingStep::IntegerSizePrompt { format_cursor, buffer } => {
                let size: u32 = match buffer.trim().parse::<u32>() {
                    Ok(v) if v > 0 => v,
                    _ => {
                        self.encoding_state = Some(EncodingState {
                            node_id,
                            step: EncodingStep::IntegerSizePrompt { format_cursor, buffer },
                        });
                        return;
                    }
                };
                let encoding = cursor_to_integer_encoding(format_cursor);
                self.apply_integer_encoding(&node_id, encoding, size);
            }
            EncodingStep::FloatSizeSelect { cursor } => {
                let size = cursor_to_float_size(cursor);
                self.apply_float_encoding(&node_id, size);
            }
        }
    }

    fn apply_integer_encoding(
        &mut self,
        node_id: &NodeId,
        encoding: xtce_core::model::types::IntegerEncoding,
        size_in_bits: u32,
    ) {
        self.push_undo_snapshot();
        use xtce_core::model::types::IntegerDataEncoding;
        use xtce_core::model::telemetry::ParameterType;
        use xtce_core::model::command::ArgumentType;
        let enc = IntegerDataEncoding { size_in_bits, encoding, byte_order: None, default_calibrator: None };
        match node_id {
            NodeId::TmParameterType(path, name) => {
                if let Some(pt) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    match pt {
                        ParameterType::Integer(t)    => t.encoding = Some(enc),
                        ParameterType::Enumerated(t) => t.encoding = Some(enc),
                        ParameterType::Boolean(t)    => t.encoding = Some(enc),
                        _ => return,
                    }
                }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(at) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    match at {
                        ArgumentType::Integer(t)    => t.encoding = Some(enc),
                        ArgumentType::Enumerated(t) => t.encoding = Some(enc),
                        ArgumentType::Boolean(t)    => t.encoding = Some(enc),
                        _ => return,
                    }
                }
            }
            _ => return,
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
    }

    fn apply_float_encoding(
        &mut self,
        node_id: &NodeId,
        size: xtce_core::model::types::FloatSizeInBits,
    ) {
        self.push_undo_snapshot();
        use xtce_core::model::types::{FloatDataEncoding, FloatEncoding};
        use xtce_core::model::telemetry::ParameterType;
        use xtce_core::model::command::ArgumentType;
        let enc = FloatDataEncoding { size_in_bits: size, encoding: FloatEncoding::IEEE754_1985, byte_order: None, default_calibrator: None };
        match node_id {
            NodeId::TmParameterType(path, name) => {
                if let Some(ParameterType::Float(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    t.encoding = Some(enc);
                }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(ArgumentType::Float(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    t.encoding = Some(enc);
                }
            }
            _ => return,
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
    }

    // ── MetaCommand argument management ───────────────────────────────────────

    fn start_arg_add(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::CmdMetaCommand(path, mc_name) = node.node_id.clone() else { return };
        self.create_state = Some(CreateState {
            kind: CreateKind::Argument,
            target_path: path,
            target_name: Some(mc_name),
            step: CreateStep::NamePrompt { buffer: String::new(), variant: None },
        });
        self.create_error = None;
    }

    fn remove_last_argument(&mut self) {
        if self.focus != Focus::Tree { return; }
        let Some(node) = self.tree.get(self.cursor) else { return };
        let NodeId::CmdMetaCommand(path, mc_name) = node.node_id.clone() else { return };
        let pre = self.space_system.clone();
        let removed = get_ss_mut(&mut self.space_system, &path)
            .and_then(|ss| ss.command.as_mut())
            .and_then(|cmd| cmd.meta_commands.get_mut(mc_name.as_str()))
            .and_then(|mc| mc.argument_list.pop())
            .is_some();
        self.commit_snapshot_if_changed(pre, removed);
        if removed {
            self.dirty = true;
            self.validation_errors = xtce_core::validator::validate(&self.space_system);
            self.rebuild_tree();
        }
    }

    // ── Enumeration entry editing ─────────────────────────────────────────────

    fn enum_entry_confirm_step(&mut self) {
        let Some(es) = self.enum_entry_state.take() else { return };
        let node_id = es.node_id;
        match es.step {
            EnumEntryStep::ValuePrompt { buffer } => {
                match buffer.trim().parse::<i64>() {
                    Ok(value) => {
                        self.enum_entry_state = Some(EnumEntryState {
                            node_id,
                            step: EnumEntryStep::LabelPrompt { value, buffer: String::new() },
                        });
                    }
                    Err(_) => {
                        self.enum_entry_state = Some(EnumEntryState {
                            node_id,
                            step: EnumEntryStep::ValuePrompt { buffer },
                        });
                    }
                }
            }
            EnumEntryStep::LabelPrompt { value, buffer } => {
                let label = buffer.trim().to_string();
                if label.is_empty() {
                    self.enum_entry_state = Some(EnumEntryState {
                        node_id,
                        step: EnumEntryStep::LabelPrompt { value, buffer },
                    });
                    return;
                }
                self.commit_enum_entry(&node_id, value, label);
            }
        }
    }

    fn enum_entry_push_char(&mut self, c: char) {
        let Some(es) = self.enum_entry_state.as_mut() else { return };
        match &mut es.step {
            EnumEntryStep::ValuePrompt { buffer } => {
                if c.is_ascii_digit() || (c == '-' && buffer.is_empty()) {
                    buffer.push(c);
                }
            }
            EnumEntryStep::LabelPrompt { buffer, .. } => { buffer.push(c); }
        }
    }

    fn enum_entry_pop_char(&mut self) {
        let Some(es) = self.enum_entry_state.as_mut() else { return };
        match &mut es.step {
            EnumEntryStep::ValuePrompt { buffer } => { buffer.pop(); }
            EnumEntryStep::LabelPrompt { buffer, .. } => { buffer.pop(); }
        }
    }

    fn commit_enum_entry(&mut self, node_id: &NodeId, value: i64, label: String) {
        self.push_undo_snapshot();
        use xtce_core::model::types::ValueEnumeration;
        use xtce_core::model::telemetry::ParameterType;
        use xtce_core::model::command::ArgumentType;
        let entry = ValueEnumeration { value, label, max_value: None, short_description: None };
        match node_id {
            NodeId::TmParameterType(path, name) => {
                if let Some(ParameterType::Enumerated(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.telemetry.as_mut())
                    .and_then(|tm| tm.parameter_types.get_mut(name.as_str()))
                {
                    t.enumeration_list.push(entry);
                }
            }
            NodeId::CmdArgumentType(path, name) => {
                if let Some(ArgumentType::Enumerated(t)) = get_ss_mut(&mut self.space_system, path)
                    .and_then(|ss| ss.command.as_mut())
                    .and_then(|cmd| cmd.argument_types.get_mut(name.as_str()))
                {
                    t.enumeration_list.push(entry);
                }
            }
            _ => return,
        }
        self.dirty = true;
        self.validation_errors = xtce_core::validator::validate(&self.space_system);
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

pub(crate) fn integer_encoding_labels() -> &'static [&'static str] {
    &["Unsigned", "Twos Complement", "Sign Magnitude", "Ones Complement", "BCD", "Packed BCD"]
}

pub(crate) fn float_size_labels() -> &'static [&'static str] {
    &["32-bit (F32)", "64-bit (F64)", "128-bit (F128)"]
}

fn cursor_to_integer_encoding(cursor: usize) -> xtce_core::model::types::IntegerEncoding {
    use xtce_core::model::types::IntegerEncoding;
    match cursor {
        1 => IntegerEncoding::TwosComplement,
        2 => IntegerEncoding::SignMagnitude,
        3 => IntegerEncoding::OnesComplement,
        4 => IntegerEncoding::BCD,
        5 => IntegerEncoding::PackedBCD,
        _ => IntegerEncoding::Unsigned,
    }
}

fn cursor_to_float_size(cursor: usize) -> xtce_core::model::types::FloatSizeInBits {
    use xtce_core::model::types::FloatSizeInBits;
    match cursor {
        1 => FloatSizeInBits::F64,
        2 => FloatSizeInBits::F128,
        _ => FloatSizeInBits::F32,
    }
}

/// Pick a value from a filtered list.  Returns `None` if the filtered list is empty.
fn pick_from_filtered(items: &[(String, String)], filter: &str, cursor: usize) -> Option<String> {
    let q = filter.to_lowercase();
    let filtered: Vec<_> = items
        .iter()
        .filter(|(label, _)| q.is_empty() || label.to_lowercase().contains(&q))
        .collect();
    if filtered.is_empty() {
        None
    } else {
        Some(filtered[cursor.min(filtered.len() - 1)].1.clone())
    }
}

/// Count items in `items` whose display label contains `filter` (case-insensitive).
fn filtered_count(items: &[(String, String)], filter: &str) -> usize {
    if filter.is_empty() {
        items.len()
    } else {
        let q = filter.to_lowercase();
        items.iter().filter(|(label, _)| label.to_lowercase().contains(&q)).count()
    }
}

fn make_parameter_type(
    v: TypeVariant,
    name: &str,
    type_ref: Option<&str>,
) -> xtce_core::model::telemetry::ParameterType {
    use xtce_core::model::telemetry::*;
    match v {
        TypeVariant::Integer   => ParameterType::Integer(IntegerParameterType::new(name)),
        TypeVariant::Float     => ParameterType::Float(FloatParameterType::new(name)),
        TypeVariant::Enumerated => ParameterType::Enumerated(EnumeratedParameterType::new(name)),
        TypeVariant::Boolean   => ParameterType::Boolean(BooleanParameterType::new(name)),
        TypeVariant::String    => ParameterType::String(StringParameterType::new(name)),
        TypeVariant::Binary    => ParameterType::Binary(BinaryParameterType::new(name)),
        TypeVariant::Aggregate => ParameterType::Aggregate(AggregateParameterType::new(name)),
        TypeVariant::Array     => ParameterType::Array(ArrayParameterType::new(name, type_ref.unwrap_or(""))),
    }
}

fn make_argument_type(
    v: TypeVariant,
    name: &str,
    type_ref: Option<&str>,
) -> xtce_core::model::command::ArgumentType {
    use xtce_core::model::command::*;
    match v {
        TypeVariant::Integer   => ArgumentType::Integer(IntegerArgumentType::new(name)),
        TypeVariant::Float     => ArgumentType::Float(FloatArgumentType::new(name)),
        TypeVariant::Enumerated => ArgumentType::Enumerated(EnumeratedArgumentType::new(name)),
        TypeVariant::Boolean   => ArgumentType::Boolean(BooleanArgumentType::new(name)),
        TypeVariant::String    => ArgumentType::String(StringArgumentType::new(name)),
        TypeVariant::Binary    => ArgumentType::Binary(BinaryArgumentType::new(name)),
        TypeVariant::Aggregate => ArgumentType::Aggregate(AggregateArgumentType::new(name)),
        TypeVariant::Array     => ArgumentType::Array(ArrayArgumentType::new(name, type_ref.unwrap_or(""))),
    }
}

fn parameter_type_variant_label(pt: &xtce_core::model::telemetry::ParameterType) -> &'static str {
    use xtce_core::model::telemetry::ParameterType;
    match pt {
        ParameterType::Integer(_)    => "Integer",
        ParameterType::Float(_)      => "Float",
        ParameterType::Enumerated(_) => "Enumerated",
        ParameterType::Boolean(_)    => "Boolean",
        ParameterType::String(_)     => "String",
        ParameterType::Binary(_)     => "Binary",
        ParameterType::Aggregate(_)  => "Aggregate",
        ParameterType::Array(_)      => "Array",
    }
}

fn argument_type_variant_label(at: &xtce_core::model::command::ArgumentType) -> &'static str {
    use xtce_core::model::command::ArgumentType;
    match at {
        ArgumentType::Integer(_)    => "Integer",
        ArgumentType::Float(_)      => "Float",
        ArgumentType::Enumerated(_) => "Enumerated",
        ArgumentType::Boolean(_)    => "Boolean",
        ArgumentType::String(_)     => "String",
        ArgumentType::Binary(_)     => "Binary",
        ArgumentType::Aggregate(_)  => "Aggregate",
        ArgumentType::Array(_)      => "Array",
    }
}
