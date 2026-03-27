//! CommandMetaData, MetaCommand, and argument type definitions.

use indexmap::IndexMap;

use super::{
    container::{SequenceContainer, SequenceEntry},
    types::{
        Alias, BinaryDataEncoding, FloatDataEncoding, IntegerDataEncoding, StringDataEncoding,
        Unit, ValueEnumeration,
    },
};

/// All command-related definitions for a SpaceSystem.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CommandMetaData {
    /// Argument type definitions, keyed by name.
    pub argument_types: IndexMap<String, ArgumentType>,
    /// MetaCommand definitions, keyed by name.
    pub meta_commands: IndexMap<String, MetaCommand>,
    /// Command containers (shared structures for command packets), keyed by name.
    pub command_containers: IndexMap<String, SequenceContainer>,
}

/// A MetaCommand defines a single telecommand.
///
/// Commands support inheritance: a child MetaCommand may extend a base
/// MetaCommand, inheriting its argument list and command container entries.
#[derive(Debug, Clone, PartialEq)]
pub struct MetaCommand {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    /// Name of the base MetaCommand this command extends, if any.
    pub base_meta_command: Option<String>,
    /// If true, this command cannot be sent directly; it is a base for others.
    pub r#abstract: bool,
    pub argument_list: Vec<Argument>,
    pub command_container: Option<CommandContainer>,
}

impl MetaCommand {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            base_meta_command: None,
            r#abstract: false,
            argument_list: Vec::new(),
            command_container: None,
        }
    }
}

/// A command argument — an input value provided by the operator at send time.
#[derive(Debug, Clone, PartialEq)]
pub struct Argument {
    pub name: String,
    /// Name of the ArgumentType that describes this argument's data type.
    pub argument_type_ref: String,
    pub short_description: Option<String>,
    /// Default value to use if the operator does not provide one.
    pub initial_value: Option<String>,
}

impl Argument {
    pub fn new(name: impl Into<String>, argument_type_ref: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            argument_type_ref: argument_type_ref.into(),
            short_description: None,
            initial_value: None,
        }
    }
}

/// The command container describes the packet layout for a MetaCommand.
///
/// Like SequenceContainer, it can reference a base container and carries an
/// entry list. Unlike SequenceContainer, it can reference command arguments.
#[derive(Debug, Clone, PartialEq)]
pub struct CommandContainer {
    pub name: String,
    pub base_container: Option<super::container::BaseContainer>,
    pub entry_list: Vec<CommandEntry>,
}

/// A single entry in a command container's entry list.
#[derive(Debug, Clone, PartialEq)]
pub enum CommandEntry {
    /// A reference to a command argument to be encoded into the packet.
    ArgumentRef(ArgumentRefEntry),
    /// A reference to a fixed parameter value (e.g., a constant header field).
    ParameterRef(SequenceEntry),
    /// A fixed bit pattern (padding or constant field).
    FixedValue(FixedValueEntry),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgumentRefEntry {
    pub argument_ref: String,
    pub location: Option<super::container::EntryLocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixedValueEntry {
    pub size_in_bits: u32,
    pub binary_value: Option<String>,
    pub location: Option<super::container::EntryLocation>,
}

// ── Argument types ───────────────────────────────────────────────────────────

/// An ArgumentType defines the data type of a command argument.
///
/// Mirrors ParameterType structurally but is used exclusively for commands.
#[derive(Debug, Clone, PartialEq)]
pub enum ArgumentType {
    Integer(IntegerArgumentType),
    Float(FloatArgumentType),
    Enumerated(EnumeratedArgumentType),
    Boolean(BooleanArgumentType),
    String(StringArgumentType),
    Binary(BinaryArgumentType),
    Aggregate(AggregateArgumentType),
    Array(ArrayArgumentType),
}

impl ArgumentType {
    pub fn name(&self) -> &str {
        match self {
            ArgumentType::Integer(t) => &t.name,
            ArgumentType::Float(t) => &t.name,
            ArgumentType::Enumerated(t) => &t.name,
            ArgumentType::Boolean(t) => &t.name,
            ArgumentType::String(t) => &t.name,
            ArgumentType::Binary(t) => &t.name,
            ArgumentType::Aggregate(t) => &t.name,
            ArgumentType::Array(t) => &t.name,
        }
    }

    pub fn set_name(&mut self, name: String) {
        match self {
            ArgumentType::Integer(t) => t.name = name,
            ArgumentType::Float(t) => t.name = name,
            ArgumentType::Enumerated(t) => t.name = name,
            ArgumentType::Boolean(t) => t.name = name,
            ArgumentType::String(t) => t.name = name,
            ArgumentType::Binary(t) => t.name = name,
            ArgumentType::Aggregate(t) => t.name = name,
            ArgumentType::Array(t) => t.name = name,
        }
    }

    pub fn short_description(&self) -> Option<&str> {
        match self {
            ArgumentType::Integer(t) => t.short_description.as_deref(),
            ArgumentType::Float(t) => t.short_description.as_deref(),
            ArgumentType::Enumerated(t) => t.short_description.as_deref(),
            ArgumentType::Boolean(t) => t.short_description.as_deref(),
            ArgumentType::String(t) => t.short_description.as_deref(),
            ArgumentType::Binary(t) => t.short_description.as_deref(),
            ArgumentType::Aggregate(t) => t.short_description.as_deref(),
            ArgumentType::Array(t) => t.short_description.as_deref(),
        }
    }

    pub fn set_short_description(&mut self, desc: Option<String>) {
        match self {
            ArgumentType::Integer(t) => t.short_description = desc,
            ArgumentType::Float(t) => t.short_description = desc,
            ArgumentType::Enumerated(t) => t.short_description = desc,
            ArgumentType::Boolean(t) => t.short_description = desc,
            ArgumentType::String(t) => t.short_description = desc,
            ArgumentType::Binary(t) => t.short_description = desc,
            ArgumentType::Aggregate(t) => t.short_description = desc,
            ArgumentType::Array(t) => t.short_description = desc,
        }
    }

    pub fn set_base_type(&mut self, base: Option<String>) {
        match self {
            ArgumentType::Integer(t) => t.base_type = base,
            ArgumentType::Float(t) => t.base_type = base,
            ArgumentType::Enumerated(t) => t.base_type = base,
            ArgumentType::Boolean(t) => t.base_type = base,
            ArgumentType::String(t) => t.base_type = base,
            ArgumentType::Binary(t) => t.base_type = base,
            ArgumentType::Aggregate(t) => t.base_type = base,
            ArgumentType::Array(t) => t.base_type = base,
        }
    }

    pub fn unit_set_mut(&mut self) -> &mut Vec<crate::model::types::Unit> {
        match self {
            ArgumentType::Integer(t)    => &mut t.unit_set,
            ArgumentType::Float(t)      => &mut t.unit_set,
            ArgumentType::Enumerated(t) => &mut t.unit_set,
            ArgumentType::Boolean(t)    => &mut t.unit_set,
            ArgumentType::String(t)     => &mut t.unit_set,
            ArgumentType::Binary(t)     => &mut t.unit_set,
            ArgumentType::Aggregate(t)  => &mut t.unit_set,
            ArgumentType::Array(t)      => &mut t.unit_set,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegerArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub signed: bool,
    pub size_in_bits: Option<u32>,
    pub encoding: Option<IntegerDataEncoding>,
    pub valid_range: Option<IntegerArgumentRange>,
    pub initial_value: Option<i64>,
}

impl IntegerArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            signed: true,
            size_in_bits: None,
            encoding: None,
            valid_range: None,
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegerArgumentRange {
    pub min_inclusive: Option<i64>,
    pub max_inclusive: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub size_in_bits: Option<u32>,
    pub encoding: Option<FloatDataEncoding>,
    pub initial_value: Option<f64>,
}

impl FloatArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            size_in_bits: None,
            encoding: None,
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumeratedArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<IntegerDataEncoding>,
    pub enumeration_list: Vec<ValueEnumeration>,
    pub initial_value: Option<String>,
}

impl EnumeratedArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            enumeration_list: Vec::new(),
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BooleanArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<IntegerDataEncoding>,
    pub one_string_value: Option<String>,
    pub zero_string_value: Option<String>,
}

impl BooleanArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            one_string_value: None,
            zero_string_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct StringArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<StringDataEncoding>,
    pub initial_value: Option<String>,
}

impl StringArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<BinaryDataEncoding>,
    pub initial_value: Option<String>,
}

impl BinaryArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            initial_value: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggregateArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub member_list: Vec<ArgumentMember>,
}

impl AggregateArgumentType {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            member_list: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArgumentMember {
    pub name: String,
    pub type_ref: String,
    pub short_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayArgumentType {
    pub name: String,
    pub short_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub array_type_ref: String,
    pub number_of_dimensions: u32,
}

impl ArrayArgumentType {
    pub fn new(name: impl Into<String>, array_type_ref: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            array_type_ref: array_type_ref.into(),
            number_of_dimensions: 1,
        }
    }
}
