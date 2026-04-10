# xtce-editor

A Rust workspace for reading, editing, validating, and generating tooling from
[XTCE](https://www.omg.org/spec/XTCE/) (XML Telemetry and Command Exchange)
documents. XTCE is an OMG/CCSDS standard used in the space industry to describe
the telemetry and command interface of a spacecraft or ground system.

## Workspace layout

```
xtce-editor/
├── xtce-core/      # Library: parser, in-memory model, serializer, validator
├── xtce-tui/       # Binary: terminal UI editor
└── xtce-tools/     # Binary: CLI utility tools
```

---

## xtce-core

The foundational library. All other crates depend on it.

### Modules

| Module | Purpose |
|---|---|
| `model` | Pure Rust structs mirroring the XTCE element hierarchy |
| `parser` | XML → model (using `quick-xml`) |
| `serializer` | model → XTCE v1.2 XML bytes (round-trip safe) |
| `validator` | Reference resolution, uniqueness checks, cycle detection |
| `schema_validator` | Full XSD validation via `xmllint` (XTCE v1.2 XSD is bundled) |
| `error` | `ParseError`, `ValidationError`, `XtceError` |

### Model overview

```
SpaceSystem                    (root, may nest recursively)
├── Header                     (version, date, classification, authors)
├── TelemetryMetaData
│   ├── ParameterType[]        (Integer, Float, Enumerated, Boolean,
│   │                           String, Binary, Aggregate, Array)
│   ├── Parameter[]            (name + typeRef + properties)
│   └── SequenceContainer[]    (packet structures with SequenceEntry list)
└── CommandMetaData
    ├── ArgumentType[]         (same variants as ParameterType)
    ├── MetaCommand[]          (with Argument list and CommandContainer)
    └── CommandContainer[]
```

All collections use `IndexMap` to preserve document order.

### Validation

`xtce_core::validator::validate` performs a full structural walk and checks:

- **Unresolved references** — every `parameterTypeRef`, `containerRef`,
  `argumentTypeRef`, `baseType`, `parameterRef`, and `argumentRef` must resolve
  to a name defined in the current SpaceSystem or any ancestor.
- **Duplicate names** — names must be unique within a SpaceSystem scope.
- **Cyclic inheritance** — base-container chains, base-command chains, and type
  `base_type` chains must be acyclic.

Scoping follows XTCE inheritance: a child SpaceSystem sees all names its parents
define.

`xtce_core::schema_validator::validate_schema` additionally validates serialized
XML against the official XTCE v1.2 XSD using `xmllint`. Requires `xmllint` to
be installed; returns an error (not a panic) if it is missing.

### Quick usage

```rust
use xtce_core::{parser, validator, serializer};

// Parse
let ss = parser::parse_file("my_mission.xml")?;

// Validate
let errors = validator::validate(&ss);
for e in &errors { eprintln!("{e}"); }

// Round-trip serialize
let xml_bytes = serializer::serialize(&ss)?;
std::fs::write("out.xml", &xml_bytes)?;
```

---

## xtce-tui

An interactive terminal editor for XTCE files, built with
[Ratatui](https://ratatui.rs/) and Crossterm.

### Running

```sh
cargo run -p xtce-tui -- path/to/file.xml
```

### UI layout

- **Left panel** — collapsible tree view of the SpaceSystem hierarchy.
- **Right panel** — detail view for the selected node (fields, entries, etc.).
- Validation errors are shown in an overlay (`e`).
- A keybinding help overlay is available (`?`).

### Input modes

The TUI has two top-level modes:

- **Explore mode** (default) — navigation, search, reference following, file
  operations. Read-only; no mutations.
- **Edit mode** — entered with `m`, exposes all mutation operations. A
  floating overlay shows the available operations for the selected node. `Esc`
  returns to Explore.

Interactive sub-flows (create wizard, encoding picker, restriction editor, etc.)
are launched from Edit mode and use `Enter` to confirm and `Esc` to cancel.

### Keybindings — Explore mode

| Key | Action |
|---|---|
| `q` / `Ctrl-c` | Quit |
| `↑` / `k`, `↓` / `j` | Move cursor |
| `Ctrl-u` / `PgUp`, `Ctrl-d` / `PgDn` | Page up / down |
| `Enter` / `Space` | Toggle expand/collapse |
| `→` / `l`, `←` / `h` | Smart expand / collapse (or jump to parent) |
| `Tab` | Cycle panel focus (tree ↔ detail) |
| `f` | Follow reference (Parameter→Type, Type→users, Container→base) |
| `[` | Go back (return from last reference follow) |
| `/` | Open search prompt — type query, Enter to search |
| `n` / `N` | Next / previous search match |
| `s` / `Ctrl-w` | Save to disk |
| `u` | Undo last change |
| `Ctrl-r` | Redo |
| `r` | Reload from disk (prompts if unsaved changes) |
| `m` | Enter Edit mode |
| `e` | Toggle validation errors overlay |
| `?` | Toggle keybinding help overlay |
| `Esc` | Close active overlay |

### Keybindings — Edit mode

Press `m` to enter, `Esc` to return to Explore.  Navigation keys still work.

| Key | Action |
|---|---|
| `i` | Rename selected item |
| `C` | Edit short description |
| `a` | Create new item (guided wizard) |
| `d` | Delete selected item (with confirmation) |
| `A` | Add entry to container or MetaCommand |
| `x` | Remove last entry |
| `t` | Change type reference (picker) |
| `b` | Set base container / type / MetaCommand (picker) |
| `E` | Edit encoding (wizard) |
| `S` | Toggle signed flag (Integer/Float types) |
| `B` | Toggle abstract flag (containers / MetaCommands) |
| `D` | Cycle data source (Telemetered / Derived / Constant / Local) |
| `P` | Toggle read-only flag (Parameters) |
| `g` / `G` | Add / remove last Argument (MetaCommands) |
| `R` | Edit restriction criteria (containers) |
| `L` | Edit entry location / bit offset |
| `K` | Edit calibrator (polynomial or spline) |
| `U` | Edit unit set |

### Architecture notes

- `App` (`app.rs`) is the single source of truth: loaded `SpaceSystem`, undo/redo
  stack, tree expansion state, cursor position, and all transient edit states.
- `event.rs` maps raw crossterm `KeyEvent`s to typed `Action` values. Key
  bindings are defined entirely there — one place to remap everything.
- `ui/mod.rs` builds the `TreeNode` list from the model and delegates rendering
  to `tree.rs` (left panel) and `detail.rs` (right panel).
- Undo/redo is implemented as a `VecDeque` of `SpaceSystem` snapshots.

See [ARCHITECTURE.md](ARCHITECTURE.md) for a detailed description of the
internals of all three crates.

---

## xtce-tools

A CLI for generating artifacts from XTCE files.

### Running

```sh
cargo run -p xtce-tools -- <SUBCOMMAND> [OPTIONS]
```

### Subcommands

#### `gen-dissector`

Generates a Wireshark Lua dissector that decodes packets matching the leaf
containers defined in the XTCE file.

```sh
xtce-tools gen-dissector path/to/file.xml [--port 4321] [--output dissector.lua]
```

- Defaults to UDP port 4321.
- Output file defaults to `<input_stem>.lua`.

#### `gen-testdata`

Generates a PCAP file with one synthetic UDP packet per leaf container, useful
for testing the Lua dissector.

```sh
xtce-tools gen-testdata path/to/file.xml [--port 4321] [--output test.pcap]
```

- Output file defaults to `<input_stem>_test.pcap`.

"Leaf containers" are sequence containers that have no child containers
inheriting from them (i.e., the concrete packet types).

---

## Building

Requires a stable Rust toolchain (edition 2024 for `xtce-core`/`xtce-tui`,
edition 2021 for `xtce-tools`).

```sh
# Build everything
cargo build --workspace

# Run the TUI editor
cargo run -p xtce-tui -- examples/my_mission.xml

# Run the CLI tools
cargo run -p xtce-tools -- gen-dissector examples/my_mission.xml

# Run all tests
cargo test --workspace
```

Optional runtime dependency: `xmllint` (part of `libxml2-utils` on Debian/Ubuntu,
`libxml2` on Arch). Only needed if you call `schema_validator::validate_schema`.

---

## Contributing / extending

See [CONTRIBUTING.md](CONTRIBUTING.md) for step-by-step guides on:

- Adding a new XTCE element type (model → parser → serializer → validator → TUI)
- Adding a new keybinding
- Adding a new `xtce-tools` subcommand
- Parser and serializer conventions
- Testing expectations
