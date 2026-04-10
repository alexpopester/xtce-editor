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
///
/// # Errors
///
/// Returns a [`ParseError`] if the input is not well-formed XML, if a required
/// attribute is absent, or if an unexpected element is encountered.
///
/// # Examples
///
/// ```
/// let xml = br#"<SpaceSystem name="Root"/>"#;
/// let ss = xtce_core::parser::parse(xml).unwrap();
/// assert_eq!(ss.name, "Root");
/// ```
pub fn parse(input: &[u8]) -> Result<SpaceSystem, ParseError> {
    let reader = NsReader::from_reader(input);
    let mut ctx = ParseContext::new(reader);
    find_root_and_parse(&mut ctx)
}

/// Parse an XTCE document from a file path.
///
/// Streams directly from disk via a `BufReader` rather than loading the
/// entire file into memory first.
///
/// # Errors
///
/// Returns [`XtceError::Io`] if the file cannot be opened, or
/// [`XtceError::Parse`] for any XML or XTCE structural error.
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

    // ── Test 20: argument type set — all 8 variants ───────────────────────────

    #[test]
    fn argument_type_set_all_variants() {
        use crate::model::command::ArgumentType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="IntArg" sizeInBits="16" signed="false"
                      shortDescription="unsigned 16-bit" initialValue="42"/>
                  <FloatArgumentType name="FloatArg" sizeInBits="32"
                      shortDescription="32-bit float" initialValue="1.5"/>
                  <EnumeratedArgumentType name="EnumArg" shortDescription="enum"
                      initialValue="ON">
                    <EnumerationList>
                      <Enumeration value="0" label="OFF"/>
                      <Enumeration value="1" label="ON" shortDescription="power on"/>
                    </EnumerationList>
                  </EnumeratedArgumentType>
                  <BooleanArgumentType name="BoolArg" oneStringValue="YES"
                      zeroStringValue="NO" shortDescription="bool"/>
                  <StringArgumentType name="StrArg" shortDescription="string"
                      initialValue="hello"/>
                  <BinaryArgumentType name="BinArg" shortDescription="binary"
                      initialValue="DEADBEEF"/>
                  <AggregateArgumentType name="AggArg" shortDescription="aggregate">
                    <MemberList>
                      <Member name="x" typeRef="IntArg" shortDescription="x axis"/>
                      <Member name="y" typeRef="IntArg"/>
                    </MemberList>
                  </AggregateArgumentType>
                  <ArrayArgumentType name="ArrArg" arrayTypeRef="IntArg"
                      numberOfDimensions="2" shortDescription="2D array"/>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        assert_eq!(cmd.argument_types.len(), 8);

        let ArgumentType::Integer(int_t) = cmd.argument_types.get("IntArg").unwrap() else {
            panic!("expected Integer")
        };
        assert_eq!(int_t.name, "IntArg");
        assert_eq!(int_t.size_in_bits, Some(16));
        assert!(!int_t.signed);
        assert_eq!(int_t.short_description.as_deref(), Some("unsigned 16-bit"));
        assert_eq!(int_t.initial_value, Some(42));

        let ArgumentType::Float(flt_t) = cmd.argument_types.get("FloatArg").unwrap() else {
            panic!("expected Float")
        };
        assert_eq!(flt_t.size_in_bits, Some(32));
        assert!((flt_t.initial_value.unwrap() - 1.5).abs() < f64::EPSILON);

        let ArgumentType::Enumerated(enum_t) = cmd.argument_types.get("EnumArg").unwrap() else {
            panic!("expected Enumerated")
        };
        assert_eq!(enum_t.enumeration_list.len(), 2);
        assert_eq!(enum_t.enumeration_list[0].value, 0);
        assert_eq!(enum_t.enumeration_list[0].label, "OFF");
        assert_eq!(enum_t.enumeration_list[1].label, "ON");
        assert_eq!(enum_t.enumeration_list[1].short_description.as_deref(), Some("power on"));
        assert_eq!(enum_t.initial_value.as_deref(), Some("ON"));

        let ArgumentType::Boolean(bool_t) = cmd.argument_types.get("BoolArg").unwrap() else {
            panic!("expected Boolean")
        };
        assert_eq!(bool_t.one_string_value.as_deref(), Some("YES"));
        assert_eq!(bool_t.zero_string_value.as_deref(), Some("NO"));

        let ArgumentType::String(str_t) = cmd.argument_types.get("StrArg").unwrap() else {
            panic!("expected String")
        };
        assert_eq!(str_t.initial_value.as_deref(), Some("hello"));

        let ArgumentType::Binary(bin_t) = cmd.argument_types.get("BinArg").unwrap() else {
            panic!("expected Binary")
        };
        assert_eq!(bin_t.initial_value.as_deref(), Some("DEADBEEF"));

        let ArgumentType::Aggregate(agg_t) = cmd.argument_types.get("AggArg").unwrap() else {
            panic!("expected Aggregate")
        };
        assert_eq!(agg_t.member_list.len(), 2);
        assert_eq!(agg_t.member_list[0].name, "x");
        assert_eq!(agg_t.member_list[0].type_ref, "IntArg");
        assert_eq!(agg_t.member_list[0].short_description.as_deref(), Some("x axis"));
        assert_eq!(agg_t.member_list[1].short_description, None);

        let ArgumentType::Array(arr_t) = cmd.argument_types.get("ArrArg").unwrap() else {
            panic!("expected Array")
        };
        assert_eq!(arr_t.array_type_ref, "IntArg");
        assert_eq!(arr_t.number_of_dimensions, 2);
    }

    // ── Test 21: argument type encodings ─────────────────────────────────────

    #[test]
    fn argument_type_encodings() {
        use crate::model::command::ArgumentType;
        use crate::model::types::{FloatSizeInBits, IntegerEncoding};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="IntArg">
                    <UnitSet>
                      <Unit power="2.0" factor="1e3" description="square km">km</Unit>
                    </UnitSet>
                    <IntegerDataEncoding sizeInBits="16" encoding="unsigned"/>
                  </IntegerArgumentType>
                  <FloatArgumentType name="FloatArg">
                    <FloatDataEncoding sizeInBits="32" encoding="IEEE754_1985"/>
                  </FloatArgumentType>
                  <BooleanArgumentType name="BoolArg">
                    <IntegerDataEncoding sizeInBits="8" encoding="twosComplement"/>
                  </BooleanArgumentType>
                  <EnumeratedArgumentType name="EnumArg">
                    <IntegerDataEncoding sizeInBits="8"/>
                    <EnumerationList>
                      <Enumeration value="0" label="OFF"/>
                    </EnumerationList>
                  </EnumeratedArgumentType>
                  <StringArgumentType name="StrArg">
                    <StringDataEncoding encoding="UTF-8">
                      <SizeInBits><Fixed><FixedValue>64</FixedValue></Fixed></SizeInBits>
                    </StringDataEncoding>
                  </StringArgumentType>
                  <BinaryArgumentType name="BinArg">
                    <BinaryDataEncoding>
                      <SizeInBits><FixedValue>32</FixedValue></SizeInBits>
                    </BinaryDataEncoding>
                  </BinaryArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();

        let ArgumentType::Integer(int_t) = cmd.argument_types.get("IntArg").unwrap() else {
            panic!("expected Integer")
        };
        assert_eq!(int_t.unit_set.len(), 1);
        assert_eq!(int_t.unit_set[0].value, "km");
        assert!((int_t.unit_set[0].power.unwrap() - 2.0).abs() < f64::EPSILON);
        assert_eq!(int_t.unit_set[0].factor.as_deref(), Some("1e3"));
        assert_eq!(int_t.unit_set[0].description.as_deref(), Some("square km"));
        let enc = int_t.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, 16);
        assert_eq!(enc.encoding, IntegerEncoding::Unsigned);

        let ArgumentType::Float(flt_t) = cmd.argument_types.get("FloatArg").unwrap() else {
            panic!("expected Float")
        };
        let fenc = flt_t.encoding.as_ref().unwrap();
        assert_eq!(fenc.size_in_bits, FloatSizeInBits::F32);

        let ArgumentType::Boolean(bool_t) = cmd.argument_types.get("BoolArg").unwrap() else {
            panic!("expected Boolean")
        };
        let benc = bool_t.encoding.as_ref().unwrap();
        assert_eq!(benc.encoding, IntegerEncoding::TwosComplement);

        let ArgumentType::String(str_t) = cmd.argument_types.get("StrArg").unwrap() else {
            panic!("expected String")
        };
        use crate::model::types::{StringSize, StringEncoding};
        let senc = str_t.encoding.as_ref().unwrap();
        assert_eq!(senc.encoding, StringEncoding::UTF8);
        assert_eq!(senc.size_in_bits, Some(StringSize::Fixed(64)));

        let ArgumentType::Binary(bin_t) = cmd.argument_types.get("BinArg").unwrap() else {
            panic!("expected Binary")
        };
        use crate::model::types::BinarySize;
        let benc = bin_t.encoding.as_ref().unwrap();
        assert_eq!(benc.size_in_bits, BinarySize::Fixed(32));
    }

    // ── Test 22: BooleanExpression restriction criteria ───────────────────────

    #[test]
    fn boolean_expression_and_or_restriction_criteria() {
        use crate::model::container::{BooleanExpression, RestrictionCriteria};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="PktAnd">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <BooleanExpression>
                          <ANDedConditions>
                            <Condition parameterRef="APID" value="5"/>
                            <Condition parameterRef="Version" value="1"/>
                          </ANDedConditions>
                        </BooleanExpression>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="PktOr">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <BooleanExpression>
                          <ORedConditions>
                            <Condition parameterRef="Type" value="A"/>
                            <Condition parameterRef="Type" value="B"/>
                          </ORedConditions>
                        </BooleanExpression>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                  <SequenceContainer name="PktCond">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <BooleanExpression>
                          <Condition parameterRef="Flag" value="1"/>
                        </BooleanExpression>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();

        // AND expression
        let pkt_and = tm.containers.get("PktAnd").unwrap();
        let base = pkt_and.base_container.as_ref().unwrap();
        let RestrictionCriteria::BooleanExpression(BooleanExpression::And(terms)) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected ANDed BooleanExpression")
        };
        assert_eq!(terms.len(), 2);
        let BooleanExpression::Condition(c0) = &terms[0] else { panic!("expected Condition") };
        assert_eq!(c0.parameter_ref, "APID");
        assert_eq!(c0.value, "5");
        let BooleanExpression::Condition(c1) = &terms[1] else { panic!("expected Condition") };
        assert_eq!(c1.parameter_ref, "Version");

        // OR expression
        let pkt_or = tm.containers.get("PktOr").unwrap();
        let base = pkt_or.base_container.as_ref().unwrap();
        let RestrictionCriteria::BooleanExpression(BooleanExpression::Or(terms)) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected ORed BooleanExpression")
        };
        assert_eq!(terms.len(), 2);

        // Bare Condition expression
        let pkt_cond = tm.containers.get("PktCond").unwrap();
        let base = pkt_cond.base_container.as_ref().unwrap();
        let RestrictionCriteria::BooleanExpression(BooleanExpression::Condition(cmp)) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected Condition BooleanExpression")
        };
        assert_eq!(cmp.parameter_ref, "Flag");
        assert_eq!(cmp.value, "1");
    }

    // ── Test 23: NextContainer restriction criteria ───────────────────────────

    #[test]
    fn next_container_restriction_criteria() {
        use crate::model::container::RestrictionCriteria;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Router">
                    <BaseContainer containerRef="Base">
                      <RestrictionCriteria>
                        <NextContainer containerRef="Payload"/>
                      </RestrictionCriteria>
                    </BaseContainer>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("Router").unwrap();
        let base = c.base_container.as_ref().unwrap();
        let RestrictionCriteria::NextContainer { container_ref } =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected NextContainer")
        };
        assert_eq!(container_ref, "Payload");
    }

    // ── Test 24: entry list variants — ContainerRef, ArrayParameterRef, FixedValue ──

    #[test]
    fn entry_list_variants() {
        use crate::model::container::{ReferenceLocation, SequenceEntry};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ParameterRefEntry parameterRef="Header">
                        <LocationInContainerInBits referenceLocation="containerStart">
                          <FixedValue>0</FixedValue>
                        </LocationInContainerInBits>
                      </ParameterRefEntry>
                      <ContainerRefEntry containerRef="SubPacket">
                        <LocationInContainerInBits>
                          <FixedValue>32</FixedValue>
                        </LocationInContainerInBits>
                      </ContainerRefEntry>
                      <ArrayParameterRefEntry parameterRef="Samples">
                        <LocationInContainerInBits referenceLocation="containerStart">
                          <FixedValue>64</FixedValue>
                        </LocationInContainerInBits>
                      </ArrayParameterRefEntry>
                      <FixedValueEntry sizeInBits="8" binaryValue="FF"/>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("Pkt").unwrap();
        assert_eq!(c.entry_list.len(), 4);

        // ParameterRefEntry with containerStart location
        let SequenceEntry::ParameterRef(p) = &c.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        assert_eq!(p.parameter_ref, "Header");
        let loc = p.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 0);

        // ContainerRefEntry with previousEntry location
        let SequenceEntry::ContainerRef(cr) = &c.entry_list[1] else {
            panic!("expected ContainerRef")
        };
        assert_eq!(cr.container_ref, "SubPacket");
        let loc = cr.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::PreviousEntry);
        assert_eq!(loc.bit_offset, 32);

        // ArrayParameterRefEntry
        let SequenceEntry::ArrayParameterRef(ar) = &c.entry_list[2] else {
            panic!("expected ArrayParameterRef")
        };
        assert_eq!(ar.parameter_ref, "Samples");
        let loc = ar.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 64);

        // FixedValueEntry
        let SequenceEntry::FixedValue(fv) = &c.entry_list[3] else {
            panic!("expected FixedValue")
        };
        assert_eq!(fv.size_in_bits, 8);
        assert_eq!(fv.binary_value.as_deref(), Some("FF"));
        assert!(fv.location.is_none());
    }

    // ── Test 25: IncludeCondition on ParameterRefEntry ────────────────────────

    #[test]
    fn include_condition_on_parameter_ref_entry() {
        use crate::model::container::{MatchCriteria, SequenceEntry};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <EntryList>
                      <ParameterRefEntry parameterRef="OptionalField">
                        <IncludeCondition>
                          <Comparison parameterRef="HasOptional" value="1"/>
                        </IncludeCondition>
                      </ParameterRefEntry>
                    </EntryList>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("Pkt").unwrap();
        let SequenceEntry::ParameterRef(e) = &c.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        let MatchCriteria::Comparison(cmp) = e.include_condition.as_ref().unwrap() else {
            panic!("expected Comparison IncludeCondition")
        };
        assert_eq!(cmp.parameter_ref, "HasOptional");
        assert_eq!(cmp.value, "1");
    }

    // ── Test 26: string encoding size variants (TerminationChar, Variable) ────

    #[test]
    fn string_parameter_type_size_variants() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::{StringEncoding, StringSize};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <StringParameterType name="NullTerm">
                    <StringDataEncoding encoding="US-ASCII">
                      <SizeInBits>
                        <TerminationChar termChar="00"/>
                      </SizeInBits>
                    </StringDataEncoding>
                  </StringParameterType>
                  <StringParameterType name="VarLen">
                    <StringDataEncoding encoding="UTF-16">
                      <SizeInBits>
                        <Variable maxSizeInBits="256"/>
                      </SizeInBits>
                    </StringDataEncoding>
                  </StringParameterType>
                  <StringParameterType name="NoSize">
                    <StringDataEncoding encoding="ISO-8859-1"/>
                  </StringParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();

        let ParameterType::String(null_t) = tm.parameter_types.get("NullTerm").unwrap() else {
            panic!("expected String")
        };
        let enc = null_t.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::UsAscii);
        assert_eq!(enc.size_in_bits, Some(StringSize::TerminationChar(0x00)));

        let ParameterType::String(var_t) = tm.parameter_types.get("VarLen").unwrap() else {
            panic!("expected String")
        };
        let enc = var_t.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::UTF16);
        assert_eq!(enc.size_in_bits, Some(StringSize::Variable { max_size_in_bits: 256 }));

        let ParameterType::String(no_t) = tm.parameter_types.get("NoSize").unwrap() else {
            panic!("expected String")
        };
        let enc = no_t.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::Iso8859_1);
        assert!(enc.size_in_bits.is_none());
    }

    // ── Test 27: binary data encoding — variable (DynamicValue) ──────────────

    #[test]
    fn binary_parameter_type_variable_size() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::BinarySize;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <BinaryParameterType name="DynBin">
                    <BinaryDataEncoding>
                      <SizeInBits>
                        <DynamicValue sizeReference="LenField"/>
                      </SizeInBits>
                    </BinaryDataEncoding>
                  </BinaryParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Binary(t) = tm.parameter_types.get("DynBin").unwrap() else {
            panic!("expected Binary")
        };
        let enc = t.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, BinarySize::Variable { size_reference: "LenField".into() });
    }

    // ── Test 28: unit attributes — power, factor, description ─────────────────

    #[test]
    fn unit_with_all_attributes() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <FloatParameterType name="Temp">
                    <UnitSet>
                      <Unit power="-1.0" factor="0.001" description="milli">mK</Unit>
                      <Unit>degC</Unit>
                    </UnitSet>
                  </FloatParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Float(t) = tm.parameter_types.get("Temp").unwrap() else {
            panic!("expected Float")
        };
        assert_eq!(t.unit_set.len(), 2);
        let u0 = &t.unit_set[0];
        assert_eq!(u0.value, "mK");
        assert!((u0.power.unwrap() - (-1.0)).abs() < f64::EPSILON);
        assert_eq!(u0.factor.as_deref(), Some("0.001"));
        assert_eq!(u0.description.as_deref(), Some("milli"));

        let u1 = &t.unit_set[1];
        assert_eq!(u1.value, "degC");
        assert!(u1.power.is_none());
        assert!(u1.factor.is_none());
        assert!(u1.description.is_none());
    }

    // ── Test 29: calibrator — polynomial and spline in data encoding ──────────

    #[test]
    fn integer_data_encoding_polynomial_calibrator() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::{Calibrator, IntegerEncoding};

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="Raw">
                    <IntegerDataEncoding sizeInBits="16" encoding="signMagnitude">
                      <DefaultCalibrator>
                        <PolynomialCalibrator>
                          <Term coefficient="0.5" exponent="1"/>
                          <Term coefficient="10.0" exponent="0"/>
                        </PolynomialCalibrator>
                      </DefaultCalibrator>
                    </IntegerDataEncoding>
                  </IntegerParameterType>
                  <FloatParameterType name="Splined">
                    <FloatDataEncoding sizeInBits="32" encoding="milStd1750A"
                        byteOrder="leastSignificantByteFirst">
                      <DefaultCalibrator>
                        <SplineCalibrator order="1">
                          <SplinePoint raw="0.0" calibrated="0.0"/>
                          <SplinePoint raw="100.0" calibrated="50.0"/>
                        </SplineCalibrator>
                      </DefaultCalibrator>
                    </FloatDataEncoding>
                  </FloatParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();

        let ParameterType::Integer(int_t) = tm.parameter_types.get("Raw").unwrap() else {
            panic!("expected Integer")
        };
        let enc = int_t.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, IntegerEncoding::SignMagnitude);
        let Calibrator::Polynomial(poly) = enc.default_calibrator.as_ref().unwrap() else {
            panic!("expected Polynomial")
        };
        // coefficients stored dense by exponent index: [constant, x^1, ...]
        // Term exponent=1 coef=0.5 → coefficients[1]=0.5; exponent=0 coef=10.0 → coefficients[0]=10.0
        assert!(poly.coefficients.len() >= 2);
        assert!((poly.coefficients[0] - 10.0).abs() < f64::EPSILON);
        assert!((poly.coefficients[1] - 0.5).abs() < f64::EPSILON);

        let ParameterType::Float(flt_t) = tm.parameter_types.get("Splined").unwrap() else {
            panic!("expected Float")
        };
        let enc = flt_t.encoding.as_ref().unwrap();
        use crate::model::types::{ByteOrder, FloatEncoding};
        assert_eq!(enc.encoding, FloatEncoding::MilStd1750A);
        assert_eq!(enc.byte_order, Some(ByteOrder::LeastSignificantByteFirst));
        let Calibrator::SplineCalibrator(spline) = enc.default_calibrator.as_ref().unwrap() else {
            panic!("expected SplineCalibrator")
        };
        assert_eq!(spline.points.len(), 2);
        assert!((spline.points[0].raw - 0.0).abs() < f64::EPSILON);
        assert!((spline.points[1].raw - 100.0).abs() < f64::EPSILON);
        assert!((spline.points[1].calibrated - 50.0).abs() < f64::EPSILON);
    }

    // ── Test 30: invalid encoding attribute returns error ─────────────────────

    #[test]
    fn invalid_integer_encoding_returns_error() {
        use crate::ParseError;
        let result = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="T">
                    <IntegerDataEncoding sizeInBits="8" encoding="bogus"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            matches!(result, Err(ParseError::InvalidValue { attr: "encoding", .. })),
            "expected InvalidValue(encoding), got {:?}",
            result
        );
    }

    #[test]
    fn invalid_float_encoding_returns_error() {
        use crate::ParseError;
        let result = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <FloatParameterType name="T">
                    <FloatDataEncoding sizeInBits="32" encoding="badformat"/>
                  </FloatParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            matches!(result, Err(ParseError::InvalidValue { attr: "encoding", .. })),
            "expected InvalidValue(encoding), got {:?}",
            result
        );
    }

    #[test]
    fn invalid_float_size_returns_error() {
        use crate::ParseError;
        let result = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <FloatParameterType name="T">
                    <FloatDataEncoding sizeInBits="48"/>
                  </FloatParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#);
        assert!(
            matches!(result, Err(ParseError::InvalidValue { attr: "sizeInBits", .. })),
            "expected InvalidValue(sizeInBits), got {:?}",
            result
        );
    }

    // ── Test 31: integer parameter type with long description and base type ───

    #[test]
    fn integer_parameter_type_long_description_and_base_type() {
        use crate::model::telemetry::ParameterType;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="DerivedInt" baseType="BaseInt"
                      signed="false" sizeInBits="32">
                    <LongDescription>A derived unsigned 32-bit integer.</LongDescription>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Integer(t) = tm.parameter_types.get("DerivedInt").unwrap() else {
            panic!("expected Integer")
        };
        assert_eq!(t.base_type.as_deref(), Some("BaseInt"));
        assert!(!t.signed);
        assert_eq!(t.size_in_bits, Some(32));
        assert_eq!(
            t.long_description.as_deref(),
            Some("A derived unsigned 32-bit integer.")
        );
    }

    // ── Test 32: abstract container and long description ──────────────────────

    #[test]
    fn abstract_container_with_long_description() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="BaseContainer" abstract="true">
                    <LongDescription>Base packet structure.</LongDescription>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("BaseContainer").unwrap();
        assert!(c.r#abstract);
        assert_eq!(c.long_description.as_deref(), Some("Base packet structure."));
    }

    // ── Test 34: CommandContainer with BaseContainer ──────────────────────────

    #[test]
    fn command_container_with_base_container() {
        use crate::model::command::CommandEntry;
        use crate::model::container::RestrictionCriteria;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="DerivedCmd">
                    <CommandContainer name="DerivedContainer">
                      <BaseContainer containerRef="BaseContainer">
                        <RestrictionCriteria>
                          <Comparison parameterRef="APID" value="200"/>
                        </RestrictionCriteria>
                      </BaseContainer>
                      <EntryList>
                        <ArgumentRefEntry argumentRef="arg1">
                          <LocationInContainerInBits referenceLocation="containerStart">
                            <FixedValue>32</FixedValue>
                          </LocationInContainerInBits>
                        </ArgumentRefEntry>
                        <ParameterRefEntry parameterRef="SharedParam"/>
                        <FixedValueEntry sizeInBits="16" binaryValue="DEAD"/>
                      </EntryList>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let mc = cmd.meta_commands.get("DerivedCmd").unwrap();
        let cc = mc.command_container.as_ref().unwrap();

        // BaseContainer
        let base = cc.base_container.as_ref().unwrap();
        assert_eq!(base.container_ref, "BaseContainer");
        let RestrictionCriteria::Comparison(cmp) =
            base.restriction_criteria.as_ref().unwrap()
        else {
            panic!("expected Comparison")
        };
        assert_eq!(cmp.parameter_ref, "APID");
        assert_eq!(cmp.value, "200");

        // Entry list
        assert_eq!(cc.entry_list.len(), 3);

        // ArgumentRefEntry with location
        let CommandEntry::ArgumentRef(ae) = &cc.entry_list[0] else {
            panic!("expected ArgumentRef at [0]")
        };
        assert_eq!(ae.argument_ref, "arg1");
        let loc = ae.location.as_ref().unwrap();
        assert_eq!(loc.bit_offset, 32);
        use crate::model::container::ReferenceLocation;
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);

        // ParameterRefEntry inside command container
        let CommandEntry::ParameterRef(_) = &cc.entry_list[1] else {
            panic!("expected ParameterRef at [1]")
        };

        // FixedValueEntry inside command container
        let CommandEntry::FixedValue(fv) = &cc.entry_list[2] else {
            panic!("expected FixedValue at [2]")
        };
        assert_eq!(fv.size_in_bits, 16);
        assert_eq!(fv.binary_value.as_deref(), Some("DEAD"));
    }

    // ── Test 35: CommandContainer FixedValueEntry with location ───────────────

    #[test]
    fn command_container_fixed_value_with_location() {
        use crate::model::command::CommandEntry;
        use crate::model::container::ReferenceLocation;

        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="PaddedCmd">
                    <CommandContainer name="PaddedContainer">
                      <EntryList>
                        <FixedValueEntry sizeInBits="8">
                          <LocationInContainerInBits referenceLocation="containerStart">
                            <FixedValue>0</FixedValue>
                          </LocationInContainerInBits>
                        </FixedValueEntry>
                      </EntryList>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let cc = cmd.meta_commands.get("PaddedCmd").unwrap()
            .command_container.as_ref().unwrap();
        let CommandEntry::FixedValue(fv) = &cc.entry_list[0] else {
            panic!("expected FixedValue")
        };
        assert_eq!(fv.size_in_bits, 8);
        let loc = fv.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 0);
    }

    // ── Test 36: CommandContainerSet (standalone shared containers) ───────────

    #[test]
    fn command_container_set_parsed() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <CommandContainerSet>
                  <SequenceContainer name="CcsdsHeader" shortDescription="CCSDS TC header">
                    <EntryList>
                      <ParameterRefEntry parameterRef="APID"/>
                    </EntryList>
                  </SequenceContainer>
                  <SequenceContainer name="CcsdsBody">
                    <EntryList/>
                  </SequenceContainer>
                </CommandContainerSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        assert_eq!(cmd.command_containers.len(), 2);

        let hdr = cmd.command_containers.get("CcsdsHeader").unwrap();
        assert_eq!(hdr.name, "CcsdsHeader");
        assert_eq!(hdr.short_description.as_deref(), Some("CCSDS TC header"));
        assert_eq!(hdr.entry_list.len(), 1);

        let body = cmd.command_containers.get("CcsdsBody").unwrap();
        assert_eq!(body.entry_list.len(), 0);
    }

    // ── Test 33: MetaCommand abstract with long description ───────────────────

    #[test]
    fn abstract_meta_command_with_long_description() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <MetaCommandSet>
                  <MetaCommand name="BaseCmd" abstract="true">
                    <LongDescription>Abstract base command.</LongDescription>
                    <CommandContainer name="BaseCmdContainer">
                      <EntryList/>
                    </CommandContainer>
                  </MetaCommand>
                </MetaCommandSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let mc = cmd.meta_commands.get("BaseCmd").unwrap();
        assert!(mc.r#abstract);
        assert_eq!(mc.long_description.as_deref(), Some("Abstract base command."));
    }

    // ── Test 37: Header with legacy AuthorInformation elements ────────────────

    #[test]
    fn header_author_information_legacy() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <Header version="1.0" date="2024-01-01" classification="Unclassified">
                <AuthorSet>
                  <AuthorInformation name="Alice" role="Engineer"/>
                  <AuthorInformation name="Bob"/>
                </AuthorSet>
              </Header>
            </SpaceSystem>
        "#).unwrap();

        let hdr = ss.header.as_ref().unwrap();
        assert_eq!(hdr.version.as_deref(), Some("1.0"));
        assert_eq!(hdr.classification.as_deref(), Some("Unclassified"));
        assert_eq!(hdr.author_set.len(), 2);
        assert_eq!(hdr.author_set[0].name, "Alice");
        assert_eq!(hdr.author_set[0].role.as_deref(), Some("Engineer"));
        assert_eq!(hdr.author_set[1].name, "Bob");
        assert!(hdr.author_set[1].role.is_none());
    }

    // ── Test 38: Header with standard <Author> text elements ─────────────────

    #[test]
    fn header_author_standard_format() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <Header version="2.0">
                <AuthorSet>
                  <Author>Charlie (Lead)</Author>
                  <Author>Dana</Author>
                </AuthorSet>
              </Header>
            </SpaceSystem>
        "#).unwrap();

        let hdr = ss.header.as_ref().unwrap();
        assert_eq!(hdr.author_set.len(), 2);
        assert_eq!(hdr.author_set[0].name, "Charlie (Lead)");
        assert!(hdr.author_set[0].role.is_none());
        assert_eq!(hdr.author_set[1].name, "Dana");
    }

    // ── Test 39: Header with NoteSet ──────────────────────────────────────────

    #[test]
    fn header_note_set() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <Header>
                <NoteSet>
                  <Note>First note.</Note>
                  <Note>Second note.</Note>
                </NoteSet>
              </Header>
            </SpaceSystem>
        "#).unwrap();

        let hdr = ss.header.as_ref().unwrap();
        assert_eq!(hdr.note_set.len(), 2);
        assert_eq!(hdr.note_set[0], "First note.");
        assert_eq!(hdr.note_set[1], "Second note.");
    }

    // ── Test 40: Parameter with LongDescription and DataSource=constant ───────

    #[test]
    fn parameter_long_description_and_datasource_constant() {
        use crate::model::telemetry::DataSource;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="uint8" signed="false">
                    <IntegerDataEncoding sizeInBits="8"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="ConstParam" parameterTypeRef="uint8"
                             shortDescription="A constant">
                    <LongDescription>This parameter is constant.</LongDescription>
                    <ParameterProperties dataSource="constant"/>
                  </Parameter>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let p = tm.parameters.get("ConstParam").unwrap();
        assert_eq!(p.short_description.as_deref(), Some("A constant"));
        assert_eq!(p.long_description.as_deref(), Some("This parameter is constant."));
        let props = p.parameter_properties.as_ref().unwrap();
        assert_eq!(props.data_source, Some(DataSource::Constant));
    }

    // ── Test 41: DataSource variants local and ground ─────────────────────────

    #[test]
    fn parameter_datasource_local_and_ground() {
        use crate::model::telemetry::DataSource;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="uint8" signed="false">
                    <IntegerDataEncoding sizeInBits="8"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="LocalP" parameterTypeRef="uint8">
                    <ParameterProperties dataSource="local"/>
                  </Parameter>
                  <Parameter name="GroundP" parameterTypeRef="uint8">
                    <ParameterProperties dataSource="ground" readOnly="true"/>
                  </Parameter>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let local = tm.parameters.get("LocalP").unwrap();
        assert_eq!(
            local.parameter_properties.as_ref().unwrap().data_source,
            Some(DataSource::Local)
        );
        let ground = tm.parameters.get("GroundP").unwrap();
        let gp = ground.parameter_properties.as_ref().unwrap();
        assert_eq!(gp.data_source, Some(DataSource::Ground));
        assert!(gp.read_only);
    }

    // ── Test 42: DataSource derived + nested SpaceSystem ─────────────────────

    #[test]
    fn parameter_datasource_derived_and_nested_spacesystem() {
        use crate::model::telemetry::DataSource;
        let ss = parse_str(r#"
            <SpaceSystem name="Root">
              <SpaceSystem name="Child">
                <TelemetryMetaData>
                  <ParameterTypeSet>
                    <IntegerParameterType name="uint8" signed="false">
                      <IntegerDataEncoding sizeInBits="8"/>
                    </IntegerParameterType>
                  </ParameterTypeSet>
                  <ParameterSet>
                    <Parameter name="DerivedP" parameterTypeRef="uint8">
                      <ParameterProperties dataSource="derived"/>
                    </Parameter>
                  </ParameterSet>
                </TelemetryMetaData>
              </SpaceSystem>
            </SpaceSystem>
        "#).unwrap();

        assert_eq!(ss.sub_systems.len(), 1);
        let child = &ss.sub_systems[0];
        assert_eq!(child.name, "Child");
        let tm = child.telemetry.as_ref().unwrap();
        let p = tm.parameters.get("DerivedP").unwrap();
        assert_eq!(
            p.parameter_properties.as_ref().unwrap().data_source,
            Some(DataSource::Derived)
        );
    }

    // ── Test 43: Header with mixed AuthorSet and all attributes ──────────────

    #[test]
    fn header_mixed_author_set_and_all_attributes() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <Header version="3.0" validationStatus="Draft"
                      classificationInstructions="Handle with care">
                <AuthorSet>
                  <AuthorInformation name="Eve" role="Reviewer"/>
                  <Author>Frank</Author>
                </AuthorSet>
                <NoteSet>
                  <Note>Review complete.</Note>
                </NoteSet>
              </Header>
            </SpaceSystem>
        "#).unwrap();

        let hdr = ss.header.as_ref().unwrap();
        assert_eq!(hdr.validation_status.as_deref(), Some("Draft"));
        assert_eq!(hdr.classification_instructions.as_deref(), Some("Handle with care"));
        assert_eq!(hdr.author_set.len(), 2);
        assert_eq!(hdr.author_set[0].name, "Eve");
        assert_eq!(hdr.author_set[0].role.as_deref(), Some("Reviewer"));
        assert_eq!(hdr.author_set[1].name, "Frank");
        assert_eq!(hdr.note_set.len(), 1);
        assert_eq!(hdr.note_set[0], "Review complete.");
    }

    // ── Task 7 tests: encoding variants and edge cases ────────────────────────

    // ── Test 44: OnesComplement integer encoding ───────────────────────────────

    #[test]
    fn integer_encoding_ones_complement() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::IntegerEncoding;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="p" signed="true">
                    <IntegerDataEncoding sizeInBits="8" encoding="onesComplement"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Integer(t) = tm.parameter_types.get("p").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(t.encoding.as_ref().unwrap().encoding, IntegerEncoding::OnesComplement);
    }

    // ── Test 45: BCD integer encoding ─────────────────────────────────────────

    #[test]
    fn integer_encoding_bcd() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::IntegerEncoding;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="p" signed="false">
                    <IntegerDataEncoding sizeInBits="16" encoding="BCD"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Integer(t) = tm.parameter_types.get("p").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(t.encoding.as_ref().unwrap().encoding, IntegerEncoding::BCD);
    }

    // ── Test 46: PackedBCD integer encoding ───────────────────────────────────

    #[test]
    fn integer_encoding_packed_bcd() {
        use crate::model::telemetry::ParameterType;
        use crate::model::types::IntegerEncoding;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="p" signed="false">
                    <IntegerDataEncoding sizeInBits="8" encoding="packedBCD"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Integer(t) = tm.parameter_types.get("p").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(t.encoding.as_ref().unwrap().encoding, IntegerEncoding::PackedBCD);
    }

    // ── Test 47: byteOrder on IntegerArgumentType ─────────────────────────────

    #[test]
    fn integer_argument_type_byte_order() {
        use crate::model::command::ArgumentType;
        use crate::model::types::ByteOrder;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="BigEndInt" signed="false">
                    <IntegerDataEncoding sizeInBits="32"
                        byteOrder="mostSignificantByteFirst"/>
                  </IntegerArgumentType>
                  <IntegerArgumentType name="LittleEndInt" signed="false">
                    <IntegerDataEncoding sizeInBits="32"
                        byteOrder="leastSignificantByteFirst"/>
                  </IntegerArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let ArgumentType::Integer(big) = cmd.argument_types.get("BigEndInt").unwrap() else {
            panic!("expected Integer");
        };
        let ArgumentType::Integer(little) = cmd.argument_types.get("LittleEndInt").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(
            big.encoding.as_ref().unwrap().byte_order,
            Some(ByteOrder::MostSignificantByteFirst)
        );
        assert_eq!(
            little.encoding.as_ref().unwrap().byte_order,
            Some(ByteOrder::LeastSignificantByteFirst)
        );
    }

    // ── Test 48: String size Fixed(0) fallback ────────────────────────────────
    // When <SizeInBits> contains no recognized child, parser falls back to Fixed(0).

    #[test]
    fn string_size_fixed_zero_fallback() {
        use crate::model::types::StringSize;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <StringParameterType name="s">
                    <StringDataEncoding encoding="UTF-8">
                      <SizeInBits>
                        <UnknownElement/>
                      </SizeInBits>
                    </StringDataEncoding>
                  </StringParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        use crate::model::telemetry::ParameterType;
        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::String(st) = tm.parameter_types.get("s").unwrap() else {
            panic!("expected String");
        };
        let enc = st.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, Some(StringSize::Fixed(0)));
    }

    // ── Test 49: UTF-16 string encoding on ArgumentType ───────────────────────

    #[test]
    fn string_argument_type_utf16_encoding() {
        use crate::model::command::ArgumentType;
        use crate::model::types::StringEncoding;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <StringArgumentType name="Utf16Arg">
                    <StringDataEncoding encoding="UTF-16"/>
                  </StringArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let ArgumentType::String(st) = cmd.argument_types.get("Utf16Arg").unwrap() else {
            panic!("expected String");
        };
        let enc = st.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::UTF16);
    }

    // ── Task 10 tests: AliasSet parsing ───────────────────────────────────────

    // ── Test 50: AliasSet on IntegerParameterType ─────────────────────────────

    #[test]
    fn alias_set_on_integer_parameter_type() {
        use crate::model::telemetry::ParameterType;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="p" signed="false">
                    <AliasSet>
                      <Alias nameSpace="YAMCS" alias="p_yamcs"/>
                      <Alias nameSpace="MDB" alias="p_mdb"/>
                    </AliasSet>
                    <IntegerDataEncoding sizeInBits="8"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let ParameterType::Integer(t) = tm.parameter_types.get("p").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(t.alias_set.len(), 2);
        assert_eq!(t.alias_set[0].name_space, "YAMCS");
        assert_eq!(t.alias_set[0].alias, "p_yamcs");
        assert_eq!(t.alias_set[1].name_space, "MDB");
        assert_eq!(t.alias_set[1].alias, "p_mdb");
    }

    // ── Test 51: AliasSet on Parameter ────────────────────────────────────────

    #[test]
    fn alias_set_on_parameter() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <IntegerParameterType name="uint8" signed="false">
                    <IntegerDataEncoding sizeInBits="8"/>
                  </IntegerParameterType>
                </ParameterTypeSet>
                <ParameterSet>
                  <Parameter name="Voltage" parameterTypeRef="uint8">
                    <AliasSet>
                      <Alias nameSpace="Ground" alias="VLT"/>
                    </AliasSet>
                  </Parameter>
                </ParameterSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let p = tm.parameters.get("Voltage").unwrap();
        assert_eq!(p.alias_set.len(), 1);
        assert_eq!(p.alias_set[0].name_space, "Ground");
        assert_eq!(p.alias_set[0].alias, "VLT");
    }

    // ── Test 52: AliasSet on SequenceContainer ────────────────────────────────

    #[test]
    fn alias_set_on_sequence_container() {
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ContainerSet>
                  <SequenceContainer name="Pkt">
                    <AliasSet>
                      <Alias nameSpace="CCSDS" alias="0x01"/>
                    </AliasSet>
                    <EntryList/>
                  </SequenceContainer>
                </ContainerSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();
        let c = tm.containers.get("Pkt").unwrap();
        assert_eq!(c.alias_set.len(), 1);
        assert_eq!(c.alias_set[0].name_space, "CCSDS");
        assert_eq!(c.alias_set[0].alias, "0x01");
    }

    // ── Test 53: AliasSet on IntegerArgumentType ──────────────────────────────

    #[test]
    fn alias_set_on_integer_argument_type() {
        use crate::model::command::ArgumentType;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <IntegerArgumentType name="IntArg" signed="false">
                    <AliasSet>
                      <Alias nameSpace="DB" alias="int_arg_db"/>
                    </AliasSet>
                    <IntegerDataEncoding sizeInBits="16"/>
                  </IntegerArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let ArgumentType::Integer(t) = cmd.argument_types.get("IntArg").unwrap() else {
            panic!("expected Integer");
        };
        assert_eq!(t.alias_set.len(), 1);
        assert_eq!(t.alias_set[0].name_space, "DB");
        assert_eq!(t.alias_set[0].alias, "int_arg_db");
    }

    // ── Test 54: AliasSet on remaining ParameterType variants ─────────────────

    #[test]
    fn alias_set_on_remaining_parameter_types() {
        use crate::model::telemetry::ParameterType;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <TelemetryMetaData>
                <ParameterTypeSet>
                  <FloatParameterType name="flt">
                    <AliasSet><Alias nameSpace="NS" alias="flt_a"/></AliasSet>
                  </FloatParameterType>
                  <EnumeratedParameterType name="enm">
                    <AliasSet><Alias nameSpace="NS" alias="enm_a"/></AliasSet>
                    <IntegerDataEncoding sizeInBits="8"/>
                    <EnumerationList>
                      <Enumeration value="0" label="OFF"/>
                    </EnumerationList>
                  </EnumeratedParameterType>
                  <BooleanParameterType name="bln">
                    <AliasSet><Alias nameSpace="NS" alias="bln_a"/></AliasSet>
                  </BooleanParameterType>
                  <StringParameterType name="str">
                    <AliasSet><Alias nameSpace="NS" alias="str_a"/></AliasSet>
                  </StringParameterType>
                  <BinaryParameterType name="bin">
                    <AliasSet><Alias nameSpace="NS" alias="bin_a"/></AliasSet>
                  </BinaryParameterType>
                  <AggregateParameterType name="agg">
                    <AliasSet><Alias nameSpace="NS" alias="agg_a"/></AliasSet>
                    <MemberList>
                      <Member name="x" typeRef="flt"/>
                    </MemberList>
                  </AggregateParameterType>
                </ParameterTypeSet>
              </TelemetryMetaData>
            </SpaceSystem>
        "#).unwrap();

        let tm = ss.telemetry.as_ref().unwrap();

        let ParameterType::Float(flt) = tm.parameter_types.get("flt").unwrap() else { panic!() };
        assert_eq!(flt.alias_set[0].alias, "flt_a");
        let ParameterType::Enumerated(enm) = tm.parameter_types.get("enm").unwrap() else { panic!() };
        assert_eq!(enm.alias_set[0].alias, "enm_a");
        let ParameterType::Boolean(bln) = tm.parameter_types.get("bln").unwrap() else { panic!() };
        assert_eq!(bln.alias_set[0].alias, "bln_a");
        let ParameterType::String(str_t) = tm.parameter_types.get("str").unwrap() else { panic!() };
        assert_eq!(str_t.alias_set[0].alias, "str_a");
        let ParameterType::Binary(bin) = tm.parameter_types.get("bin").unwrap() else { panic!() };
        assert_eq!(bin.alias_set[0].alias, "bin_a");
        let ParameterType::Aggregate(agg) = tm.parameter_types.get("agg").unwrap() else { panic!() };
        assert_eq!(agg.alias_set[0].alias, "agg_a");
    }

    // ── Test 55: AliasSet on remaining ArgumentType variants ──────────────────

    #[test]
    fn alias_set_on_remaining_argument_types() {
        use crate::model::command::ArgumentType;
        let ss = parse_str(r#"
            <SpaceSystem name="Test">
              <CommandMetaData>
                <ArgumentTypeSet>
                  <FloatArgumentType name="FltArg">
                    <AliasSet><Alias nameSpace="NS" alias="flt_a"/></AliasSet>
                  </FloatArgumentType>
                  <EnumeratedArgumentType name="EnmArg">
                    <AliasSet><Alias nameSpace="NS" alias="enm_a"/></AliasSet>
                    <IntegerDataEncoding sizeInBits="8"/>
                    <EnumerationList>
                      <Enumeration value="0" label="OFF"/>
                    </EnumerationList>
                  </EnumeratedArgumentType>
                  <BooleanArgumentType name="BlnArg">
                    <AliasSet><Alias nameSpace="NS" alias="bln_a"/></AliasSet>
                  </BooleanArgumentType>
                  <StringArgumentType name="StrArg">
                    <AliasSet><Alias nameSpace="NS" alias="str_a"/></AliasSet>
                  </StringArgumentType>
                  <BinaryArgumentType name="BinArg">
                    <AliasSet><Alias nameSpace="NS" alias="bin_a"/></AliasSet>
                  </BinaryArgumentType>
                  <AggregateArgumentType name="AggArg">
                    <AliasSet><Alias nameSpace="NS" alias="agg_a"/></AliasSet>
                    <MemberList>
                      <Member name="x" typeRef="FltArg"/>
                    </MemberList>
                  </AggregateArgumentType>
                </ArgumentTypeSet>
              </CommandMetaData>
            </SpaceSystem>
        "#).unwrap();

        let cmd = ss.command.as_ref().unwrap();
        let ArgumentType::Float(flt) = cmd.argument_types.get("FltArg").unwrap() else { panic!() };
        assert_eq!(flt.alias_set[0].alias, "flt_a");
        let ArgumentType::Enumerated(enm) = cmd.argument_types.get("EnmArg").unwrap() else { panic!() };
        assert_eq!(enm.alias_set[0].alias, "enm_a");
        let ArgumentType::Boolean(bln) = cmd.argument_types.get("BlnArg").unwrap() else { panic!() };
        assert_eq!(bln.alias_set[0].alias, "bln_a");
        let ArgumentType::String(str_t) = cmd.argument_types.get("StrArg").unwrap() else { panic!() };
        assert_eq!(str_t.alias_set[0].alias, "str_a");
        let ArgumentType::Binary(bin) = cmd.argument_types.get("BinArg").unwrap() else { panic!() };
        assert_eq!(bin.alias_set[0].alias, "bin_a");
        let ArgumentType::Aggregate(agg) = cmd.argument_types.get("AggArg").unwrap() else { panic!() };
        assert_eq!(agg.alias_set[0].alias, "agg_a");
    }
}
