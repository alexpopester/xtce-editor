use thiserror::Error;

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
    UnresolvedReference { name: String, context: String },

    #[error("duplicate name '{name}' in SpaceSystem '{space_system}'")]
    DuplicateName { name: String, space_system: String },

    #[error("cyclic inheritance involving '{name}'")]
    CyclicInheritance { name: String },

    #[error("missing required field '{field}' on {element} '{name}'")]
    MissingRequiredField { field: &'static str, element: &'static str, name: String },
}

#[derive(Debug, Error)]
pub enum XtceError {
    #[error("parse error: {0}")]
    Parse(#[from] ParseError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
