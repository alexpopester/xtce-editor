//! Parsers for `<ContainerSet>`, `<SequenceContainer>`, entry lists, and
//! restriction criteria.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};

use crate::model::container::{
    ArrayParameterRefEntry, BaseContainer, BooleanExpression, Comparison, ComparisonOperator,
    ContainerRefEntry, EntryLocation, FixedValueEntry, MatchCriteria, ParameterRefEntry,
    ReferenceLocation, RestrictionCriteria, SequenceContainer, SequenceEntry,
};
use crate::model::telemetry::TelemetryMetaData;
use crate::ParseError;

use super::context::ParseContext;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level dispatchers
// ─────────────────────────────────────────────────────────────────────────────

/// Populate `telemetry.containers` by consuming a `<ContainerSet>` element.
pub(super) fn parse_container_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    telemetry: &mut TelemetryMetaData,
) -> Result<(), ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"SequenceContainer" => {
                    let c = parse_sequence_container(ctx, &e)?;
                    telemetry.containers.insert(c.name.clone(), c);
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</ContainerSet>" }),
            _ => {}
        }
    }
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// SequenceContainer
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a single `<SequenceContainer>` element.
pub(super) fn parse_sequence_container<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<SequenceContainer, ParseError> {
    let mut c = SequenceContainer::new(
        ctx.require_attr(start, "name", "SequenceContainer")?.as_ref(),
    );
    c.short_description = ctx.get_attr_owned(start, "shortDescription");
    c.r#abstract = ctx
        .get_attr(start, "abstract")
        .map(|v| v == "true")
        .unwrap_or(false);

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LongDescription" => c.long_description = Some(ctx.read_text_content()?),
                b"AliasSet" => c.alias_set = super::types::parse_alias_set(ctx)?,
                b"BaseContainer" => c.base_container = Some(parse_base_container(ctx, &e)?),
                b"EntryList" => c.entry_list = parse_entry_list(ctx, &e)?,
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</SequenceContainer>" })
            }
            _ => {}
        }
    }

    Ok(c)
}

// ─────────────────────────────────────────────────────────────────────────────
// BaseContainer and RestrictionCriteria
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a `<BaseContainer>` element (container inheritance reference).
pub(super) fn parse_base_container<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<BaseContainer, ParseError> {
    let container_ref =
        ctx.require_attr(start, "containerRef", "BaseContainer")?.into_owned();
    let mut restriction_criteria = None;

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"RestrictionCriteria" => {
                    restriction_criteria = Some(parse_restriction_criteria(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</BaseContainer>" }),
            _ => {}
        }
    }

    Ok(BaseContainer { container_ref, restriction_criteria })
}

/// Parse a `<RestrictionCriteria>` element.
///
/// Contains one of: `Comparison`, `ComparisonList`, `BooleanExpression`,
/// or `NextContainer`.
pub(super) fn parse_restriction_criteria<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<RestrictionCriteria, ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => {
                let rc = match e.local_name().as_ref() {
                    b"Comparison" => {
                        let cmp = parse_comparison(ctx, &e)?;
                        RestrictionCriteria::Comparison(cmp)
                    }
                    b"ComparisonList" => {
                        let list = parse_comparison_list(ctx, &e)?;
                        RestrictionCriteria::ComparisonList(list)
                    }
                    b"BooleanExpression" => {
                        let expr = parse_boolean_expression(ctx, &e)?;
                        RestrictionCriteria::BooleanExpression(expr)
                    }
                    b"NextContainer" => {
                        let container_ref =
                            ctx.require_attr(&e, "containerRef", "NextContainer")?.into_owned();
                        ctx.skip_element(&e)?;
                        RestrictionCriteria::NextContainer { container_ref }
                    }
                    _ => {
                        ctx.skip_element(&e)?;
                        continue;
                    }
                };
                // Drain the RestrictionCriteria End tag.
                loop {
                    match ctx.next()? {
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(ParseError::UnexpectedEof {
                                expected: "</RestrictionCriteria>",
                            })
                        }
                        _ => {}
                    }
                }
                return Ok(rc);
            }
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</RestrictionCriteria>" })
            }
            _ => {}
        }
    }
    Err(ParseError::UnexpectedElement(
        "RestrictionCriteria with no recognised child".into(),
    ))
}

/// Parse a `<ComparisonList>` element into a `Vec<Comparison>`.
fn parse_comparison_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<Comparison>, ParseError> {
    let mut list = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Comparison" => list.push(parse_comparison(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</ComparisonList>" }),
            _ => {}
        }
    }
    Ok(list)
}

/// Parse a `<Comparison>` element (attributes only — no child elements).
pub(super) fn parse_comparison<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Comparison, ParseError> {
    let parameter_ref =
        ctx.require_attr(start, "parameterRef", "Comparison")?.into_owned();
    let value = ctx.require_attr(start, "value", "Comparison")?.into_owned();
    let comparison_operator = ctx
        .get_attr(start, "comparisonOperator")
        .map(|v| parse_comparison_operator(&v))
        .transpose()?
        .unwrap_or_default();
    let use_calibrated_value = ctx
        .get_attr(start, "useCalibratedValue")
        .map(|v| v == "true")
        .unwrap_or(true);

    // Comparison has no child elements; drain to its End tag.
    ctx.skip_element(start)?;

    Ok(Comparison { parameter_ref, value, comparison_operator, use_calibrated_value })
}

/// Parse a `<BooleanExpression>` element recursively.
fn parse_boolean_expression<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<BooleanExpression, ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => {
                let expr = match e.local_name().as_ref() {
                    b"ANDedConditions" => {
                        let terms = parse_boolean_condition_list(ctx, &e)?;
                        BooleanExpression::And(terms)
                    }
                    b"ORedConditions" => {
                        let terms = parse_boolean_condition_list(ctx, &e)?;
                        BooleanExpression::Or(terms)
                    }
                    b"Condition" => {
                        // <Condition> has the same attributes as <Comparison>.
                        let cmp = parse_comparison(ctx, &e)?;
                        BooleanExpression::Condition(cmp)
                    }
                    _ => {
                        ctx.skip_element(&e)?;
                        continue;
                    }
                };
                // Drain the BooleanExpression End tag.
                loop {
                    match ctx.next()? {
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(ParseError::UnexpectedEof {
                                expected: "</BooleanExpression>",
                            })
                        }
                        _ => {}
                    }
                }
                return Ok(expr);
            }
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</BooleanExpression>" })
            }
            _ => {}
        }
    }
    Err(ParseError::UnexpectedElement(
        "BooleanExpression with no recognised child".into(),
    ))
}

/// Collect `<Condition>` children of an `<ANDedConditions>` or `<ORedConditions>` element.
fn parse_boolean_condition_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<BooleanExpression>, ParseError> {
    let mut terms = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Condition" => {
                    let cmp = parse_comparison(ctx, &e)?;
                    terms.push(BooleanExpression::Condition(cmp));
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</ANDedConditions>" })
            }
            _ => {}
        }
    }
    Ok(terms)
}

// ─────────────────────────────────────────────────────────────────────────────
// EntryList and entry types
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an `<EntryList>` element for a `SequenceContainer`.
pub(super) fn parse_entry_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<SequenceEntry>, ParseError> {
    let mut entries = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => {
                let entry = match e.local_name().as_ref() {
                    b"ParameterRefEntry" => {
                        SequenceEntry::ParameterRef(parse_parameter_ref_entry(ctx, &e)?)
                    }
                    b"ContainerRefEntry" => {
                        SequenceEntry::ContainerRef(parse_container_ref_entry(ctx, &e)?)
                    }
                    b"ArrayParameterRefEntry" => {
                        let parameter_ref =
                            ctx.require_attr(&e, "parameterRef", "ArrayParameterRefEntry")?
                                .into_owned();
                        let mut location = None;
                        loop {
                            match ctx.next()? {
                                Event::Start(e) => match e.local_name().as_ref() {
                                    b"LocationInContainerInBits" => {
                                        location = Some(parse_entry_location(ctx, &e)?)
                                    }
                                    _ => ctx.skip_element(&e)?,
                                },
                                Event::End(_) => break,
                                Event::Eof => {
                                    return Err(ParseError::UnexpectedEof {
                                        expected: "</ArrayParameterRefEntry>",
                                    })
                                }
                                _ => {}
                            }
                        }
                        SequenceEntry::ArrayParameterRef(ArrayParameterRefEntry {
                            parameter_ref,
                            location,
                        })
                    }
                    b"FixedValueEntry" => {
                        SequenceEntry::FixedValue(parse_fixed_value_entry(ctx, &e)?)
                    }
                    _ => {
                        ctx.skip_element(&e)?;
                        continue;
                    }
                };
                entries.push(entry);
            }
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</EntryList>" }),
            _ => {}
        }
    }
    Ok(entries)
}

/// Parse a `<ParameterRefEntry>` element.
pub(super) fn parse_parameter_ref_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ParameterRefEntry, ParseError> {
    let parameter_ref =
        ctx.require_attr(start, "parameterRef", "ParameterRefEntry")?.into_owned();
    let mut location = None;
    let mut include_condition = None;

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LocationInContainerInBits" => location = Some(parse_entry_location(ctx, &e)?),
                b"IncludeCondition" => {
                    include_condition = Some(parse_match_criteria(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</ParameterRefEntry>" })
            }
            _ => {}
        }
    }

    Ok(ParameterRefEntry { parameter_ref, location, include_condition })
}

/// Parse a `<ContainerRefEntry>` element.
fn parse_container_ref_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ContainerRefEntry, ParseError> {
    let container_ref =
        ctx.require_attr(start, "containerRef", "ContainerRefEntry")?.into_owned();
    let mut location = None;
    let mut include_condition = None;

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LocationInContainerInBits" => location = Some(parse_entry_location(ctx, &e)?),
                b"IncludeCondition" => {
                    include_condition = Some(parse_match_criteria(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</ContainerRefEntry>" })
            }
            _ => {}
        }
    }

    Ok(ContainerRefEntry { container_ref, location, include_condition })
}

/// Parse a `<FixedValueEntry>` element.
fn parse_fixed_value_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<FixedValueEntry, ParseError> {
    let size_in_bits =
        ctx.require_attr(start, "sizeInBits", "FixedValueEntry")?
            .parse::<u32>()
            .map_err(|_| ParseError::InvalidValue {
                attr: "sizeInBits",
                value: ctx.get_attr_owned(start, "sizeInBits").unwrap_or_default(),
                reason: "expected non-negative integer",
            })?;
    let binary_value = ctx.get_attr_owned(start, "binaryValue");
    let mut location = None;

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LocationInContainerInBits" => location = Some(parse_entry_location(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</FixedValueEntry>" })
            }
            _ => {}
        }
    }

    Ok(FixedValueEntry { size_in_bits, binary_value, location })
}

/// Parse a `<LocationInContainerInBits>` element into an `EntryLocation`.
pub(super) fn parse_entry_location<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<EntryLocation, ParseError> {
    let reference_location = match ctx
        .get_attr(start, "referenceLocation")
        .as_deref()
        .unwrap_or("previousEntry")
    {
        "containerStart" => ReferenceLocation::ContainerStart,
        _ => ReferenceLocation::PreviousEntry,
    };

    let mut bit_offset = 0i64;
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"FixedValue" => {
                    let text = ctx.read_text_content()?;
                    bit_offset = text.parse().map_err(|_| ParseError::InvalidValue {
                        attr: "FixedValue",
                        value: text.clone(),
                        reason: "expected integer bit offset",
                    })?;
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof {
                    expected: "</LocationInContainerInBits>",
                })
            }
            _ => {}
        }
    }

    Ok(EntryLocation { reference_location, bit_offset })
}

/// Parse a `<MatchCriteria>` / `<IncludeCondition>` element.
///
/// Same dispatch as `RestrictionCriteria` but maps to `MatchCriteria`
/// (which has no `NextContainer` variant).
fn parse_match_criteria<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<MatchCriteria, ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => {
                let mc = match e.local_name().as_ref() {
                    b"Comparison" => MatchCriteria::Comparison(parse_comparison(ctx, &e)?),
                    b"ComparisonList" => {
                        MatchCriteria::ComparisonList(parse_comparison_list(ctx, &e)?)
                    }
                    b"BooleanExpression" => {
                        MatchCriteria::BooleanExpression(parse_boolean_expression(ctx, &e)?)
                    }
                    _ => {
                        ctx.skip_element(&e)?;
                        continue;
                    }
                };
                // Drain the IncludeCondition End tag.
                loop {
                    match ctx.next()? {
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(ParseError::UnexpectedEof {
                                expected: "</IncludeCondition>",
                            })
                        }
                        _ => {}
                    }
                }
                return Ok(mc);
            }
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</IncludeCondition>" }),
            _ => {}
        }
    }
    Err(ParseError::UnexpectedElement(
        "IncludeCondition with no recognised child".into(),
    ))
}

// ─────────────────────────────────────────────────────────────────────────────
// Enum converters
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a `comparisonOperator` attribute string into a `ComparisonOperator`.
pub(super) fn parse_comparison_operator(raw: &str) -> Result<ComparisonOperator, ParseError> {
    match raw {
        "==" => Ok(ComparisonOperator::Equality),
        "!=" => Ok(ComparisonOperator::Inequality),
        "<" => Ok(ComparisonOperator::LessThan),
        "<=" => Ok(ComparisonOperator::LessThanOrEqual),
        ">" => Ok(ComparisonOperator::GreaterThan),
        ">=" => Ok(ComparisonOperator::GreaterThanOrEqual),
        _ => Err(ParseError::InvalidValue {
            attr: "comparisonOperator",
            value: raw.to_owned(),
            reason: "expected ==|!=|<|<=|>|>=",
        }),
    }
}
