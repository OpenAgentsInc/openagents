//! How every Jev caller finds Jev.
//!
//! Coder asks Jev (TypeSafe's System One model) for its judgments. A
//! computer with a TypeSafe key asks TypeSafe directly, as before. A
//! computer without one — every ordinary user's — asks OpenAgents' hosted
//! decision service instead: the same `POST /v1/systemone` call, carried as
//! a NIP-CJ decision job (kind `25910`, `nips/openagents/NIP-CJ.md`) to a
//! worker on `wss://relay.openagents.com` that holds the key on the server
//! (`docs/deployment/decision-worker.md`). The job is signed by this
//! computer's decision key, which the worker meters per key and in total.
//!
//! [`resolve`] is the one resolver: a local key when there is one, else
//! the hosted service, else no Jev and the reason. A hosted call that
//! cannot be answered fails with a message that says why — "Jev is
//! unreachable: …" or "Jev refused: quota …" — and [`unavailable`] reads
//! that reason back out of a [`jev::Error`], so a caller can stop asking
//! and say so once.
//!
//! The hosted client is an ordinary [`jev::Client`] whose attempts go
//! through [`RelayExchange`]; its [`jev::Client::base_url`] names the door
//! the worker reaches (`https://api.typesafe.ai`) and
//! [`jev::Client::service`] names the worker and relay, so evidence
//! records which service answered.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use jev::exchange::{Call, Exchange, Failure, Pending, Reply};
use nostr::decision::{self, Answer, Outcome, Pending as Job, RequestBody, Seal};
use nostr::domain::{Event, RelaySigner};
use nostr::nip44;
use secp256k1::{Keypair, Secp256k1, SecretKey, XOnlyPublicKey};
use serde_json::{Value, json};

/// The relay the hosted decision worker serves on.
pub const RELAY: &str = "wss://relay.openagents.com";

/// The hosted decision worker's public key, hex. Its secret is only on the
/// worker's host (`docs/deployment/decision-worker.md`).
pub const WORKER: &str = "ad6b4d9199bf0864b1a402116d44e36daa1a72c6f8df4ae07bd57c8df5c922fc";

/// The door the hosted worker reaches: TypeSafe's System One API.
pub const DOOR: &str = "https://api.typesafe.ai";

/// Names another relay for the hosted service, for a fixture or staging.
pub const RELAY_VAR: &str = "OPENAGENTS_JEV_RELAY";
/// Names another worker public key for the hosted service.
pub const WORKER_VAR: &str = "OPENAGENTS_JEV_WORKER";
/// `off` turns the hosted service off: no local key then means no Jev.
pub const HOSTED_VAR: &str = "OPENAGENTS_JEV_HOSTED";

/// The file under `~/.openagents` holding this computer's decision key.
pub const KEY_FILE: &str = "decision.key";

/// How long one hosted attempt may take when the caller set nothing
/// tighter: a relay round trip on top of the door's own time.
pub const HOSTED_TIMEOUT: Duration = Duration::from_secs(30);

const TYPESAFE_KEY_VAR: &str = "TYPESAFE_API_KEY";
const JEV_FILE: &str = "jev.json";

/// What a resolved client is, for a caller's log line and evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Via {
    /// TypeSafe directly, with the key from `source` (a variable or path).
    Direct { source: String },
    /// The hosted decision service.
    Hosted {
        /// The worker's public key, hex.
        worker: String,
        /// The relay the job travels.
        relay: String,
        /// The decision key file that signs the jobs.
        key: PathBuf,
    },
}

impl std::fmt::Display for Via {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Via::Direct { source } => write!(f, "TypeSafe directly, key from {source}"),
            Via::Hosted { worker, relay, .. } => write!(
                f,
                "the OpenAgents hosted decision service ({}… on {relay})",
                worker.get(..8).unwrap_or(worker)
            ),
        }
    }
}

/// A resolved Jev client and how it reaches Jev.
#[derive(Debug, Clone)]
pub struct Resolved {
    pub client: jev::Client,
    pub via: Via,
}

/// What the caller pins: the door and model its evidence names.
#[derive(Debug, Clone)]
pub struct Door<'a> {
    /// The door's URL, such as [`DOOR`].
    pub url: &'a str,
    /// The model, such as `jev-1.13.0`.
    pub model: &'a str,
}

/// Find Jev: a local TypeSafe key if present (`TYPESAFE_API_KEY`, else
/// `api_key` in `<dir>/jev.json`), else the hosted decision service, else
/// the reason there is none.
///
/// `dir` is `~/.openagents`. `tune` adjusts either client's settings — a
/// timeout, a retry policy — and must not set a key or a base URL.
///
/// # Errors
///
/// The sentence that says why this computer has no Jev: the hosted
/// service is off or does not front `door`, or the decision key cannot be
/// read or made. It never carries a key.
pub fn resolve(
    env: &dyn Fn(&str) -> Option<String>,
    dir: &Path,
    door: &Door<'_>,
    tune: &dyn Fn(jev::Config) -> jev::Config,
) -> Result<Resolved, String> {
    if let Some((key, source)) = local_key(env, dir) {
        let client = jev::Client::new(
            tune(jev::Config::new().api_key(key))
                .base_url(door.url)
                .default_model(door.model),
        )
        .map_err(|error| format!("Jev: {error}"))?;
        return Ok(Resolved {
            client,
            via: Via::Direct { source },
        });
    }
    if env(HOSTED_VAR).is_some_and(|value| value.trim() == "off") {
        return Err(format!(
            "no TypeSafe key here, and {HOSTED_VAR}=off turns the hosted decision service off"
        ));
    }
    if door.url.trim_end_matches('/') != DOOR {
        return Err(format!(
            "no TypeSafe key here, and the hosted decision service answers for {DOOR}, not {}",
            door.url
        ));
    }
    let relay = present(env(RELAY_VAR)).unwrap_or_else(|| RELAY.to_string());
    let worker = present(env(WORKER_VAR)).unwrap_or_else(|| WORKER.to_string());
    let key_path = dir.join(KEY_FILE);
    let secret = decision_key(&key_path)?;
    let exchange = RelayExchange::new(&relay, &worker, secret)?;
    let client = jev::Client::new(
        tune(jev::Config::new().timeout(HOSTED_TIMEOUT))
            .exchange(Arc::new(exchange))
            .base_url(door.url)
            .default_model(door.model),
    )
    .map_err(|error| format!("Jev: {error}"))?;
    Ok(Resolved {
        client,
        via: Via::Hosted {
            worker,
            relay,
            key: key_path,
        },
    })
}

/// The local TypeSafe key and where it came from, when this computer has
/// one. Values are trimmed; blank reads as unset.
#[must_use]
pub fn local_key(env: &dyn Fn(&str) -> Option<String>, dir: &Path) -> Option<(String, String)> {
    if let Some(key) = present(env(TYPESAFE_KEY_VAR)) {
        return Some((key, format!("${TYPESAFE_KEY_VAR}")));
    }
    let path = dir.join(JEV_FILE);
    let text = std::fs::read_to_string(&path).ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let key = present(value.get("api_key")?.as_str().map(str::to_string))?;
    Some((key, path.display().to_string()))
}

/// `~/.openagents`, when the process has a home directory.
#[must_use]
pub fn openagents_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".openagents"))
}

/// Why a hosted call got no answer, when the error says Jev is
/// unavailable rather than that one call went wrong: the service was
/// unreachable, or it refused this computer's key its quota. A caller that
/// gets `Some` should stop asking for the rest of its task and say so.
#[must_use]
pub fn unavailable(error: &jev::Error) -> Option<String> {
    match error {
        jev::Error::Connection { message, .. } if message.starts_with(UNREACHABLE) => {
            Some(message.clone())
        }
        jev::Error::Api(api) => {
            let code = api
                .body
                .as_ref()
                .and_then(jev::ResponseBody::as_json)
                .and_then(|body| body["error"]["code"].as_str().map(str::to_string))?;
            matches!(
                code.as_str(),
                "quota_exhausted" | "rate_limited" | "not_admitted"
            )
            .then(|| api.message())
        }
        _ => None,
    }
}

const UNREACHABLE: &str = "Jev is unreachable";

/// Read this computer's decision key, making it on first use: 32 random
/// bytes as hex, in a file only this user can read.
///
/// # Errors
///
/// The file cannot be read, is not a key, or cannot be made.
pub fn decision_key(path: &Path) -> Result<SecretKey, String> {
    match std::fs::read_to_string(path) {
        Ok(text) => parse_secret(text.trim())
            .ok_or_else(|| format!("{} is not a decision key", path.display())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("cannot create {}: {error}", parent.display()))?;
            }
            let secret = SecretKey::new(&mut secp256k1::rand::rng());
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let written = options.open(path).and_then(|mut file| {
                use std::io::Write;
                file.write_all(format!("{}\n", secret.display_secret()).as_bytes())
            });
            match written {
                Ok(()) => Ok(secret),
                // Another process made it first: use theirs.
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    decision_key(path)
                }
                Err(error) => Err(format!("cannot write {}: {error}", path.display())),
            }
        }
        Err(error) => Err(format!("cannot read {}: {error}", path.display())),
    }
}

/// A [`jev::exchange::Exchange`] that carries each `POST /v1/systemone`
/// attempt as a NIP-CJ decision job to the hosted worker.
///
/// Each attempt opens one authenticated relay connection, subscribes to
/// the worker's answers to this key before publishing (the decision kinds
/// are ephemeral: a relay delivers them only to a subscription open at the
/// time), publishes the signed, encrypted request, and reads until the
/// worker's result or terminal refusal. Every answer is checked by
/// [`decision::bind_answer`]: the worker's signature, this request's event
/// id, this key, and the receipt's binding to the request's digest.
pub struct RelayExchange {
    relay: String,
    worker: String,
    worker_key: XOnlyPublicKey,
    secret: SecretKey,
    me: String,
    signer: RelaySigner,
}

impl std::fmt::Debug for RelayExchange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RelayExchange")
            .field("relay", &self.relay)
            .field("worker", &self.worker)
            .field("me", &self.me)
            .finish_non_exhaustive()
    }
}

impl RelayExchange {
    /// An exchange to `worker` (hex public key) on `relay`, signing with
    /// `secret`.
    ///
    /// # Errors
    ///
    /// `worker` is not a public key.
    pub fn new(relay: &str, worker: &str, secret: SecretKey) -> Result<Self, String> {
        let worker_key: XOnlyPublicKey = worker
            .parse()
            .map_err(|_| format!("the decision worker's key {worker} is not a public key"))?;
        let signer = RelaySigner::from_secret_hex(&secret.display_secret().to_string())
            .map_err(|error| error.to_string())?;
        let me = Keypair::from_secret_key(&Secp256k1::new(), &secret)
            .x_only_public_key()
            .0
            .to_string();
        Ok(Self {
            relay: relay.to_string(),
            worker: worker.to_string(),
            worker_key,
            secret,
            me,
            signer,
        })
    }

    /// This computer's public key, as the worker meters it.
    #[must_use]
    pub fn caller(&self) -> &str {
        &self.me
    }

    async fn carry(&self, call: Call) -> Result<Reply, Failure> {
        if call.method != "POST" || call.path != "/v1/systemone" {
            return Ok(refusal_reply(
                404,
                "not_found",
                &format!(
                    "The hosted decision service answers POST /v1/systemone, not {} {}.",
                    call.method, call.path
                ),
                None,
            ));
        }
        let envelope: Value = call
            .body
            .as_deref()
            .and_then(|body| serde_json::from_slice(body).ok())
            .ok_or_else(|| Failure::Unreachable(format!("{UNREACHABLE}: the call has no body")))?;
        let model = envelope["model"].as_str().unwrap_or_default().to_string();
        let questions = envelope["questions"]
            .as_object()
            .cloned()
            .unwrap_or_default();
        let (request, attempt) = match &call.idempotency_key {
            Some(key) => (key.clone(), call.attempt.max(1)),
            None => (random_id(), 1),
        };
        // The worker stops trying past the deadline; leave room for the
        // answer to travel back.
        let budget = call
            .timeout
            .saturating_sub(Duration::from_millis(250))
            .max(Duration::from_secs(1));
        let deadline = unix_now() + budget.as_secs().max(1) + 1;
        let body = RequestBody::new(
            request,
            attempt,
            model,
            envelope["state"].clone(),
            questions,
        )
        .deadline(deadline);
        let event = decision::request_event(self.seal(), &body, &self.worker).map_err(|error| {
            Failure::Unreachable(format!(
                "{UNREACHABLE}: the request cannot be sealed: {error}"
            ))
        })?;
        match tokio::time::timeout(budget, self.round_trip(&body, &event, budget)).await {
            Ok(result) => result,
            Err(_) => Err(Failure::Unreachable(format!(
                "{UNREACHABLE}: the hosted decision service on {} did not answer within {} s",
                self.relay,
                budget.as_secs()
            ))),
        }
    }

    async fn round_trip(
        &self,
        body: &RequestBody,
        event: &Event,
        budget: Duration,
    ) -> Result<Reply, Failure> {
        let unreachable = |why: String| {
            Failure::Unreachable(format!(
                "{UNREACHABLE}: the hosted decision service on {} {why}",
                self.relay
            ))
        };
        let lifetime = (budget + Duration::from_secs(2)).min(Duration::from_secs(120));
        let mut connection =
            nostr_transport::Connection::connect(&self.relay, &self.secret, lifetime)
                .await
                .map_err(|error| unreachable(format!("could not be reached: {error}")))?
                .with_frame_budget(512);
        connection
            .send(json!(["REQ", "jev", {
                "kinds": [decision::RESULT_KIND, decision::FEEDBACK_KIND],
                "#p": [self.me],
            }]))
            .await
            .map_err(|error| unreachable(format!("dropped the subscription: {error}")))?;
        loop {
            let frame = connection
                .next()
                .await
                .map_err(|error| unreachable(format!("dropped the subscription: {error}")))?;
            match frame[0].as_str() {
                Some("EOSE") if frame[1] == "jev" => break,
                Some("CLOSED") => {
                    return Err(unreachable(format!(
                        "refused the subscription: {}",
                        frame[2].as_str().unwrap_or("closed")
                    )));
                }
                _ => {}
            }
        }
        connection
            .send(json!(["EVENT", event]))
            .await
            .map_err(|error| unreachable(format!("dropped the request: {error}")))?;
        let pending = Job {
            attempt_id: &event.id,
            worker: &self.worker,
            customer: &self.me,
            request: &body.request,
            attempt: body.attempt,
            request_digest: body.digest(),
        };
        loop {
            let frame = connection
                .next()
                .await
                .map_err(|error| unreachable(format!("stopped answering: {error}")))?;
            match frame[0].as_str() {
                Some("OK") if frame[1] == event.id.as_str() && frame[2] == false => {
                    return Err(unreachable(format!(
                        "refused the request: {}",
                        frame[3].as_str().unwrap_or("no reason given")
                    )));
                }
                Some("EVENT") => {
                    let Ok(answer) = serde_json::from_value::<Event>(frame[2].clone()) else {
                        continue;
                    };
                    match decision::bind_answer(&answer, &pending, &self.secret) {
                        Ok(Answer::Status(status)) => {
                            if let Some(refusal) = status.refusal {
                                let _ = connection.close().await;
                                return Ok(from_refusal(&refusal));
                            }
                        }
                        Ok(Answer::Result(result)) => {
                            let _ = connection.close().await;
                            return Ok(match (result.outcome, result.response) {
                                (Outcome::Answered, Some(response)) => Reply {
                                    status: 200,
                                    headers: vec![
                                        ("content-type".into(), "application/json".into()),
                                        ("x-typesafe-request-id".into(), event.id.clone()),
                                    ],
                                    body: serde_json::to_vec(&response).unwrap_or_default(),
                                },
                                (outcome, _) => {
                                    let refusal = result.refusal.unwrap_or_else(|| {
                                        decision::Refusal::new(outcome.as_str())
                                    });
                                    from_refusal(&refusal)
                                }
                            });
                        }
                        // Not this job's answer, or not the worker's: keep
                        // waiting.
                        Err(_) => {}
                    }
                }
                _ => {}
            }
        }
    }

    fn seal(&self) -> Seal<'_> {
        Seal {
            signer: &self.signer,
            conversation: nip44::conversation_key(&self.secret, &self.worker_key),
            nonce: secp256k1::rand::random(),
            created_at: unix_now(),
        }
    }
}

impl Exchange for RelayExchange {
    fn exchange(&self, call: Call) -> Pending<'_> {
        Box::pin(self.carry(call))
    }

    fn service(&self) -> String {
        format!("hosted decision service {} on {}", self.worker, self.relay)
    }
}

/// The HTTP-shaped reply a worker refusal becomes: the status NIP-DEC maps
/// the refusal code to (`nostr::decision::http_status`, the statuses
/// OpenRouter's Decisions API uses), and a message that starts with
/// "Jev refused".
fn from_refusal(refusal: &decision::Refusal) -> Reply {
    let code = refusal.code.as_str();
    let status = decision::http_status(code);
    let what = match code {
        "quota_exhausted" | "rate_limited" => "quota".to_string(),
        other => other.to_string(),
    };
    let message = match &refusal.message {
        Some(message) => format!("Jev refused: {what} ({message})"),
        None => format!("Jev refused: {what}"),
    };
    refusal_reply(status, code, &message, refusal.retry_after_ms)
}

fn refusal_reply(status: u16, code: &str, message: &str, retry_after_ms: Option<u64>) -> Reply {
    let mut headers = vec![("content-type".to_string(), "application/json".to_string())];
    if let Some(ms) = retry_after_ms {
        headers.push(("retry-after".to_string(), ms.div_ceil(1000).to_string()));
    }
    Reply {
        status,
        headers,
        body: serde_json::to_vec(&json!({"error": {"code": code, "message": message}}))
            .unwrap_or_default(),
    }
}

fn parse_secret(hex: &str) -> Option<SecretKey> {
    if hex.len() != 64 {
        return None;
    }
    let bytes: Vec<u8> = (0..64)
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect::<Option<_>>()?;
    SecretKey::from_byte_array(bytes.try_into().ok()?).ok()
}

fn random_id() -> String {
    secp256k1::rand::random::<[u8; 16]>()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn present(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn no_env(_: &str) -> Option<String> {
        None
    }

    const DOOR_PIN: Door<'static> = Door {
        url: DOOR,
        model: "jev-1.13.0",
    };

    #[test]
    fn a_local_key_is_used_unchanged_and_named_by_where_it_came_from() {
        let dir = tempfile::tempdir().unwrap();
        let env = |name: &str| (name == TYPESAFE_KEY_VAR).then(|| " ts-local ".to_string());
        let resolved = resolve(&env, dir.path(), &DOOR_PIN, &|config| config).unwrap();
        assert_eq!(
            resolved.via,
            Via::Direct {
                source: "$TYPESAFE_API_KEY".into()
            }
        );
        assert_eq!(resolved.client.service(), None);
        assert_eq!(resolved.client.base_url(), DOOR);
        assert_eq!(resolved.client.default_model(), "jev-1.13.0");
        assert!(!dir.path().join(KEY_FILE).exists());

        std::fs::write(dir.path().join(JEV_FILE), r#"{"api_key":"ts-file"}"#).unwrap();
        let resolved = resolve(&no_env, dir.path(), &DOOR_PIN, &|config| config).unwrap();
        assert!(matches!(resolved.via, Via::Direct { .. }));
    }

    #[test]
    fn no_key_means_the_hosted_service_under_a_key_made_once() {
        let dir = tempfile::tempdir().unwrap();
        let first = resolve(&no_env, dir.path(), &DOOR_PIN, &|config| config).unwrap();
        let Via::Hosted { worker, relay, key } = &first.via else {
            panic!("expected the hosted service, got {:?}", first.via);
        };
        assert_eq!((worker.as_str(), relay.as_str()), (WORKER, RELAY));
        assert!(first.client.service().unwrap().contains(WORKER));
        assert_eq!(first.client.base_url(), DOOR);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(key).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o600);
        }
        let made = std::fs::read_to_string(key).unwrap();
        let again = resolve(&no_env, dir.path(), &DOOR_PIN, &|config| config).unwrap();
        assert!(matches!(again.via, Via::Hosted { .. }));
        assert_eq!(std::fs::read_to_string(key).unwrap(), made);
        assert!(!format!("{:?}", first.client).contains(made.trim()));
    }

    #[test]
    fn the_hosted_service_is_off_on_request_and_for_other_doors() {
        let dir = tempfile::tempdir().unwrap();
        let off = |name: &str| (name == HOSTED_VAR).then(|| "off".to_string());
        let error = resolve(&off, dir.path(), &DOOR_PIN, &|config| config).unwrap_err();
        assert!(error.contains(HOSTED_VAR), "{error}");
        let other = Door {
            url: "https://decision.example",
            model: "jev-1.13.0",
        };
        let error = resolve(&no_env, dir.path(), &other, &|config| config).unwrap_err();
        assert!(error.contains("decision.example"), "{error}");
    }

    #[test]
    fn a_refusal_reads_as_jev_refused_with_its_http_status() {
        let quota = from_refusal(
            &decision::Refusal::new("quota_exhausted")
                .message("This key used today's decision jobs on this worker.")
                .retry_after_ms(1_500),
        );
        assert_eq!(quota.status, 429);
        assert!(
            quota
                .headers
                .contains(&("retry-after".to_string(), "2".to_string()))
        );
        let body: Value = serde_json::from_slice(&quota.body).unwrap();
        assert_eq!(body["error"]["code"], "quota_exhausted");
        assert!(
            body["error"]["message"]
                .as_str()
                .unwrap()
                .starts_with("Jev refused: quota")
        );
        assert_eq!(from_refusal(&decision::Refusal::new("busy")).status, 503);
    }
}
