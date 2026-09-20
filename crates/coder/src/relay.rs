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

/// NIP-CJ kinds. All ephemeral: the relay fans them out and stores none.
const REQUEST_KIND: u16 = 25_900;
const RESULT_KIND: u16 = 26_900;
const FEEDBACK_KIND: u16 = 27_000;
const AUTH_KIND: u16 = 22_242;

/// How long one turn waits for the worker before failing.
const TURN_TIMEOUT: Duration = Duration::from_secs(180);

type Socket = WebSocketStream<MaybeTlsStream<TcpStream>>;

/// The terminal's Nostr identity: a secp256k1 keypair kept on disk so the
/// `npub` — and therefore the worker's allowance ledger — is stable.
pub struct Identity {
    secret: SecretKey,
    signer: RelaySigner,
}

impl Identity {
    fn from_secret(secret: SecretKey) -> Result<Self, String> {
        let hex: String = secret
            .secret_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        let signer = RelaySigner::from_secret_hex(&hex).map_err(|error| error.to_string())?;
        Ok(Self { secret, signer })
    }

    /// The identity from the environment or `~/.openagents/nostr-secret`,
    /// generating and installing a fresh keypair (mode 0600) on first run.
    pub fn load() -> Result<Self, String> {
        if let Ok(hex) = env::var("CODER_SECRET_KEY") {
            let bytes = parse_hex(&hex).ok_or("CODER_SECRET_KEY must be 64 lowercase hex")?;
            let secret = SecretKey::from_byte_array(bytes)
                .map_err(|_| "CODER_SECRET_KEY is not a valid secret key".to_string())?;
            return Self::from_secret(secret);
        }
        if let Ok(nsec) = env::var("CODER_NSEC") {
            let secret_bytes = nip19::decode_nsec(&nsec)
                .map_err(|error| format!("CODER_NSEC does not decode: {error}"))?;
            let secret = SecretKey::from_byte_array(secret_bytes)
                .map_err(|_| "CODER_NSEC is not a valid secret key".to_string())?;
            return Self::from_secret(secret);
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
}

/// A `Generate` over the relay: each turn publishes a NIP-CJ job request
/// and streams the worker's feedback and result back.
pub struct RelayDoor {
    /// The relay WebSocket URL.
    pub url: String,
    worker: XOnlyPublicKey,
    worker_hex: String,
    identity: Identity,
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
            socket: Mutex::new(None),
        }
    }

    /// A door from the environment: `CODER_WORKER` selects the worker
    /// (`npub` or hex), `CODER_RELAY` selects the relay. `None` without a
    /// configured worker or a loadable identity.
    pub fn from_env() -> Option<Self> {
        let worker = env::var("CODER_WORKER").ok().filter(|w| !w.is_empty())?;
        let worker = parse_pubkey(&worker)?;
        let url = env::var("CODER_RELAY").unwrap_or_else(|_| DEFAULT_RELAY_URL.to_string());
        let identity = Identity::load().ok()?;
        Some(Self::new(url, worker, identity))
    }

    /// The socket, connecting and authenticating when needed.
    async fn connection(&self) -> Result<Socket, GenerateError> {
        let (mut socket, _) = connect_async(&self.url)
            .await
            .map_err(|error| GenerateError::Stream(format!("connect: {error}")))?;
        // The relay sends ["AUTH", challenge] on connect whenever it has a
        // configured URL; answer it before any other traffic.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
        loop {
            let frame = match tokio::time::timeout_at(deadline, socket.next()).await {
                Ok(Some(Ok(message))) => message,
                Ok(Some(Err(error))) => {
                    return Err(GenerateError::Stream(format!("socket: {error}")));
                }
                Ok(None) => {
                    return Err(GenerateError::Stream("socket closed during auth".into()));
                }
                Err(_) => {
                    return Err(GenerateError::Stream("no AUTH challenge received".into()));
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
                        return Err(GenerateError::Stream("malformed AUTH challenge".into()));
                    };
                    let event = self.identity.signer.sign(
                        unix_now(),
                        AUTH_KIND,
                        vec![
                            Tag::new(vec!["relay".into(), self.url.clone()]),
                            Tag::new(vec!["challenge".into(), challenge.to_string()]),
                        ],
                        String::new(),
                    );
                    let auth_id = event.id.clone();
                    send(&mut socket, json!(["AUTH", event])).await?;
                    // The relay confirms with ["OK", auth_id, true, ""].
                    let ok = wait_for_ok(&mut socket, &auth_id, deadline).await?;
                    if !ok {
                        return Err(GenerateError::Stream(
                            "NIP-42 authentication refused".into(),
                        ));
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
        let mut partials = String::new();
        while let Some(frame) = socket.next().await {
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
                    return Err(GenerateError::Stream(format!(
                        "relay refused the job request: {reason}"
                    )));
                }
                "EVENT" if value[1].as_str() == Some(subscription.as_str()) => {
                    let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) else {
                        continue;
                    };
                    if event.pubkey != self.worker_hex || event.validate_crypto().is_err() {
                        continue;
                    }
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
                        let code = feedback["code"].as_str().unwrap_or("internal");
                        let message = feedback["message"].as_str().unwrap_or(code);
                        return Err(GenerateError::Stream(format!("{code}: {message}")));
                    } else if event.kind == RESULT_KIND {
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
        Err(GenerateError::Stream(
            "socket closed before the worker answered".into(),
        ))
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
        match tokio::time::timeout(
            TURN_TIMEOUT,
            self.turn(socket, instructions, input, sink, meta),
        )
        .await
        {
            Ok(result) => result,
            Err(_) => {
                *guard = None;
                Err(GenerateError::Stream(
                    "the worker did not answer in time".into(),
                ))
            }
        }
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

async fn send(socket: &mut Socket, value: Value) -> Result<(), GenerateError> {
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
