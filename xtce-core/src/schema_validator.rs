//! XSD schema validation for XTCE v1.2 documents.
//!
//! Validates serialized XML bytes against the official XTCE v1.2 XSD using
//! `xmllint`. Both XSD files are bundled at compile time and written to a
//! temporary directory for each validation run.

use crate::error::ValidationError;
use std::process::Command;

const XTCE_XSD: &str = include_str!("xtce1_2.xsd");
const XML_XSD: &str = include_str!("xml.xsd");

/// Validate `xml_bytes` against the XTCE v1.2 XSD schema.
///
/// Returns an empty `Vec` if the document is schema-valid. Returns
/// `Err(...)` only if the validator itself could not be invoked (e.g.
/// `xmllint` is not installed); schema errors are returned as `Ok(errors)`.
pub fn validate_schema(xml_bytes: &[u8]) -> Result<Vec<ValidationError>, String> {
    let dir = tempfile::TempDir::new().map_err(|e| format!("tempdir: {e}"))?;

    let xsd_path = dir.path().join("xtce1_2.xsd");
    let xml_xsd_path = dir.path().join("xml.xsd");
    let xml_path = dir.path().join("document.xml");

    std::fs::write(&xsd_path, XTCE_XSD).map_err(|e| format!("write xsd: {e}"))?;
    std::fs::write(&xml_xsd_path, XML_XSD).map_err(|e| format!("write xml.xsd: {e}"))?;
    std::fs::write(&xml_path, xml_bytes).map_err(|e| format!("write xml: {e}"))?;

    let output = Command::new("xmllint")
        .args(["--noout", "--schema"])
        .arg(&xsd_path)
        .arg(&xml_path)
        .output()
        .map_err(|e| format!("xmllint not found: {e}"))?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    // xmllint writes validation errors to stderr.
    let stderr = String::from_utf8_lossy(&output.stderr);
    let errors = parse_xmllint_errors(&stderr);
    Ok(errors)
}

/// Parse xmllint stderr output into `ValidationError::SchemaError` entries.
///
/// xmllint lines look like:
///   `/path/to/doc.xml:42: Schemas validity error : Element '...': ...`
///   `/path/to/doc.xml fails to validate`
fn parse_xmllint_errors(stderr: &str) -> Vec<ValidationError> {
    let mut errors = Vec::new();
    for line in stderr.lines() {
        // Skip the summary line ("... fails to validate").
        if line.ends_with("fails to validate") || line.ends_with("validates") {
            continue;
        }
        // Strip the file path prefix (everything up to and including the first ': ').
        // Format: "<path>:<line>: Schemas validity error : <message>"
        let msg = if let Some(rest) = line.split_once(": Schemas validity error : ") {
            rest.1.to_string()
        } else if let Some(rest) = line.split_once(": ") {
            // Fallback: strip just the path:line prefix.
            rest.1.to_string()
        } else {
            line.to_string()
        };
        if !msg.is_empty() {
            errors.push(ValidationError::SchemaError(msg));
        }
    }
    errors
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::space_system::SpaceSystem;

    fn simple_valid_xml() -> Vec<u8> {
        crate::serializer::serialize(&SpaceSystem::new("Test")).unwrap()
    }

    #[test]
    fn valid_minimal_document_passes() {
        let bytes = simple_valid_xml();
        let errors = validate_schema(&bytes).expect("xmllint should be available");
        assert!(errors.is_empty(), "unexpected schema errors: {errors:?}");
    }

    #[test]
    fn invalid_xml_produces_errors() {
        let bad = b"<?xml version=\"1.0\"?><SpaceSystem xmlns=\"http://www.omg.org/spec/XTCE/20180204\" name=\"T\"><INVALID/></SpaceSystem>";
        let errors = validate_schema(bad).expect("xmllint should be available");
        assert!(!errors.is_empty(), "expected schema errors for invalid XML");
    }

    #[test]
    fn sample_xml_round_trip_validates() {
        let src = std::fs::read_to_string("../test_data/sample.xml").expect("sample.xml not found");
        let ss = crate::parser::parse(src.as_bytes()).expect("parse failed");
        let bytes = crate::serializer::serialize(&ss).expect("serialize failed");
        let errors = validate_schema(&bytes).expect("xmllint should be available");
        assert!(errors.is_empty(), "sample.xml schema errors after round-trip: {errors:#?}");
    }

    #[test]
    fn parse_error_line_strips_path_prefix() {
        let line = "/tmp/x.xml:5: Schemas validity error : Element 'foo': This element is not expected.";
        let errors = parse_xmllint_errors(line);
        assert_eq!(errors.len(), 1);
        if let ValidationError::SchemaError(msg) = &errors[0] {
            assert_eq!(msg, "Element 'foo': This element is not expected.");
        } else {
            panic!("wrong variant");
        }
    }
}
