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
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterSet>
                  <Parameter name="P1" parameterTypeRef="IntT" shortDescription="first"/>
                  <Parameter name="P2" parameterTypeRef="FloatT"/>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        assert_eq!(tm.parameters.len(), 2);

        let p1 = tm.parameters.get("P1").unwrap();
        assert_eq!(p1.name, "P1");
        assert_eq!(p1.parameter_type_ref, "IntT");
        assert_eq!(p1.short_description.as_deref(), Some("first"));

        let p2 = tm.parameters.get("P2").unwrap();
        assert_eq!(p2.name, "P2");
        assert_eq!(p2.parameter_type_ref, "FloatT");
        assert!(p2.short_description.is_none());
    }

    // ── Test 12: SequenceContainer simple ────────────────────────────────────

    #[test]
    fn sequence_container_simple() {
        use crate::model::container::SequenceEntry;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="PrimaryHeader" shortDescription="CCSDS header">
                    <EntryList>
                      <ParameterRefEntry parameterRef="APID"/>
                      <ParameterRefEntry parameterRef="SeqCount"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("PrimaryHeader").unwrap();
        assert_eq!(c.name, "PrimaryHeader");
        assert_eq!(c.short_description.as_deref(), Some("CCSDS header"));
        assert!(c.base_container.is_none());
        assert!(!c.r#abstract);
        assert_eq!(c.entry_list.len(), 2);

        let SequenceEntry::ParameterRef(e0) = &c.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        assert_eq!(e0.parameter_ref, "APID");

        let SequenceEntry::ParameterRef(e1) = &c.entry_list[1] else {
            panic!("expected ParameterRef")
        };
        assert_eq!(e1.parameter_ref, "SeqCount");
    }

    // ── Test 13: SequenceContainer with BaseContainer ────────────────────────

    #[test]
    fn sequence_container_with_base() {
        use crate::model::container::{ComparisonOperator, RestrictionCriteria};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="TmPacket">
                    <BaseContainer containerRef="PrimaryHeader">
                      <RestrictionCriteria>
                        <Comparison parameterRef="APID" value="100"
                            comparisonOperator="==" useCalibratedValue="false"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList>
                      <ParameterRefEntry parameterRef="Payload"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("TmPacket").unwrap();

        let base = c.base_container.as_ref().unwrap();
        assert_eq!(base.container_ref, "PrimaryHeader");

        let RestrictionCriteria::Comparison(cmp) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected Comparison")
        };
        assert_eq!(cmp.parameter_ref, "APID");
        assert_eq!(cmp.value, "100");
        assert_eq!(cmp.comparison_operator, ComparisonOperator::Equality);
        assert!(!cmp.use_calibrated_value);

        assert_eq!(c.entry_list.len(), 1);
    }

    // ── Test 14: SequenceContainer with ComparisonList ───────────────────────

    #[test]
    fn sequence_container_comparison_list() {
        use crate::model::container::RestrictionCriteria;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="TmPacket">
                    <BaseContainer containerRef="PrimaryHeader">
                      <RestrictionCriteria>
                        <ComparisonList>
                          <Comparison parameterRef="APID" value="100"/>
                          <Comparison parameterRef="Version" value="1"/>
                        </ComparisonList>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("TmPacket").unwrap();

        let base = c.base_container.as_ref().unwrap();
        let RestrictionCriteria::ComparisonList(list) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected ComparisonList")
        };
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].parameter_ref, "APID");
        assert_eq!(list[0].value, "100");
        assert_eq!(list[1].parameter_ref, "Version");
        assert_eq!(list[1].value, "1");
    }

    // ── Test 15: MetaCommand simple ──────────────────────────────────────────

    #[test]
    fn meta_command_simple() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="SendTc" shortDescription="A command">
                    <ArgumentList>
                      <Argument name="arg1" argumentTypeRef="IntT"/>
                      <Argument name="arg2" argumentTypeRef="FloatT" initialValue="3.14"/>
                    </ArgumentList>
                    <CommandContainer name="SendTcContainer">
                      <EntryList>
                        <ArgumentRefEntry argumentRef="arg1"/>
                        <ArgumentRefEntry argumentRef="arg2"/>
                      </EntryList>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let mc = cmd.meta_commands.get("SendTc").unwrap();
        assert_eq!(mc.name, "SendTc");
        assert_eq!(mc.short_description.as_deref(), Some("A command"));
        assert!(!mc.r#abstract);
        assert!(mc.base_meta_command.is_none());

        assert_eq!(mc.argument_list.len(), 2);
        assert_eq!(mc.argument_list[0].name, "arg1");
        assert_eq!(mc.argument_list[0].argument_type_ref, "IntT");
        assert!(mc.argument_list[0].initial_value.is_none());
        assert_eq!(mc.argument_list[1].name, "arg2");
        assert_eq!(mc.argument_list[1].initial_value.as_deref(), Some("3.14"));

        let cc = mc.command_container.as_ref().unwrap();
        assert_eq!(cc.name, "SendTcContainer");
        assert_eq!(cc.entry_list.len(), 2);
    }

    // ── Test 16: MetaCommand with base ───────────────────────────────────────

    #[test]
    fn meta_command_with_base() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="BaseCmd" abstract="true">
                    <ArgumentList/>
                    <CommandContainer name="BaseCmdContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                  <MetaCommand name="ChildCmd" baseMetaCommand="BaseCmd">
                    <CommandContainer name="ChildCmdContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();

        let base_mc = cmd.meta_commands.get("BaseCmd").unwrap();
        assert!(base_mc.r#abstract);
        assert!(base_mc.base_meta_command.is_none());

        let child_mc = cmd.meta_commands.get("ChildCmd").unwrap();
        assert!(!child_mc.r#abstract);
        assert_eq!(child_mc.base_meta_command.as_deref(), Some("BaseCmd"));
    }

    // ── Test 17: nested SpaceSystems ─────────────────────────────────────────

    #[test]
    fn nested_space_systems() {
        let ss = parse_str(r#"
            <SpaceSystem name="Root">
              <SpaceSystem name="Child1">
                <SpaceSystem name="Grandchild"/>
              </SpaceSystem>
              <SpaceSystem name="Child2"/>
            </SpaceSystem>
        "#).unwrap();

        assert_eq!(ss.name, "Root");
        assert_eq!(ss.sub_systems.len(), 2);
        assert_eq!(ss.sub_systems[0].name, "Child1");
        assert_eq!(ss.sub_systems[0].sub_systems.len(), 1);
        assert_eq!(ss.sub_systems[0].sub_systems[0].name, "Grandchild");
        assert_eq!(ss.sub_systems[1].name, "Child2");
        assert!(ss.sub_systems[1].sub_systems.is_empty());
    }

    // ── Test 18: unknown elements are skipped ─────────────────────────────────

    #[test]
    fn unknown_elements_skipped() {
        // Future XTCE extension elements and deeply-nested unknown content
        // must be silently skipped without causing a parse error.
        let ss = parse_str(r#"
            <SpaceSystem name="Test" shortDescription="known attr">
              <UnknownFutureElement foo="bar">
                <DeepChild>text</DeepChild>
              </UnknownFutureElement>
              <LongDescription>Known text</LongDescription>
              <AnotherUnknown/>
            </SpaceSystem>
        "#).unwrap();

        assert_eq!(ss.name, "Test");
        assert_eq!(ss.short_description.as_deref(), Some("known attr"));
        assert_eq!(ss.long_description.as_deref(), Some("Known text"));
    }

    // ── Test 19: missing required attribute error ─────────────────────────────

    #[test]
    fn missing_required_attr_error() {
        // <Parameter> without a name attribute must return MissingAttribute.
        use crate::ParseError;
        let result = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterSet>
                  <Parameter parameterTypeRef="T"/>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            matches!(result, Err(ParseError::MissingAttribute { attr: "name", .. })),
            "expected MissingAttribute(name), got {:?}",
            result
        );
    }
}
