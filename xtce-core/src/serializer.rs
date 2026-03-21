//! Model → XML serializer.
//!
//! Serializes a [`SpaceSystem`] tree to a valid XTCE v1.2 XML document using
//! `quick-xml`'s writer.

use crate::{ParseError, SpaceSystem};

/// Serialize a [`SpaceSystem`] to XTCE XML bytes.
pub fn serialize(_space_system: &SpaceSystem) -> Result<Vec<u8>, ParseError> {
    todo!("XML serializer not yet implemented")
}
