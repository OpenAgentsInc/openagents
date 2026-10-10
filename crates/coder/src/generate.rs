//! The Generate side of the agent: open-ended synthesis, behind a trait.
//!
//! [`Generate`] is the whole public contract: a request carries system
//! instructions and the conversation so far, a response carries the produced
//! text and the token count. [`ResponsesDoor`] implements it against any
//! endpoint that speaks the Open Responses API — `POST {base}/v1/responses`
//! with a bearer, a stream of `response.output_text.delta` events back.
//! The door's URL, model, and key come from the environment; this crate
//! ships no endpoint of its own beyond the public gateway default.
//!
//! [`StubGenerate`] answers with a canned line so the shell and tests run
//! with no door at all.
//!
//! Every wait a door may keep is bounded, and the bounds are three rather
//! than one: [`CONNECT_TIMEOUT`] for the connection, [`Patience::first_word`]
//! for the response headers, and [`Patience::quiet`] for the silence
//! between two events of a stream. Read [`Patience`] for why they are
//! separate.
//!
//! This module is the only place in the crate a model name belongs. The
//! names are on [`Lane`], and a caller that needs one names a lane or
//! reads a variable rather than writing an identifier of its own.

use std::env;
use std::fmt;
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use serde_json::{Value, json};

/// The default door: the public Vercel AI Gateway. `CODER_DOOR_URL`
/// overrides it for a deployment's own endpoint.
pub const DEFAULT_DOOR_URL: &str = "https://ai-gateway.vercel.sh";

/// The OpenRouter door: its Open Responses route is
/// `https://openrouter.ai/api/v1/responses`. The chat worker's primary
/// model, [`Lane::SpaceBunny`], is served only here.
pub const OPENROUTER_DOOR_URL: &str = "https://openrouter.ai/api";

/// The variable holding the OpenRouter key the chat worker's primary door
/// is reached with.
pub const OPENROUTER_KEY_VAR: &str = "OPENROUTER_API_KEY";

/// The variable naming the OpenAgents inference gateway's base URL
/// (`docs/inference/gateway.md`): the door every chat worker model call goes
/// through when [`INFERENCE_KEY_VAR`] holds a service key. Unset, the local
/// gateway ([`DEFAULT_INFERENCE_URL`]).
pub const INFERENCE_URL_VAR: &str = "CODER_INFERENCE_URL";

/// The variable holding the `oak_` service key the inference gateway
/// answers. Set, the chat worker sends its model calls through the gateway
/// unless [`INFERENCE_MODE_VAR`] says `direct`.
pub const INFERENCE_KEY_VAR: &str = "CODER_INFERENCE_KEY";

/// The variable naming the model or task class the chat worker asks the
/// gateway for. Unset, [`DEFAULT_INFERENCE_MODEL`].
pub const INFERENCE_MODEL_VAR: &str = "CODER_INFERENCE_MODEL";

/// The switch: `gateway` sends the chat worker's model calls through the
/// inference gateway, `direct` keeps the provider doors it called before
/// (OpenRouter's primary, then the Vercel AI Gateway). Unset, `gateway`
/// whenever [`INFERENCE_KEY_VAR`] is set, `direct` otherwise.
pub const INFERENCE_MODE_VAR: &str = "CODER_WORKER_INFERENCE";

/// The local gateway's base URL (`openagents-gateway serve` with the
/// inference routes on loopback).
pub const DEFAULT_INFERENCE_URL: &str = "http://127.0.0.1:8790";

/// What the chat worker asks the gateway for: the general conversation
/// task class, so the gateway's router picks the model and upstream.
pub const DEFAULT_INFERENCE_MODEL: &str = "openagents/chat";

/// The gateway door from the environment, when the switch is on: the
/// gateway URL, its service key, and the model or class asked for. `None`
/// when the worker keeps its direct doors.
///
/// # Errors
///
/// A sentence when the switch names neither mode, or names `gateway` with
/// no service key to reach it.
pub fn inference_door_from_env() -> Result<Option<ResponsesDoor>, String> {
    let key = env::var(INFERENCE_KEY_VAR)
        .ok()
        .filter(|key| !key.trim().is_empty());
    let mode = env::var(INFERENCE_MODE_VAR).ok();
    let on = match mode.as_deref().map(str::trim) {
        None | Some("") => key.is_some(),
        Some("gateway") => true,
        Some("direct") => false,
        Some(other) => {
            return Err(format!(
                "{INFERENCE_MODE_VAR} is `gateway` or `direct`, not `{other}`"
            ));
        }
    };
    if !on {
        return Ok(None);
    }
    let key = key.ok_or_else(|| {
        format!(
            "{INFERENCE_MODE_VAR}=gateway needs the gateway's service key in {INFERENCE_KEY_VAR}; \
             set {INFERENCE_MODE_VAR}=direct to keep the provider doors"
        )
    })?;
    let url = env::var(INFERENCE_URL_VAR)
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_INFERENCE_URL.to_string());
    let model = env::var(INFERENCE_MODEL_VAR)
        .ok()
        .filter(|model| !model.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_INFERENCE_MODEL.to_string());
    Ok(Some(ResponsesDoor::new(url, model, key).through_gateway()))
}

/// The variable choosing what every door asks of its model provider about
/// keeping and training on the conversation: `strict` (unset), `no-training`,
/// or `off`. See [`ProviderPrivacy`] (#11040).
pub const PROVIDER_PRIVACY_VAR: &str = "CODER_PROVIDER_PRIVACY";

/// What a door asks the provider behind it about the conversation it sends.
///
/// Every door sends `"store": false` regardless. On top of that:
///
/// | Level | OpenRouter (`provider`) | Vercel AI Gateway (`providerOptions.gateway`) |
/// | --- | --- | --- |
/// | `Strict` | `data_collection: "deny"`, `zdr: true` | `zeroDataRetention: true` |
/// | `NoTraining` | `data_collection: "deny"` | nothing |
/// | `Off` | nothing | nothing |
///
/// Both routers refuse a request no provider can serve under the
/// preference rather than quietly sending it elsewhere, so under `Strict` a
/// model with no zero-retention endpoint fails, and the chat worker's
/// fallback answers the turn. Other door URLs get only `store: false`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ProviderPrivacy {
    /// No training and zero retention, where the door's router supports it.
    #[default]
    Strict,
    /// No training only (OpenRouter's data collection denied).
    NoTraining,
    /// Nothing beyond `store: false`.
    Off,
}

impl ProviderPrivacy {
    /// Reads `text` as a level: `strict`, `no-training`, or `off`.
    ///
    /// # Errors
    ///
    /// A sentence naming the accepted values.
    pub fn parse(text: &str) -> Result<Self, String> {
        match text.trim() {
            "" | "strict" => Ok(Self::Strict),
            "no-training" => Ok(Self::NoTraining),
            "off" => Ok(Self::Off),
            other => Err(format!(
                "{PROVIDER_PRIVACY_VAR} is `strict`, `no-training`, or `off`, not `{other}`"
            )),
        }
    }

    /// The level [`PROVIDER_PRIVACY_VAR`] names; unset or unreadable is
    /// [`ProviderPrivacy::Strict`], so a typo never loosens it.
    #[must_use]
    pub fn from_env() -> Self {
        std::env::var(PROVIDER_PRIVACY_VAR)
            .ok()
            .and_then(|text| Self::parse(&text).ok())
            .unwrap_or_default()
    }

    /// The level's name, as [`ProviderPrivacy::parse`] reads it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Strict => "strict",
            Self::NoTraining => "no-training",
            Self::Off => "off",
        }
    }

    /// Adds this level's fields to `body`, a request to the door at `url`,
    /// merging into any `provider` or `providerOptions` already there.
    fn apply(self, url: &str, body: &mut Value) {
        let Some(fields) = body.as_object_mut() else {
            return;
        };
        if url.contains("openrouter.ai") {
            let mut asks = serde_json::Map::new();
            if self != Self::Off {
                asks.insert("data_collection".to_string(), json!("deny"));
            }
            if self == Self::Strict {
                asks.insert("zdr".to_string(), json!(true));
            }
            merge(fields, "provider", asks);
        } else if url.contains("ai-gateway.vercel.sh") && self == Self::Strict {
            let mut gateway = serde_json::Map::new();
            gateway.insert("zeroDataRetention".to_string(), json!(true));
            let mut options = serde_json::Map::new();
            options.insert("gateway".to_string(), Value::Object(gateway));
            merge(fields, "providerOptions", options);
        }
    }
}

/// Merges `add` into the object at `fields[name]`, one level deep for
/// objects, so a lane's own provider options survive the privacy fields.
fn merge(
    fields: &mut serde_json::Map<String, Value>,
    name: &str,
    add: serde_json::Map<String, Value>,
) {
    if add.is_empty() {
        return;
    }
    let slot = fields
        .entry(name.to_string())
        .or_insert_with(|| Value::Object(serde_json::Map::new()));
    if !slot.is_object() {
        *slot = Value::Object(serde_json::Map::new());
    }
    if let Some(existing) = slot.as_object_mut() {
        for (key, value) in add {
            match (existing.get_mut(&key), value) {
                (Some(Value::Object(inner)), Value::Object(more)) => inner.extend(more),
                (_, value) => {
                    existing.insert(key, value);
                }
            }
        }
    }
}

/// A lane: a model the gateway serves, under a short name.
///
/// The gateway answers one Open Responses shape for every model in its
/// catalog, so a second model is a configuration change rather than a
/// second client. A lane is that configuration, and this enum is where
/// every model name in the crate lives — the worker, the runtime, and a
/// bench name a lane and ask for the model, so a name never spreads to a
/// fourth place.
///
/// A model name is also door identity. `docs/gym/regression.md` refuses a
/// comparison when door identity moves, so a run has to be able to say
/// which lane answered it; [`Door::model`] is what a trace records, and it
/// reports the model rather than the lane for exactly that reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Google's Gemini Flash, the lane a door runs when nothing names one.
    Gemini,
    /// Z.ai's GLM Flash.
    Glm,
    /// Space Bunny Alpha, an anonymous preview model served only through
    /// OpenRouter ([`OPENROUTER_DOOR_URL`], not the gateway), until
    /// 2026-10-05. Retired: OpenRouter answers it with HTTP 404 "No
    /// endpoints found", and the chat worker's primary is [`Lane::Gemini`]
    /// on OpenRouter since 2026-10-09. Kept so older records still name it.
    SpaceBunny,
}

impl Lane {
    /// Every lane, in the order the crate documents them.
    pub const ALL: [Lane; 3] = [Lane::Gemini, Lane::Glm, Lane::SpaceBunny];

    /// The lane's short name, which configuration may use in place of the
    /// model id.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Lane::Gemini => "gemini",
            Lane::Glm => "glm",
            Lane::SpaceBunny => "space-bunny",
        }
    }

    /// The gateway model the lane runs.
    #[must_use]
    pub const fn model(self) -> &'static str {
        match self {
            Lane::Gemini => "google/gemini-3.8-flash",
            Lane::Glm => "zai/glm-5.3-flash",
            Lane::SpaceBunny => "stealth/space-bunny-alpha",
        }
    }

    /// The model id OpenRouter serves the lane under, which differs from
    /// the gateway's for GLM (`z-ai/` there, `zai/` here).
    #[must_use]
    pub const fn openrouter_model(self) -> &'static str {
        match self {
            Lane::Gemini => "google/gemini-3.8-flash",
            Lane::Glm => "z-ai/glm-5.3-flash",
            Lane::SpaceBunny => "stealth/space-bunny-alpha",
        }
    }

    /// The door that serves the lane: the gateway for every lane but
    /// [`Lane::SpaceBunny`], which only OpenRouter serves.
    #[must_use]
    pub const fn door_url(self) -> &'static str {
        match self {
            Lane::Gemini | Lane::Glm => DEFAULT_DOOR_URL,
            Lane::SpaceBunny => OPENROUTER_DOOR_URL,
        }
    }

    /// The lane `asked` names, by short name or by model id. `None` when
    /// it names neither.
    #[must_use]
    pub fn read(asked: &str) -> Option<Lane> {
        let asked = asked.trim();
        Lane::ALL
            .into_iter()
            .find(|lane| lane.name() == asked || lane.model() == asked)
    }
}

/// The lane a door runs when nothing names one.
pub const DEFAULT_LANE: Lane = Lane::Gemini;

/// The model the default lane runs. `CODER_MODEL` overrides it, by lane
/// name or by gateway model id.
pub const DEFAULT_MODEL: &str = DEFAULT_LANE.model();

/// The variable that names the model or lane the agent's door runs.
pub const MODEL_VAR: &str = "CODER_MODEL";

/// The variable that names the model or lane `coder-worker` answers
/// through.
///
/// The worker reads this rather than [`MODEL_VAR`] because the model a
/// service pays for is not automatically the model someone would pick at
/// their own terminal, and one constant cannot be both.
pub const WORKER_MODEL_VAR: &str = "CODER_WORKER_MODEL";

/// The variable that names the chat worker's primary model: the model it
/// asks OpenRouter for first, with the door [`WORKER_MODEL_VAR`] names as
/// its fallback ([`FallbackDoor`]).
///
/// Unset, the primary is [`DEFAULT_PRIMARY`] whenever
/// [`OPENROUTER_KEY_VAR`] is set, so the worker needs no new variable; a
/// lane name or an OpenRouter model id names another, and `off` answers on
/// the fallback door alone. Read it with [`worker_primary`].
pub const WORKER_PRIMARY_VAR: &str = "CODER_WORKER_PRIMARY";

/// The chat worker's primary when [`WORKER_PRIMARY_VAR`] names none:
/// Gemini Flash on OpenRouter. It replaced Space Bunny Alpha, which
/// OpenRouter retired (#10109); a primary that goes away later costs one
/// 404 and is then benched ([`BENCH_GONE`]).
pub const DEFAULT_PRIMARY: Lane = Lane::Gemini;

/// The reasoning effort the primary door asks for. Measured 2026-10-01
/// through OpenRouter's Open Responses route, Space Bunny Alpha's first
/// token came at 0.96 s at `low`, against 1.8 to 2.9 s at its default
/// (#10109).
pub const PRIMARY_EFFORT: &str = "low";

/// How long the primary door has to show it is working — the first event
/// of its stream, such as the reasoning it streams before its answer — or
/// to send the first words of its answer, before the turn goes to the
/// fallback door instead.
///
/// The primary's first token comes in about a second on a short question;
/// the fallback's in about five and a half. Four seconds lets a slow
/// primary start and still leaves a turn that falls back short of ten.
pub const PRIMARY_FIRST_WORD: Duration = Duration::from_secs(4);

/// How long a primary that is working (streaming its reasoning) has for
/// the first words of its answer, from the request.
///
/// Measured 2026-10-01, Space Bunny Alpha's reasoning began within about
/// a second, and a question that takes thought ("What is the capital of
/// Australia and why was it chosen?") had its first words at 2.3 to 3.2 s
/// under a one-line instruction and past four seconds under the chat
/// worker's, where the first release fell back and answered in eight to
/// nine. Past [`PRIMARY_FIRST_WORD`], a primary that is thinking is still
/// sooner than a fallback that starts from nothing.
pub const PRIMARY_THINKING: Duration = Duration::from_secs(8);

/// The model and key the chat worker's primary door runs, from what
/// [`WORKER_PRIMARY_VAR`] asks (`asked`) and the OpenRouter key (`key`).
/// `None` answers on the fallback door alone.
///
/// # Errors
///
/// A sentence when a primary is named and no OpenRouter key is set: a
/// named primary that quietly ran nothing would be a measurement of the
/// fallback under the primary's name.
pub fn worker_primary(
    asked: Option<&str>,
    key: Option<&str>,
) -> Result<Option<(String, String)>, String> {
    let key = key.map(str::trim).filter(|key| !key.is_empty());
    let asked = asked.map(str::trim).filter(|asked| !asked.is_empty());
    match (asked, key) {
        (Some("off"), _) | (None, None) => Ok(None),
        (None, Some(key)) => Ok(Some((
            DEFAULT_PRIMARY.openrouter_model().to_string(),
            key.to_string(),
        ))),
        (Some(asked), Some(key)) => Ok(Some((model_named(asked).to_string(), key.to_string()))),
        (Some(asked), None) => Err(format!(
            "{WORKER_PRIMARY_VAR} names {asked} and {OPENROUTER_KEY_VAR} is not set; \
             set the key, or set {WORKER_PRIMARY_VAR}=off to answer on the fallback alone"
        )),
    }
}

/// The variable that lists the chat worker's backup doors: the doors a
/// turn goes to, in order, after the primary and the door
/// [`WORKER_MODEL_VAR`] names have both failed before their first words
/// ([`FallbackDoor`]).
///
/// A comma-separated list of `door:model` entries, where the door is
/// `openrouter` ([`OPENROUTER_KEY_VAR`]) or `vercel` (the Vercel AI
/// Gateway, [`VERCEL_KEY_VARS`]) and the model a lane name or a model id;
/// `off` for none. Unset, [`DEFAULT_BACKUPS`], each kept when its door's
/// key is here. Read it with [`worker_backups`].
pub const WORKER_BACKUPS_VAR: &str = "CODER_WORKER_BACKUPS";

/// The variables that hold a Vercel AI Gateway key for a backup door, in
/// the order they are read.
pub const VERCEL_KEY_VARS: [&str; 2] = ["AI_GATEWAY_API_KEY", "CODER_AI_GATEWAY_KEY"];

/// The backup doors when [`WORKER_BACKUPS_VAR`] is unset: GLM on
/// OpenRouter (another model family on the same account), then Gemini and
/// GLM on the Vercel AI Gateway (another account).
pub const DEFAULT_BACKUPS: &str = "openrouter:gemini,openrouter:glm,vercel:gemini,vercel:glm";

/// The backup doors `asked` lists ([`WORKER_BACKUPS_VAR`]), reached with
/// `openrouter` and `vercel` keys.
///
/// Unset, [`DEFAULT_BACKUPS`] with each entry whose key is missing left
/// out, so the worker needs no new variable.
///
/// # Errors
///
/// A sentence when an entry names a door this does not know, names no
/// model, or is listed explicitly with no key to reach it: a listed
/// backup that quietly ran nothing would be a chain shorter than the one
/// configured.
pub fn worker_backups(
    asked: Option<&str>,
    openrouter: Option<&str>,
    vercel: Option<&str>,
) -> Result<Vec<ResponsesDoor>, String> {
    let openrouter = openrouter.map(str::trim).filter(|key| !key.is_empty());
    let vercel = vercel.map(str::trim).filter(|key| !key.is_empty());
    let asked = asked.map(str::trim).filter(|asked| !asked.is_empty());
    let (list, strict) = match asked {
        Some("off") => return Ok(Vec::new()),
        Some(list) => (list, true),
        None => (DEFAULT_BACKUPS, false),
    };
    let mut doors = Vec::new();
    for entry in list.split(',').map(str::trim).filter(|e| !e.is_empty()) {
        let Some((door, model)) = entry
            .split_once(':')
            .map(|(door, model)| (door.trim(), model.trim()))
            .filter(|(_, model)| !model.is_empty())
        else {
            return Err(format!(
                "{WORKER_BACKUPS_VAR} entry `{entry}` is not `door:model`"
            ));
        };
        let (url, key, var) = match door {
            "openrouter" => (OPENROUTER_DOOR_URL, openrouter, OPENROUTER_KEY_VAR),
            "vercel" => (DEFAULT_DOOR_URL, vercel, VERCEL_KEY_VARS[0]),
            other => {
                return Err(format!(
                    "{WORKER_BACKUPS_VAR} names the door `{other}`; the doors are \
                     `openrouter` and `vercel`"
                ));
            }
        };
        match key {
            Some(key) => doors.push(ResponsesDoor::new(url, model_at(url, model), key)),
            None if strict => {
                return Err(format!(
                    "{WORKER_BACKUPS_VAR} lists {entry} and {var} is not set; set the key, \
                     or leave the entry out"
                ));
            }
            None => {}
        }
    }
    Ok(doors)
}

/// [`worker_backups`] from the environment.
///
/// # Errors
///
/// As [`worker_backups`].
pub fn worker_backups_from_env() -> Result<Vec<ResponsesDoor>, String> {
    let vercel = VERCEL_KEY_VARS
        .iter()
        .find_map(|name| env::var(name).ok().filter(|key| !key.trim().is_empty()));
    worker_backups(
        env::var(WORKER_BACKUPS_VAR).ok().as_deref(),
        env::var(OPENROUTER_KEY_VAR).ok().as_deref(),
        vercel.as_deref(),
    )
}

/// The model `asked` names: the lane's model when it names a lane, and
/// `asked` itself otherwise.
///
/// A value that names no lane is taken as a gateway model id, so the
/// gateway's whole catalog stays reachable without a lane of its own.
#[must_use]
pub fn model_named(asked: &str) -> &str {
    match Lane::read(asked) {
        Some(lane) => lane.model(),
        None => asked.trim(),
    }
}

/// The model `asked` names at the door at `url`: a lane's OpenRouter id
/// when the door is OpenRouter, and [`model_named`] otherwise.
#[must_use]
pub fn model_at<'a>(url: &str, asked: &'a str) -> &'a str {
    match Lane::read(asked) {
        Some(lane) if url.trim_end_matches('/') == OPENROUTER_DOOR_URL => lane.openrouter_model(),
        _ => model_named(asked),
    }
}

/// The model `variable` asks for, with a lane name resolved to the model
/// it runs. `None` when the variable is unset or holds only blanks.
#[must_use]
pub fn model_from_env(variable: &str) -> Option<String> {
    let asked = env::var(variable).ok()?;
    let model = model_named(&asked);
    (!model.is_empty()).then(|| model.to_string())
}

/// How long a door has to accept a connection.
///
/// Separate from [`Patience::first_word`] because a refused or
/// unroutable endpoint is a different failure from a door that took the
/// request and thought about it, and the second should not have to wait
/// out the first.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// How long each wait on a streaming door lasts, and how the waits
/// between attempts grow.
///
/// The bounds exist because a door that accepts a request and then sends
/// nothing held the turn open with no limit at all: nothing bounded the
/// wait for the first token, nothing bounded the silence between tokens,
/// and nothing asked again. An unattended fan-out over such a door has no
/// upper bound on anything.
///
/// Reimplemented from the `~/work/coder` service's `Patience`, whose
/// values are these and whose record of the failure is an operator
/// waiting three and five minutes between a tool result and the next
/// token with nothing written down about where the time went.
///
/// The three are separate because they fail differently:
///
/// - [`Patience::first_word`] ends in a request that is sent again.
/// - [`Patience::quiet`] ends the turn, because a caller has already seen
///   part of the answer and a second attempt would repeat it.
/// - [`Patience::retry_wait`] is neither; it is the pause between two
///   attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Patience {
    /// The wait for the door's response headers. A door that has not
    /// answered by then is treated like one that dropped the connection,
    /// and the request is sent again.
    pub first_word: Duration,
    /// The longest silence between two events of a stream.
    ///
    /// Measured between events rather than between bytes, because a
    /// stream's silence is not a byte-level property: a door that keeps a
    /// connection warm with blank lines is quiet, and a chunk that carries
    /// half an event is not an event. A door that goes quiet for this long
    /// has lost the turn, and the turn fails with a line saying how long
    /// it waited and how much had arrived.
    pub quiet: Duration,
    /// The longest one streaming attempt may run from its request to its
    /// completed event, however talkative the door is. A door that keeps
    /// sending events forever, or dribbles one every minute, would
    /// otherwise hold the turn for as long as it liked.
    pub whole: Duration,
    /// The wait before the second attempt; the third waits twice as long.
    pub retry_wait: Duration,
}

impl Default for Patience {
    fn default() -> Self {
        Patience {
            first_word: Duration::from_secs(30),
            quiet: Duration::from_secs(120),
            whole: Duration::from_secs(600),
            retry_wait: Duration::from_secs(1),
        }
    }
}

/// The most bytes one SSE line or one event's data may hold before the
/// stream is a failure rather than an answer. A completed event carries
/// the whole response, so the bound is wide; a door that never ends a line
/// would otherwise grow the buffer for as long as the wait allowed.
pub const MAX_EVENT_BYTES: usize = 4 * 1024 * 1024;

/// One conversational turn, user or assistant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    /// Who said it.
    pub role: Role,
    /// What they said.
    pub text: String,
}

/// The side of the conversation a [`Message`] sits on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// The person at the terminal.
    User,
    /// The agent.
    Assistant,
}

/// What a generation cost, when the door reports it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    /// Tokens the request consumed.
    pub input_tokens: u64,
    /// Tokens the response produced.
    pub output_tokens: u64,
}

/// Sideband information a door may emit mid-turn, before or between text
/// deltas. Only doors that wrap a remote worker produce it today.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Meta {
    /// A classification verdict the worker computed, as a display-ready
    /// line (the NIP-CJ `judgment` feedback payload's `line` field).
    Judgment(String),
    /// The model that produced the answer, as the far end names it (the
    /// NIP-CJ result payload's `model` field).
    ///
    /// A door that forwards a turn somewhere else does not know which
    /// model will take it, so the session header cannot say. The answer
    /// can, and this is how it says so: the door emits the name as the
    /// result lands and the trace puts it on that step.
    Model(String),
    /// The wait, in milliseconds, a worker's typed refusal names before a
    /// job would be admitted (the NIP-CJ error status's `retry_after_ms`).
    /// A door emits it just before it returns [`GenerateError::Refused`].
    RetryAfter(u64),
    /// The upstream account that answered, when the door is the inference
    /// gateway and its `openagents:route` event named one (`vertex`,
    /// `openrouter`, `vercel`, ...). Emitted with [`Meta::Model`].
    Upstream(String),
    /// Another door wrote this answer because an earlier one in the chain
    /// failed, refused, or went quiet before its first words, or sits
    /// benched for doing so ([`FallbackDoor`], #11132). Emitted with the
    /// answering door's [`Meta::Model`]; the worker puts it on the result
    /// as `switched`, and the chat says so beside the answer.
    Switched(Switched),
}

/// A turn an earlier door in a chain did not answer, as NIP-CJ carries it
/// on a result (`switched`, #11132).
pub use nostr::cj_conversation::switched::{Missed, Provider, Switched};

/// Why `error` made a door miss its turn: a refused key, account, or rate
/// (HTTP 401, 402, 403, or 429), no answer in time, or any other failure.
#[must_use]
pub fn missed(error: &GenerateError) -> Missed {
    match error {
        GenerateError::Status(401..=403 | 429, _) => Missed::Refused,
        GenerateError::Quiet { .. } | GenerateError::Silent { .. } => Missed::Timeout,
        _ => Missed::Error,
    }
}

/// Who runs `door`: our inference gateway, OpenRouter, the Vercel AI
/// Gateway, or another.
#[must_use]
pub fn provider(door: &ResponsesDoor) -> Provider {
    let url = door.url.trim_end_matches('/');
    if door.is_gateway() {
        Provider::OpenAgents
    } else if url == OPENROUTER_DOOR_URL {
        Provider::OpenRouter
    } else if url == DEFAULT_DOOR_URL {
        Provider::Vercel
    } else {
        Provider::Other
    }
}

/// `door` missed this turn because of `why`; who answered is filled in
/// when someone does.
#[must_use]
pub fn switched(door: &ResponsesDoor, why: Missed) -> Switched {
    Switched {
        provider: provider(door),
        model: door.model.clone(),
        why,
        answered: None,
    }
}

/// What generation can fail with.
///
/// The relay door splits its failures into three, because they are three
/// different states and a harness should not have to read prose to tell
/// them apart: the relay would not take the job, the relay took it and no
/// worker answered, or a worker answered by declining. `gym::eval::classify`
/// draws the same line — a typed refusal is an answer, a failure with no
/// code is the harness — and [`GenerateError::cause`] is where it is drawn
/// here.
#[derive(Debug)]
pub enum GenerateError {
    /// The door URL, key, or model is missing or wrong.
    Config(String),
    /// The HTTP call itself failed.
    Transport(reqwest::Error),
    /// The door answered with an error status and this body.
    Status(u16, String),
    /// The stream broke or carried an error event.
    Stream(String),
    /// The door took the request and went quiet.
    ///
    /// `heard` draws the same line the relay door draws with
    /// [`GenerateError::Silent`], because the two doors should fail in one
    /// vocabulary rather than two: `false` means the door never sent
    /// response headers, and `true` means it sent them and then stopped,
    /// whether or not any of the answer had arrived. The first is asked
    /// again and the second is not.
    Quiet {
        /// Whether anything came back before the wait ran out.
        heard: bool,
        /// What the wait was, in a sentence: how long it was, and how much
        /// had arrived. "It hung" and "it sent 400 characters and stopped"
        /// are different problems, and a turn that dies here should say
        /// which one it met.
        reason: String,
    },
    /// The relay would not take the job: the socket never opened, the
    /// NIP-42 challenge went unanswered, or the relay rejected the
    /// request event.
    Relay(String),
    /// The relay took the job and no answer came back.
    ///
    /// `heard` is what separates a worker that is not there from one that
    /// is slow, as far as an ephemeral protocol lets a client separate
    /// them: `false` means nothing at all came back from the worker's key,
    /// and `true` means something did and then the answer never finished.
    Silent {
        /// Whether anything came back from the worker before the wait ran
        /// out.
        heard: bool,
        /// What the wait was, in a sentence.
        reason: String,
    },
    /// A worker answered with a typed refusal: NIP-CJ `status: error`
    /// feedback carrying a machine-readable code. The job reached a
    /// worker, and the worker said no.
    Refused {
        /// The NIP-CJ refusal code, such as `quota_exhausted`.
        code: String,
        /// The worker's display text for it.
        message: String,
    },
    /// No model provider this host can reach has capacity: each one
    /// refused for a usage or rate limit, or has no usable login. The text
    /// is one sentence naming each and when it resets.
    NoCapacity(String),
}

impl GenerateError {
    /// The word a harness files this failure under.
    ///
    /// `worker_declined` is an answer in `gym::eval::classify`'s sense: the
    /// job reached a worker and the worker refused it with a code. Every
    /// other word is the harness.
    ///
    /// `worker_absent` and `worker_stalled` are both silence, and they are
    /// two words because they are two problems: nothing was listening, or
    /// something was listening and did not finish. The first is a
    /// judgment call — an ephemeral protocol gives a client no way to
    /// prove a worker is missing — and it is the judgment the short wait
    /// in [`crate::relay`] makes.
    ///
    /// `door_absent` and `door_stalled` are the streaming door's two words
    /// for the same pair of problems, and they are named for the door
    /// rather than for a worker because a direct door has no worker behind
    /// it. A harness reads either pair as a field.
    #[must_use]
    pub fn cause(&self) -> &'static str {
        match self {
            GenerateError::Config(_) => "config",
            GenerateError::Transport(_) | GenerateError::Status(..) => "door",
            GenerateError::Stream(_) => "stream",
            GenerateError::Quiet { heard: false, .. } => "door_absent",
            GenerateError::Quiet { heard: true, .. } => "door_stalled",
            GenerateError::Relay(_) => "relay_unreachable",
            GenerateError::Silent { heard: false, .. } => "worker_absent",
            GenerateError::Silent { heard: true, .. } => "worker_stalled",
            GenerateError::Refused { .. } => "worker_declined",
            GenerateError::NoCapacity(_) => "no_capacity",
        }
    }

    /// The typed refusal code, when the failure carries one.
    ///
    /// Read as a field rather than searched for in the message, so a door
    /// whose prose mentions a code is not recorded as having refused with
    /// it.
    #[must_use]
    pub fn refusal(&self) -> Option<&str> {
        match self {
            GenerateError::Refused { code, .. } => Some(code),
            _ => None,
        }
    }
}

impl fmt::Display for GenerateError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GenerateError::Config(why) => write!(f, "config: {why}"),
            GenerateError::Transport(error) => write!(f, "transport: {error}"),
            GenerateError::Status(status, body) => {
                write!(f, "the model endpoint answered HTTP {status}: {body}")
            }
            GenerateError::Stream(why) => write!(f, "stream: {why}"),
            GenerateError::Quiet {
                heard: false,
                reason,
            } => {
                write!(f, "the model endpoint did not answer: {reason}")
            }
            GenerateError::Quiet {
                heard: true,
                reason,
            } => {
                write!(
                    f,
                    "the model endpoint stopped sending partway through its answer: {reason}"
                )
            }
            GenerateError::Relay(why) => write!(f, "relay: {why}"),
            GenerateError::Silent {
                heard: false,
                reason,
            } => {
                write!(f, "no worker answered: {reason}")
            }
            GenerateError::Silent {
                heard: true,
                reason,
            } => {
                write!(f, "the worker stopped mid-answer: {reason}")
            }
            GenerateError::Refused { code, message } => {
                write!(f, "the worker declined ({code}): {message}")
            }
            GenerateError::NoCapacity(sentence) => f.write_str(sentence),
        }
    }
}

impl std::error::Error for GenerateError {}

impl From<reqwest::Error> for GenerateError {
    fn from(error: reqwest::Error) -> Self {
        GenerateError::Transport(error)
    }
}

/// How many times an empty stream is attempted before its error surfaces:
/// transient upstream failures — a malformed function call, a dropped
/// connection — retry invisibly; a dead door still reports dead.
const EMPTY_STREAM_ATTEMPTS: usize = 6;

/// How many times a request goes to a door that never sends response
/// headers.
///
/// A door that has not answered within [`Patience::first_word`] is
/// treated like one that dropped the connection, so the request is sent
/// again. Three attempts bounds the whole wait at roughly three times
/// `first_word` plus the two pauses between them, which is a number an
/// unattended run can be reasoned about with.
const HEADER_ATTEMPTS: u32 = 3;

/// One generation: instructions plus conversation in, text plus usage out.
/// `sink` receives each text delta as it streams, so a caller can draw the
/// answer as it forms. `meta` receives sideband items a door may emit —
/// doors that never emit simply do not call it.
pub trait Generate: Send + Sync {
    /// Generate the next assistant turn.
    fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> impl std::future::Future<Output = Result<(String, Option<Usage>), GenerateError>> + Send + 'a;
}

/// An HTTP client with the connect bound every door here shares.
///
/// The read bounds are not set on the client, because a client-wide read
/// timeout bounds the whole request and a streaming turn is meant to be
/// long. What has to be bounded is the silence inside it, and that is
/// measured per stream against [`Patience::quiet`].
fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// A `Generate` backed by an Open Responses endpoint.
#[derive(Clone)]
pub struct ResponsesDoor {
    http: reqwest::Client,
    /// The door's base URL; the route is `/v1/responses` under it.
    pub url: String,
    /// The model the door runs.
    pub model: String,
    key: String,
    patience: Patience,
    /// Top-level request fields added to every body, such as a reasoning
    /// setting for one lane. `None` for the chat door.
    options: Option<serde_json::Map<String, Value>>,
    /// What the door asks its provider about keeping and training on the
    /// conversation ([`ProviderPrivacy`]); [`PROVIDER_PRIVACY_VAR`] by default.
    privacy: ProviderPrivacy,
    /// Whether the door is the OpenAgents inference gateway: the privacy
    /// level goes in the request's `openagents` object, and each answer
    /// names the model and upstream the gateway chose.
    gateway: bool,
}

impl ResponsesDoor {
    /// A door for `url` serving `model` behind `key`.
    pub fn new(url: impl Into<String>, model: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            http: client(),
            url: url.into().trim_end_matches('/').to_string(),
            model: model.into(),
            key: key.into(),
            patience: Patience::default(),
            options: None,
            privacy: ProviderPrivacy::from_env(),
            gateway: false,
        }
    }

    /// The same door as the OpenAgents inference gateway
    /// (`docs/inference/gateway.md`): the request carries
    /// `openagents.privacy`, and each answer names the model and upstream
    /// the gateway's router chose ([`Meta::Model`], [`Meta::Upstream`]).
    #[must_use]
    pub fn through_gateway(mut self) -> Self {
        self.gateway = true;
        self
    }

    /// Whether the door is the OpenAgents inference gateway.
    #[must_use]
    pub fn is_gateway(&self) -> bool {
        self.gateway
    }

    /// The same door asking its provider for `privacy` instead of the
    /// level the environment set.
    #[must_use]
    pub fn with_privacy(mut self, privacy: ProviderPrivacy) -> Self {
        self.privacy = privacy;
        self
    }

    /// What the door asks its provider about the conversation.
    #[must_use]
    pub fn privacy(&self) -> ProviderPrivacy {
        self.privacy
    }

    /// The same door adding `options`' fields to every request body, over
    /// any field of the same name. The personalization lane uses it to turn
    /// a model's reasoning off (`crate::personalize`); the chat door sets
    /// none.
    #[must_use]
    pub fn with_options(mut self, options: serde_json::Map<String, Value>) -> Self {
        self.options = Some(options);
        self
    }

    /// A door from the environment: `CODER_DOOR_URL` or the public gateway,
    /// `CODER_MODEL` or the default lane's model, `CODER_DOOR_KEY` or
    /// `CODER_AI_GATEWAY_KEY` for the bearer. `None` when no key is set.
    ///
    /// `CODER_MODEL` takes a lane's short name as readily as a model id,
    /// so `glm` and `zai/glm-5.3-flash` ask for the same door.
    pub fn from_env() -> Option<Self> {
        let key = env::var("CODER_DOOR_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .or_else(|| {
                env::var("CODER_AI_GATEWAY_KEY")
                    .ok()
                    .filter(|k| !k.is_empty())
            })?;
        let url = env::var("CODER_DOOR_URL").unwrap_or_else(|_| DEFAULT_DOOR_URL.to_string());
        let model = model_from_env(MODEL_VAR).unwrap_or_else(|| DEFAULT_MODEL.to_string());
        Some(Self::new(url, model, key))
    }

    /// Open (or keep open) a pooled connection to the door, so the next
    /// turn does not pay the TCP and TLS handshake before its request.
    ///
    /// One unbilled `GET /v1/models`; the answer is read and dropped, and
    /// a failure is only a cold connection, so it is not reported.
    pub async fn warm(&self) {
        if let Ok(response) = self
            .http
            .get(format!("{}/v1/models", self.url))
            .bearer_auth(&self.key)
            .timeout(Duration::from_secs(10))
            .send()
            .await
        {
            let _ = response.bytes().await;
        }
    }

    /// The same door running `model`, which may be named as a lane.
    #[must_use]
    pub fn serving(mut self, model: &str) -> Self {
        self.model = model_at(&self.url, model).to_string();
        self
    }

    /// The same door with different waits, so a test can exercise a bound
    /// without spending the real one.
    #[must_use]
    pub fn waiting(mut self, patience: Patience) -> Self {
        self.patience = patience;
        self
    }

    fn body(&self, instructions: &str, input: &[Message]) -> Value {
        let items: Vec<Value> = input
            .iter()
            .map(|message| {
                let (role, kind) = match message.role {
                    Role::User => ("user", "input_text"),
                    Role::Assistant => ("assistant", "output_text"),
                };
                json!({
                    "type": "message",
                    "role": role,
                    "content": [{ "type": kind, "text": message.text }],
                })
            })
            .collect();
        let mut body = json!({
            "model": self.model,
            "instructions": instructions,
            "input": items,
            "stream": true,
            "store": false,
            // No tools exist on this door; Gemini will still try to emit a
            // function call when the instructions carry a JSON example, and
            // the call dies as MALFORMED_FUNCTION_CALL. Forbid it outright —
            // an empty tools array plus tool_choice none, since gateways
            // differ on which one they translate.
            "tools": [],
            "tool_choice": "none",
        });
        if let (Some(options), Some(fields)) = (&self.options, body.as_object_mut()) {
            for (name, value) in options {
                fields.insert(name.clone(), value.clone());
            }
        }
        if self.gateway {
            // The gateway carries the level to each upstream itself; the
            // provider fields are for doors that are the provider.
            let level = match self.privacy {
                ProviderPrivacy::Strict => "strict",
                ProviderPrivacy::NoTraining | ProviderPrivacy::Off => "standard",
            };
            if let Some(fields) = body.as_object_mut() {
                let mut asks = serde_json::Map::new();
                asks.insert("privacy".to_string(), json!(level));
                merge(fields, "openagents", asks);
            }
        } else {
            self.privacy.apply(&self.url, &mut body);
        }
        body
    }
}

/// The Server-Sent Events reader: bytes in, answer text and usage out.
///
/// One reader serves every lane, because the gateway sends one event shape
/// for every model in its catalog. That is what makes the recorded streams
/// under `crates/coder/fixtures/gateway/` worth pinning: a change in the
/// gateway's event shape reaches a test here rather than a broken turn.
///
/// The reader frames the stream the way the SSE specification does: bytes
/// split into lines at CR, LF, or CRLF, lines gathered into an event by
/// field name until a blank line, and `data:` lines joined with a newline.
/// Text is decoded per line, never per chunk, so a character split across
/// two chunks arrives whole. A completed stream is one that carried
/// `response.completed`; anything short of that is a failure, and the text
/// it streamed is kept for the failure to describe rather than answered
/// with.
#[derive(Default)]
struct Reader {
    /// Bytes that have not yet formed a whole line.
    buffer: Vec<u8>,
    /// The `data:` lines of the event under construction, joined.
    data: String,
    /// The answer as it has arrived.
    text: String,
    /// The token counts the completed event carried.
    usage: Option<Usage>,
    /// How many stream events have been read, which a timeout reports.
    events: u32,
    /// Whether `response.completed` has been read.
    completed: bool,
    /// The model and upstream the inference gateway's `openagents:route`
    /// event named, when the door is the gateway.
    route: Option<(String, String)>,
}

impl Reader {
    /// Reads one chunk, handing each text delta to `sink`.
    ///
    /// Answers with how many events the chunk completed, so a caller can
    /// measure a stream's silence between events rather than between
    /// bytes.
    ///
    /// # Errors
    ///
    /// Returns [`GenerateError::Stream`] when the stream carries a failure
    /// event, an event that is not JSON, a line that is not UTF-8, a line
    /// or event past [`MAX_EVENT_BYTES`], or events after the completed one.
    fn push(
        &mut self,
        chunk: &[u8],
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<u32, GenerateError> {
        self.buffer.extend_from_slice(chunk);
        let mut read = 0;
        loop {
            let Some(end) = self
                .buffer
                .iter()
                .position(|byte| *byte == b'\n' || *byte == b'\r')
            else {
                if self.buffer.len() > MAX_EVENT_BYTES {
                    return Err(GenerateError::Stream(format!(
                        "a stream line ran past {MAX_EVENT_BYTES} bytes without ending"
                    )));
                }
                return Ok(read);
            };
            // CRLF is one line ending; a CR that ends the chunk has to wait
            // for the next chunk to say whether an LF follows it.
            let ending = if self.buffer[end] == b'\r' {
                match self.buffer.get(end + 1) {
                    Some(b'\n') => 2,
                    Some(_) => 1,
                    None => return Ok(read),
                }
            } else {
                1
            };
            let line: Vec<u8> = self.buffer.drain(..end + ending).collect();
            let line = std::str::from_utf8(&line[..end])
                .map_err(|_| GenerateError::Stream("a stream line was not UTF-8".to_string()))?;
            if line.is_empty() {
                if self.dispatch(sink)? {
                    read += 1;
                }
                continue;
            }
            if line.starts_with(':') {
                continue;
            }
            let (field, value) = line.split_once(':').unwrap_or((line, ""));
            let value = value.strip_prefix(' ').unwrap_or(value);
            if field == "data" {
                if !self.data.is_empty() {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                if self.data.len() > MAX_EVENT_BYTES {
                    return Err(GenerateError::Stream(format!(
                        "a stream event ran past {MAX_EVENT_BYTES} bytes"
                    )));
                }
            }
            // `event`, `id`, and `retry` carry nothing this door reads: the
            // event's type is inside its JSON.
        }
    }

    /// Reads the event whose blank line just arrived. Answers whether one
    /// was there: a blank line after nothing is a keepalive.
    fn dispatch(&mut self, sink: &mut (dyn FnMut(&str) + Send)) -> Result<bool, GenerateError> {
        let data = std::mem::take(&mut self.data);
        if data.is_empty() || data == "[DONE]" {
            return Ok(false);
        }
        if self.completed {
            return Err(GenerateError::Stream(
                "the stream went on after response.completed".to_string(),
            ));
        }
        let event = serde_json::from_str::<Value>(&data)
            .map_err(|_| GenerateError::Stream("a stream event was not JSON".to_string()))?;
        self.events += 1;
        match event["type"].as_str().unwrap_or_default() {
            "response.output_text.delta" => {
                if let Some(delta) = event["delta"].as_str() {
                    self.text.push_str(delta);
                    sink(delta);
                }
            }
            "openagents:route" => {
                if let (Some(model), Some(upstream)) =
                    (event["model"].as_str(), event["upstream"].as_str())
                {
                    self.route = Some((model.to_string(), upstream.to_string()));
                }
            }
            "response.completed" => {
                self.completed = true;
                self.usage = event["response"]["usage"].as_object().map(|u| Usage {
                    input_tokens: u["input_tokens"].as_u64().unwrap_or(0),
                    output_tokens: u["output_tokens"].as_u64().unwrap_or(0),
                });
            }
            "response.incomplete" => {
                let reason = event["response"]["incomplete_details"]["reason"]
                    .as_str()
                    .unwrap_or("no reason given");
                return Err(GenerateError::Stream(format!(
                    "the model endpoint ended its answer early: {reason}"
                )));
            }
            "response.failed" | "error" => {
                let message = event["response"]["error"]["message"]
                    .as_str()
                    .or_else(|| event["message"].as_str())
                    .unwrap_or("the turn failed upstream");
                return Err(GenerateError::Stream(message.to_string()));
            }
            // Everything else is the shape around the answer:
            // lifecycle events, content-part frames, and the reasoning
            // deltas a thinking model streams. None of them is the
            // answer, and a lane that sends them must not have them
            // spliced into one.
            _ => {}
        }
        Ok(true)
    }

    /// What the end of the stream means: an answer, or a stream that
    /// stopped before saying it was done. A last event the door did not
    /// follow with a blank line still counts, as it does for a reader that
    /// takes the end of the stream as the end of the event.
    fn finish(
        mut self,
        sink: &mut (dyn FnMut(&str) + Send),
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        let mut ended = Vec::new();
        if !self.buffer.is_empty() {
            ended.push(b'\n');
        }
        ended.push(b'\n');
        if let Err(error) = self.push(&ended, sink) {
            return Err((self.text, error));
        }
        if !self.completed {
            return Err((
                self.text,
                GenerateError::Stream(format!(
                    "the stream ended before response.completed, after {} events",
                    self.events
                )),
            ));
        }
        if self.text.is_empty() {
            return Err((
                self.text,
                GenerateError::Stream("the stream carried no text".to_string()),
            ));
        }
        Ok((self.text, self.usage))
    }
}

impl ResponsesDoor {
    /// One streaming attempt: the request plus the SSE read. On failure the
    /// error carries whatever text streamed before it died, so the caller
    /// can tell whether anything user-visible arrived.
    async fn once(
        &self,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
        routed: &mut Option<(String, String)>,
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        self.once_watched(instructions, input, sink, None, Some(routed))
            .await
    }

    /// [`ResponsesDoor::once`], setting `alive` when the stream carries
    /// its first event of any kind, such as the reasoning a thinking model
    /// streams before its answer: the door is working, not absent.
    async fn once_watched(
        &self,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
        alive: Option<&std::sync::atomic::AtomicBool>,
        routed: Option<&mut Option<(String, String)>>,
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        let ends = Instant::now() + self.patience.whole;
        let sent = self
            .http
            .post(format!("{}/v1/responses", self.url))
            .bearer_auth(&self.key)
            // The inference gateway sends `openagents:route` only on request.
            .header("x-openagents-events", "route,cost")
            .json(&self.body(instructions, input))
            .send();
        let response = match tokio::time::timeout(self.patience.first_word, sent).await {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => return Err((String::new(), GenerateError::Transport(error))),
            // No headers, so nothing was heard and the request may go
            // again. The attempt count is added where the attempts are
            // counted.
            Err(_) => {
                return Err((
                    String::new(),
                    GenerateError::Quiet {
                        heard: false,
                        reason: format!(
                            "no response headers in {} seconds",
                            self.patience.first_word.as_secs()
                        ),
                    },
                ));
            }
        };
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err((
                String::new(),
                GenerateError::Status(status.as_u16(), clip(&body, 400)),
            ));
        }

        let mut reader = Reader::default();
        let mut stream = response.bytes_stream();
        // The quiet clock runs from the last event rather than from the
        // last byte, so a door that dribbles bytes without completing an
        // event is still quiet.
        // The whole-attempt clock runs alongside it, so a door that never
        // stops talking ends too.
        let mut spoke = Instant::now();
        loop {
            let left = self
                .patience
                .quiet
                .saturating_sub(spoke.elapsed())
                .min(ends.saturating_duration_since(Instant::now()));
            let chunk = match tokio::time::timeout(left, stream.next()).await {
                Ok(Some(Ok(chunk))) => chunk,
                Ok(Some(Err(error))) => {
                    return Err((reader.text, GenerateError::Transport(error)));
                }
                Ok(None) => break,
                Err(_) if Instant::now() >= ends => {
                    return Err((reader.text.clone(), self.ran_long(&reader)));
                }
                Err(_) => return Err((reader.text.clone(), self.went_quiet(&reader))),
            };
            match reader.push(&chunk, sink) {
                Ok(0) => {}
                Ok(_) => {
                    spoke = Instant::now();
                    if let Some(alive) = alive {
                        alive.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
                Err(error) => return Err((reader.text, error)),
            }
        }
        if let Some(routed) = routed {
            routed.clone_from(&reader.route);
        }
        reader.finish(sink)
    }

    /// The failure a stream that outlived the whole-attempt bound becomes.
    fn ran_long(&self, reader: &Reader) -> GenerateError {
        GenerateError::Quiet {
            heard: true,
            reason: format!(
                "{} seconds without completing, after {} events and {} characters",
                self.patience.whole.as_secs(),
                reader.events,
                reader.text.chars().count()
            ),
        }
    }

    /// The failure a stream that stopped sending becomes.
    ///
    /// It names the wait and what had arrived, because a door that hung
    /// before saying anything and a door that answered part way and
    /// stopped are different problems, and a turn that dies here is often
    /// the only record of which one happened.
    fn went_quiet(&self, reader: &Reader) -> GenerateError {
        GenerateError::Quiet {
            heard: true,
            reason: format!(
                "{} seconds of silence after {} events and {} characters",
                self.patience.quiet.as_secs(),
                reader.events,
                reader.text.chars().count()
            ),
        }
    }
}

/// Whether streamed text is user-visible: a reply that opens as a JSON
/// object or a code fence is a shell plan's wire format, which the
/// terminal hides while it streams — so a stream that died mid-plan
/// showed nothing and earns another attempt like an empty one did.
fn planish(text: &str) -> bool {
    let text = text.trim_start();
    text.starts_with('{') || text.starts_with("```")
}

impl Generate for ResponsesDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        // Two kinds of attempt are counted, because two kinds of failure
        // earn another one.
        //
        // A stream that fails before showing anything — empty, or holding
        // only a hidden plan — is safe to redo: the user saw nothing and
        // the request is idempotent. Upstream flakes like a model-side
        // malformed function call are transient, and the retry is
        // invisible.
        //
        // A door that never sent response headers is the other kind. It
        // is indistinguishable from one that dropped the connection, so
        // the request goes again after `Patience::retry_wait`, which
        // doubles on the third attempt.
        //
        // Nothing else is redone. A door that sent headers and then went
        // quiet has already shown the caller where the turn got to, and a
        // second attempt would repeat it. A door that keeps failing after
        // the last attempt surfaces its error — hidden retries, honest
        // failures.
        let mut empty: usize = 0;
        let mut unanswered: u32 = 0;
        loop {
            let mut routed = None;
            let (partial, error) = match self.once(instructions, input, sink, &mut routed).await {
                Ok(done) => {
                    // The gateway names what its router chose; the turn
                    // names it rather than the class it asked for.
                    if let Some((model, upstream)) = routed.filter(|_| self.gateway) {
                        meta(Meta::Model(model));
                        meta(Meta::Upstream(upstream));
                    }
                    return Ok(done);
                }
                Err(failed) => failed,
            };
            match &error {
                GenerateError::Quiet { heard: false, .. } => {
                    unanswered += 1;
                    if unanswered >= HEADER_ATTEMPTS {
                        return Err(GenerateError::Quiet {
                            heard: false,
                            reason: format!(
                                "no response headers in {} seconds, over {unanswered} attempts",
                                self.patience.first_word.as_secs()
                            ),
                        });
                    }
                    tokio::time::sleep(self.patience.retry_wait * (1 << (unanswered - 1))).await;
                }
                GenerateError::Stream(_) | GenerateError::Transport(_)
                    if partial.is_empty() || planish(&partial) =>
                {
                    empty += 1;
                    if empty >= EMPTY_STREAM_ATTEMPTS {
                        return Err(error);
                    }
                    tokio::time::sleep(Duration::from_millis(300 * empty as u64)).await;
                }
                _ => return Err(error),
            }
        }
    }
}

/// Open Responses doors in order: a primary that answers fast while it is
/// there, a fallback, and any number of backups behind it, so a turn is
/// answered while any one of them can answer it.
///
/// The failover is per turn. Every turn goes to the first door that is not
/// benched, and a door that fails before the first words of its answer — an
/// HTTP error such as an unknown model's 404, an empty account's 402, a
/// refused key's 401 or 403, a 429 or a 5xx, a connection that fails, a
/// failure event, a stream that ends empty, nothing at all within
/// [`PRIMARY_FIRST_WORD`], or a door that is streaming its reasoning with
/// no answer text by [`PRIMARY_THINKING`] — hands the same turn to the next
/// door. Nothing has reached the caller by then, so the turn is not
/// repeated where anyone can see it. The last door runs with its own
/// retries and bounds, as a single door does. A door that fails after its
/// first words has shown the caller part of an answer, and its failure is
/// the turn's, as it is for a single door.
///
/// A door whose failure will repeat on the next turn is benched
/// ([`BENCH_GONE`] for a 404, the model gone; [`BENCH_REFUSED`] for a 401,
/// 402, or 403, the key or the account), so later turns skip it rather than
/// pay its failed request first; when every door is benched, every door is
/// asked anyway. Each switch is logged with the door, its cause, and the
/// door that takes the turn ("takes the turn").
///
/// On 2026-10-09 the chat worker's primary (Space Bunny Alpha, retired from
/// OpenRouter) answered 404 and its one fallback (the Vercel AI Gateway)
/// 402, and every chat failed; this chain is why that cannot happen while
/// any door answers.
///
/// Every answer says which door wrote it with [`Meta::Model`] (and, through
/// the inference gateway, [`Meta::Upstream`]), and the door keeps which one
/// answered last ([`FallbackDoor::answering`]), so a reply that names our
/// model can name the one answering now.
pub struct FallbackDoor {
    /// The door every turn tries first.
    pub primary: ResponsesDoor,
    /// The door a turn the primary did not answer goes to.
    pub fallback: ResponsesDoor,
    /// The doors after the fallback, in order.
    pub backups: Vec<ResponsesDoor>,
    first_word: Duration,
    thinking: Duration,
    /// Whether the primary's last turn failed before its first words.
    down: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// The position in the chain of the door that answered last.
    answered: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    /// Benched doors, by URL and model, and when each may be asked again.
    benched: Bench,
}

/// Benched doors, shared by every copy of a chain ([`FallbackDoor::before`]).
///
/// Each holds when the door may be asked again and why it was benched, so
/// a turn that skips it can say why another door answered (#11132).
type Bench = std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, (Instant, Missed)>>>;

/// How long a door whose model is gone (HTTP 404) sits out before it is
/// asked again.
pub const BENCH_GONE: Duration = Duration::from_secs(30 * 60);

/// How long a door that refused its key or its account (HTTP 401, 402, or
/// 403) sits out before it is asked again.
pub const BENCH_REFUSED: Duration = Duration::from_secs(5 * 60);

/// How long `error` benches the door that failed with it: `None` for a
/// failure the next turn may not repeat (a 429, a 5xx, a timeout).
#[must_use]
pub fn bench_for(error: &GenerateError) -> Option<Duration> {
    match error {
        GenerateError::Status(404, _) => Some(BENCH_GONE),
        GenerateError::Status(401..=403, _) => Some(BENCH_REFUSED),
        _ => None,
    }
}

/// The key a door is benched under.
fn bench_key(door: &ResponsesDoor) -> String {
    format!("{} {}", door.url, door.model)
}

impl FallbackDoor {
    /// `primary` first, `fallback` for any turn it does not start
    /// answering in time ([`PRIMARY_FIRST_WORD`], [`PRIMARY_THINKING`]).
    #[must_use]
    pub fn new(primary: ResponsesDoor, fallback: ResponsesDoor) -> Self {
        Self {
            primary,
            fallback,
            backups: Vec::new(),
            first_word: PRIMARY_FIRST_WORD,
            thinking: PRIMARY_THINKING,
            down: std::sync::Arc::default(),
            answered: std::sync::Arc::default(),
            benched: Bench::default(),
        }
    }

    /// The doors in `doors`, in order, with a door that repeats an earlier
    /// one's URL and model left out: a single door when only one is left,
    /// and `None` when there are none.
    #[must_use]
    pub fn chain(doors: Vec<ResponsesDoor>) -> Option<Door> {
        let mut seen = std::collections::HashSet::new();
        let mut doors: Vec<ResponsesDoor> = doors
            .into_iter()
            .filter(|door| seen.insert(bench_key(door)))
            .collect();
        match doors.len() {
            0 => None,
            1 => doors.pop().map(Door::Live),
            _ => {
                let rest = doors.split_off(2);
                let fallback = doors.pop()?;
                let primary = doors.pop()?;
                Some(Door::Fallback(Box::new(
                    FallbackDoor::new(primary, fallback).with_backups(rest),
                )))
            }
        }
    }

    /// The same chain with `backups` after its last door, leaving out any
    /// that repeat a door's URL and model.
    #[must_use]
    pub fn with_backups(mut self, backups: Vec<ResponsesDoor>) -> Self {
        for backup in backups {
            if !self
                .doors()
                .any(|door| bench_key(door) == bench_key(&backup))
            {
                self.backups.push(backup);
            }
        }
        self
    }

    /// Every door, in the order a turn asks them.
    pub fn doors(&self) -> impl Iterator<Item = &ResponsesDoor> {
        [&self.primary, &self.fallback]
            .into_iter()
            .chain(self.backups.iter())
    }

    /// `model` on OpenRouter behind `key`, asking for [`PRIMARY_EFFORT`],
    /// in front of `fallback`.
    #[must_use]
    pub fn openrouter(model: &str, key: &str, fallback: ResponsesDoor) -> Self {
        let primary = ResponsesDoor::new(
            OPENROUTER_DOOR_URL,
            model_at(OPENROUTER_DOOR_URL, model),
            key,
        )
        .with_options(serde_json::Map::from_iter([(
            "reasoning".to_string(),
            json!({ "effort": PRIMARY_EFFORT }),
        )]));
        Self::new(primary, fallback)
    }

    /// The same order with different waits for each door's first event
    /// (`first_word`) and, once it is working, its first words (`thinking`,
    /// from the request), so a test can exercise them without spending the
    /// real ones.
    #[must_use]
    pub fn first_word(mut self, first_word: Duration, thinking: Duration) -> Self {
        self.first_word = first_word;
        self.thinking = thinking;
        self
    }

    /// The same primary and backups, with `fallback` second, sharing this
    /// door's record of whether the primary is answering and which doors
    /// are benched.
    #[must_use]
    pub fn before(&self, fallback: ResponsesDoor) -> Self {
        Self {
            primary: self.primary.clone(),
            fallback,
            backups: self.backups.clone(),
            first_word: self.first_word,
            thinking: self.thinking,
            down: self.down.clone(),
            answered: self.answered.clone(),
            benched: self.benched.clone(),
        }
    }

    /// The same order with every door's request fields replaced by
    /// `options` (see [`ResponsesDoor::with_options`]).
    #[must_use]
    pub fn with_options(mut self, options: serde_json::Map<String, Value>) -> Self {
        self.primary = self.primary.with_options(options.clone());
        self.fallback = self.fallback.with_options(options.clone());
        for backup in &mut self.backups {
            *backup = backup.clone().with_options(options.clone());
        }
        self
    }

    /// The door answering now: the one that answered the last turn, the
    /// primary before any has. The next turn asks the first door that is
    /// not benched either way.
    #[must_use]
    pub fn answering(&self) -> &ResponsesDoor {
        let at = self.answered.load(std::sync::atomic::Ordering::Relaxed);
        self.doors().nth(at).unwrap_or(&self.primary)
    }

    /// Whether the primary's last turn failed before its first words, or
    /// the primary is benched.
    #[must_use]
    pub fn primary_down(&self) -> bool {
        self.down.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether `door` is benched now.
    fn is_benched(&self, door: &ResponsesDoor) -> bool {
        self.benched_for(door).is_some()
    }

    /// Why `door` is benched now, or `None` when it is not.
    fn benched_for(&self, door: &ResponsesDoor) -> Option<Missed> {
        let now = Instant::now();
        self.benched.lock().ok().and_then(|mut benched| {
            benched.retain(|_, (until, _)| *until > now);
            benched.get(&bench_key(door)).map(|(_, why)| *why)
        })
    }

    /// Benches `door` when `error` says its next turn will fail the same
    /// way, and says so once.
    fn bench(&self, door: &ResponsesDoor, error: &GenerateError) {
        let Some(wait) = bench_for(error) else {
            return;
        };
        if let Ok(mut benched) = self.benched.lock() {
            benched.insert(bench_key(door), (Instant::now() + wait, missed(error)));
        }
        eprintln!(
            "door {} at {} benched for {} s ({})",
            door.model,
            door.url,
            wait.as_secs(),
            error.cause()
        );
    }

    /// One door's one attempt, with the caller's sink behind it.
    ///
    /// It has [`PRIMARY_FIRST_WORD`] to show it is working or to start its
    /// answer, and, once working, until [`PRIMARY_THINKING`] from the
    /// request to start it.
    async fn first(
        &self,
        door: &ResponsesDoor,
        instructions: &str,
        input: &[Message],
        sink: &mut (dyn FnMut(&str) + Send),
        routed: &mut Option<(String, String)>,
    ) -> First {
        let relaxed = std::sync::atomic::Ordering::Relaxed;
        let started = tokio::time::Instant::now();
        let spoke = std::sync::atomic::AtomicBool::new(false);
        let alive = std::sync::atomic::AtomicBool::new(false);
        let mut forward = |delta: &str| {
            spoke.store(true, relaxed);
            sink(delta);
        };
        let attempt = door.once_watched(
            instructions,
            input,
            &mut forward,
            Some(&alive),
            Some(routed),
        );
        tokio::pin!(attempt);
        let deadline = tokio::time::sleep(self.first_word);
        tokio::pin!(deadline);
        loop {
            tokio::select! {
                done = &mut attempt => {
                    return match done {
                        Ok(done) => First::Answered(done),
                        Err((partial, error)) if spoke.load(relaxed) || !partial.is_empty() => {
                            First::Failed(error)
                        }
                        Err((_, error)) => First::Missed(error),
                    };
                }
                // The condition is read when the wait begins, and the
                // first words may come while it runs: they win.
                () = &mut deadline, if !spoke.load(relaxed) => {
                    if spoke.load(relaxed) {
                        continue;
                    }
                    let thinking = started + self.thinking;
                    if alive.load(relaxed) && tokio::time::Instant::now() < thinking {
                        deadline.as_mut().reset(thinking);
                        continue;
                    }
                    let heard = alive.load(relaxed);
                    return First::Missed(GenerateError::Quiet {
                        heard,
                        reason: if heard {
                            format!(
                                "working but no answer text in {} ms",
                                started.elapsed().as_millis()
                            )
                        } else {
                            format!("nothing in {} ms", self.first_word.as_millis())
                        },
                    });
                }
            }
        }
    }
}

/// Names the door that answered: the model and upstream the inference
/// gateway's route named, or the door's own model.
fn name_answer(
    door: &ResponsesDoor,
    meta: &mut (dyn FnMut(Meta) + Send),
    routed: Option<(String, String)>,
) {
    match routed.filter(|_| door.is_gateway()) {
        Some((model, upstream)) => {
            meta(Meta::Model(model));
            meta(Meta::Upstream(upstream));
        }
        None if door.is_gateway() => {}
        None => meta(Meta::Model(door.model.clone())),
    }
}

/// How one door's one attempt went.
enum First {
    /// It answered.
    Answered((String, Option<Usage>)),
    /// It failed after its first words reached the caller: the turn's
    /// failure, since a second door would repeat what was shown.
    Failed(GenerateError),
    /// It failed before its first words: the next door takes the turn.
    Missed(GenerateError),
}

impl Generate for FallbackDoor {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        let ordering = std::sync::atomic::Ordering::Relaxed;
        let started = Instant::now();
        // The doors this turn asks, with their places in the chain: every
        // door not benched, or every door when all of them are.
        let mut asked: Vec<(usize, &ResponsesDoor)> = self
            .doors()
            .enumerate()
            .filter(|(_, door)| !self.is_benched(door))
            .collect();
        if asked.is_empty() {
            asked = self.doors().enumerate().collect();
        }
        if asked.first().is_none_or(|(at, _)| *at != 0) {
            self.down.store(true, ordering);
        }
        // The first door that did not answer this turn, and why: a benched
        // door skipped, or a door that missed its first words (#11132).
        let mut switch: Option<Switched> = self
            .doors()
            .next()
            .filter(|_| asked.first().is_some_and(|(at, _)| *at != 0))
            .map(|primary| switched(primary, self.benched_for(primary).unwrap_or(Missed::Error)));
        let last = asked.len() - 1;
        for (step, (at, door)) in asked.iter().copied().enumerate() {
            if step == last {
                // The last door has its own retries and bounds, as a single
                // door does; through the gateway it names what answered.
                let answered = door.generate(instructions, input, sink, meta).await;
                match &answered {
                    Ok(_) => {
                        if let Some(mut switch) = switch.take() {
                            switch.answered = (!door.is_gateway()).then(|| door.model.clone());
                            meta(Meta::Switched(switch));
                        }
                        if !door.is_gateway() {
                            meta(Meta::Model(door.model.clone()));
                        }
                        self.answered.store(at, ordering);
                        if at == 0 {
                            self.down.store(false, ordering);
                        }
                    }
                    Err(error) => {
                        self.bench(door, error);
                        if step > 0 {
                            eprintln!(
                                "door {} failed after {} ms ({}: {}); no door is left to take \
                                 the turn",
                                door.model,
                                started.elapsed().as_millis(),
                                error.cause(),
                                clip(&error.to_string(), 200)
                            );
                        }
                    }
                }
                return answered;
            }
            let mut routed = None;
            match self
                .first(door, instructions, input, sink, &mut routed)
                .await
            {
                First::Answered(done) => {
                    if let Some(mut switch) = switch.take() {
                        switch.answered = match (&routed, door.is_gateway()) {
                            (Some((model, _)), true) => Some(model.clone()),
                            (None, true) => None,
                            (_, false) => Some(door.model.clone()),
                        };
                        meta(Meta::Switched(switch));
                    }
                    name_answer(door, meta, routed);
                    self.answered.store(at, ordering);
                    if at == 0 {
                        self.down.store(false, ordering);
                    }
                    return Ok(done);
                }
                First::Failed(error) => {
                    if at == 0 {
                        self.down.store(false, ordering);
                    }
                    return Err(error);
                }
                First::Missed(error) => {
                    if at == 0 {
                        self.down.store(true, ordering);
                    }
                    self.bench(door, &error);
                    switch.get_or_insert_with(|| switched(door, missed(&error)));
                    let (_, next) = asked[step + 1];
                    // Door, model, cause, and the door's own words: never
                    // the turn's.
                    eprintln!(
                        "door {} missed its first words after {} ms ({}: {}); {} at {} takes \
                         the turn",
                        door.model,
                        started.elapsed().as_millis(),
                        error.cause(),
                        clip(&error.to_string(), 200),
                        next.model,
                        next.url
                    );
                }
            }
        }
        // `asked` is never empty, so the last step returned.
        Err(GenerateError::Config("no door to ask".to_string()))
    }
}

/// A door that is not there: it answers with a fixed line. The shell and
/// the tests use it so neither needs credentials.
///
/// A test that needs a conversation rather than one line gives the stub a
/// script: [`StubGenerate::scripted`] plays each line once, in order, and
/// then says `line` for every generation after that.
pub struct StubGenerate {
    /// What the stub says once the script, if any, is spent.
    pub line: String,
    /// Lines still to play before `line`, front first.
    script: std::sync::Mutex<std::collections::VecDeque<String>>,
    /// The sentence every generation fails with, for a door that has no
    /// model with capacity to answer.
    refusal: Option<String>,
}

impl StubGenerate {
    /// A stub that says `line`, every time.
    #[must_use]
    pub fn saying(line: impl Into<String>) -> Self {
        Self {
            line: line.into(),
            script: std::sync::Mutex::default(),
            refusal: None,
        }
    }

    /// A stub whose every generation fails with
    /// [`GenerateError::NoCapacity`] and `sentence`: what a session opens
    /// when no model it could reach has capacity.
    #[must_use]
    pub fn refusing(sentence: impl Into<String>) -> Self {
        Self {
            refusal: Some(sentence.into()),
            ..Self::saying(String::new())
        }
    }

    /// A stub that plays `script` once and then says `line`.
    #[must_use]
    pub fn scripted(script: Vec<String>, line: impl Into<String>) -> Self {
        Self {
            line: line.into(),
            script: std::sync::Mutex::new(script.into()),
            refusal: None,
        }
    }
}

impl Default for StubGenerate {
    fn default() -> Self {
        Self::saying("(stub door: set CODER_DOOR_KEY or CODER_AI_GATEWAY_KEY for a real answer)")
    }
}

impl Generate for StubGenerate {
    async fn generate<'a>(
        &'a self,
        _instructions: &'a str,
        _input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        _meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        if let Some(sentence) = &self.refusal {
            return Err(GenerateError::NoCapacity(sentence.clone()));
        }
        let line = self
            .script
            .lock()
            .ok()
            .and_then(|mut script| script.pop_front())
            .unwrap_or_else(|| self.line.clone());
        sink(&line);
        Ok((line, None))
    }
}

/// Whatever the environment gives: an own-key door when a key is set,
/// the relay when a worker is configured, the stub otherwise.
pub enum Door {
    /// A live Open Responses endpoint.
    Live(ResponsesDoor),
    /// Two live endpoints in order: the chat worker's primary and the
    /// fallback that takes any turn the primary does not start answering
    /// ([`FallbackDoor`]). Only `coder-worker` builds it.
    Fallback(Box<FallbackDoor>),
    /// A Nostr relay running the NIP-CJ job protocol.
    Relay(Box<crate::relay::RelayDoor>),
    /// An approved local executor, such as the Devin CLI, run through
    /// `delegate` under its boundary.
    Executor(Box<crate::executor_door::ExecutorDoor>),
    /// Claude Code or Codex from a Jev briefing, the door a terminal turn
    /// prefers. See [`crate::delegate_door`]. Only
    /// [`crate::delegate_door::open`] builds it; [`Door::from_env`] never
    /// does, so `coder-worker` keeps the door it had.
    Delegate(std::sync::Arc<crate::delegate_door::DelegateDoor>),
    /// The canned answer.
    Stub(StubGenerate),
}

/// What a session header says for a door that cannot name its model until
/// something answers. The answer steps carry the model that did.
pub const UNKNOWN_MODEL: &str = "unknown";

/// The two variables that ask for an own-key door, in the order
/// [`ResponsesDoor::from_env`] reads them.
const KEY_VARS: [&str; 2] = ["CODER_DOOR_KEY", "CODER_AI_GATEWAY_KEY"];

/// The variable that asks for the relay door.
const WORKER_VAR: &str = "CODER_WORKER";

impl Door {
    /// The configured door: own-key when a key is present, the relay when
    /// `CODER_WORKER` names a worker, stub when neither is set.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the environment asks for two doors at once,
    /// and when the door it asks for cannot be built. Both used to be
    /// silent: a key and a worker together took the key, and an unparseable
    /// worker fell through to the stub. Either precedence is defensible and
    /// the silence is not — someone measuring the relay with a key still
    /// set measures the other transport and gets a plausible number.
    pub fn from_env() -> Result<Self, String> {
        let key = KEY_VARS
            .into_iter()
            .find(|name| env::var(name).is_ok_and(|value| !value.is_empty()));
        let worker = env::var(WORKER_VAR).is_ok_and(|value| !value.is_empty());
        let executor =
            env::var(crate::executor_door::EXECUTOR_VAR).is_ok_and(|value| !value.is_empty());
        match asked_for(key, worker, executor)? {
            Asked::Own => ResponsesDoor::from_env()
                .map(Door::Live)
                .ok_or_else(|| "the door key is set and empty".to_string()),
            Asked::Relay => {
                crate::relay::RelayDoor::from_env().map(|door| Door::Relay(Box::new(door)))
            }
            Asked::Executor => crate::executor_door::ExecutorDoor::from_env()?
                .map(|door| Door::Executor(Box::new(door)))
                .ok_or_else(|| "the executor slug is set and empty".to_string()),
            Asked::Stub => Ok(Door::Stub(StubGenerate::default())),
        }
    }

    /// The same door running `model`, which may be named as a lane.
    ///
    /// This is how a caller whose lane is configured separately from the
    /// agent's takes it: `coder-worker` reads [`WORKER_MODEL_VAR`] and
    /// passes what it finds here.
    ///
    /// # Errors
    ///
    /// Returns a sentence when the door does not pick its own model. A
    /// relay door's worker picks, and the stub answers a fixed line, so a
    /// lane named for either is refused rather than ignored — the same
    /// reason [`Door::from_env`] refuses an environment that names two
    /// doors.
    pub fn serving(self, model: &str) -> Result<Self, String> {
        // Resolved first, so a refusal names the model rather than the
        // lane: what a person needs to read is what would have run.
        let model = model_named(model);
        match self {
            Door::Live(door) => Ok(Door::Live(door.serving(model))),
            Door::Fallback(door) => Err(format!(
                "{model} is named for a door whose primary is {}: name the \
                 fallback before the primary is put in front of it.",
                door.primary.model
            )),
            Door::Relay(_) => Err(format!(
                "{model} is named for a door that does not pick its model: \
                 the relay carries the turn to a worker and the worker picks."
            )),
            Door::Executor(door) => Err(format!(
                "{model} is named for a door that does not pick its model: \
                 {} runs the turn and picks its own.",
                door.slug()
            )),
            Door::Delegate(door) => Err(format!(
                "{model} is named for the delegate door, which runs {}: \
                 name its model with {}.",
                door.label(),
                crate::delegate_door::MODEL_VAR
            )),
            Door::Stub(_) => Err(format!(
                "{model} is named and no door key is set, so the stub door \
                 would answer instead. Set {} or {}.",
                KEY_VARS[0], KEY_VARS[1]
            )),
        }
    }

    /// The same door with `backups` after its last door ([`FallbackDoor`]):
    /// a live door becomes the first of the chain, a fallback door gains
    /// them at its end, and a door that picks its own model (a relay, an
    /// executor, a delegate) or the stub is kept as it is.
    #[must_use]
    pub fn with_backups(self, backups: Vec<ResponsesDoor>) -> Self {
        match self {
            Door::Live(door) => {
                let mut doors = vec![door];
                doors.extend(backups);
                FallbackDoor::chain(doors).unwrap_or_else(|| unreachable!("one door at least"))
            }
            Door::Fallback(door) => Door::Fallback(Box::new(door.with_backups(backups))),
            other => other,
        }
    }

    /// Warm the door's connection: a live door opens its pooled HTTPS
    /// connection now (see [`ResponsesDoor::warm`]); every other door has
    /// nothing to warm.
    pub async fn warm(&self) {
        match self {
            Door::Live(door) => door.warm().await,
            Door::Fallback(door) => {
                futures_util::future::join_all(door.doors().map(ResponsesDoor::warm)).await;
            }
            _ => {}
        }
    }

    /// The model name the door serves, which a trace's session header
    /// records.
    ///
    /// A relay door answers [`UNKNOWN_MODEL`]: the worker picks the model
    /// and names it in the NIP-CJ result, so the session header cannot
    /// know it and each answer step carries what answered. Recording the
    /// word `relay` there instead claimed a model by that name.
    ///
    /// A fallback door answers its primary's model: the one every turn
    /// asks first. Each answer names the model that wrote it
    /// ([`Meta::Model`]).
    pub fn model(&self) -> &str {
        match self {
            Door::Live(door) => &door.model,
            Door::Fallback(door) => &door.primary.model,
            Door::Relay(_) => UNKNOWN_MODEL,
            Door::Executor(door) => door.slug(),
            Door::Delegate(door) => door.label(),
            Door::Stub(_) => "stub",
        }
    }

    /// What the composer's location rail shows: the model when the door
    /// knows one before answering, the door's own name when it does not.
    pub fn label(&self) -> &str {
        match self {
            Door::Relay(_) => self.name(),
            _ => self.model(),
        }
    }

    /// The gateway door behind this one: a live door itself, or a fallback
    /// door's fallback. Lanes the worker runs beside the chat (the Gym's
    /// news model) clone it.
    #[must_use]
    pub fn gateway(&self) -> Option<&ResponsesDoor> {
        match self {
            Door::Live(door) => Some(door),
            Door::Fallback(door) => Some(&door.fallback),
            _ => None,
        }
    }

    /// Which kind of door this is, which a trace records beside the model:
    /// the same model answered through a relay and through an own-key
    /// endpoint is two different paths.
    pub fn name(&self) -> &'static str {
        match self {
            Door::Live(_) | Door::Fallback(_) => "live",
            Door::Relay(_) => "relay",
            Door::Executor(_) => "executor",
            Door::Delegate(_) => crate::delegate_door::NAME,
            Door::Stub(_) => "stub",
        }
    }
}

impl Generate for Door {
    async fn generate<'a>(
        &'a self,
        instructions: &'a str,
        input: &'a [Message],
        sink: &'a mut (dyn FnMut(&str) + Send),
        meta: &'a mut (dyn FnMut(Meta) + Send),
    ) -> Result<(String, Option<Usage>), GenerateError> {
        match self {
            Door::Live(door) => door.generate(instructions, input, sink, meta).await,
            Door::Fallback(door) => door.generate(instructions, input, sink, meta).await,
            Door::Relay(door) => door.generate(instructions, input, sink, meta).await,
            Door::Executor(door) => door.generate(instructions, input, sink, meta).await,
            Door::Delegate(door) => door.generate(instructions, input, sink, meta).await,
            Door::Stub(stub) => stub.generate(instructions, input, sink, meta).await,
        }
    }
}

/// Which door the environment asks for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Asked {
    /// An own-key Open Responses endpoint.
    Own,
    /// The relay, and whichever worker answers on it.
    Relay,
    /// An approved local executor.
    Executor,
    /// None of them, so the canned answer.
    Stub,
}

/// The door a key variable and a worker variable ask for between them,
/// split out from [`Door::from_env`] so it is decided without reading the
/// process environment. `key` names the key variable that is set, when one
/// is.
///
/// # Errors
///
/// Returns a sentence when both are set. Refusing is better than choosing,
/// because the thing the person needs to see is that they asked for two
/// things, and a door that quietly picks one answers the question they did
/// not ask.
fn asked_for(key: Option<&str>, worker: bool, executor: bool) -> Result<Asked, String> {
    let mut named = Vec::new();
    if let Some(key) = key {
        named.push(format!("{key} asks for your own model endpoint"));
    }
    if worker {
        named.push(format!("{WORKER_VAR} asks for the relay"));
    }
    if executor {
        named.push(format!(
            "{} asks for a local executor",
            crate::executor_door::EXECUTOR_VAR
        ));
    }
    if named.len() > 1 {
        return Err(format!(
            "the environment sets {} ways to answer a turn: {}. Unset all but one.",
            named.len(),
            named.join(", ")
        ));
    }
    Ok(match (key, worker, executor) {
        (Some(_), _, _) => Asked::Own,
        (None, true, _) => Asked::Relay,
        (None, false, true) => Asked::Executor,
        (None, false, false) => Asked::Stub,
    })
}

fn clip(text: &str, limit: usize) -> String {
    let clipped: String = text.chars().take(limit).collect();
    if clipped.len() < text.len() {
        format!("{clipped}…")
    } else {
        clipped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_body_speaks_responses_wire_format() {
        let door = ResponsesDoor::new("https://door.example", "m", "k");
        let input = vec![
            Message {
                role: Role::User,
                text: "hi".to_string(),
            },
            Message {
                role: Role::Assistant,
                text: "hello".to_string(),
            },
        ];
        let body = door.body("sys", &input);
        assert_eq!(body["model"], "m");
        assert_eq!(body["instructions"], "sys");
        assert_eq!(body["stream"], true);
        assert_eq!(body["store"], false);
        assert_eq!(body["tool_choice"], "none");
        assert_eq!(body["tools"], json!([]));
        assert_eq!(body["input"][0]["content"][0]["type"], "input_text");
        assert_eq!(body["input"][1]["content"][0]["type"], "output_text");
    }

    /// #11064: the inference gateway door carries the privacy level in
    /// `openagents`, never a provider's own fields, and reads the model and
    /// upstream the gateway chose from `openagents:route`.
    #[test]
    fn the_gateway_door_asks_in_openagents_terms_and_reads_the_route() {
        let door = ResponsesDoor::new("http://127.0.0.1:8790", "openagents/chat", "oak_x.y")
            .with_privacy(ProviderPrivacy::Strict)
            .through_gateway();
        assert!(door.is_gateway());
        let body = door.body("sys", &[]);
        assert_eq!(body["openagents"]["privacy"], "strict");
        assert!(body.get("provider").is_none() && body.get("providerOptions").is_none());
        let lowered = door.clone().with_privacy(ProviderPrivacy::Off);
        assert_eq!(
            lowered.body("sys", &[])["openagents"]["privacy"],
            "standard"
        );

        let mut reader = Reader::default();
        let stream = concat!(
            "data: {\"type\":\"response.created\",\"sequence_number\":0,\"response\":{}}\n\n",
            "data: {\"type\":\"openagents:route\",\"sequence_number\":1,\"model\":\"google/gemini-3.8-flash\",\"upstream\":\"openrouter\"}\n\n",
            "data: {\"type\":\"response.output_text.delta\",\"sequence_number\":2,\"delta\":\"hi\"}\n\n",
            "data: {\"type\":\"openagents:cost\",\"sequence_number\":3,\"cost\":{}}\n\n",
            "data: {\"type\":\"response.completed\",\"sequence_number\":4,\"response\":{}}\n\n",
            "data: [DONE]\n\n",
        );
        reader.push(stream.as_bytes(), &mut |_| {}).expect("read");
        assert_eq!(
            reader.route,
            Some((
                "google/gemini-3.8-flash".to_string(),
                "openrouter".to_string()
            ))
        );
        let (text, _) = reader.finish(&mut |_| {}).expect("answer");
        assert_eq!(text, "hi");
    }

    /// #11040: by default the OpenRouter door asks for no data collection
    /// and zero retention, and the gateway door for zero retention, both
    /// beside `store: false`.
    #[test]
    fn doors_ask_providers_not_to_keep_or_train_by_default() {
        assert_eq!(ProviderPrivacy::default(), ProviderPrivacy::Strict);
        let openrouter = ResponsesDoor::new(OPENROUTER_DOOR_URL, "m", "k")
            .with_privacy(ProviderPrivacy::default());
        let body = openrouter.body("sys", &[]);
        assert_eq!(body["store"], false);
        assert_eq!(body["provider"]["data_collection"], "deny");
        assert_eq!(body["provider"]["zdr"], true);
        assert_eq!(body.get("providerOptions"), None);

        let gateway =
            ResponsesDoor::new(DEFAULT_DOOR_URL, "m", "k").with_privacy(ProviderPrivacy::default());
        let body = gateway.body("sys", &[]);
        assert_eq!(body["store"], false);
        assert_eq!(
            body["providerOptions"]["gateway"]["zeroDataRetention"],
            true
        );
        assert_eq!(body.get("provider"), None);

        // The chat worker's primary, built the way the worker builds it.
        let ordered = FallbackDoor::openrouter(
            "space-bunny",
            "o",
            ResponsesDoor::new(DEFAULT_DOOR_URL, DEFAULT_MODEL, "g"),
        );
        if ordered.primary.privacy() == ProviderPrivacy::Strict {
            let body = ordered.primary.body("sys", &[]);
            assert_eq!(body["provider"]["data_collection"], "deny");
            assert_eq!(body["reasoning"], json!({ "effort": PRIMARY_EFFORT }));
        }

        // Another endpoint gets only `store: false`.
        let local = ResponsesDoor::new("http://127.0.0.1:9", "m", "k")
            .with_privacy(ProviderPrivacy::Strict)
            .body("sys", &[]);
        assert_eq!(local["store"], false);
        assert_eq!(local.get("provider"), None);
        assert_eq!(local.get("providerOptions"), None);
    }

    /// A lane's own provider options keep their fields beside the privacy
    /// ones, and the lower levels ask for less.
    #[test]
    fn privacy_merges_with_lane_options_and_levels_parse() {
        let options =
            json!({ "providerOptions": { "zai": { "thinking": { "type": "disabled" } } } });
        let body = ResponsesDoor::new(DEFAULT_DOOR_URL, "m", "k")
            .with_options(options.as_object().cloned().unwrap_or_default())
            .with_privacy(ProviderPrivacy::Strict)
            .body("sys", &[]);
        assert_eq!(
            body["providerOptions"]["zai"]["thinking"]["type"],
            "disabled"
        );
        assert_eq!(
            body["providerOptions"]["gateway"]["zeroDataRetention"],
            true
        );

        let body = ResponsesDoor::new(OPENROUTER_DOOR_URL, "m", "k")
            .with_privacy(ProviderPrivacy::NoTraining)
            .body("sys", &[]);
        assert_eq!(body["provider"], json!({ "data_collection": "deny" }));
        let body = ResponsesDoor::new(DEFAULT_DOOR_URL, "m", "k")
            .with_privacy(ProviderPrivacy::NoTraining)
            .body("sys", &[]);
        assert_eq!(body.get("providerOptions"), None);
        let body = ResponsesDoor::new(OPENROUTER_DOOR_URL, "m", "k")
            .with_privacy(ProviderPrivacy::Off)
            .body("sys", &[]);
        assert_eq!(body.get("provider"), None);
        assert_eq!(body["store"], false);

        for level in [
            ProviderPrivacy::Strict,
            ProviderPrivacy::NoTraining,
            ProviderPrivacy::Off,
        ] {
            assert_eq!(ProviderPrivacy::parse(level.word()), Ok(level));
        }
        assert_eq!(ProviderPrivacy::parse(""), Ok(ProviderPrivacy::Strict));
        assert!(ProviderPrivacy::parse("loose").is_err());
    }

    #[tokio::test]
    async fn the_stub_answers_and_reports_no_usage() {
        let stub = StubGenerate::default();
        let mut seen = String::new();
        let (text, usage) = stub
            .generate("sys", &[], &mut |delta| seen.push_str(delta), &mut |_| {})
            .await
            .unwrap();
        assert_eq!(text, stub.line);
        assert_eq!(seen, stub.line);
        assert!(usage.is_none());
    }

    /// Four relay failures, four causes. A typed refusal carries its code
    /// as a field; the three that never got an answer carry none.
    #[test]
    fn the_relay_failures_file_under_four_causes() {
        let unreachable = GenerateError::Relay("connect: refused".to_string());
        let absent = GenerateError::Silent {
            heard: false,
            reason: "nothing came back in 30 seconds".to_string(),
        };
        let stalled = GenerateError::Silent {
            heard: true,
            reason: "no result in 180 seconds".to_string(),
        };
        let declined = GenerateError::Refused {
            code: "quota_exhausted".to_string(),
            message: "free allowance used".to_string(),
        };

        assert_eq!(unreachable.cause(), "relay_unreachable");
        assert_eq!(declined.cause(), "worker_declined");

        // Silence is two states, and a harness reads which on the field
        // rather than on the sentence.
        assert_eq!(absent.cause(), "worker_absent");
        assert_eq!(stalled.cause(), "worker_stalled");
        assert_eq!(
            absent.to_string(),
            "no worker answered: nothing came back in 30 seconds"
        );
        assert_eq!(
            stalled.to_string(),
            "the worker stopped mid-answer: no result in 180 seconds"
        );

        assert_eq!(unreachable.refusal(), None);
        assert_eq!(absent.refusal(), None);
        assert_eq!(stalled.refusal(), None);
        assert_eq!(declined.refusal(), Some("quota_exhausted"));

        assert_eq!(
            declined.to_string(),
            "the worker declined (quota_exhausted): free allowance used"
        );
        // A door failure is not a relay failure, whatever the prose says.
        assert_eq!(
            GenerateError::Stream("the relay went away".to_string()).cause(),
            "stream"
        );
    }

    /// An environment that asks for two doors is refused, and the refusal
    /// names both variables. Silently preferring one is how a relay
    /// measurement comes to be a measurement of the other transport.
    #[test]
    fn two_configured_doors_are_a_refusal_rather_than_a_choice() {
        let both = asked_for(Some("CODER_DOOR_KEY"), true, false).expect_err("two doors");
        assert!(both.contains("CODER_DOOR_KEY"), "{both}");
        assert!(both.contains("CODER_WORKER"), "{both}");
        let with_executor = asked_for(None, true, true).expect_err("two doors");
        assert!(with_executor.contains("CODER_EXECUTOR"), "{with_executor}");

        assert_eq!(
            asked_for(Some("CODER_DOOR_KEY"), false, false),
            Ok(Asked::Own)
        );
        assert_eq!(
            asked_for(Some("CODER_AI_GATEWAY_KEY"), false, false),
            Ok(Asked::Own)
        );
        assert_eq!(asked_for(None, true, false), Ok(Asked::Relay));
        assert_eq!(asked_for(None, false, true), Ok(Asked::Executor));
        assert_eq!(asked_for(None, false, false), Ok(Asked::Stub));
    }

    /// A relay door cannot name its model before a worker answers, and
    /// says so rather than naming the transport as if it were a model.
    #[test]
    fn a_door_that_cannot_name_its_model_says_unknown() {
        let live = Door::Live(ResponsesDoor::new("https://door.example", "a/model", "k"));
        assert_eq!(live.model(), "a/model");
        assert_eq!(live.label(), "a/model");
        assert_eq!(Door::Stub(StubGenerate::default()).model(), "stub");
        assert_eq!(UNKNOWN_MODEL, "unknown");
    }

    /// Three lanes, three models, one client. A lane is readable by short
    /// name and by model id, so a shell that says `glm` and one that says
    /// `zai/glm-5.3-flash` ask for the same door.
    #[test]
    fn a_lane_is_a_model_under_a_short_name() {
        assert_eq!(Lane::Gemini.model(), "google/gemini-3.8-flash");
        assert_eq!(Lane::Glm.model(), "zai/glm-5.3-flash");
        assert_eq!(Lane::SpaceBunny.model(), "stealth/space-bunny-alpha");
        assert_eq!(DEFAULT_MODEL, Lane::Gemini.model());

        assert_eq!(Lane::read("glm"), Some(Lane::Glm));
        assert_eq!(Lane::read(" zai/glm-5.3-flash "), Some(Lane::Glm));
        assert_eq!(Lane::read("gemini"), Some(Lane::Gemini));
        assert_eq!(Lane::read("moonshot/kimi"), None);

        // A name that is no lane is a model id, so the gateway's whole
        // catalog stays reachable without a lane of its own.
        assert_eq!(model_named("glm"), "zai/glm-5.3-flash");
        assert_eq!(model_named(" moonshot/kimi "), "moonshot/kimi");

        // Every lane's name and model are distinct, which is what lets
        // one field carry either.
        let mut seen: Vec<&str> = Lane::ALL
            .iter()
            .flat_map(|lane| [lane.name(), lane.model()])
            .collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count);
    }

    /// A door that picks its own model takes a lane; one that does not
    /// refuses rather than dropping the request.
    ///
    /// A worker told to run `glm` while its door is the relay would answer
    /// on whatever model the far end picked, and the trace would name that
    /// model with nothing recording that the lane had been asked for and
    /// ignored.
    #[test]
    fn only_a_door_that_picks_its_model_takes_a_lane() {
        let live = Door::Live(ResponsesDoor::new(DEFAULT_DOOR_URL, DEFAULT_MODEL, "k"))
            .serving("glm")
            .expect("a live door takes a lane");
        assert_eq!(live.model(), Lane::Glm.model());
        assert_eq!(live.label(), Lane::Glm.model());

        let Err(stub) = Door::Stub(StubGenerate::default()).serving("glm") else {
            panic!("the stub door does not run a model");
        };
        assert!(stub.contains(Lane::Glm.model()), "{stub}");
        assert!(stub.contains("CODER_DOOR_KEY"), "{stub}");
    }

    /// Four door failures, four causes, and the streaming door's two words
    /// for silence sit beside the relay's rather than replacing them.
    #[test]
    fn a_quiet_door_files_under_two_causes() {
        let absent = GenerateError::Quiet {
            heard: false,
            reason: "no response headers in 30 seconds, over 3 attempts".to_string(),
        };
        let stalled = GenerateError::Quiet {
            heard: true,
            reason: "120 seconds of silence after 12 events and 400 characters".to_string(),
        };

        assert_eq!(absent.cause(), "door_absent");
        assert_eq!(stalled.cause(), "door_stalled");
        assert_eq!(absent.refusal(), None);
        assert_eq!(stalled.refusal(), None);
        assert_eq!(
            absent.to_string(),
            "the model endpoint did not answer: no response headers in 30 seconds, over 3 attempts"
        );
        // How long it waited and how much had arrived: "it hung" and "it
        // sent 400 characters and stopped" are different problems.
        assert_eq!(
            stalled.to_string(),
            "the model endpoint stopped sending partway through its answer: 120 seconds of silence after 12 events \
             and 400 characters"
        );
    }

    /// The worker's primary: Gemini Flash on OpenRouter whenever
    /// OpenRouter's key is here and nothing names another (Space Bunny
    /// Alpha is retired), a named one when it is, nothing when it is `off`
    /// or there is no key, and a refusal when one is named with no key to
    /// reach it.
    #[test]
    fn the_primary_is_gemini_on_openrouter_whenever_openrouter_is_reachable() {
        assert_eq!(
            worker_primary(None, Some("k")),
            Ok(Some((
                Lane::Gemini.openrouter_model().to_string(),
                "k".to_string()
            )))
        );
        assert_eq!(worker_primary(Some(" "), None), Ok(None));
        assert_eq!(worker_primary(None, None), Ok(None));
        assert_eq!(worker_primary(Some("off"), Some("k")), Ok(None));
        assert_eq!(
            worker_primary(Some("space-bunny"), Some("k")),
            Ok(Some((
                Lane::SpaceBunny.model().to_string(),
                "k".to_string()
            )))
        );
        assert_eq!(
            worker_primary(Some("stealth/gone"), Some(" k ")),
            Ok(Some(("stealth/gone".to_string(), "k".to_string())))
        );
        let refused = worker_primary(Some("space-bunny"), Some("")).expect_err("no key");
        assert!(refused.contains(OPENROUTER_KEY_VAR), "{refused}");
        assert_eq!(Lane::SpaceBunny.door_url(), OPENROUTER_DOOR_URL);
        assert_eq!(Lane::Gemini.door_url(), DEFAULT_DOOR_URL);
    }

    /// The primary asks OpenRouter at low reasoning; the door names the
    /// primary's model, is a live door, and hands lanes beside the chat
    /// its gateway door.
    #[test]
    fn a_fallback_door_asks_the_primary_at_low_reasoning() {
        let gateway = ResponsesDoor::new(DEFAULT_DOOR_URL, DEFAULT_MODEL, "g");
        let ordered = FallbackDoor::openrouter("space-bunny", "o", gateway);
        assert_eq!(ordered.primary.url, OPENROUTER_DOOR_URL);
        assert_eq!(ordered.primary.model, Lane::SpaceBunny.model());
        let body = ordered.primary.body("sys", &[]);
        assert_eq!(body["reasoning"], json!({ "effort": PRIMARY_EFFORT }));
        assert_eq!(body["tool_choice"], "none");
        assert_eq!(ordered.fallback.body("sys", &[]).get("reasoning"), None);
        assert!(!ordered.primary_down());
        assert_eq!(ordered.answering().model, Lane::SpaceBunny.model());

        let door = Door::Fallback(Box::new(ordered));
        assert_eq!(door.model(), Lane::SpaceBunny.model());
        assert_eq!(door.name(), "live");
        assert_eq!(
            door.gateway().map(|g| g.model.as_str()),
            Some(DEFAULT_MODEL)
        );
        let Err(refused) = door.serving("glm") else {
            panic!("a fallback door's lane is set before the primary goes in front");
        };
        assert!(refused.contains(Lane::Glm.model()), "{refused}");
    }

    #[test]
    fn no_key_means_no_door() {
        // The lookup reads the real environment; with neither variable set a
        // door cannot be built. When a key IS set the test still passes: the
        // door exists. What matters is it never panics.
        let _ = ResponsesDoor::from_env();
    }

    /// A short stream with a multibyte answer, CRLF line endings, a
    /// comment, an `event:` field, a keepalive, and a completed event.
    const STREAM: &str = "event: response.created\r\n\
        data: {\"type\":\"response.created\"}\r\n\r\n\
        : keepalive\r\n\r\n\
        event: response.output_text.delta\r\n\
        data: {\"type\":\"response.output_text.delta\",\"delta\":\"héllo \"}\r\n\r\n\
        data:{\"type\":\"response.output_text.delta\",\"delta\":\"wörld 日本\"}\r\n\r\n\
        data: {\"type\":\"response.completed\",\"response\":{\"usage\":{\"input_tokens\":3,\"output_tokens\":4}}}\r\n\r\n\
        data: [DONE]\r\n\r\n";

    /// Feeds `stream` split at `at` and reads it to the end.
    fn read_split(
        stream: &[u8],
        at: usize,
    ) -> Result<(String, Option<Usage>), (String, GenerateError)> {
        let mut reader = Reader::default();
        let mut seen = String::new();
        let mut sink = |delta: &str| seen.push_str(delta);
        for chunk in [&stream[..at], &stream[at..]] {
            if let Err(error) = reader.push(chunk, &mut sink) {
                return Err((reader.text, error));
            }
        }
        let answered = reader.finish(&mut sink)?;
        assert_eq!(seen, answered.0, "the sink saw what was answered");
        Ok(answered)
    }

    #[test]
    fn a_chunk_boundary_anywhere_leaves_the_answer_whole() {
        let bytes = STREAM.as_bytes();
        for at in 0..=bytes.len() {
            let (text, usage) =
                read_split(bytes, at).unwrap_or_else(|(_, error)| panic!("split at {at}: {error}"));
            assert_eq!(text, "héllo wörld 日本", "split at {at}");
            assert_eq!(
                usage,
                Some(Usage {
                    input_tokens: 3,
                    output_tokens: 4
                })
            );
        }
    }

    #[test]
    fn a_stream_arriving_one_byte_at_a_time_reads_the_same() {
        let mut reader = Reader::default();
        let mut sink = |_: &str| {};
        for byte in STREAM.as_bytes() {
            reader.push(&[*byte], &mut sink).unwrap();
        }
        let (text, _) = reader
            .finish(&mut sink)
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        assert_eq!(text, "héllo wörld 日本");
    }

    #[test]
    fn data_lines_of_one_event_join_with_a_newline() {
        let stream = "data: {\"type\":\"response.output_text.delta\",\n\
                      data: \"delta\":\"two lines\"}\n\n\
                      data: {\"type\":\"response.completed\"}\n\n";
        let mut reader = Reader::default();
        let mut sink = |_: &str| {};
        reader.push(stream.as_bytes(), &mut sink).unwrap();
        let (text, _) = reader
            .finish(&mut sink)
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        assert_eq!(text, "two lines");
    }

    #[test]
    fn a_last_event_without_a_blank_line_still_counts() {
        let stream = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"x\"}\n\n\
                      data: {\"type\":\"response.completed\"}";
        let mut reader = Reader::default();
        let mut sink = |_: &str| {};
        reader.push(stream.as_bytes(), &mut sink).unwrap();
        let (text, _) = reader
            .finish(&mut sink)
            .unwrap_or_else(|(_, error)| panic!("{error}"));
        assert_eq!(text, "x");
    }

    #[test]
    fn a_stream_that_ends_before_completing_is_a_failure_that_keeps_the_text() {
        let cut = STREAM
            .find("data: {\"type\":\"response.completed\"")
            .unwrap();
        let (partial, error) =
            read_split(&STREAM.as_bytes()[..cut], 10).expect_err("no completed event");
        assert_eq!(partial, "héllo wörld 日本");
        assert!(matches!(error, GenerateError::Stream(_)), "{error}");
        assert_eq!(
            error.to_string(),
            "stream: the stream ended before response.completed, after 3 events"
        );
    }

    #[test]
    fn an_incomplete_response_is_a_failure_with_its_reason() {
        let stream = "data: {\"type\":\"response.output_text.delta\",\"delta\":\"x\"}\n\n\
                      data: {\"type\":\"response.incomplete\",\"response\":{\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}\n\n";
        let (partial, error) = read_split(stream.as_bytes(), 5).expect_err("incomplete");
        assert_eq!(partial, "x");
        assert!(error.to_string().ends_with("max_output_tokens"), "{error}");
    }

    #[test]
    fn a_record_that_is_not_json_or_not_utf8_is_a_failure_not_a_skip() {
        let mut sink = |_: &str| {};
        let mut reader = Reader::default();
        let error = reader
            .push(b"data: {not json\n\n", &mut sink)
            .expect_err("not JSON");
        assert!(error.to_string().contains("not JSON"), "{error}");

        let mut reader = Reader::default();
        let error = reader
            .push(b"data: \"\xff\xfe\"\n\n", &mut sink)
            .expect_err("not UTF-8");
        assert!(error.to_string().contains("not UTF-8"), "{error}");
    }

    #[test]
    fn events_after_completed_are_a_failure() {
        let stream = "data: {\"type\":\"response.completed\"}\n\n\
                      data: {\"type\":\"response.output_text.delta\",\"delta\":\"late\"}\n\n";
        let mut sink = |_: &str| {};
        let mut reader = Reader::default();
        let error = reader
            .push(stream.as_bytes(), &mut sink)
            .expect_err("late event");
        assert!(
            error.to_string().contains("after response.completed"),
            "{error}"
        );
    }

    #[test]
    fn a_line_that_never_ends_is_bounded() {
        let mut sink = |_: &str| {};
        let mut reader = Reader::default();
        let chunk = vec![b'a'; MAX_EVENT_BYTES / 2 + 1];
        reader.push(&chunk, &mut sink).unwrap();
        let error = reader.push(&chunk, &mut sink).expect_err("past the bound");
        assert!(error.to_string().contains("without ending"), "{error}");
    }

    #[test]
    fn a_keepalive_is_not_an_event() {
        let mut sink = |_: &str| {};
        let mut reader = Reader::default();
        assert_eq!(reader.push(b": ping\n\n\n\n", &mut sink).unwrap(), 0);
        assert_eq!(reader.events, 0);
    }

    /// The backup doors: by default every entry whose key is here, with
    /// each lane named the way its door names it; `off` for none; a listed
    /// entry with no key, an unknown door, or no model is refused.
    #[test]
    fn the_backups_are_every_door_whose_key_is_here() {
        let named = |doors: Vec<ResponsesDoor>| {
            doors
                .into_iter()
                .map(|door| (door.url, door.model))
                .collect::<Vec<_>>()
        };
        assert_eq!(
            named(worker_backups(None, Some("o"), Some("v")).unwrap()),
            vec![
                (
                    OPENROUTER_DOOR_URL.to_string(),
                    Lane::Gemini.openrouter_model().to_string()
                ),
                (
                    OPENROUTER_DOOR_URL.to_string(),
                    Lane::Glm.openrouter_model().to_string()
                ),
                (
                    DEFAULT_DOOR_URL.to_string(),
                    Lane::Gemini.model().to_string()
                ),
                (DEFAULT_DOOR_URL.to_string(), Lane::Glm.model().to_string()),
            ]
        );
        assert_ne!(Lane::Glm.openrouter_model(), Lane::Glm.model());
        assert_eq!(worker_backups(None, None, Some("v")).unwrap().len(), 2);
        assert!(worker_backups(None, None, None).unwrap().is_empty());
        assert!(
            worker_backups(Some("off"), Some("o"), Some("v"))
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            named(worker_backups(Some("vercel:glm"), None, Some("v")).unwrap()),
            vec![(DEFAULT_DOOR_URL.to_string(), Lane::Glm.model().to_string())]
        );
        let unkeyed = worker_backups(Some("openrouter:glm"), None, Some("v"))
            .err()
            .expect("refused");
        assert!(unkeyed.contains(OPENROUTER_KEY_VAR), "{unkeyed}");
        assert!(worker_backups(Some("vertex:gemini"), Some("o"), None).is_err());
        assert!(worker_backups(Some("openrouter"), Some("o"), None).is_err());
    }

    /// A chain leaves out a door that repeats an earlier one's URL and
    /// model, is one live door when one is left, and a live door with
    /// backups becomes a chain.
    #[test]
    fn a_chain_has_each_door_once() {
        let door = |url: &str, model: &str| ResponsesDoor::new(url, model, "k");
        assert!(FallbackDoor::chain(Vec::new()).is_none());
        let one = FallbackDoor::chain(vec![door("https://a", "m"), door("https://a", "m")]);
        assert!(matches!(one, Some(Door::Live(_))));
        let Some(Door::Fallback(three)) = FallbackDoor::chain(vec![
            door("https://a", "m"),
            door("https://b", "m"),
            door("https://a", "m"),
            door("https://a", "n"),
        ]) else {
            panic!("a chain");
        };
        let order: Vec<_> = three
            .doors()
            .map(|d| format!("{} {}", d.url, d.model))
            .collect();
        assert_eq!(order, ["https://a m", "https://b m", "https://a n"]);
        let Door::Fallback(grown) =
            Door::Live(door("https://a", "m")).with_backups(vec![door("https://b", "m")])
        else {
            panic!("a live door with a backup is a chain");
        };
        assert_eq!(grown.fallback.url, "https://b");
        let stub = Door::Stub(StubGenerate::default()).with_backups(vec![door("https://b", "m")]);
        assert!(matches!(stub, Door::Stub(_)));
    }

    /// A model gone (404) and a refused key or account (401, 402, 403)
    /// bench a door; a rate limit, a 5xx, or silence does not.
    #[test]
    fn what_benches_a_door() {
        let status = |code| GenerateError::Status(code, String::new());
        assert_eq!(bench_for(&status(404)), Some(BENCH_GONE));
        for code in [401, 402, 403] {
            assert_eq!(bench_for(&status(code)), Some(BENCH_REFUSED), "{code}");
        }
        for code in [400, 429, 500, 503] {
            assert_eq!(bench_for(&status(code)), None, "{code}");
        }
        let quiet = GenerateError::Quiet {
            heard: false,
            reason: String::new(),
        };
        assert_eq!(bench_for(&quiet), None);
        assert_eq!(model_at(OPENROUTER_DOOR_URL, "glm"), "z-ai/glm-5.3-flash");
        assert_eq!(model_at(DEFAULT_DOOR_URL, "glm"), Lane::Glm.model());
        assert_eq!(model_at(OPENROUTER_DOOR_URL, "x/y"), "x/y");
    }
}
