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
//! answered by declining. `docs/coder/measurements/relay-transport.md` measures all
//! three.
//!
//! The relay is transport, not authority. Its subscription labels are
//! unsigned routing hints, so an answer is bound to a job by what the
//! signature covers — the worker's key, this request's `e` tag, this
//! identity's `p` tag, and an allowed kind — never by the label it
//! arrived under. `tests/relay_binding.rs` runs a loopback relay that
//! relabels, replays, and duplicates signed events to prove it.
//!
//! Payload version 2 adds a signed `seq` to partial deltas; the door
//! streams a delta only when it is the next one, and the first `seq`
//! that is not — early, late, or repeated — closes the stream without
//! buffering. Version-1 partials carry no sequence, so they prove the
//! worker is alive but do not stream text; a version-1 result still
//! completes the job.
//!
//! Silence is two waits, not one. [`CONTACT_TIMEOUT`] bounds the wait for
//! the first sign that a worker is there at all, and [`ANSWER_TIMEOUT`]
//! bounds the wait for the answer once one is. A single bound covering
//! both made an absent worker cost the whole long wait and then report a
//! failure that could not say whether anyone had been listening.

/// Authenticated, bounded reads and publication of exact private artifacts.
pub mod artifacts;

use std::collections::HashSet;
use std::env;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::{SinkExt, StreamExt, stream};
use nostr::domain::{Event, RelaySigner, Tag};
use nostr::{nip19, nip44};
use secp256k1::{SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream, connect_async, tungstenite};

use crate::delegate::{Delegation, Relayed, Status, Task};
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

/// How long opening a socket may take, TCP and TLS handshake and the
/// NIP-42 exchange together. A relay that accepts the connection and then
/// says nothing would otherwise hold the turn for as long as the operating
/// system lets a half-open socket live.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// The payload revision this implementation speaks: `2` adds the signed
/// `seq` that orders partial deltas. Revision `1` is the same protocol
/// without it. [`payload_version`] is the acceptance rule both ends use.
pub const PAYLOAD_VERSION: u64 = 2;

/// The payload revision a message declares, when it is one this NIP
/// defines. The field is required: a missing `v`, a string, or any other
/// value is not this protocol's text. A worker answers at the version the
/// request named, so a version-1 request gets version-1 feedback —
/// partials without `seq` — and a request naming anything else is not a
/// job it takes.
#[must_use]
pub fn payload_version(payload: &Value) -> Option<u64> {
    match payload["v"].as_u64() {
        Some(version) if version == 1 || version == PAYLOAD_VERSION => Some(version),
        _ => None,
    }
}

/// The feedback payload a worker publishes for one delta, at the
/// negotiated version: `seq` appears only where the version defines it,
/// so a version-1 answer makes no ordering promise it cannot keep.
#[must_use]
pub fn partial_payload(version: u64, seq: u64, delta: &str) -> Value {
    let mut payload = json!({"v": version, "type": "partial", "delta": delta});
    if version >= 2 {
        payload["seq"] = json!(seq);
    }
    payload
}

/// The most worker events one job reads before the turn is refused.
/// Bound events are deduplicated by id, and an ephemeral kind with a
/// deadline is not a memory bound: a relay that fans out an unbounded
/// stream of valid-looking events would otherwise grow the set for the
/// whole wait. A thousand events is far past any honest answer — a
/// judgment, a few hundred deltas, a status, a result — so past it the
/// stream is a flood, and floods are errors.
const MAX_ANSWER_EVENTS: usize = 1_024;

/// The most streamed delta bytes one job renders before the turn is
/// refused. Partials are a progress signal — the result carries the
/// answer whole — so a preview this large is already past useful.
const MAX_STREAM_BYTES: usize = 256 * 1_024;

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
        Self::load_from(&PathBuf::from(home).join(".openagents").join("nostr-secret"))
    }

    /// The identity kept at `path`, generated and installed on first run.
    ///
    /// Only a file that is not there is a first run. Any other failure to
    /// read it — permissions, a directory in its place, an I/O fault —
    /// is reported, because generating a new key over an unreadable old
    /// one would silently change the `npub` the worker's ledger knows.
    ///
    /// Installation is atomic: the key is written to a sibling file that
    /// is created exclusively with mode `0600`, so no moment exists where
    /// the file is readable to others or half written, and then linked
    /// into place, which fails rather than replacing a file that appeared
    /// in the meantime. When another process wins that
    /// race, its key is the identity and this one's is discarded.
    ///
    /// # Errors
    ///
    /// Returns a sentence naming why no identity could be loaded.
    pub fn load_from(path: &Path) -> Result<Self, String> {
        match fs::read_to_string(path) {
            Ok(text) => Self::from_file_text(&text, path),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                Self::install(path)?;
                let text = fs::read_to_string(path)
                    .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
                Self::from_file_text(&text, path)
            }
            Err(error) => Err(format!("cannot read {}: {error}", path.display())),
        }
    }

    fn from_file_text(text: &str, path: &Path) -> Result<Self, String> {
        let bytes = parse_hex(text.trim())
            .ok_or_else(|| format!("{} is not 64 lowercase hex", path.display()))?;
        let secret = SecretKey::from_byte_array(bytes)
            .map_err(|_| format!("{} is not a valid secret key", path.display()))?;
        Self::from_secret(secret)
    }

    /// Generates a key and installs it at `path` unless one is there
    /// first. Returns `Ok` either way; the caller reads the winner.
    fn install(path: &Path) -> Result<(), String> {
        let dir = path
            .parent()
            .ok_or_else(|| format!("{} has no parent directory", path.display()))?;
        if !dir.is_dir() {
            fs::create_dir_all(dir)
                .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let _ = fs::set_permissions(dir, fs::Permissions::from_mode(0o700));
            }
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} has no file name", path.display()))?;
        let staged = dir.join(format!(
            ".{name}.{}.{:016x}",
            std::process::id(),
            secp256k1::rand::random::<u64>()
        ));
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let written = (|| -> io::Result<()> {
            let mut file = options.open(&staged)?;
            let secret = SecretKey::new(&mut secp256k1::rand::rng());
            let hex: String = secret
                .secret_bytes()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            file.write_all(format!("{hex}\n").as_bytes())?;
            file.sync_all()?;
            drop(file);
            match fs::hard_link(&staged, path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::AlreadyExists => Ok(()),
                Err(error) => Err(error),
            }
        })();
        let _ = fs::remove_file(&staged);
        written.map_err(|error| format!("cannot install {}: {error}", path.display()))
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
    connect_within(url, identity, CONNECT_TIMEOUT).await
}

/// [`connect`] with the whole of opening the socket, handshake and
/// authentication together, bounded by `within`.
///
/// # Errors
///
/// As [`connect`], plus a [`GenerateError::Relay`] naming the bound when
/// the relay accepted the connection and then did not finish it.
pub async fn connect_within(
    url: &str,
    identity: &Identity,
    within: Duration,
) -> Result<Socket, GenerateError> {
    let deadline = tokio::time::Instant::now() + within;
    let (mut socket, _) = tokio::time::timeout_at(deadline, connect_async(url))
        .await
        .map_err(|_| {
            GenerateError::Relay(format!(
                "connect: no WebSocket handshake in {} seconds",
                within.as_secs()
            ))
        })?
        .map_err(|error| GenerateError::Relay(format!("connect: {error}")))?;
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

/// One request as it goes onto the wire: the signed event, the
/// subscription its answers arrive on, the key they decrypt under, and how
/// long to wait for them once the worker has been heard from.
#[derive(Clone, Copy)]
struct Posted<'a> {
    request: &'a Event,
    subscription: &'a str,
    conversation: &'a [u8; 32],
    answer: Duration,
}

/// What one NIP-CJ job came back with: the result payload and what
/// arrived before it.
#[derive(Clone, Debug)]
pub struct Answer {
    /// The result's text, whole.
    pub text: String,
    /// Token usage, when the worker reported it.
    pub usage: Option<Usage>,
    /// The model the worker named, when it named one.
    pub model: Option<String>,
    /// The decrypted kind-26900 result payload, for fields this door does
    /// not read itself — a probe's answer lives here.
    pub result: Value,
    /// How many kind-27000 feedback events were bound to the job before
    /// the result.
    pub feedback: usize,
}

/// What a worker says about itself when probed, without running anything.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Probed {
    /// The door the worker answers through, as it names it.
    pub door: String,
    /// The model behind that door.
    pub model: String,
    /// Whether the worker runs delegated tasks through an executor door.
    /// A worker that only generates cannot take a writing task.
    pub delegates: bool,
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
    connect: Duration,
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
            connect: CONNECT_TIMEOUT,
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

    /// The same door with a different bound on opening a socket.
    #[must_use]
    pub fn connecting(mut self, connect: Duration) -> Self {
        self.connect = connect;
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

    /// The relay this door publishes to.
    #[must_use]
    pub fn relay(&self) -> &str {
        &self.url
    }

    /// The worker this door names in every request's `p` tag, hex.
    #[must_use]
    pub fn worker(&self) -> &str {
        &self.worker_hex
    }

    /// The socket, connecting and authenticating when needed.
    async fn connection(&self) -> Result<Socket, GenerateError> {
        connect_within(&self.url, &self.identity, self.connect).await
    }

    /// Publishes one job carrying `payload` on a socket of its own and
    /// waits for its answer.
    ///
    /// A fan-out runs several of these at once, and each takes its own
    /// connection rather than the shared one so their frames never
    /// interleave and a slow job never holds a fast one's socket. The
    /// payload is what the worker reads; this method adds the version,
    /// encrypts, signs, and binds the answer. The request's event ID
    /// comes back beside the answer, because it is the job's name in the
    /// relay's log and a record of the delegation needs it.
    ///
    /// # Errors
    ///
    /// The same vocabulary as a turn: [`GenerateError::Relay`] when the
    /// relay could not be reached or refused the request,
    /// [`GenerateError::Silent`] when nobody answered,
    /// [`GenerateError::Refused`] when the worker said no with a code.
    pub async fn job(
        &self,
        payload: Value,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> (String, Result<Answer, GenerateError>) {
        self.job_within(payload, self.answer, sink).await
    }

    /// [`RelayDoor::job`] with its own wait for the answer once the worker
    /// has been heard from. A delegated task is allowed the bound it was
    /// given, not the door's default.
    async fn job_within(
        &self,
        mut payload: Value,
        answer: Duration,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> (String, Result<Answer, GenerateError>) {
        if let Some(object) = payload.as_object_mut() {
            object.insert("v".to_string(), json!(PAYLOAD_VERSION));
        }
        let (request, conversation) = match self.request_payload(&payload) {
            Ok(signed) => signed,
            Err(error) => return (String::new(), Err(error)),
        };
        let mut socket = match self.connection().await {
            Ok(socket) => socket,
            Err(error) => return (request.id, Err(error)),
        };
        let subscription = format!("job-{}", &request.id[..16]);
        let mut meta = |_: Meta| {};
        let answered = self
            .exchange(
                &mut socket,
                &Posted {
                    request: &request,
                    subscription: &subscription,
                    conversation: &conversation,
                    answer,
                },
                sink,
                &mut meta,
            )
            .await;
        // The socket is this job's alone and closes with it, so a CLOSE
        // it will not take changes nothing the answer has not said.
        let _ = send(&mut socket, json!(["CLOSE", subscription])).await;
        (request.id, answered)
    }

    /// Asks the worker whether it is there and what answers through it.
    ///
    /// A probe costs the worker nothing: it answers with its door's name
    /// and model and never generates. The three outcomes a capability
    /// probe needs are the three this returns: `Ok` when a worker
    /// answered, [`GenerateError::Relay`] when the relay itself could not
    /// be reached, and anything else when the relay is there and the
    /// worker is not or would not.
    ///
    /// # Errors
    ///
    /// As [`RelayDoor::job`].
    pub async fn probe(&self) -> Result<Probed, GenerateError> {
        let mut sink = |_: &str| {};
        let (_, answer) = self.job(json!({"type": "probe"}), &mut sink).await;
        let answer = answer?;
        let probe = &answer.result["probe"];
        if !probe.is_object() {
            return Err(GenerateError::Stream(
                "the worker answered the probe without describing itself".into(),
            ));
        }
        Ok(Probed {
            door: probe["door"].as_str().unwrap_or_default().to_string(),
            model: probe["model"].as_str().unwrap_or_default().to_string(),
            delegates: probe["delegates"].as_bool().unwrap_or(false),
        })
    }

    /// Runs one job over `socket`: subscribe, publish, stream the answer,
    /// and close the subscription on every path that keeps the socket.
    ///
    /// A subscription left open outlives its job: the next turn's frames
    /// would interleave with a late answer to this one, and a relay that
    /// caps subscriptions per connection would eventually refuse the
    /// `REQ` outright. So the `CLOSE` is sent whether the job ended in a
    /// result, a typed refusal, or a relay error. The paths that skip it
    /// are the ones where the socket is evicted anyway: a broken or
    /// closed socket, and a wait that ran out, where a late answer could
    /// still arrive and the next turn must not be the one to read it.
    async fn turn(
        &self,
        socket: &mut Socket,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let (request, conversation) = self.request(instructions, input)?;
        let subscription = format!("job-{}", &request.id[..16]);
        let answered = self
            .exchange(
                socket,
                &Posted {
                    request: &request,
                    subscription: &subscription,
                    conversation: &conversation,
                    answer: self.answer,
                },
                sink,
                meta,
            )
            .await;
        if keeps_socket(&answered) {
            // A `CLOSE` the socket will not take means the socket is not
            // one the next turn can use either; refiling the outcome as
            // a stream failure is what evicts it.
            send(socket, json!(["CLOSE", subscription])).await?;
        }
        answered.map(|answer| (answer.text, answer.usage))
    }

    /// The signed, encrypted job request for this turn's input, with the
    /// conversation key its answers decrypt under.
    fn request(
        &self,
        instructions: &str,
        input: &[Message],
    ) -> Result<(Event, [u8; 32]), GenerateError> {
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
            "v": PAYLOAD_VERSION,
            "task": task,
            "transcript": transcript,
            "instructions": instructions,
            "client": concat!("coder ", env!("CARGO_PKG_VERSION")),
        });
        self.request_payload(&payload)
    }

    /// `payload`, encrypted to the worker and signed as a kind-25900
    /// request naming it, with the conversation key its answers decrypt
    /// under.
    fn request_payload(&self, payload: &Value) -> Result<(Event, [u8; 32]), GenerateError> {
        let payload = payload.to_string();
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
        Ok((request, conversation))
    }

    /// Subscribes, publishes `request`, and reads until the answer, a
    /// refusal, or a wait runs out.
    async fn exchange(
        &self,
        socket: &mut Socket,
        posted: &Posted<'_>,
        sink: &mut (dyn FnMut(&str) + Send),
        meta: &mut (dyn FnMut(Meta) + Send),
    ) -> Result<Answer, GenerateError> {
        let Posted {
            request,
            subscription,
            conversation,
            answer,
        } = *posted;
        // Subscribe before publishing so no fast feedback is missed.
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
        let mut next_partial = 0u64;
        let mut partials_open = true;
        let mut streamed_bytes = 0usize;
        let mut seen = HashSet::new();
        let mut heard = false;
        let mut feedback_count = 0usize;
        let answer_by = tokio::time::Instant::now() + answer;
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
                Err(_) => return Err(self.ran_out(heard, answer)),
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
                // The relay ending the subscription ends the job: nothing
                // more can arrive under it, and a request already
                // published is unanswerable rather than pending. The
                // socket itself is fine, and the reason is the relay's
                // to give, so this is a relay error and not a silence.
                "CLOSED" if value[1].as_str() == Some(subscription) => {
                    let reason = value[2].as_str().unwrap_or("no reason given");
                    return Err(GenerateError::Relay(format!(
                        "the relay closed the job subscription: {reason}"
                    )));
                }
                // The subscription label is the relay's routing hint, not
                // the job's identity: a relay can relabel any event under
                // it. What binds an answer to this turn is inside the
                // signature — the kind, the worker's key, this request's
                // `e` tag, and this identity's `p` tag — so those are what
                // get checked before the payload is read.
                "EVENT" => {
                    let Ok(event) = serde_json::from_value::<Event>(value[2].clone()) else {
                        continue;
                    };
                    if !self.binds(&event, &request.id) {
                        continue;
                    }
                    // A relay may deliver one event twice; a delta
                    // delivered twice must not display twice. The
                    // deduplication follows the checks so a forged event
                    // cannot claim a genuine event's id and suppress it,
                    // and the set is bounded so a flood of bound events
                    // is a refusal rather than a memory leak.
                    if !seen.insert(event.id.clone()) {
                        continue;
                    }
                    if seen.len() > MAX_ANSWER_EVENTS {
                        return Err(GenerateError::Stream(format!(
                            "the worker sent more than {MAX_ANSWER_EVENTS} events for one job"
                        )));
                    }
                    let Ok(plaintext) = nip44::decrypt(&event.content, conversation) else {
                        continue;
                    };
                    let Ok(feedback) = serde_json::from_str::<Value>(&plaintext) else {
                        continue;
                    };
                    // The payload must name a version this NIP defines
                    // before its `type` means anything.
                    let Some(version) = payload_version(&feedback) else {
                        continue;
                    };
                    if event.kind == FEEDBACK_KIND {
                        feedback_count += 1;
                    }
                    // The `type` must be one the event's kind carries and
                    // the fields it needs must be there: a bound event
                    // with an unknown, mismatched, or malformed payload
                    // is neither contact nor text. Only a well-formed
                    // payload of a known type moves the wait to the long
                    // one.
                    match (event.kind, feedback["type"].as_str().unwrap_or_default()) {
                        (FEEDBACK_KIND, "judgment") => {
                            let Some(line) = feedback["line"].as_str() else {
                                continue;
                            };
                            heard = true;
                            meta(Meta::Judgment(line.to_string()));
                        }
                        (FEEDBACK_KIND, "status") => {
                            let Some(status) = feedback["status"].as_str() else {
                                continue;
                            };
                            if !matches!(status, "queued" | "processing" | "error") {
                                continue;
                            }
                            heard = true;
                            if status == "error" {
                                // A typed refusal is an answer: a worker
                                // read the job and said no, with a reason
                                // a caller can act on. It is not the
                                // transport failing.
                                let code = feedback["code"].as_str().unwrap_or("internal");
                                let message = feedback["message"].as_str().unwrap_or(code);
                                return Err(GenerateError::Refused {
                                    code: code.to_string(),
                                    message: message.to_string(),
                                });
                            }
                        }
                        (FEEDBACK_KIND, "partial") => {
                            let Some(delta) = feedback["delta"].as_str() else {
                                continue;
                            };
                            if version == 1 {
                                // A version-1 partial carries no
                                // sequence, so its order is the relay's
                                // word. It proves the worker is there; it
                                // does not stream text.
                                heard = true;
                                continue;
                            }
                            let Some(seq) = feedback["seq"].as_u64() else {
                                continue;
                            };
                            heard = true;
                            // Deltas display only in the signed order.
                            // The first `seq` that is not next — early,
                            // late, or repeated — closes the stream:
                            // nothing is buffered and nothing after it is
                            // text. The result carries the answer whole
                            // regardless.
                            if partials_open && seq == next_partial {
                                next_partial += 1;
                                streamed_bytes += delta.len();
                                if streamed_bytes > MAX_STREAM_BYTES {
                                    return Err(GenerateError::Stream(format!(
                                        "the worker streamed more than {MAX_STREAM_BYTES} \
                                         bytes of deltas"
                                    )));
                                }
                                sink(delta);
                            } else {
                                partials_open = false;
                            }
                        }
                        (RESULT_KIND, "result") => {
                            // The worker names the model it used, and the
                            // door used to drop it on the floor. A relay
                            // run's evidence has to be able to say what
                            // answered.
                            let model = feedback["model"]
                                .as_str()
                                .filter(|m| !m.is_empty())
                                .map(str::to_string);
                            if let Some(model) = &model {
                                meta(Meta::Model(model.clone()));
                            }
                            // The result is the answer, whole. Deltas are
                            // a preview of it, never a substitute: an
                            // empty result is an empty answer, however
                            // much the stream showed.
                            let text = feedback["text"].as_str().unwrap_or_default();
                            if text.is_empty() {
                                return Err(GenerateError::Stream(
                                    "the worker's result carried no text".into(),
                                ));
                            }
                            let usage = feedback["usage"].as_object().map(|usage| Usage {
                                input_tokens: usage["input"].as_u64().unwrap_or(0),
                                output_tokens: usage["output"].as_u64().unwrap_or(0),
                            });
                            return Ok(Answer {
                                text: text.to_string(),
                                usage,
                                model,
                                result: feedback,
                                feedback: feedback_count,
                            });
                        }
                        _ => {}
                    }
                }
                _ => {}
            }
        }
    }

    /// Whether `event` is this job's answer and nobody else's.
    ///
    /// Every field checked is inside the event's signature, so a relay
    /// cannot arrange any of them: the kind must be feedback or a result,
    /// the signer must be the worker's key, an `e` tag must name the
    /// request this turn published, and a `p` tag must name this
    /// identity. A signature over someone else's job is still a valid
    /// signature, which is why a check that stops at "signed by the
    /// worker" accepts replays.
    fn binds(&self, event: &Event, request_id: &str) -> bool {
        if event.kind != RESULT_KIND && event.kind != FEEDBACK_KIND {
            return false;
        }
        if event.pubkey != self.worker_hex || event.validate_crypto().is_err() {
            return false;
        }
        event.tag_values("e").any(|id| id == request_id)
            && event
                .tag_values("p")
                .any(|pubkey| pubkey == self.identity.pubkey())
    }

    /// The failure a wait that ran out produces, in whichever of the two
    /// waits it was.
    fn ran_out(&self, heard: bool, answer: Duration) -> GenerateError {
        let waited = if heard { answer } else { self.contact };
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

impl RelayDoor {
    /// Hands one bounded task to the worker as one NIP-CJ job and reports
    /// it the way a local delegation is reported.
    ///
    /// Nothing runs here. The task's `writes` and its wall bound travel in
    /// the payload's `delegation` object, and the worker applies them under
    /// its own approval. The wait is the task's bound plus the contact
    /// wait, because a worker that is there answers within what the task
    /// was allowed, and one that never says anything is a silence rather
    /// than a slow answer.
    pub async fn delegate(&self, capability: &str, task: Task, width: usize) -> Delegation {
        let started = std::time::Instant::now();
        let minutes = task.bounds.wall().as_secs().div_ceil(60).max(1);
        let payload = json!({
            "task": task.prompt,
            "delegation": {
                "writes": task.writes,
                "minutes": minutes,
            },
            "client": concat!("coder ", env!("CARGO_PKG_VERSION")),
        });
        let mut sink = |_: &str| {};
        let (request, answered) = self
            .job_within(payload, task.bounds.wall() + self.contact, &mut sink)
            .await;
        let mut relayed = Relayed {
            relay: self.url.clone(),
            worker: self.worker_hex.clone(),
            request,
            model: None,
            feedback: 0,
        };
        let (status, output, detail) = match answered {
            Ok(answer) => {
                relayed.model = answer.model.clone();
                relayed.feedback = answer.feedback;
                (Status::Answered, answer.text, String::new())
            }
            Err(GenerateError::Refused { code, message }) => {
                (Status::Refused(code), String::new(), message)
            }
            Err(GenerateError::Silent {
                heard: true,
                reason,
            }) => (Status::TimedOut, String::new(), reason),
            Err(error) => (
                Status::Harness(error.to_string()),
                String::new(),
                String::new(),
            ),
        };
        let bytes = (output.len() + detail.len()) as u64;
        Delegation {
            task,
            capability: capability.to_string(),
            binary: PathBuf::from(&self.url),
            workdir: PathBuf::from(&self.worker_hex),
            concurrent_max: width,
            status,
            output,
            detail,
            bytes,
            elapsed: started.elapsed(),
            boundary: None,
            retained: None,
            relayed: Some(relayed),
        }
    }

    /// Runs `tasks` as jobs, at most `width` in flight, and reports each.
    ///
    /// The width is the terminal's promise about how many jobs it opens
    /// at once. The worker holds its own bound and refuses past it with
    /// `busy`, which arrives here as a typed refusal rather than a
    /// failure.
    pub async fn fan_out(
        &self,
        capability: &str,
        tasks: Vec<Task>,
        width: usize,
    ) -> Vec<Delegation> {
        stream::iter(
            tasks
                .into_iter()
                .map(|task| self.delegate(capability, task, width)),
        )
        .buffered(width.max(1))
        .collect()
        .await
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
        //
        // The socket is taken out of the door for the turn and put back
        // only when the turn ends with it trustworthy. A caller that drops
        // the turn midway, a cancellation, drops the socket with it, which
        // closes the connection and every subscription on it at the relay.
        // Cancelling leaves nothing behind for the next turn to find.
        let mut guard = self.socket.lock().await;
        let mut socket = match guard.take() {
            Some(socket) => socket,
            None => self.connection().await?,
        };
        let answered = self
            .turn(&mut socket, instructions, input, sink, meta)
            .await;
        // A wait that ran out or a socket that broke leaves a connection
        // nobody can trust: the subscription is still open and a late
        // answer would arrive in the middle of the next turn. A refusal
        // leaves the socket healthy, its subscription closed, and the
        // next turn reuses it.
        if keeps_socket(&answered) {
            *guard = Some(socket);
        }
        answered
    }
}

/// Whether the socket a turn ran over is one the next turn can reuse.
///
/// A result and a typed refusal both mean the exchange completed in
/// order. A relay error is the relay declining a frame or a subscription
/// on a socket that still works. A silence is a wait that ran out with a
/// subscription that may yet deliver, and a stream failure is the socket
/// itself; neither socket is trusted again.
fn keeps_socket<T>(answered: &Result<T, GenerateError>) -> bool {
    !matches!(
        answered,
        Err(GenerateError::Silent { .. } | GenerateError::Stream(_))
    )
}

/// Refiles a socket failure raised while the connection was being opened.
/// Nothing had reached a worker yet, so the relay owns it.
fn as_relay(error: GenerateError) -> GenerateError {
    match error {
        GenerateError::Stream(why) => GenerateError::Relay(why),
        other => GenerateError::Relay(other.to_string()),
    }
}

/// Reads a public key written as an `npub` or 64 lowercase hex characters.
pub fn parse_pubkey(text: &str) -> Option<XOnlyPublicKey> {
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
    fn a_first_run_installs_a_private_identity_and_a_second_run_reads_it() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("deeper").join("nostr-secret");
        let first = Identity::load_from(&path).unwrap();
        let second = Identity::load_from(&path).unwrap();
        assert_eq!(first.pubkey(), second.pubkey());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600, "{mode:o}");
        }
        // The staging file is gone, whichever way installation went.
        let names: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .map(|entry| entry.unwrap().file_name())
            .collect();
        assert_eq!(names, vec![std::ffi::OsString::from("nostr-secret")]);
    }

    #[test]
    fn concurrent_first_runs_agree_on_one_identity() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nostr-secret");
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let path = path.clone();
                std::thread::spawn(move || Identity::load_from(&path).unwrap().pubkey().to_string())
            })
            .collect();
        let pubkeys: HashSet<String> = handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect();
        assert_eq!(pubkeys.len(), 1, "{pubkeys:?}");
        let on_disk = Identity::load_from(&path).unwrap();
        assert!(pubkeys.contains(on_disk.pubkey()));
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_identity_is_an_error_not_a_first_run() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc_geteuid() } == 0 {
            // Root reads anything; the case cannot be staged.
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nostr-secret");
        let before = Identity::load_from(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
        let error = Identity::load_from(&path).err().expect("an error");
        assert!(error.contains("cannot read"), "{error}");
        assert!(!error.contains(before.pubkey()));
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
        assert_eq!(
            Identity::load_from(&path).unwrap().pubkey(),
            before.pubkey()
        );
    }

    #[test]
    fn a_directory_in_the_identitys_place_is_an_error_not_a_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nostr-secret");
        fs::create_dir(&path).unwrap();
        let error = Identity::load_from(&path).err().expect("an error");
        assert!(error.contains("cannot read"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn a_directory_that_cannot_take_the_file_fails_the_install_and_leaves_nothing() {
        use std::os::unix::fs::PermissionsExt;
        if unsafe { libc_geteuid() } == 0 {
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o500)).unwrap();
        let path = dir.path().join("nostr-secret");
        let error = Identity::load_from(&path).err().expect("an error");
        assert!(error.contains("cannot install"), "{error}");
        assert_eq!(fs::read_dir(dir.path()).unwrap().count(), 0);
        fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o700)).unwrap();
    }

    #[cfg(unix)]
    unsafe extern "C" {
        #[link_name = "geteuid"]
        fn libc_geteuid() -> u32;
    }

    #[test]
    fn payload_versions_are_explicit() {
        assert_eq!(payload_version(&json!({"v": 1})), Some(1));
        assert_eq!(payload_version(&json!({"v": 2})), Some(2));
        // The field is required and numeric: absence, a string, and every
        // other value are all unsupported.
        assert_eq!(payload_version(&json!({})), None);
        assert_eq!(payload_version(&json!({"v": "2"})), None);
        assert_eq!(payload_version(&json!({"v": 0})), None);
        assert_eq!(payload_version(&json!({"v": 3})), None);
    }

    #[test]
    fn a_worker_answers_at_the_version_the_request_named() {
        // The worker's response version is the request's: a version-1
        // request gets version-1 feedback with no `seq` promised, and a
        // version-2 request gets ordered partials.
        let v1 = partial_payload(1, 7, "delta");
        assert_eq!(v1["v"], 1);
        assert!(v1.get("seq").is_none());
        let v2 = partial_payload(2, 7, "delta");
        assert_eq!(v2["v"], 2);
        assert_eq!(v2["seq"], 7);
        // And what it produces is what the door accepts.
        assert_eq!(payload_version(&v1), Some(1));
        assert_eq!(payload_version(&v2), Some(2));
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
