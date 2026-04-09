//! Parsers for `<CommandMetaData>`, `<MetaCommandSet>`, `<MetaCommand>`,
//! argument lists, and command containers.

use std::io::BufRead;

use quick_xml::events::{BytesStart, Event};

use crate::model::command::{
    Argument, ArgumentRefEntry, CommandContainer, CommandEntry, CommandMetaData, FixedValueEntry,
    MetaCommand,
};
use crate::model::container::SequenceEntry;
use crate::ParseError;

use super::context::ParseContext;

// ─────────────────────────────────────────────────────────────────────────────
// Top-level dispatcher
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a `<CommandMetaData>` element and all of its children
/// (ArgumentTypeSet, MetaCommandSet).
pub(super) fn parse_command_metadata<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<CommandMetaData, ParseError> {
    let mut command = CommandMetaData::default();

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"ArgumentTypeSet" => {
                    super::types::parse_argument_type_set(ctx, &mut command)?
                }
                b"MetaCommandSet" => parse_meta_command_set(ctx, &mut command)?,
                b"CommandContainerSet" => {
                    parse_command_container_set(ctx, &mut command)?
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</CommandMetaData>" })
            }
            _ => {}
        }
    }

    Ok(command)
}

// ─────────────────────────────────────────────────────────────────────────────
// MetaCommandSet and MetaCommand
// ─────────────────────────────────────────────────────────────────────────────

/// Populate `command.meta_commands` by consuming a `<MetaCommandSet>` element.
pub(super) fn parse_meta_command_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    command: &mut CommandMetaData,
) -> Result<(), ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"MetaCommand" => {
                    let mc = parse_meta_command(ctx, &e)?;
                    command.meta_commands.insert(mc.name.clone(), mc);
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</MetaCommandSet>" })
            }
            _ => {}
        }
    }
    Ok(())
}

/// Parse a single `<MetaCommand>` element.
pub(super) fn parse_meta_command<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<MetaCommand, ParseError> {
    let mut mc =
        MetaCommand::new(ctx.require_attr(start, "name", "MetaCommand")?.as_ref());
    mc.short_description = ctx.get_attr_owned(start, "shortDescription");
    mc.r#abstract = ctx
        .get_attr(start, "abstract")
        .map(|v| v == "true")
        .unwrap_or(false);
    mc.base_meta_command = ctx.get_attr_owned(start, "baseMetaCommand");

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LongDescription" => mc.long_description = Some(ctx.read_text_content()?),
                b"ArgumentList" => mc.argument_list = parse_argument_list(ctx, &e)?,
                b"CommandContainer" => {
                    mc.command_container = Some(parse_command_container(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</MetaCommand>" }),
            _ => {}
        }
    }

    Ok(mc)
}

// ─────────────────────────────────────────────────────────────────────────────
// ArgumentList and Argument
// ─────────────────────────────────────────────────────────────────────────────

/// Parse an `<ArgumentList>` element into a `Vec<Argument>`.
pub(super) fn parse_argument_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<Argument>, ParseError> {
    let mut args = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"Argument" => args.push(parse_argument(ctx, &e)?),
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => return Err(ParseError::UnexpectedEof { expected: "</ArgumentList>" }),
            _ => {}
        }
    }
    Ok(args)
}

/// Parse a single `<Argument>` element.
fn parse_argument<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Argument, ParseError> {
    let name = ctx.require_attr(start, "name", "Argument")?.into_owned();
    let argument_type_ref =
        ctx.require_attr(start, "argumentTypeRef", "Argument")?.into_owned();
    let mut arg = Argument::new(name, argument_type_ref);
    arg.short_description = ctx.get_attr_owned(start, "shortDescription");
    arg.initial_value = ctx.get_attr_owned(start, "initialValue");

    // Argument has no child elements; drain to its End tag.
    ctx.skip_element(start)?;
    Ok(arg)
}

// ─────────────────────────────────────────────────────────────────────────────
// CommandContainer and entry list
// ─────────────────────────────────────────────────────────────────────────────

/// Parse a `<CommandContainer>` element.
pub(super) fn parse_command_container<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<CommandContainer, ParseError> {
    let name = ctx.require_attr(start, "name", "CommandContainer")?.into_owned();
    let mut cc = CommandContainer { name, base_container: None, entry_list: Vec::new() };

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"BaseContainer" => {
                    cc.base_container =
                        Some(super::container::parse_base_container(ctx, &e)?)
                }
                b"EntryList" => cc.entry_list = parse_command_entry_list(ctx, &e)?,
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</CommandContainer>" })
            }
            _ => {}
        }
    }

    Ok(cc)
}

/// Parse an `<EntryList>` inside a `<CommandContainer>` into `Vec<CommandEntry>`.
fn parse_command_entry_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<CommandEntry>, ParseError> {
    let mut entries = Vec::new();
    loop {
        match ctx.next()? {
            Event::Start(e) => {
                let entry = match e.local_name().as_ref() {
                    b"ArgumentRefEntry" => {
                        CommandEntry::ArgumentRef(parse_argument_ref_entry(ctx, &e)?)
                    }
                    b"ParameterRefEntry" => {
                        // Reuse the container parser; wrap the result.
                        let pe = super::container::parse_parameter_ref_entry(ctx, &e)?;
                        CommandEntry::ParameterRef(SequenceEntry::ParameterRef(pe))
                    }
                    b"FixedValueEntry" => {
                        CommandEntry::FixedValue(parse_fixed_value_entry(ctx, &e)?)
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

/// Parse an `<ArgumentRefEntry>` element.
fn parse_argument_ref_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ArgumentRefEntry, ParseError> {
    let argument_ref =
        ctx.require_attr(start, "argumentRef", "ArgumentRefEntry")?.into_owned();
    let mut location = None;

    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"LocationInContainerInBits" => {
                    location = Some(super::container::parse_entry_location(ctx, &e)?)
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</ArgumentRefEntry>" })
            }
            _ => {}
        }
    }

    Ok(ArgumentRefEntry { argument_ref, location })
}

/// Parse a `<FixedValueEntry>` element in a command context.
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
                b"LocationInContainerInBits" => {
                    location = Some(super::container::parse_entry_location(ctx, &e)?)
                }
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

// ─────────────────────────────────────────────────────────────────────────────
// CommandContainerSet
// ─────────────────────────────────────────────────────────────────────────────

/// Populate `command.command_containers` by consuming a `<CommandContainerSet>`
/// element. Each `<SequenceContainer>` child is a shared command packet layout.
fn parse_command_container_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    command: &mut CommandMetaData,
) -> Result<(), ParseError> {
    loop {
        match ctx.next()? {
            Event::Start(e) => match e.local_name().as_ref() {
                b"SequenceContainer" => {
                    let c = super::container::parse_sequence_container(ctx, &e)?;
                    command.command_containers.insert(c.name.clone(), c);
                }
                _ => ctx.skip_element(&e)?,
            },
            Event::End(_) => break,
            Event::Eof => {
                return Err(ParseError::UnexpectedEof { expected: "</CommandContainerSet>" })
            }
            _ => {}
        }
    }
    Ok(())
}
