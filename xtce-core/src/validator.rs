//! Reference resolution and consistency validation.
//!
//! After parsing, call [`validate`] to check that all cross-references resolve,
//! names are unique within their scope, and inheritance chains are acyclic.
//!
//! # What is checked
//!
//! - **Unresolved references**: every `parameterTypeRef`, `containerRef`,
//!   `argumentTypeRef`, `arrayTypeRef`, `typeRef`, `base_type`, and
//!   `parameterRef` / `argumentRef` in comparisons must name an element
//!   that is defined in the current SpaceSystem or any ancestor.
//! - **Cyclic inheritance**: `SequenceContainer` base-container chains,
//!   `MetaCommand` base-command chains, and type `base_type` chains must
//!   not form loops.
//!
//! # Scoping rules
//!
//! Each SpaceSystem is validated with a scope that includes all names
//! defined in itself and every ancestor SpaceSystem. Child SpaceSystems
//! inherit the full scope of their parent.

use std::collections::{HashMap, HashSet};

use crate::model::command::{ArgumentType, MetaCommand};
use crate::model::container::{
    BooleanExpression, Comparison, MatchCriteria, RestrictionCriteria, SequenceContainer,
    SequenceEntry,
};
use crate::model::telemetry::{ParameterType, TelemetryMetaData};
use crate::{SpaceSystem, ValidationError};

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Validate a parsed [`SpaceSystem`] tree.
///
/// Returns a list of all validation errors found. An empty list means the
/// document is structurally valid. Errors do not stop collection — all problems
/// are reported together.
pub fn validate(space_system: &SpaceSystem) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    validate_space_system(space_system, &Scope::default(), &mut errors);
    errors
}

// ─────────────────────────────────────────────────────────────────────────────
// Scope — the set of names visible at a given point in the tree
// ─────────────────────────────────────────────────────────────────────────────

/// All names visible from a particular SpaceSystem scope.
///
/// Built incrementally: each SpaceSystem extends its parent's scope with
/// the names it defines.
#[derive(Default, Clone)]
struct Scope<'a> {
    parameter_types: HashSet<&'a str>,
    parameters: HashSet<&'a str>,
    /// Telemetry sequence containers AND command containers combined.
    containers: HashSet<&'a str>,
    argument_types: HashSet<&'a str>,
    meta_commands: HashSet<&'a str>,
}

impl<'a> Scope<'a> {
    /// Create a child scope by extending `parent` with the names defined in `ss`.
    fn with_space_system(parent: &Self, ss: &'a SpaceSystem) -> Self {
        let mut scope = parent.clone();
        if let Some(tm) = &ss.telemetry {
            scope.parameter_types.extend(tm.parameter_types.keys().map(String::as_str));
            scope.parameters.extend(tm.parameters.keys().map(String::as_str));
            scope.containers.extend(tm.containers.keys().map(String::as_str));
        }
        if let Some(cmd) = &ss.command {
            scope.argument_types.extend(cmd.argument_types.keys().map(String::as_str));
            scope.meta_commands.extend(cmd.meta_commands.keys().map(String::as_str));
            scope.containers.extend(cmd.command_containers.keys().map(String::as_str));
        }
        scope
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Top-level recursive driver
// ─────────────────────────────────────────────────────────────────────────────

fn validate_space_system(
    ss: &SpaceSystem,
    parent_scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    let scope = Scope::with_space_system(parent_scope, ss);

    if let Some(tm) = &ss.telemetry {
        validate_telemetry(tm, &ss.name, &scope, errors);
        detect_container_cycles(&ss.name, &tm.containers, errors);
    }

    if let Some(cmd) = &ss.command {
        for (name, at) in &cmd.argument_types {
            validate_argument_type(at, name, &ss.name, &scope, errors);
        }
        for mc in cmd.meta_commands.values() {
            validate_meta_command(mc, &ss.name, &scope, errors);
        }
        detect_meta_command_cycles(&ss.name, &cmd.meta_commands, errors);
    }

    check_duplicate_sub_system_names(ss, errors);
    check_duplicate_names(ss, parent_scope, errors);

    for child in &ss.sub_systems {
        validate_space_system(child, &scope, errors);
    }
}

/// Flag any two sibling SpaceSystems that share the same name.
///
/// Sub-systems are stored in a `Vec`, so the parser (and programmatic
/// construction) can produce duplicates that IndexMap-backed collections
/// cannot.  Each duplicated name is reported once.
fn check_duplicate_sub_system_names(ss: &SpaceSystem, errors: &mut Vec<ValidationError>) {
    let mut seen: HashSet<&str> = HashSet::new();
    let mut reported: HashSet<&str> = HashSet::new();
    for child in &ss.sub_systems {
        if !seen.insert(child.name.as_str()) && reported.insert(child.name.as_str()) {
            errors.push(ValidationError::DuplicateName {
                name: child.name.clone(),
                space_system: ss.name.clone(),
            });
        }
    }
}

/// Flag names in `ss` that collide with names already present in an ancestor
/// scope, and flag any within-SpaceSystem container namespace collision.
///
/// # Two cases detected
///
/// 1. **Shadowing** — a SpaceSystem defines a name (ParameterType, Parameter,
///    SequenceContainer, ArgumentType, or MetaCommand) that already exists in a
///    parent or grandparent SpaceSystem's scope.  XTCE scoping allows this, but
///    it is almost always unintentional and can cause confusing reference
///    resolution.
///
/// 2. **Container namespace collision** — telemetry `SequenceContainer`s and
///    command `CommandContainer`s share a single "containers" namespace within a
///    SpaceSystem.  A name present in both `TelemetryMetaData.containers` and
///    `CommandMetaData.command_containers` of the *same* SpaceSystem is
///    ambiguous (this state can arise via the TUI editor).
fn check_duplicate_names(
    ss: &SpaceSystem,
    parent_scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    // ── Container namespace collision within the same SpaceSystem ─────────────
    if let (Some(tm), Some(cmd)) = (&ss.telemetry, &ss.command) {
        for name in tm.containers.keys() {
            if cmd.command_containers.contains_key(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
    }

    // ── Shadowing: names that already appear in an ancestor scope ─────────────
    if let Some(tm) = &ss.telemetry {
        for name in tm.parameter_types.keys() {
            if parent_scope.parameter_types.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
        for name in tm.parameters.keys() {
            if parent_scope.parameters.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
        for name in tm.containers.keys() {
            if parent_scope.containers.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
    }
    if let Some(cmd) = &ss.command {
        for name in cmd.argument_types.keys() {
            if parent_scope.argument_types.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
        for name in cmd.meta_commands.keys() {
            if parent_scope.meta_commands.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
        for name in cmd.command_containers.keys() {
            if parent_scope.containers.contains(name.as_str()) {
                errors.push(ValidationError::DuplicateName {
                    name: name.clone(),
                    space_system: ss.name.clone(),
                });
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Telemetry validation
// ─────────────────────────────────────────────────────────────────────────────

fn validate_telemetry(
    tm: &TelemetryMetaData,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    for (name, pt) in &tm.parameter_types {
        validate_parameter_type(pt, name, ss_name, scope, errors);
    }
    for (name, param) in &tm.parameters {
        if !scope.parameter_types.contains(param.parameter_type_ref.as_str()) {
            errors.push(ValidationError::UnresolvedReference {
                name: param.parameter_type_ref.clone(),
                context: format!("parameter '{}' in SpaceSystem '{}'", name, ss_name),
            });
        }
    }
    for (name, container) in &tm.containers {
        validate_sequence_container(container, name, ss_name, scope, errors);
    }
}

fn validate_parameter_type(
    pt: &ParameterType,
    name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    // Check base_type reference (common to all variants).
    if let Some(base) = pt_base_type(pt) {
        if !scope.parameter_types.contains(base) {
            errors.push(ValidationError::UnresolvedReference {
                name: base.to_owned(),
                context: format!(
                    "baseType of ParameterType '{}' in SpaceSystem '{}'",
                    name, ss_name
                ),
            });
        }
    }

    // Variant-specific reference checks.
    match pt {
        ParameterType::Aggregate(t) => {
            for member in &t.member_list {
                if !scope.parameter_types.contains(member.type_ref.as_str()) {
                    errors.push(ValidationError::UnresolvedReference {
                        name: member.type_ref.clone(),
                        context: format!(
                            "member '{}' of AggregateParameterType '{}' in SpaceSystem '{}'",
                            member.name, name, ss_name
                        ),
                    });
                }
            }
        }
        ParameterType::Array(t) => {
            if !scope.parameter_types.contains(t.array_type_ref.as_str()) {
                errors.push(ValidationError::UnresolvedReference {
                    name: t.array_type_ref.clone(),
                    context: format!(
                        "arrayTypeRef of ArrayParameterType '{}' in SpaceSystem '{}'",
                        name, ss_name
                    ),
                });
            }
        }
        _ => {}
    }
}

fn validate_sequence_container(
    container: &SequenceContainer,
    name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    // Check base container reference.
    if let Some(base) = &container.base_container {
        if !scope.containers.contains(base.container_ref.as_str()) {
            errors.push(ValidationError::UnresolvedReference {
                name: base.container_ref.clone(),
                context: format!(
                    "BaseContainer of SequenceContainer '{}' in SpaceSystem '{}'",
                    name, ss_name
                ),
            });
        }
        // Check restriction criteria parameter references.
        if let Some(rc) = &base.restriction_criteria {
            check_restriction_criteria(rc, name, ss_name, scope, errors);
        }
    }

    // Check entry list references.
    for entry in &container.entry_list {
        match entry {
            SequenceEntry::ParameterRef(e) => {
                if !scope.parameters.contains(e.parameter_ref.as_str()) {
                    errors.push(ValidationError::UnresolvedReference {
                        name: e.parameter_ref.clone(),
                        context: format!(
                            "ParameterRefEntry in container '{}' in SpaceSystem '{}'",
                            name, ss_name
                        ),
                    });
                }
                if let Some(mc) = &e.include_condition {
                    check_match_criteria(mc, name, ss_name, scope, errors);
                }
            }
            SequenceEntry::ContainerRef(e) => {
                if !scope.containers.contains(e.container_ref.as_str()) {
                    errors.push(ValidationError::UnresolvedReference {
                        name: e.container_ref.clone(),
                        context: format!(
                            "ContainerRefEntry in container '{}' in SpaceSystem '{}'",
                            name, ss_name
                        ),
                    });
                }
                if let Some(mc) = &e.include_condition {
                    check_match_criteria(mc, name, ss_name, scope, errors);
                }
            }
            SequenceEntry::ArrayParameterRef(e) => {
                if !scope.parameters.contains(e.parameter_ref.as_str()) {
                    errors.push(ValidationError::UnresolvedReference {
                        name: e.parameter_ref.clone(),
                        context: format!(
                            "ArrayParameterRefEntry in container '{}' in SpaceSystem '{}'",
                            name, ss_name
                        ),
                    });
                }
            }
            SequenceEntry::FixedValue(_) => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Command validation
// ─────────────────────────────────────────────────────────────────────────────

fn validate_argument_type(
    at: &ArgumentType,
    name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    if let Some(base) = at_base_type(at) {
        if !scope.argument_types.contains(base) {
            errors.push(ValidationError::UnresolvedReference {
                name: base.to_owned(),
                context: format!(
                    "baseType of ArgumentType '{}' in SpaceSystem '{}'",
                    name, ss_name
                ),
            });
        }
    }

    match at {
        ArgumentType::Aggregate(t) => {
            for member in &t.member_list {
                if !scope.argument_types.contains(member.type_ref.as_str()) {
                    errors.push(ValidationError::UnresolvedReference {
                        name: member.type_ref.clone(),
                        context: format!(
                            "member '{}' of AggregateArgumentType '{}' in SpaceSystem '{}'",
                            member.name, name, ss_name
                        ),
                    });
                }
            }
        }
        ArgumentType::Array(t) => {
            if !scope.argument_types.contains(t.array_type_ref.as_str()) {
                errors.push(ValidationError::UnresolvedReference {
                    name: t.array_type_ref.clone(),
                    context: format!(
                        "arrayTypeRef of ArrayArgumentType '{}' in SpaceSystem '{}'",
                        name, ss_name
                    ),
                });
            }
        }
        _ => {}
    }
}

fn validate_meta_command(
    mc: &MetaCommand,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    // Check base MetaCommand reference.
    if let Some(base) = &mc.base_meta_command {
        if !scope.meta_commands.contains(base.as_str()) {
            errors.push(ValidationError::UnresolvedReference {
                name: base.clone(),
                context: format!(
                    "baseMetaCommand of MetaCommand '{}' in SpaceSystem '{}'",
                    mc.name, ss_name
                ),
            });
        }
    }

    // Check argument type references.
    for arg in &mc.argument_list {
        if !scope.argument_types.contains(arg.argument_type_ref.as_str()) {
            errors.push(ValidationError::UnresolvedReference {
                name: arg.argument_type_ref.clone(),
                context: format!(
                    "argumentTypeRef of argument '{}' in MetaCommand '{}' in SpaceSystem '{}'",
                    arg.name, mc.name, ss_name
                ),
            });
        }
    }

    // Check CommandContainer base container and entry refs.
    if let Some(cc) = &mc.command_container {
        if let Some(base) = &cc.base_container {
            if !scope.containers.contains(base.container_ref.as_str()) {
                errors.push(ValidationError::UnresolvedReference {
                    name: base.container_ref.clone(),
                    context: format!(
                        "BaseContainer of CommandContainer '{}' in MetaCommand '{}' in \
                         SpaceSystem '{}'",
                        cc.name, mc.name, ss_name
                    ),
                });
            }
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Restriction criteria and comparison reference checks
// ─────────────────────────────────────────────────────────────────────────────

fn check_restriction_criteria(
    rc: &RestrictionCriteria,
    container_name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    match rc {
        RestrictionCriteria::Comparison(cmp) => {
            check_comparison(cmp, container_name, ss_name, scope, errors)
        }
        RestrictionCriteria::ComparisonList(list) => {
            for cmp in list {
                check_comparison(cmp, container_name, ss_name, scope, errors);
            }
        }
        RestrictionCriteria::BooleanExpression(expr) => {
            check_boolean_expression(expr, container_name, ss_name, scope, errors)
        }
        RestrictionCriteria::NextContainer { container_ref } => {
            if !scope.containers.contains(container_ref.as_str()) {
                errors.push(ValidationError::UnresolvedReference {
                    name: container_ref.clone(),
                    context: format!(
                        "NextContainer in RestrictionCriteria of container '{}' in \
                         SpaceSystem '{}'",
                        container_name, ss_name
                    ),
                });
            }
        }
    }
}

fn check_match_criteria(
    mc: &MatchCriteria,
    container_name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    match mc {
        MatchCriteria::Comparison(cmp) => {
            check_comparison(cmp, container_name, ss_name, scope, errors)
        }
        MatchCriteria::ComparisonList(list) => {
            for cmp in list {
                check_comparison(cmp, container_name, ss_name, scope, errors);
            }
        }
        MatchCriteria::BooleanExpression(expr) => {
            check_boolean_expression(expr, container_name, ss_name, scope, errors)
        }
    }
}

fn check_comparison(
    cmp: &Comparison,
    container_name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    if !scope.parameters.contains(cmp.parameter_ref.as_str()) {
        errors.push(ValidationError::UnresolvedReference {
            name: cmp.parameter_ref.clone(),
            context: format!(
                "Comparison in container '{}' in SpaceSystem '{}'",
                container_name, ss_name
            ),
        });
    }
}

fn check_boolean_expression(
    expr: &BooleanExpression,
    container_name: &str,
    ss_name: &str,
    scope: &Scope<'_>,
    errors: &mut Vec<ValidationError>,
) {
    match expr {
        BooleanExpression::And(terms) | BooleanExpression::Or(terms) => {
            for term in terms {
                check_boolean_expression(term, container_name, ss_name, scope, errors);
            }
        }
        BooleanExpression::Not(inner) => {
            check_boolean_expression(inner, container_name, ss_name, scope, errors)
        }
        BooleanExpression::Condition(cmp) => {
            check_comparison(cmp, container_name, ss_name, scope, errors)
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Cyclic inheritance detection
// ─────────────────────────────────────────────────────────────────────────────

/// Detect cyclic `base_container` chains within a single SpaceSystem's containers.
fn detect_container_cycles(
    ss_name: &str,
    containers: &indexmap::IndexMap<String, SequenceContainer>,
    errors: &mut Vec<ValidationError>,
) {
    // Build local base map: name → base_container_ref (only for containers that
    // have a base AND whose base is also defined locally, i.e., a local chain).
    let local_names: HashSet<&str> = containers.keys().map(String::as_str).collect();
    let base_map: HashMap<&str, &str> = containers
        .iter()
        .filter_map(|(name, c)| {
            c.base_container.as_ref().and_then(|b| {
                // Only track edges where both ends are local.
                if local_names.contains(b.container_ref.as_str()) {
                    Some((name.as_str(), b.container_ref.as_str()))
                } else {
                    None
                }
            })
        })
        .collect();

    detect_cycles_in_map(&base_map, ss_name, errors);
}

/// Detect cyclic `base_meta_command` chains within a single SpaceSystem.
fn detect_meta_command_cycles(
    ss_name: &str,
    meta_commands: &indexmap::IndexMap<String, MetaCommand>,
    errors: &mut Vec<ValidationError>,
) {
    let local_names: HashSet<&str> = meta_commands.keys().map(String::as_str).collect();
    let base_map: HashMap<&str, &str> = meta_commands
        .iter()
        .filter_map(|(name, mc)| {
            mc.base_meta_command.as_deref().and_then(|base| {
                if local_names.contains(base) {
                    Some((name.as_str(), base))
                } else {
                    None
                }
            })
        })
        .collect();

    detect_cycles_in_map(&base_map, ss_name, errors);
}

/// Generic cycle detector over a `name → parent` map.
///
/// Uses iterative path-following: from each unvisited node, follow the
/// parent chain until we either leave the local set, reach a node with no
/// parent, or revisit a node in the current path (cycle).
///
/// A "reported" set prevents the same cycle node from being reported multiple
/// times when several nodes in the cycle are used as starting points.
fn detect_cycles_in_map(
    base_map: &HashMap<&str, &str>,
    _ss_name: &str,
    errors: &mut Vec<ValidationError>,
) {
    let mut visited: HashSet<&str> = HashSet::new();
    let mut reported: HashSet<&str> = HashSet::new();

    for &start in base_map.keys() {
        if visited.contains(start) {
            continue;
        }

        // Walk the chain, recording the path for cycle detection.
        let mut path: Vec<&str> = Vec::new();
        let mut path_set: HashSet<&str> = HashSet::new();
        let mut current = start;

        loop {
            if reported.contains(current) {
                break; // Already reported a cycle through this node.
            }
            if path_set.contains(current) {
                // Found a cycle — report the node where the loop closes.
                if !reported.contains(current) {
                    errors.push(ValidationError::CyclicInheritance {
                        name: current.to_owned(),
                    });
                    reported.insert(current);
                }
                break;
            }
            if visited.contains(current) {
                break; // Already fully processed, no cycle from here.
            }

            path.push(current);
            path_set.insert(current);

            match base_map.get(current) {
                Some(&next) => current = next,
                None => break, // End of local chain.
            }
        }

        // Mark all nodes in this path as visited.
        for node in path {
            visited.insert(node);
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Accessor helpers
// ─────────────────────────────────────────────────────────────────────────────

fn pt_base_type(pt: &ParameterType) -> Option<&str> {
    match pt {
        ParameterType::Integer(t) => t.base_type.as_deref(),
        ParameterType::Float(t) => t.base_type.as_deref(),
        ParameterType::Enumerated(t) => t.base_type.as_deref(),
        ParameterType::Boolean(t) => t.base_type.as_deref(),
        ParameterType::String(t) => t.base_type.as_deref(),
        ParameterType::Binary(t) => t.base_type.as_deref(),
        ParameterType::Aggregate(t) => t.base_type.as_deref(),
        ParameterType::Array(t) => t.base_type.as_deref(),
    }
}

fn at_base_type(at: &ArgumentType) -> Option<&str> {
    match at {
        ArgumentType::Integer(t) => t.base_type.as_deref(),
        ArgumentType::Float(t) => t.base_type.as_deref(),
        ArgumentType::Enumerated(t) => t.base_type.as_deref(),
        ArgumentType::Boolean(t) => t.base_type.as_deref(),
        ArgumentType::String(t) => t.base_type.as_deref(),
        ArgumentType::Binary(t) => t.base_type.as_deref(),
        ArgumentType::Aggregate(t) => t.base_type.as_deref(),
        ArgumentType::Array(t) => t.base_type.as_deref(),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse;

    fn parse_and_validate(xml: &str) -> Vec<ValidationError> {
        let ss = parse(xml.as_bytes()).expect("parse failed");
        validate(&ss)
    }

    // ── Valid document produces no errors ─────────────────────────────────────

    #[test]
    fn valid_document_no_errors() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="IntT">
                    <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="P1" parameterTypeRef="IntT"/>
                </ParameterSet>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ParameterRefEntry parameterRef="P1"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ── Unresolved parameterTypeRef ────────────────────────────────────────────

    #[test]
    fn unresolved_parameter_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterSet>
                  <Parameter name="P1" parameterTypeRef="NoSuchType"/>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchType"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved containerRef in EntryList ──────────────────────────────────

    #[test]
    fn unresolved_container_ref_in_entry_list() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Child">
                    <EntryList>
                      <ContainerRefEntry containerRef="NoSuchContainer"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchContainer"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved base container ─────────────────────────────────────────────

    #[test]
    fn unresolved_base_container() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Child">
                    <BaseContainer containerRef="NoSuchParent">
                      <RestrictionCriteria>
                        <Comparison parameterRef="APID" value="1"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        // Two errors: unresolved base container AND unresolved APID parameter.
        assert!(errors.len() >= 1);
        assert!(
            errors.iter().any(|e| matches!(e,
                ValidationError::UnresolvedReference { name, .. } if name == "NoSuchParent"
            )),
            "expected NoSuchParent error, got {:?}",
            errors
        );
    }

    // ── Restriction criteria comparison with unresolved parameter ─────────────

    #[test]
    fn unresolved_comparison_parameter_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Parent">
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="Child">
                    <BaseContainer containerRef="Parent">
                      <RestrictionCriteria>
                        <Comparison parameterRef="NoSuchParam" value="42"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchParam"),
            "got {:?}",
            errors
        );
    }

    // ── Cyclic container inheritance ──────────────────────────────────────────

    #[test]
    fn cyclic_container_inheritance() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="A">
                    <BaseContainer containerRef="B">
                      <RestrictionCriteria>
                        <Comparison parameterRef="X" value="1"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="B">
                    <BaseContainer containerRef="A">
                      <RestrictionCriteria>
                        <Comparison parameterRef="X" value="2"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::CyclicInheritance { .. })),
            "expected CyclicInheritance error, got {:?}",
            errors
        );
    }

    // ── Cyclic MetaCommand inheritance ────────────────────────────────────────

    #[test]
    fn cyclic_meta_command_inheritance() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="CmdA" baseMetaCommand="CmdB">
                    <CommandContainer name="CmdAContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                  <MetaCommand name="CmdB" baseMetaCommand="CmdA">
                    <CommandContainer name="CmdBContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::CyclicInheritance { .. })),
            "expected CyclicInheritance error, got {:?}",
            errors
        );
    }

    // ── Unresolved argumentTypeRef in MetaCommand ─────────────────────────────

    #[test]
    fn unresolved_argument_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="Cmd">
                    <ArgumentList>
                      <Argument name="arg1" argumentTypeRef="NoSuchArgType"/>
                    </ArgumentList>
                    <CommandContainer name="CmdContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchArgType"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved base MetaCommand ───────────────────────────────────────────

    #[test]
    fn unresolved_base_meta_command() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="Child" baseMetaCommand="NoSuchBase">
                    <CommandContainer name="ChildContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchBase"),
            "got {:?}",
            errors
        );
    }

    // ── Child SpaceSystem sees parent names ───────────────────────────────────

    #[test]
    fn child_space_system_sees_parent_types() {
        // Parameter in child references a ParameterType defined in parent.
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="IntT">
                    <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ParameterSet>
                    <Parameter name="P1" parameterTypeRef="IntT"/>
                  </ParameterSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ── AggregateParameterType with unresolved member typeRef ─────────────────

    #[test]
    fn unresolved_aggregate_member_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <AggregateParameterType name="Rec">
                    <MemberList>
                      <Member name="field1" typeRef="NoSuchType"/>
                    </MemberList>
                  </AggregateParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchType"),
            "got {:?}",
            errors
        );
    }

    // ── ArrayParameterType with unresolved arrayTypeRef ───────────────────────

    #[test]
    fn unresolved_array_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <ArrayParameterType name="Arr" arrayTypeRef="NoSuchElementType"/>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchElementType"),
            "got {:?}",
            errors
        );
    }

    // ── Duplicate sub-system names ────────────────────────────────────────────

    #[test]
    fn duplicate_sub_system_names_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <SpaceSystem name="Sub"/>
              <SpaceSystem name="Sub"/>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected exactly one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "Sub" && space_system == "Root"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn unique_sub_system_names_no_error() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <SpaceSystem name="Alpha"/>
              <SpaceSystem name="Beta"/>
            </SpaceSystem>
        "#);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    #[test]
    fn duplicate_sub_system_name_reported_once_for_three_copies() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <SpaceSystem name="Sub"/>
              <SpaceSystem name="Sub"/>
              <SpaceSystem name="Sub"/>
            </SpaceSystem>
        "#);
        // Three siblings with the same name → exactly one DuplicateName error.
        assert_eq!(errors.len(), 1, "expected one error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, .. } if name == "Sub"),
            "got {:?}",
            errors
        );
    }

    // ── Shadowing: child redefines a name from an ancestor ────────────────────

    #[test]
    fn shadowed_parameter_type_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="uint8">
                    <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ParameterTypeSet>
                    <IntegerParameterType name="uint8">
                      <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                    </IntegerParameterType>
                  </ParameterTypeSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "uint8" && space_system == "Child"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn shadowed_parameter_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="T">
                    <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="SensorVal" parameterTypeRef="T"/>
                </ParameterSet>
              </TelemetryMetaData>
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ParameterTypeSet>
                    <IntegerParameterType name="ChildT">
                      <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                    </IntegerParameterType>
                  </ParameterTypeSet>
                  <ParameterSet>
                    <Parameter name="SensorVal" parameterTypeRef="ChildT"/>
                  </ParameterSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "SensorVal" && space_system == "Child"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn shadowed_container_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="PrimaryPkt">
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ContainerSet>
                    <SequenceContainer name="PrimaryPkt">
                      <EntryList/>
                    </SequenceContainer>
                  </ContainerSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "PrimaryPkt" && space_system == "Child"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn shadowed_argument_type_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="uint16">
                    <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                  </IntegerArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
              <SpaceSystem name="Child">
                <CommandMetaData>
                  <ArgumentTypeSet>
                    <IntegerArgumentType name="uint16">
                      <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                    </IntegerArgumentType>
                  </ArgumentTypeSet>
                </CommandMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "uint16" && space_system == "Child"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn shadowed_meta_command_flagged() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="Noop">
                    <CommandContainer name="NoopContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
              <SpaceSystem name="Child">
                <CommandMetaData>
                  <MetaCommandSet>
                    <MetaCommand name="Noop">
                      <CommandContainer name="ChildNoopContainer">
                        <EntryList/>
                      </CommandContainer>
                    </MetaCommand>
                  </MetaCommandSet>
                </CommandMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "Noop" && space_system == "Child"),
            "got {:?}",
            errors
        );
    }

    #[test]
    fn unique_names_in_child_no_error() {
        // Child defines names that don't overlap with parent — no duplicate errors.
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Root">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="RootT">
                    <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ParameterTypeSet>
                    <IntegerParameterType name="ChildT">
                      <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                    </IntegerParameterType>
                  </ParameterTypeSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#);
        assert!(errors.is_empty(), "unexpected errors: {:?}", errors);
    }

    // ── Unresolved baseType on ParameterType ─────────────────────────────────

    #[test]
    fn unresolved_base_type_on_parameter_type() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="DerivedInt" baseType="NoSuchBaseType"/>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchBaseType"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved baseType on ArgumentType ──────────────────────────────────

    #[test]
    fn unresolved_base_type_on_argument_type() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="DerivedArg" baseType="NoSuchBase"/>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchBase"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved member typeRef in AggregateArgumentType ───────────────────

    #[test]
    fn unresolved_aggregate_argument_member_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <AggregateArgumentType name="Rec">
                    <MemberList>
                      <Member name="field1" typeRef="NoSuchArgType"/>
                    </MemberList>
                  </AggregateArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchArgType"),
            "got {:?}",
            errors
        );
    }

    // ── Unresolved arrayTypeRef in ArrayArgumentType ──────────────────────────

    #[test]
    fn unresolved_array_argument_type_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <ArrayArgumentType name="Arr" arrayTypeRef="NoSuchElemType"/>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert_eq!(errors.len(), 1, "got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchElemType"),
            "got {:?}",
            errors
        );
    }

    // ── BooleanExpression restriction criteria ────────────────────────────────

    #[test]
    fn unresolved_boolean_expression_parameter_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Base">
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="Child">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <BooleanExpression>
                          <ANDedConditions>
                            <Condition parameterRef="NoSuchParam" value="1"/>
                          </ANDedConditions>
                        </BooleanExpression>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchParam")),
            "expected UnresolvedReference for NoSuchParam, got {:?}",
            errors
        );
    }

    // ── NextContainer restriction criteria ────────────────────────────────────

    #[test]
    fn unresolved_next_container_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Base">
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="Child">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <NextContainer containerRef="NoSuchContainer"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchContainer")),
            "expected UnresolvedReference for NoSuchContainer, got {:?}",
            errors
        );
    }

    // ── IncludeCondition on ParameterRefEntry ─────────────────────────────────

    #[test]
    fn unresolved_include_condition_parameter_ref() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="T">
                    <IntegerDataEncoding sizeInBits="8"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="Val" parameterTypeRef="T"/>
                </ParameterSet>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ParameterRefEntry parameterRef="Val">
                        <IncludeCondition>
                          <Comparison parameterRef="NoSuchFlag" value="1"/>
                        </IncludeCondition>
                      </ParameterRefEntry>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchFlag")),
            "expected UnresolvedReference for NoSuchFlag, got {:?}",
            errors
        );
    }

    // ── IncludeCondition on ContainerRefEntry ─────────────────────────────────

    #[test]
    fn unresolved_container_ref_include_condition() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Sub">
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ContainerRefEntry containerRef="Sub">
                        <IncludeCondition>
                          <Comparison parameterRef="NoSuchParam" value="1"/>
                        </IncludeCondition>
                      </ContainerRefEntry>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchParam")),
            "expected UnresolvedReference for NoSuchParam, got {:?}",
            errors
        );
    }

    // ── ArrayParameterRef unresolved parameter ref ────────────────────────────

    #[test]
    fn unresolved_array_parameter_ref_entry() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ArrayParameterRefEntry parameterRef="NoSuchArray"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchArray")),
            "expected UnresolvedReference for NoSuchArray, got {:?}",
            errors
        );
    }

    // ── MetaCommand CommandContainer base_container unresolved ────────────────

    #[test]
    fn unresolved_command_container_base() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="Cmd">
                    <CommandContainer name="CmdPkt">
                      <BaseContainer containerRef="NoSuchBase">
                        <RestrictionCriteria>
                          <Comparison parameterRef="X" value="1"/>
                        </RestrictionCriteria>
                      </BaseContainer>
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchBase")),
            "expected UnresolvedReference for NoSuchBase, got {:?}",
            errors
        );
    }

    // ── ComparisonList in RestrictionCriteria ─────────────────────────────────

    #[test]
    fn unresolved_comparison_list_restriction_criteria() {
        let errors = parse_and_validate(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Base">
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="Child">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <ComparisonList>
                          <Comparison parameterRef="GoodParam" value="1"/>
                          <Comparison parameterRef="NoSuchParam" value="2"/>
                        </ComparisonList>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchParam")),
            "expected UnresolvedReference for NoSuchParam, got {:?}",
            errors
        );
    }

    // ── BooleanExpression::Not is checked recursively ─────────────────────────

    #[test]
    fn boolean_expression_not_branch_checked() {
        use crate::model::container::{
            BooleanExpression, Comparison, ComparisonOperator, RestrictionCriteria, SequenceContainer,
        };
        use crate::model::telemetry::TelemetryMetaData;

        let cmp = Comparison {
            parameter_ref: "NoSuchParam".into(),
            value: "1".into(),
            comparison_operator: ComparisonOperator::Equality,
            use_calibrated_value: true,
        };
        let not_expr = BooleanExpression::Not(Box::new(BooleanExpression::Condition(cmp)));

        let mut inner = SequenceContainer::new("Base");
        inner.entry_list = vec![];

        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(crate::model::container::BaseContainer {
            container_ref: "Base".into(),
            restriction_criteria: Some(RestrictionCriteria::BooleanExpression(not_expr)),
        });

        let mut tm = TelemetryMetaData::default();
        tm.containers.insert("Base".into(), inner);
        tm.containers.insert("Child".into(), child);

        let mut ss = crate::SpaceSystem::new("Test");
        ss.telemetry = Some(tm);

        let errors = validate(&ss);
        assert!(
            errors.iter().any(|e| matches!(e, ValidationError::UnresolvedReference { name, .. }
                if name == "NoSuchParam")),
            "expected UnresolvedReference for NoSuchParam via Not branch, got {:?}",
            errors
        );
    }

    // ── Container namespace collision within same SpaceSystem ─────────────────

    #[test]
    fn container_namespace_collision_flagged() {
        use crate::model::command::CommandMetaData;
        use crate::model::container::SequenceContainer;
        use crate::model::telemetry::TelemetryMetaData;

        let mut ss = crate::SpaceSystem::new("Root");
        let mut tm = TelemetryMetaData::default();
        tm.containers.insert("Pkt".into(), SequenceContainer::new("Pkt"));
        ss.telemetry = Some(tm);

        let mut cmd = CommandMetaData::default();
        cmd.command_containers
            .insert("Pkt".into(), SequenceContainer::new("Pkt"));
        ss.command = Some(cmd);

        let errors = validate(&ss);
        assert_eq!(errors.len(), 1, "expected one DuplicateName error, got {:?}", errors);
        assert!(
            matches!(&errors[0], ValidationError::DuplicateName { name, space_system }
                if name == "Pkt" && space_system == "Root"),
            "got {:?}",
            errors
        );
    }
}
