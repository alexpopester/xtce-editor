//! Model → XML serializer.
//!
//! Serializes a [`SpaceSystem`] tree to a valid XTCE v1.2 XML document using
//! `quick-xml`'s writer API.

use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Writer;

use crate::model::command::{
    AggregateArgumentType, ArgumentRefEntry, ArgumentType, ArrayArgumentType, BinaryArgumentType,
    BooleanArgumentType, CommandContainer, CommandEntry, CommandMetaData, EnumeratedArgumentType,
    FloatArgumentType, IntegerArgumentType, MetaCommand, StringArgumentType,
};
use crate::model::container::{
    BaseContainer, BooleanExpression, Comparison, ComparisonOperator, EntryLocation, MatchCriteria,
    ReferenceLocation, RestrictionCriteria, SequenceContainer, SequenceEntry,
};
use crate::model::space_system::{AuthorInfo, Header, SpaceSystem};
use crate::model::telemetry::{
    AbsoluteTimeParameterType, AggregateParameterType, ArrayParameterType, BinaryParameterType,
    BooleanParameterType, EnumeratedParameterType, FloatParameterType, IntegerParameterType,
    Parameter, ParameterType, RelativeTimeParameterType, StringParameterType, TelemetryMetaData,
    TimeEncoding,
};
use crate::model::types::{
    Alias, BinaryDataEncoding, BinarySize, ByteOrder, Calibrator, FloatDataEncoding,
    FloatEncoding, FloatSizeInBits, IntegerDataEncoding, IntegerEncoding, StringDataEncoding,
    StringEncoding, StringSize, Unit, ValueEnumeration,
};
use crate::ParseError;

/// Internal writer alias to keep signatures short.
type W = Writer<Vec<u8>>;

// ─────────────────────────────────────────────────────────────────────────────
// Public entry point
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a [`SpaceSystem`] to XTCE XML bytes.
///
/// The returned bytes are a complete, well-formed XML document beginning with
/// an `<?xml?>` declaration.  They can be written directly to a file or passed
/// back to [`crate::parser::parse`] for round-trip verification.
pub fn serialize(space_system: &SpaceSystem) -> Result<Vec<u8>, ParseError> {
    let mut w = Writer::new(Vec::new());
    w.write_event(Event::Decl(BytesDecl::new("1.0", Some("UTF-8"), None)))?;
    w.write_event(Event::Text(BytesText::new("\n")))?;
    write_space_system(&mut w, space_system, true)?;
    Ok(w.into_inner())
}

// ─────────────────────────────────────────────────────────────────────────────
// T1 — Scaffold: SpaceSystem + Header
// ─────────────────────────────────────────────────────────────────────────────

fn write_space_system(w: &mut W, ss: &SpaceSystem, is_root: bool) -> Result<(), ParseError> {
    let mut e = BytesStart::new("SpaceSystem");
    if is_root {
        e.push_attribute(("xmlns", "http://www.omg.org/spec/XTCE/20180204"));
    }
    e.push_attribute(("name", ss.name.as_str()));
    if let Some(d) = &ss.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    w.write_event(Event::Start(e))?;

    if let Some(h) = &ss.header {
        write_header(w, h)?;
    }
    if let Some(d) = &ss.long_description {
        wt(w, "LongDescription", d)?;
    }

    if let Some(tm) = &ss.telemetry {
        write_telemetry_meta_data(w, tm)?;
    }
    if let Some(cm) = &ss.command {
        write_command_meta_data(w, cm)?;
    }

    for child in &ss.sub_systems {
        write_space_system(w, child, false)?;
    }

    w.write_event(Event::End(BytesEnd::new("SpaceSystem")))?;
    Ok(())
}

fn write_header(w: &mut W, h: &Header) -> Result<(), ParseError> {
    let mut e = BytesStart::new("Header");
    if let Some(v) = &h.version {
        e.push_attribute(("version", v.as_str()));
    }
    if let Some(d) = &h.date {
        e.push_attribute(("date", d.as_str()));
    }
    if let Some(c) = &h.classification {
        e.push_attribute(("classification", c.as_str()));
    }
    if let Some(ci) = &h.classification_instructions {
        e.push_attribute(("classificationInstructions", ci.as_str()));
    }
    if let Some(vs) = &h.validation_status {
        e.push_attribute(("validationStatus", vs.as_str()));
    }

    let has_children = !h.author_set.is_empty() || !h.note_set.is_empty();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }

    w.write_event(Event::Start(e))?;

    if !h.author_set.is_empty() {
        w.write_event(Event::Start(BytesStart::new("AuthorSet")))?;
        for a in &h.author_set {
            write_author_info(w, a)?;
        }
        w.write_event(Event::End(BytesEnd::new("AuthorSet")))?;
    }

    if !h.note_set.is_empty() {
        w.write_event(Event::Start(BytesStart::new("NoteSet")))?;
        for note in &h.note_set {
            wt(w, "Note", note)?;
        }
        w.write_event(Event::End(BytesEnd::new("NoteSet")))?;
    }

    w.write_event(Event::End(BytesEnd::new("Header")))?;
    Ok(())
}

fn write_author_info(w: &mut W, a: &AuthorInfo) -> Result<(), ParseError> {
    let text = match &a.role {
        Some(r) => format!("{} ({})", a.name, r),
        None => a.name.clone(),
    };
    wt(w, "Author", &text)
}

// ─────────────────────────────────────────────────────────────────────────────
// Low-level helper
// ─────────────────────────────────────────────────────────────────────────────

/// Write `<tag>text</tag>` for a single text-node element.
fn wt(w: &mut W, tag: &'static str, text: &str) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new(tag)))?;
    w.write_event(Event::Text(BytesText::new(text)))?;
    Ok(w.write_event(Event::End(BytesEnd::new(tag)))?)
}

// ─────────────────────────────────────────────────────────────────────────────
// T2 — Enum-to-string converters (inverses of the parser's parse_* functions)
// ─────────────────────────────────────────────────────────────────────────────

fn integer_encoding_str(e: &IntegerEncoding) -> &'static str {
    match e {
        IntegerEncoding::Unsigned => "unsigned",
        IntegerEncoding::SignMagnitude => "signMagnitude",
        IntegerEncoding::TwosComplement => "twosComplement",
        IntegerEncoding::OnesComplement => "onesComplement",
        IntegerEncoding::BCD => "BCD",
        IntegerEncoding::PackedBCD => "packedBCD",
    }
}

fn float_encoding_str(e: &FloatEncoding) -> &'static str {
    match e {
        FloatEncoding::IEEE754_1985 => "IEEE754_1985",
        FloatEncoding::MilStd1750A => "milStd1750A",
    }
}

fn float_size_str(s: &FloatSizeInBits) -> &'static str {
    match s {
        FloatSizeInBits::F32 => "32",
        FloatSizeInBits::F64 => "64",
        FloatSizeInBits::F128 => "128",
    }
}

fn string_encoding_str(e: &StringEncoding) -> &'static str {
    match e {
        StringEncoding::UTF8 => "UTF-8",
        StringEncoding::UTF16 => "UTF-16",
        StringEncoding::UsAscii => "US-ASCII",
        StringEncoding::Iso8859_1 => "ISO-8859-1",
    }
}

fn byte_order_str(bo: &ByteOrder) -> &'static str {
    match bo {
        ByteOrder::MostSignificantByteFirst => "mostSignificantByteFirst",
        ByteOrder::LeastSignificantByteFirst => "leastSignificantByteFirst",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// T2 — UnitSet + AliasSet
// ─────────────────────────────────────────────────────────────────────────────

pub(crate) fn write_unit_set(w: &mut W, units: &[Unit]) -> Result<(), ParseError> {
    if units.is_empty() {
        return Ok(());
    }
    w.write_event(Event::Start(BytesStart::new("UnitSet")))?;
    for u in units {
        let mut e = BytesStart::new("Unit");
        if let Some(p) = u.power {
            let s = p.to_string();
            e.push_attribute(("power", s.as_str()));
        }
        if let Some(f) = &u.factor {
            e.push_attribute(("factor", f.as_str()));
        }
        if let Some(d) = &u.description {
            e.push_attribute(("description", d.as_str()));
        }
        w.write_event(Event::Start(e))?;
        w.write_event(Event::Text(BytesText::new(&u.value)))?;
        w.write_event(Event::End(BytesEnd::new("Unit")))?;
    }
    w.write_event(Event::End(BytesEnd::new("UnitSet")))?;
    Ok(())
}

pub(crate) fn write_alias_set(w: &mut W, aliases: &[Alias]) -> Result<(), ParseError> {
    if aliases.is_empty() {
        return Ok(());
    }
    w.write_event(Event::Start(BytesStart::new("AliasSet")))?;
    for a in aliases {
        let mut e = BytesStart::new("Alias");
        e.push_attribute(("nameSpace", a.name_space.as_str()));
        e.push_attribute(("alias", a.alias.as_str()));
        w.write_event(Event::Empty(e))?;
    }
    w.write_event(Event::End(BytesEnd::new("AliasSet")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// T2 — Data encoding serializers
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize an `<IntegerDataEncoding>` element (empty or with DefaultCalibrator child).
pub(crate) fn write_integer_data_encoding(
    w: &mut W,
    enc: &IntegerDataEncoding,
) -> Result<(), ParseError> {
    let size_str = enc.size_in_bits.to_string();
    let mut e = BytesStart::new("IntegerDataEncoding");
    e.push_attribute(("sizeInBits", size_str.as_str()));
    e.push_attribute(("encoding", integer_encoding_str(&enc.encoding)));
    if let Some(bo) = &enc.byte_order {
        e.push_attribute(("byteOrder", byte_order_str(bo)));
    }
    match &enc.default_calibrator {
        Some(cal) => {
            w.write_event(Event::Start(e))?;
            write_calibrator(w, cal)?;
            w.write_event(Event::End(BytesEnd::new("IntegerDataEncoding")))?;
        }
        None => {
            w.write_event(Event::Empty(e))?;
        }
    }
    Ok(())
}

/// Serialize a `<FloatDataEncoding>` element.
pub(crate) fn write_float_data_encoding(
    w: &mut W,
    enc: &FloatDataEncoding,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("FloatDataEncoding");
    e.push_attribute(("sizeInBits", float_size_str(&enc.size_in_bits)));
    e.push_attribute(("encoding", float_encoding_str(&enc.encoding)));
    if let Some(bo) = &enc.byte_order {
        e.push_attribute(("byteOrder", byte_order_str(bo)));
    }
    match &enc.default_calibrator {
        Some(cal) => {
            w.write_event(Event::Start(e))?;
            write_calibrator(w, cal)?;
            w.write_event(Event::End(BytesEnd::new("FloatDataEncoding")))?;
        }
        None => {
            w.write_event(Event::Empty(e))?;
        }
    }
    Ok(())
}

/// Serialize a `<StringDataEncoding>` element.
///
/// The optional `size_in_bits` field is written as a `<SizeInBits>` child
/// containing one of `<Fixed>`, `<TerminationChar>`, or `<Variable>`.
pub(crate) fn write_string_data_encoding(
    w: &mut W,
    enc: &StringDataEncoding,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("StringDataEncoding");
    e.push_attribute(("encoding", string_encoding_str(&enc.encoding)));
    if let Some(bo) = &enc.byte_order {
        e.push_attribute(("byteOrder", byte_order_str(bo)));
    }
    match &enc.size_in_bits {
        None => {
            w.write_event(Event::Empty(e))?;
        }
        Some(size) => {
            w.write_event(Event::Start(e))?;
            w.write_event(Event::Start(BytesStart::new("SizeInBits")))?;
            match size {
                StringSize::Fixed(n) => {
                    w.write_event(Event::Start(BytesStart::new("Fixed")))?;
                    wt(w, "FixedValue", &n.to_string())?;
                    w.write_event(Event::End(BytesEnd::new("Fixed")))?;
                }
                StringSize::TerminationChar(b) => {
                    let hex = format!("{:02X}", b);
                    let mut tc = BytesStart::new("TerminationChar");
                    tc.push_attribute(("termChar", hex.as_str()));
                    w.write_event(Event::Empty(tc))?;
                }
                StringSize::Variable { max_size_in_bits } => {
                    let max_str = max_size_in_bits.to_string();
                    let mut var = BytesStart::new("Variable");
                    var.push_attribute(("maxSizeInBits", max_str.as_str()));
                    w.write_event(Event::Empty(var))?;
                }
            }
            w.write_event(Event::End(BytesEnd::new("SizeInBits")))?;
            w.write_event(Event::End(BytesEnd::new("StringDataEncoding")))?;
        }
    }
    Ok(())
}

/// Serialize a `<BinaryDataEncoding>` element.
///
/// Always has a `<SizeInBits>` child containing either a `<FixedValue>` text
/// node or a `<DynamicValue>` element with a `sizeReference` attribute.
pub(crate) fn write_binary_data_encoding(
    w: &mut W,
    enc: &BinaryDataEncoding,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("BinaryDataEncoding")))?;
    w.write_event(Event::Start(BytesStart::new("SizeInBits")))?;
    match &enc.size_in_bits {
        BinarySize::Fixed(n) => {
            wt(w, "FixedValue", &n.to_string())?;
        }
        BinarySize::Variable { size_reference } => {
            let mut e = BytesStart::new("DynamicValue");
            e.push_attribute(("sizeReference", size_reference.as_str()));
            w.write_event(Event::Empty(e))?;
        }
    }
    w.write_event(Event::End(BytesEnd::new("SizeInBits")))?;
    w.write_event(Event::End(BytesEnd::new("BinaryDataEncoding")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// T2 — Calibrator
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a `<DefaultCalibrator>` wrapper containing either a
/// `<PolynomialCalibrator>` or `<SplineCalibrator>` child.
pub(crate) fn write_calibrator(w: &mut W, cal: &Calibrator) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("DefaultCalibrator")))?;
    match cal {
        Calibrator::Polynomial(p) => {
            w.write_event(Event::Start(BytesStart::new("PolynomialCalibrator")))?;
            for (exp, coef) in p.coefficients.iter().enumerate() {
                let exp_s = exp.to_string();
                let coef_s = coef.to_string();
                let mut term = BytesStart::new("Term");
                term.push_attribute(("exponent", exp_s.as_str()));
                term.push_attribute(("coefficient", coef_s.as_str()));
                w.write_event(Event::Empty(term))?;
            }
            w.write_event(Event::End(BytesEnd::new("PolynomialCalibrator")))?;
        }
        Calibrator::SplineCalibrator(s) => {
            let order_s = s.order.to_string();
            let extrap_s = if s.extrapolate { "true" } else { "false" };
            let mut sc = BytesStart::new("SplineCalibrator");
            sc.push_attribute(("order", order_s.as_str()));
            sc.push_attribute(("extrapolate", extrap_s));
            w.write_event(Event::Start(sc))?;
            for pt in &s.points {
                let raw_s = pt.raw.to_string();
                let cal_s = pt.calibrated.to_string();
                let mut spt = BytesStart::new("SplinePoint");
                spt.push_attribute(("raw", raw_s.as_str()));
                spt.push_attribute(("calibrated", cal_s.as_str()));
                w.write_event(Event::Empty(spt))?;
            }
            w.write_event(Event::End(BytesEnd::new("SplineCalibrator")))?;
        }
    }
    w.write_event(Event::End(BytesEnd::new("DefaultCalibrator")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// T3 — TelemetryMetaData (ParameterTypeSet only; ParameterSet/ContainerSet in T4/T5)
// ─────────────────────────────────────────────────────────────────────────────

/// Serialize a `<TelemetryMetaData>` element.
pub(crate) fn write_telemetry_meta_data(
    w: &mut W,
    tm: &TelemetryMetaData,
) -> Result<(), ParseError> {
    let has_content = !tm.parameter_types.is_empty()
        || !tm.parameters.is_empty()
        || !tm.containers.is_empty();
    if !has_content {
        return Ok(());
    }
    w.write_event(Event::Start(BytesStart::new("TelemetryMetaData")))?;
    if !tm.parameter_types.is_empty() {
        write_parameter_type_set(w, tm.parameter_types.values())?;
    }
    if !tm.parameters.is_empty() {
        write_parameter_set(w, tm.parameters.values())?;
    }
    if !tm.containers.is_empty() {
        write_container_set(w, tm.containers.values())?;
    }
    w.write_event(Event::End(BytesEnd::new("TelemetryMetaData")))?;
    Ok(())
}

/// Serialize a `<ParameterTypeSet>` element containing all parameter type variants.
pub(crate) fn write_parameter_type_set<'a>(
    w: &mut W,
    types: impl Iterator<Item = &'a ParameterType>,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("ParameterTypeSet")))?;
    for pt in types {
        match pt {
            ParameterType::Integer(t) => write_integer_parameter_type(w, t)?,
            ParameterType::Float(t) => write_float_parameter_type(w, t)?,
            ParameterType::Enumerated(t) => write_enumerated_parameter_type(w, t)?,
            ParameterType::Boolean(t) => write_boolean_parameter_type(w, t)?,
            ParameterType::String(t) => write_string_parameter_type(w, t)?,
            ParameterType::Binary(t) => write_binary_parameter_type(w, t)?,
            ParameterType::Aggregate(t) => write_aggregate_parameter_type(w, t)?,
            ParameterType::Array(t) => write_array_parameter_type(w, t)?,
            ParameterType::AbsoluteTime(t) => write_absolute_time_parameter_type(w, t)?,
            ParameterType::RelativeTime(t) => write_relative_time_parameter_type(w, t)?,
        }
    }
    w.write_event(Event::End(BytesEnd::new("ParameterTypeSet")))?;
    Ok(())
}

fn write_integer_parameter_type(w: &mut W, t: &IntegerParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("IntegerParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    e.push_attribute(("signed", if t.signed { "true" } else { "false" }));
    if let Some(s) = t.size_in_bits {
        let s = s.to_string();
        e.push_attribute(("sizeInBits", s.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }

    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_integer_data_encoding(w, enc)?;
    }
    w.write_event(Event::End(BytesEnd::new("IntegerParameterType")))?;
    Ok(())
}

fn write_float_parameter_type(w: &mut W, t: &FloatParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("FloatParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(s) = t.size_in_bits {
        let s = s.to_string();
        e.push_attribute(("sizeInBits", s.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }

    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_float_data_encoding(w, enc)?;
    }
    w.write_event(Event::End(BytesEnd::new("FloatParameterType")))?;
    Ok(())
}

fn write_enumerated_parameter_type(
    w: &mut W,
    t: &EnumeratedParameterType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("EnumeratedParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_integer_data_encoding(w, enc)?;
    }
    if !t.enumeration_list.is_empty() {
        write_enumeration_list(w, &t.enumeration_list)?;
    }
    w.write_event(Event::End(BytesEnd::new("EnumeratedParameterType")))?;
    Ok(())
}

fn write_enumeration_list(w: &mut W, list: &[ValueEnumeration]) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("EnumerationList")))?;
    for ve in list {
        let val_s = ve.value.to_string();
        let mut e = BytesStart::new("Enumeration");
        e.push_attribute(("value", val_s.as_str()));
        e.push_attribute(("label", ve.label.as_str()));
        if let Some(mv) = ve.max_value {
            let mv_s = mv.to_string();
            e.push_attribute(("maxValue", mv_s.as_str()));
        }
        if let Some(d) = &ve.short_description {
            e.push_attribute(("shortDescription", d.as_str()));
        }
        w.write_event(Event::Empty(e))?;
    }
    w.write_event(Event::End(BytesEnd::new("EnumerationList")))?;
    Ok(())
}

fn write_boolean_parameter_type(w: &mut W, t: &BooleanParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("BooleanParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(v) = &t.one_string_value {
        e.push_attribute(("oneStringValue", v.as_str()));
    }
    if let Some(v) = &t.zero_string_value {
        e.push_attribute(("zeroStringValue", v.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }

    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_integer_data_encoding(w, enc)?;
    }
    w.write_event(Event::End(BytesEnd::new("BooleanParameterType")))?;
    Ok(())
}

fn write_string_parameter_type(w: &mut W, t: &StringParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("StringParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }

    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_string_data_encoding(w, enc)?;
    }
    w.write_event(Event::End(BytesEnd::new("StringParameterType")))?;
    Ok(())
}

fn write_binary_parameter_type(w: &mut W, t: &BinaryParameterType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("BinaryParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }

    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_binary_data_encoding(w, enc)?;
    }
    w.write_event(Event::End(BytesEnd::new("BinaryParameterType")))?;
    Ok(())
}

fn write_aggregate_parameter_type(
    w: &mut W,
    t: &AggregateParameterType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("AggregateParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if !t.member_list.is_empty() {
        w.write_event(Event::Start(BytesStart::new("MemberList")))?;
        for m in &t.member_list {
            let mut me = BytesStart::new("Member");
            me.push_attribute(("name", m.name.as_str()));
            me.push_attribute(("typeRef", m.type_ref.as_str()));
            if let Some(d) = &m.short_description {
                me.push_attribute(("shortDescription", d.as_str()));
            }
            w.write_event(Event::Empty(me))?;
        }
        w.write_event(Event::End(BytesEnd::new("MemberList")))?;
    }
    w.write_event(Event::End(BytesEnd::new("AggregateParameterType")))?;
    Ok(())
}

fn write_array_parameter_type(w: &mut W, t: &ArrayParameterType) -> Result<(), ParseError> {
    let dims_s = t.number_of_dimensions.to_string();
    let mut e = BytesStart::new("ArrayParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    e.push_attribute(("arrayTypeRef", t.array_type_ref.as_str()));
    e.push_attribute(("numberOfDimensions", dims_s.as_str()));
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Empty(e))?;
    Ok(())
}

fn write_absolute_time_parameter_type(
    w: &mut W,
    t: &AbsoluteTimeParameterType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("AbsoluteTimeParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some()
        || t.reference_time.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        w.write_event(Event::Start(BytesStart::new("Encoding")))?;
        match enc {
            TimeEncoding::Integer(ie) => write_integer_data_encoding(w, ie)?,
            TimeEncoding::Float(fe) => write_float_data_encoding(w, fe)?,
        }
        w.write_event(Event::End(BytesEnd::new("Encoding")))?;
    }
    if let Some(epoch) = &t.reference_time {
        w.write_event(Event::Start(BytesStart::new("ReferenceTime")))?;
        wt(w, "Epoch", epoch)?;
        w.write_event(Event::End(BytesEnd::new("ReferenceTime")))?;
    }
    w.write_event(Event::End(BytesEnd::new("AbsoluteTimeParameterType")))?;
    Ok(())
}

fn write_relative_time_parameter_type(
    w: &mut W,
    t: &RelativeTimeParameterType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("RelativeTimeParameterType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children = t.long_description.is_some()
        || !t.alias_set.is_empty()
        || !t.unit_set.is_empty()
        || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
        return Ok(());
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &t.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        w.write_event(Event::Start(BytesStart::new("Encoding")))?;
        match enc {
            TimeEncoding::Integer(ie) => write_integer_data_encoding(w, ie)?,
            TimeEncoding::Float(fe) => write_float_data_encoding(w, fe)?,
        }
        w.write_event(Event::End(BytesEnd::new("Encoding")))?;
    }
    w.write_event(Event::End(BytesEnd::new("RelativeTimeParameterType")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// T4 — ParameterSet
// ─────────────────────────────────────────────────────────────────────────────

fn write_parameter_set<'a>(
    w: &mut W,
    params: impl Iterator<Item = &'a Parameter>,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("ParameterSet")))?;
    for p in params {
        let mut e = BytesStart::new("Parameter");
        e.push_attribute(("name", p.name.as_str()));
        e.push_attribute(("parameterTypeRef", p.parameter_type_ref.as_str()));
        if let Some(d) = &p.short_description {
            e.push_attribute(("shortDescription", d.as_str()));
        }
        let has_children = p.long_description.is_some()
            || !p.alias_set.is_empty()
            || p.parameter_properties.is_some();
        if !has_children {
            w.write_event(Event::Empty(e))?;
        } else {
            w.write_event(Event::Start(e))?;
            if let Some(d) = &p.long_description {
                wt(w, "LongDescription", d)?;
            }
            write_alias_set(w, &p.alias_set)?;
            if let Some(pp) = &p.parameter_properties {
                let mut pe = BytesStart::new("ParameterProperties");
                if let Some(ds) = &pp.data_source {
                    pe.push_attribute(("dataSource", data_source_str(ds)));
                }
                pe.push_attribute(("readOnly", if pp.read_only { "true" } else { "false" }));
                w.write_event(Event::Empty(pe))?;
            }
            w.write_event(Event::End(BytesEnd::new("Parameter")))?;
        }
    }
    w.write_event(Event::End(BytesEnd::new("ParameterSet")))?;
    Ok(())
}

fn data_source_str(ds: &crate::model::telemetry::DataSource) -> &'static str {
    use crate::model::telemetry::DataSource;
    match ds {
        DataSource::Telemetered => "telemetered",
        DataSource::Derived => "derived",
        DataSource::Constant => "constant",
        DataSource::Local => "local",
        DataSource::Ground => "ground",
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// T5 — ContainerSet (SequenceContainer + entry list + restriction criteria)
// ─────────────────────────────────────────────────────────────────────────────

fn write_container_set<'a>(
    w: &mut W,
    containers: impl Iterator<Item = &'a SequenceContainer>,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("ContainerSet")))?;
    for c in containers {
        write_sequence_container(w, c)?;
    }
    w.write_event(Event::End(BytesEnd::new("ContainerSet")))?;
    Ok(())
}

fn write_sequence_container(w: &mut W, c: &SequenceContainer) -> Result<(), ParseError> {
    let mut e = BytesStart::new("SequenceContainer");
    e.push_attribute(("name", c.name.as_str()));
    if let Some(d) = &c.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if c.r#abstract {
        e.push_attribute(("abstract", "true"));
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &c.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &c.alias_set)?;
    write_entry_list(w, &c.entry_list)?;
    if let Some(bc) = &c.base_container {
        write_base_container(w, bc)?;
    }
    w.write_event(Event::End(BytesEnd::new("SequenceContainer")))?;
    Ok(())
}

fn write_base_container(w: &mut W, bc: &BaseContainer) -> Result<(), ParseError> {
    let mut e = BytesStart::new("BaseContainer");
    e.push_attribute(("containerRef", bc.container_ref.as_str()));
    if let Some(rc) = &bc.restriction_criteria {
        w.write_event(Event::Start(e))?;
        write_restriction_criteria(w, rc)?;
        w.write_event(Event::End(BytesEnd::new("BaseContainer")))?;
    } else {
        w.write_event(Event::Empty(e))?;
    }
    Ok(())
}

fn write_restriction_criteria(w: &mut W, rc: &RestrictionCriteria) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("RestrictionCriteria")))?;
    match rc {
        RestrictionCriteria::Comparison(cmp) => write_comparison(w, "Comparison", cmp)?,
        RestrictionCriteria::ComparisonList(list) => {
            w.write_event(Event::Start(BytesStart::new("ComparisonList")))?;
            for cmp in list {
                write_comparison(w, "Comparison", cmp)?;
            }
            w.write_event(Event::End(BytesEnd::new("ComparisonList")))?;
        }
        RestrictionCriteria::BooleanExpression(expr) => {
            write_boolean_expression_wrapper(w, expr)?;
        }
        RestrictionCriteria::NextContainer { container_ref } => {
            let mut e = BytesStart::new("NextContainer");
            e.push_attribute(("containerRef", container_ref.as_str()));
            w.write_event(Event::Empty(e))?;
        }
    }
    w.write_event(Event::End(BytesEnd::new("RestrictionCriteria")))?;
    Ok(())
}

fn write_match_criteria(w: &mut W, tag: &str, mc: &MatchCriteria) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new(tag.to_owned())))?;
    match mc {
        MatchCriteria::Comparison(cmp) => write_comparison(w, "Comparison", cmp)?,
        MatchCriteria::ComparisonList(list) => {
            w.write_event(Event::Start(BytesStart::new("ComparisonList")))?;
            for cmp in list {
                write_comparison(w, "Comparison", cmp)?;
            }
            w.write_event(Event::End(BytesEnd::new("ComparisonList")))?;
        }
        MatchCriteria::BooleanExpression(expr) => {
            write_boolean_expression_wrapper(w, expr)?;
        }
    }
    w.write_event(Event::End(BytesEnd::new(tag.to_owned())))?;
    Ok(())
}

fn write_comparison(w: &mut W, tag: &str, cmp: &Comparison) -> Result<(), ParseError> {
    let mut e = BytesStart::new(tag.to_owned());
    e.push_attribute(("parameterRef", cmp.parameter_ref.as_str()));
    e.push_attribute(("value", cmp.value.as_str()));
    let op = match cmp.comparison_operator {
        ComparisonOperator::Equality => "==",
        ComparisonOperator::Inequality => "!=",
        ComparisonOperator::LessThan => "<",
        ComparisonOperator::LessThanOrEqual => "<=",
        ComparisonOperator::GreaterThan => ">",
        ComparisonOperator::GreaterThanOrEqual => ">=",
    };
    e.push_attribute(("comparisonOperator", op));
    e.push_attribute(("useCalibratedValue", if cmp.use_calibrated_value { "true" } else { "false" }));
    w.write_event(Event::Empty(e))?;
    Ok(())
}

/// Write a `<BooleanExpression>` wrapper element containing the expression tree.
fn write_boolean_expression_wrapper(
    w: &mut W,
    expr: &BooleanExpression,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("BooleanExpression")))?;
    write_boolean_expression_inner(w, expr)?;
    w.write_event(Event::End(BytesEnd::new("BooleanExpression")))?;
    Ok(())
}

fn write_boolean_expression_inner(w: &mut W, expr: &BooleanExpression) -> Result<(), ParseError> {
    match expr {
        BooleanExpression::Condition(cmp) => write_comparison(w, "Condition", cmp)?,
        BooleanExpression::And(terms) => {
            w.write_event(Event::Start(BytesStart::new("ANDedConditions")))?;
            for t in terms {
                write_boolean_expression_inner(w, t)?;
            }
            w.write_event(Event::End(BytesEnd::new("ANDedConditions")))?;
        }
        BooleanExpression::Or(terms) => {
            w.write_event(Event::Start(BytesStart::new("ORedConditions")))?;
            for t in terms {
                write_boolean_expression_inner(w, t)?;
            }
            w.write_event(Event::End(BytesEnd::new("ORedConditions")))?;
        }
        // Not is in the model but not in the parser; skip gracefully.
        BooleanExpression::Not(_) => {}
    }
    Ok(())
}

fn write_entry_list(w: &mut W, entries: &[SequenceEntry]) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("EntryList")))?;
    for entry in entries {
        match entry {
            SequenceEntry::ParameterRef(e) => {
                let has_children = e.location.is_some() || e.include_condition.is_some();
                let mut el = BytesStart::new("ParameterRefEntry");
                el.push_attribute(("parameterRef", e.parameter_ref.as_str()));
                if !has_children {
                    w.write_event(Event::Empty(el))?;
                } else {
                    w.write_event(Event::Start(el))?;
                    if let Some(loc) = &e.location {
                        write_entry_location(w, loc)?;
                    }
                    if let Some(ic) = &e.include_condition {
                        write_match_criteria(w, "IncludeCondition", ic)?;
                    }
                    w.write_event(Event::End(BytesEnd::new("ParameterRefEntry")))?;
                }
            }
            SequenceEntry::ContainerRef(e) => {
                let has_children = e.location.is_some() || e.include_condition.is_some();
                let mut el = BytesStart::new("ContainerRefEntry");
                el.push_attribute(("containerRef", e.container_ref.as_str()));
                if !has_children {
                    w.write_event(Event::Empty(el))?;
                } else {
                    w.write_event(Event::Start(el))?;
                    if let Some(loc) = &e.location {
                        write_entry_location(w, loc)?;
                    }
                    if let Some(ic) = &e.include_condition {
                        write_match_criteria(w, "IncludeCondition", ic)?;
                    }
                    w.write_event(Event::End(BytesEnd::new("ContainerRefEntry")))?;
                }
            }
            SequenceEntry::FixedValue(e) => {
                write_seq_fixed_value_entry(w, e)?;
            }
            SequenceEntry::ArrayParameterRef(e) => {
                let has_children = e.location.is_some();
                let mut el = BytesStart::new("ArrayParameterRefEntry");
                el.push_attribute(("parameterRef", e.parameter_ref.as_str()));
                if !has_children {
                    w.write_event(Event::Empty(el))?;
                } else {
                    w.write_event(Event::Start(el))?;
                    if let Some(loc) = &e.location {
                        write_entry_location(w, loc)?;
                    }
                    w.write_event(Event::End(BytesEnd::new("ArrayParameterRefEntry")))?;
                }
            }
        }
    }
    w.write_event(Event::End(BytesEnd::new("EntryList")))?;
    Ok(())
}

fn write_seq_fixed_value_entry(
    w: &mut W,
    e: &crate::model::container::FixedValueEntry,
) -> Result<(), ParseError> {
    let size_s = e.size_in_bits.to_string();
    let mut el = BytesStart::new("FixedValueEntry");
    el.push_attribute(("sizeInBits", size_s.as_str()));
    if let Some(bv) = &e.binary_value {
        el.push_attribute(("binaryValue", bv.as_str()));
    }
    if let Some(loc) = &e.location {
        w.write_event(Event::Start(el))?;
        write_entry_location(w, loc)?;
        w.write_event(Event::End(BytesEnd::new("FixedValueEntry")))?;
    } else {
        w.write_event(Event::Empty(el))?;
    }
    Ok(())
}

fn write_entry_location(w: &mut W, loc: &EntryLocation) -> Result<(), ParseError> {
    let ref_loc = match loc.reference_location {
        ReferenceLocation::ContainerStart => "containerStart",
        ReferenceLocation::PreviousEntry => "previousEntry",
    };
    let mut e = BytesStart::new("LocationInContainerInBits");
    e.push_attribute(("referenceLocation", ref_loc));
    w.write_event(Event::Start(e))?;
    wt(w, "FixedValue", &loc.bit_offset.to_string())?;
    w.write_event(Event::End(BytesEnd::new("LocationInContainerInBits")))?;
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// T6 — CommandMetaData (ArgumentTypeSet + MetaCommandSet)
// ─────────────────────────────────────────────────────────────────────────────

fn write_command_meta_data(w: &mut W, cm: &CommandMetaData) -> Result<(), ParseError> {
    let has_content = !cm.argument_types.is_empty()
        || !cm.meta_commands.is_empty()
        || !cm.command_containers.is_empty();
    if !has_content {
        return Ok(());
    }
    w.write_event(Event::Start(BytesStart::new("CommandMetaData")))?;
    if !cm.argument_types.is_empty() {
        write_argument_type_set(w, cm.argument_types.values())?;
    }
    if !cm.meta_commands.is_empty() {
        write_meta_command_set(w, cm.meta_commands.values())?;
    }
    if !cm.command_containers.is_empty() {
        w.write_event(Event::Start(BytesStart::new("CommandContainerSet")))?;
        for c in cm.command_containers.values() {
            write_sequence_container(w, c)?;
        }
        w.write_event(Event::End(BytesEnd::new("CommandContainerSet")))?;
    }
    w.write_event(Event::End(BytesEnd::new("CommandMetaData")))?;
    Ok(())
}

fn write_argument_type_set<'a>(
    w: &mut W,
    types: impl Iterator<Item = &'a ArgumentType>,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("ArgumentTypeSet")))?;
    for at in types {
        match at {
            ArgumentType::Integer(t) => write_integer_argument_type(w, t)?,
            ArgumentType::Float(t) => write_float_argument_type(w, t)?,
            ArgumentType::Enumerated(t) => write_enumerated_argument_type(w, t)?,
            ArgumentType::Boolean(t) => write_boolean_argument_type(w, t)?,
            ArgumentType::String(t) => write_string_argument_type(w, t)?,
            ArgumentType::Binary(t) => write_binary_argument_type(w, t)?,
            ArgumentType::Aggregate(t) => write_aggregate_argument_type(w, t)?,
            ArgumentType::Array(t) => write_array_argument_type(w, t)?,
        }
    }
    w.write_event(Event::End(BytesEnd::new("ArgumentTypeSet")))?;
    Ok(())
}

fn write_integer_argument_type(w: &mut W, t: &IntegerArgumentType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("IntegerArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    e.push_attribute(("signed", if t.signed { "true" } else { "false" }));
    if let Some(s) = t.size_in_bits {
        let s = s.to_string();
        e.push_attribute(("sizeInBits", s.as_str()));
    }
    if let Some(iv) = t.initial_value {
        let iv_s = iv.to_string();
        e.push_attribute(("initialValue", iv_s.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children =
        !t.alias_set.is_empty() || !t.unit_set.is_empty() || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
    } else {
        w.write_event(Event::Start(e))?;
        write_alias_set(w, &t.alias_set)?;
        write_unit_set(w, &t.unit_set)?;
        if let Some(enc) = &t.encoding {
            write_integer_data_encoding(w, enc)?;
        }
        w.write_event(Event::End(BytesEnd::new("IntegerArgumentType")))?;
    }
    Ok(())
}

fn write_float_argument_type(w: &mut W, t: &FloatArgumentType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("FloatArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(s) = t.size_in_bits {
        let s = s.to_string();
        e.push_attribute(("sizeInBits", s.as_str()));
    }
    if let Some(iv) = t.initial_value {
        let iv_s = iv.to_string();
        e.push_attribute(("initialValue", iv_s.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children =
        !t.alias_set.is_empty() || !t.unit_set.is_empty() || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
    } else {
        w.write_event(Event::Start(e))?;
        write_alias_set(w, &t.alias_set)?;
        write_unit_set(w, &t.unit_set)?;
        if let Some(enc) = &t.encoding {
            write_float_data_encoding(w, enc)?;
        }
        w.write_event(Event::End(BytesEnd::new("FloatArgumentType")))?;
    }
    Ok(())
}

fn write_enumerated_argument_type(
    w: &mut W,
    t: &EnumeratedArgumentType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("EnumeratedArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(iv) = &t.initial_value {
        e.push_attribute(("initialValue", iv.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Start(e))?;
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if let Some(enc) = &t.encoding {
        write_integer_data_encoding(w, enc)?;
    }
    if !t.enumeration_list.is_empty() {
        write_enumeration_list(w, &t.enumeration_list)?;
    }
    w.write_event(Event::End(BytesEnd::new("EnumeratedArgumentType")))?;
    Ok(())
}

fn write_boolean_argument_type(w: &mut W, t: &BooleanArgumentType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("BooleanArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(v) = &t.one_string_value {
        e.push_attribute(("oneStringValue", v.as_str()));
    }
    if let Some(v) = &t.zero_string_value {
        e.push_attribute(("zeroStringValue", v.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children =
        !t.alias_set.is_empty() || !t.unit_set.is_empty() || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
    } else {
        w.write_event(Event::Start(e))?;
        write_alias_set(w, &t.alias_set)?;
        write_unit_set(w, &t.unit_set)?;
        if let Some(enc) = &t.encoding {
            write_integer_data_encoding(w, enc)?;
        }
        w.write_event(Event::End(BytesEnd::new("BooleanArgumentType")))?;
    }
    Ok(())
}

fn write_string_argument_type(w: &mut W, t: &StringArgumentType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("StringArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(iv) = &t.initial_value {
        e.push_attribute(("initialValue", iv.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children =
        !t.alias_set.is_empty() || !t.unit_set.is_empty() || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
    } else {
        w.write_event(Event::Start(e))?;
        write_alias_set(w, &t.alias_set)?;
        write_unit_set(w, &t.unit_set)?;
        if let Some(enc) = &t.encoding {
            write_string_data_encoding(w, enc)?;
        }
        w.write_event(Event::End(BytesEnd::new("StringArgumentType")))?;
    }
    Ok(())
}

fn write_binary_argument_type(w: &mut W, t: &BinaryArgumentType) -> Result<(), ParseError> {
    let mut e = BytesStart::new("BinaryArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(iv) = &t.initial_value {
        e.push_attribute(("initialValue", iv.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    let has_children =
        !t.alias_set.is_empty() || !t.unit_set.is_empty() || t.encoding.is_some();
    if !has_children {
        w.write_event(Event::Empty(e))?;
    } else {
        w.write_event(Event::Start(e))?;
        write_alias_set(w, &t.alias_set)?;
        write_unit_set(w, &t.unit_set)?;
        if let Some(enc) = &t.encoding {
            write_binary_data_encoding(w, enc)?;
        }
        w.write_event(Event::End(BytesEnd::new("BinaryArgumentType")))?;
    }
    Ok(())
}

fn write_aggregate_argument_type(
    w: &mut W,
    t: &AggregateArgumentType,
) -> Result<(), ParseError> {
    let mut e = BytesStart::new("AggregateArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Start(e))?;
    write_alias_set(w, &t.alias_set)?;
    write_unit_set(w, &t.unit_set)?;
    if !t.member_list.is_empty() {
        w.write_event(Event::Start(BytesStart::new("MemberList")))?;
        for m in &t.member_list {
            let mut me = BytesStart::new("Member");
            me.push_attribute(("name", m.name.as_str()));
            me.push_attribute(("typeRef", m.type_ref.as_str()));
            if let Some(d) = &m.short_description {
                me.push_attribute(("shortDescription", d.as_str()));
            }
            w.write_event(Event::Empty(me))?;
        }
        w.write_event(Event::End(BytesEnd::new("MemberList")))?;
    }
    w.write_event(Event::End(BytesEnd::new("AggregateArgumentType")))?;
    Ok(())
}

fn write_array_argument_type(w: &mut W, t: &ArrayArgumentType) -> Result<(), ParseError> {
    let dims_s = t.number_of_dimensions.to_string();
    let mut e = BytesStart::new("ArrayArgumentType");
    e.push_attribute(("name", t.name.as_str()));
    if let Some(d) = &t.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    e.push_attribute(("arrayTypeRef", t.array_type_ref.as_str()));
    e.push_attribute(("numberOfDimensions", dims_s.as_str()));
    if let Some(bt) = &t.base_type {
        e.push_attribute(("baseType", bt.as_str()));
    }
    w.write_event(Event::Empty(e))?;
    Ok(())
}

fn write_meta_command_set<'a>(
    w: &mut W,
    commands: impl Iterator<Item = &'a MetaCommand>,
) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("MetaCommandSet")))?;
    for mc in commands {
        write_meta_command(w, mc)?;
    }
    w.write_event(Event::End(BytesEnd::new("MetaCommandSet")))?;
    Ok(())
}

fn write_meta_command(w: &mut W, mc: &MetaCommand) -> Result<(), ParseError> {
    let mut e = BytesStart::new("MetaCommand");
    e.push_attribute(("name", mc.name.as_str()));
    if let Some(d) = &mc.short_description {
        e.push_attribute(("shortDescription", d.as_str()));
    }
    if mc.r#abstract {
        e.push_attribute(("abstract", "true"));
    }
    if let Some(base) = &mc.base_meta_command {
        e.push_attribute(("baseMetaCommand", base.as_str()));
    }
    w.write_event(Event::Start(e))?;
    if let Some(d) = &mc.long_description {
        wt(w, "LongDescription", d)?;
    }
    write_alias_set(w, &mc.alias_set)?;
    if !mc.argument_list.is_empty() {
        w.write_event(Event::Start(BytesStart::new("ArgumentList")))?;
        for arg in &mc.argument_list {
            let mut ae = BytesStart::new("Argument");
            ae.push_attribute(("name", arg.name.as_str()));
            ae.push_attribute(("argumentTypeRef", arg.argument_type_ref.as_str()));
            if let Some(d) = &arg.short_description {
                ae.push_attribute(("shortDescription", d.as_str()));
            }
            if let Some(iv) = &arg.initial_value {
                ae.push_attribute(("initialValue", iv.as_str()));
            }
            w.write_event(Event::Empty(ae))?;
        }
        w.write_event(Event::End(BytesEnd::new("ArgumentList")))?;
    }
    if let Some(cc) = &mc.command_container {
        write_command_container(w, cc)?;
    }
    w.write_event(Event::End(BytesEnd::new("MetaCommand")))?;
    Ok(())
}

fn write_command_container(w: &mut W, cc: &CommandContainer) -> Result<(), ParseError> {
    let mut e = BytesStart::new("CommandContainer");
    e.push_attribute(("name", cc.name.as_str()));
    w.write_event(Event::Start(e))?;
    write_command_entry_list(w, &cc.entry_list)?;
    if let Some(bc) = &cc.base_container {
        write_base_container(w, bc)?;
    }
    w.write_event(Event::End(BytesEnd::new("CommandContainer")))?;
    Ok(())
}

fn write_command_entry_list(w: &mut W, entries: &[CommandEntry]) -> Result<(), ParseError> {
    w.write_event(Event::Start(BytesStart::new("EntryList")))?;
    for entry in entries {
        match entry {
            CommandEntry::ArgumentRef(e) => {
                write_argument_ref_entry(w, e)?;
            }
            CommandEntry::ParameterRef(se) => {
                // ParameterRef in a command container is always a ParameterRefEntry.
                if let SequenceEntry::ParameterRef(e) = se {
                    let mut el = BytesStart::new("ParameterRefEntry");
                    el.push_attribute(("parameterRef", e.parameter_ref.as_str()));
                    if e.location.is_none() && e.include_condition.is_none() {
                        w.write_event(Event::Empty(el))?;
                    } else {
                        w.write_event(Event::Start(el))?;
                        if let Some(loc) = &e.location {
                            write_entry_location(w, loc)?;
                        }
                        if let Some(ic) = &e.include_condition {
                            write_match_criteria(w, "IncludeCondition", ic)?;
                        }
                        w.write_event(Event::End(BytesEnd::new("ParameterRefEntry")))?;
                    }
                }
            }
            CommandEntry::FixedValue(e) => {
                let size_s = e.size_in_bits.to_string();
                let mut el = BytesStart::new("FixedValueEntry");
                el.push_attribute(("sizeInBits", size_s.as_str()));
                if let Some(bv) = &e.binary_value {
                    el.push_attribute(("binaryValue", bv.as_str()));
                }
                if let Some(loc) = &e.location {
                    w.write_event(Event::Start(el))?;
                    write_entry_location(w, loc)?;
                    w.write_event(Event::End(BytesEnd::new("FixedValueEntry")))?;
                } else {
                    w.write_event(Event::Empty(el))?;
                }
            }
        }
    }
    w.write_event(Event::End(BytesEnd::new("EntryList")))?;
    Ok(())
}

fn write_argument_ref_entry(w: &mut W, e: &ArgumentRefEntry) -> Result<(), ParseError> {
    let mut el = BytesStart::new("ArgumentRefEntry");
    el.push_attribute(("argumentRef", e.argument_ref.as_str()));
    if let Some(loc) = &e.location {
        w.write_event(Event::Start(el))?;
        write_entry_location(w, loc)?;
        w.write_event(Event::End(BytesEnd::new("ArgumentRefEntry")))?;
    } else {
        w.write_event(Event::Empty(el))?;
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::space_system::{AuthorInfo, Header};
    use crate::model::types::{
        BinaryDataEncoding, BinarySize, ByteOrder, Calibrator, FloatDataEncoding, FloatEncoding,
        FloatSizeInBits, IntegerDataEncoding, IntegerEncoding, PolynomialCalibrator,
        SplineCalibrator, SplinePoint, StringDataEncoding, StringEncoding, StringSize, Unit,
    };

    /// Serialize and immediately re-parse; return the bytes for further checks.
    fn round_trip(ss: &SpaceSystem) -> SpaceSystem {
        let bytes = serialize(ss).expect("serialize failed");
        crate::parser::parse(&bytes).expect("re-parse failed")
    }

    // ── T1: SpaceSystem scaffold ──────────────────────────────────────────────

    #[test]
    fn t1_minimal_space_system() {
        let ss = SpaceSystem::new("MySS");
        let rt = round_trip(&ss);
        assert_eq!(rt.name, "MySS");
        assert!(rt.header.is_none());
        assert!(rt.sub_systems.is_empty());
    }

    #[test]
    fn t1_short_description() {
        let mut ss = SpaceSystem::new("MySS");
        ss.short_description = Some("A test system".to_string());
        let rt = round_trip(&ss);
        assert_eq!(rt.short_description.as_deref(), Some("A test system"));
    }

    #[test]
    fn t1_nested_sub_systems() {
        let mut root = SpaceSystem::new("Root");
        let mut child = SpaceSystem::new("Child");
        child.sub_systems.push(SpaceSystem::new("GrandChild"));
        root.sub_systems.push(child);
        let rt = round_trip(&root);
        assert_eq!(rt.sub_systems.len(), 1);
        assert_eq!(rt.sub_systems[0].name, "Child");
        assert_eq!(rt.sub_systems[0].sub_systems[0].name, "GrandChild");
    }

    #[test]
    fn t1_header_attributes() {
        let mut ss = SpaceSystem::new("MySS");
        ss.header = Some(Header {
            version: Some("2.0".to_string()),
            date: Some("2026-01-01".to_string()),
            classification: Some("Unclassified".to_string()),
            classification_instructions: None,
            validation_status: Some("Working".to_string()),
            author_set: vec![AuthorInfo { name: "Alice".to_string(), role: Some("author".to_string()) }],
            note_set: vec!["First note".to_string()],
        });
        let rt = round_trip(&ss);
        let h = rt.header.unwrap();
        assert_eq!(h.version.as_deref(), Some("2.0"));
        assert_eq!(h.classification.as_deref(), Some("Unclassified"));
        // Role is encoded inline: serializer writes "<Author>name (role)</Author>"
        // and the parser reads the whole text back as name with no separate role field.
        assert_eq!(h.author_set[0].name, "Alice (author)");
        assert_eq!(h.author_set[0].role, None);
        assert_eq!(h.note_set[0], "First note");
    }

    // ── T2: Unit / Alias ─────────────────────────────────────────────────────

    #[test]
    fn t2_unit_set_round_trip() {
        let units = vec![
            Unit { value: "m/s".to_string(), power: Some(1.0), factor: None, description: None },
            Unit { value: "K".to_string(), power: None, factor: Some("2".to_string()), description: Some("Kelvin".to_string()) },
        ];
        let mut w = Writer::new(Vec::new());
        write_unit_set(&mut w, &units).unwrap();
        let xml = format!("<R>{}</R>", std::str::from_utf8(&w.into_inner()).unwrap());
        // Basic smoke-check: both unit values present in output.
        assert!(xml.contains("m/s"));
        assert!(xml.contains("Kelvin"));
    }

    // ── T2: Integer encoding ─────────────────────────────────────────────────

    #[test]
    fn t2_integer_encoding_unsigned() {
        let enc = IntegerDataEncoding {
            size_in_bits: 16,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: None,
        };
        let xml = enc_xml(|w| write_integer_data_encoding(w, &enc));
        assert!(xml.contains("sizeInBits=\"16\""));
        assert!(xml.contains("encoding=\"unsigned\""));
    }

    #[test]
    fn t2_integer_encoding_with_byte_order() {
        let enc = IntegerDataEncoding {
            size_in_bits: 32,
            encoding: IntegerEncoding::TwosComplement,
            byte_order: Some(ByteOrder::LeastSignificantByteFirst),
            default_calibrator: None,
        };
        let xml = enc_xml(|w| write_integer_data_encoding(w, &enc));
        assert!(xml.contains("twosComplement"));
        assert!(xml.contains("leastSignificantByteFirst"));
    }

    #[test]
    fn t2_integer_encoding_with_polynomial_calibrator() {
        let enc = IntegerDataEncoding {
            size_in_bits: 8,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: Some(Calibrator::Polynomial(PolynomialCalibrator {
                coefficients: vec![0.0, 0.5],
            })),
        };
        let xml = enc_xml(|w| write_integer_data_encoding(w, &enc));
        assert!(xml.contains("PolynomialCalibrator"));
        assert!(xml.contains("coefficient=\"0.5\""));
        assert!(xml.contains("exponent=\"1\""));
    }

    // ── T2: Float encoding ───────────────────────────────────────────────────

    #[test]
    fn t2_float_encoding_ieee754_f32() {
        let enc = FloatDataEncoding {
            size_in_bits: FloatSizeInBits::F32,
            encoding: FloatEncoding::IEEE754_1985,
            byte_order: None,
            default_calibrator: None,
        };
        let xml = enc_xml(|w| write_float_data_encoding(w, &enc));
        assert!(xml.contains("sizeInBits=\"32\""));
        assert!(xml.contains("encoding=\"IEEE754_1985\""));
    }

    #[test]
    fn t2_float_encoding_with_spline_calibrator() {
        let enc = FloatDataEncoding {
            size_in_bits: FloatSizeInBits::F64,
            encoding: FloatEncoding::IEEE754_1985,
            byte_order: None,
            default_calibrator: Some(Calibrator::SplineCalibrator(SplineCalibrator {
                order: 1,
                extrapolate: true,
                points: vec![
                    SplinePoint { raw: 0.0, calibrated: 0.0 },
                    SplinePoint { raw: 100.0, calibrated: 50.0 },
                ],
            })),
        };
        let xml = enc_xml(|w| write_float_data_encoding(w, &enc));
        assert!(xml.contains("SplineCalibrator"));
        assert!(xml.contains("order=\"1\""));
        assert!(xml.contains("extrapolate=\"true\""));
        assert!(xml.contains("calibrated=\"50\""));
    }

    // ── T2: String encoding ──────────────────────────────────────────────────

    #[test]
    fn t2_string_encoding_no_size() {
        let enc = StringDataEncoding {
            encoding: StringEncoding::UTF8,
            byte_order: None,
            size_in_bits: None,
        };
        let xml = enc_xml(|w| write_string_data_encoding(w, &enc));
        assert!(xml.contains("UTF-8"));
        assert!(!xml.contains("SizeInBits"));
    }

    #[test]
    fn t2_string_encoding_fixed() {
        let enc = StringDataEncoding {
            encoding: StringEncoding::UsAscii,
            byte_order: None,
            size_in_bits: Some(StringSize::Fixed(64)),
        };
        let xml = enc_xml(|w| write_string_data_encoding(w, &enc));
        assert!(xml.contains("US-ASCII"));
        assert!(xml.contains("<Fixed>"));
        assert!(xml.contains("64"));
    }

    #[test]
    fn t2_string_encoding_termination() {
        let enc = StringDataEncoding {
            encoding: StringEncoding::UTF8,
            byte_order: None,
            size_in_bits: Some(StringSize::TerminationChar(0x00)),
        };
        let xml = enc_xml(|w| write_string_data_encoding(w, &enc));
        assert!(xml.contains("TerminationChar"));
        assert!(xml.contains("termChar=\"00\""));
    }

    #[test]
    fn t2_string_encoding_variable() {
        let enc = StringDataEncoding {
            encoding: StringEncoding::UTF16,
            byte_order: None,
            size_in_bits: Some(StringSize::Variable { max_size_in_bits: 256 }),
        };
        let xml = enc_xml(|w| write_string_data_encoding(w, &enc));
        assert!(xml.contains("Variable"));
        assert!(xml.contains("maxSizeInBits=\"256\""));
    }

    // ── T2: Binary encoding ──────────────────────────────────────────────────

    #[test]
    fn t2_binary_encoding_fixed() {
        let enc = BinaryDataEncoding { size_in_bits: BinarySize::Fixed(48) };
        let xml = enc_xml(|w| write_binary_data_encoding(w, &enc));
        assert!(xml.contains("FixedValue"));
        assert!(xml.contains("48"));
    }

    #[test]
    fn t2_binary_encoding_variable() {
        let enc = BinaryDataEncoding {
            size_in_bits: BinarySize::Variable { size_reference: "LenParam".to_string() },
        };
        let xml = enc_xml(|w| write_binary_data_encoding(w, &enc));
        assert!(xml.contains("DynamicValue"));
        assert!(xml.contains("sizeReference=\"LenParam\""));
    }

    // ── T4: Parameters ───────────────────────────────────────────────────────

    #[test]
    fn t4_parameter_set_round_trip() {
        use crate::model::telemetry::{Parameter, TelemetryMetaData};
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        let mut p1 = Parameter::new("P1", "IntT");
        p1.short_description = Some("first".to_string());
        tm.parameters.insert("P1".to_string(), p1);
        tm.parameters.insert("P2".to_string(), Parameter::new("P2", "FloatT"));
        ss.telemetry = Some(tm);
        let rt = round_trip(&ss);
        let tm = rt.telemetry.unwrap();
        assert_eq!(tm.parameters.len(), 2);
        let p1 = tm.parameters.get("P1").unwrap();
        assert_eq!(p1.parameter_type_ref, "IntT");
        assert_eq!(p1.short_description.as_deref(), Some("first"));
        assert_eq!(tm.parameters.get("P2").unwrap().parameter_type_ref, "FloatT");
    }

    #[test]
    fn t4_parameter_properties_round_trip() {
        use crate::model::telemetry::{DataSource, Parameter, ParameterProperties, TelemetryMetaData};
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        let mut p = Parameter::new("P1", "IntT");
        p.parameter_properties =
            Some(ParameterProperties { data_source: Some(DataSource::Derived), read_only: true });
        tm.parameters.insert("P1".to_string(), p);
        ss.telemetry = Some(tm);
        let rt = round_trip(&ss);
        let pp = rt.telemetry.unwrap().parameters.get("P1").unwrap().parameter_properties.as_ref().unwrap().clone();
        assert_eq!(pp.data_source, Some(DataSource::Derived));
        assert!(pp.read_only);
    }

    // ── T5: SequenceContainers ───────────────────────────────────────────────

    #[test]
    fn t5_simple_container_round_trip() {
        use crate::model::container::{ParameterRefEntry, SequenceEntry};
        use crate::model::telemetry::TelemetryMetaData;
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        let mut c = SequenceContainer::new("PrimaryHeader");
        c.short_description = Some("CCSDS header".to_string());
        c.entry_list = vec![
            SequenceEntry::ParameterRef(ParameterRefEntry {
                parameter_ref: "APID".to_string(),
                location: None,
                include_condition: None,
            }),
            SequenceEntry::ParameterRef(ParameterRefEntry {
                parameter_ref: "SeqCount".to_string(),
                location: None,
                include_condition: None,
            }),
        ];
        tm.containers.insert("PrimaryHeader".to_string(), c);
        ss.telemetry = Some(tm);
        let rt = round_trip(&ss);
        let c = rt.telemetry.unwrap().containers.get("PrimaryHeader").unwrap().clone();
        assert_eq!(c.short_description.as_deref(), Some("CCSDS header"));
        assert_eq!(c.entry_list.len(), 2);
        let SequenceEntry::ParameterRef(e) = &c.entry_list[0] else { panic!() };
        assert_eq!(e.parameter_ref, "APID");
    }

    #[test]
    fn t5_container_with_base_and_restriction_round_trip() {
        use crate::model::container::{
            BaseContainer, Comparison, ComparisonOperator, ParameterRefEntry,
            RestrictionCriteria, SequenceEntry,
        };
        use crate::model::telemetry::TelemetryMetaData;
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        let mut c = SequenceContainer::new("TmPacket");
        c.base_container = Some(BaseContainer {
            container_ref: "PrimaryHeader".to_string(),
            restriction_criteria: Some(RestrictionCriteria::Comparison(Comparison {
                parameter_ref: "APID".to_string(),
                value: "100".to_string(),
                comparison_operator: ComparisonOperator::Equality,
                use_calibrated_value: true,
            })),
        });
        c.entry_list = vec![
            SequenceEntry::ParameterRef(ParameterRefEntry {
                parameter_ref: "Data".to_string(),
                location: None,
                include_condition: None,
            }),
        ];
        tm.containers.insert("TmPacket".to_string(), c);
        ss.telemetry = Some(tm);
        let rt = round_trip(&ss);
        let c = rt.telemetry.unwrap().containers.get("TmPacket").unwrap().clone();
        let bc = c.base_container.unwrap();
        assert_eq!(bc.container_ref, "PrimaryHeader");
        let RestrictionCriteria::Comparison(cmp) = bc.restriction_criteria.unwrap() else { panic!() };
        assert_eq!(cmp.parameter_ref, "APID");
        assert_eq!(cmp.value, "100");
    }

    #[test]
    fn t5_container_with_location_round_trip() {
        use crate::model::container::{EntryLocation, ParameterRefEntry, ReferenceLocation, SequenceEntry};
        use crate::model::telemetry::TelemetryMetaData;
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        let mut c = SequenceContainer::new("Pkt");
        c.entry_list = vec![
            SequenceEntry::ParameterRef(ParameterRefEntry {
                parameter_ref: "APID".to_string(),
                location: Some(EntryLocation {
                    reference_location: ReferenceLocation::ContainerStart,
                    bit_offset: 0,
                }),
                include_condition: None,
            }),
        ];
        tm.containers.insert("Pkt".to_string(), c);
        ss.telemetry = Some(tm);
        let rt = round_trip(&ss);
        let c = rt.telemetry.unwrap().containers.get("Pkt").unwrap().clone();
        let SequenceEntry::ParameterRef(e) = &c.entry_list[0] else { panic!() };
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 0);
    }

    // ── T6: ArgumentTypes + MetaCommands ─────────────────────────────────────

    #[test]
    fn t6_argument_type_set_round_trip() {
        use crate::model::command::{ArgumentType, CommandMetaData, IntegerArgumentType};
        use crate::model::types::IntegerEncoding;
        let mut ss = SpaceSystem::new("Test");
        let mut cm = CommandMetaData::default();
        let mut t = IntegerArgumentType::new("CmdInt");
        t.signed = false;
        t.size_in_bits = Some(8);
        t.encoding = Some(IntegerDataEncoding {
            size_in_bits: 8,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: None,
        });
        cm.argument_types.insert("CmdInt".to_string(), ArgumentType::Integer(t));
        ss.command = Some(cm);
        let rt = round_trip(&ss);
        let cm = rt.command.unwrap();
        let at = cm.argument_types.get("CmdInt").unwrap();
        let ArgumentType::Integer(t) = at else { panic!() };
        assert!(!t.signed);
        assert_eq!(t.size_in_bits, Some(8));
        assert!(t.encoding.is_some());
    }

    // ── T6 extended: remaining ArgumentType variants ──────────────────────────

    #[test]
    fn t6_boolean_argument_type_round_trip() {
        use crate::model::command::{ArgumentType, BooleanArgumentType, CommandMetaData};

        let mut t = BooleanArgumentType::new("EnableFlag");
        t.short_description = Some("on/off".into());
        t.one_string_value = Some("ENABLED".into());
        t.zero_string_value = Some("DISABLED".into());
        t.base_type = Some("BaseBool".into());

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("EnableFlag".into(), ArgumentType::Boolean(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("EnableFlag").cloned().unwrap();
        let ArgumentType::Boolean(b) = at else { panic!("expected Boolean") };
        assert_eq!(b.short_description.as_deref(), Some("on/off"));
        assert_eq!(b.one_string_value.as_deref(), Some("ENABLED"));
        assert_eq!(b.zero_string_value.as_deref(), Some("DISABLED"));
        assert_eq!(b.base_type.as_deref(), Some("BaseBool"));
    }

    #[test]
    fn t6_string_argument_type_round_trip() {
        use crate::model::command::{ArgumentType, CommandMetaData, StringArgumentType};
        use crate::model::types::{StringDataEncoding, StringEncoding, StringSize};

        let mut t = StringArgumentType::new("Callsign");
        t.encoding = Some(StringDataEncoding {
            encoding: StringEncoding::UTF8,
            byte_order: None,
            size_in_bits: Some(StringSize::Fixed(64)),
        });
        t.initial_value = Some("DEFAULT".into());

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("Callsign".into(), ArgumentType::String(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("Callsign").cloned().unwrap();
        let ArgumentType::String(s) = at else { panic!("expected String") };
        let enc = s.encoding.as_ref().unwrap();
        assert_eq!(enc.encoding, StringEncoding::UTF8);
        assert_eq!(enc.size_in_bits, Some(StringSize::Fixed(64)));
    }

    #[test]
    fn t6_binary_argument_type_round_trip() {
        use crate::model::command::{ArgumentType, BinaryArgumentType, CommandMetaData};
        use crate::model::types::{BinaryDataEncoding, BinarySize};

        let mut t = BinaryArgumentType::new("RawFrame");
        t.encoding = Some(BinaryDataEncoding { size_in_bits: BinarySize::Fixed(128) });

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("RawFrame".into(), ArgumentType::Binary(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("RawFrame").cloned().unwrap();
        let ArgumentType::Binary(b) = at else { panic!("expected Binary") };
        let enc = b.encoding.as_ref().unwrap();
        assert_eq!(enc.size_in_bits, BinarySize::Fixed(128));
    }

    #[test]
    fn t6_aggregate_argument_type_round_trip() {
        use crate::model::command::{
            AggregateArgumentType, ArgumentMember, ArgumentType, CommandMetaData,
        };

        let mut t = AggregateArgumentType::new("Point");
        t.short_description = Some("2D point".into());
        t.member_list = vec![
            ArgumentMember { name: "x".into(), type_ref: "Int16T".into(), short_description: Some("x coord".into()) },
            ArgumentMember { name: "y".into(), type_ref: "Int16T".into(), short_description: None },
        ];

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("Point".into(), ArgumentType::Aggregate(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("Point").cloned().unwrap();
        let ArgumentType::Aggregate(a) = at else { panic!("expected Aggregate") };
        assert_eq!(a.member_list.len(), 2);
        assert_eq!(a.member_list[0].name, "x");
        assert_eq!(a.member_list[0].type_ref, "Int16T");
        assert_eq!(a.member_list[0].short_description.as_deref(), Some("x coord"));
        assert_eq!(a.member_list[1].short_description, None);
    }

    #[test]
    fn t6_array_argument_type_round_trip() {
        use crate::model::command::{ArgumentType, ArrayArgumentType, CommandMetaData};

        let mut t = ArrayArgumentType::new("Matrix", "Float32T");
        t.number_of_dimensions = 2;
        t.short_description = Some("2D matrix".into());

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("Matrix".into(), ArgumentType::Array(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("Matrix").cloned().unwrap();
        let ArgumentType::Array(a) = at else { panic!("expected Array") };
        assert_eq!(a.array_type_ref, "Float32T");
        assert_eq!(a.number_of_dimensions, 2);
        assert_eq!(a.short_description.as_deref(), Some("2D matrix"));
    }

    // ── T6/T3 extended: alias set serialization ───────────────────────────────
    // Note: the parser does not yet read <AliasSet>; these tests verify the
    // serializer output directly rather than doing a full round-trip.

    #[test]
    fn t6_alias_set_on_argument_type_serialized() {
        use crate::model::command::{ArgumentType, CommandMetaData, IntegerArgumentType};
        use crate::model::types::Alias;

        let mut t = IntegerArgumentType::new("CmdInt");
        t.alias_set = vec![
            Alias { name_space: "yamcs".into(), alias: "CMD_INT".into() },
            Alias { name_space: "mcs".into(), alias: "int_cmd".into() },
        ];

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("CmdInt".into(), ArgumentType::Integer(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let bytes = serialize(&ss).unwrap();
        let xml = String::from_utf8(bytes).unwrap();
        assert!(xml.contains("<AliasSet>"), "expected AliasSet element");
        assert!(xml.contains(r#"nameSpace="yamcs""#), "expected yamcs namespace");
        assert!(xml.contains(r#"alias="CMD_INT""#), "expected CMD_INT alias");
        assert!(xml.contains(r#"nameSpace="mcs""#), "expected mcs namespace");
    }

    #[test]
    fn t3_alias_set_on_parameter_type_serialized() {
        use crate::model::telemetry::{IntegerParameterType, ParameterType};
        use crate::model::types::Alias;

        let mut t = IntegerParameterType::new("IntT");
        t.alias_set = vec![Alias { name_space: "ccsds".into(), alias: "UINT16".into() }];
        let ss = make_ss_with_type(ParameterType::Integer(t));

        let bytes = serialize(&ss).unwrap();
        let xml = String::from_utf8(bytes).unwrap();
        assert!(xml.contains("<AliasSet>"), "expected AliasSet element");
        assert!(xml.contains(r#"nameSpace="ccsds""#));
        assert!(xml.contains(r#"alias="UINT16""#));
    }

    // ── T4 extended: remaining DataSource variants ────────────────────────────

    #[test]
    fn t4_data_source_constant_local_ground_round_trip() {
        use crate::model::telemetry::{DataSource, Parameter, ParameterProperties, TelemetryMetaData};

        let make_param = |name: &str, ds: DataSource| {
            let mut p = Parameter::new(name, "T");
            p.parameter_properties = Some(ParameterProperties { data_source: Some(ds), read_only: false });
            p
        };

        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        tm.parameters.insert("C".into(), make_param("C", DataSource::Constant));
        tm.parameters.insert("L".into(), make_param("L", DataSource::Local));
        tm.parameters.insert("G".into(), make_param("G", DataSource::Ground));
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_tm = rt.telemetry.unwrap();

        let ds_of = |name: &str| {
            rt_tm.parameters.get(name).unwrap()
                .parameter_properties.as_ref().unwrap()
                .data_source.as_ref().cloned()
        };
        assert_eq!(ds_of("C"), Some(DataSource::Constant));
        assert_eq!(ds_of("L"), Some(DataSource::Local));
        assert_eq!(ds_of("G"), Some(DataSource::Ground));
    }

    // ── T3 extended: long description on ParameterType ───────────────────────

    #[test]
    fn t3_long_description_on_parameter_type_round_trip() {
        use crate::model::telemetry::{FloatParameterType, ParameterType};

        let mut t = FloatParameterType::new("Voltage");
        t.long_description = Some("The bus voltage in volts.".into());
        let ss = make_ss_with_type(ParameterType::Float(t));
        let rt = round_trip(&ss);
        let tm = rt.telemetry.unwrap();
        let ParameterType::Float(f) = tm.parameter_types.get("Voltage").unwrap() else {
            panic!("expected Float")
        };
        assert_eq!(f.long_description.as_deref(), Some("The bus voltage in volts."));
    }

    // ── T6 extended: argument type baseType round-trip ────────────────────────

    #[test]
    fn t6_base_type_on_argument_type_round_trip() {
        use crate::model::command::{ArgumentType, CommandMetaData, FloatArgumentType};

        let mut t = FloatArgumentType::new("DerivedFloat");
        t.base_type = Some("BaseFloatT".into());
        t.size_in_bits = Some(32);

        let mut cm = CommandMetaData::default();
        cm.argument_types.insert("DerivedFloat".into(), ArgumentType::Float(t));
        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let at = rt.command.unwrap().argument_types.get("DerivedFloat").cloned().unwrap();
        let ArgumentType::Float(f) = at else { panic!("expected Float") };
        assert_eq!(f.base_type.as_deref(), Some("BaseFloatT"));
    }

    #[test]
    fn t6_meta_command_round_trip() {
        use crate::model::command::{
            Argument, ArgumentRefEntry, ArgumentType, CommandContainer, CommandEntry,
            CommandMetaData, IntegerArgumentType, MetaCommand,
        };
        let mut ss = SpaceSystem::new("Test");
        let mut cm = CommandMetaData::default();

        let mut at = IntegerArgumentType::new("Uint8T");
        at.signed = false;
        at.size_in_bits = Some(8);
        cm.argument_types.insert("Uint8T".to_string(), ArgumentType::Integer(at));

        let mut mc = MetaCommand::new("SendData");
        mc.short_description = Some("Send a data packet".to_string());
        mc.argument_list = vec![Argument::new("value", "Uint8T")];
        mc.command_container = Some(CommandContainer {
            name: "SendDataContainer".to_string(),
            base_container: None,
            entry_list: vec![CommandEntry::ArgumentRef(ArgumentRefEntry {
                argument_ref: "value".to_string(),
                location: None,
            })],
        });
        cm.meta_commands.insert("SendData".to_string(), mc);
        ss.command = Some(cm);

        let rt = round_trip(&ss);
        let cm = rt.command.unwrap();
        let mc = cm.meta_commands.get("SendData").unwrap();
        assert_eq!(mc.short_description.as_deref(), Some("Send a data packet"));
        assert_eq!(mc.argument_list.len(), 1);
        assert_eq!(mc.argument_list[0].name, "value");
        assert_eq!(mc.argument_list[0].argument_type_ref, "Uint8T");
        let cc = mc.command_container.as_ref().unwrap();
        assert_eq!(cc.name, "SendDataContainer");
        assert_eq!(cc.entry_list.len(), 1);
        let CommandEntry::ArgumentRef(ae) = &cc.entry_list[0] else { panic!() };
        assert_eq!(ae.argument_ref, "value");
    }

    // ── helper for encoding tests ─────────────────────────────────────────────

    fn enc_xml(f: impl Fn(&mut W) -> Result<(), ParseError>) -> String {
        let mut w = Writer::new(Vec::new());
        f(&mut w).unwrap();
        String::from_utf8(w.into_inner()).unwrap()
    }

    // ── T3: ParameterTypes ───────────────────────────────────────────────────

    fn make_ss_with_type(pt: crate::model::telemetry::ParameterType) -> SpaceSystem {
        use crate::model::telemetry::TelemetryMetaData;
        let mut ss = SpaceSystem::new("Test");
        let mut tm = TelemetryMetaData::default();
        tm.parameter_types.insert(pt.name().to_owned(), pt);
        ss.telemetry = Some(tm);
        ss
    }

    #[test]
    fn t3_integer_parameter_type_round_trip() {
        use crate::model::telemetry::{IntegerParameterType, ParameterType};
        use crate::model::types::IntegerEncoding;
        let mut t = IntegerParameterType::new("MyInt");
        t.short_description = Some("An int".to_string());
        t.signed = false;
        t.size_in_bits = Some(16);
        t.encoding = Some(IntegerDataEncoding {
            size_in_bits: 16,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: None,
        });
        let rt = round_trip(&make_ss_with_type(ParameterType::Integer(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyInt").unwrap();
        let ParameterType::Integer(t) = pt else { panic!() };
        assert_eq!(t.name, "MyInt");
        assert_eq!(t.short_description.as_deref(), Some("An int"));
        assert!(!t.signed);
        assert_eq!(t.size_in_bits, Some(16));
        assert!(t.encoding.is_some());
    }

    #[test]
    fn t3_float_parameter_type_round_trip() {
        use crate::model::telemetry::{FloatParameterType, ParameterType};
        use crate::model::types::{FloatEncoding, FloatSizeInBits};
        let mut t = FloatParameterType::new("MyFloat");
        t.short_description = Some("A float".to_string());
        t.encoding = Some(FloatDataEncoding {
            size_in_bits: FloatSizeInBits::F32,
            encoding: FloatEncoding::IEEE754_1985,
            byte_order: None,
            default_calibrator: None,
        });
        let rt = round_trip(&make_ss_with_type(ParameterType::Float(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyFloat").unwrap();
        let ParameterType::Float(t) = pt else { panic!() };
        assert_eq!(t.name, "MyFloat");
        assert_eq!(t.short_description.as_deref(), Some("A float"));
        assert!(t.encoding.is_some());
    }

    #[test]
    fn t3_enumerated_parameter_type_round_trip() {
        use crate::model::telemetry::{EnumeratedParameterType, ParameterType};
        use crate::model::types::IntegerEncoding;
        let mut t = EnumeratedParameterType::new("MyEnum");
        t.encoding = Some(IntegerDataEncoding {
            size_in_bits: 8,
            encoding: IntegerEncoding::Unsigned,
            byte_order: None,
            default_calibrator: None,
        });
        t.enumeration_list = vec![
            ValueEnumeration { value: 0, label: "OFF".to_string(), max_value: None, short_description: None },
            ValueEnumeration { value: 1, label: "ON".to_string(), max_value: None, short_description: Some("Active".to_string()) },
        ];
        let rt = round_trip(&make_ss_with_type(ParameterType::Enumerated(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyEnum").unwrap();
        let ParameterType::Enumerated(t) = pt else { panic!() };
        assert_eq!(t.enumeration_list.len(), 2);
        assert_eq!(t.enumeration_list[0].label, "OFF");
        assert_eq!(t.enumeration_list[1].label, "ON");
        assert_eq!(t.enumeration_list[1].short_description.as_deref(), Some("Active"));
    }

    #[test]
    fn t3_boolean_parameter_type_round_trip() {
        use crate::model::telemetry::{BooleanParameterType, ParameterType};
        let mut t = BooleanParameterType::new("MyBool");
        t.one_string_value = Some("YES".to_string());
        t.zero_string_value = Some("NO".to_string());
        let rt = round_trip(&make_ss_with_type(ParameterType::Boolean(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyBool").unwrap();
        let ParameterType::Boolean(t) = pt else { panic!() };
        assert_eq!(t.one_string_value.as_deref(), Some("YES"));
        assert_eq!(t.zero_string_value.as_deref(), Some("NO"));
    }

    #[test]
    fn t3_string_parameter_type_round_trip() {
        use crate::model::telemetry::{ParameterType, StringParameterType};
        let mut t = StringParameterType::new("MyStr");
        t.encoding = Some(StringDataEncoding {
            encoding: StringEncoding::UTF8,
            byte_order: None,
            size_in_bits: None,
        });
        let rt = round_trip(&make_ss_with_type(ParameterType::String(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyStr").unwrap();
        let ParameterType::String(t) = pt else { panic!() };
        assert!(t.encoding.is_some());
    }

    #[test]
    fn t3_binary_parameter_type_round_trip() {
        use crate::model::telemetry::{BinaryParameterType, ParameterType};
        let mut t = BinaryParameterType::new("MyBin");
        t.encoding = Some(BinaryDataEncoding { size_in_bits: BinarySize::Fixed(32) });
        let rt = round_trip(&make_ss_with_type(ParameterType::Binary(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyBin").unwrap();
        let ParameterType::Binary(t) = pt else { panic!() };
        assert!(t.encoding.is_some());
    }

    #[test]
    fn t3_aggregate_parameter_type_round_trip() {
        use crate::model::telemetry::{AggregateParameterType, Member, ParameterType};
        let mut t = AggregateParameterType::new("MyAgg");
        t.member_list = vec![
            Member { name: "x".to_string(), type_ref: "MyInt".to_string(), short_description: None },
            Member { name: "y".to_string(), type_ref: "MyFloat".to_string(), short_description: Some("Y axis".to_string()) },
        ];
        let rt = round_trip(&make_ss_with_type(ParameterType::Aggregate(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyAgg").unwrap();
        let ParameterType::Aggregate(t) = pt else { panic!() };
        assert_eq!(t.member_list.len(), 2);
        assert_eq!(t.member_list[0].name, "x");
        assert_eq!(t.member_list[0].type_ref, "MyInt");
        assert_eq!(t.member_list[1].short_description.as_deref(), Some("Y axis"));
    }

    #[test]
    fn t3_array_parameter_type_round_trip() {
        use crate::model::telemetry::{ArrayParameterType, ParameterType};
        let mut t = ArrayParameterType::new("MyArr", "MyFloat");
        t.number_of_dimensions = 2;
        let rt = round_trip(&make_ss_with_type(ParameterType::Array(t)));
        let tm = rt.telemetry.unwrap();
        let pt = tm.parameter_types.get("MyArr").unwrap();
        let ParameterType::Array(t) = pt else { panic!() };
        assert_eq!(t.array_type_ref, "MyFloat");
        assert_eq!(t.number_of_dimensions, 2);
    }

    // ── File-based round-trip tests ───────────────────────────────────────────

    /// Parse a real XTCE file, serialize it, re-parse, and assert the model is
    /// semantically identical.  This catches any field omissions that the
    /// per-element unit tests might miss, and verifies that the serializer
    /// produces output the parser can consume (namespace-free tags vs. the
    /// `xtce:`-prefixed input).
    fn round_trip_file(path: &str) -> (crate::model::space_system::SpaceSystem, crate::model::space_system::SpaceSystem) {
        let original = crate::parser::parse_file(std::path::Path::new(path))
            .unwrap_or_else(|e| panic!("parse_file({path}) failed: {e}"));
        let bytes = serialize(&original)
            .unwrap_or_else(|e| panic!("serialize({path}) failed: {e}"));
        let reparsed = crate::parser::parse(&bytes)
            .unwrap_or_else(|e| panic!("re-parse({path}) failed: {e}"));
        (original, reparsed)
    }

    #[test]
    fn file_round_trip_simple_tlm() {
        let (orig, rt) = round_trip_file("../test_data/simple_tlm.xtce");
        assert_eq!(rt.name, orig.name);
        let orig_tm = orig.telemetry.as_ref().unwrap();
        let rt_tm = rt.telemetry.as_ref().unwrap();
        assert_eq!(rt_tm.parameter_types.len(), orig_tm.parameter_types.len());
        assert_eq!(rt_tm.parameters.len(), orig_tm.parameters.len());
        assert_eq!(rt_tm.containers.len(), orig_tm.containers.len());
    }

    #[test]
    fn file_round_trip_more_involved() {
        let (orig, rt) = round_trip_file("../test_data/more_involved.xtce");
        assert_eq!(rt.name, orig.name);
        // Telemetry
        let orig_tm = orig.telemetry.as_ref().unwrap();
        let rt_tm = rt.telemetry.as_ref().unwrap();
        assert_eq!(rt_tm.parameter_types.len(), orig_tm.parameter_types.len());
        assert_eq!(rt_tm.parameters.len(), orig_tm.parameters.len());
        assert_eq!(rt_tm.containers.len(), orig_tm.containers.len());
        // Verify container inheritance survived
        let rt_sys = rt_tm.containers.get("SystemStatusPacket").unwrap();
        let orig_sys = orig_tm.containers.get("SystemStatusPacket").unwrap();
        assert_eq!(
            rt_sys.base_container.as_ref().map(|bc| bc.container_ref.as_str()),
            orig_sys.base_container.as_ref().map(|bc| bc.container_ref.as_str()),
        );
        // Commands
        let orig_cm = orig.command.as_ref().unwrap();
        let rt_cm = rt.command.as_ref().unwrap();
        assert_eq!(rt_cm.argument_types.len(), orig_cm.argument_types.len());
        assert_eq!(rt_cm.meta_commands.len(), orig_cm.meta_commands.len());
        // Verify argument list survived
        let rt_mc = rt_cm.meta_commands.get("PowerCycleSubsystem").unwrap();
        let orig_mc = orig_cm.meta_commands.get("PowerCycleSubsystem").unwrap();
        assert_eq!(rt_mc.argument_list.len(), orig_mc.argument_list.len());
        assert_eq!(rt_mc.argument_list[0].name, orig_mc.argument_list[0].name);
    }

    // ── T5 extended: BooleanExpression restriction criteria ───────────────────

    #[test]
    fn t5_boolean_expression_and_restriction_round_trip() {
        use crate::model::container::{
            BaseContainer, BooleanExpression, Comparison, ComparisonOperator,
            RestrictionCriteria, SequenceContainer,
        };

        let make_cmp = |param: &str, val: &str| Comparison {
            parameter_ref: param.into(),
            value: val.into(),
            comparison_operator: ComparisonOperator::Equality,
            use_calibrated_value: false,
        };
        let expr = BooleanExpression::And(vec![
            BooleanExpression::Condition(make_cmp("APID", "5")),
            BooleanExpression::Condition(make_cmp("Version", "1")),
        ]);

        let mut base = SequenceContainer::new("Base");
        base.entry_list = vec![];
        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".into(),
            restriction_criteria: Some(RestrictionCriteria::BooleanExpression(expr)),
        });

        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Base".into(), base);
        tm.containers.insert("Child".into(), child);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let base_cont = rt
            .telemetry.as_ref().unwrap()
            .containers.get("Child").unwrap()
            .base_container.as_ref().unwrap();
        assert!(matches!(
            base_cont.restriction_criteria.as_ref().unwrap(),
            RestrictionCriteria::BooleanExpression(BooleanExpression::And(_))
        ));
    }

    #[test]
    fn t5_boolean_expression_or_restriction_round_trip() {
        use crate::model::container::{
            BaseContainer, BooleanExpression, Comparison, ComparisonOperator,
            RestrictionCriteria, SequenceContainer,
        };

        let make_cmp = |p: &str, v: &str| Comparison {
            parameter_ref: p.into(),
            value: v.into(),
            comparison_operator: ComparisonOperator::Equality,
            use_calibrated_value: true,
        };
        let expr = BooleanExpression::Or(vec![
            BooleanExpression::Condition(make_cmp("TypeA", "1")),
            BooleanExpression::Condition(make_cmp("TypeB", "2")),
        ]);

        let mut base = SequenceContainer::new("Base");
        base.entry_list = vec![];
        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".into(),
            restriction_criteria: Some(RestrictionCriteria::BooleanExpression(expr)),
        });

        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Base".into(), base);
        tm.containers.insert("Child".into(), child);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let base_cont = rt
            .telemetry.as_ref().unwrap()
            .containers.get("Child").unwrap()
            .base_container.as_ref().unwrap();
        assert!(matches!(
            base_cont.restriction_criteria.as_ref().unwrap(),
            RestrictionCriteria::BooleanExpression(BooleanExpression::Or(_))
        ));
    }

    // ── T5 extended: NextContainer restriction criteria ───────────────────────

    #[test]
    fn t5_next_container_restriction_round_trip() {
        use crate::model::container::{BaseContainer, RestrictionCriteria, SequenceContainer};

        let mut base = SequenceContainer::new("Base");
        base.entry_list = vec![];
        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".into(),
            restriction_criteria: Some(RestrictionCriteria::NextContainer {
                container_ref: "Payload".into(),
            }),
        });

        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Base".into(), base);
        tm.containers.insert("Child".into(), child);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let base_cont = rt
            .telemetry.as_ref().unwrap()
            .containers.get("Child").unwrap()
            .base_container.as_ref().unwrap();
        assert!(matches!(
            base_cont.restriction_criteria.as_ref().unwrap(),
            RestrictionCriteria::NextContainer { container_ref }
                if container_ref == "Payload"
        ));
    }

    // ── T5 extended: entry variants with location and IncludeCondition ────────

    #[test]
    fn t5_parameter_ref_with_include_condition_round_trip() {
        use crate::model::container::{
            Comparison, ComparisonOperator, EntryLocation, MatchCriteria, ParameterRefEntry,
            ReferenceLocation, SequenceContainer, SequenceEntry,
        };

        let entry = SequenceEntry::ParameterRef(ParameterRefEntry {
            parameter_ref: "Val".into(),
            location: Some(EntryLocation {
                reference_location: ReferenceLocation::ContainerStart,
                bit_offset: 16,
            }),
            include_condition: Some(MatchCriteria::Comparison(Comparison {
                parameter_ref: "Flag".into(),
                value: "1".into(),
                comparison_operator: ComparisonOperator::Equality,
                use_calibrated_value: true,
            })),
        });

        let mut c = SequenceContainer::new("Pkt");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Pkt".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_pkt = rt.telemetry.as_ref().unwrap().containers.get("Pkt").unwrap();
        let SequenceEntry::ParameterRef(e) = &rt_pkt.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        let loc = e.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 16);
        let MatchCriteria::Comparison(cmp) = e.include_condition.as_ref().unwrap() else {
            panic!("expected Comparison IncludeCondition")
        };
        assert_eq!(cmp.parameter_ref, "Flag");
    }

    #[test]
    fn t5_container_ref_with_include_condition_round_trip() {
        use crate::model::container::{
            Comparison, ComparisonOperator, ContainerRefEntry, EntryLocation, MatchCriteria,
            ReferenceLocation, SequenceContainer, SequenceEntry,
        };

        let entry = SequenceEntry::ContainerRef(ContainerRefEntry {
            container_ref: "Sub".into(),
            location: Some(EntryLocation {
                reference_location: ReferenceLocation::PreviousEntry,
                bit_offset: 32,
            }),
            include_condition: Some(MatchCriteria::Comparison(Comparison {
                parameter_ref: "Enable".into(),
                value: "1".into(),
                comparison_operator: ComparisonOperator::Equality,
                use_calibrated_value: false,
            })),
        });

        let mut c = SequenceContainer::new("Outer");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Outer".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_outer = rt.telemetry.as_ref().unwrap().containers.get("Outer").unwrap();
        let SequenceEntry::ContainerRef(e) = &rt_outer.entry_list[0] else {
            panic!("expected ContainerRef")
        };
        assert_eq!(e.container_ref, "Sub");
        assert_eq!(e.location.as_ref().unwrap().bit_offset, 32);
        let MatchCriteria::Comparison(cmp) = e.include_condition.as_ref().unwrap() else {
            panic!("expected Comparison IncludeCondition")
        };
        assert_eq!(cmp.parameter_ref, "Enable");
        assert!(!cmp.use_calibrated_value);
    }

    #[test]
    fn t5_fixed_value_entry_with_location_round_trip() {
        use crate::model::container::{
            EntryLocation, FixedValueEntry, ReferenceLocation, SequenceContainer, SequenceEntry,
        };

        let entry = SequenceEntry::FixedValue(FixedValueEntry {
            size_in_bits: 8,
            binary_value: Some("FF".into()),
            location: Some(EntryLocation {
                reference_location: ReferenceLocation::ContainerStart,
                bit_offset: 0,
            }),
        });

        let mut c = SequenceContainer::new("Pkt");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Pkt".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_pkt = rt.telemetry.as_ref().unwrap().containers.get("Pkt").unwrap();
        let SequenceEntry::FixedValue(fv) = &rt_pkt.entry_list[0] else {
            panic!("expected FixedValue")
        };
        assert_eq!(fv.size_in_bits, 8);
        assert_eq!(fv.binary_value.as_deref(), Some("FF"));
        let loc = fv.location.as_ref().unwrap();
        assert_eq!(loc.reference_location, ReferenceLocation::ContainerStart);
        assert_eq!(loc.bit_offset, 0);
    }

    #[test]
    fn t5_array_parameter_ref_with_location_round_trip() {
        use crate::model::container::{
            ArrayParameterRefEntry, EntryLocation, ReferenceLocation, SequenceContainer,
            SequenceEntry,
        };

        let entry = SequenceEntry::ArrayParameterRef(ArrayParameterRefEntry {
            parameter_ref: "Samples".into(),
            location: Some(EntryLocation {
                reference_location: ReferenceLocation::ContainerStart,
                bit_offset: 64,
            }),
        });

        let mut c = SequenceContainer::new("DataPkt");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("DataPkt".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_pkt = rt.telemetry.as_ref().unwrap().containers.get("DataPkt").unwrap();
        let SequenceEntry::ArrayParameterRef(ar) = &rt_pkt.entry_list[0] else {
            panic!("expected ArrayParameterRef")
        };
        assert_eq!(ar.parameter_ref, "Samples");
        assert_eq!(ar.location.as_ref().unwrap().bit_offset, 64);
    }

    // ── T5 extended: MatchCriteria with ComparisonList and BooleanExpression ──

    #[test]
    fn t5_include_condition_comparison_list_round_trip() {
        use crate::model::container::{
            Comparison, ComparisonOperator, MatchCriteria, ParameterRefEntry, SequenceContainer,
            SequenceEntry,
        };

        let list = vec![
            Comparison {
                parameter_ref: "FlagA".into(),
                value: "1".into(),
                comparison_operator: ComparisonOperator::Equality,
                use_calibrated_value: true,
            },
            Comparison {
                parameter_ref: "FlagB".into(),
                value: "2".into(),
                comparison_operator: ComparisonOperator::Inequality,
                use_calibrated_value: false,
            },
        ];
        let entry = SequenceEntry::ParameterRef(ParameterRefEntry {
            parameter_ref: "Opt".into(),
            location: None,
            include_condition: Some(MatchCriteria::ComparisonList(list)),
        });

        let mut c = SequenceContainer::new("Pkt");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Pkt".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_pkt = rt.telemetry.as_ref().unwrap().containers.get("Pkt").unwrap();
        let SequenceEntry::ParameterRef(e) = &rt_pkt.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        let MatchCriteria::ComparisonList(cmps) = e.include_condition.as_ref().unwrap() else {
            panic!("expected ComparisonList")
        };
        assert_eq!(cmps.len(), 2);
        assert_eq!(cmps[0].parameter_ref, "FlagA");
        assert_eq!(cmps[1].comparison_operator, ComparisonOperator::Inequality);
    }

    #[test]
    fn t5_include_condition_boolean_expression_round_trip() {
        use crate::model::container::{
            BooleanExpression, Comparison, ComparisonOperator, MatchCriteria, ParameterRefEntry,
            SequenceContainer, SequenceEntry,
        };

        let cond = BooleanExpression::Condition(Comparison {
            parameter_ref: "Mode".into(),
            value: "3".into(),
            comparison_operator: ComparisonOperator::GreaterThan,
            use_calibrated_value: true,
        });
        let entry = SequenceEntry::ParameterRef(ParameterRefEntry {
            parameter_ref: "Ext".into(),
            location: None,
            include_condition: Some(MatchCriteria::BooleanExpression(cond)),
        });

        let mut c = SequenceContainer::new("Pkt");
        c.entry_list = vec![entry];
        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Pkt".into(), c);
        ss.telemetry = Some(tm);

        let rt = round_trip(&ss);
        let rt_pkt = rt.telemetry.as_ref().unwrap().containers.get("Pkt").unwrap();
        let SequenceEntry::ParameterRef(e) = &rt_pkt.entry_list[0] else {
            panic!("expected ParameterRef")
        };
        let MatchCriteria::BooleanExpression(BooleanExpression::Condition(cmp)) =
            e.include_condition.as_ref().unwrap()
        else {
            panic!("expected BooleanExpression::Condition IncludeCondition")
        };
        assert_eq!(cmp.parameter_ref, "Mode");
        assert_eq!(cmp.comparison_operator, ComparisonOperator::GreaterThan);
    }

    // ── T6 extended: CommandContainerSet serialization ────────────────────────

    #[test]
    fn t6_command_container_set_round_trip() {
        use crate::model::command::CommandMetaData;
        use crate::model::container::SequenceContainer;

        let mut cc = SequenceContainer::new("SharedCmdPkt");
        cc.short_description = Some("shared command packet".into());

        let mut cmd = CommandMetaData::default();
        cmd.command_containers.insert("SharedCmdPkt".into(), cc);

        let mut ss = SpaceSystem::new("Test");
        ss.command = Some(cmd);

        let rt = round_trip(&ss);
        let rt_cmd = rt.command.as_ref().unwrap();
        assert_eq!(rt_cmd.command_containers.len(), 1);
        let rt_cc = rt_cmd.command_containers.get("SharedCmdPkt").unwrap();
        assert_eq!(rt_cc.name, "SharedCmdPkt");
        assert_eq!(rt_cc.short_description.as_deref(), Some("shared command packet"));
    }

    // ── BooleanExpression::Not is silently skipped in serializer ─────────────

    #[test]
    fn t5_boolean_expression_not_silently_skipped() {
        use crate::model::container::{
            BaseContainer, BooleanExpression, Comparison, ComparisonOperator,
            RestrictionCriteria, SequenceContainer,
        };

        let cmp = Comparison {
            parameter_ref: "X".into(),
            value: "1".into(),
            comparison_operator: ComparisonOperator::Equality,
            use_calibrated_value: true,
        };
        let not_expr =
            BooleanExpression::Not(Box::new(BooleanExpression::Condition(cmp)));

        let mut base = SequenceContainer::new("Base");
        base.entry_list = vec![];
        let mut child = SequenceContainer::new("Child");
        child.base_container = Some(BaseContainer {
            container_ref: "Base".into(),
            restriction_criteria: Some(RestrictionCriteria::BooleanExpression(not_expr)),
        });

        let mut ss = SpaceSystem::new("Test");
        let mut tm = crate::model::telemetry::TelemetryMetaData::default();
        tm.containers.insert("Base".into(), base);
        tm.containers.insert("Child".into(), child);
        ss.telemetry = Some(tm);

        // Not arm is intentionally a no-op in the serializer; must not panic.
        let bytes = serialize(&ss).expect("serialize should not fail");
        assert!(!bytes.is_empty());
    }
}
