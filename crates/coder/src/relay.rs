//! The relay door: a Coder Jobs client ([`nips/openagents/NIP-CJ.md`]) over a
//! Nostr relay.
//!
//! The terminal holds a secp256k1 identity under `~/.openagents/`, answers
//! the relay's NIP-42 challenge on connect, and speaks the ephemeral
//! job protocol: a kind-`25900` request NIP-44-encrypted to the worker,
//! then `27000` feedback and a `26900` result streamed back `e`-tagged to
//! it. No HTTP, no bearer tokens — the `npub` is the account.
//!
//! Configuration: `CODER_WORKER` is the worker `npub` (or hex pubkey) that
//! job requests encrypt to — without it the door does not build.
//! `CODER_RELAY` picks the relay, defaulting to the production relay.
//! `CODER_SECRET_KEY` or `CODER_NSEC` overrides the on-disk identity.
//!
//! [`Identity`], [`connect`], [`send`], and the kind constants are public
//! because the fulfillment side needs the same pieces: `coder-worker` is
//! the other end of this door, and a second copy of the connect-and-
//! authenticate dance would be a second thing to keep in step with the
//! relay.
//!
//! A turn that does not finish says which of three things happened, as a
//! field rather than as prose: [`GenerateError::Relay`] when nothing
//! reached a worker, [`GenerateError::Silent`] when the relay took the job
//! and no answer came back, and [`GenerateError::Refused`] when a worker
//! answered by declining. `docs/coder/relay-transport.md` measures all
//! three.
//!
//! Silence is two waits, not one. [`CONTACT_TIMEOUT`] bounds the wait for
//! the first sign that a worker is there at all, and [`ANSWER_TIMEOUT`]
//! bounds the wait for the answer once one is. A single bound covering
//! both made an absent worker cost the whole long wait and then report a
//! failure that could not say whether anyone had been listening.

use std::env;
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::{nip19, nip44};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};

use crate::generate::{Generate, GenerateError, Message, Meta, Role, Usage};

/// The production relay the door defaults to.
pub const DEFAULT_RELAY_URL: &str = "wss://relay.openagents.com";

/// The NIP-CJ job request, terminal to worker. Ephemeral: the relay fans
/// it out and stores none of it.
pub const REQUEST_KIND: u16 = 25_900;
/// The NIP-CJ job result, worker to terminal. Ephemeral.
pub const RESULT_KIND: u16 = 26_900;
/// NIP-CJ job feedback — judgment, partial, status — worker to terminal.
/// Ephemeral.
pub const FEEDBACK_KIND: u16 = 27_000;
const AUTH_KIND: u16 = 22_242;

/// How long a turn waits for the first sign that a worker is there.
///
/// NIP-CJ's kinds are ephemeral: an unanswered request leaves nothing on
/// the relay to ask about, so a client cannot prove a worker is missing.
/// What it can do is stop waiting for one sooner than it waits for a
/// model. Any event from the worker's key `e`-tagged to the request is the
/// sign — a judgment, a partial, a refusal, or the result itself — because
/// all of them prove something read the job.
///
/// The window is wide enough that a worker whose own door is retrying an
/// empty stream still gets counted as present, and short enough that an
/// absent one fails while someone is still watching.
pub const CONTACT_TIMEOUT: Duration = Duration::from_secs(30);

/// How long a turn waits for the answer once a worker has been heard from.
///
/// This is the model's wait, and it is the long one. Before the two were
/// split, it was also the absent worker's wait.
pub const ANSWER_TIMEOUT: Duration = Duration::from_secs(180);

/// An authenticated relay connection. Both ends of NIP-CJ hold one.
pub type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The terminal's Nostr identity: a secp256k1 keypair kept on disk so the
/// `npub` — and therefore the worker's allowance ledger — is stable.
pub struct Identity {
    secret: SecretKey,
    signer: RelaySigner,
}

impl Identity {
    /// The identity for a secret key.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the key would not load.
    pub fn from_secret(secret: SecretKey) -> Result<Self, String> {
        let hex: String = secret
            .secret_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let signer = RelaySigner::from_secret_hex(&hex).map_err(|error| error.to_string())?;
        Ok(Self { secret, signer })
    }

    /// The identity for a secret key written as 64 lowercase hex or as an
    /// `nsec`. `what` names the source in any error.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the text is not a secret key.
    pub fn from_text(text: &str, what: &str) -> Result<Self, String> {
        let text = text.trim();
        let bytes = if text.starts_with("nsec1") {
            nip19::decode_nsec(text).map_err(|error| format!("{what} does not decode: {error}"))?
        } else {
            parse_hex(text).ok_or_else(|| format!("{what} must be 64 lowercase hex or an nsec"))?
        };
        let secret = SecretKey::from_byte_array(bytes)
            .map_err(|_| format!("{what} is not a valid secret key"))?;
        Self::from_secret(secret)
    }

    /// The identity from the environment or `~/.openagents/nostr-secret`,
    /// generating and installing a fresh keypair (mode 0600) on first run.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why no identity could be loaded.
    pub fn load() -> Result<Self, String> {
        if let Ok(hex) = env::var("CODER_SECRET_KEY") {
            return Self::from_text(&hex, "CODER_SECRET_KEY");
        }
        if let Ok(nsec) = env::var("CODER_NSEC") {
            return Self::from_text(&nsec, "CODER_NSEC");
        }

        let home = env::var("HOME").map_err(|_| "HOME is not set".to_string())?;
        let dir = PathBuf::from(home).join(".openagents");
        let path = dir.join("nostr-secret");
        match fs::read_to_string(&path) {
            Ok(text) => {
                let bytes = parse_hex(text.trim())
                    .ok_or_else(|| format!("{} is not 64 lowercase hex", path.display()))?;
                let secret = SecretKey::from_byte_array(bytes)
                    .map_err(|_| format!("{} is not a valid secret key", path.display()))?;
                Self::from_secret(secret)
            }
            Err(_) => {
                fs::create_dir_all(&dir)
                    .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    let _ = fs::set_permissions(&dir, fs::Permissions::from_mode(0o700));
                }
                let secret = SecretKey::new(&mut secp256k1::rand::rng());
                let hex: String = secret
                    .secret_bytes()
                    .iter()
                    .map(|byte| format!("{byte:02x}"))
                    .collect();
                fs::write(&path, format!("{hex}\n"))
                    .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                        .map_err(|error| format!("cannot protect {}: {error}", path.display()))?;
                }
                Self::from_secret(secret)
            }
        }
    }

    /// The identity's x-only public key, hex.
    pub fn pubkey(&self) -> &str {
        self.signer.pubkey()
    }

    /// The identity's secret key, for the NIP-44 conversation key a job
    /// payload is encrypted under.
    pub fn secret(&self) -> &SecretKey {
        &self.secret
    }

    /// The signer that puts this identity on an event.
    pub fn signer(&self) -> &RelaySigner {
        &self.signer
    }
}

/// Connects to `url` and answers the relay's NIP-42 challenge as
/// `identity`.
///
/// Both ends of NIP-CJ open a socket the same way: the relay sends
/// `["AUTH", challenge]` on connect whenever it has a configured URL, and
/// the challenge is answered before any other traffic. A relay that sends
/// something else first is open, and the socket is handed back as it is.
///
/// # Errors
///
/// Returns a [`GenerateError::Relay`] naming what the connection or the
/// authentication did instead. Nothing reached a worker, so nothing here
/// is a refusal.
pub async fn connect(url: &str, identity: &Identity) -> Result<Socket, GenerateError> {
    let (mut socket, _) = connect_async(url)
        .await
        .map_err(|error| GenerateError::Relay(format!("connect: {error}")))?;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    loop {
        let frame = match tokio::time::timeout_at(deadline, socket.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(error))) => {
                return Err(GenerateError::Relay(format!("socket: {error}")));
            }
            Ok(None) => {
                return Err(GenerateError::Relay("socket closed during auth".into()));
            }
            Err(_) => {
                return Err(GenerateError::Relay("no AUTH challenge received".into()));
            }
        };
        let tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        match value[0].as_str().unwrap_or_default() {
            "AUTH" => {
                let Some(challenge) = value[1].as_str() else {
                    return Err(GenerateError::Relay("malformed AUTH challenge".into()));
                };
                let event = identity.signer.sign(
                    unix_now(),
                    AUTH_KIND,
                    vec![
                        Tag::new(vec!["relay".into(), url.to_string()]),
                        Tag::new(vec!["challenge".into(), challenge.to_string()]),
                    ],
                    String::new(),
                );
                let auth_id = event.id.clone();
                send(&mut socket, json!(["AUTH", event]))
                    .await
                    .map_err(as_relay)?;
                // The relay confirms with ["OK", auth_id, true, ""].
                let ok = wait_for_ok(&mut socket, &auth_id, deadline)
                    .await
                    .map_err(as_relay)?;
                if !ok {
                    return Err(GenerateError::Relay("NIP-42 authentication refused".into()));
                }
                return Ok(socket);
            }
            // AUTH_REQUIRED relays challenge first; open relays may send
            // nothing before traffic. Any other frame means no challenge
            // is coming on this socket.
            _ => return Ok(socket),
        }
    }
}

/// A `Generate` over the relay: each turn publishes a NIP-CJ job request
/// and streams the worker's feedback and result back.
pub struct RelayDoor {
    /// The relay WebSocket URL.
    pub url: String,
    worker: XOnlyPublicKey,
    worker_hex: String,
    identity: Identity,
    contact: Duration,
    answer: Duration,
    socket: Mutex<Option<Socket>>,
}

impl RelayDoor {
    /// A door for `url` talking to `worker` as `identity`.
    pub fn new(url: impl Into<String>, worker: XOnlyPublicKey, identity: Identity) -> Self {
        Self {
            url: url.into(),
            worker,
            worker_hex: worker.to_string(),
            identity,
            contact: CONTACT_TIMEOUT,
            answer: ANSWER_TIMEOUT,
            socket: Mutex::new(None),
        }
    }

    /// The same door with different waits, so a test can exercise the
    /// split between them without spending the real ones.
    #[must_use]
    pub fn waiting(mut self, contact: Duration, answer: Duration) -> Self {
        self.contact = contact;
        self.answer = answer;
        self
    }

    /// A door from the environment: `CODER_WORKER` selects the worker
    /// (`npub` or hex), `CODER_RELAY` selects the relay.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why the door could not be built. A worker
    /// that does not parse used to read as no worker at all, which put the
    /// turn on the stub door and said nothing about the typo.
    pub fn from_env() -> Result<Self, String> {
        let worker = env::var("CODER_WORKER")
            .ok()
            .filter(|worker| !worker.is_empty())
            .ok_or_else(|| "CODER_WORKER is not set".to_string())?;
        let worker = parse_pubkey(&worker)
            .ok_or_else(|| "CODER_WORKER must be an npub or 64 lowercase hex".to_string())?;
        let url = env::var("CODER_RELAY").unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
        let identity = Identity::load()?;
        Ok(Self::new(url, worker, identity))
    }

    /// The socket, connecting and authenticating when needed.
    async fn connection(&self) -> Result<Socket, GenerateError> {
        connect(&self.url, &self.identity).await
    }

    /// Runs one job over `socket`: subscribe, publish, stream the answer.
    async fn turn(
        &self,
        socket: &mut Socket,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let task = input
            .iter()
            .rev()
            .find(|message| message.role == Role::User)
            .map(|message| message.text.clone())
            .unwrap_or_default();
        let transcript: Vec<Value> = input
            .iter()
            .map(|message| {
                json!({
                    "role": match message.role {
                        Role::User => "user",
                        Role::Assistant => "assistant",
                    },
                    "content": message.text,
                })
            })
            .collect();
        let payload = json!({
            "v": 1,
            "task": task,
            "transcript": transcript,
            "instructions": instructions,
            "client": concat!("coder ", env!("CARGO_PKG_VERSION")),
        })
        .to_string();

        let conversation = nip44::conversation_key(&self.identity.secret, &self.worker);
        let content = nip44::encrypt(
            &payload,
            &conversation,
            secp256k1::rand::random::<[u8; 32]>(),
        )
        .map_err(|error| GenerateError::Stream(format!("encrypt: {error}")))?;
        let request = self.identity.signer.sign(
            unix_now(),
            REQUEST_KIND,
            vec![Tag::new(vec!["p".into(), self.worker_hex.clone()])],
            content,
        );

        // Subscribe before publishing so no fast feedback is missed.
        let subscription = format!("job-{}", &request.id[..16]);
        send(
            socket,
            json!(["REQ", subscription, {
                "kinds": [RESULT_KIND, FEEDBACK_KIND],
                "#e": [request.id],
            }]),
        )
        .await?;
        send(socket, json!(["EVENT", request])).await?;

        // One read loop for the publish `OK` and the worker's feedback:
        // a fast worker can answer before the OK lands, so the frames must
        // interleave rather than arrive in separate phases.
        //
        // The deadline the loop reads against changes the moment a worker
        // is heard from: until then it is the short one, and the failure
        // it produces says nobody was there.
        let mut partials = String::new();
        let mut heard = false;
        let answer_by = tokio::time::Instant::now() + self.answer;
        let contact_by = tokio::time::Instant::now() + self.contact;
        loop {
            let deadline = if heard { answer_by } else { contact_by };
            let frame = match tokio::time::timeout_at(deadline, socket.next()).await {
                Ok(Some(frame)) => frame,
                Ok(None) => {
                    return Err(GenerateError::Silent {
                        heard,
                        reason: "the socket closed before the worker answered".into(),
                    });
                }
                Err(_) => return Err(self.ran_out(heard)),
            };
            let message =
                frame.map_err(|error| GenerateError::Stream(format!("socket: {error}")))?;
            let tungstenite::Message::Text(text) = message else {
                continue;
            };
            let Ok(value) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            match value[0].as_str().unwrap_or_default() {
                "OK" if value[1].as_str() == Some(request.id.as_str())
                    && !value[2].as_bool().unwrap_or(false) =>
                {
                    let reason = value[3].as_str().unwrap_or("refused");
                    return Err(GenerateError::Relay(format!(
                        "the relay refused the job request: {reason}"
                    )));
                }
                "EVENT" if value[1].as_str() == Some(subscription.as_str()) => {
                    let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) else {
                        continue;
                    };
                    if event.pubkey != self.worker_hex || event.validate_crypto().is_err() {
                        continue;
                    }
                    // A signed event from the worker, tagged to this
                    // request: somebody is there. Whatever it says, the
                    // wait is no longer the short one.
                    heard = true;
                    let Ok(plaintext) = nip44::decrypt(&event.content, &conversation) else {
                        continue;
                    };
                    let Ok(feedback) = serde_json::from_str::<Value>(&plaintext) else {
                        continue;
                    };
                    let kind = feedback["type"].as_str().unwrap_or_default();
                    if event.kind == FEEDBACK_KIND && kind == "judgment" {
                        if let Some(line) = feedback["line"].as_str() {
                            meta(Meta::Judgment(line.to_string()));
                        }
                    } else if event.kind == FEEDBACK_KIND && kind == "partial" {
                        if let Some(delta) = feedback["delta"].as_str() {
                            partials.push_str(delta);
                            sink(delta);
                        }
                    } else if event.kind == FEEDBACK_KIND
                        && kind == "status"
                        && feedback["status"].as_str() == Some("error")
                    {
                        // A typed refusal is an answer: a worker read the
                        // job and said no, with a reason a caller can act
                        // on. It is not the transport failing.
                        let code = feedback["code"].as_str().unwrap_or("internal");
                        let message = feedback["message"].as_str().unwrap_or(code);
                        return Err(GenerateError::Refused {
                            code: code.to_string(),
                            message: message.to_string(),
                        });
                    } else if event.kind == RESULT_KIND {
                        // The worker names the model it used, and the door
                        // used to drop it on the floor. A relay run's
                        // evidence has to be able to say what answered.
                        if let Some(model) = feedback["model"].as_str().filter(|m| !m.is_empty()) {
                            meta(Meta::Model(model.to_string()));
                        }
                        let text = feedback["text"].as_str().unwrap_or_default().to_string();
                        let text = if text.is_empty() { partials } else { text };
                        if text.is_empty() {
                            return Err(GenerateError::Stream(
                                "the worker's result carried no text".into(),
                            ));
                        }
                        let usage = feedback["usage"].as_object().map(|usage| Usage {
                            input_tokens: usage["input"].as_u64().unwrap_or(0),
                            output_tokens: usage["output"].as_u64().unwrap_or(0),
                        });
                        return Ok((text, usage));
                    }
                }
                _ => {}
            }
        }
    }

    /// The failure a wait that ran out produces, in whichever of the two
    /// waits it was.
    fn ran_out(&self, heard: bool) -> GenerateError {
        let waited = if heard { self.answer } else { self.contact };
        let reason = if heard {
            format!(
                "{} started answering and stopped: no result in {} seconds",
                self.worker_hex,
                waited.as_secs()
            )
        } else {
            format!(
                "nothing came back from {} in {} seconds: either no worker is listening, \
                 or one is and said nothing while it worked",
                self.worker_hex,
                waited.as_secs()
            )
        };
        GenerateError::Silent { heard, reason }
    }
}

impl Generate for RelayDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        // A turn gets the shared socket or opens a fresh one; any failure
        // drops it so the next turn reconnects cleanly.
        let mut guard = self.socket.lock().await;
        if guard.is_none() {
            *guard = Some(self.connection().await?);
        }
        let socket = guard.as_mut().expect("a socket was just stored");
        let answered = self.turn(socket, instructions, input, sink, meta).await;
        // A wait that ran out or a socket that broke leaves a connection
        // nobody can trust: the subscription is still open and a late
        // answer would arrive in the middle of the next turn. A refusal
        // leaves the socket healthy, and the next turn reuses it.
        if matches!(
            answered,
            Err(GenerateError::Silent { .. } | GenerateError::Stream(_))
        ) {
            *guard = None;
        }
        answered
    }
}

/// Refiles a socket failure raised while the connection was being opened.
/// Nothing had reached a worker yet, so the relay owns it.
fn as_relay(error: GenerateError) -> GenerateError {
    match error {
        GenerateError::Stream(why) => GenerateError::Relay(why),
        other => GenerateError::Relay(other.to_string()),
    }
}

fn parse_pubkey(text: &str) -> Option<XOnlyPublicKey> {
    let text = text.trim();
    if let Ok(bytes) = nip19::decode_npub(text) {
        return XOnlyPublicKey::from_byte_array(bytes).ok();
    }
    let bytes = parse_hex(text)?;
    XOnlyPublicKey::from_byte_array(bytes).ok()
}

fn parse_hex(text: &str) -> Option<[u8; 32]> {
    let text = text.trim();
    if text.len() != 64 {
        return None;
    }
    let mut bytes = [0u8; 32];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        let high = (pair[0] as char).to_digit(16)?;
        let low = (pair[1] as char).to_digit(16)?;
        bytes[index] = ((high << 4) | low) as u8;
    }
    Some(bytes)
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

/// Writes one relay frame.
///
/// # Errors
///
/// Returns a [`GenerateError::Stream`] when the socket will not take it.
pub async fn send(socket: &mut Socket, value: Value) -> Result<(), GenerateError> {
    socket
        .send(tungstenite::Message::Text(value.to_string().into()))
        .await
        .map_err(|error| GenerateError::Stream(format!("send: {error}")))
}

/// Reads frames until the relay's `["OK", id, accepted, reason]` for `id`
/// lands or `deadline` passes. `Err` only on socket failure; the caller
/// inspects `accepted`.
async fn wait_for_ok(
    socket: &mut Socket,
    id: &str,
    deadline: tokio::time::Instant,
) -> Result<bool, GenerateError> {
    loop {
        let frame = match tokio::time::timeout_at(deadline, socket.next()).await {
            Ok(Some(Ok(message))) => message,
            Ok(Some(Err(error))) => {
                return Err(GenerateError::Stream(format!("socket: {error}")));
            }
            Ok(None) => {
                return Err(GenerateError::Stream("socket closed waiting for OK".into()));
            }
            Err(_) => {
                return Err(GenerateError::Stream(format!(
                    "no OK for event {id} in time"
                )));
            }
        };
        let tungstenite::Message::Text(text) = frame else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value[0].as_str() == Some("OK") && value[1].as_str() == Some(id) {
            let accepted = value[2].as_bool().unwrap_or(false);
            if !accepted && let Some(reason) = value[3].as_str() {
                return Err(GenerateError::Stream(format!("rejected: {reason}")));
            }
            return Ok(accepted);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secp256k1::Secp256k1;

    #[test]
    fn npub_and_hex_pubkeys_parse() {
        let secret = SecretKey::from_byte_array([7; 32]).unwrap();
        let secp = Secp256k1::new();
        let (public, _) = secret.public_key(&secp).x_only_public_key();
        let npub = nip19::encode_npub(&public.serialize());
        assert_eq!(parse_pubkey(&npub), Some(public));
        assert_eq!(parse_pubkey(&public.to_string()), Some(public));
        assert_eq!(parse_pubkey("not-a-key"), None);
    }

    #[test]
    fn identity_round_trips_a_hex_secret() {
        let secret = SecretKey::from_byte_array([9; 32]).unwrap();
        let identity = Identity::from_secret(secret).unwrap();
        let (public, _) = secret.public_key(&Secp256k1::new()).x_only_public_key();
        assert_eq!(identity.pubkey(), public.to_string());
    }

    #[test]
    fn signed_auth_and_request_events_verify() {
        let identity = Identity::from_secret(SecretKey::from_byte_array([5; 32]).unwrap()).unwrap();
        let auth = identity.signer.sign(
            1_700_000_000,
            AUTH_KIND,
            vec![
                Tag::new(vec!["relay".into(), "wss://relay.example.com".into()]),
                Tag::new(vec!["challenge".into(), "abc123".into()]),
            ],
            String::new(),
        );
        assert_eq!(auth.kind, AUTH_KIND);
        auth.validate_structure().unwrap();
        auth.validate_crypto().unwrap();
    }
}
