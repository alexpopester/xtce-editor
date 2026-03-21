//! Parsers for `<TelemetryMetaData>`, `<ParameterSet>`, and `<Parameter>`.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};

use crate::model::telemetry::{DataSource, Parameter, ParameterProperties, TelemetryMetaData};
use crate::ParseError;

use super::context::ParseContext;

/// Parse a `<TelemetryMetaData>` element and all of its children
/// (ParameterTypeSet, ParameterSet, ContainerSet).
pub(super) fn parse_telemetry_metadata<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<TelemetryMetaData, ParseError> {
    let mut telemetry = TelemetryMetaData::default();

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"ParameterTypeSet" => {
                    super::types::parse_parameter_type_set(ctx, &mut telemetry)?
                }
                b"ParameterSet" => parse_parameter_set(ctx, &mut telemetry)?,
                b"ContainerSet" => {
                    super::container::parse_container_set(ctx, &mut telemetry)?
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</TelemetryMetaData>" })
            }
            _ => {}
        }
    }

    Ok(telemetry)
}

/// Populate `telemetry.parameters` by consuming a `<ParameterSet>` element.
pub(super) fn parse_parameter_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    telemetry: &mut TelemetryMetaData,
) -> Result<(), ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Parameter" => {
                    let p = parse_parameter(ctx, &e)?;
                    telemetry.parameters.insert(p.name.clone(), p);
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</ParameterSet>" }),
            _ => {}
        }
    }
    Ok(())
}

/// Parse a single `<Parameter>` element.
pub(super) fn parse_parameter<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Parameter, ParseError> {
    let name = ctx.require_attr(start, "name", "Parameter")?.into_owned();
    let parameter_type_ref =
        ctx.require_attr(start, "parameterTypeRef", "Parameter")?.into_owned();
    let mut p = Parameter::new(name, parameter_type_ref);
    p.short_description = ctx.get_attr_owned(start, "shortDescription");

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LongDescription" => p.long_description = Some(ctx.read_text_content()?),
                b"ParameterProperties" => {
                    p.parameter_properties = Some(parse_parameter_properties(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</Parameter>" }),
            _ => {}
        }
    }

    Ok(p)
}

/// Parse a `<ParameterProperties>` child element.
fn parse_parameter_properties<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ParameterProperties, ParseError> {
    let data_source = ctx.get_attr(start, "dataSource").and_then(|v| match v.as_ref() {
        "telemetered" => Some(DataSource::Telemetered),
        "derived" => Some(DataSource::Derived),
        "constant" => Some(DataSource::Constant),
        "local" => Some(DataSource::Local),
        "ground" => Some(DataSource::Ground),
        _ => None,
    });
    let read_only = ctx
        .get_attr(start, "readOnly")
        .map(|v| v == "true")
        .unwrap_or(false);

    // ParameterProperties may have child elements; drain to End.
    ctx.skip_element(start)?;

    Ok(ParameterProperties { data_source, read_only })
}
