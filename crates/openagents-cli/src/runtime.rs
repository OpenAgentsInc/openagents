//! The coder execution layer: tiers, threads, grants, and the streaming turn.
//!
//! Two lanes reach a model, and they share nothing but this file's message
//! list:
//!
//! - The **thread lane** opens `POST /api/v1/threads`, takes the grant that
//!   comes back, and streams `POST /api/inference/proxy` with the grant's
//!   bearer token. The thread ends by saying what it did, with
//!   `POST /api/v1/threads/{id}/report`.
//! - The **local lane** talks to an Ollama server on this machine and never
//!   touches openagents.com at all, so it answers with the proxy unreachable.
//!
//! ## A turn that cannot reach a model fails
//!
//! Every path out of [`CoderRuntimeSession::execute_turn`] is either the
//! model's own words or an `Err`. There is no fallback model, no synthesized
//! reply, and no invented grant. Two of those existed here and both reached
//! readers: `create_thread` answered a refusal by returning a grant it made up
//! with the caller's own PAT inside it, and the proxy arm answered a rejected
//! request with the sentence `Completed autonomous reasoning turn (offline
//! fallback).` and exit 0. Neither comes back.
//!
//! ## A turn writes itself locally before the thread is settled
//!
//! Coder appends `turn.user`, `turn.reasoning`, `tool.ran`, and
//! `turn.assistant` to its local session journal as they happen. That is the
//! vocabulary [`crate::resume`] replays. `--cloud-history` additionally sends
//! those records to `POST /api/v1/threads/{id}/events`; local-only is the
//! default.
//!
//! A turn that did not answer records [`ThreadRecord::failed`] instead, and
//! never an answer: the point of writing the turn down is that the record
//! matches what happened, which it does not if a refused proxy, a broken
//! stream, an interrupted session, or an exhausted step budget lands in the
//! transcript looking like an answer.
//!
//! A write failure does not replace an answer. It is retained in
//! [`CoderRuntimeSession::record_failures`] so the caller can report the
//! persistence failure separately.
//!
//! ## A session says how it ended, and `DELETE` is a disposal
//!
//! `DELETE /api/v1/threads/{id}` writes `error_code: cancelled` and the
//! sentence *The thread was cancelled before it reported.* That is the only
//! thing this file used to send, so a run that answered correctly and exited 0
//! left a permanent record saying it had been cancelled — 31 of one account's
//! 50 most recent threads (issue #106).
//!
//! Every exit now goes through [`CoderRuntimeSession::finish`], which sends
//! `POST /api/v1/threads/{id}/report` with the outcome the session actually
//! reached and revokes in the same call. `DELETE` stays for the one case it
//! describes: throwing a thread away.
//!
//! The mirror of that bug would be worse, so the outcome is not a caller's to
//! assert. [`CoderRuntimeSession::execute_turn`] settles it from the turn's own
//! `Result` in one place, and [`ThreadOutcome`] has no constructor that can
//! pair `succeeded` with an error code or a failure without one — the server
//! refuses an incoherent pair twice over, and this file does not try to send
//! one. A turn still in flight leaves the session's standing outcome
//! [`ThreadOutcome::interrupted`], so a session dropped mid-turn reports as
//! interrupted rather than inheriting the last turn's success.
//!
//! Reporting rather than cancelling is also what makes `--resume` work across
//! processes: `POST /threads/{id}/grants` reopens a thread that reported and
//! refuses one that was cancelled.
//!
//! ## Model ids are the server's, not this file's
//!
//! The deployment publishes its catalog at `GET /api/v1/models` and refuses
//! anything outside it. A previous version of this file carried five model ids
//! it had invented — `gemini-3.7-pro`, `claude-3-7-sonnet`, `codex-preview`
//! among them — and sent them as the thread's `lane`, which the server also
//! refuses. The tier table below maps the names a reader types onto ids the
//! catalog actually served when it was written, and a name outside that table
//! is checked against the live catalog before a thread is opened rather than
//! guessed at.

use crate::tools::{HarnessToolRegistry, ToolCall, ToolDefinition};
use eventsource_stream::Eventsource;
use futures::{Stream, StreamExt, future::join_all};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;

type Failure = Box<dyn std::error::Error + Send + Sync>;
const FIRST_RESPONSE_RETRIES: usize = 1;

/// How a turn's first-response watchdog changes while the model is silent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TurnProgress {
    /// The model has produced no content, reasoning, or tool call yet.
    Waiting { retry: usize, max_retries: usize },
    /// The silent request reached its deadline and the runtime is replacing it.
    Retrying { retry: usize, max_retries: usize },
    /// The model responded or the turn ended, so the waiting state can leave.
    Clear,
}

pub type TurnProgressObserver = Arc<dyn Fn(TurnProgress) + Send + Sync>;

/// One live model event, before the runtime knows whether the current model
/// step will answer or call a tool.
///
/// Content is provisional until [`ModelStreamEvent::ContentCommitted`]. A
/// model can write a preamble and then request a tool in the same response;
/// [`ModelStreamEvent::ContentDiscarded`] lets a live interface remove that
/// preamble instead of preserving it as the turn's final answer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModelStreamEvent {
    /// A piece of assistant-visible text, in wire order.
    ContentDelta(String),
    /// The current step's text is the final answer.
    ContentCommitted,
    /// The current step requested tools, so its text is not the final answer.
    ContentDiscarded,
    /// A piece of the model's reasoning summary, in wire order.
    ReasoningDelta(String),
}

pub type ModelStreamObserver = Arc<dyn Fn(ModelStreamEvent) + Send + Sync>;

#[derive(Debug, Clone, Copy)]
struct FirstResponsePolicy {
    waiting_after: Duration,
    timeout_after: Duration,
}

impl Default for FirstResponsePolicy {
    fn default() -> Self {
        Self {
            waiting_after: Duration::from_secs(3),
            timeout_after: Duration::from_secs(10),
        }
    }
}

enum WatchdogNext<T> {
    Item(Option<T>),
    Waiting,
    TimedOut,
}

async fn next_before_first_response<S>(
    stream: &mut S,
    policy: FirstResponsePolicy,
    started: tokio::time::Instant,
    waiting_sent: &mut bool,
    retry: usize,
    observer: Option<&TurnProgressObserver>,
) -> WatchdogNext<S::Item>
where
    S: Stream + Unpin,
{
    let waiting_at = started + policy.waiting_after;
    let timeout_at = started + policy.timeout_after;

    if *waiting_sent {
        tokio::select! {
            item = stream.next() => WatchdogNext::Item(item),
            _ = tokio::time::sleep_until(timeout_at) => WatchdogNext::TimedOut,
        }
    } else {
        tokio::select! {
            item = stream.next() => WatchdogNext::Item(item),
            _ = tokio::time::sleep_until(waiting_at) => {
                *waiting_sent = true;
                if let Some(observer) = observer {
                    observer(TurnProgress::Waiting {
                        retry,
                        max_retries: FIRST_RESPONSE_RETRIES,
                    });
                }
                WatchdogNext::Waiting
            }
            _ = tokio::time::sleep_until(timeout_at) => WatchdogNext::TimedOut,
        }
    }
}

fn openai_response_started(value: &serde_json::Value) -> bool {
    let Some(delta) = value
        .get("choices")
        .and_then(|choices| choices.as_array())
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("delta"))
    else {
        return false;
    };

    delta
        .get("content")
        .and_then(|value| value.as_str())
        .is_some_and(|value| !value.is_empty())
        || delta
            .get("reasoning")
            .and_then(|value| value.as_str())
            .is_some_and(|value| !value.is_empty())
        || delta
            .get("tool_calls")
            .and_then(|value| value.as_array())
            .is_some_and(|calls| !calls.is_empty())
}

fn responses_response_started(value: &serde_json::Value) -> bool {
    matches!(
        value.get("type").and_then(|value| value.as_str()),
        Some(
            "response.output_text.delta"
                | "response.reasoning_summary_text.delta"
                | "response.output_item.done"
                | "response.completed"
                | "response.failed"
        )
    )
}

/// What a session says about the tools it runs, while it runs them.
///
/// The chunk callback carries the model's words and nothing else, so a caller
/// drawing a frame had no way to show that a `shell` call was in flight: a
/// two-minute test run was two minutes of a spinner over an empty transcript.
/// The call and its result are two events rather than one for exactly that
/// reason — the header goes up when the call starts, and the output fills in
/// when it returns.
#[derive(Debug, Clone)]
pub enum ToolEvent {
    /// A call the model made, as the arguments arrived on the wire.
    Started {
        call_id: String,
        name: String,
        /// The raw JSON string, not a re-encoding of it.
        arguments: String,
    },
    /// What the call answered, and whether it worked.
    Finished {
        call_id: String,
        name: String,
        output: String,
        is_error: bool,
        /// Whole milliseconds the call held the session. A record says what a
        /// run cost without anyone subtracting timestamps that collapse when
        /// several calls share one model step.
        duration_ms: u64,
    },
}

/// Who to tell about [`ToolEvent`]s.
///
/// `Arc` rather than `Box` because the session is moved into a task of its own
/// and the observer usually outlives the call that installed it.
pub type ToolObserver = Arc<dyn Fn(ToolEvent) + Send + Sync>;

pub const THREAD_LANE_NOTICE: &str = crate::surfaces::system_prompt::CODER_LANE_THREAD;

pub const LOCAL_LANE_NOTICE: &str = crate::surfaces::system_prompt::CODER_LANE_LOCAL_RUST;

/// Where an Ollama server listens unless `OPENAGENTS_OLLAMA_HOST` says otherwise.
pub const OLLAMA_HOST: &str = "http://127.0.0.1:11434";

/// How many rounds of tool calls one turn may take before it has to answer.
///
/// A backstop against a model that loops, not a budget.
const MAX_TOOL_STEPS: usize = 100;

/// Tool-call counts at which the countdown notice rides a tool result.
///
/// A model that learns its budget only by dying to it never gets to wrap up:
/// session `1a0434b26a4` burned two full turns to the cap and died mid-fix
/// with the work unreported (#188). The notice names the number left, so the
/// model can spend the last calls finishing rather than discovering.
const BUDGET_NOTICES: [usize; 4] = [50, 20, 5, 1];

/// The tool-call counts a finished turn reports, as `turn.budget` in the
/// transcript. Riding every turn's end keeps the cost of one turn visible
/// without anyone reconstructing it from per-call records.
const BUDGET_REPORT_AT: [usize; 3] = [25, 50, MAX_TOOL_STEPS];

/// How many transcript events one append may carry.
///
/// The server's own cap (`OpenAgents.Threads.maximum_event_batch/0`). A longer
/// list is split into several appends rather than refused as one.
const MAX_EVENT_BATCH: usize = 100;

/// One switchable lane: what a reader types, what the row calls it, and the
/// catalog ids it prefers in order.
///
/// `candidates` is a *preference*, never a pin. The ids are resolved against
/// `GET /api/v1/models` at thread open, so a lane whose first choice has left
/// the catalog opens on its next one instead of naming a model that is gone.
pub struct LaneSpec {
    /// What `--lane` takes for this lane.
    pub name: &'static str,
    /// What the row under the composer calls it.
    pub label: &'static str,
    /// Catalog ids in the order this lane would rather have them.
    pub candidates: &'static [&'static str],
}

/// The lanes shift+tab walks, in the order it walks them.
///
/// The table holds lane names and *preferences*, never a pinned id. The
/// previous table mapped each tier to one compiled catalog id, and two of the
/// three ended up naming models that had left the selectable list — the exact
/// failure that resolving against the live catalog prevents.
///
/// Nothing outside this table counts the lanes: [`Lane::cycle`] walks it and
/// [`admitted_lanes`] reads it. Restoring a retired lane — Pro, which the
/// owner pulled for now — is one entry here, one [`Lane`] variant, and one
/// [`Lane::from_str`] arm.
pub const LANES: &[LaneSpec] = &[
    LaneSpec {
        name: "flash",
        label: "Coder Flash",
        // The Vercel gateway lane: GLM 5.3 Flash, falling back to Gemini 3.7
        // Flash.
        //
        // Two spellings of the same model, because a catalog's client-facing
        // `id` and its gateway's vendor slug are not the same string and only
        // the deployment knows which it publishes — `ox-alpha` was the id for
        // `stealth/ox-alpha` on exactly this pattern. Listing both is not
        // guessing: an id the catalog does not serve never matches, so the
        // deployment decides and neither spelling can be pinned wrongly. The
        // OpenRouter spelling of this model (`z-ai/…`, hyphenated) is
        // deliberately absent — that gateway is Free's, and normalising the
        // two into one "canonical" spelling would be wrong on one of them.
        candidates: &[
            "glm-5.3-flash",
            "zai/glm-5.3-flash",
            "gemini-3.7-flash",
            "gpt-5.6-sol",
            "gpt-5.6-terra",
            "gpt-5.6-luna",
        ],
    },
    LaneSpec {
        name: "free",
        label: "Coder Free",
        // The OpenRouter lane. Inkling first, then whatever the deployment
        // publishes as its free lane.
        candidates: &["thinkingmachines/inkling", "openrouter/free"],
    },
    LaneSpec {
        name: "local",
        label: "Coder Local",
        // Ollama on this machine. `candidates` stays empty on purpose: the
        // local lane resolves against `GET /api/tags`, never the catalog, and
        // `Lane::resolve` already answers `Ok(None)` for it, so a catalog id
        // here would be a lie the resolution path never reads.
        //
        // The lane joins the shift+tab walk only when the open-time probe
        // found a server with models on it — see [`Lane::cycle_gated`] (#291).
        candidates: &[],
    },
];

/// The spec for a lane name, or `None` if nothing admits that name.
pub fn lane_spec(name: &str) -> Option<&'static LaneSpec> {
    LANES.iter().find(|lane| lane.name == name)
}

/// What `--lane` takes, for a refusal that leaves the reader somewhere to go.
///
/// Lane names only. The catalog ids behind them are deliberately absent: they
/// are resolved at open and printing a guess here is how a refusal comes to
/// recommend a model the deployment does not serve.
pub fn admitted_lanes() -> String {
    let lanes = LANES
        .iter()
        .map(|lane| lane.name)
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{lanes}, local or ollama:<model> for a model on this machine, \
         or any id `GET /api/v1/models` serves"
    )
}

/// Which lane a turn runs on.
///
/// [`Lane::from_str`] is total on purpose: an unrecognised name becomes
/// [`Lane::Named`] and is checked against the live catalog at the top of the
/// turn, so `--lane bogus` is refused by name with the list of what this
/// deployment serves. It used to fall through to `_ => Lane::OxAlpha`, which
/// ran the default lane while the reader believed they had chosen another.
/// `Auto`, `OxAlpha`, and `Pro` are gone on purpose, and the names `auto` and
/// `pro` are left unclaimed rather than aliased onto a surviving lane. `Auto`
/// meant "name no model and let the deployment decide"; if it returns it
/// should mean that again, and a convenience alias pointing it at Flash would
/// spend the identifier on something it does not mean.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum Lane {
    /// The Vercel gateway lane, and the lane a session opens on when none was
    /// named. Its ids come from [`LANES`], resolved against the catalog.
    #[default]
    Flash,
    /// The OpenRouter lane. Its ids come from [`LANES`] too.
    Free,
    /// A model id named directly, checked against `GET /api/v1/models`.
    Named(String),
    /// Ollama on this machine. An empty string means "whatever is installed".
    Local(String),
}

impl Lane {
    /// Named `from_str` rather than `FromStr::from_str` because it cannot
    /// fail: a name nothing admits is carried to the turn, where the catalog
    /// settles it and the refusal can name what this deployment serves.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        match s.trim().to_lowercase().as_str() {
            // Nothing named opens on the default lane. That is not the same
            // as the word "auto", which no longer names a lane and falls
            // through to `Named` below to be refused by the catalog.
            "" => Lane::default(),
            "flash" | "coder-flash" => Lane::Flash,
            "free" | "coder-free" => Lane::Free,
            // `gemini`, `gemini-3.7-flash`, `luna`, `gpt-5.6-luna`, `glm-5.3-flash`
            // and `pro` are deliberately absent. They used to be tier aliases,
            // and a reader who types a model id and receives a *different*
            // model because the tier behind the alias moved is the worst
            // outcome this enum can produce. They fall through to `Named`,
            // which pins exactly what was typed and earns a refusal by name
            // when the deployment does not serve it.
            "local" | "ollama" => Lane::Local(String::new()),
            other if other.starts_with("ollama:") => {
                Lane::Local(other.trim_start_matches("ollama:").trim().to_string())
            }
            other => Lane::Named(other.to_string()),
        }
    }

    /// The [`LaneSpec`] behind this lane, or `None` for one that names its own
    /// model.
    pub fn spec(&self) -> Option<&'static LaneSpec> {
        match self {
            Lane::Flash => lane_spec("flash"),
            Lane::Free => lane_spec("free"),
            // The bare local lane is a table member now (#291): it cycles, and
            // its label comes from the table. A local lane *with a named
            // model* is a pin, like `Named` — the reader asked for that exact
            // tag, and a keystroke does not throw the pin away.
            Lane::Local(model) if model.is_empty() => lane_spec("local"),
            Lane::Named(_) | Lane::Local(_) => None,
        }
    }

    /// The next lane shift+tab moves to.
    ///
    /// Walks [`LANES`] rather than toggling between two known variants, so a
    /// lane added back to that table joins the cycle without another edit
    /// here. A lane that is not in the table — a directly-named model — is
    /// left where it is: the reader pinned that on purpose and a keystroke
    /// should not throw it away.
    ///
    /// Gate-free, gate-open walk: every [`LANES`] member, with the local hop
    /// landing on the bare lane. The interactive TUI walks with
    /// [`Self::cycle_gated`] instead, where the open-time probe decides both
    /// whether `local` is offered and which tag it resolves to (#291, #292).
    pub fn cycle(&self) -> Lane {
        self.cycle_gated(Some(String::new()))
    }

    /// [`Self::cycle`] with the local lane gated on what the open-time probe
    /// found (issue #291, resolution refined by #292).
    ///
    /// `local_available` is the probe's answer. `None` skips `local`
    /// entirely — a lane the probe found absent is never landed on and then
    /// refused. `Some(tag)` walks the full table: flash, free, local, back
    /// to flash, and the local hop resolves to `Lane::Local(tag)` so the row
    /// names the exact model that will answer (e.g.
    /// `Coder Local (qwen3.8:27b-mtp-q8_0)`) rather than a lane-shaped
    /// promise. A lane already sitting on `local` with the gate closed moves
    /// to the hosted lanes; the reader asked to move, and cycling in place
    /// would land on a lane nothing serves. `Named` keeps its no-op: a
    /// directly pinned model is not a walk member.
    pub fn cycle_gated(&self, local_available: Option<String>) -> Lane {
        // Which table position this lane sits at. `Named` and a local lane
        // pinned to some other tag are not walk members: the reader named an
        // exact model, and a keystroke does not throw that away. The local
        // member is the bare lane or the lane carrying the probed tag — the
        // two states cycling itself produces, which are indistinguishable
        // from an explicit pin of the same tag and are allowed to move.
        let spec = match self {
            Lane::Named(_) => return self.clone(),
            Lane::Local(model) => {
                let is_member = match &local_available {
                    Some(tag) => model.is_empty() || model == tag,
                    None => model.is_empty(),
                };
                if !is_member {
                    return self.clone();
                }
                match lane_spec("local") {
                    Some(spec) => spec,
                    // Unreachable while the table has a `local` entry; a
                    // non-member return is the honest fallback.
                    None => return self.clone(),
                }
            }
            other => match other.spec() {
                Some(spec) => spec,
                None => return other.clone(),
            },
        };
        let mut at = LANES
            .iter()
            .position(|lane| lane.name == spec.name)
            .unwrap_or(0);
        for _ in 0..LANES.len() {
            at = (at + 1) % LANES.len();
            let name = LANES[at].name;
            match (name, &local_available) {
                ("local", None) => continue,
                ("local", Some(tag)) => return Lane::Local(tag.clone()),
                (_, _) => return Lane::from_str(name),
            }
        }
        // Unreachable while `flash` and `free` are never skipped; the default
        // is the honest answer if the table ever changes shape.
        Lane::default()
    }

    /// The catalog id to send at thread open, given what the deployment serves.
    ///
    /// `Ok(None)` means "send no model", which only the local lane wants —
    /// it names its model to Ollama and never reaches the server.
    ///
    /// A switchable lane takes the first of its [`LaneSpec::candidates`] the
    /// catalog both lists and reports available. Two outcomes, and they are
    /// not the same:
    ///
    /// - The primary is missing and a fallback is served. That is the designed
    ///   path, and it resolves quietly — but the caller records the id that
    ///   came back, so the row names the model that answered rather than the
    ///   lane that was asked for.
    /// - **Nothing the lane prefers is served.** Then it refuses, naming what
    ///   this deployment does serve. It does not pin a compiled default. A
    ///   lane that fell back to a constant here would be the failure this
    ///   whole table was rewritten to delete, and harder to see than the last
    ///   one, because the label would still look right.
    pub fn resolve(&self, served: &[ServedModel]) -> Result<Option<String>, Failure> {
        match self {
            Lane::Local(_) => Ok(None),
            Lane::Named(id) => Ok(Some(id.clone())),
            _ => {
                let spec = self
                    .spec()
                    .expect("a lane that is neither named nor local has a spec");
                let served_here = |id: &str| served.iter().any(|m| m.id == id && m.available);
                match spec.candidates.iter().find(|id| served_here(id)) {
                    Some(id) => Ok(Some((*id).to_string())),
                    None => Err(unresolved_lane(spec, served)),
                }
            }
        }
    }

    /// Whether this lane answers from this machine.
    pub fn is_local(&self) -> bool {
        matches!(self, Lane::Local(_))
    }

    /// The tier this lane belongs to: `auto`, `flash`, `pro`, or `local`.
    ///
    /// A model id no tier pins has no tier, which is a different answer from
    /// "auto" and is worth keeping separate — a reader who named a model
    /// directly did not ask for a tier.
    pub fn tier(&self) -> Option<&'static str> {
        match self {
            Lane::Local(_) => Some("local"),
            Lane::Named(_) => None,
            _ => self.spec().map(|spec| spec.name),
        }
    }

    /// The name for this lane on a status line.
    pub fn label(&self) -> String {
        match self {
            Lane::Local(model) if model.is_empty() => "Coder Local".to_string(),
            Lane::Local(model) => format!("Coder Local ({model})"),
            Lane::Named(id) => format!("Coder ({id})"),
            _ => self
                .spec()
                .map(|spec| spec.label.to_string())
                .unwrap_or_else(|| "Coder".to_string()),
        }
    }

    /// The stable command-line spelling stored with a local session.
    pub fn name(&self) -> String {
        match self {
            Lane::Flash => "flash".to_string(),
            Lane::Free => "free".to_string(),
            Lane::Named(id) => id.clone(),
            Lane::Local(model) if model.is_empty() => "local".to_string(),
            Lane::Local(model) => format!("ollama:{model}"),
        }
    }
}

/// The refusal for a lane none of whose candidates this deployment serves.
///
/// Shaped after the server's own `unadmitted_model/1`: it names what was
/// wanted, then interpolates what is actually served, so the reader is left
/// with something they can type rather than only the news that they cannot
/// type what they did.
fn unresolved_lane(spec: &LaneSpec, served: &[ServedModel]) -> Failure {
    let wanted = spec.candidates.join(", ");
    let usable: Vec<&str> = served
        .iter()
        .filter(|m| m.available)
        .map(|m| m.id.as_str())
        .collect();
    let alternatives = match usable.is_empty() {
        true => {
            "This deployment serves no model with a configured provider credential.".to_string()
        }
        false => format!("This deployment serves {}.", usable.join(", ")),
    };
    format!(
        "{} cannot open: it prefers {wanted}, and this deployment serves none of them. \
         {alternatives} Name one directly with --lane <id>, or use one of: {}.",
        spec.label,
        admitted_lanes()
    )
    .into()
}

/// As much of an error body as belongs in a one-line message.
fn snippet(body: &str) -> String {
    let body = body.trim();
    if body.chars().count() <= 200 {
        return body.to_string();
    }
    let head: String = body.chars().take(200).collect();
    format!("{head}…")
}

/// One model as `GET /api/v1/models` publishes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServedModel {
    pub id: String,
    /// Served here *and* its provider credential configured.
    pub available: bool,
    pub default: bool,
}

/// What one turn spent, as the server reported it.
///
/// Reported rather than estimated. The proxy sends a final chunk carrying
/// `usage` and Ollama sends its counts on the `done` line; both land here, and
/// a lane that reports nothing leaves this zero rather than guessing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUsage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

impl TurnUsage {
    pub fn add(&mut self, other: TurnUsage) {
        self.prompt_tokens += other.prompt_tokens;
        self.completion_tokens += other.completion_tokens;
        self.total_tokens += other.total_tokens;
    }

    pub fn reported(&self) -> bool {
        self.total_tokens > 0 || self.prompt_tokens > 0 || self.completion_tokens > 0
    }

    /// One line for a transcript or a status bar.
    pub fn line(&self) -> String {
        format!(
            "{} prompt + {} completion = {} tokens",
            self.prompt_tokens, self.completion_tokens, self.total_tokens
        )
    }
}

/// A grant the server issued. Never constructed from anything else.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InferenceGrant {
    pub thread_id: String,
    pub token: String,
    pub proxy_url: String,
    pub model: String,
}

/// One event on a thread's transcript, in the vocabulary `--resume` replays.
///
/// The words are the ones the server's own readers know: `turn.user`,
/// `turn.reasoning`, `tool.ran` and `turn.assistant` are what
/// [`crate::resume::replay_wire`] rebuilds a conversation from and what the
/// WEKA export cuts model calls at. `turn.failed` is deliberately outside that
/// set — a turn that did not answer has no answer to replay, and a reader that
/// does not know the word skips it rather than feeding a failure back to a
/// model as though the model had said it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ThreadRecord {
    pub event_type: String,
    pub payload: serde_json::Value,
}

impl ThreadRecord {
    fn new(event_type: &str, payload: serde_json::Value) -> Self {
        Self {
            event_type: event_type.to_string(),
            payload,
        }
    }

    /// What the reader asked.
    pub fn user(text: &str) -> Self {
        Self::user_with_images(text, &[])
    }

    /// What the reader asked, including inline images kept in local history.
    pub fn user_with_images(text: &str, images: &[ImageAttachment]) -> Self {
        let payload = if images.is_empty() {
            serde_json::json!({ "text": text })
        } else {
            serde_json::json!({
                "text": text,
                "images": images.iter().map(|image| &image.data_url).collect::<Vec<_>>(),
            })
        };
        Self::new("turn.user", payload)
    }

    /// What the model thought before it answered, recorded whole.
    pub fn reasoning(text: &str) -> Self {
        Self::new("turn.reasoning", serde_json::json!({ "text": text }))
    }

    /// One call with its arguments and its result: one fact, one event.
    ///
    /// `arguments` is the raw JSON string the wire carried, not a re-encoding
    /// of it, because that is what goes back to a model on replay.
    pub fn tool_ran(call_id: &str, tool: &str, arguments: &str, output: &str) -> Self {
        Self::tool_ran_on(call_id, tool, arguments, output, 0)
    }

    /// [`Self::tool_ran`] with what the call cost in whole milliseconds.
    ///
    /// Recorded on the event rather than reconstructed later from envelope
    /// timestamps, which collapse when several calls share one model step.
    pub fn tool_ran_on(
        call_id: &str,
        tool: &str,
        arguments: &str,
        output: &str,
        duration_ms: u64,
    ) -> Self {
        Self::new(
            "tool.ran",
            serde_json::json!({
                "call_id": call_id,
                "tool": tool,
                "arguments": arguments,
                "output": output,
                "duration_ms": duration_ms,
            }),
        )
    }

    /// The answer, with what the turn spent reaching it.
    pub fn assistant(text: &str, usage: TurnUsage, calls: usize) -> Self {
        Self::assistant_on(text, usage, calls, None)
    }

    /// The answer, including the exact model that produced it when known.
    pub fn assistant_on(text: &str, usage: TurnUsage, calls: usize, model: Option<&str>) -> Self {
        Self::new(
            "turn.assistant",
            serde_json::json!({
                "text": text,
                "usage": usage,
                "calls": calls,
                "model": model,
            }),
        )
    }

    /// A turn that produced no answer, and the sentence saying why.
    ///
    /// This is the half of the record that keeps it honest. A session that
    /// failed must not read afterwards as one that succeeded, so every exit
    /// from a turn that is not the model's own answer writes one of these and
    /// no `turn.assistant`.
    pub fn failed(why: &str, usage: TurnUsage, calls: usize) -> Self {
        Self::new(
            "turn.failed",
            serde_json::json!({
                "error": why,
                "usage": usage,
                "calls": calls,
            }),
        )
    }

    /// The turn reached its tool-call budget. Recorded as its own fact, not a
    /// failure: the turn goes on to be finalized, and the transcript should
    /// show the moment the budget bit (#188).
    pub fn budget_reached(usage: TurnUsage, calls: usize) -> Self {
        Self::new(
            "turn.budget",
            serde_json::json!({
                "phase": "reached",
                "calls": calls,
                "usage": usage,
            }),
        )
    }

    /// The finish-and-report instruction the budget finalization sent, so the
    /// next turn can see the model was asked for a summary and find it.
    pub fn budget_instruction() -> Self {
        Self::new(
            "turn.budget",
            serde_json::json!({
                "phase": "finalization_prompted",
            }),
        )
    }

    /// What a turn spent, recorded when it crosses a report line. The same
    /// counters `turn.failed` carries, attached to a turn that is still going.
    pub fn budget_report(usage: TurnUsage, calls: usize) -> Self {
        Self::new(
            "turn.budget",
            serde_json::json!({
                "phase": "spent",
                "calls": calls,
                "usage": usage,
            }),
        )
    }

    /// The model's own milestone note: what landed, what is broken, next step
    /// (#189). A turn that dies — to the budget, to a crash, to a cancel —
    /// leaves this behind for whoever picks the work up.
    pub fn checkpoint(text: &str) -> Self {
        Self::new("turn.checkpoint", serde_json::json!({ "text": text }))
    }

    /// One swarm send or receive, attributed to the sessions that talked.
    /// Never a user turn: the ATIF exporter and resume skip this as speech
    /// and carry it as a tool observation when the TUI drew one.
    pub fn swarm_message(direction: &str, message: &crate::swarm::SwarmMessage) -> Self {
        Self::new(
            "swarm_message",
            serde_json::json!({
                "direction": direction,
                "id": message.id,
                "from": message.from,
                "to": message.to,
                "kind": message.kind,
                "thread": message.thread,
                "reply_expected": message.reply_expected,
                "reply_depth": message.reply_depth,
                "body": message.body,
                "sequence": message.sequence,
            }),
        )
    }
}

/// The three ways a thread may end, as the server spells them.
pub const SUCCEEDED: &str = "succeeded";
pub const FAILED: &str = "failed";
pub const CANCELLED: &str = "cancelled";

/// The error codes this CLI files, one per way a turn can fail to answer.
///
/// `&'static str` throughout, so a code is one of these and not a sentence
/// somebody assembled at a call site: `GET /api/v1/threads` groups on this
/// field, and a code built from an error message would make every failure its
/// own category.
pub mod error_code {
    /// The proxy refused the turn or could not be reached.
    pub const PROVIDER_FAILED: &str = "provider_failed";
    /// The reply stopped part way through, so there is no whole answer.
    pub const STREAM_BROKEN: &str = "stream_broken";
    /// The turn spent its whole tool-step budget without answering.
    pub const MAX_STEPS: &str = "max_steps";
    /// The session was stopped before its turn finished. Ctrl-C is this.
    pub const INTERRUPTED: &str = "interrupted";
    /// A turn failed some other way, named in the report rather than the code.
    pub const TURN_FAILED: &str = "turn_failed";
    /// The session held a thread but never ran a turn on it.
    pub const NO_TURN: &str = "no_turn";
}

/// The largest report the server takes, `OpenAgents.Threads.Thread`'s
/// `@objective_bytes`. A longer one is refused whole, so it is cut here.
const MAX_REPORT_BYTES: usize = 32_768;

/// How a session ended, in the vocabulary `POST /threads/{id}/report` takes.
///
/// The status and the error code are one decision, not two, and the
/// constructors are the only way to make either: `succeeded` carries no code
/// and every other end has to name one. That is not politeness towards the
/// server's validation — it is the same rule as [`ThreadRecord::failed`],
/// pointed at the thread's permanent report instead of its transcript. A run
/// that failed, was interrupted, or exhausted its steps and is filed as a
/// success would be a worse record than the wall of cancellations this
/// replaces, because a reader can tell a cancellation is uninformative and
/// cannot tell a false success from a true one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadOutcome {
    status: &'static str,
    error_code: Option<&'static str>,
    report: String,
}

impl ThreadOutcome {
    /// The session answered. No error code, because there was no error.
    pub fn succeeded(report: &str) -> Self {
        Self {
            status: SUCCEEDED,
            error_code: None,
            report: bounded_report(report, "The session answered."),
        }
    }

    /// The session did not answer, and `code` says which way.
    pub fn failed(code: &'static str, report: &str) -> Self {
        Self {
            status: FAILED,
            error_code: Some(code),
            report: bounded_report(report, "The turn failed without saying why."),
        }
    }

    /// The session was stopped before the turn in flight could finish.
    ///
    /// `cancelled` rather than `failed`: nothing was wrong with the work, a
    /// reader ended it. The server treats a cancelled thread as disposed of and
    /// refuses to re-mint authority on it, which is the right answer for a
    /// thread somebody stopped on purpose.
    pub fn interrupted(report: &str) -> Self {
        Self {
            status: CANCELLED,
            error_code: Some(error_code::INTERRUPTED),
            report: bounded_report(report, "The session was stopped before the turn finished."),
        }
    }

    /// The session held a thread and never ran a turn on it.
    ///
    /// Not a success: nothing was answered. Not a cancellation either, because
    /// nobody asked for the thread to be over and a thread that reports stays
    /// resumable.
    pub fn no_turn() -> Self {
        Self::failed(
            error_code::NO_TURN,
            "The session ended without running a turn on this thread.",
        )
    }

    pub fn status(&self) -> &str {
        self.status
    }

    /// `None` exactly when the session succeeded.
    pub fn error_code(&self) -> Option<&str> {
        self.error_code
    }

    pub fn report(&self) -> &str {
        &self.report
    }

    /// Keep settlement state while removing conversation-derived text.
    fn without_conversation(mut self) -> Self {
        self.report = match self.status {
            SUCCEEDED => "The session answered.".to_string(),
            CANCELLED => "The session was stopped before the turn finished.".to_string(),
            _ => "The session ended without an answer.".to_string(),
        };
        self
    }

    /// The body `POST /threads/{id}/report` takes.
    ///
    /// `usage` is what this process counted, sent as the session's own figure
    /// and labelled as such: the account is charged against the grant's spend,
    /// which the server already holds and this call reads back.
    fn wire(&self, usage: TurnUsage) -> serde_json::Value {
        let mut body = serde_json::json!({
            "status": self.status,
            "report": self.report,
            "usage": {
                "prompt_tokens": usage.prompt_tokens,
                "completion_tokens": usage.completion_tokens,
                "total_tokens": usage.total_tokens,
                "counted_by": "client",
            },
        });
        if let Some(code) = self.error_code {
            body["error_code"] = serde_json::json!(code);
        }
        body
    }
}

/// A report the server will take: never blank, never over the bound.
///
/// A model that answered with nothing still ended a session, and a blank
/// report is refused — so the stand-in says what happened rather than letting
/// the whole report fail over an empty answer.
fn bounded_report(text: &str, if_blank: &str) -> String {
    let text = text.trim();
    if text.is_empty() {
        return if_blank.to_string();
    }
    if text.len() <= MAX_REPORT_BYTES {
        return text.to_string();
    }
    // On a character boundary, and with the cut named: a report that stops mid
    // sentence should say that it was cut rather than look like the whole of
    // what the session said.
    const NOTE: &str = "\n[report truncated]";
    let mut end = MAX_REPORT_BYTES - NOTE.len();
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}{NOTE}", &text[..end])
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Data URLs attached to a user message.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<String>,
}

/// One image the composer sends as multimodal user content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageAttachment {
    pub id: usize,
    pub filename: String,
    pub mime_type: String,
    pub data_url: String,
}

pub struct CoderRuntimeSession {
    pub lane: Lane,
    /// Whether turns talk to the OpenResponses streaming surface instead of
    /// opening a thread. `Lane` still names the model; this flag only picks the
    /// transport.
    pub use_openresponses: bool,
    /// The grant this session opened, reused across its turns.
    ///
    /// `None` on the local lane, always: there is no grant to hold, and
    /// putting a made-up one here is the bug this file exists to keep dead.
    pub last_grant: Option<InferenceGrant>,
    /// The model that actually answered the last turn.
    ///
    /// From the grant on the thread lane, and from the resolved Ollama name on
    /// the local lane. A caller reporting what answered should read this
    /// rather than the lane, which is only what was asked for.
    pub last_model: Option<String>,
    /// What the last turn spent, summed over its steps.
    pub last_usage: TurnUsage,
    /// How many tools the last turn ran, counted as they were recorded.
    pub last_calls: usize,
    /// What every turn this session ran has spent, summed.
    ///
    /// The figure to hold a server-reported grant spend against: `last_usage`
    /// is one turn, and a session that ran four of them and compared the
    /// fourth against the grant's total would report a divergence on every
    /// multi-turn session and call it a mismatch.
    pub session_usage: TurnUsage,
    /// Transcript appends this session could not make, in the order they failed.
    ///
    /// Recording is best effort — a thread the server would not take an event
    /// for is not a reason to throw away an answer the reader is waiting on —
    /// but a silent best effort is how the record came to disagree with the
    /// session in the first place. The failure is kept here and reported.
    pub record_failures: Vec<String>,
    /// The reasoning the last turn emitted, if the model emits any.
    ///
    /// Kept off the content callback deliberately: `delta.reasoning` and
    /// `delta.content` interleave on the wire, and appending both to the
    /// transcript would put the model's scratch work in the middle of its
    /// answer. It is parsed, summed and kept here so a caller that wants to
    /// show it can, and the transcript stays the answer.
    pub last_reasoning: String,
    /// The effort recorded on the thread as its admitted execution shape.
    ///
    /// `--reasoning`. Sent as the thread's `reasoning` at open, which is the
    /// only place the server takes it: `GET /api/v1/threads` reports it back
    /// as `reasoning_effort`, and the proxy reads it from the thread rather
    /// than from each request. `None` leaves the deployment's own default.
    pub reasoning: Option<String>,
    /// `options.num_ctx` for the local lane, when the reader set one
    /// (`--num-ctx` / `OPENAGENTS_OLLAMA_NUM_CTX`, issue #293). `None` sends
    /// nothing and leaves Ollama's default resolution standing.
    pub ollama_num_ctx: Option<u32>,
    /// The repository this session was opened from, as `owner/name`.
    ///
    /// Recorded on the thread so `--resume` has something to filter on. A
    /// thread with no repository is not attributable to a checkout and shows
    /// up only under `--resume --all`.
    pub repository: Option<String>,
    pub api_base: String,
    pub user_token: Option<String>,
    pub ollama_host: String,
    pub http: reqwest::Client,
    pub tools: HarnessToolRegistry,
    pub messages: Vec<ChatMessage>,
    /// The local append-only record. It is Coder's source of truth and never
    /// performs network I/O.
    local_session: Option<crate::session_store::LocalSessionStore>,
    /// Whether Coder may upload transcript events and outcome text.
    cloud_history: bool,
    /// Told about every tool this session runs, as it runs it.
    ///
    /// `None` by default: a caller that does not draw a frame has nothing to
    /// do with the events, and the turn behaves exactly as it did before.
    pub tool_observer: Option<ToolObserver>,
    /// Told when a model has not begun its response within the watchdog window.
    pub progress_observer: Option<TurnProgressObserver>,
    /// Told about model text and reasoning as each wire delta arrives.
    ///
    /// The existing answer callback remains a committed-answer interface for
    /// callers that cannot retract provisional content. A live frame can use
    /// this richer observer to draw the stream immediately and remove a model
    /// preamble when the same response proceeds to a tool call.
    pub stream_observer: Option<ModelStreamObserver>,
    /// The current turn's cancellation signal, cloned into every tool call.
    tool_cancellation: Option<tokio::sync::watch::Receiver<bool>>,
    first_response: FirstResponsePolicy,
    /// The thread to revoke when the session closes.
    thread_id: Option<String>,
    /// What this session would report if it ended now.
    ///
    /// Written in exactly one place — the end of [`Self::execute_turn`], from
    /// that turn's own `Result` — and by [`Self::note_interruption`] for a turn
    /// that never got to return. `None` means no turn has run, which is a
    /// different thing from a turn that ran and failed.
    outcome: Option<ThreadOutcome>,
    /// The failure the turn in flight already wrote down, with its code.
    ///
    /// [`Self::record_failure`] knows which way a turn failed; the `Err` that
    /// comes back up the stack is only a sentence. This carries the code from
    /// one to the other so the reported outcome is specific rather than
    /// `turn_failed` for everything. Cleared at the top of every turn.
    pending_failure: Option<ThreadOutcome>,
    /// This session's swarm identity. Set when the session registers; drain
    /// and the swarm tools are no-ops without it.
    swarm: Option<crate::swarm::SwarmBinding>,
}

impl CoderRuntimeSession {
    pub fn new(
        lane: Lane,
        api_base: Option<String>,
        user_token: Option<String>,
        tools: HarnessToolRegistry,
    ) -> Self {
        Self {
            lane,
            use_openresponses: false,
            last_grant: None,
            last_model: None,
            last_usage: TurnUsage::default(),
            last_calls: 0,
            session_usage: TurnUsage::default(),
            record_failures: Vec::new(),
            last_reasoning: String::new(),
            reasoning: None,
            ollama_num_ctx: std::env::var("OPENAGENTS_OLLAMA_NUM_CTX")
                .ok()
                .and_then(|v| v.trim().parse::<u32>().ok()),
            repository: None,
            // `OPENAGENTS_API_BASE` points the session at another host. A test
            // that has to prove the streaming path end to end needs somewhere
            // to point it that is not production, and an operator on staging
            // needs the same switch.
            api_base: api_base
                .or_else(|| {
                    std::env::var("OPENAGENTS_API_BASE")
                        .ok()
                        .filter(|v| !v.trim().is_empty())
                })
                .unwrap_or_else(|| "https://openagents.com/api/v1".to_string()),
            user_token,
            ollama_host: std::env::var("OPENAGENTS_OLLAMA_HOST")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .unwrap_or_else(|| OLLAMA_HOST.to_string()),
            http: reqwest::Client::builder()
                .timeout(Duration::from_secs(300))
                .build()
                .unwrap_or_default(),
            tools,
            messages: Vec::new(),
            local_session: None,
            // Headless and delegated callers may have no local store. Keep
            // their existing server record unless the interactive Coder front
            // door explicitly selects local-only history.
            cloud_history: true,
            tool_observer: None,
            progress_observer: None,
            stream_observer: None,
            tool_cancellation: None,
            first_response: FirstResponsePolicy::default(),
            thread_id: None,
            outcome: None,
            pending_failure: None,
            swarm: None,
        }
    }

    /// Attach this session's swarm identity so turn-boundary drain and the
    /// `swarm_*` tools share one budget and one mute list.
    pub fn bind_swarm(&mut self, binding: crate::swarm::SwarmBinding) {
        self.tools.bind_swarm(binding.clone());
        self.swarm = Some(binding);
    }

    pub fn with_swarm(mut self, binding: crate::swarm::SwarmBinding) -> Self {
        self.bind_swarm(binding);
        self
    }

    /// Drain new inbox messages into the tool stream. Called before every
    /// model call. Injected entries are `role: tool` with name `swarm.inbox`;
    /// they are never user speech, even when a neighbor's body looks like it.
    pub async fn drain_swarm_inbox(&mut self) {
        let Some(binding) = self.swarm.clone() else {
            return;
        };
        let _ = crate::swarm::heartbeat(&binding.home, &binding.session_id);
        let plan = match crate::swarm::drain_turn(&binding) {
            Ok(plan) => plan,
            Err(why) => {
                self.record_failures
                    .push(format!("swarm inbox could not be drained: {why}"));
                return;
            }
        };
        if plan.inject.is_empty() {
            return;
        }
        let call_id = format!(
            "swarm-inbox-{}",
            plan.inject
                .last()
                .and_then(|message| message.sequence)
                .unwrap_or_default()
        );
        let first = plan.inject.first();
        let arguments = serde_json::json!({
            "from": first.map(|message| message.from.as_str()),
            "kind": first.map(|message| message.kind.as_str()),
            "count": plan.inject.len(),
            "deferred": plan.deferred,
        })
        .to_string();
        let output = serde_json::json!({
            "schema": "openagents.swarm.inbox.v1",
            "source": "turn_boundary",
            "deferred": plan.deferred,
            "muted": plan.muted.len(),
            "messages": plan
                .inject
                .iter()
                .map(crate::swarm::message_document)
                .collect::<Vec<_>>(),
        })
        .to_string();
        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![serde_json::json!({
                "id": call_id,
                "type": "function",
                "function": {
                    "name": crate::swarm::INBOX_TOOL,
                    "arguments": arguments,
                }
            })]),
            tool_call_id: None,
            images: Vec::new(),
        });
        self.messages.push(ChatMessage {
            role: "tool".to_string(),
            content: Some(output.clone()),
            tool_calls: None,
            tool_call_id: Some(call_id.clone()),
            images: Vec::new(),
        });
        self.tell(ToolEvent::Started {
            call_id: call_id.clone(),
            name: crate::swarm::INBOX_TOOL.to_string(),
            arguments: arguments.clone(),
        });
        self.tell(ToolEvent::Finished {
            call_id: call_id.clone(),
            name: crate::swarm::INBOX_TOOL.to_string(),
            output: output.clone(),
            is_error: false,
            duration_ms: 0,
        });
        let mut records = vec![ThreadRecord::tool_ran_on(
            &call_id,
            crate::swarm::INBOX_TOOL,
            &arguments,
            &output,
            0,
        )];
        for message in &plan.inject {
            records.push(ThreadRecord::swarm_message("received", message));
        }
        self.note(records).await;
    }

    /// Report every tool this session runs to `observer`.
    pub fn observing_tools(mut self, observer: ToolObserver) -> Self {
        self.tool_observer = Some(observer);
        self
    }

    /// Attach the local source-of-truth session and its model-facing replay.
    pub fn with_local_session(
        mut self,
        store: crate::session_store::LocalSessionStore,
        replayed: Vec<ChatMessage>,
    ) -> Self {
        if !replayed.is_empty() {
            let system = self
                .messages
                .first()
                .filter(|message| message.role == "system")
                .cloned();
            self.messages.clear();
            if let Some(system) = system {
                self.messages.push(system);
            }
            self.messages.extend(replayed);
        }
        self.local_session = Some(store);
        self
    }

    /// Opt in to durable server transcript storage.
    pub fn with_cloud_history(mut self, enabled: bool) -> Self {
        self.cloud_history = enabled;
        self
    }

    /// Set the Ollama request controls the local lane sends (issue #293).
    ///
    /// `num_ctx` rides every local chat request as `options.num_ctx`, so a
    /// long session is not silently truncated to the server's default
    /// window. `think` is derived from `--reasoning` at the call sites
    /// (minimal/low → false, medium/high/max → true); the session carries
    /// the effort, the mapping lives with the wire shape.
    pub fn with_ollama_options(mut self, num_ctx: Option<u32>) -> Self {
        self.ollama_num_ctx = num_ctx;
        self
    }

    /// The `think` value the local lane sends for this session's reasoning
    /// effort, or `None` to send nothing and leave Ollama's own default.
    ///
    /// A thinking-default model (Qwen 3.8 thinks, at `xhigh` by default)
    /// spends context on scratch the harness never asked for; `--reasoning
    /// low` on a local session should mean the same kind of restraint it
    /// means on a thread. `None` stays `None`: an unset effort is the
    /// deployment's default, not a lie about one.
    pub fn ollama_think(&self) -> Option<bool> {
        match self.reasoning.as_deref() {
            Some("minimal") | Some("low") => Some(false),
            Some("medium") | Some("high") | Some("max") => Some(true),
            _ => None,
        }
    }

    pub fn local_session_summary(&self) -> Option<&crate::session_store::SessionSummary> {
        self.local_session.as_ref().map(|store| store.summary())
    }

    /// Report first-response waiting and retry state to a caller that draws it.
    pub fn observing_progress(mut self, observer: TurnProgressObserver) -> Self {
        self.progress_observer = Some(observer);
        self
    }

    /// Report live response and reasoning deltas to a caller that can draw
    /// provisional content and retract it when a tool call follows.
    pub fn observing_stream(mut self, observer: ModelStreamObserver) -> Self {
        self.stream_observer = Some(observer);
        self
    }

    /// Change the first-response watchdog on an existing session.
    #[doc(hidden)]
    pub fn set_first_response_policy(&mut self, waiting_after: Duration, timeout_after: Duration) {
        assert!(waiting_after < timeout_after);
        self.first_response = FirstResponsePolicy {
            waiting_after,
            timeout_after,
        };
    }

    /// Replace the cancellation signal used by tools in the next turn.
    pub fn set_tool_cancellation(&mut self, cancellation: tokio::sync::watch::Receiver<bool>) {
        self.tool_cancellation = Some(cancellation);
    }

    /// Use the OpenResponses streaming surface for this session's turns.
    pub fn use_openresponses(mut self, yes: bool) -> Self {
        self.use_openresponses = yes;
        self
    }

    fn tell(&self, event: ToolEvent) {
        if let Some(observer) = &self.tool_observer {
            observer(event);
        }
    }

    fn tell_progress(&self, progress: TurnProgress) {
        if let Some(observer) = &self.progress_observer {
            observer(progress);
        }
    }

    fn tell_stream(&self, event: ModelStreamEvent) {
        if let Some(observer) = &self.stream_observer {
            observer(event);
        }
    }

    /// Deliver one provider text delta at a readable live cadence.
    ///
    /// Some providers stream a whole paragraph as one delta. Passing that
    /// value straight through technically uses SSE but still paints as one
    /// completed block. Only an attached live observer gets these paced
    /// pieces; the committed-answer callback and headless callers keep the
    /// provider's original chunk and incur no delay.
    async fn tell_content_delta(&self, delta: String) {
        const CHARS_PER_PIECE: usize = 8;
        const PIECE_DELAY: Duration = Duration::from_millis(20);

        let Some(observer) = self.stream_observer.clone() else {
            return;
        };
        let chars = delta.chars().collect::<Vec<_>>();
        let pieces = chars.chunks(CHARS_PER_PIECE).collect::<Vec<_>>();
        let last = pieces.len().saturating_sub(1);
        for (index, piece) in pieces.into_iter().enumerate() {
            observer(ModelStreamEvent::ContentDelta(piece.iter().collect()));
            if index < last {
                tokio::time::sleep(PIECE_DELAY).await;
            }
        }
    }

    pub fn build_system_prompt(&self, tool_defs: &[ToolDefinition]) -> String {
        let notice = if self.lane.is_local() {
            LOCAL_LANE_NOTICE
        } else {
            THREAD_LANE_NOTICE
        };
        use crate::surfaces::system_prompt as prompt;
        let mut lines = vec![
            prompt::CODER_OPENING.replace("{lane}", notice),
            "".to_string(),
            prompt::CODER_CONCISION.to_string(),
            "".to_string(),
            prompt::CODER_VERIFICATION.to_string(),
            "".to_string(),
            format!(
                "The session's working directory is `{}`. This is a runtime fact: when asked \
                 where you are working, state this path and do not invent another one. Use the \
                 `shell` tool with `pwd` when you need to verify it.",
                self.tools.cwd.display()
            ),
            "".to_string(),
            "When you call a tool, wait for its result before giving the reader a final answer. \
             Use the next model round to synthesize the result. Text from a tool-call round is \
             withheld from the reader."
                .to_string(),
            "".to_string(),
            prompt::CODER_BUDGET.to_string(),
            "".to_string(),
            prompt::CODER_CHECKPOINTS.to_string(),
            "".to_string(),
        ];

        if tool_defs.is_empty() {
            lines.push(prompt::CODER_NO_TOOLS.to_string());
        } else {
            lines.push(
                prompt::CODER_TOOL_LIST_HEADER_RUST
                    .replace("{count}", &tool_defs.len().to_string()),
            );
            for t in tool_defs {
                lines.push(format!("- `{}`", t.name));
            }
            lines.push("".to_string());
            lines.push(prompt::CODER_TOOL_LIST_CLOSING.to_string());
        }

        // Skill injection. The `skill` tool's catalog is names and
        // descriptions only, so a body costs nothing until it is asked for.
        // A skill marked `auto: true` in its front matter is the exception:
        // it says how to approach the work, and a session needs the method
        // before its first decision rather than after thinking to ask.
        if let Some(context) = self.tools.standing_context() {
            lines.push("".to_string());
            lines.push(context);
        }

        lines.join("\n")
    }

    // ───────────────────────────────────────────────────────── the catalog

    /// What this deployment serves, read from the server rather than assumed.
    pub async fn served_models(&self) -> Result<Vec<ServedModel>, Failure> {
        let url = format!("{}/models", self.api_base);
        let mut request = self.http.get(&url).timeout(Duration::from_secs(15));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url} refused the catalog request: {status} {}",
                snippet(&body)
            )
            .into());
        }
        let body: serde_json::Value = resp.json().await?;
        let models = body
            .get("models")
            .and_then(|v| v.as_array())
            .ok_or_else(|| -> Failure { format!("{url} published no model list").into() })?;
        Ok(models
            .iter()
            .filter_map(|model| {
                let id = model.get("id").and_then(|v| v.as_str())?;
                Some(ServedModel {
                    id: id.to_string(),
                    // Any word other than `available` is read as unavailable: a
                    // vocabulary this client has not seen is a reason to pick
                    // another model, not to assume the new word is benign.
                    available: model.get("availability").and_then(|v| v.as_str())
                        == Some("available"),
                    default: model.get("default").and_then(|v| v.as_bool()) == Some(true),
                })
            })
            .collect())
    }

    /// Refuse a directly-named model the catalog cannot answer with.
    ///
    /// The tiers skip this: they are checked at open, and the server's own 422
    /// carries the news if one is retired. Only a name this crate does not
    /// recognise pays for the round trip, and it pays it so that `--lane
    /// bogus` earns a sentence naming what it could have said instead.
    async fn check_named(&self, id: &str) -> Result<(), Failure> {
        let served = match self.served_models().await {
            Ok(served) => served,
            Err(error) => {
                return Err(format!(
                    "'{id}' is not a lane this CLI knows, and the catalog that would settle it \
                     could not be read: {error}. Lanes: {}.",
                    admitted_lanes()
                )
                .into());
            }
        };
        let usable: Vec<&str> = served
            .iter()
            .filter(|m| m.available)
            .map(|m| m.id.as_str())
            .collect();
        let alternatives = if usable.is_empty() {
            "This deployment has no model with a configured provider credential.".to_string()
        } else {
            format!("This deployment serves {}.", usable.join(", "))
        };

        match served.iter().find(|m| m.id == id) {
            None => Err(format!(
                "'{id}' is not a lane this CLI knows and no model of that name is served here. \
                 {alternatives} Lanes: {}.",
                admitted_lanes()
            )
            .into()),
            Some(model) if !model.available => Err(format!(
                "'{id}' is in the catalog but its provider is not configured on this deployment. \
                 {alternatives}"
            )
            .into()),
            Some(_) => Ok(()),
        }
    }

    /// The thread this session opened, while it still holds one.
    pub fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    /// Refuse a directly-named model before a child or worktree is created.
    pub async fn ensure_named_served(&self) -> Result<(), Failure> {
        if let Lane::Named(id) = self.lane.clone() {
            self.check_named(&id).await
        } else {
            Ok(())
        }
    }

    /// Move this session onto another lane.
    ///
    /// The thread it was holding is dropped rather than reused. A thread's
    /// grant pins one model for its whole life, so a turn run on the old
    /// thread would be answered by the old lane's model while the row named
    /// the new one — the precise lie the row exists to prevent. The next turn
    /// opens its own thread on the new lane.
    ///
    /// `last_model` is cleared with it: what answered belonged to the lane
    /// that has just been left, and carrying it forward would attribute a
    /// model to a lane that has not yet run a turn.
    pub fn set_lane(&mut self, lane: Lane) {
        self.lane = lane;
        self.thread_id = None;
        self.last_model = None;
        if let Some(store) = self.local_session.as_mut() {
            let _ = store.set_lane(&self.lane.name());
        }
    }

    /// The model id this session's lane opens on, read from the live catalog.
    ///
    /// A lane that names its own model (`Named`) or names none (`Local`) never
    /// pays for the round trip; only a switchable lane does, and it pays it so
    /// that the id it opens on is one the deployment is serving *now* rather
    /// than one that was true when this crate was compiled.
    ///
    /// A catalog that cannot be read is a refusal, not a licence to guess. The
    /// alternative — pinning a compiled id when the server is unreachable — is
    /// how a lane comes to open on a model that left the list months ago.
    async fn lane_model(&self) -> Result<Option<String>, Failure> {
        if self.lane.spec().is_none() {
            return self.lane.resolve(&[]);
        }
        let served = self.served_models().await.map_err(|error| -> Failure {
            format!(
                "{} could not be opened: the catalog that says which model it runs \
                 could not be read: {error}",
                self.lane.label()
            )
            .into()
        })?;
        self.lane.resolve(&served)
    }

    // ────────────────────────────────────────────────── threads and grants

    pub async fn create_thread(&self) -> Result<InferenceGrant, Failure> {
        let url = format!("{}/threads", self.api_base);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(tok) = &self.user_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {tok}"))?,
            );
        }

        // `lane` is the thread's execution shape and the server admits only
        // `thread` and `local`; this path is the proxy, which is `thread`. It
        // used to send a model name here and every request was refused with
        // `"glm-5.3-flash" is not an admitted lane` — invisibly, because the caller
        // answered the refusal with a fabricated grant.
        //
        // `model` is separate and optional. Omitting it opens on the
        // deployment's own default, which is what `Lane::Auto` wants; naming
        // one pins the lane. Either way the grant that comes back is the
        // authority on which model answers, and that is what gets reported.
        let mut body = serde_json::json!({
            "objective": "Coding assistant session",
            "lane": "thread",
        });
        if let Some(model) = self.lane_model().await? {
            body["model"] = serde_json::json!(model);
        }
        // `--reasoning`. The thread carries the effort; the proxy reads it from
        // there. Omitted, the deployment's own default stands, which is a
        // different answer from naming one and worth keeping separate.
        if let Some(effort) = &self.reasoning {
            body["reasoning"] = serde_json::json!(effort);
        }
        // What `--resume` filters on. A thread with no repository is not
        // attributable to a checkout.
        if let Some(repository) = &self.repository {
            body["repository"] = serde_json::json!(repository);
        }

        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await?;

        if !resp.status().is_success() {
            // This used to invent a grant with a placeholder token and carry
            // on, so a refused request reached the reader as a completed turn.
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url} refused the thread request: {status} {}",
                snippet(&text)
            )
            .into());
        }

        let body: serde_json::Value = resp.json().await?;
        let thread = body.get("thread").cloned().unwrap_or(serde_json::json!({}));
        let grant = body.get("grant").cloned().unwrap_or(serde_json::json!({}));

        let thread_id = thread
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("{url} accepted the thread but published no id").into()
            })?
            .to_string();
        let token = grant
            .get("token")
            .and_then(|v| v.as_str())
            .filter(|t| !t.is_empty())
            .ok_or_else(|| -> Failure {
                format!(
                    "{url} opened thread {thread_id} but minted no inference grant, \
                     so there is no token to call the proxy with"
                )
                .into()
            })?
            .to_string();
        let proxy_url = grant
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("the grant on thread {thread_id} names no proxy url").into()
            })?
            .to_string();
        let model = grant
            .get("model")
            .and_then(|v| v.as_str())
            .ok_or_else(|| -> Failure {
                format!("the grant on thread {thread_id} names no model").into()
            })?
            .to_string();

        Ok(InferenceGrant {
            thread_id,
            token,
            proxy_url,
            model,
        })
    }

    /// Continue an existing thread by asking the server to re-mint its
    /// authority.
    ///
    /// `POST /api/v1/threads/{id}/grants` is the server's resume fence: it
    /// revokes every active grant naming the thread, bumps the thread's
    /// generation, and mints fresh authority against the same thread. That is
    /// the only honest way to spend a thread that already exists — the grant's
    /// plaintext token exists exactly once, at minting, so `GET
    /// /api/v1/threads/{id}` reports the grant's status and never its token.
    ///
    /// After this the session holds the resumed thread the way it would hold
    /// one it opened: [`Self::close`] revokes it and turns spend against it.
    pub async fn adopt_thread(&mut self, thread_id: &str) -> Result<InferenceGrant, Failure> {
        let url = format!("{}/threads/{thread_id}/grants", self.api_base);
        let mut request = self.http.post(&url).json(&serde_json::json!({}));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            return Err(format!(
                "{url} refused to re-mint the thread's grant: {status} {}",
                snippet(&text)
            )
            .into());
        }
        let body: serde_json::Value = resp.json().await?;
        let grant = body.get("grant").cloned().unwrap_or(serde_json::json!({}));
        let field = |name: &str| -> Result<String, Failure> {
            grant
                .get(name)
                .and_then(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(str::to_string)
                .ok_or_else(|| -> Failure {
                    format!(
                        "{url} re-minted thread {thread_id} but the grant names no {name}, \
                         so there is no authority to spend it with"
                    )
                    .into()
                })
        };
        let grant = InferenceGrant {
            thread_id: body
                .get("thread")
                .and_then(|t| t.get("id"))
                .and_then(|v| v.as_str())
                .unwrap_or(thread_id)
                .to_string(),
            token: field("token")?,
            proxy_url: field("url")?,
            model: field("model")?,
        };
        self.thread_id = Some(grant.thread_id.clone());
        self.last_model = Some(grant.model.clone());
        self.last_grant = Some(grant.clone());
        Ok(grant)
    }

    /// What this session would report if it ended now.
    ///
    /// `None` until a turn has run. Read-only on purpose: the outcome is
    /// settled from a turn's own result, not asserted by whoever is holding
    /// the session.
    pub fn outcome(&self) -> Option<&ThreadOutcome> {
        self.outcome.as_ref()
    }

    /// End this session's thread by saying what it did.
    ///
    /// This is the exit every path takes. `POST /api/v1/threads/{id}/report`
    /// writes the outcome and revokes the grant in the same call, so the thread
    /// does not stay open holding its remaining budget and the permanent record
    /// says what happened. A session with no thread — the local lane, or one
    /// already ended — writes nothing and answers `Ok(None)`.
    ///
    /// The outcome is [`Self::outcome`], which is the turn's own result and not
    /// a claim made here. A session that held a thread and ran no turn on it
    /// reports [`ThreadOutcome::no_turn`] rather than a success it never had.
    ///
    /// If the report is refused — a deployment older than the route, most
    /// plainly — the thread is still revoked with [`Self::close`] and the
    /// refusal is kept in [`Self::record_failures`], which every caller
    /// already prints. Leaving the thread open because the honest ending was
    /// unavailable would trade one bug for a worse one.
    pub async fn finish(&mut self) -> Result<Option<TurnUsage>, Failure> {
        if self.thread_id.is_none() {
            return Ok(None);
        }
        let outcome = self.outcome.clone().unwrap_or_else(ThreadOutcome::no_turn);
        match self.report(outcome).await {
            Ok(spent) => Ok(spent),
            Err(error) => {
                self.record_failures.push(format!(
                    "the thread could not report how it ended: {error}. \
                     It was cancelled instead, so it does not stay open holding \
                     its grant's remaining budget."
                ));
                self.close().await
            }
        }
    }

    /// Say what the thread did, and end it.
    ///
    /// `POST /api/v1/threads/{id}/report`. The reply carries the revoked
    /// grant's spend, exactly as a revocation's does, so a caller reads what
    /// the session cost in the answer that ends it.
    ///
    /// The thread is released only once the server has taken the report: a
    /// refused report leaves the session still holding its thread, so
    /// [`Self::finish`] can still revoke it rather than leaking it.
    pub async fn report(&mut self, outcome: ThreadOutcome) -> Result<Option<TurnUsage>, Failure> {
        let Some(thread_id) = self.thread_id.clone() else {
            return Ok(None);
        };
        let outcome = if self.cloud_history {
            outcome
        } else {
            outcome.without_conversation()
        };
        let url = format!("{}/threads/{thread_id}/report", self.api_base);
        let mut request = self
            .http
            .post(&url)
            .timeout(Duration::from_secs(30))
            .json(&outcome.wire(self.session_usage));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(format!("{url} refused the report: {status} {}", snippet(&body)).into());
        }
        // Only now: the thread has ended, and there is nothing left to revoke.
        self.thread_id = None;
        self.last_grant = None;
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        Ok(grant_spend(&body))
    }

    /// Throw this session's thread away.
    ///
    /// `DELETE /api/v1/threads/{id}` writes `cancelled` with the sentence *The
    /// thread was cancelled before it reported.*, and the server will not
    /// re-mint authority on a cancelled thread. So this is the disposal, for a
    /// caller that means the thread to be over — not the way a session that did
    /// its work ends. That is [`Self::finish`], and using this instead is how
    /// every session in the account's history came to read as a cancellation
    /// (issue #106).
    ///
    /// A thread left open holds its grant's remaining budget, and the reply
    /// returns the grant's spend, which is why it is worth reading.
    pub async fn close(&mut self) -> Result<Option<TurnUsage>, Failure> {
        let Some(thread_id) = self.thread_id.clone() else {
            return Ok(None);
        };
        let url = format!("{}/threads/{thread_id}", self.api_base);
        let mut request = self.http.delete(&url).timeout(Duration::from_secs(30));
        if let Some(token) = &self.user_token {
            request = request.bearer_auth(token);
        }
        let resp = request.send().await.map_err(|error| -> Failure {
            format!("{url} could not be reached: {error}").into()
        })?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(
                format!("{url} refused the revocation: {status} {}", snippet(&body)).into(),
            );
        }
        self.thread_id = None;
        self.last_grant = None;
        let body: serde_json::Value = resp.json().await.unwrap_or(serde_json::json!({}));
        Ok(grant_spend(&body))
    }

    /// What the server billed this session against what this process counted.
    ///
    /// Ending a thread hands back the grant's own `spent`, and the caller used
    /// to drop it, so the CLI printed its client-side accumulation and nothing could
    /// ever notice the two disagreeing. This is the line that notices: the
    /// server's figure, and a second sentence naming the gap when there is one.
    ///
    /// `None` when the server reported no spend — the local lane, or a session
    /// that never opened a thread — because there is nothing to reconcile.
    pub fn spend_line(&self, reported: Option<TurnUsage>) -> Option<String> {
        let reported = reported?;
        let counted = self.session_usage.total_tokens;
        let billed = reported.total_tokens;
        let line = format!("Billed by the server: {billed} tokens");
        if billed == counted {
            return Some(line);
        }
        Some(format!(
            "{line} — this session counted {counted}, a difference of {}. \
             The server's figure is the one the account is charged against.",
            billed.abs_diff(counted)
        ))
    }

    // ───────────────────────────────────────────────────────── the transcript

    /// The thread this session holds, while it holds one.
    pub fn thread(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }

    /// Append events to this session's thread transcript.
    ///
    /// `POST /api/v1/threads/{id}/events`, batched: one round trip lands the
    /// whole list in order or none of it, capped at the server's own batch
    /// maximum so a long step is split rather than refused whole. A session
    /// holding no thread — the local lane, or one already revoked — writes
    /// nothing and answers `Ok(false)`, because nowhere to write is not a
    /// failure to write.
    pub async fn record(&self, events: &[ThreadRecord]) -> Result<bool, Failure> {
        let Some(thread_id) = &self.thread_id else {
            return Ok(false);
        };
        if events.is_empty() {
            return Ok(false);
        }
        let url = format!("{}/threads/{thread_id}/events", self.api_base);
        for batch in events.chunks(MAX_EVENT_BATCH) {
            let mut request = self
                .http
                .post(&url)
                .timeout(Duration::from_secs(30))
                .json(&serde_json::json!({ "events": batch }));
            if let Some(token) = &self.user_token {
                request = request.bearer_auth(token);
            }
            let resp = request.send().await.map_err(|error| -> Failure {
                format!("{url} could not be reached: {error}").into()
            })?;
            if !resp.status().is_success() {
                let status = resp.status();
                let body = resp.text().await.unwrap_or_default();
                return Err(format!(
                    "{url} refused the transcript append: {status} {}",
                    snippet(&body)
                )
                .into());
            }
        }
        Ok(true)
    }

    /// Record, keeping a refusal rather than failing the turn over it.
    pub async fn note(&mut self, events: Vec<ThreadRecord>) {
        if let Some(store) = self.local_session.as_mut() {
            if let Err(error) = store.append(&events) {
                let kinds = events
                    .iter()
                    .map(|event| event.event_type.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                self.record_failures
                    .push(format!("{kinds} were not saved locally: {error}"));
            }
            // A checkpoint is more than an event: it is the note a resuming
            // session is shown, so it also lands on the summary, which is
            // the one file a resume reads before any replay (#189).
            for event in &events {
                if event.event_type == "turn.checkpoint"
                    && let Some(text) = event.payload.get("text").and_then(|v| v.as_str())
                    && let Err(error) = store.set_last_checkpoint(text)
                {
                    self.record_failures.push(format!(
                        "the checkpoint was not saved to the summary: {error}"
                    ));
                }
                // The registration's status line is derived from the same
                // note (#281): first sentence, truncated — what discovery
                // shows a neighbor deciding where to send work. A missing
                // registration is not an error here: only the session's own
                // startup registers, and an unregistered session has no
                // status to publish.
                if event.event_type == "turn.checkpoint"
                    && let Some(binding) = &self.swarm
                    && let Some(text) = event.payload.get("text").and_then(|v| v.as_str())
                    && let Some(status) = crate::tools::status_from_checkpoint(text)
                    && let Err(error) =
                        crate::swarm::set_status(&binding.home, &binding.session_id, &status)
                {
                    self.record_failures
                        .push(format!("the swarm status was not published: {error}"));
                }
            }
        }
        self.note_cloud(&events).await;
    }

    async fn note_cloud(&mut self, events: &[ThreadRecord]) {
        if self.cloud_history
            && let Err(error) = self.record(events).await
        {
            let kinds = events
                .iter()
                .map(|event| event.event_type.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            self.record_failures
                .push(format!("{kinds} were not recorded: {error}"));
        }
    }

    // ─────────────────────────────────────────────────────────── the turn

    pub async fn execute_turn<F>(
        &mut self,
        prompt: &str,
        chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        self.execute_turn_with_images(prompt, &[], chunk_callback)
            .await
    }

    /// Run one turn with image data attached to the user message.
    pub async fn execute_turn_with_images<F>(
        &mut self,
        prompt: &str,
        images: &[ImageAttachment],
        chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let tool_defs = self.tools.list_tools();
        if self.messages.is_empty() {
            let sys = self.build_system_prompt(&tool_defs);
            self.messages.push(ChatMessage {
                role: "system".to_string(),
                content: Some(sys),
                tool_calls: None,
                tool_call_id: None,
                images: Vec::new(),
            });
        }

        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(prompt.to_string()),
            tool_calls: None,
            tool_call_id: None,
            images: images.iter().map(|image| image.data_url.clone()).collect(),
        });
        self.note(vec![ThreadRecord::user_with_images(prompt, images)])
            .await;

        self.last_usage = TurnUsage::default();
        self.last_calls = 0;
        self.last_reasoning.clear();
        self.pending_failure = None;
        // While the turn runs, the session's standing outcome is an
        // interruption. A turn that returns replaces it below; a session
        // dropped or quit mid-turn never gets that far, and this is what
        // stops it reporting the *previous* turn's success as this session's
        // ending.
        self.outcome = Some(ThreadOutcome::interrupted(
            "The session ended while a turn was still running, so the turn never \
             reported an outcome.",
        ));

        let answered = if self.use_openresponses {
            self.run_responses_turn(&tool_defs, chunk_callback).await
        } else if self.lane.is_local() {
            self.run_local_turn(&tool_defs, chunk_callback).await
        } else {
            self.run_thread_turn(prompt, &tool_defs, chunk_callback)
                .await
        };
        if let Err(error) = &answered
            && self.pending_failure.is_none()
        {
            let why = error.to_string();
            self.note(vec![ThreadRecord::failed(
                &why,
                self.last_usage,
                self.last_calls,
            )])
            .await;
            self.pending_failure = Some(ThreadOutcome::failed(error_code::TURN_FAILED, &why));
        }
        self.session_usage.add(self.last_usage);
        if let Some(store) = self.local_session.as_mut() {
            let _ = store.set_last_model(self.last_model.as_deref());
        }

        // The one place the outcome is decided, and it is decided from the
        // turn's own result rather than from anything a caller believes. An
        // `Err` cannot land here as `succeeded` whatever it says, which is the
        // rule that keeps this from becoming issue #106 pointed the other way.
        self.outcome = Some(match &answered {
            Ok(answer) => ThreadOutcome::succeeded(answer),
            Err(error) => self.pending_failure.take().unwrap_or_else(|| {
                ThreadOutcome::failed(error_code::TURN_FAILED, &error.to_string())
            }),
        });
        answered
    }

    async fn run_thread_turn<F>(
        &mut self,
        prompt: &str,
        tool_defs: &[ToolDefinition],
        mut chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        if let Lane::Named(id) = self.lane.clone() {
            self.check_named(&id).await?;
        }

        // One thread per session, reused across its turns. Opening a fresh one
        // per turn threw away the conversation's own budget and left a trail of
        // open threads nothing ever revoked.
        let grant = match &self.last_grant {
            Some(grant) => grant.clone(),
            None => {
                let grant = self.create_thread().await?;
                self.thread_id = Some(grant.thread_id.clone());
                self.last_grant = Some(grant.clone());
                grant
            }
        };
        self.last_model = Some(grant.model.clone());
        // The local copy was appended before transport selection. Once the
        // server thread exists, an explicit cloud-history opt-in mirrors it.
        self.note_cloud(&[ThreadRecord::user(prompt)]).await;

        let mut final_answer = String::new();
        // False means the step budget ran out with every step still calling
        // tools, which is not an answer and must not be returned as one.
        let mut answered = false;

        for _ in 0..MAX_TOOL_STEPS {
            self.drain_swarm_inbox().await;
            let req_body = serde_json::json!({
                "model": grant.model,
                "messages": chat_completion_messages(&self.messages),
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });

            let (step, deferred_text) = 'attempt: {
                let retry = FIRST_RESPONSE_RETRIES;
                for attempt in 0..=retry {
                    let mut headers = HeaderMap::new();
                    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
                    headers.insert(
                        AUTHORIZATION,
                        HeaderValue::from_str(&format!("Bearer {}", grant.token))?,
                    );

                    let resp = self
                        .http
                        .post(&grant.proxy_url)
                        .headers(headers)
                        .json(&req_body)
                        .send()
                        .await;

                    // A refused or unreachable proxy is a failed turn. The version
                    // this replaces streamed the words `Completed autonomous reasoning
                    // turn (offline fallback).` and returned success, so a rejected
                    // request and a finished one looked the same on screen.
                    let resp = match resp {
                        Ok(r) if r.status().is_success() => r,
                        Ok(r) => {
                            self.tell_progress(TurnProgress::Clear);
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            let why = format!(
                                "{} refused the turn: {status} {}",
                                grant.proxy_url,
                                snippet(&body)
                            );
                            return Err(self
                                .record_failure(error_code::PROVIDER_FAILED, why)
                                .await);
                        }
                        Err(error) => {
                            self.tell_progress(TurnProgress::Clear);
                            let why = format!("{} could not be reached: {error}", grant.proxy_url);
                            return Err(self
                                .record_failure(error_code::PROVIDER_FAILED, why)
                                .await);
                        }
                    };

                    // The grant names the Flash lane. The proxy may reroute a
                    // trivial turn to Gemini; that fact is this header, not
                    // the grant. rc8 ignored it and ATIF always said GLM.
                    if let Some(model) = answered_model(resp.headers()) {
                        self.last_model = Some(model);
                    }

                    let mut stream = resp.bytes_stream().eventsource();
                    let mut step = StepAccumulator::default();
                    // A model can emit prose before it finishes declaring a tool call.
                    // Hold that prose until this round is known to be its final answer;
                    // otherwise the reader sees an unsupported answer before the tools
                    // that were meant to establish it have run.
                    let mut deferred_text = String::new();
                    let started = tokio::time::Instant::now();
                    let mut waiting_sent = false;
                    let mut response_started = false;
                    let mut timed_out = false;

                    loop {
                        let next = if response_started {
                            WatchdogNext::Item(stream.next().await)
                        } else {
                            next_before_first_response(
                                &mut stream,
                                self.first_response,
                                started,
                                &mut waiting_sent,
                                attempt,
                                self.progress_observer.as_ref(),
                            )
                            .await
                        };
                        let event = match next {
                            WatchdogNext::Waiting => continue,
                            WatchdogNext::TimedOut => {
                                timed_out = true;
                                break;
                            }
                            WatchdogNext::Item(Some(Ok(event))) => event,
                            WatchdogNext::Item(Some(Err(error))) => {
                                self.tell_progress(TurnProgress::Clear);
                                let why = format!(
                                    "the reply from {} stopped mid-stream: {error}",
                                    grant.proxy_url
                                );
                                self.last_usage.add(step.usage);
                                self.last_reasoning.push_str(&step.reasoning);
                                return Err(self
                                    .record_failure(error_code::STREAM_BROKEN, why)
                                    .await);
                            }
                            WatchdogNext::Item(None) => break,
                        };
                        if event.data == "[DONE]" {
                            break;
                        }
                        let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data)
                        else {
                            continue;
                        };
                        if !response_started && openai_response_started(&json) {
                            response_started = true;
                            self.tell_progress(TurnProgress::Clear);
                        }
                        let reasoning_start = step.reasoning.len();
                        let mut live_chunks = Vec::new();
                        step.absorb_openai(&json, &mut |chunk| {
                            deferred_text.push_str(chunk);
                            live_chunks.push(chunk.to_string());
                        });
                        for chunk in live_chunks {
                            self.tell_content_delta(chunk).await;
                        }
                        if step.reasoning.len() > reasoning_start {
                            self.tell_stream(ModelStreamEvent::ReasoningDelta(
                                step.reasoning[reasoning_start..].to_string(),
                            ));
                        }
                    }

                    if timed_out {
                        if attempt < retry {
                            self.tell_progress(TurnProgress::Retrying {
                                retry: attempt + 1,
                                max_retries: retry,
                            });
                            continue;
                        }
                        self.tell_progress(TurnProgress::Clear);
                        let why = format!(
                            "{} produced no response within {} seconds after {} retry",
                            grant.proxy_url,
                            self.first_response.timeout_after.as_secs(),
                            retry
                        );
                        return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
                    }

                    self.tell_progress(TurnProgress::Clear);
                    break 'attempt (step, deferred_text);
                }
                unreachable!("the first-response attempt loop always returns or breaks")
            };

            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);

            // The working comes before whatever it led to, in the order it
            // happened. Recorded whole: it is the largest part of what a
            // session produces and a transcript without it is a summary.
            if !step.reasoning.trim().is_empty() {
                let thought = ThreadRecord::reasoning(&step.reasoning);
                self.note(vec![thought]).await;
            }

            if step.tool_calls.is_empty() {
                final_answer = step.content;
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                if !deferred_text.is_empty() {
                    chunk_callback(&deferred_text);
                }
                // The answer joins the transcript. `run_tools` records an
                // assistant turn only when that turn called a tool, so without
                // this the model never sees anything it said itself: asked in
                // the next turn what it just told you, it confabulates a new
                // answer rather than reading the old one. A test that asks it
                // to recall something from the *user's* prompt hides this,
                // because user messages were always recorded.
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: if final_answer.is_empty() {
                        None
                    } else {
                        Some(final_answer.clone())
                    },
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                let said = ThreadRecord::assistant_on(
                    &final_answer,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                );
                self.note(vec![said]).await;
                answered = true;
                break;
            }
            if !step.content.is_empty() {
                self.tell_stream(ModelStreamEvent::ContentDiscarded);
            }
            let ran = self.run_tools(step).await;
            self.note(ran.records).await;
            if ran.cancelled {
                return Err("The turn was canceled while its tools were running.".into());
            }
            if self.last_calls >= MAX_TOOL_STEPS {
                // The budget is spent. Instead of the old bare kill -- a
                // `turn.failed` with no answer and the work unreported
                // (#188's two dead turns) -- ask the model once to stop and
                // report, tools withheld so the request is answerable, and
                // treat the words it returns as the turn's final answer.
                final_answer = self.finalize_turn_with_a_report(tool_defs).await?;
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(final_answer.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                let said = ThreadRecord::assistant_on(
                    &final_answer,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                );
                self.note(vec![said]).await;
                answered = true;
                break;
            }
            self.report_budget_milestones().await;
        }

        if !answered {
            let why = "the model ended the turn without a final answer".to_string();
            return Err(self.record_failure(error_code::MAX_STEPS, why).await);
        }
        Ok(final_answer)
    }

    /// Ask the model for its end-of-turn report, tools withheld (#188).
    ///
    /// One model round, no tools declared: the cap is spent, so the only
    /// useful move left is words. The reply is prefixed so a reader can tell
    /// a budget report from a completed run. A model that answers the report
    /// request with more tool calls gets one retry with the instruction
    /// restated; anything further is what the old code did, a failure, but
    /// now after the model has had its chance.
    async fn finalize_turn_with_a_report(
        &mut self,
        tool_defs: &[ToolDefinition],
    ) -> Result<String, Failure> {
        self.note(vec![ThreadRecord::budget_reached(
            self.last_usage,
            self.last_calls,
        )])
        .await;
        for attempt in 0..2 {
            if attempt == 0 {
                self.messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(
                        "This turn has used its whole tool-call budget. Do not call tools now. \
                         Reply with the state a reader needs to take over: what this turn was \
                         doing, what landed (files, commits, test results), what is unfinished \
                         or broken, and the exact next step. Be brief and concrete."
                            .to_string(),
                    ),
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
            } else {
                self.messages.push(ChatMessage {
                    role: "user".to_string(),
                    content: Some(
                        "No tools are available: the turn's budget is spent. Reply in words \
                         only, with the state summary asked for."
                            .to_string(),
                    ),
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
            }
            self.note(vec![ThreadRecord::budget_instruction()]).await;
            let step = self.step_thread_once(tool_defs).await?;
            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);
            if !step.reasoning.trim().is_empty() {
                self.note(vec![ThreadRecord::reasoning(&step.reasoning)])
                    .await;
            }
            if step.tool_calls.is_empty() {
                self.note(vec![ThreadRecord::budget_report(
                    self.last_usage,
                    self.last_calls,
                )])
                .await;
                return Ok(Self::finalize_answer(self.last_calls, step.content));
            }
            self.tell_stream(ModelStreamEvent::ContentDiscarded);
            let ran = self.run_tools(step).await;
            self.note(ran.records).await;
            if ran.cancelled {
                return Err("The turn was canceled while its tools were running.".into());
            }
        }
        let why = "the model ended the turn without a final answer: asked twice to report its \
                   state after the tool-call budget was spent, it kept calling tools"
            .to_string();
        Err(self.record_failure(error_code::MAX_STEPS, why).await)
    }

    /// One model round on the thread transport, whatever it yields.
    ///
    /// `tools` are declared empty so the model physically cannot spend the
    /// budget it no longer has; the step still carries usage and reasoning,
    /// which the finalization records like any other round.
    async fn step_thread_once(
        &mut self,
        _tool_defs: &[ToolDefinition],
    ) -> Result<StepAccumulator, Failure> {
        let grant = match &self.last_grant {
            Some(grant) => grant.clone(),
            None => return Err("the turn's thread grant is gone".into()),
        };
        let req_body = serde_json::json!({
            "model": grant.model,
            "messages": chat_completion_messages(&self.messages),
            "tools": [],
            "stream": true
        });
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {}", grant.token))?,
        );
        let resp = self
            .http
            .post(&grant.proxy_url)
            .headers(headers)
            .json(&req_body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                let why = format!(
                    "{} refused the turn: {status} {}",
                    grant.proxy_url,
                    snippet(&body)
                );
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
            Err(error) => {
                let why = format!("{} could not be reached: {error}", grant.proxy_url);
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
        };
        if let Some(model) = answered_model(resp.headers()) {
            self.last_model = Some(model);
        }
        let mut stream = resp.bytes_stream().eventsource();
        let mut step = StepAccumulator::default();
        loop {
            match stream.next().await {
                Some(Ok(event)) => {
                    if event.data == "[DONE]" {
                        break;
                    }
                    let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) else {
                        continue;
                    };
                    // The report is not streamed live: it is committed once,
                    // below, with its budget prefix. Chunks land in the
                    // accumulator only.
                    let mut sink = |_chunk: &str| {};
                    step.absorb_openai(&json, &mut sink);
                }
                Some(Err(error)) => {
                    let why = format!(
                        "the reply from {} stopped mid-stream: {error}",
                        grant.proxy_url
                    );
                    return Err(self.record_failure(error_code::STREAM_BROKEN, why).await);
                }
                None => break,
            }
        }
        Ok(step)
    }

    /// One no-tools round on the OpenResponses transport, for the budget
    /// report. Same request the turn loop sends, tools withheld, stream read
    /// to completion without painting the frame.
    async fn step_responses_once(
        &mut self,
        model: Option<&str>,
    ) -> Result<StepAccumulator, Failure> {
        let input = messages_to_responses_input(&self.messages);
        let mut body = serde_json::json!({
            "input": input,
            "tools": [],
            "stream": true
        });
        if let Some(model) = model {
            body["model"] = serde_json::Value::String(model.to_string());
        }
        let url = format!("{}/responses", self.api_base);
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        if let Some(token) = &self.user_token {
            headers.insert(
                AUTHORIZATION,
                HeaderValue::from_str(&format!("Bearer {token}"))?,
            );
        }
        let resp = self
            .http
            .post(&url)
            .headers(headers)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let response_body = r.text().await.unwrap_or_default();
                let why = format!(
                    "{url} refused the turn: {status} {}",
                    snippet(&response_body)
                );
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
            Err(error) => {
                let why = format!("{url} could not be reached: {error}");
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
        };
        let mut stream = resp.bytes_stream().eventsource();
        let mut step = StepAccumulator::default();
        loop {
            match stream.next().await {
                Some(Ok(event)) => {
                    if event.data == "[DONE]" {
                        break;
                    }
                    let Ok(json) = serde_json::from_str::<serde_json::Value>(&event.data) else {
                        continue;
                    };
                    let mut sink = |_chunk: &str| {};
                    step.absorb_responses(&json, &mut sink);
                }
                Some(Err(error)) => {
                    let why = format!("the reply from {url} stopped mid-stream: {error}");
                    return Err(self.record_failure(error_code::STREAM_BROKEN, why).await);
                }
                None => break,
            }
        }
        Ok(step)
    }

    async fn run_responses_turn<F>(
        &mut self,
        tool_defs: &[ToolDefinition],
        mut chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let mut final_answer = String::new();
        let mut answered = false;
        // Resolve a switchable lane once per turn. A tool loop stays on the
        // same model, and the frame can label the answer with the exact model
        // this request named after the answer completes.
        let resolved_model = self.lane_model().await?;

        for _ in 0..MAX_TOOL_STEPS {
            self.drain_swarm_inbox().await;
            let input = messages_to_responses_input(&self.messages);
            let mut body = serde_json::json!({
                "input": input,
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });
            if let Some(model) = &resolved_model {
                body["model"] = serde_json::Value::String(model.clone());
            }

            let url = format!("{}/responses", self.api_base);
            let mut headers = HeaderMap::new();
            headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
            if let Some(token) = &self.user_token {
                headers.insert(
                    AUTHORIZATION,
                    HeaderValue::from_str(&format!("Bearer {token}"))?,
                );
            }

            let (step, deferred_text, completed) = 'attempt: {
                let retry = FIRST_RESPONSE_RETRIES;
                for attempt in 0..=retry {
                    let resp = self
                        .http
                        .post(&url)
                        .headers(headers.clone())
                        .json(&body)
                        .send()
                        .await;
                    let resp = match resp {
                        Ok(r) if r.status().is_success() => r,
                        Ok(r) => {
                            self.tell_progress(TurnProgress::Clear);
                            let status = r.status();
                            let body = r.text().await.unwrap_or_default();
                            let why =
                                format!("{url} refused the turn: {status} {}", snippet(&body));
                            return Err(self
                                .record_failure(error_code::PROVIDER_FAILED, why)
                                .await);
                        }
                        Err(error) => {
                            self.tell_progress(TurnProgress::Clear);
                            let why = format!("{url} could not be reached: {error}");
                            return Err(self
                                .record_failure(error_code::PROVIDER_FAILED, why)
                                .await);
                        }
                    };

                    let mut stream = resp.bytes_stream().eventsource();
                    let mut step = StepAccumulator::default();
                    let mut deferred_text = String::new();
                    let mut completed = false;
                    let started = tokio::time::Instant::now();
                    let mut waiting_sent = false;
                    let mut response_started = false;
                    let mut timed_out = false;

                    loop {
                        let next = if response_started {
                            WatchdogNext::Item(stream.next().await)
                        } else {
                            next_before_first_response(
                                &mut stream,
                                self.first_response,
                                started,
                                &mut waiting_sent,
                                attempt,
                                self.progress_observer.as_ref(),
                            )
                            .await
                        };
                        let event = match next {
                            WatchdogNext::Waiting => continue,
                            WatchdogNext::TimedOut => {
                                timed_out = true;
                                break;
                            }
                            WatchdogNext::Item(Some(Ok(event))) => event,
                            WatchdogNext::Item(Some(Err(error))) => {
                                self.tell_progress(TurnProgress::Clear);
                                let why =
                                    format!("the reply from {url} stopped mid-stream: {error}");
                                self.last_usage.add(step.usage);
                                self.last_reasoning.push_str(&step.reasoning);
                                return Err(self
                                    .record_failure(error_code::STREAM_BROKEN, why)
                                    .await);
                            }
                            WatchdogNext::Item(None) => break,
                        };

                        if let Ok(value) = serde_json::from_str::<serde_json::Value>(&event.data) {
                            if !response_started && responses_response_started(&value) {
                                response_started = true;
                                self.tell_progress(TurnProgress::Clear);
                            }
                            if event.event == "response.completed" {
                                let reasoning_start = step.reasoning.len();
                                let mut live_chunks = Vec::new();
                                step.absorb_responses(&value, &mut |chunk| {
                                    deferred_text.push_str(chunk);
                                    live_chunks.push(chunk.to_string());
                                });
                                for chunk in live_chunks {
                                    self.tell_content_delta(chunk).await;
                                }
                                if step.reasoning.len() > reasoning_start {
                                    self.tell_stream(ModelStreamEvent::ReasoningDelta(
                                        step.reasoning[reasoning_start..].to_string(),
                                    ));
                                }
                                completed = true;
                                break;
                            }
                            if event.event == "response.failed" {
                                self.tell_progress(TurnProgress::Clear);
                                let message = value
                                    .get("response")
                                    .and_then(|r| r.get("error"))
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .unwrap_or("response failed");
                                let why = format!("{url} reported a failed response: {message}");
                                self.last_usage.add(step.usage);
                                self.last_reasoning.push_str(&step.reasoning);
                                return Err(self
                                    .record_failure(error_code::PROVIDER_FAILED, why)
                                    .await);
                            }
                            let reasoning_start = step.reasoning.len();
                            let mut live_chunks = Vec::new();
                            step.absorb_responses(&value, &mut |chunk| {
                                deferred_text.push_str(chunk);
                                live_chunks.push(chunk.to_string());
                            });
                            for chunk in live_chunks {
                                self.tell_content_delta(chunk).await;
                            }
                            if step.reasoning.len() > reasoning_start {
                                self.tell_stream(ModelStreamEvent::ReasoningDelta(
                                    step.reasoning[reasoning_start..].to_string(),
                                ));
                            }
                        }
                    }

                    if timed_out {
                        if attempt < retry {
                            self.tell_progress(TurnProgress::Retrying {
                                retry: attempt + 1,
                                max_retries: retry,
                            });
                            continue;
                        }
                        self.tell_progress(TurnProgress::Clear);
                        let why = format!(
                            "{url} produced no response within {} seconds after one retry",
                            self.first_response.timeout_after.as_secs()
                        );
                        return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
                    }

                    self.tell_progress(TurnProgress::Clear);
                    break 'attempt (step, deferred_text, completed);
                }
                unreachable!("the first-response attempt loop always returns or breaks")
            };

            if !completed {
                let why = format!("the reply from {url} ended without response.completed");
                self.last_usage.add(step.usage);
                self.last_reasoning.push_str(&step.reasoning);
                return Err(self.record_failure(error_code::STREAM_BROKEN, why).await);
            }

            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);

            if !step.reasoning.trim().is_empty() {
                let thought = ThreadRecord::reasoning(&step.reasoning);
                self.note(vec![thought]).await;
            }

            if step.tool_calls.is_empty() {
                final_answer = step.content;
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                if !deferred_text.is_empty() {
                    chunk_callback(&deferred_text);
                }
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: if final_answer.is_empty() {
                        None
                    } else {
                        Some(final_answer.clone())
                    },
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                self.last_model = resolved_model.clone();
                self.note(vec![ThreadRecord::assistant_on(
                    &final_answer,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                )])
                .await;
                answered = true;
                break;
            }

            if !step.content.is_empty() {
                self.tell_stream(ModelStreamEvent::ContentDiscarded);
            }

            let ran = self.run_tools(step).await;
            self.note(ran.records).await;
            if ran.cancelled {
                return Err("The turn was canceled while its tools were running.".into());
            }
            if self.last_calls >= MAX_TOOL_STEPS {
                // The budget is spent on the OpenResponses transport too:
                // one no-tools round, its words prefixed and committed as
                // the answer (#188). `step_once_responses` shares the
                // request shape the loop above uses, with tools withheld.
                let step = self.step_responses_once(resolved_model.as_deref()).await?;
                self.last_usage.add(step.usage);
                self.last_reasoning.push_str(&step.reasoning);
                if !step.reasoning.trim().is_empty() {
                    self.note(vec![ThreadRecord::reasoning(&step.reasoning)])
                        .await;
                }
                let report = Self::finalize_answer(self.last_calls, step.content);
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(report.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                self.last_model = resolved_model.clone();
                self.note(vec![ThreadRecord::assistant_on(
                    &report,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                )])
                .await;
                return Ok(report);
            }
            self.report_budget_milestones().await;
        }

        if !answered {
            let why = "the model ended the turn without a final answer".to_string();
            return Err(self.record_failure(error_code::MAX_STEPS, why).await);
        }
        Ok(final_answer)
    }

    /// Write a turn's failure to the transcript, then hand back the failure.
    ///
    /// Every exit from a turn that is not the model's own answer goes through
    /// here, so the record cannot say a session succeeded where it did not.
    ///
    /// `code` is which way it failed, kept for the thread's report: the `Err`
    /// that travels back up the stack is a sentence, and a sentence is not a
    /// category anything can group on.
    async fn record_failure(&mut self, code: &'static str, why: String) -> Failure {
        let (usage, calls) = (self.last_usage, self.last_calls);
        self.note(vec![ThreadRecord::failed(&why, usage, calls)])
            .await;
        self.pending_failure = Some(ThreadOutcome::failed(code, &why));
        // The counters ride the sentence, not only the structured record:
        // the sentence is what the live frame, the ATIF export notice, and
        // this `Err` all carry. A diagnosis that needed a session-store dig
        // to learn two turns died at 100 calls (#188) reads it here instead.
        let mut with_budget = why;
        if calls > 0 {
            with_budget.push_str(&format!(
                " ({} tool calls, {} tokens this turn)",
                calls, usage.total_tokens
            ));
        }
        with_budget.into()
    }

    /// The countdown line a tool result carries at a budget threshold.
    ///
    /// Tool output already names what the model must read; this names what it
    /// must ration. A model that learns its budget only by dying to it never
    /// gets to wrap up: session `1a0434b26a4` burned two full turns to the
    /// cap and died mid-fix with the work unreported (#188). The notice says
    /// how many calls are left, so the model can spend them finishing rather
    /// than discovering the limit.
    fn budget_notice(calls_used: usize) -> Option<String> {
        let left = MAX_TOOL_STEPS.checked_sub(calls_used)?;
        if !BUDGET_NOTICES.contains(&left) {
            return None;
        }
        Some(format!(
            "[Turn budget: {left} tool {} left before this turn is stopped and asked to report. \
             Spend them finishing, or stop and report what landed, what is broken, and what is \
             next.]",
            if left == 1 { "call" } else { "calls" }
        ))
    }

    /// The answer a finalizing turn records from the model's words (#188).
    ///
    /// The turn ended under budget rules, so this is a real answer, but it is
    /// a report made at the cap, not a completed run: the prefix says so.
    /// A model that complied with "no tools" by saying nothing gets the plain
    /// fact instead of an invented summary.
    fn finalize_answer(calls: usize, content: String) -> String {
        if content.trim().is_empty() {
            format!(
                "The turn reached its tool-call limit ({calls}) before finishing, and the model \
                 returned no summary. Work from the transcript: the last tool results show where \
                 it stopped."
            )
        } else {
            format!(
                "[The turn reached its tool-call limit before finishing. The model's report of \
                 where it stopped:]\n\n{content}"
            )
        }
    }

    /// Report the turn's spending when it crosses a report line.
    ///
    /// A model that can see what a turn has cost can pace the next one.
    /// `turn.budget` carries the counters the failure record carries, at the
    /// same moment, from the same source.
    async fn report_budget_milestones(&mut self) {
        for &line in &BUDGET_REPORT_AT {
            if self.last_calls == line {
                self.note(vec![ThreadRecord::budget_report(
                    self.last_usage,
                    self.last_calls,
                )])
                .await;
                return;
            }
        }
    }

    /// Record that this session stopped before the turn in flight answered.
    ///
    /// A dropped turn future never reaches [`Self::record_failure`] — a
    /// cancelled child, a session quit mid-turn — so the caller that dropped
    /// it says so here. Without this an interruption leaves a transcript that
    /// simply stops, which a later reader has no way to tell from a turn that
    /// finished quietly.
    ///
    /// It settles the session's outcome too, so the thread's report says the
    /// session was stopped rather than carrying whatever the last turn that
    /// did finish had said.
    pub async fn note_interruption(&mut self, why: &str) {
        let (usage, calls) = (self.last_usage, self.last_calls);
        self.note(vec![ThreadRecord::failed(why, usage, calls)])
            .await;
        self.outcome = Some(ThreadOutcome::interrupted(why));
    }

    /// Record the assistant's tool calls, run them, and put the results back.
    ///
    /// Hands back one `tool.ran` per call for the transcript: the call and its
    /// result are one fact, and the caller has both only once the tool has run.
    async fn run_tools(&mut self, step: StepAccumulator) -> ToolBatch {
        let recorded: Vec<serde_json::Value> = step
            .tool_calls
            .values()
            .map(|(id, name, args)| {
                serde_json::json!({
                    "id": id,
                    "type": "function",
                    "function": { "name": name, "arguments": args }
                })
            })
            .collect();

        self.messages.push(ChatMessage {
            role: "assistant".to_string(),
            content: if step.content.is_empty() {
                None
            } else {
                Some(step.content)
            },
            tool_calls: Some(recorded),
            tool_call_id: None,
            images: Vec::new(),
        });

        let calls = step
            .tool_calls
            .into_values()
            .map(|(id, name, args_str)| {
                let arguments: serde_json::Value =
                    serde_json::from_str(&args_str).unwrap_or(serde_json::json!({}));
                (
                    ToolCall {
                        id: id.clone(),
                        name: name.clone(),
                        arguments,
                    },
                    args_str,
                )
            })
            .collect::<Vec<_>>();

        for (call, args_str) in &calls {
            // Before the call, so a caller drawing a frame can show what is in
            // flight rather than only what has already finished.
            self.tell(ToolEvent::Started {
                call_id: call.id.clone(),
                name: call.name.clone(),
                arguments: args_str.clone(),
            });
        }

        let (_keep_open, fallback) = tokio::sync::watch::channel(false);
        let cancellation = self.tool_cancellation.as_ref().cloned().unwrap_or(fallback);
        let results = join_all(calls.iter().map(|(call, _)| {
            self.tools
                .execute_tool_cancellable(call, cancellation.clone())
        }))
        .await;

        // The countdown rides the results of the batch that crossed a
        // threshold, on the last result of it: one line, in the place the
        // model reads next. Attached after `last_calls` below is updated.
        self.last_calls += calls.len();
        let mut ran = Vec::with_capacity(calls.len());
        let mut cancelled = false;
        for ((call, args_str), result) in calls.into_iter().zip(results) {
            cancelled |= result.output == crate::tools::CANCELLED_TOOL_RESULT;
            self.tell(ToolEvent::Finished {
                call_id: call.id.clone(),
                name: call.name.clone(),
                output: result.output.clone(),
                is_error: result.is_error,
                duration_ms: result.duration_ms,
            });
            ran.push(ThreadRecord::tool_ran_on(
                &call.id,
                &call.name,
                &args_str,
                &result.output,
                result.duration_ms,
            ));
            // The checkpoint's own event, beside its tool record: `note`
            // replays it into the transcript and, for a local session, onto
            // the summary the next session resumes from (#189). The note
            // keeps the argument as sent, not the tool's acknowledgement --
            // the acknowledgement is for the model, the note is for whoever
            // reads later.
            if call.name == "checkpoint"
                && !result.is_error
                && let Some(text) = call.arguments.get("text").and_then(|v| v.as_str())
                && !text.trim().is_empty()
            {
                ran.push(ThreadRecord::checkpoint(text));
            }
            if call.name == "swarm_send" && !result.is_error {
                if let Ok(report) = serde_json::from_str::<serde_json::Value>(&result.output) {
                    ran.push(ThreadRecord::new(
                        "swarm_message",
                        serde_json::json!({
                            "direction": "sent",
                            "id": report.get("message_id"),
                            "from": report.get("from"),
                            "to": report.get("to"),
                            "kind": report.get("kind"),
                            "thread": report.get("thread"),
                            "reply_expected": call.arguments.get("reply_expected"),
                            "body": call.arguments.get("body"),
                        }),
                    ));
                }
            }
            self.messages.push(ChatMessage {
                role: "tool".to_string(),
                content: Some(result.output),
                tool_calls: None,
                tool_call_id: Some(call.id),
                images: Vec::new(),
            });
        }
        // One countdown line, once per batch, on the reply the model reads
        // next. A model that learns its budget only by dying to it never
        // gets to wrap up; this is how it learns in time (#188).
        if let Some(notice) = Self::budget_notice(self.last_calls)
            && let Some(last) = ran.last_mut()
        {
            let mut output = last
                .payload
                .get("output")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            output.push_str("\n\n");
            output.push_str(&notice);
            last.payload["output"] = serde_json::Value::String(output);
        }

        ToolBatch {
            records: ran,
            cancelled,
        }
    }

    // ────────────────────────────────────────────────────── the local lane

    /// Whether an Ollama server on this machine can answer, from one bounded
    /// probe (issue #291).
    ///
    /// `Some(model)` names the model the local lane would resolve to — the
    /// most recently modified installed one, the same choice the bare local
    /// lane makes in [`Self::resolve_local_model`]. `None` covers every kind
    /// of absence: no server, a refusal, a timeout, an empty library. The
    /// caller treats them all as "the local lane is not on offer" and never
    /// surfaces this probe's failure, because a machine without Ollama is a
    /// normal machine, not a broken one.
    ///
    /// This is a walk-membership probe, not a turn: it opens no thread,
    /// mints nothing, and is paid once per interactive session at open, not
    /// per keystroke. A turn still goes through [`Self::resolve_local_model`],
    /// which re-resolves against the live server and fails honestly if the
    /// model vanished between the probe and the prompt.
    pub async fn probe_local_lane() -> Option<String> {
        Self::probe_local_lane_at(&OLLAMA_HOST).await
    }

    /// The model family the cycle gate requires (issue #292).
    ///
    /// The owner's gate, verbatim: the local lane joins the shift+tab walk
    /// only when Ollama is installed **and** a Qwen 3.8 model is loaded.
    /// Family, not substring: `qwen3.8` and `qwen3` are different families,
    /// and Rust's `starts_with` would happily say `"qwen3.8:x".starts_with(
    /// "qwen3")` — so the tag's family segment (before the first `:`) is
    /// compared whole against `qwen3.8`.
    pub const CYCLE_GATE_FAMILY: &str = "qwen3.8";

    /// [`Self::probe_local_lane`] against a named host, for tests.
    async fn probe_local_lane_at(host: &str) -> Option<String> {
        let url = format!("{}/api/tags", host.trim_end_matches('/'));
        let client = reqwest::Client::builder()
            .timeout(Duration::from_millis(300))
            .build()
            .unwrap_or_default();
        let resp = client.get(&url).send().await.ok()?;
        if !resp.status().is_success() {
            return None;
        }
        let body: serde_json::Value = resp.json().await.ok()?;
        let mut models: Vec<(String, String)> = body
            .get("models")
            .and_then(|v| v.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        let name = m.get("name").and_then(|v| v.as_str())?;
                        let modified = m
                            .get("modified_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        Some((name.to_string(), modified.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        // The walk gate (#292): only a model of the gated family lights the
        // lane up. Everything else is invisible to the cycle — it keeps every
        // explicit path it has, it just does not join the walk.
        models.retain(|(name, _)| {
            name.split(':')
                .next()
                .is_some_and(|family| family.eq_ignore_ascii_case(Self::CYCLE_GATE_FAMILY))
        });
        if models.is_empty() {
            return None;
        }
        models.sort_by(|left, right| right.1.cmp(&left.1));
        Some(models.remove(0).0)
    }

    /// The Ollama models installed here, most recently modified first.
    pub async fn installed_local_models(&self) -> Result<Vec<String>, Failure> {
        let url = format!("{}/api/tags", self.ollama_host.trim_end_matches('/'));
        let resp = self
            .http
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|error| -> Failure {
                format!(
                    "no Ollama server answered at {}: {error}. \
                     Start one, or choose a hosted lane.",
                    self.ollama_host
                )
                .into()
            })?;
        if !resp.status().is_success() {
            return Err(format!("{url} refused the model list: {}", resp.status()).into());
        }
        let body: serde_json::Value = resp.json().await?;
        let mut models: Vec<(String, String)> = body
            .get("models")
            .and_then(|v| v.as_array())
            .map(|models| {
                models
                    .iter()
                    .filter_map(|m| {
                        let name = m.get("name").and_then(|v| v.as_str())?;
                        let modified = m
                            .get("modified_at")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default();
                        Some((name.to_string(), modified.to_string()))
                    })
                    .collect()
            })
            .unwrap_or_default();
        models.sort_by(|left, right| right.1.cmp(&left.1));
        Ok(models.into_iter().map(|(name, _)| name).collect())
    }

    /// The installed model a name means.
    ///
    /// Exact first, so a full name is never reinterpreted; then the family
    /// prefix, because a reader who pulled `qwen3.8:27b-mtp-q8_0` says
    /// `qwen3.8` and sending that unresolved earns `model not found` from a
    /// server that has the model. An empty name takes the most recently
    /// modified, which is the one they were last working with.
    async fn resolve_local_model(&self, wanted: &str) -> Result<String, Failure> {
        let installed = self.installed_local_models().await?;
        if installed.is_empty() {
            return Err(format!(
                "the Ollama server at {} has no models installed. \
                 Pull one with `ollama pull <model>`.",
                self.ollama_host
            )
            .into());
        }
        if wanted.is_empty() {
            return Ok(installed[0].clone());
        }
        if installed.iter().any(|m| m == wanted) {
            return Ok(wanted.to_string());
        }
        if let Some(family) = installed
            .iter()
            .find(|m| m.starts_with(&format!("{wanted}:")))
        {
            return Ok(family.clone());
        }
        Err(format!(
            "the Ollama server at {} does not have '{wanted}'. It has: {}.",
            self.ollama_host,
            installed.join(", ")
        )
        .into())
    }

    async fn run_local_turn<F>(
        &mut self,
        tool_defs: &[ToolDefinition],
        mut chunk_callback: F,
    ) -> Result<String, Failure>
    where
        F: FnMut(&str) + Send + 'static,
    {
        let Lane::Local(wanted) = self.lane.clone() else {
            return Err("run_local_turn was called off the local lane".into());
        };
        let model = self.resolve_local_model(&wanted).await?;
        self.last_model = Some(format!("ollama:{model}"));
        // No grant is minted here and none is invented: the model is on this
        // machine, so there is nothing for the server to authorise.
        self.last_grant = None;

        let url = format!("{}/api/chat", self.ollama_host.trim_end_matches('/'));
        let mut final_answer = String::new();
        // False means the step budget ran out with every step still calling
        // tools, which is not an answer and must not be returned as one.
        let mut answered = false;

        for _ in 0..MAX_TOOL_STEPS {
            self.drain_swarm_inbox().await;
            let mut req_body = serde_json::json!({
                "model": model,
                "messages": self.messages.iter().map(ollama_message).collect::<Vec<_>>(),
                "tools": tool_defs.iter().map(|t| serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters
                    }
                })).collect::<Vec<_>>(),
                "stream": true
            });
            self.apply_ollama_options(&mut req_body);

            let resp = self.http.post(&url).json(&req_body).send().await;
            let resp = match resp {
                Ok(r) if r.status().is_success() => r,
                Ok(r) => {
                    let status = r.status();
                    let body = r.text().await.unwrap_or_default();
                    let why = format!("{url} refused the turn: {status} {}", snippet(&body));
                    return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
                }
                Err(error) => {
                    let why = format!("{url} could not be reached: {error}");
                    return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
                }
            };

            // Ollama streams newline-delimited JSON rather than server-sent
            // events, so the frames are split here rather than by `Eventsource`.
            let mut bytes = resp.bytes_stream();
            let mut pending = String::new();
            let mut step = StepAccumulator::default();
            let mut deferred_text = String::new();

            while let Some(chunk) = bytes.next().await {
                let chunk = match chunk {
                    Ok(chunk) => chunk,
                    Err(error) => {
                        let why = format!("the reply from {url} stopped mid-stream: {error}");
                        self.last_usage.add(step.usage);
                        self.last_reasoning.push_str(&step.reasoning);
                        return Err(self.record_failure(error_code::STREAM_BROKEN, why).await);
                    }
                };
                pending.push_str(&String::from_utf8_lossy(&chunk));
                while let Some(newline) = pending.find('\n') {
                    let line: String = pending.drain(..=newline).collect();
                    let line = line.trim().to_string();
                    if line.is_empty() {
                        continue;
                    }
                    if let Ok(json) = serde_json::from_str::<serde_json::Value>(&line) {
                        let reasoning_start = step.reasoning.len();
                        let mut live_chunks = Vec::new();
                        step.absorb_ollama(&json, &mut |chunk| {
                            deferred_text.push_str(chunk);
                            live_chunks.push(chunk.to_string());
                        });
                        for chunk in live_chunks {
                            self.tell_content_delta(chunk).await;
                        }
                        if step.reasoning.len() > reasoning_start {
                            self.tell_stream(ModelStreamEvent::ReasoningDelta(
                                step.reasoning[reasoning_start..].to_string(),
                            ));
                        }
                    }
                }
            }
            let tail = pending.trim();
            if !tail.is_empty()
                && let Ok(json) = serde_json::from_str::<serde_json::Value>(tail)
            {
                let reasoning_start = step.reasoning.len();
                let mut live_chunks = Vec::new();
                step.absorb_ollama(&json, &mut |chunk| {
                    deferred_text.push_str(chunk);
                    live_chunks.push(chunk.to_string());
                });
                for chunk in live_chunks {
                    self.tell_content_delta(chunk).await;
                }
                if step.reasoning.len() > reasoning_start {
                    self.tell_stream(ModelStreamEvent::ReasoningDelta(
                        step.reasoning[reasoning_start..].to_string(),
                    ));
                }
            }

            self.last_usage.add(step.usage);
            self.last_reasoning.push_str(&step.reasoning);

            if !step.reasoning.trim().is_empty() {
                self.note(vec![ThreadRecord::reasoning(&step.reasoning)])
                    .await;
            }

            if step.tool_calls.is_empty() {
                final_answer = step.content;
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                if !deferred_text.is_empty() {
                    chunk_callback(&deferred_text);
                }
                // The answer joins the transcript. `run_tools` records an
                // assistant turn only when that turn called a tool, so without
                // this the model never sees anything it said itself: asked in
                // the next turn what it just told you, it confabulates a new
                // answer rather than reading the old one. A test that asks it
                // to recall something from the *user's* prompt hides this,
                // because user messages were always recorded.
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: if final_answer.is_empty() {
                        None
                    } else {
                        Some(final_answer.clone())
                    },
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                self.note(vec![ThreadRecord::assistant_on(
                    &final_answer,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                )])
                .await;
                answered = true;
                break;
            }
            if !step.content.is_empty() {
                self.tell_stream(ModelStreamEvent::ContentDiscarded);
            }
            // The local lane holds no thread of its own, so `note` writes
            // nothing; it is called anyway so a local session that resumed
            // somebody's thread records against it like any other.
            let ran = self.run_tools(step).await;
            self.note(ran.records).await;
            if ran.cancelled {
                return Err("The turn was canceled while its tools were running.".into());
            }
            if self.last_calls >= MAX_TOOL_STEPS {
                // The local lane gets the same soft landing (#188), with one
                // no-tools round against the local model. Ollama answers a
                // tools-less chat with words; the report is the answer.
                let report =
                    Self::finalize_answer(self.last_calls, self.step_local_report().await?);
                self.tell_stream(ModelStreamEvent::ContentCommitted);
                self.messages.push(ChatMessage {
                    role: "assistant".to_string(),
                    content: Some(report.clone()),
                    tool_calls: None,
                    tool_call_id: None,
                    images: Vec::new(),
                });
                self.note(vec![ThreadRecord::assistant_on(
                    &report,
                    self.last_usage,
                    self.last_calls,
                    self.last_model.as_deref(),
                )])
                .await;
                return Ok(report);
            }
            self.report_budget_milestones().await;
        }

        if !answered {
            let why = "the model ended the turn without a final answer".to_string();
            return Err(self.record_failure(error_code::MAX_STEPS, why).await);
        }
        Ok(final_answer)
    }

    /// One no-tools round against the local model, for the budget report.
    ///
    /// Ollama takes plain chat; the instruction is pushed as a user message
    /// and the reply is read whole, since the local lane streams NDJSON and
    /// only the final content matters here.
    /// Fold this session's Ollama request controls into a local chat body
    /// (issue #293).
    ///
    /// `options.num_ctx` when the reader set one; `think` when the session
    /// carries a reasoning effort. Both are additive: unset controls are
    /// absent from the body, so Ollama's own defaults stand and a body that
    /// never asked for a control cannot be blamed for one.
    fn apply_ollama_options(&self, body: &mut serde_json::Value) {
        if let Some(num_ctx) = self.ollama_num_ctx {
            body["options"]["num_ctx"] = serde_json::json!(num_ctx);
        }
        if let Some(think) = self.ollama_think() {
            body["think"] = serde_json::json!(think);
        }
    }

    async fn step_local_report(&mut self) -> Result<String, Failure> {
        let notice = "This turn has used its whole tool-call budget. Reply in words only, \
                      with the state a reader needs to take over: what this turn was doing, \
                      what landed, what is unfinished or broken, and the exact next step. \
                      Be brief and concrete.";
        self.messages.push(ChatMessage {
            role: "user".to_string(),
            content: Some(notice.to_string()),
            tool_calls: None,
            tool_call_id: None,
            images: Vec::new(),
        });
        self.note(vec![ThreadRecord::budget_instruction()]).await;
        let Lane::Local(wanted) = self.lane.clone() else {
            return Err("run_local_turn was called off the local lane".into());
        };
        let model = self.resolve_local_model(&wanted).await?;
        let mut req_body = serde_json::json!({
            "model": model,
            "messages": self.messages.iter().map(ollama_message).collect::<Vec<_>>(),
            "stream": false
        });
        self.apply_ollama_options(&mut req_body);
        let url = format!("{}/api/chat", self.ollama_host.trim_end_matches('/'));
        let resp = self.http.post(&url).json(&req_body).send().await;
        let resp = match resp {
            Ok(r) if r.status().is_success() => r,
            Ok(r) => {
                let status = r.status();
                let body = r.text().await.unwrap_or_default();
                let why = format!("{url} refused the turn: {status} {}", snippet(&body));
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
            Err(error) => {
                let why = format!("{url} could not be reached: {error}");
                return Err(self.record_failure(error_code::PROVIDER_FAILED, why).await);
            }
        };
        let value: serde_json::Value = resp.json().await?;
        let text = value
            .get("message")
            .and_then(|m| m.get("content"))
            .and_then(|c| c.as_str())
            .unwrap_or("")
            .to_string();
        Ok(text)
    }
}

struct ToolBatch {
    records: Vec<ThreadRecord>,
    cancelled: bool,
}

/// A thread left open holds its grant's remaining budget, and the interactive
/// session has no place to await an ending on its way out. This is the
/// backstop: best effort, on whatever runtime is still up. [`CoderRuntimeSession::finish`]
/// is the path that can be awaited and proven, and it clears the id so this
/// does not fire twice.
///
/// It says what the session did where the session knows — a dropped session is
/// not a reason for the record to claim a cancellation that did not happen —
/// and falls back to the disposal only for a thread no turn ever ran on, which
/// is the one case where there is genuinely nothing to report.
impl Drop for CoderRuntimeSession {
    fn drop(&mut self) {
        let Some(thread_id) = self.thread_id.take() else {
            return;
        };
        let Ok(handle) = tokio::runtime::Handle::try_current() else {
            return;
        };
        let token = self.user_token.clone();
        let http = self.http.clone();
        let ending = self
            .outcome
            .as_ref()
            .map(|outcome| (outcome.wire(self.session_usage), true))
            .unwrap_or((serde_json::json!({}), false));
        let url = match ending.1 {
            true => format!("{}/threads/{thread_id}/report", self.api_base),
            false => format!("{}/threads/{thread_id}", self.api_base),
        };
        handle.spawn(async move {
            let (body, reporting) = ending;
            let mut request = match reporting {
                true => http.post(&url).json(&body),
                false => http.delete(&url),
            }
            .timeout(Duration::from_secs(10));
            if let Some(token) = token {
                request = request.bearer_auth(token);
            }
            let _ = request.send().await;
        });
    }
}

/// What one call to a model produced, before the caller decides what it means.
#[derive(Default)]
struct StepAccumulator {
    content: String,
    reasoning: String,
    usage: TurnUsage,
    /// Keyed by the wire's `index` so fragments land in call order.
    tool_calls: std::collections::BTreeMap<usize, (String, String, String)>,
}

impl StepAccumulator {
    /// One OpenAI-shaped streaming chunk.
    ///
    /// `content`, `reasoning` and `tool_calls` interleave inside one `delta`,
    /// and `usage` arrives on a chunk of its own with an empty `choices` array
    /// — which is why usage is read before the choices are.
    fn absorb_openai<F: FnMut(&str)>(&mut self, json: &serde_json::Value, on_chunk: &mut F) {
        if let Some(usage) = json.get("usage") {
            self.usage.add(TurnUsage {
                prompt_tokens: field(usage, "prompt_tokens"),
                completion_tokens: field(usage, "completion_tokens"),
                total_tokens: field(usage, "total_tokens"),
            });
        }

        let Some(delta) = json
            .get("choices")
            .and_then(|v| v.as_array())
            .and_then(|choices| choices.first())
            .and_then(|choice| choice.get("delta"))
        else {
            return;
        };

        if let Some(reasoning) = delta.get("reasoning").and_then(|v| v.as_str()) {
            self.reasoning.push_str(reasoning);
        }
        if let Some(content) = delta.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                on_chunk(content);
                self.content.push_str(content);
            }
        }
        if let Some(calls) = delta.get("tool_calls").and_then(|v| v.as_array()) {
            for call in calls {
                self.absorb_openai_tool_call(call);
            }
        }
    }

    /// One OpenAI-shaped tool-call delta.
    ///
    /// Fragments of one call share an `index` and, after the first chunk, omit
    /// `id` and `name`. A second complete call that reuses that index — GLM
    /// and the live proxy both do this — is a new call, not more of the first.
    /// Concatenating `id` and `name` is what produced `openagentsopenagents`.
    fn absorb_openai_tool_call(&mut self, call: &serde_json::Value) {
        let incoming_id = nonempty_str(call.get("id"));
        let function = call.get("function");
        let incoming_name = function.and_then(|value| nonempty_str(value.get("name")));
        let incoming_args = function
            .and_then(|value| value.get("arguments"))
            .and_then(|value| value.as_str());

        let index = self.tool_call_slot(call.get("index"), incoming_id);
        let entry = self.tool_calls.entry(index).or_default();
        if let Some(id) = incoming_id
            && entry.0.is_empty()
        {
            entry.0 = id.to_string();
        }
        if let Some(name) = incoming_name {
            if entry.1.is_empty() {
                entry.1 = name.to_string();
            } else if name.starts_with(&entry.1) {
                entry.1 = name.to_string();
            }
        }
        if let Some(args) = incoming_args {
            entry.2.push_str(args);
        }
    }

    fn tool_call_slot(
        &self,
        raw_index: Option<&serde_json::Value>,
        incoming_id: Option<&str>,
    ) -> usize {
        let index = parse_tool_call_index(raw_index).unwrap_or(0);
        match (self.tool_calls.get(&index), incoming_id) {
            (Some((held_id, _, _)), Some(id)) if !held_id.is_empty() && held_id != id => {
                self.next_tool_call_slot()
            }
            _ => index,
        }
    }

    fn next_tool_call_slot(&self) -> usize {
        self.tool_calls
            .keys()
            .next_back()
            .map(|key| key + 1)
            .unwrap_or(0)
    }

    /// One OpenResponses-shaped streaming event.
    fn absorb_responses<F: FnMut(&str)>(&mut self, value: &serde_json::Value, on_chunk: &mut F) {
        let Some(event_type) = value.get("type").and_then(|v| v.as_str()) else {
            return;
        };

        match event_type {
            "response.output_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                    if !delta.is_empty() {
                        on_chunk(delta);
                        self.content.push_str(delta);
                    }
                }
            }
            "response.reasoning_summary_text.delta" => {
                if let Some(delta) = value.get("delta").and_then(|v| v.as_str()) {
                    self.reasoning.push_str(delta);
                }
            }
            "response.output_item.done" => {
                if let Some(item) = value.get("item") {
                    if item.get("type").and_then(|v| v.as_str()) == Some("function_call") {
                        if let (Some(call_id), Some(name), Some(arguments), Some(index)) = (
                            item.get("call_id").and_then(|v| v.as_str()),
                            item.get("name").and_then(|v| v.as_str()),
                            item.get("arguments").and_then(|v| v.as_str()),
                            value.get("output_index").and_then(|v| v.as_u64()),
                        ) {
                            self.tool_calls.insert(
                                index as usize,
                                (call_id.to_string(), name.to_string(), arguments.to_string()),
                            );
                        }
                    }
                }
            }
            "response.completed" => {
                if let Some(usage) = value.get("response").and_then(|r| r.get("usage")) {
                    self.usage.add(TurnUsage {
                        prompt_tokens: field(usage, "input_tokens"),
                        completion_tokens: field(usage, "output_tokens"),
                        total_tokens: field(usage, "total_tokens"),
                    });
                }
            }
            _ => {}
        }
    }

    /// One Ollama-shaped streaming line.
    ///
    /// The counts arrive on the `done` line, the model's scratch work under
    /// `thinking`, and a tool call whole rather than in fragments — with its
    /// arguments as an object, where the proxy sends a string.
    fn absorb_ollama<F: FnMut(&str)>(&mut self, json: &serde_json::Value, on_chunk: &mut F) {
        if json.get("done").and_then(|v| v.as_bool()) == Some(true) {
            self.usage.add(TurnUsage {
                prompt_tokens: field(json, "prompt_eval_count"),
                completion_tokens: field(json, "eval_count"),
                total_tokens: field(json, "prompt_eval_count") + field(json, "eval_count"),
            });
        }

        let Some(message) = json.get("message") else {
            return;
        };
        if let Some(thinking) = message.get("thinking").and_then(|v| v.as_str()) {
            self.reasoning.push_str(thinking);
        }
        if let Some(content) = message.get("content").and_then(|v| v.as_str()) {
            if !content.is_empty() {
                on_chunk(content);
                self.content.push_str(content);
            }
        }
        if let Some(calls) = message.get("tool_calls").and_then(|v| v.as_array()) {
            for call in calls {
                let Some(function) = call.get("function") else {
                    continue;
                };
                let index = self.tool_calls.len();
                let name = function
                    .get("name")
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string();
                let arguments = match function.get("arguments") {
                    Some(serde_json::Value::String(raw)) => raw.clone(),
                    Some(value) => value.to_string(),
                    None => "{}".to_string(),
                };
                // Ollama mints no call id and the tool result has to name the
                // call it answers, so one is made from the position. This is a
                // local correlation key, not a server-issued identifier.
                self.tool_calls
                    .insert(index, (format!("local_{index}_{name}"), name, arguments));
            }
        }
    }
}

/// What the grant in an ending's reply had spent, when it says.
///
/// Both endings answer with the same body, so both read it the same way.
/// `None` when the server reported no spend, because there is nothing to
/// reconcile against.
fn grant_spend(body: &serde_json::Value) -> Option<TurnUsage> {
    let spent = body.get("grant").and_then(|grant| grant.get("spent"))?;
    Some(TurnUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: field(spent, "total_tokens"),
    })
}

fn field(value: &serde_json::Value, key: &str) -> u64 {
    value.get(key).and_then(|v| v.as_u64()).unwrap_or(0)
}

fn nonempty_str(value: Option<&serde_json::Value>) -> Option<&str> {
    value
        .and_then(|value| value.as_str())
        .filter(|value| !value.is_empty())
}

fn parse_tool_call_index(value: Option<&serde_json::Value>) -> Option<usize> {
    match value {
        Some(serde_json::Value::Number(number)) => number.as_u64().map(|n| n as usize),
        Some(serde_json::Value::String(text)) => text.parse().ok(),
        _ => None,
    }
}

/// The model that actually answered, from the proxy. The grant may still
/// name the Flash default while a trivial turn was rerouted to Gemini.
fn answered_model(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-openagents-model")
        .and_then(|value| value.to_str().ok())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// Convert the session's messages to the OpenAI chat-completions wire shape.
///
/// Text-only messages keep the compact string form. A user message carrying
/// images becomes a content-block array so providers receive image data rather
/// than a local path they cannot access.
fn chat_completion_messages(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    messages
        .iter()
        .map(|message| {
            let mut value = serde_json::json!({ "role": message.role });
            if message.images.is_empty() {
                if let Some(content) = &message.content {
                    value["content"] = serde_json::Value::String(content.clone());
                }
            } else {
                let mut content = Vec::with_capacity(message.images.len() + 1);
                if let Some(text) = message.content.as_deref().filter(|text| !text.is_empty()) {
                    content.push(serde_json::json!({ "type": "text", "text": text }));
                }
                content.extend(message.images.iter().map(|url| {
                    serde_json::json!({
                        "type": "image_url",
                        "image_url": { "url": url }
                    })
                }));
                value["content"] = serde_json::Value::Array(content);
            }
            if let Some(calls) = &message.tool_calls {
                value["tool_calls"] = serde_json::Value::Array(calls.clone());
            }
            if let Some(call_id) = &message.tool_call_id {
                value["tool_call_id"] = serde_json::Value::String(call_id.clone());
            }
            value
        })
        .collect()
}

/// Convert the session's message list to OpenResponses input items.
///
/// The OpenResponses surface takes a flat list of `input` items — `user`,
/// `assistant`, and `system` messages plus `function_call` and
/// `function_call_output` replay items — so a multi-turn conversation can be
/// sent again on each stateless request.
fn messages_to_responses_input(messages: &[ChatMessage]) -> Vec<serde_json::Value> {
    let mut items = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "system" => {
                if let Some(content) = &message.content {
                    items.push(serde_json::json!({
                        "role": message.role,
                        "content": content
                    }));
                }
            }
            "user" => {
                if message.images.is_empty() {
                    if let Some(content) = &message.content {
                        items.push(serde_json::json!({
                            "role": "user",
                            "content": content
                        }));
                    }
                } else {
                    let mut content = Vec::with_capacity(message.images.len() + 1);
                    if let Some(text) = message.content.as_deref().filter(|text| !text.is_empty()) {
                        content.push(serde_json::json!({
                            "type": "input_text",
                            "text": text
                        }));
                    }
                    content.extend(message.images.iter().map(|url| {
                        serde_json::json!({
                            "type": "input_image",
                            "image_url": url
                        })
                    }));
                    items.push(serde_json::json!({
                        "role": "user",
                        "content": content
                    }));
                }
            }
            "assistant" => {
                let content = message.content.as_deref().unwrap_or("");
                if let Some(calls) = &message.tool_calls {
                    for (i, call) in calls.iter().enumerate() {
                        let call_id = call
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let function = call.get("function").unwrap_or(&serde_json::Value::Null);
                        let name = function
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        let arguments = function
                            .get("arguments")
                            .and_then(|v| v.as_str())
                            .unwrap_or("{}")
                            .to_string();
                        let text = if i == 0 { content } else { "" };
                        items.push(serde_json::json!({
                            "type": "function_call",
                            "content": text,
                            "call_id": call_id,
                            "name": name,
                            "arguments": arguments
                        }));
                    }
                } else if !content.is_empty() {
                    items.push(serde_json::json!({
                        "role": "assistant",
                        "content": content
                    }));
                }
            }
            "tool" => {
                if let Some(content) = &message.content {
                    items.push(serde_json::json!({
                        "type": "function_call_output",
                        "call_id": message.tool_call_id.as_deref().unwrap_or(""),
                        "output": content
                    }));
                }
            }
            _ => {}
        }
    }
    items
}

/// One message in the shape Ollama's chat API takes back.
///
/// The differences from the proxy's shape are small and all load-bearing:
/// `arguments` is an object rather than a string, and a tool result is named by
/// `tool_name` rather than by a call id Ollama never issued.
fn ollama_message(message: &ChatMessage) -> serde_json::Value {
    let mut out = serde_json::json!({
        "role": message.role,
        "content": message.content.clone().unwrap_or_default(),
    });
    if !message.images.is_empty() {
        out["images"] = serde_json::Value::Array(
            message
                .images
                .iter()
                .filter_map(|url| url.split_once(";base64,").map(|(_, data)| data))
                .map(|data| serde_json::Value::String(data.to_string()))
                .collect(),
        );
    }
    if let Some(calls) = &message.tool_calls {
        out["tool_calls"] = serde_json::Value::Array(
            calls
                .iter()
                .map(|call| {
                    let function = call
                        .get("function")
                        .cloned()
                        .unwrap_or(serde_json::json!({}));
                    let name = function
                        .get("name")
                        .cloned()
                        .unwrap_or(serde_json::json!(""));
                    let arguments = match function.get("arguments") {
                        Some(serde_json::Value::String(raw)) => {
                            serde_json::from_str(raw).unwrap_or(serde_json::json!({}))
                        }
                        Some(value) => value.clone(),
                        None => serde_json::json!({}),
                    };
                    serde_json::json!({ "function": { "name": name, "arguments": arguments } })
                })
                .collect(),
        );
    }
    if message.role == "tool" {
        if let Some(id) = &message.tool_call_id {
            // `local_<index>_<name>` — the name is what Ollama matches on.
            let name = id.splitn(3, '_').nth(2).unwrap_or(id);
            out["tool_name"] = serde_json::json!(name);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every id a lane can send is resolved from the catalog it was handed.
    fn served(ids: &[&str]) -> Vec<ServedModel> {
        ids.iter()
            .map(|id| ServedModel {
                id: (*id).to_string(),
                available: true,
                default: false,
            })
            .collect()
    }

    /// The designed path: the primary is served, so the primary answers.
    #[test]
    fn a_lane_opens_on_its_primary_when_the_catalog_serves_it() {
        let catalog = served(&["glm-5.3-flash", "gemini-3.7-flash", "openrouter/free"]);
        assert_eq!(
            Lane::from_str("flash").resolve(&catalog).unwrap(),
            Some("glm-5.3-flash".to_string())
        );
        let catalog = served(&["thinkingmachines/inkling", "openrouter/free"]);
        assert_eq!(
            Lane::from_str("free").resolve(&catalog).unwrap(),
            Some("thinkingmachines/inkling".to_string())
        );
    }

    /// The primary has left the catalog, so the declared fallback answers.
    #[test]
    fn a_lane_falls_back_when_its_primary_is_not_served() {
        let catalog = served(&["gemini-3.7-flash", "openrouter/free"]);
        assert_eq!(
            Lane::from_str("flash").resolve(&catalog).unwrap(),
            Some("gemini-3.7-flash".to_string())
        );
        assert_eq!(
            Lane::from_str("free").resolve(&catalog).unwrap(),
            Some("openrouter/free".to_string())
        );
    }

    /// The case that put us here: **neither** id is served.
    ///
    /// The lane refuses and names what the deployment does serve. It does not
    /// pin a compiled default. A build that quietly fell back to a constant
    /// here would pass both tests above and reintroduce the exact failure
    /// this table was rewritten to delete — with a label that still looked
    /// right.
    #[test]
    fn a_lane_refuses_rather_than_pin_a_model_the_catalog_does_not_serve() {
        // A catalog that serves real models, none of which any lane prefers.
        let catalog = served(&["gpt-4o", "some-other-model"]);
        for name in ["flash", "free"] {
            let error = Lane::from_str(name)
                .resolve(&catalog)
                .expect_err("a lane whose candidates are all gone must refuse");
            let text = error.to_string();
            for id in Lane::from_str(name).spec().unwrap().candidates {
                assert!(
                    text.contains(id),
                    "the refusal does not say '{id}' was wanted: {text}"
                );
            }
            assert!(
                text.contains("gpt-4o") && text.contains("some-other-model"),
                "the refusal does not name what is served: {text}"
            );
        }
    }

    #[test]
    fn flash_opens_on_sol_when_that_is_what_the_catalog_serves() {
        let catalog = served(&["gpt-5.6-sol", "gpt-5.6-terra", "gpt-5.6-luna"]);
        assert_eq!(
            Lane::from_str("flash").resolve(&catalog).unwrap(),
            Some("gpt-5.6-sol".to_string())
        );
    }

    /// An empty catalog is a refusal too, not a licence to guess.
    #[test]
    fn a_lane_refuses_against_an_empty_catalog() {
        for name in ["flash", "free"] {
            assert!(Lane::from_str(name).resolve(&[]).is_err());
        }
    }

    /// Listed but with no provider credential is not "served".
    #[test]
    fn an_unavailable_model_does_not_satisfy_a_lane() {
        let catalog = vec![ServedModel {
            id: "glm-5.3-flash".to_string(),
            available: false,
            default: false,
        }];
        assert!(Lane::from_str("flash").resolve(&catalog).is_err());
    }

    /// No hosted lane holds a compiled id any more.
    ///
    /// The local lane is exempt on purpose: `resolve` answers `Ok(None)` for
    /// it because it names no server model at all — it resolves against
    /// `/api/tags` and mints no grant. `Ok(None)` is not a compiled pin.
    #[test]
    fn no_lane_pins_a_model_without_asking_the_catalog() {
        for lane in LANES.iter().filter(|lane| lane.name != "local") {
            let resolved = Lane::from_str(lane.name).resolve(&[]);
            assert!(
                resolved.is_err(),
                "lane '{}' produced {resolved:?} from an empty catalog, so it is \
                 pinning something that was compiled in rather than served",
                lane.name
            );
        }
    }

    /// A fresh session does not open on `ox-alpha`, and `auto` is not a lane.
    #[test]
    fn the_default_lane_is_flash_and_the_retired_names_are_unclaimed() {
        assert_eq!(Lane::default(), Lane::Flash);
        assert_eq!(Lane::from_str(""), Lane::Flash);
        assert_eq!(Lane::default().label(), "Coder Flash");
        // A fresh session opens on Flash, and Flash is not `ox-alpha`.
        assert_ne!(Lane::default(), Lane::Named("ox-alpha".to_string()));
        // `auto`, `pro`, and `ox-alpha` name no lane. They are carried as
        // directly-named models so the catalog refuses them by name, rather
        // than aliased onto a surviving lane.
        for retired in ["auto", "pro", "coder-pro", "ox-alpha", "ox", "openagents"] {
            assert_eq!(
                Lane::from_str(retired),
                Lane::Named(retired.to_string()),
                "'{retired}' still resolves to a lane"
            );
        }
    }

    /// A model id means that model, whatever a tier is called this month.
    #[test]
    fn a_model_id_is_never_read_as_a_tier() {
        for id in [
            "gemini-3.7-flash",
            "gemini",
            "gemini-flash",
            "gpt-5.6-luna",
            "luna",
        ] {
            assert_eq!(
                Lane::from_str(id),
                Lane::Named(id.to_string()),
                "'{id}' is being read as a tier, so typing it can return another model"
            );
        }
        // And a directly-named model pins exactly what was typed.
        assert_eq!(
            Lane::from_str("gemini-3.7-flash").resolve(&[]).unwrap(),
            Some("gemini-3.7-flash".to_string())
        );
    }

    /// Shift+tab walks the lane table, so restoring a lane is one entry.
    #[test]
    fn the_cycle_walks_every_lane_and_returns_to_where_it_started() {
        let start = Lane::default();
        let mut seen = vec![start.label()];
        let mut lane = start.clone();
        for _ in 0..LANES.len() - 1 {
            lane = lane.cycle();
            assert!(
                !seen.contains(&lane.label()),
                "the cycle repeated {} before visiting every lane",
                lane.label()
            );
            seen.push(lane.label());
        }
        assert_eq!(seen.len(), LANES.len());
        assert_eq!(lane.cycle(), start, "the cycle does not close");
        assert_eq!(seen, vec!["Coder Flash", "Coder Free", "Coder Local"]);
    }

    /// With the gate closed, shift+tab never lands on the local lane (#291).
    #[test]
    fn the_cycle_skips_the_local_lane_when_the_probe_found_nothing() {
        let start = Lane::default();
        assert_eq!(start.cycle_gated(None), Lane::Free);
        assert_eq!(start.cycle_gated(None).cycle_gated(None), start);
        // A session sitting on local when the gate is closed still moves; a
        // reader asked to change lanes, not to stay put.
        let local = Lane::from_str("local");
        assert_eq!(local.cycle_gated(None), Lane::Flash);
    }

    /// With the gate open the local lane is a full walk member, resolved to
    /// the exact tag the probe found — not a lane-shaped promise (#292).
    #[test]
    fn the_cycle_includes_the_local_lane_resolved_to_the_probed_tag() {
        let start = Lane::default();
        let tag = Some("qwen3.8:27b-mtp-q8_0".to_string());
        let local = start.cycle_gated(tag.clone()).cycle_gated(tag.clone());
        assert_eq!(local, Lane::Local("qwen3.8:27b-mtp-q8_0".to_string()));
        assert_eq!(local.label(), "Coder Local (qwen3.8:27b-mtp-q8_0)");
        assert_eq!(local.name(), "ollama:qwen3.8:27b-mtp-q8_0");
        assert_eq!(local.cycle_gated(tag), start, "the cycle does not close");
    }

    /// The gate compares the family segment whole, not by substring: `qwen3`
    /// does not satisfy a `qwen3.8` gate, though Rust's `starts_with` would
    /// happily claim otherwise — which is why the probe splits the segment.
    #[test]
    fn the_gate_is_family_not_substring() {
        assert_ne!(
            Some(CoderRuntimeSession::CYCLE_GATE_FAMILY),
            "qwen3:0.6b".split(':').next()
        );
        // The trap itself, pinned: `"qwen3.8".starts_with("qwen3")` is true.
        assert!(CoderRuntimeSession::CYCLE_GATE_FAMILY.starts_with("qwen3"));
    }

    /// The local lane's spec resolves against `/api/tags`, never the catalog:
    /// a `resolve` of `Ok(None)` is what keeps a grant from being minted.
    #[test]
    fn the_local_lane_spec_carries_no_catalog_candidates() {
        let spec = Lane::from_str("local")
            .spec()
            .expect("local is a table member");
        assert_eq!(spec.name, "local");
        assert_eq!(spec.label, "Coder Local");
        assert!(spec.candidates.is_empty());
        assert_eq!(Lane::from_str("local").resolve(&[]).unwrap(), None);
    }

    /// The probe answers the model a bare local lane would resolve to, or
    /// `None` for every kind of absence — server down, empty library, and a
    /// refusal all read the same way (#291).
    ///
    /// Real sockets, the way the integration tests prove the wire: one
    /// tokio listener per case, no mocking between the probe and the bytes.
    #[tokio::test]
    async fn the_local_lane_probe_names_the_most_recent_model_or_nothing() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        use tokio::net::TcpListener;

        async fn serve(body: &'static str) -> String {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let port = listener.local_addr().unwrap().port();
            tokio::spawn(async move {
                if let Ok((mut socket, _)) = listener.accept().await {
                    let mut buf = [0u8; 4096];
                    let _ = socket.read(&mut buf).await;
                    let response = format!(
                        "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\n\
                         content-length: {}\r\nconnection: close\r\n\r\n{body}",
                        body.len()
                    );
                    let _ = socket.write_all(response.as_bytes()).await;
                }
            });
            format!("http://127.0.0.1:{port}")
        }

        let two = serve(
            r#"{"models":[
                {"name":"older:1b","modified_at":"2026-01-01T00:00:00Z"},
                {"name":"qwen3:0.6b","modified_at":"2026-08-25T15:08:15Z"},
                {"name":"qwen3.8:27b-mtp-q8_0","modified_at":"2026-08-24T02:05:03Z"}]}"#,
        )
        .await;
        let found = CoderRuntimeSession::probe_local_lane_at(&two)
            .await
            .expect("a live server with a gated model is found");
        assert_eq!(
            found, "qwen3.8:27b-mtp-q8_0",
            "the gated family is resolved to its exact tag, newer modified_at wins"
        );

        // A library without the gated family is absence for the walk's
        // purposes (#292) — the same None an empty library gives.
        let other_family = serve(
            r#"{"models":[
                {"name":"qwen3:0.6b","modified_at":"2026-08-25T15:08:15Z"},
                {"name":"llama9:7b","modified_at":"2026-08-26T10:00:00Z"}]}"#,
        )
        .await;
        assert_eq!(
            CoderRuntimeSession::probe_local_lane_at(&other_family).await,
            None,
            "a loaded model of another family does not light the lane"
        );

        let empty = serve(r#"{"models":[]}"#).await;
        assert_eq!(
            CoderRuntimeSession::probe_local_lane_at(&empty).await,
            None,
            "an empty library is absence, not an error"
        );

        // No server at all: the same `None`, with no error to surface.
        assert_eq!(
            CoderRuntimeSession::probe_local_lane_at("http://127.0.0.1:1").await,
            None
        );
    }

    /// A lane the reader pinned by hand is not thrown away by a keystroke.
    ///
    /// A local *model* (`ollama:qwen3`) is such a pin — a named tag, not a
    /// walk member — so a keystroke leaves it alone. Bare `local` is the
    /// walk member, and follows the cycle (#291).
    #[test]
    fn the_cycle_leaves_a_directly_named_lane_alone() {
        let named = Lane::from_str("some-model");
        assert_eq!(named.cycle(), named);
        let local = Lane::from_str("ollama:qwen3");
        assert_eq!(local.cycle(), local);
    }

    #[test]
    fn an_unknown_name_is_carried_as_named_rather_than_becoming_the_default() {
        assert_eq!(Lane::from_str("bogus"), Lane::Named("bogus".to_string()));
        assert_eq!(Lane::from_str("claude"), Lane::Named("claude".to_string()));
        assert_ne!(Lane::from_str("bogus"), Lane::default());
    }

    #[test]
    fn the_local_lane_is_parsed_from_both_spellings() {
        assert_eq!(Lane::from_str("local"), Lane::Local(String::new()));
        assert_eq!(
            Lane::from_str("ollama:qwen3:0.6b"),
            Lane::Local("qwen3:0.6b".to_string())
        );
        assert!(Lane::from_str("ollama:qwen3:0.6b").is_local());
        assert!(!Lane::from_str("flash").is_local());
        // The local lane names no catalog model, because no grant carries it.
        assert_eq!(Lane::from_str("ollama:qwen3").resolve(&[]).unwrap(), None);
    }

    #[test]
    fn every_lane_has_a_name_and_a_label() {
        assert_eq!(Lane::from_str("flash").tier(), Some("flash"));
        assert_eq!(Lane::from_str("free").tier(), Some("free"));
        assert_eq!(Lane::from_str("local").tier(), Some("local"));
        assert_eq!(Lane::from_str("bogus").tier(), None);
        assert_eq!(Lane::from_str("flash").label(), "Coder Flash");
        assert_eq!(Lane::from_str("free").label(), "Coder Free");
        assert_eq!(
            Lane::from_str("ollama:qwen3:0.6b").label(),
            "Coder Local (qwen3:0.6b)"
        );
        for lane in [
            Lane::Flash,
            Lane::Free,
            Lane::Named("model/one".to_string()),
            Lane::Local(String::new()),
            Lane::Local("qwen3:0.6b".to_string()),
        ] {
            assert_eq!(Lane::from_str(&lane.name()), lane);
        }
    }

    #[test]
    fn the_admitted_lane_sentence_names_every_way_in() {
        let sentence = admitted_lanes();
        for fragment in ["flash", "free", "ollama:<model>"] {
            assert!(
                sentence.contains(fragment),
                "'{fragment}' missing from: {sentence}"
            );
        }
        // It recommends lane names, never a catalog id it has not checked.
        for guess in [
            "glm-5.3-flash",
            "gpt-5.6-luna",
            "gemini-3.7-flash",
            "ox-alpha",
        ] {
            assert!(
                !sentence.contains(guess),
                "the sentence recommends '{guess}', which it has not confirmed is served"
            );
        }
    }

    #[test]
    fn the_proxy_header_names_the_model_that_answered() {
        let mut headers = HeaderMap::new();
        headers.insert(
            reqwest::header::HeaderName::from_static("x-openagents-model"),
            HeaderValue::from_static("gemini-3.7-flash"),
        );
        assert_eq!(
            answered_model(&headers).as_deref(),
            Some("gemini-3.7-flash")
        );
        assert_eq!(answered_model(&HeaderMap::new()), None);
    }

    #[test]
    fn usage_sums_across_the_steps_of_a_turn() {
        let mut usage = TurnUsage::default();
        assert!(!usage.reported());
        usage.add(TurnUsage {
            prompt_tokens: 99,
            completion_tokens: 17,
            total_tokens: 116,
        });
        usage.add(TurnUsage {
            prompt_tokens: 140,
            completion_tokens: 8,
            total_tokens: 148,
        });
        assert!(usage.reported());
        assert_eq!(usage.line(), "239 prompt + 25 completion = 264 tokens");
    }

    /// The exact frames a live `glm-5.3-flash` turn sent, in order: reasoning
    /// first, then the answer, then usage on a chunk with no choices at all.
    #[test]
    fn a_real_proxy_stream_yields_the_answer_the_reasoning_and_the_counts() {
        let frames = [
            r#"{"choices":[{"delta":{"reasoning":"The user wants "},"index":0}],"model":"glm-5.3-flash"}"#,
            r#"{"choices":[{"delta":{"reasoning":"PONG."},"index":0}],"model":"glm-5.3-flash"}"#,
            r#"{"choices":[{"delta":{"content":"PONG"},"index":0}],"model":"glm-5.3-flash"}"#,
            r#"{"choices":[{"delta":{},"finish_reason":"stop","index":0}],"model":"glm-5.3-flash"}"#,
            r#"{"choices":[],"model":"glm-5.3-flash","usage":{"completion_tokens":17,"prompt_tokens":99,"total_tokens":116}}"#,
        ];
        let mut step = StepAccumulator::default();
        let mut streamed = String::new();
        {
            let mut sink = |chunk: &str| streamed.push_str(chunk);
            for frame in frames {
                step.absorb_openai(&serde_json::from_str(frame).unwrap(), &mut sink);
            }
        }

        assert_eq!(step.content, "PONG");
        // Reasoning is parsed and kept, and stays off the transcript.
        assert_eq!(step.reasoning, "The user wants PONG.");
        assert_eq!(streamed, "PONG", "reasoning leaked into the answer");
        assert_eq!(step.usage.total_tokens, 116);
        assert_eq!(step.usage.prompt_tokens, 99);
        assert_eq!(step.usage.completion_tokens, 17);
    }

    fn absorb_tool_frames(frames: &[&str]) -> Vec<(String, String, String)> {
        let mut step = StepAccumulator::default();
        let mut sink = |_: &str| {};
        for frame in frames {
            step.absorb_openai(&serde_json::from_str(frame).unwrap(), &mut sink);
        }
        step.tool_calls.into_values().collect()
    }

    #[test]
    fn tool_call_fragments_are_joined_by_their_index() {
        let calls = absorb_tool_frames(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"read_file","arguments":"{\"path\":"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"arguments":"\"a.txt\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":1,"id":"call_b","function":{"name":"ls","arguments":"{}"}}]}}]}"#,
        ]);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "call_a");
        assert_eq!(calls[0].1, "read_file");
        assert_eq!(calls[0].2, r#"{"path":"a.txt"}"#);
        assert_eq!(calls[1].1, "ls");
    }

    /// The live proxy emits every complete tool call with `index: 0`. GLM also
    /// puts two finished calls on that same slot. Concatenating them is how
    /// `openagentsopenagents` and `skillbash` reached the tool runner.
    #[test]
    fn parallel_tool_calls_that_share_an_index_stay_separate() {
        let calls = absorb_tool_frames(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_e36103fd27e348c78f6296c4","function":{"name":"openagents","arguments":"{\"name\":\"openagents-cli\"}"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_b65f63297876475a819300bf","function":{"name":"openagents","arguments":"{\"command\":\"git status\"}"}}]}}]}"#,
        ]);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].0, "call_e36103fd27e348c78f6296c4");
        assert_eq!(calls[0].1, "openagents");
        assert_eq!(calls[0].2, r#"{"name":"openagents-cli"}"#);
        assert_eq!(calls[1].0, "call_b65f63297876475a819300bf");
        assert_eq!(calls[1].1, "openagents");
        assert_eq!(calls[1].2, r#"{"command":"git status"}"#);
    }

    #[test]
    fn a_resent_tool_name_is_not_concatenated() {
        let calls = absorb_tool_frames(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"id":"call_a","function":{"name":"openagents","arguments":"{"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":0,"function":{"name":"openagents","arguments":"\"x\":1}"}}]}}]}"#,
        ]);
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "call_a");
        assert_eq!(calls[0].1, "openagents");
        assert_eq!(calls[0].2, r#"{"x":1}"#);
    }

    #[test]
    fn tool_calls_without_an_index_are_split_by_id() {
        let calls = absorb_tool_frames(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_a","function":{"name":"skill","arguments":"{\"name\":\"openagents-cli\"}"}},{"id":"call_b","function":{"name":"bash","arguments":"{\"command\":\"git status\"}"}}]}}]}"#,
        ]);
        assert_eq!(calls.len(), 2);
        assert_eq!(
            calls[0],
            (
                "call_a".into(),
                "skill".into(),
                r#"{"name":"openagents-cli"}"#.into()
            )
        );
        assert_eq!(
            calls[1],
            (
                "call_b".into(),
                "bash".into(),
                r#"{"command":"git status"}"#.into()
            )
        );
    }

    #[test]
    fn a_string_tool_call_index_is_still_an_index() {
        let calls = absorb_tool_frames(&[
            r#"{"choices":[{"delta":{"tool_calls":[{"index":"0","id":"call_a","function":{"name":"skill","arguments":"{}"}}]}}]}"#,
            r#"{"choices":[{"delta":{"tool_calls":[{"index":"1","id":"call_b","function":{"name":"bash","arguments":"{}"}}]}}]}"#,
        ]);
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].1, "skill");
        assert_eq!(calls[1].1, "bash");
    }

    /// The exact frames a live `qwen3:0.6b` turn sent.
    #[test]
    fn a_real_ollama_stream_yields_the_answer_and_the_counts() {
        let frames = [
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","thinking":"short"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":"PO"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":"NG"},"done":false}"#,
            r#"{"model":"qwen3:0.6b","message":{"role":"assistant","content":""},"done":true,"done_reason":"stop","prompt_eval_count":19,"eval_count":28}"#,
        ];
        let mut step = StepAccumulator::default();
        let mut streamed = String::new();
        {
            let mut sink = |chunk: &str| streamed.push_str(chunk);
            for frame in frames {
                step.absorb_ollama(&serde_json::from_str(frame).unwrap(), &mut sink);
            }
        }

        assert_eq!(streamed, "PONG");
        assert_eq!(step.content, "PONG");
        assert_eq!(step.reasoning, "short");
        assert_eq!(step.usage.prompt_tokens, 19);
        assert_eq!(step.usage.completion_tokens, 28);
        assert_eq!(step.usage.total_tokens, 47);
    }

    #[test]
    fn an_ollama_tool_call_arrives_whole_with_object_arguments() {
        let frame = r#"{"message":{"role":"assistant","tool_calls":[{"function":{"name":"read_file","arguments":{"path":"a.txt"}}}]},"done":false}"#;
        let mut step = StepAccumulator::default();
        let mut sink = |_: &str| {};
        step.absorb_ollama(&serde_json::from_str(frame).unwrap(), &mut sink);
        let calls: Vec<_> = step.tool_calls.into_values().collect();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].1, "read_file");
        let parsed: serde_json::Value = serde_json::from_str(&calls[0].2).unwrap();
        assert_eq!(parsed["path"], "a.txt");
    }

    #[test]
    fn a_tool_result_goes_back_to_ollama_named_by_its_tool() {
        let message = ChatMessage {
            role: "tool".to_string(),
            content: Some("hello".to_string()),
            tool_calls: None,
            tool_call_id: Some("local_0_read_file".to_string()),
            images: Vec::new(),
        };
        let wire = ollama_message(&message);
        assert_eq!(wire["role"], "tool");
        assert_eq!(wire["content"], "hello");
        assert_eq!(wire["tool_name"], "read_file");
    }

    #[test]
    fn an_assistant_tool_call_reaches_ollama_with_its_arguments_parsed() {
        let message = ChatMessage {
            role: "assistant".to_string(),
            content: None,
            tool_calls: Some(vec![serde_json::json!({
                "id": "call_a",
                "type": "function",
                "function": { "name": "read_file", "arguments": "{\"path\":\"a.txt\"}" }
            })]),
            tool_call_id: None,
            images: Vec::new(),
        };
        let wire = ollama_message(&message);
        assert_eq!(wire["tool_calls"][0]["function"]["name"], "read_file");
        assert_eq!(
            wire["tool_calls"][0]["function"]["arguments"]["path"], "a.txt",
            "Ollama takes arguments as an object, not as the proxy's string"
        );
    }

    #[test]
    fn image_attachments_use_each_transports_multimodal_shape() {
        let message = ChatMessage {
            role: "user".to_string(),
            content: Some("What is shown? [Image #1]".to_string()),
            tool_calls: None,
            tool_call_id: None,
            images: vec!["data:image/png;base64,aW1hZ2U=".to_string()],
        };

        let chat = chat_completion_messages(std::slice::from_ref(&message));
        assert_eq!(chat[0]["content"][0]["type"], "text");
        assert_eq!(chat[0]["content"][1]["type"], "image_url");
        assert_eq!(
            chat[0]["content"][1]["image_url"]["url"],
            "data:image/png;base64,aW1hZ2U="
        );

        let responses = messages_to_responses_input(std::slice::from_ref(&message));
        assert_eq!(responses[0]["content"][1]["type"], "input_image");
        assert_eq!(
            responses[0]["content"][1]["image_url"],
            "data:image/png;base64,aW1hZ2U="
        );

        let ollama = ollama_message(&message);
        assert_eq!(ollama["images"][0], "aW1hZ2U=");
    }

    /// The status and the error code cannot disagree, whichever way round.
    ///
    /// The server refuses an incoherent pair in a changeset and again in a
    /// database constraint, and the point of the constructors is that this
    /// client never sends the server one to refuse. There is no way to build a
    /// success carrying an error code or a failure carrying none, so this
    /// walks every ending the CLI can file and asserts the pair.
    #[test]
    fn a_reported_outcome_and_its_error_code_always_agree() {
        let endings = [
            ThreadOutcome::succeeded("it answered"),
            ThreadOutcome::failed(error_code::PROVIDER_FAILED, "the proxy refused it"),
            ThreadOutcome::failed(error_code::STREAM_BROKEN, "the reply stopped"),
            ThreadOutcome::failed(error_code::MAX_STEPS, "no answer in 100 steps"),
            ThreadOutcome::failed(error_code::TURN_FAILED, "something else"),
            ThreadOutcome::interrupted("ctrl-c"),
            ThreadOutcome::no_turn(),
        ];

        for ending in endings {
            let body = ending.wire(TurnUsage::default());
            let coded = body.get("error_code").and_then(|v| v.as_str());
            match ending.status() {
                SUCCEEDED => assert_eq!(
                    coded, None,
                    "a success named an error code, which the server refuses: {body}"
                ),
                other => {
                    assert!(
                        other == FAILED || other == CANCELLED,
                        "'{other}' is not one of the server's terminal statuses"
                    );
                    assert!(
                        coded.is_some_and(|code| !code.trim().is_empty()),
                        "a thread that did not succeed named no error code: {body}"
                    );
                }
            }
            assert_eq!(coded, ending.error_code());
            assert!(
                !body["report"]
                    .as_str()
                    .unwrap_or_default()
                    .trim()
                    .is_empty(),
                "a blank report is refused: {body}"
            );
        }
    }

    /// An interruption is a cancellation, and it is never a success.
    #[test]
    fn an_interruption_is_recorded_as_the_cancellation_it_was() {
        let stopped = ThreadOutcome::interrupted("stopped before finishing");
        assert_eq!(stopped.status(), CANCELLED);
        assert_eq!(stopped.error_code(), Some(error_code::INTERRUPTED));
        assert_ne!(stopped.status(), SUCCEEDED);
        assert_eq!(stopped.report(), "stopped before finishing");
    }

    /// A report is never blank and never over the server's bound.
    ///
    /// A model that answered with nothing still ended a session; filing a
    /// blank report is refused outright, so the whole ending would fail over
    /// an empty answer and the thread would be left open.
    #[test]
    fn a_report_is_always_something_the_server_will_take() {
        let empty = ThreadOutcome::succeeded("   ");
        assert_eq!(empty.report(), "The session answered.");

        let long = ThreadOutcome::succeeded(&"é".repeat(MAX_REPORT_BYTES));
        assert!(
            long.report().len() <= MAX_REPORT_BYTES,
            "a report of {} bytes is over the server's bound",
            long.report().len()
        );
        assert!(long.report().ends_with("[report truncated]"));
        // Cut on a character boundary: `é` is two bytes, so a naive cut splits
        // one and the string is not valid UTF-8 to begin with.
        assert!(long.report().starts_with('é'));
    }

    /// The local lane's system prompt must not promise a metered proxy, and the
    /// thread lane's must not promise that nothing leaves the machine.
    #[test]
    fn each_lane_tells_the_model_where_it_is_running() {
        let local = CoderRuntimeSession::new(
            Lane::Local("qwen3".to_string()),
            Some("http://127.0.0.1:1/api/v1".to_string()),
            None,
            HarnessToolRegistry::new(Some(std::env::temp_dir())),
        );
        assert!(local.build_system_prompt(&[]).contains("on this machine"));

        let hosted = CoderRuntimeSession::new(
            Lane::default(),
            Some("http://127.0.0.1:1/api/v1".to_string()),
            None,
            HarnessToolRegistry::new(Some(std::env::temp_dir())),
        );
        assert!(
            hosted
                .build_system_prompt(&[])
                .contains("OpenAgents inference proxy")
        );
    }

    #[test]
    fn the_system_prompt_names_the_session_working_directory() {
        let workspace = std::path::PathBuf::from("/workspace/verified-cwd");
        let session = CoderRuntimeSession::new(
            Lane::default(),
            None,
            None,
            HarnessToolRegistry::new(Some(workspace.clone())),
        );

        let prompt = session.build_system_prompt(&[]);
        assert!(
            prompt.contains(&format!(
                "The session's working directory is `{}`",
                workspace.display()
            )),
            "the system prompt did not name the runtime working directory: {prompt}"
        );
        assert!(
            prompt.contains("do not invent another one"),
            "the system prompt did not forbid made-up working directories: {prompt}"
        );
        assert!(
            prompt.contains("wait for its result before giving the reader a final answer"),
            "the system prompt did not require a post-tool synthesis: {prompt}"
        );
    }
}
