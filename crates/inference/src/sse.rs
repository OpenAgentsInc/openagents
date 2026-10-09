//! Server-sent events: the byte-level codec and the Open Responses stream
//! on top of it.
//!
//! [`SseDecoder`] follows the WHATWG event-stream rules: lines end in
//! `\n`, `\r\n`, or `\r`; `:` lines are comments; `data` lines join with
//! `\n`; a blank line dispatches. Bytes may arrive split anywhere, inside
//! a UTF-8 character included. [`ResponsesDecoder`] turns frames into
//! [`Event`]s and the `[DONE]` sentinel. The encoders write what the spec
//! asks of a server: `event:` equal to the body's `type`, one `data:` line
//! of JSON, no `id:`, and a final `data: [DONE]`.

use crate::event::Event;

/// The terminal frame of an Open Responses or Chat Completions stream.
pub const DONE_FRAME: &str = "data: [DONE]\n\n";

/// One dispatched server-sent event.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Frame {
    /// The `event:` field, when one was sent.
    pub event: Option<String>,
    /// The `data:` lines, joined with `\n`.
    pub data: String,
}

/// An incremental event-stream parser.
#[derive(Debug, Default)]
pub struct SseDecoder {
    pending: Vec<u8>,
    /// A `\r` ended the last chunk; a `\n` starting the next belongs to it.
    after_cr: bool,
    event: Option<String>,
    data: Option<String>,
}

impl SseDecoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds bytes; returns the frames they complete.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Frame> {
        let mut frames = Vec::new();
        let mut bytes = bytes;
        if self.after_cr {
            self.after_cr = false;
            if let Some(rest) = bytes.strip_prefix(b"\n") {
                bytes = rest;
            }
        }
        for (index, &byte) in bytes.iter().enumerate() {
            match byte {
                b'\n' => {
                    // A `\n` straight after a `\r` in this chunk was
                    // consumed with the `\r`.
                    if index > 0 && bytes[index - 1] == b'\r' {
                        continue;
                    }
                    self.line(&mut frames);
                }
                b'\r' => {
                    self.line(&mut frames);
                    if index + 1 == bytes.len() {
                        self.after_cr = true;
                    }
                }
                _ => self.pending.push(byte),
            }
        }
        frames
    }

    /// Ends the stream. A frame whose blank line never came is dispatched
    /// anyway, since a server that closes after its last `data:` line has
    /// still said what it meant.
    pub fn finish(&mut self) -> Option<Frame> {
        let mut frames = Vec::new();
        if !self.pending.is_empty() {
            self.line(&mut frames);
        }
        self.dispatch(&mut frames);
        frames.pop()
    }

    fn line(&mut self, frames: &mut Vec<Frame>) {
        let line = String::from_utf8_lossy(&self.pending).into_owned();
        self.pending.clear();
        if line.is_empty() {
            self.dispatch(frames);
            return;
        }
        if line.starts_with(':') {
            return;
        }
        let (field, value) = match line.split_once(':') {
            Some((field, value)) => (field, value.strip_prefix(' ').unwrap_or(value)),
            None => (line.as_str(), ""),
        };
        match field {
            "event" => self.event = Some(value.to_owned()),
            "data" => match &mut self.data {
                Some(data) => {
                    data.push('\n');
                    data.push_str(value);
                }
                None => self.data = Some(value.to_owned()),
            },
            // `id` and `retry` mean nothing to this API.
            _ => {}
        }
    }

    fn dispatch(&mut self, frames: &mut Vec<Frame>) {
        let event = self.event.take();
        if let Some(data) = self.data.take() {
            frames.push(Frame { event, data });
        }
    }
}

/// Encodes one frame. Multi-line data becomes several `data:` lines.
#[must_use]
pub fn encode_frame(event: Option<&str>, data: &str) -> String {
    let mut out = String::with_capacity(data.len() + 32);
    if let Some(event) = event {
        out.push_str("event: ");
        out.push_str(event);
        out.push('\n');
    }
    for line in data.split('\n') {
        out.push_str("data: ");
        out.push_str(line);
        out.push('\n');
    }
    out.push('\n');
    out
}

/// Encodes an Open Responses event: `event: <type>` and its JSON.
#[must_use]
pub fn encode_event(event: &Event) -> String {
    let data = serde_json::to_string(event).unwrap_or_else(|error| {
        // Serializing our own types cannot fail on valid values; if it
        // does, say so in-band rather than tearing the stream.
        serde_json::json!({
            "type": "error",
            "sequence_number": event.sequence_number,
            "error": {"type": "server_error", "code": null, "param": null,
                      "message": format!("could not encode event: {error}")},
        })
        .to_string()
    });
    encode_frame(Some(event.type_name()), &data)
}

/// What one frame of an Open Responses stream held.
#[derive(Clone, Debug, PartialEq)]
pub enum StreamItem {
    Event(Event),
    /// `data: [DONE]`.
    Done,
}

/// A frame that was not a valid event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecodeError {
    pub frame: Frame,
    pub reason: String,
}

impl std::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} in frame {:?}", self.reason, self.frame.data)
    }
}

impl std::error::Error for DecodeError {}

/// Decodes one frame of an Open Responses stream.
pub fn decode_frame(frame: Frame) -> Result<StreamItem, DecodeError> {
    if frame.data.trim() == "[DONE]" {
        return Ok(StreamItem::Done);
    }
    let event: Event = match serde_json::from_str(&frame.data) {
        Ok(event) => event,
        Err(error) => {
            return Err(DecodeError {
                frame,
                reason: error.to_string(),
            });
        }
    };
    if let Some(name) = &frame.event
        && name != event.type_name()
    {
        let reason = format!(
            "`event: {name}` does not match the body's type `{}`",
            event.type_name()
        );
        return Err(DecodeError { frame, reason });
    }
    Ok(StreamItem::Event(event))
}

/// An incremental Open Responses stream decoder.
#[derive(Debug, Default)]
pub struct ResponsesDecoder {
    sse: SseDecoder,
}

impl ResponsesDecoder {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Feeds bytes; returns what they complete.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<Result<StreamItem, DecodeError>> {
        self.sse.push(bytes).into_iter().map(decode_frame).collect()
    }

    /// Ends the stream.
    pub fn finish(&mut self) -> Option<Result<StreamItem, DecodeError>> {
        self.sse.finish().map(decode_frame)
    }

    /// Decodes a whole recorded stream.
    pub fn decode_all(bytes: &[u8]) -> Vec<Result<StreamItem, DecodeError>> {
        let mut decoder = Self::new();
        let mut out = decoder.push(bytes);
        out.extend(decoder.finish());
        out
    }
}
