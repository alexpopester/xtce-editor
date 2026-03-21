//! Parsers for all `ParameterType` and `ArgumentType` variants.
//!
//! Both families share the same element-name-as-discriminator dispatch pattern
//! and nearly identical field structure, so they live in one module.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};

use crate::model::command::{
    AggregateArgumentType, ArrayArgumentType, BinaryArgumentType, BooleanArgumentType,
    CommandMetaData, EnumeratedArgumentType, FloatArgumentType, IntegerArgumentType,
    StringArgumentType,
};
use crate::model::telemetry::{
    AggregateParameterType, ArrayParameterType, BinaryParameterType, BooleanParameterType,
    EnumeratedParameterType, FloatParameterType, IntegerParameterType, StringParameterType,
    TelemetryMetaData,
};
use crate::model::types::{
    BinaryDataEncoding, BinarySize, ByteOrder, Calibrator, FloatDataEncoding, FloatEncoding,
    FloatSizeInBits, IntegerDataEncoding, IntegerEncoding, PolynomialCalibrator, SplineCalibrator,
    SplinePoint, StringDataEncoding, StringEncoding, StringSize, Unit, ValueEnumeration,
};
use crate::ParseError;

use super::context::ParseContext;

// ─────────────────────────────────────────────────────────────────────────────
// Enum-from-string converters
// ─────────────────────────────────────────────────────────────────────────────

fn parse_integer_encoding(s: &str) -> Result<IntegerEncoding, ParseError> {
    match s {
        "unsigned" => Ok(IntegerEncoding::Unsigned),
        "signMagnitude" => Ok(IntegerEncoding::SignMagnitude),
        "twosComplement" => Ok(IntegerEncoding::TwosComplement),
        "onesComplement" => Ok(IntegerEncoding::OnesComplement),
        "BCD" => Ok(IntegerEncoding::BCD),
        "packedBCD" => Ok(IntegerEncoding::PackedBCD),
        _ => Err(ParseError::InvalidValue {
            attr: "encoding",
            value: s.to_owned(),
            reason: "expected unsigned|signMagnitude|twosComplement|onesComplement|BCD|packedBCD",
        }),
    }
}

fn parse_float_encoding(s: &str) -> Result<FloatEncoding, ParseError> {
    match s {
        "IEEE754_1985" => Ok(FloatEncoding::IEEE754_1985),
        "milStd1750A" => Ok(FloatEncoding::MilStd1750A),
        _ => Err(ParseError::InvalidValue {
            attr: "encoding",
            value: s.to_owned(),
            reason: "expected IEEE754_1985|milStd1750A",
        }),
    }
}

fn parse_float_size(s: &str) -> Result<FloatSizeInBits, ParseError> {
    match s {
        "32" => Ok(FloatSizeInBits::F32),
        "64" => Ok(FloatSizeInBits::F64),
        "128" => Ok(FloatSizeInBits::F128),
        _ => Err(ParseError::InvalidValue {
            attr: "sizeInBits",
            value: s.to_owned(),
            reason: "expected 32|64|128",
        }),
    }
}

fn parse_byte_order(s: &str) -> Result<ByteOrder, ParseError> {
    match s {
        "mostSignificantByteFirst" => Ok(ByteOrder::MostSignificantByteFirst),
        "leastSignificantByteFirst" => Ok(ByteOrder::LeastSignificantByteFirst),
        _ => Err(ParseError::InvalidValue {
            attr: "byteOrder",
            value: s.to_owned(),
            reason: "expected mostSignificantByteFirst|leastSignificantByteFirst",
        }),
    }
}

fn parse_string_encoding(s: &str) -> Result<StringEncoding, ParseError> {
    match s {
        "UTF-8" => Ok(StringEncoding::UTF8),
        "UTF-16" => Ok(StringEncoding::UTF16),
        "US-ASCII" => Ok(StringEncoding::UsAscii),
        "ISO-8859-1" => Ok(StringEncoding::Iso8859_1),
        _ => Err(ParseError::InvalidValue {
            attr: "encoding",
            value: s.to_owned(),
            reason: "expected UTF-8|UTF-16|US-ASCII|ISO-8859-1",
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Numeric / bool attribute parsing helpers
// ─────────────────────────────────────────────────────────────────────────────

fn parse_u32(attr: &'static str, val: &str) -> Result<u32, ParseError> {
    val.parse().map_err(|_| ParseError::InvalidValue {
        attr,
        value: val.to_owned(),
        reason: "expected non-negative integer",
    })
}

fn parse_i64(attr: &'static str, val: &str) -> Result<i64, ParseError> {
    val.parse().map_err(|_| ParseError::InvalidValue {
        attr,
        value: val.to_owned(),
        reason: "expected integer",
    })
}

fn parse_f64(attr: &'static str, val: &str) -> Result<f64, ParseError> {
    val.parse().map_err(|_| ParseError::InvalidValue {
        attr,
        value: val.to_owned(),
        reason: "expected floating-point number",
    })
}

fn parse_bool(attr: &'static str, val: &str) -> Result<bool, ParseError> {
    match val {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(ParseError::InvalidValue {
            attr,
            value: val.to_owned(),
            reason: "expected true|false",
        }),
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Unit and ValueEnumeration
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a `<Unit>` element. Attributes carry metadata; text content is the
/// unit string itself (e.g., "m/s", "degC").
pub(super) fn parse_unit<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Unit, ParseError> {
    let power = ctx
        .get_attr(start, "power")
        .map(|v| parse_f64("power", &v))
        .transpose()?;
    let factor = ctx.get_attr_owned(start, "factor");
    let description = ctx.get_attr_owned(start, "description");
    let value = ctx.read_text_content()?;
    Ok(Unit { value, power, factor, description })
}

/// Consume a `<UnitSet>` element and collect all `<Unit>` children.
pub(super) fn parse_unit_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
) -> Result<Vec<Unit>, ParseError> {
    let mut units = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Unit" => units.push(parse_unit(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</UnitSet>" }),
            _ => {}
        }
    }
    Ok(units)
}

/// Parse an `<Enumeration>` element inside `<EnumerationList>`.
pub(super) fn parse_value_enumeration<R: BufRead>(
    ctx: &ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ValueEnumeration, ParseError> {
    let value = parse_i64("value", &ctx.require_attr(start, "value", "Enumeration")?)?;
    let label = ctx.require_attr(start, "label", "Enumeration")?.into_owned();
    let max_value = ctx
        .get_attr(start, "maxValue")
        .map(|v| parse_i64("maxValue", &v))
        .transpose()?;
    let short_description = ctx.get_attr_owned(start, "shortDescription");
    Ok(ValueEnumeration { value, label, max_value, short_description })
}

// ── ParameterType dispatch ───────────────────────────────────────────────────

/// Populate `telemetry.parameter_types` by consuming a `<ParameterTypeSet>`.
///
/// Dispatches to the appropriate variant parser based on the child element name.
pub(super) fn parse_parameter_type_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    telemetry: &mut TelemetryMetaData,
) -> Result<(), ParseError> {
    todo!("loop: match local element name → call variant parser → insert into telemetry.parameter_types")
}

// ── Concrete ParameterType variant parsers ───────────────────────────────────

pub(super) fn parse_integer_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<IntegerParameterType, ParseError> {
    todo!("parse name, signed, sizeInBits attrs; parse IntegerDataEncoding child")
}

pub(super) fn parse_float_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<FloatParameterType, ParseError> {
    todo!("parse name; parse FloatDataEncoding child")
}

pub(super) fn parse_enumerated_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<EnumeratedParameterType, ParseError> {
    todo!("parse name; parse EnumerationList children")
}

pub(super) fn parse_boolean_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<BooleanParameterType, ParseError> {
    todo!("parse name, oneStringValue, zeroStringValue attrs")
}

pub(super) fn parse_string_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<StringParameterType, ParseError> {
    todo!("parse name; parse StringDataEncoding child")
}

pub(super) fn parse_binary_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<BinaryParameterType, ParseError> {
    todo!("parse name; parse BinaryDataEncoding child")
}

pub(super) fn parse_aggregate_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<AggregateParameterType, ParseError> {
    todo!("parse name; parse MemberList children into member_list")
}

pub(super) fn parse_array_parameter_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ArrayParameterType, ParseError> {
    todo!("parse name and arrayTypeRef attrs; parse number of dimensions")
}

// ── ArgumentType dispatch ────────────────────────────────────────────────────

/// Populate `command.argument_types` by consuming an `<ArgumentTypeSet>`.
pub(super) fn parse_argument_type_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    command: &mut CommandMetaData,
) -> Result<(), ParseError> {
    todo!("loop: match local element name → call variant parser → insert into command.argument_types")
}

// ── Concrete ArgumentType variant parsers ────────────────────────────────────

pub(super) fn parse_integer_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<IntegerArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_float_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<FloatArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_enumerated_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<EnumeratedArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_boolean_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<BooleanArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_string_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<StringArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_binary_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<BinaryArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_aggregate_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<AggregateArgumentType, ParseError> {
    todo!()
}

pub(super) fn parse_array_argument_type<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ArrayArgumentType, ParseError> {
    todo!()
}

// ── Shared encoding / calibration parsers ────────────────────────────────────

/// Parse an `<IntegerDataEncoding>` element.
pub(super) fn parse_integer_data_encoding<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<IntegerDataEncoding, ParseError> {
    let size_in_bits = parse_u32(
        "sizeInBits",
        &ctx.require_attr(start, "sizeInBits", "IntegerDataEncoding")?,
    )?;
    let encoding = ctx
        .get_attr(start, "encoding")
        .map(|v| parse_integer_encoding(&v))
        .transpose()?
        .unwrap_or_default();
    let byte_order = ctx
        .get_attr(start, "byteOrder")
        .map(|v| parse_byte_order(&v))
        .transpose()?;

    let mut default_calibrator = None;
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"DefaultCalibrator" => default_calibrator = Some(parse_calibrator(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</IntegerDataEncoding>" })
            }
            _ => {}
        }
    }

    Ok(IntegerDataEncoding { size_in_bits, encoding, byte_order, default_calibrator })
}

/// Parse a `<FloatDataEncoding>` element.
pub(super) fn parse_float_data_encoding<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<FloatDataEncoding, ParseError> {
    let size_in_bits = parse_float_size(
        &ctx.require_attr(start, "sizeInBits", "FloatDataEncoding")?,
    )?;
    let encoding = ctx
        .get_attr(start, "encoding")
        .map(|v| parse_float_encoding(&v))
        .transpose()?
        .unwrap_or_default();
    let byte_order = ctx
        .get_attr(start, "byteOrder")
        .map(|v| parse_byte_order(&v))
        .transpose()?;

    let mut default_calibrator = None;
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"DefaultCalibrator" => default_calibrator = Some(parse_calibrator(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</FloatDataEncoding>" })
            }
            _ => {}
        }
    }

    Ok(FloatDataEncoding { size_in_bits, encoding, byte_order, default_calibrator })
}

/// Parse a `<StringDataEncoding>` element.
///
/// The optional `<SizeInBits>` child has three mutually exclusive inner forms:
/// `<Fixed>`, `<TerminationChar>`, and `<Variable>`.
pub(super) fn parse_string_data_encoding<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<StringDataEncoding, ParseError> {
    let encoding = ctx
        .get_attr(start, "encoding")
        .map(|v| parse_string_encoding(&v))
        .transpose()?
        .unwrap_or_default();
    let byte_order = ctx
        .get_attr(start, "byteOrder")
        .map(|v| parse_byte_order(&v))
        .transpose()?;

    let mut size_in_bits = None;
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"SizeInBits" => size_in_bits = Some(parse_string_size(ctx)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</StringDataEncoding>" })
            }
            _ => {}
        }
    }

    Ok(StringDataEncoding { encoding, byte_order, size_in_bits })
}

/// Parse the contents of a `<SizeInBits>` element inside a StringDataEncoding.
///
/// Dispatches on the first child element: `Fixed`, `TerminationChar`, or
/// `Variable`. Any other child is skipped and the function returns `None`
/// (no size constraint was parseable).
fn parse_string_size<R: BufRead>(ctx: &mut ParseContext<R>) -> Result<StringSize, ParseError> {
    let mut result = None;
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Fixed" => {
                    // <Fixed><FixedValue>16</FixedValue></Fixed>
                    let mut bits = None;
                    loop {
                        match ctx.next()? {
                            Event::Start(e) => match e.local_name().as_ref() {
                                b"FixedValue" => {
                                    bits = Some(parse_u32("FixedValue", &ctx.read_text_content()?)?)
                                }
                                _ => ctx.skip_element(&e)?,
                            },
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof { expected: "</Fixed>" })
                            }
                            _ => {}
                        }
                    }
                    if let Some(n) = bits {
                        result = Some(StringSize::Fixed(n));
                    }
                }
                b"TerminationChar" => {
                    // termChar attribute is a hex byte value, e.g. "00"
                    let raw = ctx.get_attr(&e, "termChar").unwrap_or_default();
                    let byte = u8::from_str_radix(&raw, 16).unwrap_or(0);
                    result = Some(StringSize::TerminationChar(byte));
                    ctx.skip_element(&e)?;
                }
                b"Variable" => {
                    let max = ctx
                        .get_attr(&e, "maxSizeInBits")
                        .map(|v| parse_u32("maxSizeInBits", &v))
                        .transpose()?
                        .unwrap_or(0);
                    result = Some(StringSize::Variable { max_size_in_bits: max });
                    ctx.skip_element(&e)?;
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</SizeInBits>" }),
            _ => {}
        }
    }
    // Fall back to a zero-width fixed size if no child was recognised.
    Ok(result.unwrap_or(StringSize::Fixed(0)))
}

/// Parse a `<BinaryDataEncoding>` element.
///
/// The `<SizeInBits>` child contains either a `<FixedValue>` text node (fixed
/// number of bits) or a `<DynamicValue>` element with a `sizeReference` attr.
pub(super) fn parse_binary_data_encoding<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<BinaryDataEncoding, ParseError> {
    let mut size_in_bits = BinarySize::Fixed(0);
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"SizeInBits" => {
                    loop {
                        match ctx.next()? {
                            Event::Start(e) => match e.local_name().as_ref() {
                                b"FixedValue" => {
                                    let n = parse_u32("FixedValue", &ctx.read_text_content()?)?;
                                    size_in_bits = BinarySize::Fixed(n);
                                }
                                b"DynamicValue" => {
                                    let r = ctx
                                        .get_attr_owned(&e, "sizeReference")
                                        .unwrap_or_default();
                                    size_in_bits = BinarySize::Variable { size_reference: r };
                                    ctx.skip_element(&e)?;
                                }
                                _ => ctx.skip_element(&e)?,
                            },
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: "</SizeInBits>",
                                })
                            }
                            _ => {}
                        }
                    }
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</BinaryDataEncoding>" })
            }
            _ => {}
        }
    }
    Ok(BinaryDataEncoding { size_in_bits })
}

/// Parse a `<DefaultCalibrator>` element.
///
/// Dispatches on the first child: `PolynomialCalibrator` (collects `Term`
/// children) or `SplineCalibrator` (reads `order`/`extrapolate` attrs and
/// collects `SplinePoint` children).
fn parse_calibrator<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Calibrator, ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"PolynomialCalibrator" => {
                    let mut coefficients: Vec<(u32, f64)> = Vec::new();
                    loop {
                        match ctx.next()? {
                            Event::Start(e) => match e.local_name().as_ref() {
                                b"Term" => {
                                    let exp = parse_u32(
                                        "exponent",
                                        &ctx.require_attr(&e, "exponent", "Term")?,
                                    )?;
                                    let coef = parse_f64(
                                        "coefficient",
                                        &ctx.require_attr(&e, "coefficient", "Term")?,
                                    )?;
                                    coefficients.push((exp, coef));
                                    ctx.skip_element(&e)?;
                                }
                                _ => ctx.skip_element(&e)?,
                            },
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: "</PolynomialCalibrator>",
                                })
                            }
                            _ => {}
                        }
                    }
                    // Build coefficient Vec indexed by exponent (sparse → dense).
                    let max_exp = coefficients.iter().map(|(e, _)| *e).max().unwrap_or(0) as usize;
                    let mut dense = vec![0.0f64; max_exp + 1];
                    for (exp, coef) in coefficients {
                        dense[exp as usize] = coef;
                    }
                    // Consume the DefaultCalibrator End tag.
                    loop {
                        match ctx.next()? {
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: "</DefaultCalibrator>",
                                })
                            }
                            _ => {}
                        }
                    }
                    return Ok(Calibrator::Polynomial(PolynomialCalibrator {
                        coefficients: dense,
                    }));
                }
                b"SplineCalibrator" => {
                    let order = ctx
                        .get_attr(&e, "order")
                        .map(|v| parse_u32("order", &v))
                        .transpose()?
                        .unwrap_or(0);
                    let extrapolate = ctx
                        .get_attr(&e, "extrapolate")
                        .map(|v| parse_bool("extrapolate", &v))
                        .transpose()?
                        .unwrap_or(false);
                    let mut points = Vec::new();
                    loop {
                        match ctx.next()? {
                            Event::Start(e) => match e.local_name().as_ref() {
                                b"SplinePoint" => {
                                    let raw = parse_f64(
                                        "raw",
                                        &ctx.require_attr(&e, "raw", "SplinePoint")?,
                                    )?;
                                    let calibrated = parse_f64(
                                        "calibrated",
                                        &ctx.require_attr(&e, "calibrated", "SplinePoint")?,
                                    )?;
                                    points.push(SplinePoint { raw, calibrated });
                                    ctx.skip_element(&e)?;
                                }
                                _ => ctx.skip_element(&e)?,
                            },
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: "</SplineCalibrator>",
                                })
                            }
                            _ => {}
                        }
                    }
                    // Consume the DefaultCalibrator End tag.
                    loop {
                        match ctx.next()? {
                            Event::End(_) => break,
                            Event::Eof => {
                                return Err(ParseError::UnexpectedEof {
                                    expected: "</DefaultCalibrator>",
                                })
                            }
                            _ => {}
                        }
                    }
                    return Ok(Calibrator::SplineCalibrator(SplineCalibrator {
                        order,
                        extrapolate,
                        points,
                    }));
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</DefaultCalibrator>" }),
            _ => {}
        }
    }
    Err(ParseError::UnexpectedElement("DefaultCalibrator with no recognised child".into()))
}

