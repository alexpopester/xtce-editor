//! ParseContext — shared reader wrapper for the recursive-descent parser.

use std::borrow::Cow;
use std::io::BufRead;

use quick_xml::NsReader;
use quick_xml::events::{BytesStart, Event};

use crate::ParseError;

/// Wraps `NsReader` with a reusable buffer and convenience helpers used by
/// every sub-parser.
pub(crate) struct ParseContext<R: BufRead> {
    pub(crate) reader: NsReader<R>,
    buf: Vec<u8>,
}

impl<R: BufRead> ParseContext<R> {
    /// Create a new `ParseContext` wrapping the given `NsReader`.
    ///
    /// # Reader configuration
    ///
    /// Two options are set here that establish a contract relied upon by every
    /// sub-parser in this crate:
    ///
    /// - `expand_empty_elements(true)`: self-closing tags (`<Foo/>`) are
    ///   transparently expanded into a `Start` + `End` pair. Sub-parsers
    ///   therefore never observe `Event::Empty` and can always assume that a
    ///   `Start` event is followed by zero or more children and then an `End`.
    ///
    /// - `trim_text(true)`: leading and trailing whitespace is stripped from
    ///   `Event::Text` payloads. Combined with the whitespace-skipping in
    ///   [`Self::next`], this means parsers never encounter whitespace-only
    ///   text nodes between tags.
    pub(crate) fn new(mut reader: NsReader<R>) -> Self {
        {
            let config = reader.config_mut();
            config.expand_empty_elements = true;
            config.trim_text(true);
        }
        Self {
            reader,
            buf: Vec::new(),
        }
    }

    /// Read the next meaningful event, skipping comments, processing
    /// instructions, and whitespace-only text nodes. Returns an owned
    /// (heap-allocated) event so callers are not tied to the buffer's
    /// lifetime.
    ///
    /// Uses `read_resolved_event_into` so namespace prefixes are stripped from
    /// element names — parsers dispatch on local names only and work regardless
    /// of whether the document uses an `xtce:` prefix or none.
    pub(crate) fn next(&mut self) -> Result<Event<'static>, ParseError> {
        loop {
            self.buf.clear();
            let (_, event) = self.reader.read_resolved_event_into(&mut self.buf)?;
            match event {
                // Skip noise — none of these carry semantic meaning for XTCE.
                Event::Comment(_) | Event::PI(_) | Event::DocType(_) => continue,
                // With trim_text(true) set, whitespace-only inter-element text
                // is trimmed to empty. Skip those; return real text content.
                Event::Text(ref t) if t.is_empty() => continue,
                other => return Ok(other.into_owned()),
            }
        }
    }

    /// Extract a required attribute value from a start element.
    ///
    /// Returns `ParseError::MissingAttribute` if the attribute is absent.
    /// XML entities in the value are unescaped (e.g. `&amp;` → `&`).
    pub(crate) fn require_attr<'a>(
        &self,
        start: &'a BytesStart<'_>,
        name: &'static str,
        element: &'static str,
    ) -> Result<Cow<'a, str>, ParseError> {
        self.get_attr(start, name)
            .ok_or(ParseError::MissingAttribute { element, attr: name })
    }

    /// Extract an optional attribute value and immediately convert it to an
    /// owned `String`. Shorthand for `get_attr(...).map(|v| v.into_owned())`.
    pub(crate) fn get_attr_owned(&self, start: &BytesStart<'_>, name: &'static str) -> Option<String> {
        self.get_attr(start, name).map(|v| v.into_owned())
    }

    /// Extract an optional attribute value. Returns `None` if absent.
    /// XML entities in the value are unescaped (e.g. `&amp;` → `&`).
    pub(crate) fn get_attr<'a>(
        &self,
        start: &'a BytesStart<'_>,
        name: &'static str,
    ) -> Option<Cow<'a, str>> {
        start
            .attributes()
            .filter_map(|res| res.ok())
            .find(|attr| attr.key.local_name().as_ref() == name.as_bytes())
            .and_then(|attr| attr.decode_and_unescape_value(self.reader.decoder()).ok())
    }

    /// Read the text content of an element whose `Start` tag has already been
    /// consumed, returning the unescaped string and consuming the closing `End`.
    ///
    /// Three cases:
    /// - Non-empty `Text` → unescape, return the string, consume `End`.
    /// - `End` immediately → element was empty (`<Foo/>` or `<Foo></Foo>`),
    ///   returns an empty string.
    /// - `Eof` → `ParseError::UnexpectedEof`.
    pub(crate) fn read_text_content(&mut self) -> Result<String, ParseError> {
        match self.next()? {
            Event::Text(t) => {
                let content = t.unescape().map_err(quick_xml::Error::from)?.into_owned();
                // Consume the closing End tag.
                match self.next()? {
                    Event::End(_) => {}
                    Event::Eof => {
                        return Err(ParseError::UnexpectedEof { expected: "closing tag" })
                    }
                    _ => {} // unexpected but not fatal — caller's loop will handle it
                }
                Ok(content)
            }
            Event::End(_) => Ok(String::new()),
            Event::Eof => Err(ParseError::UnexpectedEof { expected: "text content or closing tag" }),
            _ => Ok(String::new()),
        }
    }

    /// Consume and discard all events up to and including the `End` event
    /// matching `start`. Used to skip unknown elements gracefully.
    ///
    /// Delegates to `read_to_end_into` which correctly tracks nesting depth,
    /// so elements with the same name as `start` nested inside are handled
    /// without a manual depth counter. This works correctly because
    /// `expand_empty_elements(true)` ensures every `Start` has a matching
    /// `End` — there are no `Empty` events to miscount.
    pub(crate) fn skip_element(&mut self, start: &BytesStart<'_>) -> Result<(), ParseError> {
        self.reader.read_to_end_into(start.to_end().name(), &mut self.buf)?;
        Ok(())
    }
}
