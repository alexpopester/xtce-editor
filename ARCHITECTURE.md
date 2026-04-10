# Architecture

This document describes the internal design of the xtce-editor workspace: how
the crates fit together, how data flows through the system, and the key design
decisions in each component.

---

## Workspace overview

```
xtce-editor/
├── xtce-core/   # Library: the shared XTCE data model + all I/O
├── xtce-tui/    # Binary: interactive terminal editor
└── xtce-tools/  # Binary: offline artifact generators (dissector, test PCAP)
```

Both binaries depend on `xtce-core` and nothing else crosses crate boundaries.
`xtce-tools` uses edition 2021; the other two crates use edition 2024.

---

## Data flow

```
disk XML
  │
  ▼ xtce_core::parser::parse_file()
SpaceSystem  (in-memory model)
  │
  ├─▶ xtce_core::validator::validate()  →  Vec<ValidationError>
  │
  ├─▶ xtce_core::serializer::serialize()  →  XML bytes  →  disk
  │
  ├─▶ xtce-tui:  App holds SpaceSystem, renders tree + detail, mutates on action
  │
  └─▶ xtce-tools:  layout::find_leaf_containers() → dissector / PCAP
```

---

## xtce-core

### Model (`src/model/`)

The model is a set of plain Rust structs that mirror the XTCE element
hierarchy.  There is no inheritance, no trait magic, and no XML knowledge
here — it is pure data.

Key design decisions:

- **`IndexMap` everywhere** instead of `HashMap`.  XTCE documents have a
  defined element order that must be preserved on round-trip serialization.
  `IndexMap` gives O(1) lookup while maintaining insertion order.

- **`ParameterType` and `ArgumentType` are enums**, not trait objects.  Each
  variant (Integer, Float, Enumerated, …) wraps a concrete struct.  This
  makes exhaustive matching straightforward and avoids `dyn Trait` overhead.

- **`Option<T>` for everything optional**.  No sentinel values or magic
  strings.  A field that is absent in the XML is `None` in the model.

The hierarchy mirrors XTCE:

```
SpaceSystem
├── Header
├── TelemetryMetaData
│   ├── parameter_types: IndexMap<String, ParameterType>
│   ├── parameters:      IndexMap<String, Parameter>
│   └── containers:      IndexMap<String, SequenceContainer>
├── CommandMetaData
│   ├── argument_types:  IndexMap<String, ArgumentType>
│   ├── meta_commands:   IndexMap<String, MetaCommand>
│   └── command_containers: IndexMap<String, CommandContainer>
└── sub_systems: Vec<SpaceSystem>   ← recursive
```

### Parser (`src/parser/`)

The parser turns XML bytes into a `SpaceSystem` using `quick-xml`'s
namespace-aware event reader.  It is a hand-written recursive-descent parser
— not serde-based — because XTCE elements have context-sensitive semantics
that a generic deserializer cannot express.

**`ParseContext`** (`context.rs`) wraps the `quick_xml::NsReader` and provides
a `next()` method that silently drops XML comments, processing instructions,
DOCTYPE declarations, and whitespace-only text nodes.  All parser functions
call `ctx.next()` rather than the raw reader.

**Entry point** (`mod.rs`): `parse_file` opens the file as a `BufReader` (no
full-file load into memory), creates a `ParseContext`, and calls
`find_root_and_parse`, which advances until it sees the root `<SpaceSystem>`
start tag and delegates to `space_system::parse_space_system`.

**Recursive descent** (`space_system.rs`, `telemetry.rs`, `command.rs`,
`container.rs`, `types.rs`): Each XTCE element has a corresponding
`parse_*` function.  The pattern is:

```
parse_element(ctx, start_event) → Result<T>
  read attributes from start_event
  loop {
    match ctx.next() {
      Start(child) => match child.local_name() { ... dispatch to child parser ... }
      End(_)       => break   ← back to caller
      _            => {}      ← skip text, comments, etc.
    }
  }
  build and return T
```

Because `quick-xml` is event-driven (like SAX), each parser consumes exactly
the events that belong to its element, leaving the stream positioned after the
closing tag for the caller to continue.

**Error handling**: `ParseError::UnexpectedElement` is returned for unknown
child elements at the top level. Unknown attributes are silently ignored (XTCE
has many optional attributes; strict rejection would break real-world files).

### Serializer (`src/serializer.rs`)

The serializer is the inverse of the parser: it walks the model tree and
emits `quick_xml` events.

**Naming convention**: every private function is named `write_<element_name>`
and takes `(w: &mut W, data: &T) -> Result<(), ParseError>`.  The `W` type
alias is `Writer<Vec<u8>>`.

**Empty vs. Start/End**: when an element has no children, `Event::Empty` is
used (`<Foo bar="x"/>`).  When it has children, `Event::Start` + children +
`Event::End` is used.  This avoids redundant close tags and keeps output
compact.

**Round-trip safety**: all information stored in the model is serialized back.
Attributes not modeled (e.g. rare optional XTCE attributes not yet
implemented) are lost on a round-trip, but all semantically significant data
is preserved.

**Namespace**: the XTCE namespace attribute (`xmlns=…`) is only written on the
root `<SpaceSystem>` element (`is_root: bool` parameter).

### Validator (`src/validator.rs`)

Validation is a two-phase recursive walk of the `SpaceSystem` tree.

**Phase 1 — scope building**: Before the main walk, `collect_all_containers`
scans the entire tree and adds every `SequenceContainer` and
`CommandContainer` name to a flat `HashSet`.  Containers are globally
scoped in XTCE (a child container can reference a container defined anywhere
in the tree), so we need the full picture upfront.

**Phase 2 — per-SpaceSystem validation**: `validate_space_system` is called
recursively.  It takes a `Scope` value that accumulates all names visible at
the current depth:

```
Scope {
    parameter_types: HashSet<&str>,
    parameters:      HashSet<&str>,
    containers:      HashSet<&str>,
    argument_types:  HashSet<&str>,
    meta_commands:   HashSet<&str>,
}
```

Each child SpaceSystem inherits its parent's scope and extends it with its own
names.  References are checked against `scope.parameter_types`, etc.  An
unresolved reference produces a `ValidationError::UnresolvedReference`.

**Cycle detection**: inheritance chains (container base, type base_type,
MetaCommand base) are checked by walking the chain and collecting visited
names in a `HashSet`.  A name that appears twice is a cycle.

**Self-referential base**: a container or type whose base is itself (`base ==
own_name`) is valid XTCE and is silently treated as standalone (no base).

### Schema validator (`src/schema_validator.rs`)

Shells out to `xmllint --schema xtce.xsd` with the bundled XTCE v1.2 XSD.
Returns a structured list of `ValidationError::SchemaError` values parsed
from xmllint's stdout.  If `xmllint` is not installed, returns an error rather
than panicking.  This is a separate step from structural validation (above)
and is only called after a successful save.

---

## xtce-tui

### Crate structure

```
main.rs          entry point: terminal init, loading screen, run loop
tui.rs           thin wrappers around ratatui terminal init/restore
app.rs           App struct: all runtime state + action dispatch
event.rs         key → Action mapping (one function per modal layer)
ui/
  mod.rs         top-level render() + overlay renderers
  tree.rs        NodeId, TreeNode, build_tree, enumerate_all_nodes
  detail.rs      detail panel renderer (field-level view of selected node)
  theme.rs       colour palette constants
```

### The event loop (`main.rs: run()`)

```
loop {
    terminal.draw(|frame| ui::render(app, frame))  ← render current state

    block until first KeyPress event               ← avoids busy-wait
    drain remaining events from OS queue (non-blocking poll)
    coalesce consecutive navigation events         ← stop key-repeat drift
    map each key event to an Action via map_key()
    for each action: app.apply_action(action)
}
```

**Key-repeat coalescing**: when a user holds `j`, the OS queues many
`KeyPress` events.  After lifting the key the queue may still contain dozens
of pending moves.  The drain loop replaces the last queued navigation
`Action` in-place rather than appending, so at most one move is processed
regardless of how many are queued.

**Panic hook**: the panic hook is installed before terminal init so that raw
mode is always restored if a panic occurs mid-session.

### Modal input dispatch (`main.rs: map_key()`)

Key events are mapped to `Action` values by a priority chain of checks on
`App` fields.  The first matching condition wins:

```
picker_state.is_some()          → picker_key_to_action
encoding_state.is_some()        → encoding_key_to_action
enum_entry_state.is_some()      → enum_entry_key_to_action
entry_location_state.is_some()  → entry_location_key_to_action
restriction_edit_state.is_some()→ restriction_edit_key_to_action
calibrator_state.is_some()      → calibrator_key_to_action
unit_edit_state.is_some()       → unit_edit_key_to_action
create_state.is_some()          → create_key_to_action
entry_add_state.is_some()       → entry_add_key_to_action
delete_confirm.is_some()        → delete_confirm_key_to_action
reload_confirm                  → reload_confirm_key_to_action
edit_state.is_some()            → edit_key_to_action
search_mode                     → search_key_to_action
mode == AppMode::Edit           → edit_mode_key_to_action
(default)                       → key_to_action   (Explore mode)
```

Sub-modals (pickers, wizards) take priority over the top-level mode.  Adding
a new sub-modal means inserting a new check near the top of this chain and
implementing a `*_key_to_action` function.

### `App` — application state (`app.rs`)

`App` is the single source of truth for all runtime state.  There are no
globals, no channels, no shared references.  The render functions receive
`&App` (or `&mut App` for overlays that measure their own height) and produce
output; `apply_action` receives `&mut App` and mutates state.

**Key fields**:

- `space_system` — the loaded XTCE model; mutated by edit operations.
- `tree: Vec<TreeNode>` — flattened, visible tree rows; rebuilt by
  `rebuild_tree()` after any model change or expansion toggle.
- `expanded: HashSet<NodeId>` — which nodes have their children visible.
- `cursor: usize` — index into `tree` for the selected row.
- `list_state: ListState` — ratatui's list scroll state (kept in sync with cursor).
- `undo_stack / redo_stack: VecDeque<SpaceSystem>` — full `SpaceSystem`
  snapshots.  Each mutation calls `push_undo_snapshot()` first.  Capped at 50
  entries.  This is simple and correct but memory-heavy for large documents;
  a diff-based approach would be more efficient if needed.
- `search_matches_set: HashSet<NodeId>` — O(1) lookup during tree render to
  decide whether to highlight a row.  Rebuilt only when the search is
  committed (Enter), not on each keypress.
- `nav_stack: Vec<NodeId>` — navigation history for `f` / `[` reference
  following.  Departure node is pushed before every `jump_to` call.

**`apply_action`**: a large match that checks active sub-modals first (each
returns early), then the overlay states (errors, help), then the main action
dispatch.  Following the same priority order as `map_key` ensures consistency.

**`jump_to(NodeId)`**: expands all ancestors of the target, rebuilds the tree,
moves the cursor, and centers the view by setting the ratatui `ListState`
offset to `cursor - (tree_panel_height / 2)`.

### TUI sub-modal state machines

Each interactive editor is a small state machine stored as `Option<XxxState>`
on `App`.  When the field is `Some`, that modal intercepts all input.
`Esc` / cancel transitions always set the field back to `None`.

| State field | Steps | What it edits |
|---|---|---|
| `edit_state` | Single text buffer | Name or description of selected node |
| `create_state` | TypeVariantSelect → NamePrompt → PickerPrompt | Add a new item |
| `entry_add_state` | ContainerTypeSelect → Picker | Add an entry to a container/MetaCommand |
| `delete_confirm` | Single y/n prompt | Confirm deletion |
| `picker_state` | Single filterable list | Change type ref or set base |
| `encoding_state` | FormatSelect → SizePrompt | Set Integer/Float encoding |
| `enum_entry_state` | ValuePrompt → LabelPrompt | Add an enumeration entry |
| `entry_location_state` | PickEntry → EnterOffset | Set a field bit offset |
| `restriction_edit_state` | PickParameter → PickOperator → EnterValue | Set restriction criteria |
| `unit_edit_state` | Review → AddUnit | Add/remove units |
| `calibrator_state` | KindSelect → PolynomialReview/SplineReview → … | Set calibrator |

The `CreateStep::PickerPrompt` step is only reached for items that require a
type reference (Parameters need a `parameterTypeRef`; Array types need an
`arrayTypeRef`).  For items that don't require one, the create flow commits
after `NamePrompt`.

### Tree rendering (`ui/tree.rs`)

`NodeId` is the stable identity for every possible node.  It encodes the full
path through the SpaceSystem hierarchy plus the node type and name, so two
nodes with the same name in different SpaceSystems always compare unequal.

`build_tree` performs a depth-first walk of the `SpaceSystem` and emits a
`TreeNode` for every visible node (i.e. the node's parent is in `expanded`).
Section and group nodes (e.g. `TmSection`, `TmParameters`) are virtual — they
have no counterpart in the model but provide collapsible organization.

`enumerate_all_nodes` produces `(NodeId, label)` pairs for the entire
SpaceSystem regardless of expansion state.  This is the search index; it is
rebuilt whenever the model changes, not on every frame.

### Detail rendering (`ui/detail.rs`)

The detail panel is rebuilt from scratch every frame (no caching).  It reads
the selected `NodeId`, resolves it against `app.space_system`, and builds a
`Vec<Line>` of styled text.  Scroll is `app.detail_scroll` (line count).

---

## xtce-tools

### Pipeline

```
parse_file() → SpaceSystem
  │
  ▼ layout::SsIndex::build()
SsIndex { containers, param_types, param_type_refs }
  │
  ▼ layout::find_leaf_containers()
Vec<LeafContainer>   (each has resolved bit offsets for every field)
  │
  ├─▶ dissector::generate_lua()  →  Lua script
  └─▶ testdata::generate_pcap()  →  PCAP bytes
```

### Layout resolution (`layout.rs`)

**Leaf detection**: a container is a leaf if it is not abstract and no other
container names it as its `base_container`.  The non-leaf set is built as a
`HashSet` from a single pass over all containers.

**Field flattening** (`collect_entries`): recursively follows the
`base_container` chain, prepending base entries before child entries.
`ContainerRef` entries inline the referenced container's fields at the
appropriate offset.  A `visited: HashSet<String>` prevents infinite loops
on cyclic references (which the validator catches, but the tool handles
defensively).

**Offset computation** (`compute_offsets`): a sequential pass over the flat
field list.  `ContainerStart` locations are absolute from byte 0 of the
payload.  `PreviousEntry` locations are relative to the end of the previous
field.  Fields with no explicit location are packed sequentially.

### Dissector generation (`dissector.rs`)

Emits a Wireshark Lua script.  Each leaf container gets a `dissect_<name>`
function that extracts its fields from the packet buffer using
`ProtoField` declarations.  If all containers share a common equality
discriminator (e.g. an APID field), a top-level dispatch table routes to
the correct `dissect_*` function.

### Test data generation (`testdata.rs`)

Emits a libpcap-format file (little-endian, link type Ethernet).  Each leaf
container produces one packet: Ethernet II + IPv4 + UDP headers followed by
a synthetic payload where each field is set to a fixed pattern
(field index × 3, clamped to the field's bit width).
