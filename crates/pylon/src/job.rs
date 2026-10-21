//! The NIP-CJ conversation lane as a pylon speaks it: a `25900` request
//! NIP-44 encrypted to the pylon, `27000` feedback, and a `26900` result
//! encrypted back to the buyer, each bound to the request by its `e` tag.
//!
//! A request may name `pylon.deadline.v1` ([`nostr::pylon::deadline`]) with
//! the buyer's budget; any other feature is refused.

use nostr::domain::{Event, Tag};
use nostr::nip44;
use nostr::pylon::deadline::{self, Profile};
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
pub const MAX_INPUT_BYTES: usize = 96 * 1024;
/// The encoded JSON must also fit the encrypted transport, including
/// duplicated task text, history, and JSON escapes.
pub const MAX_REQUEST_BYTES: usize = nip44::MAX_CLIENT_PLAINTEXT_BYTES;
/// The most request text a pylon built before #12414 accepts, inside a
/// standard NIP-44 body: what a buyer assumes of a beacon that names no
/// [`INPUT_LIMIT`].
pub const LEGACY_MAX_INPUT_BYTES: usize = 16 * 1024;
/// The beacon `meta` key under which a complete pylon names the most
/// request text it accepts.
pub const INPUT_LIMIT: &str = "max_input_bytes";

/// The most request text the pylon behind `beacon` accepts, never more
/// than [`MAX_INPUT_BYTES`]: a group head's request limits, else
/// [`MAX_INPUT_BYTES`] (groups came after the limit was raised); a complete
/// pylon's [`INPUT_LIMIT`], else [`LEGACY_MAX_INPUT_BYTES`].
#[must_use]
pub fn input_limit(beacon: &nostr::pylon::Beacon) -> usize {
    let named = if beacon.group.is_some() {
        Some(
            crate::group::request_limits(beacon)
                .map_or(MAX_INPUT_BYTES as u64, |l| l.max_input_bytes),
        )
    } else {
        beacon
            .meta
            .as_ref()
            .and_then(|meta| meta.get(INPUT_LIMIT))
            .and_then(Value::as_u64)
    };
    named.map_or(LEGACY_MAX_INPUT_BYTES, |bytes| {
        usize::try_from(bytes).map_or(MAX_INPUT_BYTES, |b| b.min(MAX_INPUT_BYTES))
    })
}

/// A decoded conversation request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    pub version: u64,
    pub turns: Vec<Turn>,
    /// Ordinary, or the declared-deadline extension with the buyer's budget.
    pub profile: Profile,
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
    if plaintext.len() > MAX_REQUEST_BYTES {
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
    let profile = deadline::request_profile(&body, true).map_err(|refusal| Refusal {
        code: refusal.code,
        message: refusal.message,
        retry_after_ms: None,
    })?;
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
            "request text is empty or over 96 KiB",
        ));
    }
    Ok(Request {
        version,
        turns,
        profile,
    })
}

/// [`request_body`] with the declared-deadline extension: the buyer's own
/// budget of `budget_secs`.
#[must_use]
pub fn request_body_within(prompt: &str, history: &[Turn], budget_secs: u64) -> Value {
    let mut body = request_body(prompt, history);
    deadline::declare(&mut body, budget_secs);
    body
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_prompt_round_trips_through_extended_nip44_without_clipping() {
        let buyer = Identity::generate();
        let provider = Identity::generate();
        let prompt = format!("{}\nEND-OF-PROMPT", "let x = 1;\n".repeat(6500));
        assert!(prompt.len() > 65535 && prompt.len() < MAX_INPUT_BYTES);
        let body = request_body_within(&prompt, &[], 1800);
        let event = seal(&buyer, provider.pubkey(), REQUEST_KIND, vec![], &body, 1).unwrap();
        let plaintext = open(&provider, &event).unwrap();
        let request = parse_request(&plaintext).unwrap();
        assert_eq!(request.turns.last().unwrap().content, prompt);
        assert!(
            parse_request(&request_body(&"x".repeat(MAX_INPUT_BYTES + 1), &[]).to_string())
                .is_err()
        );
        // Escapes can overflow the encrypted body before the text cap.
        assert!(
            parse_request(&request_body(&"\0".repeat(MAX_INPUT_BYTES), &[]).to_string()).is_err()
        );
    }
}
