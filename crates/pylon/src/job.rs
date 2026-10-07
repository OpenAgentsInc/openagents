//! The NIP-CJ conversation lane as a pylon speaks it: a `25900` request
//! NIP-44 encrypted to the pylon, `27000` feedback, and a `26900` result
//! encrypted back to the buyer, each bound to the request by its `e` tag.

use nostr::domain::{Event, Tag};
use nostr::nip44;
use secp256k1::XOnlyPublicKey;
use serde_json::{Value, json};

use crate::engine::Turn;
use crate::identity::{Identity, parse_pubkey};

/// The conversation request kind.
pub const REQUEST_KIND: u16 = 25_900;
/// The conversation result kind.
pub const RESULT_KIND: u16 = 26_900;
/// The conversation feedback kind.
pub const FEEDBACK_KIND: u16 = 27_000;

/// The most transcript entries a pylon accepts.
pub const MAX_TURNS: usize = 32;
/// The most request text, in bytes, a pylon accepts.
pub const MAX_INPUT_BYTES: usize = 16 * 1024;

/// A decoded conversation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub version: u64,
    pub turns: Vec<Turn>,
}

/// A refusal a pylon sends as `status: error` feedback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refusal {
    pub code: &'static str,
    pub message: String,
    pub retry_after_ms: Option<u64>,
}

impl Refusal {
    pub fn new(code: &'static str, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            retry_after_ms: None,
        }
    }
}

fn nonce() -> [u8; 32] {
    secp256k1::rand::random()
}

fn conversation_key(me: &Identity, peer: &str) -> Result<[u8; 32], String> {
    let peer: XOnlyPublicKey = parse_pubkey(peer).ok_or("peer key does not parse")?;
    Ok(nip44::conversation_key(me.secret(), &peer))
}

/// Encrypt `body` to `peer` and sign it as `kind` with `tags`.
///
/// # Errors
///
/// When the peer key is invalid or the body is too large.
pub fn seal(
    me: &Identity,
    peer: &str,
    kind: u16,
    tags: Vec<Tag>,
    body: &Value,
    created_at: u64,
) -> Result<Event, String> {
    let key = conversation_key(me, peer)?;
    let content = nip44::encrypt(&body.to_string(), &key, nonce())?;
    Ok(me.signer().sign(created_at, kind, tags, content))
}

/// Decrypt an event addressed to `me` from its signer.
///
/// # Errors
///
/// When the ciphertext does not open under the pair's conversation key.
pub fn open(me: &Identity, event: &Event) -> Result<String, String> {
    let key = conversation_key(me, &event.pubkey)?;
    nip44::decrypt(&event.content, &key)
}

/// The conversation request body for one prompt.
#[must_use]
pub fn request_body(prompt: &str, history: &[Turn]) -> Value {
    let mut transcript: Vec<Value> = history
        .iter()
        .map(|t| json!({"role": t.role, "content": t.content}))
        .collect();
    transcript.push(json!({"role": "user", "content": prompt}));
    json!({
        "v": 1,
        "requires": [],
        "task": prompt,
        "transcript": transcript,
        "client": concat!("openagents-pylon ", env!("CARGO_PKG_VERSION")),
    })
}

/// Parse and bound a decrypted request body.
///
/// # Errors
///
/// A refusal with the NIP-CJ code for what is wrong.
pub fn parse_request(plaintext: &str) -> Result<Request, Refusal> {
    if plaintext.len() > 4 * MAX_INPUT_BYTES {
        return Err(Refusal::new("limit_exceeded", "request is too large"));
    }
    let body: Value = serde_json::from_str(plaintext)
        .map_err(|_| Refusal::new("malformed", "request is not JSON"))?;
    let version = body["v"].as_u64().unwrap_or(0);
    if !matches!(version, 1 | 2) {
        return Err(Refusal::new(
            "unsupported_version",
            "conversation version 1 or 2 only",
        ));
    }
    match body["requires"].as_array() {
        Some(requires) if requires.is_empty() => {}
        Some(_) => {
            return Err(Refusal::new(
                "unsupported_feature",
                "this pylon requires no features",
            ));
        }
        None => return Err(Refusal::new("malformed", "requires is missing")),
    }
    let task = body["task"]
        .as_str()
        .ok_or_else(|| Refusal::new("malformed", "task is missing"))?;
    let mut turns = Vec::new();
    if let Some(instructions) = body["instructions"].as_str()
        && !instructions.is_empty()
    {
        turns.push(Turn {
            role: "system".into(),
            content: instructions.into(),
        });
    }
    let transcript = body["transcript"].as_array().cloned().unwrap_or_default();
    if transcript.len() > MAX_TURNS {
        return Err(Refusal::new(
            "limit_exceeded",
            "transcript has more than 32 entries",
        ));
    }
    for entry in &transcript {
        let role = entry["role"].as_str().unwrap_or_default();
        if role != "user" && role != "assistant" {
            return Err(Refusal::new(
                "malformed",
                "transcript role is user or assistant",
            ));
        }
        let content = entry["content"]
            .as_str()
            .ok_or_else(|| Refusal::new("malformed", "transcript content is a string"))?;
        turns.push(Turn {
            role: role.into(),
            content: content.into(),
        });
    }
    if turns
        .last()
        .is_none_or(|t| t.role != "user" || t.content != task)
    {
        turns.push(Turn {
            role: "user".into(),
            content: task.into(),
        });
    }
    let bytes: usize = turns.iter().map(|t| t.content.len()).sum();
    if bytes > MAX_INPUT_BYTES || task.trim().is_empty() {
        return Err(Refusal::new(
            "limit_exceeded",
            "request text is empty or over 16 KiB",
        ));
    }
    Ok(Request { version, turns })
}

/// The `e` and `p` tags of a response to `request`.
#[must_use]
pub fn response_tags(request: &Event) -> Vec<Tag> {
    vec![
        Tag::new(vec!["p".into(), request.pubkey.clone()]),
        Tag::new(vec!["e".into(), request.id.clone()]),
    ]
}

/// A `status` feedback body.
#[must_use]
pub fn status_body(version: u64, status: &str) -> Value {
    json!({"v": version, "requires": [], "type": "status", "status": status})
}

/// An error feedback body for `refusal`.
#[must_use]
pub fn refusal_body(version: u64, refusal: &Refusal) -> Value {
    let mut body = json!({
        "v": version,
        "requires": [],
        "type": "status",
        "status": "error",
        "code": refusal.code,
        "message": refusal.message,
    });
    if let Some(ms) = refusal.retry_after_ms {
        body["retry_after_ms"] = json!(ms);
    }
    body
}

/// Whether `event` answers `request` from `pylon` to `me`: the signer, kind,
/// one `p` naming the buyer, and one `e` naming the request.
#[must_use]
pub fn answers(event: &Event, request_id: &str, pylon: &str, me: &str) -> bool {
    let p: Vec<_> = event.tag_values("p").collect();
    let e: Vec<_> = event.tag_values("e").collect();
    event.pubkey == pylon
        && matches!(event.kind, RESULT_KIND | FEEDBACK_KIND)
        && p == [me]
        && e == [request_id]
        && event.validate_crypto().is_ok()
}
