//! Detail panel content builder.
//!
//! [`detail_lines`] maps the currently selected [`NodeId`] to a list of
//! styled [`Line`]s ready for display in a scrollable [`Paragraph`].

use ratatui::text::{Line, Span};

use xtce_core::model::{
    command::{
        AggregateArgumentType, ArgumentType, ArrayArgumentType, BinaryArgumentType,
        BooleanArgumentType, CommandMetaData, EnumeratedArgumentType, FloatArgumentType,
        IntegerArgumentType, MetaCommand, StringArgumentType,
    },
    container::{
        Comparison, ComparisonOperator, RestrictionCriteria, SequenceContainer, SequenceEntry,
    },
    space_system::SpaceSystem,
    telemetry::{
        AbsoluteTimeParameterType, AggregateParameterType, ArrayParameterType,
        BinaryParameterType, BooleanParameterType, DataSource, EnumeratedParameterType,
        FloatParameterType, IntegerParameterType, Parameter, ParameterType,
        RelativeTimeParameterType, StringParameterType, TelemetryMetaData, TimeEncoding,
    },
    types::{
        BinaryDataEncoding, BinarySize, ByteOrder, Calibrator, FloatDataEncoding, FloatEncoding,
        FloatSizeInBits, IntegerDataEncoding, IntegerEncoding, StringDataEncoding, StringEncoding,
        StringSize, Unit,
    },
};

use crate::app::App;
use crate::ui::{theme, tree::get_ss, NodeId};

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Build the list of styled lines for the detail panel based on the current
/// tree selection.  All returned lines own their strings (`'static`).
pub fn detail_lines(app: &App) -> Vec<Line<'static>> {
    let Some(node) = app.tree.get(app.cursor) else {
        return vec![note("Nothing selected")];
    };
    let root = &app.space_system;

    match &node.node_id {
        // ── SpaceSystem ───────────────────────────────────────────────────────
        NodeId::SpaceSystem(path) => match get_ss(root, path) {
            Some(ss) => detail_space_system(ss),
            None => vec![],
        },

        // ── Section summary nodes ─────────────────────────────────────────────
        NodeId::TmSection(path) => {
            let tm = get_ss(root, path).and_then(|s| s.telemetry.as_ref());
            detail_tm_section(tm)
        }
        NodeId::CmdSection(path) => {
            let cmd = get_ss(root, path).and_then(|s| s.command.as_ref());
            detail_cmd_section(cmd)
        }

        // ── Group list nodes ──────────────────────────────────────────────────
        NodeId::TmParameterTypes(path) => {
            let names = get_ss(root, path)
                .and_then(|s| s.telemetry.as_ref())
                .map(|tm| tm.parameter_types.keys().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            detail_group_names("Parameter Types", &names)
        }
        NodeId::TmParameters(path) => {
            let names = get_ss(root, path)
                .and_then(|s| s.telemetry.as_ref())
                .map(|tm| tm.parameters.keys().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            detail_group_names("Parameters", &names)
        }
        NodeId::TmContainers(path) => {
            let names = get_ss(root, path)
                .and_then(|s| s.telemetry.as_ref())
                .map(|tm| tm.containers.keys().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            detail_group_names("Containers", &names)
        }
        NodeId::CmdArgumentTypes(path) => {
            let names = get_ss(root, path)
                .and_then(|s| s.command.as_ref())
                .map(|cmd| cmd.argument_types.keys().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            detail_group_names("Argument Types", &names)
        }
        NodeId::CmdMetaCommands(path) => {
            let names = get_ss(root, path)
                .and_then(|s| s.command.as_ref())
                .map(|cmd| cmd.meta_commands.keys().map(String::as_str).collect::<Vec<_>>())
                .unwrap_or_default();
            detail_group_names("MetaCommands", &names)
        }

        // ── Leaf nodes ────────────────────────────────────────────────────────
        NodeId::TmParameterType(path, name) => {
            let pt = get_ss(root, path)
                .and_then(|s| s.telemetry.as_ref())
                .and_then(|tm| tm.parameter_types.get(name));
            pt.map(detail_parameter_type).unwrap_or_default()
        }
        NodeId::TmParameter(path, name) => {
            let ss = get_ss(root, path);
            let param = ss
                .and_then(|s| s.telemetry.as_ref())
                .and_then(|tm| tm.parameters.get(name));
            match (ss, param) {
                (Some(ss), Some(p)) => detail_parameter(p, ss, root),
                _ => vec![],
            }
        }
        NodeId::TmContainer(path, name) => {
            let ss = get_ss(root, path);
            let container = ss
                .and_then(|s| s.telemetry.as_ref())
                .and_then(|tm| tm.containers.get(name));
            match (ss, container) {
                (Some(ss), Some(c)) => detail_sequence_container(c, ss, root),
                _ => vec![],
            }
        }
        NodeId::CmdArgumentType(path, name) => {
            let at = get_ss(root, path)
                .and_then(|s| s.command.as_ref())
                .and_then(|cmd| cmd.argument_types.get(name));
            at.map(detail_argument_type).unwrap_or_default()
        }
        NodeId::CmdMetaCommand(path, name) => {
            let mc = get_ss(root, path)
                .and_then(|s| s.command.as_ref())
                .and_then(|cmd| cmd.meta_commands.get(name));
            mc.map(detail_meta_command).unwrap_or_default()
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SpaceSystem
// ─────────────────────────────────────────────────────────────────────────────

fn detail_space_system(ss: &SpaceSystem) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("SpaceSystem: {}", ss.name)), sep()];

    if let Some(d) = &ss.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(d) = &ss.long_description {
        lines.push(field("Details:", d.clone()));
    }

    lines.push(blank());
    lines.push(subheading("Contents"));
    if let Some(tm) = &ss.telemetry {
        lines.push(field("  Parameter types:", tm.parameter_types.len().to_string()));
        lines.push(field("  Parameters:", tm.parameters.len().to_string()));
        lines.push(field("  Containers:", tm.containers.len().to_string()));
    } else {
        lines.push(note("  No telemetry data"));
    }
    if let Some(cmd) = &ss.command {
        let cmd_container_count = cmd.command_containers.len()
            + cmd.meta_commands.values().filter(|mc| mc.command_container.is_some()).count();
        lines.push(field("  Argument types:", cmd.argument_types.len().to_string()));
        lines.push(field("  MetaCommands:", cmd.meta_commands.len().to_string()));
        lines.push(field("  Cmd containers:", cmd_container_count.to_string()));
    } else {
        lines.push(note("  No command data"));
    }
    if !ss.sub_systems.is_empty() {
        lines.push(field("  Sub-systems:", ss.sub_systems.len().to_string()));
    }

    if let Some(h) = &ss.header {
        lines.push(blank());
        lines.push(subheading("Header"));
        if let Some(v) = &h.version {
            lines.push(field("  Version:", v.clone()));
        }
        if let Some(d) = &h.date {
            lines.push(field("  Date:", d.clone()));
        }
        if let Some(c) = &h.classification {
            lines.push(field("  Classification:", c.clone()));
        }
        if let Some(ci) = &h.classification_instructions {
            lines.push(field("  Cls. instructions:", ci.clone()));
        }
        if let Some(vs) = &h.validation_status {
            lines.push(field("  Validation status:", vs.clone()));
        }
        for a in &h.author_set {
            let val = match &a.role {
                Some(r) => format!("{} ({})", a.name, r),
                None => a.name.clone(),
            };
            lines.push(field("  Author:", val));
        }
        for n in &h.note_set {
            lines.push(field("  Note:", n.clone()));
        }
    }

    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// Section / group summary nodes
// ─────────────────────────────────────────────────────────────────────────────

fn detail_tm_section(tm: Option<&TelemetryMetaData>) -> Vec<Line<'static>> {
    let Some(tm) = tm else {
        return vec![note("No telemetry data")];
    };
    vec![
        heading("TelemetryMetaData"),
        sep(),
        field("Parameter types:", tm.parameter_types.len().to_string()),
        field("Parameters:", tm.parameters.len().to_string()),
        field("Containers:", tm.containers.len().to_string()),
    ]
}

fn detail_cmd_section(cmd: Option<&CommandMetaData>) -> Vec<Line<'static>> {
    let Some(cmd) = cmd else {
        return vec![note("No command data")];
    };
    let cmd_container_count = cmd.command_containers.len()
        + cmd.meta_commands.values().filter(|mc| mc.command_container.is_some()).count();
    vec![
        heading("CommandMetaData"),
        sep(),
        field("Argument types:", cmd.argument_types.len().to_string()),
        field("MetaCommands:", cmd.meta_commands.len().to_string()),
        field("Command containers:", cmd_container_count.to_string()),
    ]
}

fn detail_group_names(title: &str, names: &[&str]) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("{} ({})", title, names.len())), sep()];
    if names.is_empty() {
        lines.push(note("  (empty)"));
    } else {
        for (i, name) in names.iter().enumerate() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>3}.  ", i + 1), theme::dim()),
                Span::styled(name.to_string(), theme::leaf_node()),
            ]));
        }
    }
    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// ParameterType (all variants)
// ─────────────────────────────────────────────────────────────────────────────

fn detail_parameter_type(pt: &ParameterType) -> Vec<Line<'static>> {
    match pt {
        ParameterType::Integer(t)      => detail_integer_pt(t),
        ParameterType::Float(t)        => detail_float_pt(t),
        ParameterType::Enumerated(t)   => detail_enumerated_pt(t),
        ParameterType::Boolean(t)      => detail_boolean_pt(t),
        ParameterType::String(t)       => detail_string_pt(t),
        ParameterType::Binary(t)       => detail_binary_pt(t),
        ParameterType::Aggregate(t)    => detail_aggregate_pt(t),
        ParameterType::Array(t)        => detail_array_pt(t),
        ParameterType::AbsoluteTime(t) => detail_absolute_time_pt(t),
        ParameterType::RelativeTime(t) => detail_relative_time_pt(t),
    }
}

fn detail_integer_pt(t: &IntegerParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("IntegerParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    lines.push(field("Signed:", if t.signed { "yes" } else { "no" }.to_string()));
    if let Some(s) = t.size_in_bits {
        lines.push(field("Size in bits:", s.to_string()));
    }
    push_units(&mut lines, &t.unit_set);

    if let Some(enc) = &t.encoding {
        lines.push(blank());
        lines.push(subheading("Encoding"));
        lines.push(field("  Format:", fmt_integer_encoding(enc)));
        if let Some(cal) = &enc.default_calibrator {
            lines.push(field("  Calibrator:", fmt_calibrator(cal)));
        }
        lines.push(note("  K: edit calibrator"));
    }
    if let Some(vr) = &t.valid_range {
        lines.push(blank());
        lines.push(subheading("Valid Range"));
        if let Some(v) = vr.min_inclusive {
            lines.push(field("  Min (incl):", v.to_string()));
        }
        if let Some(v) = vr.max_inclusive {
            lines.push(field("  Max (incl):", v.to_string()));
        }
        if let Some(v) = vr.min_exclusive {
            lines.push(field("  Min (excl):", v.to_string()));
        }
        if let Some(v) = vr.max_exclusive {
            lines.push(field("  Max (excl):", v.to_string()));
        }
    }
    if let Some(alarm) = &t.default_alarm {
        lines.push(blank());
        lines.push(subheading("Default Alarm"));
        if let Some(v) = alarm.min_inclusive {
            lines.push(field("  Min:", v.to_string()));
        }
        if let Some(v) = alarm.max_inclusive {
            lines.push(field("  Max:", v.to_string()));
        }
    }
    lines
}

fn detail_float_pt(t: &FloatParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("FloatParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(s) = t.size_in_bits {
        lines.push(field("Size in bits:", s.to_string()));
    }
    push_units(&mut lines, &t.unit_set);

    if let Some(enc) = &t.encoding {
        lines.push(blank());
        lines.push(subheading("Encoding"));
        lines.push(field("  Format:", fmt_float_encoding(enc)));
        if let Some(cal) = &enc.default_calibrator {
            lines.push(field("  Calibrator:", fmt_calibrator(cal)));
        }
        lines.push(note("  K: edit calibrator"));
    }
    if let Some(vr) = &t.valid_range {
        lines.push(blank());
        lines.push(subheading("Valid Range"));
        if let Some(v) = vr.min_inclusive {
            lines.push(field("  Min (incl):", v.to_string()));
        }
        if let Some(v) = vr.max_inclusive {
            lines.push(field("  Max (incl):", v.to_string()));
        }
    }
    if let Some(cal) = &t.default_calibrator {
        lines.push(blank());
        lines.push(field("Default calibrator:", fmt_calibrator(cal)));
    }
    lines
}

fn detail_enumerated_pt(t: &EnumeratedParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("EnumeratedParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_integer_encoding(enc)));
    }

    if !t.enumeration_list.is_empty() {
        lines.push(blank());
        lines.push(subheading(format!("Enumeration ({} values)", t.enumeration_list.len())));
        lines.push(Line::from(vec![
            Span::styled(format!("  {:>10}  ", "Value"), theme::detail_label()),
            Span::styled("Label", theme::detail_label()),
        ]));
        lines.push(Line::from(Span::styled(
            "  ──────────  ─────────────────────────",
            theme::detail_separator(),
        )));
        for e in &t.enumeration_list {
            let range = match e.max_value {
                Some(max) => format!("{}..{}", e.value, max),
                None => e.value.to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>10}  ", range), theme::detail_value()),
                Span::styled(e.label.clone(), theme::detail_value()),
            ]));
        }
    }
    lines
}

fn detail_boolean_pt(t: &BooleanParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("BooleanParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_integer_encoding(enc)));
    }
    if let Some(v) = &t.one_string_value {
        lines.push(field("True string:", v.clone()));
    }
    if let Some(v) = &t.zero_string_value {
        lines.push(field("False string:", v.clone()));
    }
    lines
}

fn detail_string_pt(t: &StringParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("StringParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_string_encoding(enc)));
    }
    lines
}

fn detail_binary_pt(t: &BinaryParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("BinaryParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_binary_encoding(enc)));
    }
    lines
}

fn detail_aggregate_pt(t: &AggregateParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("AggregateParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);

    lines.push(blank());
    lines.push(subheading(format!("Members ({})", t.member_list.len())));
    if t.member_list.is_empty() {
        lines.push(note("  (empty)"));
    } else {
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<24}  ", "Name"), theme::detail_label()),
            Span::styled("Type Ref", theme::detail_label()),
        ]));
        lines.push(Line::from(Span::styled(
            "  ────────────────────────  ────────────────────────",
            theme::detail_separator(),
        )));
        for m in &t.member_list {
            let val = match &m.short_description {
                Some(d) => format!("{}  — {}", m.type_ref, d),
                None => m.type_ref.clone(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<24}  ", m.name), theme::leaf_node()),
                Span::styled(val, theme::detail_value()),
            ]));
        }
    }
    lines
}

fn detail_array_pt(t: &ArrayParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("ArrayParameterType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    lines.push(field("Element type:", t.array_type_ref.clone()));
    lines.push(field("Dimensions:", t.number_of_dimensions.to_string()));
    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// Parameter
// ─────────────────────────────────────────────────────────────────────────────

fn detail_parameter(p: &Parameter, local_ss: &SpaceSystem, root: &SpaceSystem) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("Parameter: {}", p.name)), sep()];
    lines.push(field("Type ref:", p.parameter_type_ref.clone()));

    // Inline the resolved type so the user doesn't have to navigate to it.
    if let Some(pt) = resolve_parameter_type(&p.parameter_type_ref, local_ss, root) {
        lines.push(blank());
        lines.push(subheading("Resolved type"));
        lines.extend(detail_parameter_type_inline(pt));
    }

    if let Some(d) = &p.short_description {
        lines.push(blank());
        lines.push(field("Description:", d.clone()));
    }
    if let Some(d) = &p.long_description {
        lines.push(field("Details:", d.clone()));
    }

    if let Some(props) = &p.parameter_properties {
        if let Some(ds) = &props.data_source {
            let s = match ds {
                DataSource::Telemetered => "Telemetered",
                DataSource::Derived => "Derived",
                DataSource::Constant => "Constant",
                DataSource::Local => "Local",
                DataSource::Ground => "Ground",
            };
            lines.push(field("Data source:", s.to_string()));
        }
        if props.read_only {
            lines.push(field("Read-only:", "yes".to_string()));
        }
    }

    if !p.alias_set.is_empty() {
        lines.push(blank());
        lines.push(subheading("Aliases"));
        for a in &p.alias_set {
            lines.push(field(format!("  {}:", a.name_space), a.alias.clone()));
        }
    }
    lines
}

/// Walk from `local_ss` up to `root` looking for a ParameterType named `type_ref`.
/// Mirrors XTCE scoping: current SpaceSystem is checked first, then ancestors.
fn resolve_parameter_type<'a>(
    type_ref: &str,
    local_ss: &'a SpaceSystem,
    root: &'a SpaceSystem,
) -> Option<&'a ParameterType> {
    // Try the local SS first.
    if let Some(pt) = local_ss.telemetry.as_ref().and_then(|tm| tm.parameter_types.get(type_ref)) {
        return Some(pt);
    }
    // Walk ancestors by finding all SpaceSystems that contain `local_ss` as a descendant.
    find_ancestor_type(type_ref, root, &local_ss.name)
}

/// Recursively search ancestor SpaceSystems (those that contain `target_name`
/// in their subtree) for a ParameterType named `type_ref`.
///
/// Returns `None` when `ss` IS `target_name` (we've reached the local scope,
/// which is handled by the caller) or when `ss` does not contain `target_name`
/// as a descendant.
fn find_ancestor_type<'a>(
    type_ref: &str,
    ss: &'a SpaceSystem,
    target_name: &str,
) -> Option<&'a ParameterType> {
    // Check if this node IS the target — stop ascending.
    if ss.name == target_name {
        return None;
    }
    // Check if any child is the target (or contains it); if so, this node is an ancestor.
    let has_descendant = ss.sub_systems.iter().any(|c| subtree_contains(c, target_name));
    if !has_descendant {
        return None;
    }
    // Check this SS's own types first.
    if let Some(pt) = ss.telemetry.as_ref().and_then(|tm| tm.parameter_types.get(type_ref)) {
        return Some(pt);
    }
    // Recurse into children to continue the ancestor walk.
    for child in &ss.sub_systems {
        if let Some(pt) = find_ancestor_type(type_ref, child, target_name) {
            return Some(pt);
        }
    }
    None
}

/// Return true if `ss` or any of its descendants is named `name`.
fn subtree_contains(ss: &SpaceSystem, name: &str) -> bool {
    ss.name == name || ss.sub_systems.iter().any(|c| subtree_contains(c, name))
}

/// Compact inline summary of a ParameterType for display inside a Parameter detail block.
fn detail_parameter_type_inline(pt: &ParameterType) -> Vec<Line<'static>> {
    match pt {
        ParameterType::Integer(t) => {
            let mut lines = vec![field("  Kind:", "Integer".to_string())];
            if let Some(enc) = &t.encoding {
                lines.push(field("  Encoding:", format!("{:?} {}b", enc.encoding, enc.size_in_bits)));
                if let Some(cal) = &enc.default_calibrator {
                    lines.push(field("  Calibrator:", fmt_calibrator_kind(cal)));
                }
            }
            if !t.unit_set.is_empty() {
                lines.push(field("  Units:", t.unit_set.iter().map(fmt_unit).collect::<Vec<_>>().join(", ")));
            }
            if let Some(r) = &t.valid_range {
                let lo = r.min_inclusive.map(|v| v.to_string()).unwrap_or_else(|| "-∞".to_string());
                let hi = r.max_inclusive.map(|v| v.to_string()).unwrap_or_else(|| "+∞".to_string());
                lines.push(field("  Range:", format!("[{lo}, {hi}]")));
            }
            lines
        }
        ParameterType::Float(t) => {
            let mut lines = vec![field("  Kind:", "Float".to_string())];
            if let Some(enc) = &t.encoding {
                let bits = match enc.size_in_bits {
                    FloatSizeInBits::F32 => 32,
                    FloatSizeInBits::F64 => 64,
                    FloatSizeInBits::F128 => 128,
                };
                lines.push(field("  Encoding:", format!("{:?} {bits}b", enc.encoding)));
                if let Some(cal) = &enc.default_calibrator {
                    lines.push(field("  Calibrator:", fmt_calibrator_kind(cal)));
                }
            }
            if !t.unit_set.is_empty() {
                lines.push(field("  Units:", t.unit_set.iter().map(fmt_unit).collect::<Vec<_>>().join(", ")));
            }
            lines
        }
        ParameterType::Enumerated(t) => {
            let mut lines = vec![field("  Kind:", "Enumerated".to_string())];
            if let Some(enc) = &t.encoding {
                lines.push(field("  Encoding:", format!("{:?} {}b", enc.encoding, enc.size_in_bits)));
            }
            lines.push(field("  Enumerations:", t.enumeration_list.len().to_string()));
            for e in t.enumeration_list.iter().take(6) {
                lines.push(Line::from(format!("    {} \u{2192} {}", e.value, e.label)));
            }
            if t.enumeration_list.len() > 6 {
                lines.push(Line::from(format!("    \u{2026} ({} total)", t.enumeration_list.len())));
            }
            lines
        }
        ParameterType::Boolean(t) => {
            let mut lines = vec![field("  Kind:", "Boolean".to_string())];
            if let Some(enc) = &t.encoding {
                lines.push(field("  Encoding:", format!("{:?} {}b", enc.encoding, enc.size_in_bits)));
            }
            let true_label = t.one_string_value.clone().unwrap_or_else(|| "true".to_string());
            let false_label = t.zero_string_value.clone().unwrap_or_else(|| "false".to_string());
            lines.push(field("  True label:", true_label));
            lines.push(field("  False label:", false_label));
            lines
        }
        ParameterType::String(_) => vec![field("  Kind:", "String".to_string())],
        ParameterType::Binary(_) => vec![field("  Kind:", "Binary".to_string())],
        ParameterType::Aggregate(t) => {
            let mut lines = vec![
                field("  Kind:", "Aggregate".to_string()),
                field("  Members:", t.member_list.len().to_string()),
            ];
            for m in &t.member_list {
                lines.push(Line::from(format!("    {} : {}", m.name, m.type_ref)));
            }
            lines
        }
        ParameterType::Array(t) => vec![
            field("  Kind:", "Array".to_string()),
            field("  Element type:", t.array_type_ref.clone()),
        ],
        ParameterType::AbsoluteTime(t) => {
            let mut lines = vec![field("  Kind:", "AbsoluteTime".to_string())];
            if let Some(epoch) = &t.reference_time {
                lines.push(field("  Epoch:", epoch.clone()));
            }
            lines
        }
        ParameterType::RelativeTime(_) => vec![field("  Kind:", "RelativeTime".to_string())],
    }
}

fn detail_absolute_time_pt(t: &AbsoluteTimeParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![
        heading(format!("AbsoluteTimeParameterType: {}", t.name)),
        sep(),
    ];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(epoch) = &t.reference_time {
        lines.push(field("Epoch:", epoch.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", match enc {
            TimeEncoding::Integer(_) => "Integer".to_string(),
            TimeEncoding::Float(_)   => "Float".to_string(),
        }));
    }
    if !t.unit_set.is_empty() {
        lines.push(field("Units:", t.unit_set.iter().map(fmt_unit).collect::<Vec<_>>().join(", ")));
    }
    lines
}

fn detail_relative_time_pt(t: &RelativeTimeParameterType) -> Vec<Line<'static>> {
    let mut lines = vec![
        heading(format!("RelativeTimeParameterType: {}", t.name)),
        sep(),
    ];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", match enc {
            TimeEncoding::Integer(_) => "Integer".to_string(),
            TimeEncoding::Float(_)   => "Float".to_string(),
        }));
    }
    if !t.unit_set.is_empty() {
        lines.push(field("Units:", t.unit_set.iter().map(fmt_unit).collect::<Vec<_>>().join(", ")));
    }
    lines
}

fn fmt_calibrator_kind(cal: &Calibrator) -> String {
    match cal {
        Calibrator::Polynomial(p) => format!("Polynomial ({} coeffs)", p.coefficients.len()),
        Calibrator::SplineCalibrator(s) => format!("Spline ({} points)", s.points.len()),
    }
}

fn fmt_unit(u: &Unit) -> String {
    u.value.clone()
}

// ─────────────────────────────────────────────────────────────────────────────
// SequenceContainer
// ─────────────────────────────────────────────────────────────────────────────

fn detail_sequence_container(
    c: &SequenceContainer,
    ss: &SpaceSystem,
    root: &SpaceSystem,
) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("SequenceContainer: {}", c.name)), sep()];

    if c.r#abstract {
        lines.push(field("Abstract:", "yes".to_string()));
    }
    if let Some(d) = &c.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(d) = &c.long_description {
        lines.push(field("Details:", d.clone()));
    }

    if let Some(bc) = &c.base_container {
        // Self-referential base is valid XTCE; treat the container as standalone (no inheritance).
        if bc.container_ref != c.name {
            lines.push(blank());
            lines.push(subheading("Inheritance"));
            lines.push(field("  Extends:", bc.container_ref.clone()));
            if let Some(rc) = &bc.restriction_criteria {
                lines.push(field("  Restriction:", fmt_restriction(rc)));
            }
            lines.push(note("  R: edit restriction criteria"));
        }
    }

    // Collect ancestor entry layers (oldest ancestor first).
    let ancestors = collect_inheritance_chain(c, ss, root);
    let has_ancestors = !ancestors.is_empty();

    let mut grand_bits: u32 = 0;
    let mut grand_variable = false;

    // Inherited sections — shown in dim to distinguish from own entries.
    for (ancestor_name, ancestor_entries) in &ancestors {
        lines.push(blank());
        lines.push(Line::from(Span::styled(
            format!("Inherited from {} ({} entries)", ancestor_name, ancestor_entries.len()),
            theme::dim(),
        )));
        if !ancestor_entries.is_empty() {
            lines.push(Line::from(Span::styled(
                format!("  {:<28}  {:>8}", "─".repeat(28), "────────"),
                theme::dim(),
            )));
            for entry in ancestor_entries {
                let (label, bits_opt) = entry_label_and_bits(entry, ss, root);
                let bits_str = match bits_opt {
                    Some(b) => { grand_bits += b; format!("{:>8}", b) }
                    None => { grand_variable = true; "variable".to_string() }
                };
                lines.push(Line::from(vec![
                    Span::styled(format!("  {:<28}  ", label), theme::dim()),
                    Span::styled(bits_str, theme::dim()),
                ]));
            }
        }
    }

    // Own entries.
    lines.push(blank());
    let own_label = if has_ancestors {
        format!("Own entries ({} entries)", c.entry_list.len())
    } else {
        format!("Entry List ({} entries)", c.entry_list.len())
    };
    lines.push(subheading(own_label));

    if c.entry_list.is_empty() {
        lines.push(note("  (empty)"));
    } else {
        lines.push(note("  L: set entry bit offset"));
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<28}  {:>8}", "Name / Ref", "Bits"), theme::detail_label()),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {:<28}  {:>8}", "─".repeat(28), "────────"),
            theme::detail_separator(),
        )));
        for entry in &c.entry_list {
            let (label, bits_opt) = entry_label_and_bits(entry, ss, root);
            let bits_str = match bits_opt {
                Some(b) => { grand_bits += b; format!("{:>8}", b) }
                None => { grand_variable = true; format!("{:>8}", "variable") }
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<28}  ", label), theme::leaf_node()),
                Span::styled(bits_str, theme::detail_value()),
            ]));
        }
    }

    // Grand total across all layers.
    lines.push(Line::from(Span::styled(
        format!("  {:<28}  {:>8}", "─".repeat(28), "────────"),
        theme::detail_separator(),
    )));
    let total_label = if has_ancestors { "Total (all layers)" } else { "Total" };
    let total_str = if grand_variable {
        format!("{}+ bits (variable fields present)", grand_bits)
    } else {
        let bytes = grand_bits / 8;
        let rem = grand_bits % 8;
        if rem == 0 {
            format!("{} bits ({} bytes)", grand_bits, bytes)
        } else {
            format!("{} bits ({} bytes + {} bits)", grand_bits, bytes, rem)
        }
    };
    lines.push(Line::from(vec![
        Span::styled(format!("  {:<28}  ", total_label), theme::detail_label()),
        Span::styled(total_str, theme::detail_value()),
    ]));

    lines
}

/// Return a display label and optional bit-width for a single container entry.
///
/// The label includes any explicit location annotation; the bit-width is
/// `None` for entries whose size cannot be statically resolved (e.g.
/// `ContainerRef` and `ArrayParameterRef`).
fn entry_label_and_bits(
    entry: &SequenceEntry,
    ss: &SpaceSystem,
    root: &SpaceSystem,
) -> (String, Option<u32>) {
    match entry {
        SequenceEntry::ParameterRef(e) => {
            let bits = resolve_param_bits(ss, root, &e.parameter_ref);
            let label = fmt_entry_label(&e.parameter_ref, e.location.as_ref());
            (label, bits)
        }
        SequenceEntry::ContainerRef(e) => {
            (fmt_entry_label(&format!("[{}]", e.container_ref), e.location.as_ref()), None)
        }
        SequenceEntry::FixedValue(e) => {
            let name = match &e.binary_value {
                Some(v) => format!("<0x{}>", v),
                None => "<fixed>".to_string(),
            };
            (fmt_entry_label(&name, e.location.as_ref()), Some(e.size_in_bits))
        }
        SequenceEntry::ArrayParameterRef(e) => {
            (fmt_entry_label(&format!("{}[]", e.parameter_ref), e.location.as_ref()), None)
        }
    }
}

/// Try to resolve the encoded bit-width of a parameter by name.
/// Finds the parameter in ss or root, then resolves its type from ss or root.
fn resolve_param_bits(ss: &SpaceSystem, root: &SpaceSystem, param_ref: &str) -> Option<u32> {
    // Step 1: find the parameter in ss or root.
    let param = [ss, root].iter().find_map(|s| {
        s.telemetry.as_ref()?.parameters.get(param_ref)
    })?;
    // Step 2: find the type in ss or root (may differ from where the param lives).
    let type_ref = &param.parameter_type_ref;
    let pt = [ss, root].iter().find_map(|s| {
        s.telemetry.as_ref()?.parameter_types.get(type_ref)
    })?;
    parameter_type_bits(pt)
}

/// Walk ss and root to find a container by name.
fn find_container_in_ancestors<'a>(
    name: &str,
    ss: &'a SpaceSystem,
    root: &'a SpaceSystem,
) -> Option<&'a SequenceContainer> {
    [ss, root].iter().find_map(|s| {
        s.telemetry.as_ref()?.containers.get(name)
    })
}

/// Return the ancestor layers for `c`, oldest first.
///
/// Each element is `(container_name, cloned_entry_list)`. The current container
/// is NOT included — call this to get what was inherited.
fn collect_inheritance_chain(
    c: &SequenceContainer,
    ss: &SpaceSystem,
    root: &SpaceSystem,
) -> Vec<(String, Vec<SequenceEntry>)> {
    let mut layers: Vec<(String, Vec<SequenceEntry>)> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut next_ref: Option<String> = c.base_container.as_ref().map(|bc| bc.container_ref.clone());

    while let Some(ref_name) = next_ref {
        if seen.contains(&ref_name) {
            break; // cycle guard
        }
        seen.insert(ref_name.clone());
        if let Some(parent) = find_container_in_ancestors(&ref_name, ss, root) {
            let next = parent.base_container.as_ref().map(|bc| bc.container_ref.clone());
            layers.push((parent.name.clone(), parent.entry_list.clone()));
            next_ref = next;
        } else {
            break;
        }
    }
    layers.reverse(); // oldest ancestor first
    layers
}

/// Return the encoded size in bits for a `ParameterType`, if statically known.
///
/// Returns `None` for aggregate, array, and time types, and for variable-length
/// string and binary encodings.
fn parameter_type_bits(pt: &ParameterType) -> Option<u32> {
    match pt {
        ParameterType::Integer(t) => t.encoding.as_ref().map(|e| e.size_in_bits),
        ParameterType::Float(t) => t.encoding.as_ref().and_then(|e| float_bits(&e.size_in_bits)),
        ParameterType::Enumerated(t) => t.encoding.as_ref().map(|e| e.size_in_bits),
        ParameterType::Boolean(t) => t.encoding.as_ref().map(|e| e.size_in_bits),
        ParameterType::String(t) => t.encoding.as_ref().and_then(|e| {
            match &e.size_in_bits {
                Some(StringSize::Fixed(n)) => Some(*n),
                _ => None,
            }
        }),
        ParameterType::Binary(t) => t.encoding.as_ref().and_then(|e| match &e.size_in_bits {
            BinarySize::Fixed(n) => Some(*n),
            BinarySize::Variable { .. } => None,
        }),
        ParameterType::Aggregate(_)
        | ParameterType::Array(_)
        | ParameterType::AbsoluteTime(_)
        | ParameterType::RelativeTime(_) => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// MetaCommand
// ─────────────────────────────────────────────────────────────────────────────

fn detail_meta_command(mc: &MetaCommand) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("MetaCommand: {}", mc.name)), sep()];

    if mc.r#abstract {
        lines.push(field("Abstract:", "yes".to_string()));
    }
    if let Some(d) = &mc.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(d) = &mc.long_description {
        lines.push(field("Details:", d.clone()));
    }
    if let Some(b) = &mc.base_meta_command {
        lines.push(field("Extends:", b.clone()));
    }

    lines.push(blank());
    if mc.argument_list.is_empty() {
        lines.push(note("No arguments"));
    } else {
        lines.push(subheading(format!("Arguments ({})", mc.argument_list.len())));
        lines.push(Line::from(vec![
            Span::styled(format!("  {:<20}  {:<24}  ", "Name", "Type"), theme::detail_label()),
            Span::styled("Default", theme::detail_label()),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {:<20}  {:<24}  {}", "─".repeat(20), "─".repeat(24), "─────────"),
            theme::detail_separator(),
        )));
        for arg in &mc.argument_list {
            let default = arg.initial_value.as_deref().unwrap_or("—");
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<20}  {:<24}  ", arg.name, arg.argument_type_ref), theme::leaf_node()),
                Span::styled(default.to_string(), theme::dim()),
            ]));
            if let Some(d) = &arg.short_description {
                lines.push(Line::from(Span::styled(
                    format!("    ↳ {}", d),
                    theme::dim(),
                )));
            }
        }
    }

    if let Some(cc) = &mc.command_container {
        lines.push(blank());
        lines.push(subheading(format!("Command Container: {}", cc.name)));
        if let Some(bc) = &cc.base_container {
            lines.push(field("  Extends:", bc.container_ref.clone()));
        }
        lines.push(field("  Entries:", cc.entry_list.len().to_string()));
    }
    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// ArgumentType (all 8 variants)
// ─────────────────────────────────────────────────────────────────────────────

fn detail_argument_type(at: &ArgumentType) -> Vec<Line<'static>> {
    match at {
        ArgumentType::Integer(t) => detail_integer_at(t),
        ArgumentType::Float(t) => detail_float_at(t),
        ArgumentType::Enumerated(t) => detail_enumerated_at(t),
        ArgumentType::Boolean(t) => detail_boolean_at(t),
        ArgumentType::String(t) => detail_string_at(t),
        ArgumentType::Binary(t) => detail_binary_at(t),
        ArgumentType::Aggregate(t) => detail_aggregate_at(t),
        ArgumentType::Array(t) => detail_array_at(t),
    }
}

fn detail_integer_at(t: &IntegerArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("IntegerArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    lines.push(field("Signed:", if t.signed { "yes" } else { "no" }.to_string()));
    if let Some(s) = t.size_in_bits {
        lines.push(field("Size in bits:", s.to_string()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(blank());
        lines.push(subheading("Encoding"));
        lines.push(field("  Format:", fmt_integer_encoding(enc)));
    }
    if let Some(vr) = &t.valid_range {
        lines.push(blank());
        lines.push(subheading("Valid Range"));
        if let Some(v) = vr.min_inclusive {
            lines.push(field("  Min (incl):", v.to_string()));
        }
        if let Some(v) = vr.max_inclusive {
            lines.push(field("  Max (incl):", v.to_string()));
        }
    }
    if let Some(v) = t.initial_value {
        lines.push(field("Initial value:", v.to_string()));
    }
    lines
}

fn detail_float_at(t: &FloatArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("FloatArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(s) = t.size_in_bits {
        lines.push(field("Size in bits:", s.to_string()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(blank());
        lines.push(subheading("Encoding"));
        lines.push(field("  Format:", fmt_float_encoding(enc)));
    }
    if let Some(v) = t.initial_value {
        lines.push(field("Initial value:", v.to_string()));
    }
    lines
}

fn detail_enumerated_at(t: &EnumeratedArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("EnumeratedArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_integer_encoding(enc)));
    }
    if let Some(v) = &t.initial_value {
        lines.push(field("Initial value:", v.clone()));
    }
    if !t.enumeration_list.is_empty() {
        lines.push(blank());
        lines.push(subheading(format!("Enumeration ({} values)", t.enumeration_list.len())));
        lines.push(Line::from(vec![
            Span::styled(format!("  {:>10}  ", "Value"), theme::detail_label()),
            Span::styled("Label", theme::detail_label()),
        ]));
        lines.push(Line::from(Span::styled(
            "  ──────────  ─────────────────────────",
            theme::detail_separator(),
        )));
        for e in &t.enumeration_list {
            let range = match e.max_value {
                Some(max) => format!("{}..{}", e.value, max),
                None => e.value.to_string(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>10}  ", range), theme::detail_value()),
                Span::styled(e.label.clone(), theme::detail_value()),
            ]));
        }
    }
    lines
}

fn detail_boolean_at(t: &BooleanArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("BooleanArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_integer_encoding(enc)));
    }
    if let Some(v) = &t.one_string_value {
        lines.push(field("True string:", v.clone()));
    }
    if let Some(v) = &t.zero_string_value {
        lines.push(field("False string:", v.clone()));
    }
    lines
}

fn detail_string_at(t: &StringArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("StringArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_string_encoding(enc)));
    }
    if let Some(v) = &t.initial_value {
        lines.push(field("Initial value:", v.clone()));
    }
    lines
}

fn detail_binary_at(t: &BinaryArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("BinaryArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    if let Some(enc) = &t.encoding {
        lines.push(field("Encoding:", fmt_binary_encoding(enc)));
    }
    if let Some(v) = &t.initial_value {
        lines.push(field("Initial value:", v.clone()));
    }
    lines
}

fn detail_aggregate_at(t: &AggregateArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("AggregateArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    push_units(&mut lines, &t.unit_set);
    lines.push(blank());
    lines.push(subheading(format!("Members ({})", t.member_list.len())));
    if t.member_list.is_empty() {
        lines.push(note("  (empty)"));
    } else {
        for m in &t.member_list {
            let val = match &m.short_description {
                Some(d) => format!("{}  — {}", m.type_ref, d),
                None => m.type_ref.clone(),
            };
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<24}  ", m.name), theme::leaf_node()),
                Span::styled(val, theme::detail_value()),
            ]));
        }
    }
    lines
}

fn detail_array_at(t: &ArrayArgumentType) -> Vec<Line<'static>> {
    let mut lines = vec![heading(format!("ArrayArgumentType: {}", t.name)), sep()];
    if let Some(d) = &t.short_description {
        lines.push(field("Description:", d.clone()));
    }
    if let Some(b) = &t.base_type {
        lines.push(field("Base type:", b.clone()));
    }
    lines.push(field("Element type:", t.array_type_ref.clone()));
    lines.push(field("Dimensions:", t.number_of_dimensions.to_string()));
    lines
}

// ─────────────────────────────────────────────────────────────────────────────
// Encoding formatters
// ─────────────────────────────────────────────────────────────────────────────

fn fmt_integer_encoding(enc: &IntegerDataEncoding) -> String {
    let scheme = match enc.encoding {
        IntegerEncoding::Unsigned => "unsigned",
        IntegerEncoding::SignMagnitude => "sign-magnitude",
        IntegerEncoding::TwosComplement => "two's complement",
        IntegerEncoding::OnesComplement => "one's complement",
        IntegerEncoding::BCD => "BCD",
        IntegerEncoding::PackedBCD => "packed BCD",
    };
    let bo = fmt_byte_order(enc.byte_order.as_ref());
    format!("{}-bit {}{}", enc.size_in_bits, scheme, bo)
}

fn fmt_float_encoding(enc: &FloatDataEncoding) -> String {
    let size = match enc.size_in_bits {
        FloatSizeInBits::F32 => "32",
        FloatSizeInBits::F64 => "64",
        FloatSizeInBits::F128 => "128",
    };
    let scheme = match enc.encoding {
        FloatEncoding::IEEE754_1985 => "IEEE 754",
        FloatEncoding::MilStd1750A => "MIL-STD-1750A",
    };
    let bo = fmt_byte_order(enc.byte_order.as_ref());
    format!("{}-bit {}{}", size, scheme, bo)
}

fn fmt_string_encoding(enc: &StringDataEncoding) -> String {
    let scheme = match enc.encoding {
        StringEncoding::UTF8 => "UTF-8",
        StringEncoding::UTF16 => "UTF-16",
        StringEncoding::UsAscii => "US-ASCII",
        StringEncoding::Iso8859_1 => "ISO-8859-1",
    };
    let size = match &enc.size_in_bits {
        Some(StringSize::Fixed(n)) => format!(", fixed {} bits", n),
        Some(StringSize::TerminationChar(c)) => format!(", terminated by 0x{:02X}", c),
        Some(StringSize::Variable { max_size_in_bits: m }) => {
            format!(", variable (max {} bits)", m)
        }
        None => String::new(),
    };
    format!("{}{}", scheme, size)
}

fn fmt_binary_encoding(enc: &BinaryDataEncoding) -> String {
    match &enc.size_in_bits {
        BinarySize::Fixed(n) => format!("{} bits (fixed)", n),
        BinarySize::Variable { size_reference } => format!("variable (ref: {})", size_reference),
    }
}

fn fmt_calibrator(cal: &Calibrator) -> String {
    match cal {
        Calibrator::Polynomial(p) => {
            let terms: Vec<String> = p
                .coefficients
                .iter()
                .enumerate()
                .map(|(i, c)| match i {
                    0 => format!("{}", c),
                    1 => format!("{}·x", c),
                    _ => format!("{}·x^{}", c, i),
                })
                .collect();
            format!("Polynomial: {}", terms.join(" + "))
        }
        Calibrator::SplineCalibrator(s) => {
            format!("Spline ({} points, order {})", s.points.len(), s.order)
        }
    }
}

fn fmt_byte_order(bo: Option<&ByteOrder>) -> &'static str {
    match bo {
        Some(ByteOrder::LeastSignificantByteFirst) => ", little-endian",
        _ => "", // big-endian is the default; omit for brevity
    }
}

fn fmt_restriction(rc: &RestrictionCriteria) -> String {
    match rc {
        RestrictionCriteria::Comparison(c) => fmt_comparison(c),
        RestrictionCriteria::ComparisonList(list) => list
            .iter()
            .map(fmt_comparison)
            .collect::<Vec<_>>()
            .join(" AND "),
        RestrictionCriteria::BooleanExpression(_) => "(boolean expression)".to_string(),
        RestrictionCriteria::NextContainer { container_ref } => {
            format!("next: {}", container_ref)
        }
    }
}

fn fmt_comparison(c: &Comparison) -> String {
    let op = match c.comparison_operator {
        ComparisonOperator::Equality => "==",
        ComparisonOperator::Inequality => "!=",
        ComparisonOperator::LessThan => "<",
        ComparisonOperator::LessThanOrEqual => "<=",
        ComparisonOperator::GreaterThan => ">",
        ComparisonOperator::GreaterThanOrEqual => ">=",
    };
    format!("{} {} {}", c.parameter_ref, op, c.value)
}

/// Convert a `FloatSizeInBits` variant to its numeric bit-count.
fn float_bits(size: &FloatSizeInBits) -> Option<u32> {
    Some(match size {
        FloatSizeInBits::F32 => 32,
        FloatSizeInBits::F64 => 64,
        FloatSizeInBits::F128 => 128,
    })
}

// ─────────────────────────────────────────────────────────────────────────────
// Line / span builder helpers
// ─────────────────────────────────────────────────────────────────────────────

/// A bold section-header line.
fn heading(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), theme::section_header()))
}

/// A dimmer sub-section label line.
fn subheading(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), theme::group_node()))
}

/// A horizontal separator line.
fn sep() -> Line<'static> {
    Line::from(Span::styled(
        "─".repeat(50),
        theme::detail_separator(),
    ))
}

/// An empty line (vertical spacing).
fn blank() -> Line<'static> {
    Line::from("")
}

/// A two-column label/value line, left-padded to 22 characters.
fn field(label: impl Into<String>, value: impl Into<String>) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("{:<22}", label.into()), theme::detail_label()),
        Span::styled(value.into(), theme::detail_value()),
    ])
}

/// A dimmed informational line (e.g. keyboard hints, empty-state messages).
fn note(text: impl Into<String>) -> Line<'static> {
    Line::from(Span::styled(text.into(), theme::dim()))
}

/// Format an entry name with an optional location annotation.
///
/// Uses `@<n>b` for `ContainerStart` locations and `+<n>b` for
/// `PreviousEntry` locations (e.g. `"APID @0b"` or `"SeqCount +8b"`).
fn fmt_entry_label(name: &str, loc: Option<&xtce_core::model::container::EntryLocation>) -> String {
    match loc {
        None => name.to_string(),
        Some(l) => {
            use xtce_core::model::container::ReferenceLocation;
            let ref_char = match l.reference_location {
                ReferenceLocation::ContainerStart => '@',
                ReferenceLocation::PreviousEntry  => '+',
            };
            format!("{} {}{}b", name, ref_char, l.bit_offset)
        }
    }
}

/// Append a `Units:` field row and an edit hint to `lines`.
///
/// No-ops on the units row if the slice is empty, but always appends the
/// keyboard hint.
fn push_units(lines: &mut Vec<Line<'static>>, units: &[Unit]) {
    if !units.is_empty() {
        let s = units.iter().map(|u| u.value.as_str()).collect::<Vec<_>>().join(", ");
        lines.push(field("Units:", s));
    }
    lines.push(note("U: edit units"));
}
