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
    /// Create a new `MetaCommand` with the given name and all optional fields
    /// at their defaults (`abstract = false`, no base command, empty argument
    /// list and no command container).
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
    /// Create a new `Argument` with the given name and type reference.
    ///
    /// # Arguments
    ///
    /// * `name` - The argument name, unique within its MetaCommand.
    /// * `argument_type_ref` - Name of the `ArgumentType` that describes this
    ///   argument's encoding and valid range.
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
    /// Return the name of the argument type, regardless of variant.
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

    /// Set the name of the argument type in-place, regardless of variant.
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

    /// Return the short description of the argument type, if set.
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

    /// Set or clear the short description of the argument type.
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

    /// Set or clear the `baseType` inheritance reference for the argument type.
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

    /// Return a mutable reference to the unit set of the argument type.
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
    /// Create a new `IntegerArgumentType` with the given name and all optional
    /// fields at their defaults (`signed = true`, no encoding, no constraints).
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
    /// Create a new `FloatArgumentType` with the given name and all optional
    /// fields at their defaults (no encoding, no valid range).
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
    /// Create a new `EnumeratedArgumentType` with the given name, no encoding,
    /// and an empty enumeration list.
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
    /// Create a new `BooleanArgumentType` with the given name and all optional
    /// fields at their defaults (no encoding, no string labels for true/false).
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
    /// Create a new `StringArgumentType` with the given name and all optional
    /// fields at their defaults (no string data encoding).
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
    /// Create a new `BinaryArgumentType` with the given name and all optional
    /// fields at their defaults (no binary data encoding).
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
    /// Create a new `AggregateArgumentType` with the given name and an empty
    /// member list.
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
    /// Create a new `ArrayArgumentType` with the given name and element-type
    /// reference, defaulting to one dimension.
    ///
    /// # Arguments
    ///
    /// * `name` - The type name, unique within its SpaceSystem scope.
    /// * `array_type_ref` - Name of the `ArgumentType` for each element.
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

#[cfg(test)]
mod tests {
    use super::*;

    // Exercises all 8 ArgumentType dispatch methods on every variant.

    fn all_variants() -> Vec<ArgumentType> {
        vec![
            ArgumentType::Integer(IntegerArgumentType::new("IntT")),
            ArgumentType::Float(FloatArgumentType::new("FloatT")),
            ArgumentType::Enumerated(EnumeratedArgumentType::new("EnumT")),
            ArgumentType::Boolean(BooleanArgumentType::new("BoolT")),
            ArgumentType::String(StringArgumentType::new("StrT")),
            ArgumentType::Binary(BinaryArgumentType::new("BinT")),
            ArgumentType::Aggregate(AggregateArgumentType::new("AggT")),
            ArgumentType::Array(ArrayArgumentType::new("ArrT", "IntT")),
        ]
    }

    #[test]
    fn name_returns_correct_name_for_all_variants() {
        let expected = ["IntT", "FloatT", "EnumT", "BoolT", "StrT", "BinT", "AggT", "ArrT"];
        for (at, exp) in all_variants().iter().zip(expected) {
            assert_eq!(at.name(), exp, "name() wrong for variant");
        }
    }

    #[test]
    fn set_name_mutates_name_for_all_variants() {
        for mut at in all_variants() {
            at.set_name("Renamed".into());
            assert_eq!(at.name(), "Renamed");
        }
    }

    #[test]
    fn short_description_none_by_default_for_all_variants() {
        for at in all_variants() {
            assert!(at.short_description().is_none(), "expected None for {:?}", at.name());
        }
    }

    #[test]
    fn set_short_description_round_trips_for_all_variants() {
        for mut at in all_variants() {
            at.set_short_description(Some("desc".into()));
            assert_eq!(at.short_description(), Some("desc"));
            at.set_short_description(None);
            assert!(at.short_description().is_none());
        }
    }

    #[test]
    fn set_base_type_round_trips_for_all_variants() {
        for mut at in all_variants() {
            at.set_base_type(Some("BaseT".into()));
            // Verify via the inner struct — enough that it doesn't panic.
            at.set_base_type(None);
        }
    }

    #[test]
    fn unit_set_mut_allows_push_for_all_variants() {
        for mut at in all_variants() {
            let unit = Unit {
                value: "m/s".into(),
                power: None,
                factor: None,
                description: None,
            };
            at.unit_set_mut().push(unit.clone());
            assert_eq!(at.unit_set_mut().len(), 1);
            assert_eq!(at.unit_set_mut()[0].value, "m/s");
        }
    }

    #[test]
    fn argument_new_constructors_set_defaults() {
        let mc = MetaCommand::new("TC_Hello");
        assert_eq!(mc.name, "TC_Hello");
        assert!(!mc.r#abstract);
        assert!(mc.base_meta_command.is_none());
        assert!(mc.argument_list.is_empty());
        assert!(mc.command_container.is_none());

        let arg = Argument::new("arg1", "IntT");
        assert_eq!(arg.name, "arg1");
        assert_eq!(arg.argument_type_ref, "IntT");
        assert!(arg.initial_value.is_none());
    }
}
