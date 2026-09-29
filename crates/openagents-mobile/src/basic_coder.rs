//! The basic Coder: the hosted chat a conversation starts with, before any
//! computer is connected.
//!
//! The phone holds no model key. It sends each turn as a NIP-CJ
//! conversation job (kind `25900`), signed by this device's key and
//! encrypted (NIP-44) to the OpenAgents chat worker, through
//! `relay.openagents.com`. The worker is `coder-worker` on the gateway
//! door's Gemini Flash lane, open to every caller under a per-key quota
//! (`coder::relay::quota`); it streams the reply back as `27000` partial
//! feedback and one `26900` result. The relay sees ciphertext and routing
//! tags only. Read `docs/deployment/chat-worker.md` for the serving path and
//! its limits.
//!
//! [`Reading`] checks every answer before it is shown: signed by the worker,
//! addressed to this device, bound to the request, decrypted, and, for a
//! partial, next in sequence. A gap stops the preview; the result replaces
//! it whole.

use crate::basic_link::Link;
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::kinds::{CJ_CONVERSATION_FEEDBACK, CJ_CONVERSATION_REQUEST, CJ_CONVERSATION_RESULT};
use nostr::nip44;
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// The relay the basic Coder's jobs travel through.
pub(crate) const RELAY: &str = "wss://relay.openagents.com";

/// The OpenAgents chat worker's public key. It is public: the worker's
/// secret, and the model key it spends, stay on the worker's host.
pub(crate) const WORKER: &str = "32c078952ff8b1f1d6f431e30fb240b0d1f91e30f977844557e8267390e3599b";

/// What the basic Coder is told about itself. The worker's own limits
/// outrank it; it grants nothing.
pub(crate) const INSTRUCTIONS: &str = "You are Coder, the OpenAgents assistant, \
chatting with the user in the OpenAgents app on their phone. Answer directly and \
helpfully; use Markdown when it helps, and keep answers short on a small screen. \
In this chat you cannot run commands, read files, or reach the user's computer. \
When the user asks for work that needs a computer, such as running code or \
reading or changing a repository, say that plainly in one sentence and tell them \
to tap Run Coder below the chat: it starts Coder on their connected computer with \
this conversation, or helps them connect one first.";

/// How long the worker has to answer at all, connection included.
const CONTACT: Duration = Duration::from_secs(30);
/// The longest one job may take, connection included.
const LIFETIME: Duration = Duration::from_secs(120);
/// The most conversation text one job sends, newest turns first.
pub(crate) const MAX_TRANSCRIPT_BYTES: usize = 48 * 1024;
/// The most streamed text one reply shows.
const MAX_REPLY_BYTES: usize = 256 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Role {
    User,
    Assistant,
}

/// One message of a basic conversation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Turn {
    pub role: Role,
    pub text: String,
}

/// Where a turn belongs, as the worker's typed judgment (NIP-CJ
/// `judgment`, its `lane`) says: an optional observation that grants
/// nothing. The phone offers Run Coder on a computer beside a reply the
/// judgment placed on a computer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Lane {
    Chat,
    Computer,
}

/// The most candidates one rank job orders (NIP-CJ `rank`).
pub(crate) const MAX_RANK_CANDIDATES: usize = 16;

/// Why a reply did not arrive.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum Failure {
    /// The worker answered with a typed refusal.
    Refused {
        code: String,
        message: String,
        retry_after_ms: Option<u64>,
    },
    /// Nothing came back in time.
    Silent,
    /// The relay could not be reached or dropped the job.
    Transport(String),
}

impl Failure {
    /// What the chat says, from the refusal's code, never its words alone.
    pub(crate) fn describe(&self) -> String {
        match self {
            Failure::Refused {
                code,
                retry_after_ms,
                message,
            } => match code.as_str() {
                "rate_limited" => format!(
                    "You're sending messages quickly. Try again in {}.",
                    wait(retry_after_ms.unwrap_or(60_000))
                ),
                "quota_exhausted" => format!(
                    "Coder has answered all the messages it can today. Try again in {}, \
                     or run Coder on your computer.",
                    wait(retry_after_ms.unwrap_or(3_600_000))
                ),
                "limit_exceeded" => {
                    "This conversation is too long for Coder here. Start a new chat.".into()
                }
                "busy" => "Coder is busy. Try again in a moment.".into(),
                _ => format!("Coder couldn't answer ({code}): {message}"),
            },
            Failure::Silent => "Coder didn't answer. Try again.".into(),
            Failure::Transport(_) => "Coder couldn't be reached. Check your connection.".into(),
        }
    }
}

/// A wait in words: seconds under two minutes, else minutes, else hours.
fn wait(ms: u64) -> String {
    let seconds = ms.div_ceil(1_000).max(1);
    match seconds {
        1 => "a second".into(),
        2..=119 => format!("{seconds} seconds"),
        120..=7_199 => format!("{} minutes", seconds.div_ceil(60)),
        _ => format!("{} hours", seconds.div_ceil(3_600)),
    }
}

/// A reply as it arrives.
#[derive(Clone, Debug, Default)]
pub(crate) struct Reply {
    /// The text so far: the partials in order, then the result whole.
    pub text: String,
    pub done: bool,
    pub failure: Option<Failure>,
    /// The model the worker named, an attribution claim.
    pub model: Option<String>,
    /// Where the worker's first-response judgment says the turn belongs.
    pub lane: Option<Lane>,
    /// A rank job's ordering: candidate IDs, most likely first.
    pub ranked: Vec<String>,
    /// The next partial's sequence number.
    next: u64,
    /// A gap or repeat stopped the preview; wait for the result.
    unordered: bool,
    /// The worker answered at all.
    heard: bool,
}

impl Reply {
    pub(crate) fn ended(&self) -> bool {
        self.done || self.failure.is_some()
    }
}

/// Where a basic conversation's turns are answered. The app uses [`Relay`];
/// tests answer in process.
pub(crate) trait Door: Send + Sync {
    /// Answer the conversation, whose last turn is the user's, into `reply`
    /// until it ends.
    fn ask(
        &self,
        turns: Vec<Turn>,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>>;

    /// Order `candidates` (ID and label) for a new chat with a rank job,
    /// into `reply.ranked`. A door without ranking refuses.
    fn rank(
        &self,
        _candidates: Vec<(String, String)>,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        lock(&reply).failure = Some(Failure::Transport("no ranking here".into()));
        Box::pin(async {})
    }

    /// Keep the way to the worker open, as while the Coder tab shows.
    fn warm(&self, _runtime: &tokio::runtime::Handle) {}

    /// Close the way to the worker once no reply waits on it.
    fn rest(&self) {}
}

/// The job's payload: the newest turns within [`MAX_TRANSCRIPT_BYTES`], the
/// last user message as the task, and the basic Coder's instructions. It asks
/// for the worker's first response (`opener`): a typed judgment, and a short
/// opener as the reply's first partial while the model starts.
pub(crate) fn payload(turns: &[Turn]) -> Value {
    let mut kept: Vec<&Turn> = vec![];
    let mut bytes = 0;
    for turn in turns.iter().rev() {
        bytes += turn.text.len();
        if bytes > MAX_TRANSCRIPT_BYTES && !kept.is_empty() {
            break;
        }
        kept.push(turn);
    }
    kept.reverse();
    // A transcript opens with the user's turn.
    while kept.first().is_some_and(|turn| turn.role != Role::User) {
        kept.remove(0);
    }
    let task = turns
        .iter()
        .rev()
        .find(|turn| turn.role == Role::User)
        .map_or("", |turn| turn.text.as_str());
    let task: String = truncate(task, MAX_TRANSCRIPT_BYTES).into();
    json!({
        "v": 2,
        "requires": [],
        "task": task,
        "transcript": kept
            .iter()
            .map(|turn| json!({
                "role": turn.role,
                "content": truncate(&turn.text, MAX_TRANSCRIPT_BYTES),
            }))
            .collect::<Vec<_>>(),
        "instructions": INSTRUCTIONS,
        "client": "openagents-mobile",
        "opener": true,
    })
}

/// A rank job's payload: the candidates for a new chat, at most
/// [`MAX_RANK_CANDIDATES`], each ID 1 to 64 bytes and not `none`, each label
/// up to 200 bytes; others are left out.
pub(crate) fn rank_payload(candidates: &[(String, String)]) -> Option<Value> {
    let candidates: Vec<Value> = candidates
        .iter()
        .filter(|(id, label)| (1..=64).contains(&id.len()) && id != "none" && !label.is_empty())
        .take(MAX_RANK_CANDIDATES)
        .map(|(id, label)| json!({"id": id, "label": truncate(label, 200)}))
        .collect();
    (candidates.len() >= 2).then(|| {
        json!({
            "v": 2,
            "requires": [],
            "type": "rank",
            "draft": "",
            "transcript": [],
            "candidates": candidates,
            "client": "openagents-mobile",
        })
    })
}

fn truncate(text: &str, limit: usize) -> &str {
    if text.len() <= limit {
        return text;
    }
    let mut end = limit;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// Checks the worker's answers to one request.
pub(crate) struct Reading {
    worker: String,
    me: String,
    request: String,
    key: [u8; 32],
}

impl Reading {
    pub(crate) fn new(
        secret: &SecretKey,
        me: &str,
        worker: &XOnlyPublicKey,
        request: &str,
    ) -> Self {
        Self {
            worker: hex(&worker.serialize()),
            me: me.to_owned(),
            request: request.to_owned(),
            key: nip44::conversation_key(secret, worker),
        }
    }

    /// Take one event the relay delivered. Anything that is not the
    /// worker's signed answer to this request, for this device, is set
    /// aside unread.
    pub(crate) fn take(&self, event: &Event, reply: &mut Reply) {
        if reply.ended()
            || !matches!(
                event.kind,
                CJ_CONVERSATION_FEEDBACK | CJ_CONVERSATION_RESULT
            )
            || event.pubkey != self.worker
            || !event.tag_values("e").any(|id| id == self.request)
            || !event.tag_values("p").any(|key| key == self.me)
            || event.validate_crypto().is_err()
        {
            return;
        }
        let Some(payload) = nip44::decrypt(&event.content, &self.key)
            .ok()
            .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        else {
            return;
        };
        if !matches!(payload["v"].as_u64(), Some(1 | 2)) {
            return;
        }
        reply.heard = true;
        match (event.kind, payload["type"].as_str()) {
            (CJ_CONVERSATION_RESULT, Some("result")) => {
                let Some(text) = payload["text"].as_str().filter(|text| !text.is_empty()) else {
                    reply.failure = Some(Failure::Refused {
                        code: "malformed".into(),
                        message: "the answer was empty".into(),
                        retry_after_ms: None,
                    });
                    return;
                };
                reply.text = truncate(text, MAX_REPLY_BYTES).to_owned();
                reply.model = payload["model"].as_str().map(str::to_owned);
                reply.ranked = payload["ranked"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .take(MAX_RANK_CANDIDATES)
                    .filter_map(|entry| entry["id"].as_str())
                    .filter(|id| (1..=64).contains(&id.len()))
                    .map(str::to_owned)
                    .collect();
                reply.done = true;
            }
            (CJ_CONVERSATION_FEEDBACK, Some("partial")) => {
                let (Some(seq), Some(delta)) = (payload["seq"].as_u64(), payload["delta"].as_str())
                else {
                    reply.unordered = true;
                    return;
                };
                if reply.unordered || seq != reply.next {
                    reply.unordered = true;
                    return;
                }
                if reply.text.len() + delta.len() <= MAX_REPLY_BYTES {
                    reply.text.push_str(delta);
                }
                reply.next += 1;
            }
            // The typed first-response judgment: its lane is an exact
            // enum value, never read from text.
            (CJ_CONVERSATION_FEEDBACK, Some("judgment")) => {
                reply.lane = match payload["lane"].as_str() {
                    Some("computer") => Some(Lane::Computer),
                    Some("chat") => Some(Lane::Chat),
                    _ => None,
                };
            }
            (CJ_CONVERSATION_FEEDBACK, Some("status"))
                if payload["status"].as_str() == Some("error") =>
            {
                let text = |field: &str, limit: usize| {
                    truncate(payload[field].as_str().unwrap_or_default(), limit).to_owned()
                };
                reply.failure = Some(Failure::Refused {
                    code: text("code", 64),
                    message: text("message", 512),
                    retry_after_ms: payload["retry_after_ms"].as_u64(),
                });
            }
            _ => {}
        }
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

/// The basic Coder through the OpenAgents chat worker, on a relay, over
/// one kept connection ([`Link`]).
pub(crate) struct Relay {
    worker: XOnlyPublicKey,
    secret: SecretKey,
    link: Link,
}

impl Relay {
    pub(crate) fn new(url: &str, worker: &str, secret: SecretKey) -> Result<Self, String> {
        let bytes: Vec<u8> = (0..worker.len())
            .step_by(2)
            .filter_map(|at| worker.get(at..at + 2))
            .filter_map(|pair| u8::from_str_radix(pair, 16).ok())
            .collect();
        let bytes: [u8; 32] = bytes
            .try_into()
            .map_err(|_| "the chat worker's key is not 64 hex characters")?;
        let worker = XOnlyPublicKey::from_byte_array(bytes)
            .map_err(|_| "the chat worker's key is not a public key")?;
        Ok(Self {
            worker,
            secret,
            link: Link::new(url, secret),
        })
    }

    #[cfg(test)]
    pub(crate) fn link(&self) -> &Link {
        &self.link
    }

    async fn run(
        link: Link,
        worker: XOnlyPublicKey,
        secret: SecretKey,
        payload: Value,
        reply: Arc<Mutex<Reply>>,
    ) -> Result<(), Failure> {
        let started = tokio::time::Instant::now();
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|error| Failure::Transport(error.to_string()))?;
        let me = crate::account::public(&secret).0;
        let key = nip44::conversation_key(&secret, &worker);
        let content = nip44::encrypt(
            &payload.to_string(),
            &key,
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .map_err(Failure::Transport)?;
        let worker_hex = hex(&worker.serialize());
        let request = signer.sign(
            unix_now(),
            CJ_CONVERSATION_REQUEST,
            vec![Tag::new(vec!["p".into(), worker_hex])],
            content,
        );
        let reading = Reading::new(&secret, &me, &worker, &request.id);
        let mut job = link
            .publish(&request, CONTACT)
            .await
            .map_err(Failure::Transport)?;
        let contact = started + CONTACT;
        let end = started + LIFETIME;
        loop {
            let heard = lock(&reply).heard;
            let deadline = if heard { end } else { contact.min(end) };
            let frame = match tokio::time::timeout_at(deadline, job.frames.recv()).await {
                Ok(Some(frame)) => frame,
                Ok(None) => return Err(Failure::Transport("the relay connection closed".into())),
                Err(_) => return Err(Failure::Silent),
            };
            match frame[0].as_str() {
                Some("OK") if frame[1] == request.id.as_str() && frame[2] == false => {
                    return Err(Failure::Transport(
                        frame[3]
                            .as_str()
                            .unwrap_or("the relay refused the job")
                            .into(),
                    ));
                }
                Some("EVENT") => {
                    let Ok(event) = serde_json::from_value::<Event>(frame[2].clone()) else {
                        continue;
                    };
                    let mut reply = lock(&reply);
                    reading.take(&event, &mut reply);
                    if reply.ended() {
                        return Ok(());
                    }
                }
                _ => {}
            }
        }
    }

    fn job(
        &self,
        payload: Value,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        let (link, worker, secret) = (self.link.clone(), self.worker, self.secret);
        Box::pin(async move {
            if let Err(failure) = Self::run(link, worker, secret, payload, reply.clone()).await {
                let mut reply = lock(&reply);
                if !reply.ended() {
                    // Half a reply is better than none: keep what streamed
                    // only when the worker had finished it.
                    reply.failure = Some(match failure {
                        Failure::Transport(_) if reply.heard => Failure::Silent,
                        other => other,
                    });
                }
            }
        })
    }
}

impl Door for Relay {
    fn ask(
        &self,
        turns: Vec<Turn>,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        self.job(payload(&turns), reply)
    }

    fn rank(
        &self,
        candidates: Vec<(String, String)>,
        reply: Arc<Mutex<Reply>>,
    ) -> Pin<Box<dyn Future<Output = ()> + Send>> {
        match rank_payload(&candidates) {
            Some(payload) => self.job(payload, reply),
            None => {
                lock(&reply).failure = Some(Failure::Transport("nothing to rank".into()));
                Box::pin(async {})
            }
        }
    }

    fn warm(&self, runtime: &tokio::runtime::Handle) {
        self.link.warm(runtime);
    }

    fn rest(&self) {
        self.link.rest();
    }
}

pub(crate) fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys() -> (SecretKey, String, SecretKey, XOnlyPublicKey) {
        let me = SecretKey::from_byte_array([0x21; 32]).unwrap();
        let worker = SecretKey::from_byte_array([0x22; 32]).unwrap();
        let worker_public =
            secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &worker)
                .x_only_public_key()
                .0;
        (me, crate::account::public(&me).0, worker, worker_public)
    }

    /// An answer signed by `signer`, to `request`, for `to`.
    fn answer(signer: &SecretKey, to: &str, request: &str, kind: u16, body: Value) -> Event {
        let (me_secret, _, _, _) = keys();
        let me_public =
            secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &me_secret)
                .x_only_public_key()
                .0;
        let key = nip44::conversation_key(signer, &me_public);
        let content = nip44::encrypt(&body.to_string(), &key, [7; 32]).unwrap();
        RelaySigner::from_secret_hex(&signer.display_secret().to_string())
            .unwrap()
            .sign(
                unix_now(),
                kind,
                vec![
                    Tag::new(vec!["e".into(), request.into()]),
                    Tag::new(vec!["p".into(), to.into()]),
                ],
                content,
            )
    }

    fn partial(seq: u64, delta: &str) -> Value {
        json!({"v": 2, "type": "partial", "seq": seq, "delta": delta})
    }

    #[test]
    fn partials_stream_in_order_and_the_result_replaces_them() {
        let (me, me_hex, worker, worker_public) = keys();
        let request = "ab".repeat(32);
        let reading = Reading::new(&me, &me_hex, &worker_public, &request);
        let mut reply = Reply::default();
        let event = |kind, body| answer(&worker, &me_hex, &request, kind, body);
        reading.take(
            &event(CJ_CONVERSATION_FEEDBACK, partial(0, "Hel")),
            &mut reply,
        );
        reading.take(
            &event(CJ_CONVERSATION_FEEDBACK, partial(1, "lo")),
            &mut reply,
        );
        assert_eq!(reply.text, "Hello");
        assert!(!reply.ended());
        // A gap stops the preview; the result still arrives whole.
        reading.take(
            &event(CJ_CONVERSATION_FEEDBACK, partial(3, "!!")),
            &mut reply,
        );
        reading.take(
            &event(CJ_CONVERSATION_FEEDBACK, partial(2, " there")),
            &mut reply,
        );
        assert_eq!(reply.text, "Hello");
        let result = json!({"v": 2, "type": "result", "text": "Hello there!", "model": "m"});
        reading.take(&event(CJ_CONVERSATION_RESULT, result), &mut reply);
        assert_eq!(reply.text, "Hello there!");
        assert_eq!(reply.model.as_deref(), Some("m"));
        assert!(reply.done);
        // Nothing moves an ended reply.
        let late = json!({"v": 2, "type": "result", "text": "Other"});
        reading.take(&event(CJ_CONVERSATION_RESULT, late), &mut reply);
        assert_eq!(reply.text, "Hello there!");
    }

    #[test]
    fn answers_from_anyone_else_or_for_another_job_are_set_aside() {
        let (me, me_hex, worker, worker_public) = keys();
        let request = "ab".repeat(32);
        let reading = Reading::new(&me, &me_hex, &worker_public, &request);
        let mut reply = Reply::default();
        let stranger = SecretKey::from_byte_array([0x23; 32]).unwrap();
        let body = json!({"v": 2, "type": "result", "text": "forged"});
        for event in [
            answer(
                &stranger,
                &me_hex,
                &request,
                CJ_CONVERSATION_RESULT,
                body.clone(),
            ),
            answer(
                &worker,
                &me_hex,
                &"cd".repeat(32),
                CJ_CONVERSATION_RESULT,
                body.clone(),
            ),
            answer(
                &worker,
                &"ef".repeat(32),
                &request,
                CJ_CONVERSATION_RESULT,
                body.clone(),
            ),
            answer(
                &worker,
                &me_hex,
                &request,
                CJ_CONVERSATION_REQUEST,
                body.clone(),
            ),
        ] {
            reading.take(&event, &mut reply);
        }
        // A tampered signature.
        let mut tampered = answer(&worker, &me_hex, &request, CJ_CONVERSATION_RESULT, body);
        tampered.created_at += 1;
        reading.take(&tampered, &mut reply);
        assert!(!reply.ended() && reply.text.is_empty() && !reply.heard);
    }

    #[test]
    fn a_refusal_carries_its_code_and_wait() {
        let (me, me_hex, worker, worker_public) = keys();
        let request = "ab".repeat(32);
        let reading = Reading::new(&me, &me_hex, &worker_public, &request);
        let mut reply = Reply::default();
        let status = json!({"v": 2, "type": "status", "status": "error",
            "code": "rate_limited", "message": "slow down", "retry_after_ms": 40_000});
        reading.take(
            &answer(&worker, &me_hex, &request, CJ_CONVERSATION_FEEDBACK, status),
            &mut reply,
        );
        let failure = reply.failure.clone().expect("refused");
        assert_eq!(
            failure.describe(),
            "You're sending messages quickly. Try again in 40 seconds."
        );
        let exhausted = Failure::Refused {
            code: "quota_exhausted".into(),
            message: String::new(),
            retry_after_ms: Some(5 * 3_600_000),
        };
        assert!(
            exhausted.describe().contains("5 hours"),
            "{}",
            exhausted.describe()
        );
    }

    #[test]
    fn the_payload_keeps_the_newest_turns_within_its_bound() {
        let long = "x".repeat(MAX_TRANSCRIPT_BYTES / 2 + 10);
        let turns = vec![
            Turn {
                role: Role::User,
                text: long.clone(),
            },
            Turn {
                role: Role::Assistant,
                text: long.clone(),
            },
            Turn {
                role: Role::User,
                text: "last".into(),
            },
        ];
        let body = payload(&turns);
        assert_eq!(body["v"], 2);
        assert_eq!(body["task"], "last");
        assert_eq!(body["instructions"], INSTRUCTIONS);
        let transcript = body["transcript"].as_array().unwrap();
        // The oldest turn did not fit, and the assistant turn left without
        // its question is dropped too: a transcript opens with the user.
        assert_eq!(transcript.len(), 1);
        assert_eq!(transcript[0]["role"], "user");
        assert_eq!(transcript[0]["content"], "last");
    }

    /// The job names the conversation and nothing else: no credential, no
    /// model, no grant. The worker's lane and limits are its own.
    #[test]
    fn a_basic_job_carries_no_credential() {
        let body = payload(&[Turn {
            role: Role::User,
            text: "hi".into(),
        }]);
        let mut fields: Vec<&str> = body
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        fields.sort_unstable();
        assert_eq!(
            fields,
            [
                "client",
                "instructions",
                "opener",
                "requires",
                "task",
                "transcript",
                "v"
            ]
        );
    }

    /// The job asks for the worker's first response: the typed judgment
    /// and an opener as the reply's first partial.
    #[test]
    fn a_basic_job_asks_for_the_first_response() {
        let body = payload(&[Turn {
            role: Role::User,
            text: "hi".into(),
        }]);
        assert_eq!(body["opener"], true);
    }

    #[test]
    fn the_judgment_places_the_turn_and_the_opener_leads_the_reply() {
        let (me, me_hex, worker, worker_public) = keys();
        let request = "ab".repeat(32);
        let reading = Reading::new(&me, &me_hex, &worker_public, &request);
        let mut reply = Reply::default();
        let event = |kind, body| answer(&worker, &me_hex, &request, kind, body);
        let judgment = json!({"v": 2, "requires": [], "type": "judgment",
            "verdict": "respond", "line": "That needs your computer.",
            "set": "coder-first-response-v1", "lane": "computer",
            "opener": "computer", "confidence": 0.81});
        reading.take(&event(CJ_CONVERSATION_FEEDBACK, judgment), &mut reply);
        assert_eq!(reply.lane, Some(Lane::Computer));
        assert!(reply.heard && reply.text.is_empty());
        reading.take(
            &event(
                CJ_CONVERSATION_FEEDBACK,
                partial(0, "That needs your computer."),
            ),
            &mut reply,
        );
        assert_eq!(reply.text, "That needs your computer.");
        // An unknown lane places nothing.
        let mut other = Reply::default();
        let unknown = json!({"v": 2, "type": "judgment", "verdict": "respond",
            "line": "", "lane": "unknown"});
        reading.take(&event(CJ_CONVERSATION_FEEDBACK, unknown), &mut other);
        assert_eq!(other.lane, None);
    }

    #[test]
    fn a_rank_job_names_its_candidates_and_reads_the_order() {
        let candidates = vec![
            ("workspace:openagents".to_owned(), "openagents".to_owned()),
            ("none".to_owned(), "Refused ID".to_owned()),
            ("talk:1".to_owned(), "Deploy preview".to_owned()),
        ];
        let body = rank_payload(&candidates).expect("two candidates");
        assert_eq!(body["type"], "rank");
        assert_eq!(body["candidates"].as_array().unwrap().len(), 2);
        assert!(rank_payload(&candidates[..2]).is_none());
        let (me, me_hex, worker, worker_public) = keys();
        let request = "ab".repeat(32);
        let reading = Reading::new(&me, &me_hex, &worker_public, &request);
        let mut reply = Reply::default();
        let result = json!({"v": 2, "type": "result", "text": "talk:1", "model": "jev",
            "ranked": [{"id": "talk:1", "p": 0.7}, {"id": "workspace:openagents", "p": 0.2}]});
        reading.take(
            &answer(&worker, &me_hex, &request, CJ_CONVERSATION_RESULT, result),
            &mut reply,
        );
        assert_eq!(reply.ranked, ["talk:1", "workspace:openagents"]);
    }

    #[test]
    fn the_worker_key_parses() {
        let secret = SecretKey::from_byte_array([0x21; 32]).unwrap();
        assert!(Relay::new(RELAY, WORKER, secret).is_ok());
        assert!(Relay::new(RELAY, "zz", secret).is_err());
    }
}

/// The basic Coder against the real relay and chat worker, from a fresh
/// key, as a new install sends its first message. Run it with the worker
/// up:
///
/// ```sh
/// cargo test --manifest-path crates/openagents-mobile/Cargo.toml \
///   live_basic_coder_streams_a_reply -- --ignored --nocapture
/// ```
///
/// `OPENAGENTS_TEST_CHAT_RELAY` and `OPENAGENTS_TEST_CHAT_WORKER` point it
/// at another relay and worker.
#[cfg(test)]
#[test]
#[ignore = "network: needs the chat worker on the relay"]
fn live_basic_coder_streams_a_reply() {
    let relay = std::env::var("OPENAGENTS_TEST_CHAT_RELAY").unwrap_or_else(|_| RELAY.into());
    let worker = std::env::var("OPENAGENTS_TEST_CHAT_WORKER").unwrap_or_else(|_| WORKER.into());
    let _ = rustls::crypto::ring::default_provider().install_default();
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let secret = SecretKey::new(&mut secp256k1::rand::rng());
    let door = Relay::new(&relay, &worker, secret).unwrap();
    let reply = Arc::new(Mutex::new(Reply::default()));
    let started = std::time::Instant::now();
    let asking = runtime.spawn(door.ask(
        vec![Turn {
            role: Role::User,
            text: "In three short sentences, what does a Nostr relay do?".into(),
        }],
        reply.clone(),
    ));
    let mut first = None;
    let mut lengths = vec![];
    while !lock(&reply).ended() {
        let length = lock(&reply).text.len();
        if length > 0 && first.is_none() {
            first = Some(started.elapsed());
        }
        if lengths.last() != Some(&length) {
            lengths.push(length);
        }
        assert!(started.elapsed() < Duration::from_secs(60), "no answer");
        std::thread::sleep(Duration::from_millis(20));
    }
    runtime.block_on(asking).unwrap();
    let reply = lock(&reply).clone();
    eprintln!(
        "first words {:?}, answered {:?}, {} states {:?}, model {:?}\n{}",
        first,
        started.elapsed(),
        lengths.len(),
        lengths,
        reply.model,
        reply.text
    );
    assert!(reply.failure.is_none(), "{:?}", reply.failure);
    assert!(reply.done && !reply.text.is_empty());
    assert!(first.is_some_and(|first| first < Duration::from_secs(10)));
}
