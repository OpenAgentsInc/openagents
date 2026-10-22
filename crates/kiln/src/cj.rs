//! A minimal NIP-CJ conversation buyer, sans I/O: the page's own key,
//! beacon discovery and choice (NIP-PYLON), the sealed `25900` request,
//! and reading the `27000` feedback and `26900` result. The page's
//! WebSocket carries the frames; this module only builds and reads them.
//!
//! The choice follows `crates/pylon` (`client::choose` and
//! `group::serves_want`): a request that names a model goes only to a
//! fresh, online pylon or a ready group head serving that model; without
//! one, Kiln falls back to any fresh, online pylon with a free slot.

use nostr::domain::{Event, RelaySigner, Tag};
use nostr::nip44;
use nostr::pylon::{
    self, BEACON_MARKER, Beacon, BeaconBook, Freshness, GROUP_BEACON_MARKER, Lane, Readiness,
    Service, Status, deadline,
};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};

/// The conversation request kind.
pub const REQUEST_KIND: u16 = 25_900;
/// The conversation result kind.
pub const RESULT_KIND: u16 = 26_900;
/// The conversation feedback kind.
pub const FEEDBACK_KIND: u16 = 27_000;
/// The model Kiln asks for first.
pub const PREFERRED_MODEL: &str = "deepseek-v4.1-flash";
/// What a beacon that names no input limit accepts (`pylon::job`).
pub const LEGACY_MAX_INPUT_BYTES: usize = 16 * 1024;
/// The most request text any pylon accepts.
pub const MAX_INPUT_BYTES: usize = 96 * 1024;

/// The agent's key: generated in the page, never sent anywhere.
#[derive(Clone)]
pub struct Key {
    secret: SecretKey,
    signer: RelaySigner,
}

impl Key {
    /// The key for 32 secret bytes.
    ///
    /// # Errors
    ///
    /// When the bytes are not a valid secret key.
    pub fn from_bytes(bytes: [u8; 32]) -> Result<Self, String> {
        let secret = SecretKey::from_byte_array(bytes).map_err(|e| e.to_string())?;
        let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let signer = RelaySigner::from_secret_hex(&hex).map_err(|e| e.to_string())?;
        Ok(Self { secret, signer })
    }

    #[must_use]
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }

    #[must_use]
    pub fn npub(&self) -> String {
        let pk = self
            .secret
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0;
        nostr::nip19::encode_npub(&pk.serialize())
    }

    /// The NIP-42 `AUTH` event for `relay`'s `challenge`.
    #[must_use]
    pub fn auth(&self, relay: &str, challenge: &str, now: u64) -> Event {
        self.signer.sign(
            now,
            22_242,
            vec![
                Tag::new(vec!["relay".into(), relay.into()]),
                Tag::new(vec!["challenge".into(), challenge.into()]),
            ],
            String::new(),
        )
    }

    fn conversation_key(&self, peer: &str) -> Result<[u8; 32], String> {
        let peer: XOnlyPublicKey = peer.parse().map_err(|_| "peer key does not parse")?;
        Ok(nip44::conversation_key(&self.secret, &peer))
    }

    /// Encrypt `body` to `provider` as a signed `25900` request.
    ///
    /// # Errors
    ///
    /// When the provider key is invalid or the body is too large.
    pub fn seal_request(
        &self,
        provider: &str,
        body: &Value,
        nonce: [u8; 32],
        now: u64,
    ) -> Result<Event, String> {
        self.seal_request_tagged(provider, body, nonce, now, &[])
    }

    /// [`Self::seal_request`] with `extra` tags, such as an agent's `auth`
    /// and `mandate` tags.
    ///
    /// # Errors
    ///
    /// When the provider key is invalid or the body is too large.
    pub fn seal_request_tagged(
        &self,
        provider: &str,
        body: &Value,
        nonce: [u8; 32],
        now: u64,
        extra: &[Tag],
    ) -> Result<Event, String> {
        let key = self.conversation_key(provider)?;
        let content = nip44::encrypt(&body.to_string(), &key, nonce)?;
        let mut tags = vec![Tag::new(vec!["p".into(), provider.into()])];
        tags.extend(extra.iter().cloned());
        Ok(self.signer.sign(now, REQUEST_KIND, tags, content))
    }

    /// Encrypt `body` to `peer` and sign it as `kind` with `tags`.
    ///
    /// # Errors
    ///
    /// When the peer key is invalid or the body is too large.
    pub fn seal(
        &self,
        peer: &str,
        kind: u16,
        tags: Vec<Tag>,
        body: &Value,
        nonce: [u8; 32],
        now: u64,
    ) -> Result<Event, String> {
        let key = self.conversation_key(peer)?;
        let content = nip44::encrypt(&body.to_string(), &key, nonce)?;
        Ok(self.signer.sign(now, kind, tags, content))
    }

    /// Sign a plain (unencrypted) event.
    #[must_use]
    pub fn sign(&self, kind: u16, tags: Vec<Tag>, content: String, now: u64) -> Event {
        self.signer.sign(now, kind, tags, content)
    }

    /// Decrypt an answer from its signer.
    ///
    /// # Errors
    ///
    /// When it does not open under the pair's key.
    pub fn open(&self, event: &Event) -> Result<String, String> {
        let key = self.conversation_key(&event.pubkey)?;
        nip44::decrypt(&event.content, &key)
    }
}

/// The relay filter for every beacon, complete pylons and group heads.
#[must_use]
pub fn beacon_filter() -> Value {
    json!({"kinds": [pylon::BEACON_KIND], "#t": [BEACON_MARKER, GROUP_BEACON_MARKER], "limit": 500})
}

/// Add a beacon event to the book when it verifies.
pub fn offer(book: &mut BeaconBook, event: Event) -> bool {
    match pylon::parse_beacon(&event) {
        Ok(beacon) => book.offer(event, beacon),
        Err(_) => false,
    }
}

fn serves_model(service: &Service, want: &str) -> bool {
    let name = service
        .model
        .split_once('@')
        .map_or(service.model.as_str(), |(n, _)| n);
    service.model == want || name == want
}

fn usable(beacon: &Beacon, at: u64) -> bool {
    !beacon.is_stage()
        && pylon::freshness(beacon, at) == Freshness::Fresh
        && beacon.status == Status::Online
        && beacon.slots.free > 0
        && beacon
            .group_head()
            .is_none_or(|g| g.readiness == Some(Readiness::Ready))
}

/// The pylon and service for a job: the best one serving `want`, else
/// (when `fallback`) the best complete pylon on the conversation lane.
#[must_use]
pub fn choose(
    book: &BeaconBook,
    at: u64,
    want: Option<&str>,
    fallback: bool,
) -> Option<(Beacon, Service)> {
    let best = |pick: &dyn Fn(&Beacon) -> Option<Service>| {
        book.iter()
            .map(|(_, b)| b)
            .filter(|b| usable(b, at))
            .filter_map(|b| pick(b).map(|s| (b.clone(), s)))
            .max_by_key(|(b, _)| (b.slots.free, b.observed_at))
    };
    if let Some(want) = want
        && let Some(found) = best(&|b: &Beacon| {
            b.services
                .iter()
                .find(|s| s.lanes.contains(&Lane::CjConversation) && serves_model(s, want))
                .cloned()
        })
    {
        return Some(found);
    }
    if !fallback && want.is_some() {
        return None;
    }
    best(&|b: &Beacon| {
        if b.group.is_some() {
            return None;
        }
        b.serves(Lane::CjConversation).cloned()
    })
}

/// Whether a fresh pylon serves `want` at all, busy or not: the reason to
/// wait for a slot rather than fall back to another model.
#[must_use]
pub fn serves_somewhere(book: &BeaconBook, at: u64, want: &str) -> bool {
    book.iter().map(|(_, b)| b).any(|b| {
        !b.is_stage()
            && pylon::freshness(b, at) == Freshness::Fresh
            && matches!(b.status, Status::Online | Status::Draining)
            && b.services
                .iter()
                .any(|s| s.lanes.contains(&Lane::CjConversation) && serves_model(s, want))
    })
}

/// A summary of the book for the page: each usable pylon's label and models.
#[must_use]
pub fn describe(book: &BeaconBook, at: u64) -> Vec<String> {
    book.iter()
        .map(|(_, b)| b)
        .filter(|b| usable(b, at))
        .map(|b| {
            let models: Vec<&str> = b
                .services
                .iter()
                .filter(|s| s.lanes.contains(&Lane::CjConversation))
                .map(|s| s.model.as_str())
                .collect();
            format!("{} [{}] free {}", b.label, models.join(", "), b.slots.free)
        })
        .collect()
}

/// Bytes of request text a prompt token stands for, conservatively: code and
/// JSON run about 3 to 4 bytes a token.
pub const BYTES_PER_TOKEN: u64 = 3;

/// The most request text the pylon behind `beacon` accepts: its byte limit,
/// and for a group head also its prompt-token limit at
/// [`BYTES_PER_TOKEN`].
#[must_use]
pub fn input_limit(beacon: &Beacon) -> usize {
    let meta = beacon.meta.as_ref();
    let named = if beacon.group.is_some() {
        let limits = meta.and_then(|m| m.get("request_limits"));
        let bytes = limits
            .and_then(|l| l.get("max_input_bytes"))
            .and_then(Value::as_u64)
            .unwrap_or(MAX_INPUT_BYTES as u64);
        let tokens = limits
            .and_then(|l| l.get("max_input_tokens"))
            .and_then(Value::as_u64)
            .map_or(u64::MAX, |t| t.saturating_mul(BYTES_PER_TOKEN));
        Some(bytes.min(tokens))
    } else {
        meta.and_then(|m| m.get("max_input_bytes"))
            .and_then(Value::as_u64)
    };
    named.map_or(LEGACY_MAX_INPUT_BYTES, |b| {
        usize::try_from(b).map_or(MAX_INPUT_BYTES, |b| b.min(MAX_INPUT_BYTES))
    })
}

/// The token limit in a pylon's `limit_exceeded` refusal ("this group takes
/// at most N"), when it names one.
#[must_use]
pub fn refused_token_limit(message: &str) -> Option<u64> {
    let rest = message.split("takes at most ").nth(1)?;
    rest.split(|c: char| !c.is_ascii_digit())
        .next()?
        .parse()
        .ok()
}

/// The deadline the beacon declares, past the ordinary 90 seconds.
#[must_use]
pub fn declared_deadline(beacon: &Beacon) -> Option<u64> {
    beacon
        .group_head()
        .and_then(|g| g.deadline_secs)
        .or_else(|| {
            beacon
                .meta
                .as_ref()?
                .get("deadline_secs")?
                .as_u64()
                .filter(|n| (1..=deadline::MAX_SECS).contains(n))
        })
}

/// The request body: the system turn, the prompt as the task, and, when
/// the pylon declares a longer deadline, `pylon.deadline.v1` with the
/// buyer's budget held to it. Returns the body and the seconds to wait.
#[must_use]
pub fn request_body(
    beacon: &Beacon,
    instructions: &str,
    prompt: &str,
    budget_secs: u64,
) -> (Value, u64) {
    let mut body = json!({
        "v": 1,
        "requires": [],
        "task": prompt,
        "transcript": [{"role": "user", "content": prompt}],
        "instructions": instructions,
        "client": concat!("kiln ", env!("CARGO_PKG_VERSION")),
    });
    let ceiling = declared_deadline(beacon);
    let wait = match ceiling {
        Some(c) if c > deadline::ORDINARY_SECS => {
            let secs = deadline::effective(budget_secs, Some(c));
            deadline::declare(&mut body, secs);
            secs
        }
        _ => deadline::ORDINARY_SECS,
    };
    (body, wait)
}

/// The subscription filter for answers to `request`.
#[must_use]
pub fn answer_filter(me: &str, request: &str) -> Value {
    json!({"kinds": [RESULT_KIND, FEEDBACK_KIND], "#p": [me], "#e": [request]})
}

/// What one answer event says.
#[derive(Debug, Clone, PartialEq)]
pub enum Answer {
    /// `processing` feedback, with a progress line when the worker sent one.
    Processing(Option<String>),
    Result {
        text: String,
        model: Option<String>,
        usage: Option<Value>,
        /// The model's reasoning, when the pylon sends it apart from the
        /// answer.
        reasoning: Option<String>,
    },
    Error(String),
    Other,
}

/// Whether `event` answers `request` from `provider` to `me`.
#[must_use]
pub fn answers(event: &Event, request: &str, provider: &str, me: &str) -> bool {
    let p: Vec<_> = event.tag_values("p").collect();
    let e: Vec<_> = event.tag_values("e").collect();
    event.pubkey == provider
        && matches!(event.kind, RESULT_KIND | FEEDBACK_KIND)
        && p == [me]
        && e == [request]
        && event.validate_crypto().is_ok()
}

/// Read a decrypted answer body.
#[must_use]
pub fn read(kind: u16, plain: &str) -> Answer {
    let Ok(value) = serde_json::from_str::<Value>(plain) else {
        return Answer::Other;
    };
    if let Err(refusal) = deadline::read_response(&value, true) {
        return Answer::Error(format!("{}: {}", refusal.code, refusal.message));
    }
    if kind == RESULT_KIND && value["type"] == "result" {
        return match value["text"].as_str().filter(|t| !t.is_empty()) {
            Some(text) => Answer::Result {
                text: text.to_string(),
                model: value["model"].as_str().map(str::to_string),
                usage: value.get("usage").cloned(),
                reasoning: value["reasoning"]
                    .as_str()
                    .filter(|r| !r.trim().is_empty())
                    .map(str::to_string),
            },
            None => Answer::Error("the result is empty".into()),
        };
    }
    if value["status"] == "error" {
        return Answer::Error(format!(
            "{}: {}",
            value["code"].as_str().unwrap_or("error"),
            value["message"].as_str().unwrap_or_default()
        ));
    }
    if value["status"] == "processing" {
        return Answer::Processing(value["progress"].as_str().map(str::to_string));
    }
    Answer::Other
}

/// Strip a model's `<think>` block and chat-template leftovers.
#[must_use]
pub fn clean(text: &str) -> String {
    let mut out = text.to_string();
    while let (Some(start), Some(end)) = (out.find("<think>"), out.find("</think>")) {
        if end < start {
            break;
        }
        out.replace_range(start..end + "</think>".len(), "");
    }
    if let Some(cut) = out.find("<|im_start|>") {
        out.truncate(cut);
    }
    out.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(byte: u8) -> Key {
        Key::from_bytes([byte; 32]).unwrap()
    }

    fn beacon(provider: &Key, model: &str, at: u64, group: bool) -> Event {
        let mut value = json!({
            "v": if group { pylon::BEACON_V2 } else { pylon::BEACON_V },
            "requires": [],
            "provider": provider.pubkey(),
            "pylon": "p1",
            "label": "test",
            "status": "online",
            "generation": 1,
            "since": at - 10,
            "observed_at": at,
            "valid_until": at + 120,
            "class": {"family": "gpu", "tier": "large", "memory_gb": 32},
            "slots": {"total": 2, "free": 1},
            "services": [{"capability": "pylon/conversation", "model": model, "lanes": ["cj-conversation"], "offering": null, "price_hint_msat": null}],
            "settlement": ["free"],
            "pools": [],
        });
        if group {
            value["slots"] = json!({"total": 1, "free": 1});
            value["services"][0]["model"] = json!(format!("{model}@sha256:{}", "a".repeat(64)));
            value["group"] = json!({"id": "ds", "role": "head", "generation": 1, "artifact": format!("sha256:{}", "a".repeat(64)), "readiness": "ready", "deadline_secs": 600, "max_output_tokens": 4096});
        }
        let beacon: Beacon = serde_json::from_value(value).unwrap();
        pylon::beacon_event(&provider.signer, &beacon).unwrap()
    }

    #[test]
    fn the_preferred_model_wins_and_any_pylon_is_the_fallback() {
        let at = 1_800_000_000;
        let (small, big) = (key(2), key(3));
        let mut book = BeaconBook::default();
        assert!(offer(
            &mut book,
            beacon(&small, "qwen3.5-0.8b-q8_0", at, false)
        ));
        let (b, s) = choose(&book, at, Some(PREFERRED_MODEL), true).unwrap();
        assert_eq!(
            (b.provider.as_str(), s.model.as_str()),
            (small.pubkey(), "qwen3.5-0.8b-q8_0")
        );
        assert!(choose(&book, at, Some(PREFERRED_MODEL), false).is_none());
        assert!(offer(
            &mut book,
            beacon(&big, "deepseek-v4.1-flash", at, true)
        ));
        let (b, _) = choose(&book, at, Some(PREFERRED_MODEL), true).unwrap();
        assert_eq!(b.provider, big.pubkey());
        let (body, wait) = request_body(&b, "sys", "hi", 300);
        assert_eq!(wait, 300);
        assert_eq!(body["requires"][0], deadline::FEATURE);
        assert_eq!(input_limit(&b), MAX_INPUT_BYTES);
        assert_eq!(
            refused_token_limit(
                "limit_exceeded: the prompt is 4216 tokens; this group takes at most 2872"
            ),
            Some(2872)
        );
        assert!(serves_somewhere(&book, at, PREFERRED_MODEL));
        assert!(!serves_somewhere(&book, at, "other"));
        assert!(choose(&book, at + 1_000, None, true).is_none());
    }

    #[test]
    fn a_request_round_trips_to_the_provider() {
        let (buyer, provider) = (key(5), key(6));
        let event = beacon(&provider, "m", 1_800_000_000, false);
        let beacon = pylon::parse_beacon(&event).unwrap();
        assert_eq!(input_limit(&beacon), LEGACY_MAX_INPUT_BYTES);
        let (body, wait) = request_body(&beacon, "sys", "write fib", 300);
        assert_eq!(wait, 90);
        let request = buyer
            .seal_request(provider.pubkey(), &body, [9; 32], 1)
            .unwrap();
        let plain = provider.open(&request).unwrap();
        assert!(plain.contains("write fib"));
        assert_eq!(buyer.auth("wss://r", "c", 1).kind, 22_242);
    }

    #[test]
    fn answers_read_and_clean() {
        let result = r#"{"v":1,"requires":[],"type":"result","text":"<think>\nhm\n</think>\n{\"a\":1}<|im_start|>user","model":"m"}"#;
        let Answer::Result { text, .. } = read(RESULT_KIND, result) else {
            panic!("not a result");
        };
        assert_eq!(clean(&text), "{\"a\":1}");
        let refused = r#"{"v":1,"requires":[],"type":"status","status":"error","code":"busy","message":"full"}"#;
        assert_eq!(
            read(FEEDBACK_KIND, refused),
            Answer::Error("busy: full".into())
        );
        let busy = r#"{"v":1,"requires":[],"type":"status","status":"processing"}"#;
        assert_eq!(read(FEEDBACK_KIND, busy), Answer::Processing(None));
    }
}
