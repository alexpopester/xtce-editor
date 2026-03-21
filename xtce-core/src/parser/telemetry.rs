//! Parsers for `<TelemetryMetaData>`, `<ParameterSet>`, and `<Parameter>`.

use std::io::BufRead;

use quick_xml::events::BytesStart;

use crate::model::telemetry::{Parameter, ParameterProperties, TelemetryMetaData};
use crate::ParseError;

use super::context::ParseContext;

/// Parse a `<TelemetryMetaData>` element and all of its children
/// (ParameterTypeSet, ParameterSet, ContainerSet).
pub(super) fn parse_telemetry_metadata<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<TelemetryMetaData, ParseError> {
    todo!("create empty TelemetryMetaData, loop over children, dispatch to sub-parsers")
}

/// Populate `telemetry.parameters` by consuming a `<ParameterSet>` element.
pub(super) fn parse_parameter_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    telemetry: &mut TelemetryMetaData,
) -> Result<(), ParseError> {
    todo!("loop over <Parameter> children, insert into telemetry.parameters")
}

/// Parse a single `<Parameter>` element.
pub(super) fn parse_parameter<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Parameter, ParseError> {
    todo!("parse name and parameterTypeRef attrs, then optional child elements")
}

/// Parse a `<ParameterProperties>` child element.
fn parse_parameter_properties<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<ParameterProperties, ParseError> {
    todo!("parse dataSource and readOnly attrs/children")
}
