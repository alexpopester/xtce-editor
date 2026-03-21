//! XML → model parser.
//!
//! Parses an XTCE v1.2 XML document into a [`SpaceSystem`] tree using
//! `quick-xml`'s namespace-aware event reader.
//!
//! # Entry points
//! - [`parse`] — parse from a byte slice
//! - [`parse_file`] — parse from a file path

pub(crate) mod command;
pub(crate) mod container;
pub(crate) mod context;
pub(crate) mod space_system;
pub(crate) mod telemetry;
pub(crate) mod types;

use std::io::BufRead;

use quick_xml::NsReader;
use quick_xml::events::Event;

use crate::{ParseError, SpaceSystem, XtceError};

use context::ParseContext;

/// Parse an XTCE document from a byte slice.
pub fn parse(input: &[u8]) -> Result<SpaceSystem, ParseError> {
    let reader = NsReader::from_reader(input);
    let mut ctx = ParseContext::new(reader);
    find_root_and_parse(&mut ctx)
}

/// Parse an XTCE document from a file path.
///
/// Streams directly from disk via a `BufReader` rather than loading the
/// entire file into memory first.
pub fn parse_file(path: &std::path::Path) -> Result<SpaceSystem, XtceError> {
    let file = std::fs::File::open(path)?;
    let reader = NsReader::from_reader(std::io::BufReader::new(file));
    let mut ctx = ParseContext::new(reader);
    Ok(find_root_and_parse(&mut ctx)?)
}

/// Advance through the document until the root `<SpaceSystem>` element is
/// found, then delegate to the recursive-descent parser.
///
/// Any events before the root element (XML declaration, comments, DOCTYPE)
/// are already filtered by `ParseContext::next`. The first `Start` event
/// encountered must be `SpaceSystem`; anything else is an error.
fn find_root_and_parse<R: BufRead>(ctx: &mut ParseContext<R>) -> Result<SpaceSystem, ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"SpaceSystem" => return space_system::parse_space_system(ctx, &e),
                other => {
                    return Err(ParseError::UnexpectedElement(
                        String::from_utf8_lossy(other).into_owned(),
                    ))
                }
            },
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "<SpaceSystem>" })
            }
            // next() filters comments, PIs, DocType, and whitespace text.
            // Any other event type here is unexpected but harmless to skip.
            _ => {}
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_str(xml: &str) -> Result<SpaceSystem, ParseError> {
        parse(xml.as_bytes())
    }

    // ── Test 1: minimal SpaceSystem ──────────────────────────────────────────

    #[test]
    fn minimal_space_system() {
        // Bare element name (no namespace prefix).
        let ss = parse_str(r#"<SpaceSystem name="Test"/>"#).unwrap();
        assert_eq!(ss.name, "Test");
        assert!(ss.short_description.is_none());
        assert!(ss.long_description.is_none());
        assert!(ss.header.is_none());
        assert!(ss.telemetry.is_none());
        assert!(ss.command.is_none());
        assert!(ss.sub_systems.is_empty());

        // Same document but with xtce: namespace prefix — must parse identically.
        let ss = parse_str(
            r#"<xtce:SpaceSystem xmlns:xtce="http://www.omg.org/space/xtce" name="Test"/>"#,
        )
        .unwrap();
        assert_eq!(ss.name, "Test");
    }

    // ── Test 2: SpaceSystem with Header ──────────────────────────────────────

    #[test]
    fn space_system_with_header() {
        let ss = parse_str(r#"
            <SpaceSystem name="MySys" shortDescription="A test system">
                <LongDescription>Full description text</LongDescription>
                <Header version="1.2" date="2026-01-01" classification="Unclassified">
                    <AuthorSet>
                        <AuthorInformation name="Alice" role="Engineer"/>
                        <AuthorInformation name="Bob"/>
                    </AuthorSet>
                    <NoteSet>
                        <Note>First note</Note>
                        <Note>Second note</Note>
                    </NoteSet>
                </Header>
            </SpaceSystem>
        "#).unwrap();

        assert_eq!(ss.name, "MySys");
        assert_eq!(ss.short_description.as_deref(), Some("A test system"));
        assert_eq!(ss.long_description.as_deref(), Some("Full description text"));

        let header = ss.header.as_ref().unwrap();
        assert_eq!(header.version.as_deref(), Some("1.2"));
        assert_eq!(header.date.as_deref(), Some("2026-01-01"));
        assert_eq!(header.classification.as_deref(), Some("Unclassified"));

        assert_eq!(header.author_set.len(), 2);
        assert_eq!(header.author_set[0].name, "Alice");
        assert_eq!(header.author_set[0].role.as_deref(), Some("Engineer"));
        assert_eq!(header.author_set[1].name, "Bob");
        assert!(header.author_set[1].role.is_none());

        assert_eq!(header.note_set, vec!["First note", "Second note"]);
    }

    // ── Test 3: IntegerParameterType ─────────────────────────────────────────

    #[test]
    fn integer_parameter_type() {
        todo!("parse IntegerParameterType with signed, sizeInBits, IntegerDataEncoding")
    }

    // ── Test 4: FloatParameterType ───────────────────────────────────────────

    #[test]
    fn float_parameter_type() {
        todo!("parse FloatParameterType with FloatDataEncoding")
    }

    // ── Test 5: EnumeratedParameterType ──────────────────────────────────────

    #[test]
    fn enumerated_parameter_type() {
        todo!("parse EnumeratedParameterType with EnumerationList values")
    }

    // ── Test 6: BooleanParameterType ─────────────────────────────────────────

    #[test]
    fn boolean_parameter_type() {
        todo!("parse BooleanParameterType with oneStringValue and zeroStringValue attrs")
    }

    // ── Test 7: StringParameterType ──────────────────────────────────────────

    #[test]
    fn string_parameter_type() {
        todo!("parse StringParameterType with StringDataEncoding")
    }

    // ── Test 8: BinaryParameterType ──────────────────────────────────────────

    #[test]
    fn binary_parameter_type() {
        todo!("parse BinaryParameterType with BinaryDataEncoding")
    }

    // ── Test 9: AggregateParameterType ───────────────────────────────────────

    #[test]
    fn aggregate_parameter_type() {
        todo!("parse AggregateParameterType with MemberList containing named members")
    }

    // ── Test 10: ArrayParameterType ──────────────────────────────────────────

    #[test]
    fn array_parameter_type() {
        todo!("parse ArrayParameterType with arrayTypeRef and numberOfDimensions")
    }

    // ── Test 11: ParameterSet ────────────────────────────────────────────────

    #[test]
    fn parameter_set() {
        todo!("parse multiple <Parameter> elements, assert names and parameterTypeRef values")
    }

    // ── Test 12: SequenceContainer simple ────────────────────────────────────

    #[test]
    fn sequence_container_simple() {
        todo!("parse SequenceContainer with an EntryList of ParameterRefEntry elements")
    }

    // ── Test 13: SequenceContainer with BaseContainer ────────────────────────

    #[test]
    fn sequence_container_with_base() {
        todo!("parse SequenceContainer with BaseContainer + RestrictionCriteria (single Comparison)")
    }

    // ── Test 14: SequenceContainer with ComparisonList ───────────────────────

    #[test]
    fn sequence_container_comparison_list() {
        todo!("parse SequenceContainer with ComparisonList containing multiple Comparison elements")
    }

    // ── Test 15: MetaCommand simple ──────────────────────────────────────────

    #[test]
    fn meta_command_simple() {
        todo!("parse MetaCommand with ArgumentList and CommandContainer")
    }

    // ── Test 16: MetaCommand with base ───────────────────────────────────────

    #[test]
    fn meta_command_with_base() {
        todo!("parse MetaCommand with baseMetaCommand attribute set")
    }

    // ── Test 17: nested SpaceSystems ─────────────────────────────────────────

    #[test]
    fn nested_space_systems() {
        todo!("parse SpaceSystem that contains child SpaceSystems recursively")
    }

    // ── Test 18: unknown elements are skipped ─────────────────────────────────

    #[test]
    fn unknown_elements_skipped() {
        todo!("parse document with future/unknown elements, assert no error and known fields parsed")
    }

    // ── Test 19: missing required attribute error ─────────────────────────────

    #[test]
    #[should_panic] // replace with proper error assertion once implemented
    fn missing_required_attr_error() {
        todo!("parse '<Parameter parameterTypeRef=\"T\"/>' (no name), assert MissingAttribute error")
    }
}
