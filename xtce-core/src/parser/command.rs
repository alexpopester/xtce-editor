//! Parsers for `<CommandMetaData>`, `<MetaCommandSet>`, `<MetaCommand>`,
//! argument lists, and command containers.

use std::io::BufRead;

use quick_xml::events::BytesStart;

use crate::model::command::{
    Argument, ArgumentRefEntry, CommandContainer, CommandEntry, CommandMetaData, FixedValueEntry,
    MetaCommand,
};
use crate::model::container::{BaseContainer, EntryLocation};
use crate::ParseError;

use super::context::ParseContext;

/// Parse a `<CommandMetaData>` element and all of its children
/// (ArgumentTypeSet, MetaCommandSet).
pub(super) fn parse_command_metadata<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<CommandMetaData, ParseError> {
    todo!("create empty CommandMetaData, loop over children, dispatch to sub-parsers")
}

/// Populate `command.meta_commands` by consuming a `<MetaCommandSet>` element.
pub(super) fn parse_meta_command_set<R: BufRead>(
    ctx: &mut ParseContext<R>,
    command: &mut CommandMetaData,
) -> Result<(), ParseError> {
    todo!("loop over <MetaCommand> children, insert into command.meta_commands")
}

/// Parse a single `<MetaCommand>` element.
pub(super) fn parse_meta_command<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<MetaCommand, ParseError> {
    todo!("parse name, abstract, baseMetaCommand attrs; dispatch ArgumentList and CommandContainer children")
}

/// Parse an `<ArgumentList>` element into a `Vec<Argument>`.
pub(super) fn parse_argument_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<Argument>, ParseError> {
    todo!("loop over <Argument> children")
}

/// Parse a single `<Argument>` element.
fn parse_argument<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<Argument, ParseError> {
    todo!("parse name, argumentTypeRef, initialValue attrs")
}

/// Parse a `<CommandContainer>` element.
pub(super) fn parse_command_container<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<CommandContainer, ParseError> {
    todo!("parse name attr; optionally parse BaseContainer child; parse EntryList")
}

/// Parse an `<EntryList>` inside a `<CommandContainer>` into `Vec<CommandEntry>`.
fn parse_command_entry_list<R: BufRead>(
    ctx: &mut ParseContext<R>,
    _start: &BytesStart<'_>,
) -> Result<Vec<CommandEntry>, ParseError> {
    todo!("loop over ArgumentRefEntry / ParameterRefEntry / FixedValueEntry children")
}

/// Parse an `<ArgumentRefEntry>` element.
fn parse_argument_ref_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<ArgumentRefEntry, ParseError> {
    todo!("parse argumentRef attr; optionally parse LocationInContainerInBits")
}

/// Parse a `<FixedValueEntry>` element in a command context.
fn parse_fixed_value_entry<R: BufRead>(
    ctx: &mut ParseContext<R>,
    start: &BytesStart<'_>,
) -> Result<FixedValueEntry, ParseError> {
    todo!("parse sizeInBits and binaryValue attrs")
}
