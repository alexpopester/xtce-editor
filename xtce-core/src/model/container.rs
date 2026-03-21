//! SequenceContainer and related entry/restriction types.

use super::types::Alias;

/// A SequenceContainer describes the structure of a data packet or frame.
///
/// Containers support single inheritance: a child container may declare a
/// `base_container` with optional `restriction_criteria` that constrain when
/// the child container applies (e.g., based on a header field value).
#[derive(Debug, Clone, PartialEq)]
pub struct SequenceContainer {
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub alias_set: Vec<Alias>,
    /// If set, this container extends another container. Entries from the base
    /// container are prepended to this container's entry list.
    pub base_container: Option<BaseContainer>,
    /// An `abstract` container cannot be instantiated directly; it serves only
    /// as a base for other containers.
    pub r#abstract: bool,
    pub entry_list: Vec<SequenceEntry>,
}

impl SequenceContainer {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            alias_set: Vec::new(),
            base_container: None,
            r#abstract: false,
            entry_list: Vec::new(),
        }
    }
}

/// Reference to a parent container, with optional restriction criteria.
#[derive(Debug, Clone, PartialEq)]
pub struct BaseContainer {
    /// Unqualified or fully-qualified name of the referenced container.
    pub container_ref: String,
    /// If provided, this container is only selected when the criteria evaluate
    /// to true (e.g., a specific APID value in the parent container).
    pub restriction_criteria: Option<RestrictionCriteria>,
}

/// Conditions that determine whether a child container applies.
#[derive(Debug, Clone, PartialEq)]
pub enum RestrictionCriteria {
    Comparison(Comparison),
    ComparisonList(Vec<Comparison>),
    BooleanExpression(BooleanExpression),
    /// Reference to an external condition defined elsewhere.
    NextContainer { container_ref: String },
}

/// A single comparison of a parameter value against a constant.
#[derive(Debug, Clone, PartialEq)]
pub struct Comparison {
    /// Reference to the parameter whose value is compared.
    pub parameter_ref: String,
    pub value: String,
    pub comparison_operator: ComparisonOperator,
    /// If true, compare using calibrated (engineering) value; otherwise raw.
    pub use_calibrated_value: bool,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ComparisonOperator {
    #[default]
    Equality,
    Inequality,
    LessThan,
    LessThanOrEqual,
    GreaterThan,
    GreaterThanOrEqual,
}

/// A boolean expression combining comparisons with AND/OR logic.
#[derive(Debug, Clone, PartialEq)]
pub enum BooleanExpression {
    And(Vec<BooleanExpression>),
    Or(Vec<BooleanExpression>),
    Not(Box<BooleanExpression>),
    Condition(Comparison),
}

/// A single entry in a container's entry list.
#[derive(Debug, Clone, PartialEq)]
pub enum SequenceEntry {
    /// A reference to a parameter to be extracted from this container.
    ParameterRef(ParameterRefEntry),
    /// A reference to a nested container embedded within this container.
    ContainerRef(ContainerRefEntry),
    /// A span of bits with no semantic meaning (padding/alignment).
    FixedValue(FixedValueEntry),
    /// An array parameter reference (size may be dynamic).
    ArrayParameterRef(ArrayParameterRefEntry),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ParameterRefEntry {
    pub parameter_ref: String,
    pub location: Option<EntryLocation>,
    /// If true, include only when this condition holds.
    pub include_condition: Option<MatchCriteria>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ContainerRefEntry {
    pub container_ref: String,
    pub location: Option<EntryLocation>,
    pub include_condition: Option<MatchCriteria>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FixedValueEntry {
    /// Size of the fixed value in bits.
    pub size_in_bits: u32,
    /// Hex string representing the bit pattern.
    pub binary_value: Option<String>,
    pub location: Option<EntryLocation>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ArrayParameterRefEntry {
    pub parameter_ref: String,
    pub location: Option<EntryLocation>,
}

/// Specifies where in the container an entry is located.
#[derive(Debug, Clone, PartialEq)]
pub struct EntryLocation {
    pub reference_location: ReferenceLocation,
    /// Offset from the reference location in bits.
    pub bit_offset: i64,
}

#[derive(Debug, Clone, PartialEq, Default)]
pub enum ReferenceLocation {
    /// Offset from the start of the container.
    ContainerStart,
    /// Offset from the previous entry.
    #[default]
    PreviousEntry,
}

/// A condition used to decide whether to include an entry.
#[derive(Debug, Clone, PartialEq)]
pub enum MatchCriteria {
    Comparison(Comparison),
    ComparisonList(Vec<Comparison>),
    BooleanExpression(BooleanExpression),
}
