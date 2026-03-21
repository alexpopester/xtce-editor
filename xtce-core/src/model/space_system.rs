//! Top-level SpaceSystem and Header definitions.

use super::{command::CommandMetaData, telemetry::TelemetryMetaData};

/// The root element of an XTCE document.
///
/// A SpaceSystem may contain sub-SpaceSystems, forming a tree. Names of major
/// elements (parameters, types, containers, commands) must be unique within
/// a given SpaceSystem scope.
#[derive(Debug, Clone, PartialEq)]
pub struct SpaceSystem {
    /// Required. The unique name within its parent SpaceSystem.
    pub name: String,
    pub short_description: Option<String>,
    pub long_description: Option<String>,
    pub header: Option<Header>,
    pub telemetry: Option<TelemetryMetaData>,
    pub command: Option<CommandMetaData>,
    /// Child SpaceSystems (recursive).
    pub sub_systems: Vec<SpaceSystem>,
}

impl SpaceSystem {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            short_description: None,
            long_description: None,
            header: None,
            telemetry: None,
            command: None,
            sub_systems: Vec::new(),
        }
    }
}

/// Document-level metadata.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    pub version: Option<String>,
    pub date: Option<String>,
    pub classification: Option<String>,
    pub classification_instructions: Option<String>,
    pub validation_status: Option<String>,
    pub author_set: Vec<AuthorInfo>,
    pub note_set: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AuthorInfo {
    pub name: String,
    pub role: Option<String>,
}
