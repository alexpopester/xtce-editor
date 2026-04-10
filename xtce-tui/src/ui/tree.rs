//! Tree data model and builder.
//!
//! [`NodeId`] uniquely identifies any node in the SpaceSystem tree.
//! [`TreeNode`] is a single visible row in the flattened tree view.
//! [`build_tree`] converts a [`SpaceSystem`] model into a `Vec<TreeNode>`,
//! honouring the current expansion state.

use std::collections::HashSet;

use xtce_core::model::command::ArgumentType;
use xtce_core::model::telemetry::ParameterType;
use xtce_core::SpaceSystem;

// ─────────────────────────────────────────────────────────────────────────────
// NodeId — stable identity for every possible tree node
// ─────────────────────────────────────────────────────────────────────────────

/// Path of SpaceSystem names from the root SpaceSystem to a given node.
///
/// An empty path refers to the root SpaceSystem. A path `["Child"]` refers
/// to the SpaceSystem named "Child" directly under the root.
pub type SsPath = Vec<String>;

/// Uniquely identifies a node in the SpaceSystem tree.
///
/// Used as the key in the expansion state [`HashSet`] and to look up
/// the corresponding model data for the detail panel.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum NodeId {
    /// A SpaceSystem node. The path locates it from the root.
    SpaceSystem(SsPath),
    /// The TelemetryMetaData section of a SpaceSystem.
    TmSection(SsPath),
    /// The "Parameter Types" group node inside TelemetryMetaData.
    TmParameterTypes(SsPath),
    /// A specific ParameterType.
    TmParameterType(SsPath, String),
    /// The "Parameters" group node.
    TmParameters(SsPath),
    /// A specific Parameter.
    TmParameter(SsPath, String),
    /// The "Containers" group node.
    TmContainers(SsPath),
    /// A specific SequenceContainer.
    TmContainer(SsPath, String),
    /// The CommandMetaData section of a SpaceSystem.
    CmdSection(SsPath),
    /// The "Argument Types" group node inside CommandMetaData.
    CmdArgumentTypes(SsPath),
    /// A specific ArgumentType.
    CmdArgumentType(SsPath, String),
    /// The "MetaCommands" group node.
    CmdMetaCommands(SsPath),
    /// A specific MetaCommand.
    CmdMetaCommand(SsPath, String),
}

// ─────────────────────────────────────────────────────────────────────────────
// TreeNode — one visible row in the tree
// ─────────────────────────────────────────────────────────────────────────────

/// A single visible row in the flattened tree panel.
#[derive(Clone, Debug)]
pub struct TreeNode {
    /// Indentation level (0 = root).
    pub depth: usize,
    /// Text to display for this row.
    pub label: String,
    /// Optional type annotation shown after the label (e.g. " (Integer)").
    pub annotation: Option<String>,
    /// Stable identity — used for expansion state and detail lookup.
    pub node_id: NodeId,
    /// Whether this node can be expanded/collapsed.
    pub expandable: bool,
    /// Whether this node is currently expanded.
    pub expanded: bool,
}

// ─────────────────────────────────────────────────────────────────────────────
// Tree builder
// ─────────────────────────────────────────────────────────────────────────────

/// Build the flat list of visible [`TreeNode`]s from a [`SpaceSystem`].
///
/// `expanded` controls which nodes are open. Call this again whenever the
/// expansion state or the model changes.
pub fn build_tree(root: &SpaceSystem, expanded: &HashSet<NodeId>) -> Vec<TreeNode> {
    let mut nodes = Vec::new();
    add_space_system(root, vec![], 0, expanded, &mut nodes);
    nodes
}

fn add_space_system(
    ss: &SpaceSystem,
    ss_path: SsPath,
    depth: usize,
    expanded: &HashSet<NodeId>,
    out: &mut Vec<TreeNode>,
) {
    let node_id = NodeId::SpaceSystem(ss_path.clone());
    let has_children =
        ss.telemetry.is_some() || ss.command.is_some() || !ss.sub_systems.is_empty();
    let is_expanded = expanded.contains(&node_id);

    out.push(TreeNode {
        depth,
        label: ss.name.clone(),
        annotation: None,
        node_id,
        expandable: has_children,
        expanded: is_expanded,
    });

    if !is_expanded {
        return;
    }

    // TelemetryMetaData section
    if let Some(tm) = &ss.telemetry {
        let tm_id = NodeId::TmSection(ss_path.clone());
        let tm_has =
            !tm.parameter_types.is_empty() || !tm.parameters.is_empty() || !tm.containers.is_empty();
        let tm_exp = expanded.contains(&tm_id);

        out.push(TreeNode {
            depth: depth + 1,
            label: "Telemetry".to_string(),
            annotation: None,
            node_id: tm_id,
            expandable: tm_has,
            expanded: tm_exp,
        });

        if tm_exp {
            // ParameterTypes group
            if !tm.parameter_types.is_empty() {
                let ptg_id = NodeId::TmParameterTypes(ss_path.clone());
                let ptg_exp = expanded.contains(&ptg_id);
                let count = tm.parameter_types.len();
                out.push(TreeNode {
                    depth: depth + 2,
                    label: "Parameter Types".to_string(),
                    annotation: Some(format!("({})", count)),
                    node_id: ptg_id,
                    expandable: true,
                    expanded: ptg_exp,
                });
                if ptg_exp {
                    for (name, pt) in &tm.parameter_types {
                        out.push(TreeNode {
                            depth: depth + 3,
                            label: name.clone(),
                            annotation: Some(format!("({})", pt_kind(pt))),
                            node_id: NodeId::TmParameterType(ss_path.clone(), name.clone()),
                            expandable: false,
                            expanded: false,
                        });
                    }
                }
            }

            // Parameters group
            if !tm.parameters.is_empty() {
                let pg_id = NodeId::TmParameters(ss_path.clone());
                let pg_exp = expanded.contains(&pg_id);
                let count = tm.parameters.len();
                out.push(TreeNode {
                    depth: depth + 2,
                    label: "Parameters".to_string(),
                    annotation: Some(format!("({})", count)),
                    node_id: pg_id,
                    expandable: true,
                    expanded: pg_exp,
                });
                if pg_exp {
                    for name in tm.parameters.keys() {
                        out.push(TreeNode {
                            depth: depth + 3,
                            label: name.clone(),
                            annotation: None,
                            node_id: NodeId::TmParameter(ss_path.clone(), name.clone()),
                            expandable: false,
                            expanded: false,
                        });
                    }
                }
            }

            // Containers group
            if !tm.containers.is_empty() {
                let cg_id = NodeId::TmContainers(ss_path.clone());
                let cg_exp = expanded.contains(&cg_id);
                let count = tm.containers.len();
                out.push(TreeNode {
                    depth: depth + 2,
                    label: "Containers".to_string(),
                    annotation: Some(format!("({})", count)),
                    node_id: cg_id,
                    expandable: true,
                    expanded: cg_exp,
                });
                if cg_exp {
                    for name in tm.containers.keys() {
                        out.push(TreeNode {
                            depth: depth + 3,
                            label: name.clone(),
                            annotation: None,
                            node_id: NodeId::TmContainer(ss_path.clone(), name.clone()),
                            expandable: false,
                            expanded: false,
                        });
                    }
                }
            }
        }
    }

    // CommandMetaData section
    if let Some(cmd) = &ss.command {
        let cmd_id = NodeId::CmdSection(ss_path.clone());
        let cmd_has = !cmd.argument_types.is_empty() || !cmd.meta_commands.is_empty();
        let cmd_exp = expanded.contains(&cmd_id);

        out.push(TreeNode {
            depth: depth + 1,
            label: "Commands".to_string(),
            annotation: None,
            node_id: cmd_id,
            expandable: cmd_has,
            expanded: cmd_exp,
        });

        if cmd_exp {
            // ArgumentTypes group
            if !cmd.argument_types.is_empty() {
                let atg_id = NodeId::CmdArgumentTypes(ss_path.clone());
                let atg_exp = expanded.contains(&atg_id);
                let count = cmd.argument_types.len();
                out.push(TreeNode {
                    depth: depth + 2,
                    label: "Argument Types".to_string(),
                    annotation: Some(format!("({})", count)),
                    node_id: atg_id,
                    expandable: true,
                    expanded: atg_exp,
                });
                if atg_exp {
                    for (name, at) in &cmd.argument_types {
                        out.push(TreeNode {
                            depth: depth + 3,
                            label: name.clone(),
                            annotation: Some(format!("({})", at_kind(at))),
                            node_id: NodeId::CmdArgumentType(ss_path.clone(), name.clone()),
                            expandable: false,
                            expanded: false,
                        });
                    }
                }
            }

            // MetaCommands group
            if !cmd.meta_commands.is_empty() {
                let mcg_id = NodeId::CmdMetaCommands(ss_path.clone());
                let mcg_exp = expanded.contains(&mcg_id);
                let count = cmd.meta_commands.len();
                out.push(TreeNode {
                    depth: depth + 2,
                    label: "MetaCommands".to_string(),
                    annotation: Some(format!("({})", count)),
                    node_id: mcg_id,
                    expandable: true,
                    expanded: mcg_exp,
                });
                if mcg_exp {
                    for name in cmd.meta_commands.keys() {
                        out.push(TreeNode {
                            depth: depth + 3,
                            label: name.clone(),
                            annotation: None,
                            node_id: NodeId::CmdMetaCommand(ss_path.clone(), name.clone()),
                            expandable: false,
                            expanded: false,
                        });
                    }
                }
            }
        }
    }

    // Child SpaceSystems
    for child in &ss.sub_systems {
        let mut child_path = ss_path.clone();
        child_path.push(child.name.clone());
        add_space_system(child, child_path, depth + 1, expanded, out);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Full-tree enumeration (expansion-independent)
// ─────────────────────────────────────────────────────────────────────────────

/// Return every possible `(NodeId, label)` pair in the SpaceSystem hierarchy,
/// regardless of the current expansion state.
///
/// Used by the search feature so that collapsed nodes are still findable.
pub fn enumerate_all_nodes(root: &SpaceSystem) -> Vec<(NodeId, String)> {
    let mut out = Vec::new();
    enum_ss(root, vec![], &mut out);
    out
}

fn enum_ss(ss: &SpaceSystem, path: SsPath, out: &mut Vec<(NodeId, String)>) {
    out.push((NodeId::SpaceSystem(path.clone()), ss.name.clone()));

    if let Some(tm) = &ss.telemetry {
        out.push((NodeId::TmSection(path.clone()), "Telemetry".to_string()));
        if !tm.parameter_types.is_empty() {
            out.push((NodeId::TmParameterTypes(path.clone()), "Parameter Types".to_string()));
            for name in tm.parameter_types.keys() {
                out.push((NodeId::TmParameterType(path.clone(), name.clone()), name.clone()));
            }
        }
        if !tm.parameters.is_empty() {
            out.push((NodeId::TmParameters(path.clone()), "Parameters".to_string()));
            for name in tm.parameters.keys() {
                out.push((NodeId::TmParameter(path.clone(), name.clone()), name.clone()));
            }
        }
        if !tm.containers.is_empty() {
            out.push((NodeId::TmContainers(path.clone()), "Containers".to_string()));
            for name in tm.containers.keys() {
                out.push((NodeId::TmContainer(path.clone(), name.clone()), name.clone()));
            }
        }
    }

    if let Some(cmd) = &ss.command {
        out.push((NodeId::CmdSection(path.clone()), "Commands".to_string()));
        if !cmd.argument_types.is_empty() {
            out.push((NodeId::CmdArgumentTypes(path.clone()), "Argument Types".to_string()));
            for name in cmd.argument_types.keys() {
                out.push((NodeId::CmdArgumentType(path.clone(), name.clone()), name.clone()));
            }
        }
        if !cmd.meta_commands.is_empty() {
            out.push((NodeId::CmdMetaCommands(path.clone()), "MetaCommands".to_string()));
            for name in cmd.meta_commands.keys() {
                out.push((NodeId::CmdMetaCommand(path.clone(), name.clone()), name.clone()));
            }
        }
    }

    for child in &ss.sub_systems {
        let mut child_path = path.clone();
        child_path.push(child.name.clone());
        enum_ss(child, child_path, out);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Navigation helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Walk the SpaceSystem tree to find the node at `path`.
///
/// An empty path returns `root`. A path `["Child"]` returns the SpaceSystem
/// named "Child" directly under root.
pub fn get_ss<'a>(root: &'a SpaceSystem, path: &[String]) -> Option<&'a SpaceSystem> {
    let mut current = root;
    for name in path {
        current = current.sub_systems.iter().find(|ss| &ss.name == name)?;
    }
    Some(current)
}

// ─────────────────────────────────────────────────────────────────────────────
// Display helpers
// ─────────────────────────────────────────────────────────────────────────────

fn pt_kind(pt: &ParameterType) -> &'static str {
    match pt {
        ParameterType::Integer(_)      => "Integer",
        ParameterType::Float(_)        => "Float",
        ParameterType::Enumerated(_)   => "Enum",
        ParameterType::Boolean(_)      => "Boolean",
        ParameterType::String(_)       => "String",
        ParameterType::Binary(_)       => "Binary",
        ParameterType::Aggregate(_)    => "Aggregate",
        ParameterType::Array(_)        => "Array",
        ParameterType::AbsoluteTime(_) => "AbsoluteTime",
        ParameterType::RelativeTime(_) => "RelativeTime",
    }
}

fn at_kind(at: &ArgumentType) -> &'static str {
    match at {
        ArgumentType::Integer(_) => "Integer",
        ArgumentType::Float(_) => "Float",
        ArgumentType::Enumerated(_) => "Enum",
        ArgumentType::Boolean(_) => "Boolean",
        ArgumentType::String(_) => "String",
        ArgumentType::Binary(_) => "Binary",
        ArgumentType::Aggregate(_) => "Aggregate",
        ArgumentType::Array(_) => "Array",
    }
}
