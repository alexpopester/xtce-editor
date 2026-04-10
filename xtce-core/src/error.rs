use thiserror::Error;

/// Structured location of a validation error within the SpaceSystem tree.
///
/// Carried by some [`ValidationError`] variants to identify the exact item
/// responsible for the error. The TUI uses this to implement "jump to error"
/// — it converts `ss_path` + `item_kind` + `item_name` into a [`NodeId`] and
/// calls `App::jump_to`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLocation {
    /// Path of SpaceSystem names from the root down to the SpaceSystem that
    /// owns the offending item. An empty vec means the root SpaceSystem.
    pub ss_path: Vec<String>,
    /// Category of the item the error points at.
    pub item_kind: ErrorItemKind,
    /// Name of the item within its SpaceSystem.
    pub item_name: String,
}

/// The category of XTCE item that an [`ErrorLocation`] identifies.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorItemKind {
    ParameterType,
    Parameter,
    Container,
    ArgumentType,
    MetaCommand,
}

/// Errors that can occur while parsing an XTCE XML document.
///
/// Returned by [`crate::parser::parse`] and [`crate::parser::parse_file`].
/// Most variants carry enough context to point at the offending element or
/// attribute in the source document.
#[derive(Debug, Error)]
pub enum ParseError {
    /// A `quick-xml` error (malformed XML, encoding issues, etc.).
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    /// A required XML attribute was absent on a known element.
    #[error("missing required attribute '{attr}' on <{element}>")]
    MissingAttribute { element: &'static str, attr: &'static str },

    /// An element was encountered at a position where it is not expected.
    #[error("unexpected element <{0}>")]
    UnexpectedElement(String),

    /// An attribute value could not be interpreted as the expected type.
    #[error("invalid value '{value}' for attribute '{attr}': {reason}")]
    InvalidValue { attr: &'static str, value: String, reason: &'static str },

    /// An attribute value contained invalid UTF-8 bytes.
    #[error("attribute value is not valid UTF-8: {0}")]
    AttrUtf8(#[from] std::str::Utf8Error),

    /// The document ended before the expected closing element was found.
    #[error("unexpected end of document while expecting {expected}")]
    UnexpectedEof { expected: &'static str },

    /// An IO error occurred while reading the input file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

/// Errors found during structural validation of a parsed [`crate::SpaceSystem`].
///
/// Returned by [`crate::validator::validate`] as a `Vec`; all errors are
/// collected in a single pass rather than stopping at the first problem.
/// Each variant optionally carries an [`ErrorLocation`] that the TUI uses
/// to jump to the offending item in the tree.
#[derive(Debug, Error)]
pub enum ValidationError {
    /// A name was used as a reference but no item with that name exists in the
    /// visible scope (current SpaceSystem and all ancestors).
    #[error("unresolved reference '{name}' in {context}")]
    UnresolvedReference {
        /// The unresolved name string.
        name: String,
        /// Human-readable description of where the reference appears.
        context: String,
        /// Tree location of the item containing the broken reference, if known.
        location: Option<ErrorLocation>,
    },

    /// Two items share the same name within a scope where names must be unique.
    #[error("duplicate name '{name}' in SpaceSystem '{space_system}'")]
    DuplicateName {
        /// The duplicated name.
        name: String,
        /// The SpaceSystem in which the collision was detected.
        space_system: String,
    },

    /// A base-container, base-type, or base-command chain forms a cycle.
    #[error("cyclic inheritance involving '{name}'")]
    CyclicInheritance {
        /// A name that appears more than once in the inheritance chain.
        name: String,
        /// Tree location of the item that closes the cycle, if known.
        location: Option<ErrorLocation>,
    },

    /// A required field was absent from a model item.
    #[error("missing required field '{field}' on {element} '{name}'")]
    MissingRequiredField {
        field: &'static str,
        element: &'static str,
        name: String,
    },

    /// A line emitted by `xmllint` during XSD schema validation.
    #[error("schema error: {0}")]
    SchemaError(String),
}

impl ValidationError {
    /// Number of terminal lines this error occupies when rendered in the TUI
    /// error overlay.
    ///
    /// Used by the virtual-scroll renderer to compute cumulative line offsets
    /// without rendering every error on every frame.
    pub fn render_line_count(&self) -> usize {
        match self {
            ValidationError::UnresolvedReference { .. } => 3,
            ValidationError::CyclicInheritance { .. } => 2,
            ValidationError::DuplicateName { .. } => 3,
            ValidationError::MissingRequiredField { .. } => 2,
            ValidationError::SchemaError(_) => 2,
        }
    }
}

/// Top-level error type that wraps both parse and IO errors.
///
/// Returned by [`crate::parser::parse_file`], which may fail either because
/// the file cannot be opened (IO) or because its contents are invalid XML /
/// XTCE (Parse).
#[derive(Debug, Error)]
pub enum XtceError {
    /// Wraps any [`ParseError`] produced while parsing the document.
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    /// Wraps any [`std::io::Error`] produced while opening or reading the file.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
