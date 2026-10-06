//! Bounded shell metadata. Output marks describe a terminal; they grant no authority.

use std::collections::VecDeque;

pub const MAX_EVENTS: usize = 256;
pub const MAX_TEXT: usize = 8192;

/// An advisory mark anchored to a primary-screen absolute line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mark {
    pub line: u64,
    pub col: usize,
    pub event: Event,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Prompt,
    Input,
    Output,
    Finished {
        status: Option<i32>,
    },
    Directory(String),
    Command(String),
    Buffer(String),
    /// The shell's `whence -w` report on the line's first word.
    Word(String),
    /// The shell's command table: `PATH`, alias names, function names.
    Table(String),
    Request(String),
    /// Metadata was lost. Consumers must abandon incomplete blocks.
    Gap,
}

#[derive(Default)]
pub(crate) struct Metadata {
    pub events: VecDeque<Mark>,
}

impl Metadata {
    pub fn push(&mut self, line: u64, col: usize, event: Event) {
        if self.events.len() >= MAX_EVENTS {
            self.events.clear();
            self.events.push_back(Mark {
                line,
                col,
                event: Event::Gap,
            });
        }
        self.events.push_back(Mark { line, col, event });
    }
}

pub(crate) fn parse(params: &[&[u8]]) -> Option<Event> {
    match params {
        [b"133", b"A", ..] => Some(Event::Prompt),
        [b"133", b"B", ..] => Some(Event::Input),
        [b"133", b"C", ..] => Some(Event::Output),
        [b"133", b"D", rest @ ..] => {
            let status = match rest.first() {
                Some(bytes) => Some(std::str::from_utf8(bytes).ok()?.parse::<i32>().ok()?),
                None => None,
            };
            Some(Event::Finished { status })
        }
        [b"7", uri] if uri.len() <= MAX_TEXT => {
            let uri = std::str::from_utf8(uri).ok()?.strip_prefix("file://")?;
            let slash = uri.find('/')?;
            let path = decode_percent(&uri[slash..])?;
            Some(Event::Directory(path))
        }
        // A table keeps the newlines that separate its parts, which the
        // other kinds refuse, so it decodes on its own.
        [b"777", b"openagents", b"table", hex] if hex.len() <= MAX_TEXT * 2 => {
            decode_table(hex).map(Event::Table)
        }
        [b"777", b"openagents", kind, hex] if hex.len() <= MAX_TEXT * 2 => {
            let text = decode_hex(hex)?;
            match *kind {
                b"command" => Some(Event::Command(text)),
                b"buffer" => Some(Event::Buffer(text)),
                b"word" => Some(Event::Word(text)),
                b"request" => Some(Event::Request(text)),
                _ => None,
            }
        }
        _ => None,
    }
}

/// fish's native OSC 133 carries its command before the output boundary.
pub(crate) fn output_command(params: &[&[u8]]) -> Option<String> {
    let [b"133", b"C", rest @ ..] = params else {
        return None;
    };
    let mut commands = rest
        .iter()
        .filter_map(|field| field.strip_prefix(b"cmdline_url="));
    let encoded = commands.next()?;
    if commands.next().is_some() || encoded.len() > MAX_TEXT * 3 {
        return None;
    }
    let command =
        String::from_utf8(decode_percent_data(std::str::from_utf8(encoded).ok()?)?).ok()?;
    (!command.is_empty()
        && command.len() <= MAX_TEXT
        && !command
            .chars()
            .any(|character| character.is_control() && character != '\n' && character != '\t'))
    .then_some(command)
}

fn digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn printable(bytes: Vec<u8>) -> Option<String> {
    let text = String::from_utf8(bytes).ok()?;
    (!text.chars().any(char::is_control)).then_some(text)
}

/// A table keeps its newlines, which separate its parts.
fn decode_table(hex: &[u8]) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes = hex
        .chunks_exact(2)
        .map(|pair| Some(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect::<Option<Vec<_>>>()?;
    let text = String::from_utf8(bytes).ok()?;
    (!text.chars().any(|c| c.is_control() && c != '\n')).then_some(text)
}

fn decode_hex(hex: &[u8]) -> Option<String> {
    if !hex.len().is_multiple_of(2) {
        return None;
    }
    let bytes = hex
        .chunks_exact(2)
        .map(|pair| Some(digit(pair[0])? * 16 + digit(pair[1])?))
        .collect::<Option<Vec<_>>>()?;
    printable(bytes)
}

fn decode_percent(text: &str) -> Option<String> {
    printable(decode_percent_data(text)?)
}

fn decode_percent_data(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len());
    let mut source = text.as_bytes().iter().copied();
    while let Some(byte) = source.next() {
        bytes.push(if byte == b'%' {
            digit(source.next()?)? * 16 + digit(source.next()?)?
        } else {
            byte
        });
    }
    Some(bytes)
}

#[cfg(test)]
mod native_command_tests {
    use super::*;
    #[test]
    fn native_output_command_is_bounded_and_unambiguous() {
        assert_eq!(
            output_command(&[b"133", b"C", b"cmdline_url=printf%20hello"]),
            Some("printf hello".into())
        );
        assert_eq!(output_command(&[b"133", b"C", b"cmdline_url=%00"]), None);
        assert_eq!(
            output_command(&[b"133", b"C", b"cmdline_url=if%20true%0Aecho%20ok%0Aend"]),
            Some("if true\necho ok\nend".into())
        );
        assert_eq!(output_command(&[b"133", b"C", b"cmdline_url=%1b"]), None);
        assert_eq!(
            output_command(&[b"133", b"C", b"cmdline_url=a", b"cmdline_url=b"]),
            None
        );
        let large = format!("cmdline_url={}", "x".repeat(MAX_TEXT + 1));
        assert_eq!(output_command(&[b"133", b"C", large.as_bytes()]), None);
    }
}
