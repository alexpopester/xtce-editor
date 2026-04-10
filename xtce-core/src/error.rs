use thiserror::Error;

/// Structured location of a validation error within the SpaceSystem tree.
///
/// The TUI converts this into a [`NodeId`](xtce-tui) to allow jumping directly
/// to the offending item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErrorLocation {
    /// Path from the root SpaceSystem down to the SpaceSystem that owns the item.
    /// Empty means the root SpaceSystem.
    pub ss_path: Vec<String>,
    /// What kind of item the error points at.
    pub item_kind: ErrorItemKind,
    /// Name of the item within its SpaceSystem.
    pub item_name: String,
}

/// The kind of item an [`ErrorLocation`] points at.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErrorItemKind {
    ParameterType,
    Parameter,
    Container,
    ArgumentType,
    MetaCommand,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("XML error: {0}")]
    Xml(#[from] quick_xml::Error),

    #[error("missing required attribute '{attr}' on <{element}>")]
    MissingAttribute { element: &'static str, attr: &'static str },

    #[error("unexpected element <{0}>")]
    UnexpectedElement(String),

    #[error("invalid value '{value}' for attribute '{attr}': {reason}")]
    InvalidValue { attr: &'static str, value: String, reason: &'static str },

    #[error("attribute value is not valid UTF-8: {0}")]
    AttrUtf8(#[from] std::str::Utf8Error),

    #[error("unexpected end of document while expecting {expected}")]
    UnexpectedEof { expected: &'static str },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("unresolved reference '{name}' in {context}")]
    UnresolvedReference {
        name: String,
        context: String,
        /// Where in the tree the broken reference lives (for TUI jump-to-error).
        location: Option<ErrorLocation>,
    },

    #[error("duplicate name '{name}' in SpaceSystem '{space_system}'")]
    DuplicateName { name: String, space_system: String },

    #[error("cyclic inheritance involving '{name}'")]
    CyclicInheritance {
        name: String,
        location: Option<ErrorLocation>,
    },

    #[error("missing required field '{field}' on {element} '{name}'")]
    MissingRequiredField { field: &'static str, element: &'static str, name: String },

    #[error("schema error: {0}")]
    SchemaError(String),
}

impl ValidationError {
    /// Number of terminal lines this error occupies in the TUI error overlay.
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

#[derive(Debug, Error)]
pub enum XtceError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
