//! TelemetryMetaData and all ParameterType variants.

use indexmap::IndexMap;

use super::{
    container::SequenceContainer,
    types::{
        Alias, BinaryDataEncoding, Calibrator, FloatDataEncoding, IntegerDataEncoding,
        StringDataEncoding, Unit, ValueEnumeration,
    },
};

/// Encoding used inside `AbsoluteTimeParameterType` and `RelativeTimeParameterType`.
/// XTCE wraps the encoding in a `<Encoding>` element; the inner element selects the variant.
#[derive(Debug, Clone, PartialEq)]
pub enum TimeEncoding {
    Integer(IntegerDataEncoding),
    Float(FloatDataEncoding),
}

/// All telemetry-related definitions for a SpaceSystem.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TelemetryMetaData {
    /// All parameter type definitions, keyed by name for fast lookup.
    pub parameter_types: IndexMap<String, ParameterType>,
    /// All parameter definitions, keyed by name.
    pub parameters: IndexMap<String, Parameter>,
    /// Sequence containers (packet structures), keyed by name.
    pub containers: IndexMap<String, SequenceContainer>,
}

/// A telemetry parameter — a named, typed data item extracted from a container.
#[derive(Debug, Clone, PartialEq)]
pub struct Parameter {
    pub name: String,
    /// Name of the ParameterType that describes this parameter's data type.
    pub parameter_type_ref: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub parameter_properties: Option<ParameterProperties>,
}

impl Parameter {
    /// Create a new `Parameter` with the given name and type reference.
    ///
    /// All other fields are set to their defaults (`None` / empty).
    ///
    /// # Arguments
    ///
    /// * `name` - The parameter name, unique within its SpaceSystem scope.
    /// * `parameter_type_ref` - Name of the `ParameterType` that describes this
    ///   parameter's encoding and engineering units.
    pub fn new(name: impl Into<String>, parameter_type_ref: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            parameter_type_ref: parameter_type_ref.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            parameter_properties: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
pub struct ParameterProperties {
    pub data_source: Option<DataSource>,
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DataSource {
    Telemetered,
    Derived,
    Constant,
    Local,
    Ground,
}

/// A ParameterType defines the data type of one or more parameters.
///
/// All variants share a common set of fields (name, description, units).
/// The variant determines the encoding and calibration rules.
#[derive(Debug, Clone, PartialEq)]
pub enum ParameterType {
    Integer(IntegerParameterType),
    Float(FloatParameterType),
    Enumerated(EnumeratedParameterType),
    Boolean(BooleanParameterType),
    String(StringParameterType),
    Binary(BinaryParameterType),
    Aggregate(AggregateParameterType),
    Array(ArrayParameterType),
    AbsoluteTime(AbsoluteTimeParameterType),
    RelativeTime(RelativeTimeParameterType),
}

impl ParameterType {
    /// Return the name of the parameter type, regardless of variant.
    pub fn name(&self) -> &str {
        match self {
            ParameterType::Integer(t)      => &t.name,
            ParameterType::Float(t)        => &t.name,
            ParameterType::Enumerated(t)   => &t.name,
            ParameterType::Boolean(t)      => &t.name,
            ParameterType::String(t)       => &t.name,
            ParameterType::Binary(t)       => &t.name,
            ParameterType::Aggregate(t)    => &t.name,
            ParameterType::Array(t)        => &t.name,
            ParameterType::AbsoluteTime(t) => &t.name,
            ParameterType::RelativeTime(t) => &t.name,
        }
    }

    /// Set the name of the parameter type in-place, regardless of variant.
    pub fn set_name(&mut self, name: String) {
        match self {
            ParameterType::Integer(t)      => t.name = name,
            ParameterType::Float(t)        => t.name = name,
            ParameterType::Enumerated(t)   => t.name = name,
            ParameterType::Boolean(t)      => t.name = name,
            ParameterType::String(t)       => t.name = name,
            ParameterType::Binary(t)       => t.name = name,
            ParameterType::Aggregate(t)    => t.name = name,
            ParameterType::Array(t)        => t.name = name,
            ParameterType::AbsoluteTime(t) => t.name = name,
            ParameterType::RelativeTime(t) => t.name = name,
        }
    }

    /// Return the short description of the parameter type, if set.
    pub fn short_description(&self) -> Option<&str> {
        match self {
            ParameterType::Integer(t)      => t.short_description.as_deref(),
            ParameterType::Float(t)        => t.short_description.as_deref(),
            ParameterType::Enumerated(t)   => t.short_description.as_deref(),
            ParameterType::Boolean(t)      => t.short_description.as_deref(),
            ParameterType::String(t)       => t.short_description.as_deref(),
            ParameterType::Binary(t)       => t.short_description.as_deref(),
            ParameterType::Aggregate(t)    => t.short_description.as_deref(),
            ParameterType::Array(t)        => t.short_description.as_deref(),
            ParameterType::AbsoluteTime(t) => t.short_description.as_deref(),
            ParameterType::RelativeTime(t) => t.short_description.as_deref(),
        }
    }

    /// Set or clear the short description of the parameter type.
    pub fn set_short_description(&mut self, desc: Option<String>) {
        match self {
            ParameterType::Integer(t)      => t.short_description = desc,
            ParameterType::Float(t)        => t.short_description = desc,
            ParameterType::Enumerated(t)   => t.short_description = desc,
            ParameterType::Boolean(t)      => t.short_description = desc,
            ParameterType::String(t)       => t.short_description = desc,
            ParameterType::Binary(t)       => t.short_description = desc,
            ParameterType::Aggregate(t)    => t.short_description = desc,
            ParameterType::Array(t)        => t.short_description = desc,
            ParameterType::AbsoluteTime(t) => t.short_description = desc,
            ParameterType::RelativeTime(t) => t.short_description = desc,
        }
    }

    /// Set or clear the `baseType` inheritance reference for the parameter type.
    pub fn set_base_type(&mut self, base: Option<String>) {
        match self {
            ParameterType::Integer(t)      => t.base_type = base,
            ParameterType::Float(t)        => t.base_type = base,
            ParameterType::Enumerated(t)   => t.base_type = base,
            ParameterType::Boolean(t)      => t.base_type = base,
            ParameterType::String(t)       => t.base_type = base,
            ParameterType::Binary(t)       => t.base_type = base,
            ParameterType::Aggregate(t)    => t.base_type = base,
            ParameterType::Array(t)        => t.base_type = base,
            ParameterType::AbsoluteTime(t) => t.base_type = base,
            ParameterType::RelativeTime(t) => t.base_type = base,
        }
    }

    /// Return a mutable reference to the unit set of the parameter type.
    ///
    /// Used by the TUI unit editor to modify units in-place without needing to
    /// know the concrete variant.
    pub fn unit_set_mut(&mut self) -> &mut Vec<crate::model::types::Unit> {
        match self {
            ParameterType::Integer(t)      => &mut t.unit_set,
            ParameterType::Float(t)        => &mut t.unit_set,
            ParameterType::Enumerated(t)   => &mut t.unit_set,
            ParameterType::Boolean(t)      => &mut t.unit_set,
            ParameterType::String(t)       => &mut t.unit_set,
            ParameterType::Binary(t)       => &mut t.unit_set,
            ParameterType::Aggregate(t)    => &mut t.unit_set,
            ParameterType::Array(t)        => &mut t.unit_set,
            ParameterType::AbsoluteTime(t) => &mut t.unit_set,
            ParameterType::RelativeTime(t) => &mut t.unit_set,
        }
    }
}

// ── Common fields shared by all parameter types ─────────────────────────────

/// Fields common to all ParameterType variants, pulled out for ergonomics.
#[derive(Debug, Clone, PartialEq)]
pub struct ParameterTypeBase {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    /// Name of a base type to inherit from (type inheritance, not container inheritance).
    pub base_type: Option<String>,
}

impl ParameterTypeBase {
    /// Create a new `ParameterTypeBase` with the given name and all optional
    /// fields set to their defaults (empty / `None`).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
        }
    }
}

// ── Concrete parameter type variants ────────────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub struct IntegerParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub signed: bool,
    pub size_in_bits: Option<u32>,
    pub encoding: Option<IntegerDataEncoding>,
    pub valid_range: Option<IntegerValidRange>,
    pub default_alarm: Option<IntegerAlarm>,
}

impl IntegerParameterType {
    /// Create a new `IntegerParameterType` with the given name and all optional
    /// fields at their defaults (`signed = true`, no encoding, no constraints).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            signed: true,
            size_in_bits: None,
            encoding: None,
            valid_range: None,
            default_alarm: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegerValidRange {
    pub min_inclusive: Option<i64>,
    pub max_inclusive: Option<i64>,
    pub min_exclusive: Option<i64>,
    pub max_exclusive: Option<i64>,
    pub valid_range_applies_to_calibrated: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct IntegerAlarm {
    pub min_inclusive: Option<i64>,
    pub max_inclusive: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub size_in_bits: Option<u32>,
    pub encoding: Option<FloatDataEncoding>,
    pub valid_range: Option<FloatValidRange>,
    pub default_calibrator: Option<Calibrator>,
}

impl FloatParameterType {
    /// Create a new `FloatParameterType` with the given name and all optional
    /// fields at their defaults (no encoding, no valid range, no calibrator).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            size_in_bits: None,
            encoding: None,
            valid_range: None,
            default_calibrator: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct FloatValidRange {
    pub min_inclusive: Option<f64>,
    pub max_inclusive: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct EnumeratedParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<IntegerDataEncoding>,
    /// The ordered list of integer→label mappings.
    pub enumeration_list: Vec<ValueEnumeration>,
}

impl EnumeratedParameterType {
    /// Create a new `EnumeratedParameterType` with the given name, no encoding,
    /// and an empty enumeration list.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            enumeration_list: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BooleanParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<IntegerDataEncoding>,
    /// String representation of the `true` state.
    pub one_string_value: Option<String>,
    /// String representation of the `false` state.
    pub zero_string_value: Option<String>,
}

impl BooleanParameterType {
    /// Create a new `BooleanParameterType` with the given name and all optional
    /// fields at their defaults (no encoding, no string labels for true/false).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
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
pub struct StringParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<StringDataEncoding>,
}

impl StringParameterType {
    /// Create a new `StringParameterType` with the given name and all optional
    /// fields at their defaults (no string data encoding).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BinaryParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub encoding: Option<BinaryDataEncoding>,
}

impl BinaryParameterType {
    /// Create a new `BinaryParameterType` with the given name and all optional
    /// fields at their defaults (no binary data encoding).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
        }
    }
}

/// An aggregate (struct-like) type whose value is a record of named members.
#[derive(Debug, Clone, PartialEq)]
pub struct AggregateParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    pub member_list: Vec<Member>,
}

impl AggregateParameterType {
    /// Create a new `AggregateParameterType` with the given name and an empty
    /// member list.
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            member_list: Vec::new(),
        }
    }
}

/// A single named member of an AggregateParameterType.
#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: String,
    pub type_ref: String,
    pub short_description: Option<String>,
}

/// An array type whose element type is given by `array_type_ref`.
#[derive(Debug, Clone, PartialEq)]
pub struct ArrayParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    /// Reference to the ParameterType of each array element.
    pub array_type_ref: String,
    pub number_of_dimensions: u32,
}

impl ArrayParameterType {
    /// Create a new `ArrayParameterType` with the given name and element-type
    /// reference, defaulting to one dimension.
    ///
    /// # Arguments
    ///
    /// * `name` - The type name, unique within its SpaceSystem scope.
    /// * `array_type_ref` - Name of the `ParameterType` for each element.
    pub fn new(name: impl Into<String>, array_type_ref: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            array_type_ref: array_type_ref.into(),
            number_of_dimensions: 1,
        }
    }
}

/// An absolute-time parameter type (XTCE `AbsoluteTimeParameterType`).
///
/// Represents a timestamp with an optional data encoding and an optional
/// epoch reference (e.g. "UNIX", "GPS", "J2000", "TAI").
#[derive(Debug, Clone, PartialEq)]
pub struct AbsoluteTimeParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    /// Optional data encoding for the raw time value.
    pub encoding: Option<TimeEncoding>,
    /// Optional epoch string (e.g. "UNIX", "GPS", "J2000", "TAI").
    pub reference_time: Option<String>,
}

impl AbsoluteTimeParameterType {
    /// Create a new `AbsoluteTimeParameterType` with the given name and all
    /// optional fields at their defaults (no encoding, no epoch reference).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
            reference_time: None,
        }
    }
}

/// A relative-time (duration) parameter type (XTCE `RelativeTimeParameterType`).
#[derive(Debug, Clone, PartialEq)]
pub struct RelativeTimeParameterType {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    pub unit_set: Vec<Unit>,
    pub base_type: Option<String>,
    /// Optional data encoding for the raw duration value.
    pub encoding: Option<TimeEncoding>,
}

impl RelativeTimeParameterType {
    /// Create a new `RelativeTimeParameterType` with the given name and all
    /// optional fields at their defaults (no encoding).
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            unit_set: Vec::new(),
            base_type: None,
            encoding: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::types::Unit;

    fn all_variants() -> Vec<ParameterType> {
        vec![
            ParameterType::Integer(IntegerParameterType::new("IntT")),
            ParameterType::Float(FloatParameterType::new("FloatT")),
            ParameterType::Enumerated(EnumeratedParameterType::new("EnumT")),
            ParameterType::Boolean(BooleanParameterType::new("BoolT")),
            ParameterType::String(StringParameterType::new("StrT")),
            ParameterType::Binary(BinaryParameterType::new("BinT")),
            ParameterType::Aggregate(AggregateParameterType::new("AggT")),
            ParameterType::Array(ArrayParameterType::new("ArrT", "IntT")),
        ]
    }

    #[test]
    fn name_returns_correct_name_for_all_variants() {
        let expected = ["IntT", "FloatT", "EnumT", "BoolT", "StrT", "BinT", "AggT", "ArrT"];
        for (pt, exp) in all_variants().iter().zip(expected) {
            assert_eq!(pt.name(), exp);
        }
    }

    #[test]
    fn set_name_mutates_name_for_all_variants() {
        for mut pt in all_variants() {
            pt.set_name("Renamed".into());
            assert_eq!(pt.name(), "Renamed");
        }
    }

    #[test]
    fn short_description_none_by_default_for_all_variants() {
        for pt in all_variants() {
            assert!(pt.short_description().is_none());
        }
    }

    #[test]
    fn set_short_description_round_trips_for_all_variants() {
        for mut pt in all_variants() {
            pt.set_short_description(Some("desc".into()));
            assert_eq!(pt.short_description(), Some("desc"));
            pt.set_short_description(None);
            assert!(pt.short_description().is_none());
        }
    }

    #[test]
    fn set_base_type_does_not_panic_for_all_variants() {
        for mut pt in all_variants() {
            pt.set_base_type(Some("Base".into()));
            pt.set_base_type(None);
        }
    }

    #[test]
    fn unit_set_mut_allows_push_for_all_variants() {
        for mut pt in all_variants() {
            let unit = Unit { value: "Hz".into(), power: None, factor: None, description: None };
            pt.unit_set_mut().push(unit);
            assert_eq!(pt.unit_set_mut().len(), 1);
        }
    }

    #[test]
    fn parameter_new_sets_defaults() {
        let p = Parameter::new("P1", "IntT");
        assert_eq!(p.name, "P1");
        assert_eq!(p.parameter_type_ref, "IntT");
        assert!(p.short_description.is_none());
        assert!(p.long_description.is_none());
        assert!(p.alias_set.is_empty());
        assert!(p.parameter_properties.is_none());
    }

    #[test]
    fn data_source_variants_are_distinct() {
        // Ensure all DataSource variants exist and are not equal to each other.
        let sources = vec![
            DataSource::Telemetered,
            DataSource::Derived,
            DataSource::Constant,
            DataSource::Local,
            DataSource::Ground,
        ];
        for i in 0..sources.len() {
            for j in 0..sources.len() {
                if i == j {
                    assert_eq!(sources[i], sources[j]);
                } else {
                    assert_ne!(sources[i], sources[j]);
                }
            }
        }
    }
}
