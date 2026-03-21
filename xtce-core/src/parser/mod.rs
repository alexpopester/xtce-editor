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
        use crate::model::telemetry::ParameterType;
        use crate::model::types::IntegerEncoding;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="MyInt" shortDescription="An int"
                      signed="false" sizeInBits="16">
                    <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyInt").unwrap();
        let ParameterType::Integer(t) = pt else { panic!("expected Integer variant") };

        assert_eq!(t.name, "MyInt");
        assert_eq!(t.short_description.as_deref(), Some("An int"));
        assert!(!t.signed);
        assert_eq!(t.size_in_bits, Some(16));

        let enc = t.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, 16);
        assert_eq!(enc.encoding, IntegerEncoding::Unsigned);
    }

    // ── Test 4: FloatParameterType ───────────────────────────────────────────

    #[test]
    fn float_parameter_type() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::{FloatEncoding, FloatSizeInBits};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <FloatParameterType name="MyFloat" shortDescription="A float">
                    <FloatDataEncoding sizeInBits="32" encoding="IEEE754_1985"/>
                  </FloatParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyFloat").unwrap();
        let ParameterType::Float(t) = pt else { panic!("expected Float variant") };

        assert_eq!(t.name, "MyFloat");
        assert_eq!(t.short_description.as_deref(), Some("A float"));

        let enc = t.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, FloatSizeInBits::F32);
        assert_eq!(enc.encoding, FloatEncoding::IEEE754_1985);
    }

    // ── Test 5: EnumeratedParameterType ──────────────────────────────────────

    #[test]
    fn enumerated_parameter_type() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <EnumeratedParameterType name="MyEnum">
                    <IntegerDataEncoding sizeInBits="8" encoding="unsigned"/>
                    <EnumerationList>
                      <Enumeration value="0" label="OFF"/>
                      <Enumeration value="1" label="ON" shortDescription="Active"/>
                    </EnumerationList>
                  </EnumeratedParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyEnum").unwrap();
        let ParameterType::Enumerated(t) = pt else { panic!("expected Enumerated variant") };

        assert_eq!(t.name, "MyEnum");
        assert!(t.encoding.is_some());

        assert_eq!(t.enumeration_list.len(), 2);
        assert_eq!(t.enumeration_list[0].value, 0);
        assert_eq!(t.enumeration_list[0].label, "OFF");
        assert!(t.enumeration_list[0].short_description.is_none());
        assert_eq!(t.enumeration_list[1].value, 1);
        assert_eq!(t.enumeration_list[1].label, "ON");
        assert_eq!(t.enumeration_list[1].short_description.as_deref(), Some("Active"));
    }

    // ── Test 6: BooleanParameterType ─────────────────────────────────────────

    #[test]
    fn boolean_parameter_type() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <BooleanParameterType name="MyBool"
                      oneStringValue="YES" zeroStringValue="NO"/>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyBool").unwrap();
        let ParameterType::Boolean(t) = pt else { panic!("expected Boolean variant") };

        assert_eq!(t.name, "MyBool");
        assert_eq!(t.one_string_value.as_deref(), Some("YES"));
        assert_eq!(t.zero_string_value.as_deref(), Some("NO"));
    }

    // ── Test 7: StringParameterType ──────────────────────────────────────────

    #[test]
    fn string_parameter_type() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::{StringEncoding, StringSize};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <StringParameterType name="MyString">
                    <StringDataEncoding encoding="UTF-8">
                      <SizeInBits>
                        <Fixed><FixedValue>64</FixedValue></Fixed>
                      </SizeInBits>
                    </StringDataEncoding>
                  </StringParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyString").unwrap();
        let ParameterType::String(t) = pt else { panic!("expected String variant") };

        assert_eq!(t.name, "MyString");
        let enc = t.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::UTF8);
        assert_eq!(enc.size_in_bits, Some(StringSize::Fixed(64)));
    }

    // ── Test 8: BinaryParameterType ──────────────────────────────────────────

    #[test]
    fn binary_parameter_type() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::BinarySize;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <BinaryParameterType name="MyBinary">
                    <BinaryDataEncoding>
                      <SizeInBits>
                        <FixedValue>32</FixedValue>
                      </SizeInBits>
                    </BinaryDataEncoding>
                  </BinaryParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyBinary").unwrap();
        let ParameterType::Binary(t) = pt else { panic!("expected Binary variant") };

        assert_eq!(t.name, "MyBinary");
        let enc = t.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, BinarySize::Fixed(32));
    }

    // ── Test 9: AggregateParameterType ───────────────────────────────────────

    #[test]
    fn aggregate_parameter_type() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <AggregateParameterType name="MyAggregate" shortDescription="A struct">
                    <MemberList>
                      <Member name="field1" typeRef="MyInt"/>
                      <Member name="field2" typeRef="MyFloat" shortDescription="float field"/>
                    </MemberList>
                  </AggregateParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyAggregate").unwrap();
        let ParameterType::Aggregate(t) = pt else { panic!("expected Aggregate variant") };

        assert_eq!(t.name, "MyAggregate");
        assert_eq!(t.short_description.as_deref(), Some("A struct"));
        assert_eq!(t.member_list.len(), 2);
        assert_eq!(t.member_list[0].name, "field1");
        assert_eq!(t.member_list[0].type_ref, "MyInt");
        assert!(t.member_list[0].short_description.is_none());
        assert_eq!(t.member_list[1].name, "field2");
        assert_eq!(t.member_list[1].type_ref, "MyFloat");
        assert_eq!(t.member_list[1].short_description.as_deref(), Some("float field"));
    }

    // ── Test 10: ArrayParameterType ──────────────────────────────────────────

    #[test]
    fn array_parameter_type() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <ArrayParameterType name="MyArray" arrayTypeRef="MyInt" numberOfDimensions="2"/>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let pt = tm.parameter_types.get("MyArray").unwrap();
        let ParameterType::Array(t) = pt else { panic!("expected Array variant") };

        assert_eq!(t.name, "MyArray");
        assert_eq!(t.array_type_ref, "MyInt");
        assert_eq!(t.number_of_dimensions, 2);
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
