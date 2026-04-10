# Contributing

This document covers how to build the project, the testing expectations, and
step-by-step guides for the most common extension tasks.

---

## Building and testing

```sh
# Build everything
cargo build --workspace

# Run all tests (unit + integration)
cargo test --workspace

# Run only xtce-core tests
cargo test -p xtce-core

# Check for warnings without building artefacts
cargo check --workspace

# Generate and open rustdoc
cargo doc --workspace --open
```

Optional runtime dependency: `xmllint` (part of `libxml2-utils` on
Debian/Ubuntu, `libxml2` on Arch Linux, `libxml2` via Homebrew on macOS).
Only needed for `schema_validator::validate_schema`.  Its absence causes a
graceful error, not a build failure.

---

## Code style

- Follow standard `rustfmt` formatting (`cargo fmt --all`).
- Use `///` doc comments on all public items and `//!` module-level docs on
  every file.  See [Rust by Example — Documentation][rbe-doc] for the
  conventions.  The `# Arguments`, `# Returns`, `# Errors`, and `# Examples`
  section headings are the standard way to structure longer doc comments.
- New functions visible to other crates should have a doc comment.  Private
  helpers inside a module can use regular `//` comments, but document anything
  non-obvious.
- Prefer `Option` over sentinel values; prefer named fields over bare tuples.
- Keep `app.rs` action handlers small — delegate to `self.do_thing()` methods
  rather than putting logic inline in `apply_action`.

[rbe-doc]: https://doc.rust-lang.org/rust-by-example/meta/doc.html

---

## Adding a new XTCE element type

Adding support for a new XTCE XML element touches all three crates in order.
The full pipeline is: **model → parser → serializer → validator → TUI**.

As a worked example, imagine adding `CustomParameterType`.

### 1. Model (`xtce-core/src/model/`)

Add a struct (or variant) in the appropriate model file.

```rust
// in xtce-core/src/model/telemetry.rs

/// A custom parameter type with a single string property.
#[derive(Debug, Clone, PartialEq)]
pub struct CustomParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub custom_field: String,
}

// Add the variant to the existing ParameterType enum:
pub enum ParameterType {
    // ... existing variants ...
    Custom(CustomParameterType),
}
```

Update any `match` on `ParameterType` that the compiler flags — most are in
`model/telemetry.rs` itself (the `name`, `set_name`, `short_description`,
etc. forwarding methods).

### 2. Parser (`xtce-core/src/parser/types.rs`)

Add a branch in `parse_parameter_type_set` for the new element name, and
implement a `parse_custom_parameter_type` function:

```rust
b"CustomParameterType" => {
    let pt = parse_custom_parameter_type(ctx, &e)?;
    tm.parameter_types.insert(pt.name.clone(), ParameterType::Custom(pt));
}
```

```rust
fn parse_custom_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart,
) -> Result<CustomParameterType, ParseError> {
    let name = attr_str(start, "name")?.to_string();
    let short_description = opt_attr_str(start, "shortDescription")?.map(str::to_string);
    let custom_field = attr_str(start, "customField")?.to_string();
    // consume children (or just close the element)
    ctx.skip_element()?;
    Ok(CustomParameterType { name, short_description, custom_field })
}
```

### 3. Serializer (`xtce-core/src/serializer.rs`)

Add an arm in `write_parameter_type_set` and implement a write function:

```rust
ParameterType::Custom(t) => write_custom_parameter_type(w, t)?,
```

```rust
/// Serialize a `<CustomParameterType>` element.
fn write_custom_parameter_type(w: &mut W, t: &CustomParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("CustomParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    e.push_attribute(("customField", t.custom_field.as_str()));
    w.write_event(Event::Empty(e))?;
    Ok(())
}
```

### 4. Validator (`xtce-core/src/validator.rs`)

Add arms in the `ParameterType` match inside `validate_parameter_type_set`
(for checking any references the type contains) and in `pt_base_type` (for
inheritance checking):

```rust
ParameterType::Custom(_) => {} // no references to check
```

### 5. TUI detail panel (`xtce-tui/src/ui/detail.rs`)

Add a `ParameterType::Custom` arm in `detail_parameter_type`:

```rust
ParameterType::Custom(t) => detail_custom_pt(t, lines),
```

```rust
fn detail_custom_pt(t: &CustomParameterType, lines: &mut Vec<Line<'static>>) {
    lines.push(field_line("Custom field", &t.custom_field));
}
```

Also update `pt_size_in_bits` (return `None` or a known size) and the `pt_kind`
function in `xtce-tui/src/ui/tree.rs`.

### 6. Tests

Add a round-trip test in `xtce-core/src/parser/mod.rs` (or next to the
relevant module) that parses a minimal XML snippet containing the new element
and asserts that the model fields are populated correctly:

```rust
#[test]
fn parse_custom_parameter_type() {
    let xml = r#"<SpaceSystem name="Root">
        <TelemetryMetaData>
            <ParameterTypeSet>
                <CustomParameterType name="MyCustom" customField="hello"/>
            </ParameterTypeSet>
        </TelemetryMetaData>
    </SpaceSystem>"#;
    let ss = parse(xml.as_bytes()).unwrap();
    let pt = ss.telemetry.unwrap().parameter_types.get("MyCustom").unwrap();
    assert!(matches!(pt, ParameterType::Custom(t) if t.custom_field == "hello"));
}
```

---

## Adding a new keybinding

1. **Add the `Action` variant** in `xtce-tui/src/event.rs`:

   ```rust
   /// Brief description of what this action does.
   MyNewAction,
   ```

2. **Map a key** in the appropriate `*_key_to_action` function in the same
   file.  Choose the right layer:
   - `key_to_action` — Explore mode (navigation, file ops, read-only actions).
   - `edit_mode_key_to_action` — Edit mode (mutation operations).
   - A sub-modal function — only active while that wizard is open.

   ```rust
   (KeyCode::Char('X'), _) => Some(Action::MyNewAction),
   ```

3. **Handle the action** in `App::apply_action` in `xtce-tui/src/app.rs`.
   Add it to the main `match action { … }` block, or to the relevant
   sub-modal guard section near the top.

4. **Update the help overlay** in `xtce-tui/src/ui/mod.rs`
   (`render_help_overlay`'s `bindings` array) and the status bar hint if
   appropriate.

5. **Update README.md** if the keybinding is user-facing.

---

## Adding a new `xtce-tools` subcommand

1. **Add a variant** to the `Commands` enum in `xtce-tools/src/main.rs`:

   ```rust
   /// One-line help shown by `xtce-tools --help`.
   MyCommand {
       /// Path to the XTCE XML file.
       input: PathBuf,
       // ... other CLI args ...
   }
   ```

2. **Add a match arm** in `main`:

   ```rust
   Commands::MyCommand { input, .. } => {
       let ss = load_xtce(&input);
       let output = my_command::generate(&ss);
       // write output ...
   }
   ```

3. **Create a module** `xtce-tools/src/my_command.rs`.  Start with a
   `//!` module doc comment explaining what the subcommand produces.
   Implement `generate(root: &SpaceSystem) -> T`.

4. **Write tests** for the generator logic, ideally using the helpers in
   `layout.rs` tests as a template.

---

## Changing the TUI layout or adding a new overlay

- New overlays are rendered last in `ui::render()` so they appear on top.
  Use `frame.render_widget(Clear, area)` before drawing into the area.
- Use `centered_rect(width_pct, height_pct, frame.area())` for centred
  floating panels.
- If the overlay needs to intercept input, add a `Option<MyOverlayState>`
  field to `App`, guard it in `App::apply_action`, and add it to the modal
  priority chain in `main.rs: map_key()`.
- If the overlay renders differently depending on which node is selected,
  pass `app.tree.get(app.cursor)` into the render function.

---

## Parser conventions

- Use `attr_str(start, "attrName")?` for required attributes.
- Use `opt_attr_str(start, "attrName")?` for optional attributes.
- Call `ctx.skip_element()` for elements whose children you do not yet
  parse, so the event stream is consumed and the parent loop can continue.
- Unknown child elements inside a known parent should call `ctx.skip_element()`
  rather than returning an error, for forward-compatibility.

---

## Serializer conventions

- Name write functions `write_<XmlElementName>` (camelCase element name,
  snake_case function name).
- Use `Event::Empty` when an element has no children; use `Event::Start` +
  children + `Event::End` when it does.  Check `has_children` before
  choosing, as in `write_header`.
- The `wt(w, "Tag", text)` helper writes `<Tag>text</Tag>` in one call.
- The namespace attribute (`xmlns=…`) is written only on the root
  `<SpaceSystem>` element; do not add it to child elements.
