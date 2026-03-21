//! Reference resolution and consistency validation.
//!
//! After parsing, call [`validate`] to check that all cross-references resolve,
//! names are unique within their scope, and inheritance chains are acyclic.

use crate::{SpaceSystem, ValidationError};

/// Validate a parsed [`SpaceSystem`] tree.
///
/// Returns a list of all validation errors found. An empty list means the
/// document is structurally valid. Errors do not stop collection — all problems
/// are reported together.
pub fn validate(_space_system: &SpaceSystem) -> Vec<ValidationError> {
    todo!("validator not yet implemented")
}
