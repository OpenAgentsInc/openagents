//! What travels inside a direct channel's data frames.
//!
//! A message is one JSON object whose `v` names its type. NIP-TERM bodies
//! keep their own `v`. NIP-HOST requests and replies travel as the exact
//! signed `3188` events of the direct artifact binding, wrapped in a call or
//! an answer, so admission, retention, and retries are identical to the
//! relay binding. A ping tests the channel, and a closing message tells the
//! device why the host is about to close it.
//!
//! A message longer than one data frame is split into fragments. Each data
//! frame starts with one flag byte: `1` when more fragments follow and `0`
//! for the last. A message is at most [`MAX_MESSAGE_BYTES`] bytes.

use coder_pty::wire::{
    ATTACH, Attach, CLOSE, Close, DETACH, Detach, FRAME, Frame, INPUT, Input, OPEN, Open, RESIZE,
    RESULT, Reason, Refusal, Resize, SIGNAL, Signal, TerminalResult,
};
use coder_reach::channel::MAX_DATA_BYTES;
use nostr::domain::Event;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// A device's signed NIP-HOST request.
pub const CALL: &str = "openagents.host-call.v1";
/// The host's signed NIP-HOST reply.
pub const ANSWER: &str = "openagents.host-answer.v1";
/// A liveness check. The host answers with a pong carrying the same nonce.
pub const PING: &str = "openagents.host-ping.v1";
pub const PONG: &str = "openagents.host-pong.v1";
/// The host's last message before it closes a channel whose grant or
/// generation stopped admitting it.
pub const CLOSING: &str = "openagents.host-closing.v1";

/// The largest reassembled message.
pub const MAX_MESSAGE_BYTES: usize = 256 * 1024;
const MORE: u8 = 1;
const LAST: u8 = 0;

/// Why a message could not be framed or read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MessageError {
    /// Larger than [`MAX_MESSAGE_BYTES`].
    TooLarge,
    /// Not a known message, or a known one with the wrong shape.
    Malformed,
}

/// Split one message into data-frame payloads.
///
/// # Errors
/// Refuses a message over [`MAX_MESSAGE_BYTES`].
pub fn fragments(bytes: &[u8]) -> Result<Vec<Vec<u8>>, MessageError> {
    if bytes.len() > MAX_MESSAGE_BYTES {
        return Err(MessageError::TooLarge);
    }
    let chunk = MAX_DATA_BYTES - 1;
    let count = bytes.len().div_ceil(chunk).max(1);
    Ok((0..count)
        .map(|index| {
            let part = &bytes[index * chunk..bytes.len().min((index + 1) * chunk)];
            let mut payload = Vec::with_capacity(part.len() + 1);
            payload.push(if index + 1 == count { LAST } else { MORE });
            payload.extend_from_slice(part);
            payload
        })
        .collect())
}

/// Reassembles fragments into messages.
#[derive(Debug, Default)]
pub struct Assembler {
    buffer: Vec<u8>,
}

impl Assembler {
    /// Add one data-frame payload. Returns the message it completes.
    ///
    /// # Errors
    /// Refuses an empty payload, an unknown flag, or a message that grows
    /// past [`MAX_MESSAGE_BYTES`]. The caller closes the channel.
    pub fn push(&mut self, payload: &[u8]) -> Result<Option<Vec<u8>>, MessageError> {
        let (&flag, rest) = payload.split_first().ok_or(MessageError::Malformed)?;
        if self.buffer.len() + rest.len() > MAX_MESSAGE_BYTES {
            return Err(MessageError::TooLarge);
        }
        self.buffer.extend_from_slice(rest);
        match flag {
            LAST => Ok(Some(std::mem::take(&mut self.buffer))),
            MORE => Ok(None),
            _ => Err(MessageError::Malformed),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Wrapped {
    v: String,
    event: Event,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Nonce {
    v: String,
    nonce: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Closing {
    v: String,
    code: String,
}

/// A NIP-TERM request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TermRequest {
    Open(Open),
    Attach(Attach),
    Detach(Detach),
    Input(Input),
    Resize(Resize),
    Signal(Signal),
    Close(Close),
}

impl TermRequest {
    /// Parse a body by its `v`.
    ///
    /// # Errors
    /// Refuses an unknown version or a body that does not match its schema.
    pub fn from_value(value: Value) -> Result<Self, Refusal> {
        let bad = || {
            Refusal::new(
                Reason::Malformed,
                "terminal request does not match its schema",
            )
        };
        let v = value.get("v").and_then(Value::as_str).ok_or_else(bad)?;
        let parsed = match v {
            OPEN => serde_json::from_value(value).map(Self::Open),
            ATTACH => serde_json::from_value(value).map(Self::Attach),
            DETACH => serde_json::from_value(value).map(Self::Detach),
            INPUT => serde_json::from_value(value).map(Self::Input),
            RESIZE => serde_json::from_value(value).map(Self::Resize),
            SIGNAL => serde_json::from_value(value).map(Self::Signal),
            CLOSE => serde_json::from_value(value).map(Self::Close),
            _ => {
                return Err(Refusal::new(
                    Reason::UnsupportedVersion,
                    "unknown terminal request",
                ));
            }
        };
        parsed.map_err(|_| bad())
    }

    /// The request ID the result answers.
    #[must_use]
    pub fn request(&self) -> &str {
        match self {
            Self::Open(r) => &r.request,
            Self::Attach(r) => &r.request,
            Self::Detach(r) => &r.request,
            Self::Input(r) => &r.request,
            Self::Resize(r) => &r.request,
            Self::Signal(r) => &r.request,
            Self::Close(r) => &r.request,
        }
    }

    /// The body's schema.
    #[must_use]
    pub fn schema(&self) -> &'static str {
        match self {
            Self::Open(_) => OPEN,
            Self::Attach(_) => ATTACH,
            Self::Detach(_) => DETACH,
            Self::Input(_) => INPUT,
            Self::Resize(_) => RESIZE,
            Self::Signal(_) => SIGNAL,
            Self::Close(_) => CLOSE,
        }
    }

    /// The body as JSON.
    #[must_use]
    pub fn to_value(&self) -> Value {
        let value = match self {
            Self::Open(r) => serde_json::to_value(r),
            Self::Attach(r) => serde_json::to_value(r),
            Self::Detach(r) => serde_json::to_value(r),
            Self::Input(r) => serde_json::to_value(r),
            Self::Resize(r) => serde_json::to_value(r),
            Self::Signal(r) => serde_json::to_value(r),
            Self::Close(r) => serde_json::to_value(r),
        };
        value.unwrap_or(Value::Null)
    }
}

/// A message from a device to the host.
#[derive(Clone, Debug)]
pub enum ToHost {
    Call(Event),
    Ping(String),
    Terminal(TermRequest),
}

/// A message from the host to a device.
#[derive(Clone, Debug)]
pub enum ToDevice {
    Answer(Event),
    Pong(String),
    Closing(String),
    Result(TerminalResult),
    Frame(Frame),
}

impl ToHost {
    /// Encode as message bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let value = match self {
            Self::Call(event) => json!({"v": CALL, "event": event}),
            Self::Ping(nonce) => json!({"v": PING, "nonce": nonce}),
            Self::Terminal(request) => request.to_value(),
        };
        value.to_string().into_bytes()
    }

    /// Decode message bytes.
    ///
    /// # Errors
    /// Refuses unknown or malformed messages. A terminal body with the wrong
    /// shape returns its NIP-TERM refusal so the host can answer it.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let value = nostr::contracts::parse_strict_bounded(bytes, MAX_MESSAGE_BYTES)
            .map_err(|_| DecodeError::Malformed)?;
        match value.get("v").and_then(Value::as_str) {
            Some(CALL) => serde_json::from_value::<Wrapped>(value)
                .map(|w| Self::Call(w.event))
                .map_err(|_| DecodeError::Malformed),
            Some(PING) => nonce(value).map(Self::Ping),
            Some(v) if v.starts_with("openagents.terminal-") => {
                let request = value
                    .get("request")
                    .and_then(Value::as_str)
                    .filter(|id| coder_pty::wire::is_common_id(id))
                    .map(str::to_owned);
                TermRequest::from_value(value)
                    .map(Self::Terminal)
                    .map_err(|refusal| DecodeError::Terminal(request, refusal))
            }
            _ => Err(DecodeError::Malformed),
        }
    }
}

impl ToDevice {
    /// Encode as message bytes.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let value = match self {
            Self::Answer(event) => json!({"v": ANSWER, "event": event}),
            Self::Pong(nonce) => json!({"v": PONG, "nonce": nonce}),
            Self::Closing(code) => json!({"v": CLOSING, "code": code}),
            Self::Result(result) => serde_json::to_value(result).unwrap_or(Value::Null),
            Self::Frame(frame) => serde_json::to_value(frame).unwrap_or(Value::Null),
        };
        value.to_string().into_bytes()
    }

    /// Decode message bytes.
    ///
    /// # Errors
    /// Refuses unknown or malformed messages.
    pub fn decode(bytes: &[u8]) -> Result<Self, DecodeError> {
        let value = nostr::contracts::parse_strict_bounded(bytes, MAX_MESSAGE_BYTES)
            .map_err(|_| DecodeError::Malformed)?;
        let malformed = |_| DecodeError::Malformed;
        match value.get("v").and_then(Value::as_str) {
            Some(ANSWER) => serde_json::from_value::<Wrapped>(value)
                .map(|w| Self::Answer(w.event))
                .map_err(malformed),
            Some(PONG) => nonce(value).map(Self::Pong),
            Some(CLOSING) => serde_json::from_value::<Closing>(value)
                .map(|c| Self::Closing(c.code))
                .map_err(malformed),
            Some(RESULT) => serde_json::from_value(value)
                .map(Self::Result)
                .map_err(malformed),
            Some(FRAME) => {
                let frame: Frame = serde_json::from_value(value).map_err(malformed)?;
                frame.check().map_err(|_| DecodeError::Malformed)?;
                Ok(Self::Frame(frame))
            }
            _ => Err(DecodeError::Malformed),
        }
    }
}

/// Why message bytes did not decode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DecodeError {
    Malformed,
    /// A terminal body that failed its own validation, with its request ID
    /// when the body carried a valid one.
    Terminal(Option<String>, Refusal),
}

fn nonce(value: Value) -> Result<String, DecodeError> {
    let body: Nonce = serde_json::from_value(value).map_err(|_| DecodeError::Malformed)?;
    if body.nonce.is_empty() || body.nonce.len() > 64 {
        return Err(DecodeError::Malformed);
    }
    Ok(body.nonce)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fragments_reassemble_and_bounds_hold() {
        let big: Vec<u8> = (0..40_000u32).map(|n| (n % 251) as u8).collect();
        let parts = fragments(&big).unwrap();
        assert_eq!(parts.len(), 3);
        assert!(parts.iter().all(|p| p.len() <= MAX_DATA_BYTES));
        let mut assembler = Assembler::default();
        assert_eq!(assembler.push(&parts[0]).unwrap(), None);
        assert_eq!(assembler.push(&parts[1]).unwrap(), None);
        assert_eq!(assembler.push(&parts[2]).unwrap().unwrap(), big);
        // An empty message is one final fragment.
        assert_eq!(fragments(b"").unwrap(), vec![vec![LAST]]);
        assert_eq!(
            fragments(&vec![0; MAX_MESSAGE_BYTES + 1]).unwrap_err(),
            MessageError::TooLarge
        );
        let mut assembler = Assembler::default();
        assert_eq!(assembler.push(&[]).unwrap_err(), MessageError::Malformed);
        assert_eq!(
            assembler.push(&[9, 1]).unwrap_err(),
            MessageError::Malformed
        );
        let mut assembler = Assembler::default();
        let chunk = vec![MORE; MAX_DATA_BYTES];
        let mut refused = false;
        for _ in 0..(MAX_MESSAGE_BYTES / MAX_DATA_BYTES + 2) {
            if assembler.push(&chunk) == Err(MessageError::TooLarge) {
                refused = true;
                break;
            }
        }
        assert!(refused);
    }

    #[test]
    fn messages_round_trip_and_unknown_ones_refuse() {
        let ping = ToHost::Ping("abc".into()).encode();
        assert!(matches!(ToHost::decode(&ping), Ok(ToHost::Ping(n)) if n == "abc"));
        let closing = ToDevice::Closing("revoked".into()).encode();
        assert!(matches!(ToDevice::decode(&closing), Ok(ToDevice::Closing(c)) if c == "revoked"));
        assert_eq!(
            ToHost::decode(br#"{"v":"openagents.other.v1"}"#).unwrap_err(),
            DecodeError::Malformed
        );
        assert_eq!(
            ToHost::decode(br#"{"v":"openagents.host-ping.v1","nonce":"a","x":1}"#).unwrap_err(),
            DecodeError::Malformed
        );
        let id = "a".repeat(64);
        let bad = format!(r#"{{"v":"openagents.terminal-input.v1","request":"{id}"}}"#);
        match ToHost::decode(bad.as_bytes()).unwrap_err() {
            DecodeError::Terminal(Some(request), refusal) => {
                assert_eq!(request, id);
                assert_eq!(refusal.reason, Reason::Malformed);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
}
