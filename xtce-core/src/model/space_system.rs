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
    /// Create a new, empty `SpaceSystem` with the given name.
    ///
    /// All optional fields are `None` and all collections are empty.
    /// Use the public fields to populate the system after construction.
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

/// Document-level metadata for a SpaceSystem.
///
/// Maps to the XTCE `<Header>` element. All fields are optional; an absent
/// `<Header>` element is represented as `SpaceSystem::header == None`.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Header {
    /// Free-form version string (e.g. `"1.0"` or `"draft-3"`).
    pub version: Option<String>,
    /// Document creation or revision date (free-form string).
    pub date: Option<String>,
    /// Security or export classification label.
    pub classification: Option<String>,
    /// Additional classification handling instructions.
    pub classification_instructions: Option<String>,
    /// Validation / review status (e.g. `"Working"`, `"Released"`).
    pub validation_status: Option<String>,
    /// Zero or more author entries from the `<AuthorSet>` child element.
    pub author_set: Vec<AuthorInfo>,
    /// Zero or more free-text notes from the `<NoteSet>` child element.
    pub note_set: Vec<String>,
}

/// A single author entry inside a `<Header><AuthorSet>`.
#[derive(Debug, Clone, PartialEq)]
pub struct AuthorInfo {
    /// Author name (required).
    pub name: String,
    /// Optional role (e.g. `"Lead Engineer"`, `"Reviewer"`).
    pub role: Option<String>,
}
