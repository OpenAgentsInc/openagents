//! The OpenAgents cloud: Coder's no-setup fallback provider.
//!
//! A host with no Codex login, no Claude Code sign-in, or none with
//! capacity still answers: Microcoder's loop generates each step through
//! the OpenAgents cloud, the same serving path the OpenAgents app's basic
//! chat uses. Each step is one NIP-CJ conversation job (kind `25900`),
//! signed by the host's own Nostr key (`~/.openagents/nostr-secret`, the
//! identity [`Identity::load`] keeps) and encrypted to the OpenAgents cloud
//! worker, through `relay.openagents.com`. The worker is `coder-worker`
//! open under a per-key quota (`coder::relay::quota`); it answers on the
//! AI gateway's Gemini Flash lane, which the gateway serves from Vertex. The
//! model credential, and every quota and abuse limit, live on the worker:
//! nothing on the host is configured, and no provider token or Google
//! credential is read here or anywhere on the host. INVARIANTS.md records
//! that line.
//!
//! In the capacity book the cloud is [`Provider::Vertex`]. A worker's
//! typed refusal (`rate_limited`, `quota_exhausted`, `busy`) becomes a
//! capacity refusal with the worker's `retry_after_ms`
//! ([`Refusal::cloud`]), so failover passes the cloud over until then and
//! the no-capacity sentence names when it returns. Any other refusal, such
//! as `limit_exceeded` for a step too large to send, is the step's error,
//! reported in plain words.
//!
//! Read `docs/coder/runtime/cloud-fallback.md`.

use std::sync::Arc;
use std::time::Instant;

use microcoder_loop::capacity::{CLOUD_ENDPOINT, Provider, Refusal};
use microcoder_loop::failover::{Lane, unix_now};
use microcoder_loop::models::{Basis, Generated, next_action_schema, parse_action};

use crate::generate::{Door, Generate, GenerateError, Message, Meta, Role};
use crate::relay::{Identity, RelayDoor, parse_pubkey};

/// The relay the cloud's jobs travel through.
pub const RELAY: &str = CLOUD_ENDPOINT;

/// The OpenAgents cloud worker's public key: the same worker the
/// OpenAgents app's basic chat talks to
/// (`crates/openagents-mobile/src/basic_coder.rs`). It is public; the
/// worker's secret and the model credential it spends stay on the worker's
/// host.
pub const WORKER: &str = "32c078952ff8b1f1d6f431e30fb240b0d1f91e30f977844557e8267390e3599b";

/// The variable naming another cloud worker (npub or hex), for a staging
/// worker or a test's loopback one.
pub const WORKER_VAR: &str = "CODER_CLOUD_WORKER";

/// The variable naming another relay for the cloud's jobs.
pub const RELAY_VAR: &str = "CODER_CLOUD_RELAY";

/// What the step's generation is called when the worker names no model.
pub const MODEL: &str = "openagents-cloud";

/// The door to the OpenAgents cloud, as this host's own key.
///
/// # Errors
///
/// A sentence when the worker override is not a public key or the host's
/// Nostr identity cannot be loaded.
pub fn door(env: &impl Fn(&str) -> Option<String>) -> Result<RelayDoor, String> {
    let worker_text = env(WORKER_VAR).unwrap_or_else(|| WORKER.to_string());
    let worker = parse_pubkey(&worker_text)
        .ok_or_else(|| format!("{WORKER_VAR} must be an npub or 64 lowercase hex"))?;
    let relay = env(RELAY_VAR).unwrap_or_else(|| RELAY.to_string());
    Ok(RelayDoor::new(relay, worker, Identity::load()?))
}

/// The instructions a step sends: the loop's system text, then the reply
/// shape, since a conversation job carries no response format.
#[must_use]
pub fn instructions(system: &str) -> String {
    format!(
        "{system}\n\nReply with exactly one JSON object and nothing else: no prose and no \
         Markdown fence. It must match this JSON schema:\n{}",
        next_action_schema()
    )
}

/// Microcoder's lane through the OpenAgents cloud: one conversation job per
/// step, whose reply is read as the step's action, keeping the capacity
/// refusal the last one met.
pub struct CloudLane<G = Door> {
    door: Arc<G>,
    refusal: std::cell::RefCell<Option<Refusal>>,
    now: fn() -> u64,
}

impl<G> CloudLane<G> {
    /// A lane over `door`: the cloud's relay door, or a test's stand-in.
    #[must_use]
    pub fn new(door: Arc<G>) -> Self {
        CloudLane {
            door,
            refusal: std::cell::RefCell::new(None),
            now: unix_now,
        }
    }

    /// The same lane on another clock, for a test.
    #[must_use]
    pub fn clocked(mut self, now: fn() -> u64) -> Self {
        self.now = now;
        self
    }
}

impl<G: Generate> microcoder_loop::models::Generate for CloudLane<G> {
    async fn generate(&self, system: &str, prompt: &str) -> Generated {
        let started = Instant::now();
        let instructions = instructions(system);
        let input = [Message {
            role: Role::User,
            text: prompt.to_string(),
        }];
        let mut model: Option<String> = None;
        let mut retry_after_ms: Option<u64> = None;
        let answered = self
            .door
            .generate(&instructions, &input, &mut |_| {}, &mut |meta| match meta {
                Meta::Model(name) => model = Some(name),
                Meta::RetryAfter(ms) => retry_after_ms = Some(ms),
                Meta::Judgment(_) => {}
                Meta::Upstream(_) | Meta::Switched(_) => {}
            })
            .await;
        let (action, usage) = match answered {
            Ok((text, usage)) => (parse_action(&text), usage),
            Err(error) => {
                if let GenerateError::Refused { code, .. } = &error {
                    *self.refusal.borrow_mut() = Refusal::cloud(code, retry_after_ms, (self.now)());
                }
                (Err(failure(&error)), None)
            }
        };
        let usage = usage.unwrap_or_default();
        // The cloud bills this host nothing: the worker pays for the model.
        Generated {
            action,
            model: model.unwrap_or_else(|| MODEL.to_string()),
            prompt_tokens: usage.input_tokens,
            completion_tokens: usage.output_tokens,
            usd: Some(0.0),
            known_usd: 0.0,
            cost_unknown: None,
            usd_upper: Some(0.0),
            cost_basis: Basis::Billed,
            milliseconds: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
        }
    }
}

impl<G: Generate> Lane for CloudLane<G> {
    fn refusal(&self) -> Option<Refusal> {
        self.refusal.borrow_mut().take()
    }
}

/// A cloud failure in plain words, from its type and code.
fn failure(error: &GenerateError) -> String {
    match error {
        GenerateError::Refused { code, message } => match code.as_str() {
            "limit_exceeded" => format!(
                "the OpenAgents cloud refused this step as too large (limit_exceeded): {message}"
            ),
            _ => format!("the OpenAgents cloud refused ({code}): {message}"),
        },
        GenerateError::Relay(why) => format!("the OpenAgents cloud's relay: {why}"),
        GenerateError::Silent { reason, .. } => {
            format!("the OpenAgents cloud did not answer: {reason}")
        }
        other => format!("the OpenAgents cloud: {other}"),
    }
}

/// Where the cloud stands for `coder doctor`, in one clause.
#[must_use]
pub fn describe() -> String {
    format!(
        "the OpenAgents cloud (worker {}… on {}), signed by this host's key; no token",
        &WORKER[..12],
        Provider::Vertex.endpoint()
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generate::Usage;
    use microcoder_loop::capacity::Kind;
    use microcoder_loop::models::Generate as _;

    /// A stand-in cloud: answers with `text`, or refuses with `code` and a
    /// wait, and keeps the instructions it was sent.
    struct Fake {
        answer: Result<String, (String, Option<u64>)>,
        seen: std::sync::Mutex<Vec<String>>,
    }

    impl Generate for Fake {
        async fn generate<'a>(
            &'a self,
            instructions: &'a str,
            _input: &'a [Message],
            _sink: &'a mut (dyn FnMut(&str) + Send),
            meta: &'a mut (dyn FnMut(Meta) + Send),
        ) -> Result<(String, Option<Usage>), GenerateError> {
            self.seen.lock().unwrap().push(instructions.to_string());
            match &self.answer {
                Ok(text) => {
                    meta(Meta::Model("the-cloud-model".into()));
                    Ok((
                        text.clone(),
                        Some(Usage {
                            input_tokens: 120,
                            output_tokens: 30,
                        }),
                    ))
                }
                Err((code, wait)) => {
                    if let Some(ms) = wait {
                        meta(Meta::RetryAfter(*ms));
                    }
                    Err(GenerateError::Refused {
                        code: code.clone(),
                        message: "no".into(),
                    })
                }
            }
        }
    }

    fn lane(answer: Result<String, (String, Option<u64>)>) -> CloudLane<Fake> {
        CloudLane::new(Arc::new(Fake {
            answer,
            seen: std::sync::Mutex::default(),
        }))
        .clocked(|| 1_000)
    }

    const ACTION: &str = r#"{"rationale":"say hi","commands":[],"view":[],"freeze_tests":false,"expand":[],"finished":true,"reply":"Hello.","ask":"none"}"#;

    #[tokio::test]
    async fn a_step_through_the_cloud_reads_the_reply_as_the_action() {
        let lane = lane(Ok(format!("```json\n{ACTION}\n```")));
        let generated = lane.generate("You are Microcoder.", "hello").await;
        let action = generated.action.unwrap();
        assert!(action.finished);
        assert_eq!(action.reply, "Hello.");
        assert_eq!(generated.model, "the-cloud-model");
        assert_eq!(generated.prompt_tokens, 120);
        // The host pays nothing.
        assert_eq!(generated.usd, Some(0.0));
        assert!(lane.refusal().is_none());
        // The step asked for the action's shape.
        let seen = lane.door.seen.lock().unwrap();
        assert!(seen[0].starts_with("You are Microcoder."));
        assert!(seen[0].contains("\"freeze_tests\""));
    }

    #[tokio::test]
    async fn a_quota_refusal_is_a_typed_capacity_refusal_with_its_wait() {
        let lane = lane(Err(("quota_exhausted".into(), Some(7_200_000))));
        let generated = lane.generate("s", "p").await;
        assert!(
            generated
                .action
                .unwrap_err()
                .contains("the OpenAgents cloud refused (quota_exhausted)")
        );
        let refusal = lane.refusal().unwrap();
        assert_eq!(refusal.provider, Provider::Vertex);
        assert_eq!(refusal.kind, Kind::UsageLimit);
        assert_eq!(refusal.until, 1_000 + 7_200);
        // Taken once.
        assert!(lane.refusal().is_none());
    }

    #[tokio::test]
    async fn a_rate_limit_holds_a_minute_and_a_too_large_step_is_only_an_error() {
        let limited = lane(Err(("rate_limited".into(), None)));
        limited.generate("s", "p").await;
        assert_eq!(limited.refusal().unwrap().kind, Kind::RateLimit);
        let large = lane(Err(("limit_exceeded".into(), None)));
        let generated = large.generate("s", "p").await;
        assert!(generated.action.unwrap_err().contains("too large"));
        assert!(large.refusal().is_none());
    }

    #[test]
    fn the_default_worker_is_a_public_key_and_the_door_needs_no_token() {
        assert!(parse_pubkey(WORKER).is_some());
        assert_eq!(RELAY, "wss://relay.openagents.com");
        // A bad override is refused in words, before any key is loaded.
        let bad = |name: &str| (name == WORKER_VAR).then(|| "nope".to_string());
        match door(&bad) {
            Err(why) => assert!(why.contains(WORKER_VAR), "{why}"),
            Ok(_) => panic!("a worker that is not a key was accepted"),
        }
    }
}
