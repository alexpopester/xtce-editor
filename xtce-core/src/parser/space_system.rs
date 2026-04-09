//! Parsers for `<SpaceSystem>` and `<Header>`.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};

use crate::model::space_system::{AuthorInfo, Header, SpaceSystem};
use crate::ParseError;

use super::context::ParseContext;

/// Parse a `<SpaceSystem>` element (including all children) from the current
/// position in `ctx`. `start` is the already-consumed opening tag.
pub(super) fn parse_space_system<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<SpaceSystem, ParseError> {
    let mut ss = SpaceSystem::new(
        ctx.require_attr(start, "name", "SpaceSystem")?.as_ref(),
    );
    ss.short_description = ctx.get_attr_owned(start, "shortDescription");

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LongDescription" => ss.long_description = Some(ctx.read_text_content()?),
                b"Header" => ss.header = Some(parse_header(ctx, &e)?),
                b"TelemetryMetaData" => {
                    ss.telemetry = Some(super::telemetry::parse_telemetry_metadata(ctx, &e)?)
                }
                b"CommandMetaData" => {
                    ss.command = Some(super::command::parse_command_metadata(ctx, &e)?)
                }
                b"SpaceSystem" => ss.sub_systems.push(parse_space_system(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</SpaceSystem>" }),
            _ => {}
        }
    }

    Ok(ss)
}

/// Parse a `<Header>` element. `start` is the already-consumed opening tag.
pub(super) fn parse_header<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Header, ParseError> {
    let mut header = Header {
        version: ctx.get_attr_owned(start, "version"),
        date: ctx.get_attr_owned(start, "date"),
        classification: ctx.get_attr_owned(start, "classification"),
        classification_instructions: ctx.get_attr_owned(start, "classificationInstructions"),
        validation_status: ctx.get_attr_owned(start, "validationStatus"),
        author_set: Vec::new(),
        note_set: Vec::new(),
    };

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"AuthorSet" => loop {
                    match ctx.next()? {
                        Event::Start(e) => match e.local_name().as_ref() {
                            // Legacy format: <AuthorInformation name="..." role="..."/>
                            b"AuthorInformation" => {
                                header.author_set.push(parse_author_info(ctx, &e)?)
                            }
                            // Standard XTCE format: <Author>name (role)</Author>
                            b"Author" => {
                                let text = ctx.read_text_content()?;
                                header.author_set.push(AuthorInfo { name: text, role: None });
                            }
                            _ => ctx.skip_element(&e)?,
                        },
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(ParseError::UnexpectedEof { expected: "</AuthorSet>" })
                        }
                        _ => {}
                    }
                },
                b"NoteSet" => loop {
                    match ctx.next()? {
                        Event::Start(e) => match e.local_name().as_ref() {
                            b"Note" => header.note_set.push(ctx.read_text_content()?),
                            _ => ctx.skip_element(&e)?,
                        },
                        Event::End(_) => break,
                        Event::Eof => {
                            return Err(ParseError::UnexpectedEof { expected: "</NoteSet>" })
                        }
                        _ => {}
                    }
                },
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</Header>" }),
            _ => {}
        }
    }

    Ok(header)
}

/// Parse an `<AuthorInformation>` element inside `<AuthorSet>`.
fn parse_author_info<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<AuthorInfo, ParseError> {
    let name = ctx.require_attr(start, "name", "AuthorInformation")?.into_owned();
    let role = ctx.get_attr_owned(start, "role");

    // AuthorInformation has no child elements; drain to its End tag.
    ctx.skip_element(start)?;

    Ok(AuthorInfo { name, role })
}
