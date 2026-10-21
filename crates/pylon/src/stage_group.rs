//! Stage groups: several managed stage workers that serve one model together.
//!
//! A model too large for one process runs as an ordered group of stages.
//! Each stage is a managed worker in its own process group, started, killed,
//! and reaped by the same rules as the serial worker in [`crate::serial`].
//! The front (this module) owns the request, the step order, the sampling
//! settings, and the token stream. Stage-state messages (#12396,
//! `psionic_models::stage_state`) travel from stage to stage; each stage
//! acknowledges its step to the front, and the last stage samples with the
//! front's settings and returns only the token id. A stage that takes state
//! announces a `listen` address; the front passes it to the stage before as
//! `next` with each step, and passes the previous token to the first stage as
//! `token`.
//!
//! - **Readiness.** The group is ready only when every stage is running,
//!   has announced the expected weights digest, runtime, protocol version,
//!   and group generation, and has passed a known-answer check at that
//!   generation. [`StageGroupStatus::ready`] is the only signal that may make
//!   the group routable as a complete model; [`StageGroup`] reports it to the
//!   group head beacon through [`crate::group::GroupReadiness`].
//! - **Fencing.** Replacing a stage bumps the group generation. Surviving
//!   stages receive a fence, drop their request state, and announce
//!   themselves again. [`Fence`] accepts only the next expected
//!   acknowledgment of the current request at the current generation;
//!   anything else is refused before it is used.
//! - **Failure.** A crash, a lost connection, a missed health check, or a
//!   step that makes no progress fails the request at once, with the number
//!   of tokens already delivered. A request is never replayed.
//! - **Cancellation.** When the caller goes away, the admission slot stays
//!   occupied until every stage has confirmed it stopped or has been killed
//!   and reaped.
//! - **Restarts.** Consecutive failures share one [`RestartBudget`] with the
//!   serial worker's rules: a completed request resets it, readiness does
//!   not, and an exhausted budget stops the group until the operator acts.
//!
//! The control wire between front and stages is line-delimited JSON over a
//! loopback connection the stage opens to the front. Its identity fields are
//! W9's: group generation, request ID, attempt, stage index, and step
//! ([`StepIdentity`]). A stage maps the front's `fence` to
//! `StageWorker::advance_generation` and `cancel` to `StageWorker::reset`, so
//! its `StageInbox` refuses stale stage state by the same rules this fence
//! applies to acknowledgments.
//!
//! **Where stage state travels.** Under [`Transport::Front`] (W10) every
//! stage acknowledges every step to the front, and the front starts the
//! next stage. Under [`Transport::Direct`] (W12, #12399) the front sends
//! each step only to the first stage; each stage sends its stage state
//! straight to the next over an authenticated `stage_link` connection, and
//! only the last stage answers the front, with the token. The front still
//! owns admission, sampling, cancellation, generation fencing, and
//! readiness: it gives every stage a fresh link key for each generation
//! (`link`), the group is ready only once every stage has linked to the
//! next (`linked`), a stage that loses a link says so (`link_failed`) and
//! the front replaces the stage it names, and each stage reports the steps
//! it passed on (`forwarded`), so a step that stops between two stages
//! fails the stage that holds it.
//!
//! **What the last stage returns.** It samples and returns the token id, not
//! logits or a top-k. W9 measured full F32 logits at 517,280 bytes per token,
//! more than every forward hop together; a token id is four bytes. The front
//! still owns sampling: it sends temperature, top-p, and seed with each step
//! to the last stage, and every token reaches the caller through the front.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::engine::Sampling;

#[cfg(unix)]
use crate::serial::{OwnedChild, RestartBudget};

/// The stage protocol version this front speaks.
pub const PROTOCOL: u32 = 1;

/// The identity fields of a stage message, as W9's
/// `psionic_models::stage_state::StageMessage` carries them.
///
/// [`Fence`] checks these before anything else in the message is used, so a
/// stale, duplicate, missing, reordered, or cross-request message is refused.
pub trait StepIdentity {
    /// The group generation the sender believes is current.
    fn generation(&self) -> u64;
    /// The sending stage's index in step order.
    fn stage(&self) -> u16;
    /// The request it belongs to.
    fn request_id(&self) -> &str;
    /// The request's attempt number.
    fn attempt(&self) -> u32;
    /// The decoding step it acknowledges.
    fn step(&self) -> u64;
}

/// A stage's acknowledgment that it finished one step of one request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepAck {
    pub stage: u16,
    pub generation: u64,
    pub request_id: String,
    pub attempt: u32,
    pub step: u64,
    /// The sampled token id. Only the last stage sends one, and it must.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<u32>,
    /// The token's text, when the last stage decodes it. A group whose
    /// stages send no text answers with nothing but token ids.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// The last stage sampled the end of the answer: the request completes
    /// with this token.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub end: bool,
    /// The reasoning text this token adds, when the last stage parses the
    /// answer with the model's output parser.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    /// The final-answer text this token adds, parsed the same way. A
    /// literal `</think>` in the text stays text; only the model's special
    /// token ends the reasoning.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    /// How the answer ended, on its last token: `stop` for the end of the
    /// answer, `length` when the front's output limit cut it off.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub finish: Option<String>,
    /// The step's own time on the stage, in microseconds, when the stage
    /// reports it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compute_us: Option<u64>,
}

/// Parsed answer text from one token.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AnswerPiece {
    pub reasoning: String,
    pub content: String,
}

impl StepIdentity for StepAck {
    fn generation(&self) -> u64 {
        self.generation
    }
    fn stage(&self) -> u16 {
        self.stage
    }
    fn request_id(&self) -> &str {
        &self.request_id
    }
    fn attempt(&self) -> u32 {
        self.attempt
    }
    fn step(&self) -> u64 {
        self.step
    }
}

/// Why a stage message was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// From an earlier group generation.
    StaleGeneration { got: u64, current: u64 },
    /// From a generation the front has not started.
    FutureGeneration { got: u64, current: u64 },
    /// No request is running.
    NoRequest,
    /// For a different request.
    OtherRequest { got: String, current: String },
    /// For a different attempt of the running request.
    OtherAttempt { got: u32, current: u32 },
    /// From a stage that is not in the group.
    UnknownStage(u16),
    /// A message the front already accepted.
    Duplicate { stage: u16, step: u64 },
    /// A message that skips one the front still waits for.
    OutOfOrder {
        stage: u16,
        step: u64,
        expected_stage: u16,
        expected_step: u64,
    },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StaleGeneration { got, current } => {
                write!(f, "generation {got} is older than {current}")
            }
            Self::FutureGeneration { got, current } => {
                write!(f, "generation {got} is newer than {current}")
            }
            Self::NoRequest => write!(f, "no request is running"),
            Self::OtherRequest { got, current } => {
                write!(f, "request {got} is not the running request {current}")
            }
            Self::OtherAttempt { got, current } => {
                write!(f, "attempt {got} is not the running attempt {current}")
            }
            Self::UnknownStage(stage) => write!(f, "stage {stage} is not in this group"),
            Self::Duplicate { stage, step } => {
                write!(f, "stage {stage} already acknowledged step {step}")
            }
            Self::OutOfOrder {
                stage,
                step,
                expected_stage,
                expected_step,
            } => write!(
                f,
                "stage {stage} step {step} arrived before stage {expected_stage} step {expected_step}"
            ),
        }
    }
}

/// What an accepted acknowledgment means for the request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accepted {
    /// The next stage, by index, runs the same step.
    Next(usize),
    /// Every stage finished this step; its token may go to the caller.
    StepDone(u64),
}

/// Accepts exactly the next acknowledgment of one request at one generation.
#[derive(Debug, Clone)]
pub struct Fence {
    generation: u64,
    stages: u16,
    /// Only the last stage acknowledges (W12's direct transport).
    direct: bool,
    request: Option<(String, u32)>,
    step: u64,
    stage: u16,
}

impl Fence {
    pub fn new(generation: u64, stages: u16) -> Self {
        Self {
            generation,
            stages,
            direct: false,
            request: None,
            step: 0,
            stage: 0,
        }
    }

    /// A fence for a direct transport: stages pass each step to the next
    /// themselves, and only the last stage acknowledges it.
    pub fn direct(generation: u64, stages: u16) -> Self {
        Self {
            direct: true,
            ..Self::new(generation, stages)
        }
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Move to a new generation; any running request's state is invalid.
    pub fn advance(&mut self) -> u64 {
        self.generation += 1;
        self.end();
        self.generation
    }

    /// Start a request at step zero on the first stage.
    pub fn begin(&mut self, request_id: &str, attempt: u32) {
        self.request = Some((request_id.into(), attempt));
        self.step = 0;
        self.stage = if self.direct { self.stages - 1 } else { 0 };
    }

    /// Forget the request; every later message for it is refused.
    pub fn end(&mut self) {
        self.request = None;
    }

    /// The stage index and step the front waits for.
    pub fn expected(&self) -> Option<(usize, u64)> {
        self.request
            .as_ref()
            .map(|_| (usize::from(self.stage), self.step))
    }

    /// Check one message and, if it is the expected one, advance.
    ///
    /// # Errors
    ///
    /// A [`Refusal`] when the message must not be used.
    pub fn accept<M: StepIdentity + ?Sized>(&mut self, message: &M) -> Result<Accepted, Refusal> {
        let (got, current) = (message.generation(), self.generation);
        if got < current {
            return Err(Refusal::StaleGeneration { got, current });
        }
        if got > current {
            return Err(Refusal::FutureGeneration { got, current });
        }
        let Some((request, attempt)) = &self.request else {
            return Err(Refusal::NoRequest);
        };
        if message.request_id() != request {
            return Err(Refusal::OtherRequest {
                got: message.request_id().into(),
                current: request.clone(),
            });
        }
        if message.attempt() != *attempt {
            return Err(Refusal::OtherAttempt {
                got: message.attempt(),
                current: *attempt,
            });
        }
        let stage = message.stage();
        if stage >= self.stages {
            return Err(Refusal::UnknownStage(stage));
        }
        let at = (message.step(), stage);
        let expected = (self.step, self.stage);
        if at < expected {
            return Err(Refusal::Duplicate {
                stage,
                step: message.step(),
            });
        }
        if at > expected {
            return Err(Refusal::OutOfOrder {
                stage,
                step: message.step(),
                expected_stage: self.stage,
                expected_step: self.step,
            });
        }
        if !self.direct {
            self.stage += 1;
            if self.stage < self.stages {
                return Ok(Accepted::Next(usize::from(self.stage)));
            }
            self.stage = 0;
        }
        self.step += 1;
        Ok(Accepted::StepDone(self.step - 1))
    }
}

/// Where stage state travels between stages.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Transport {
    /// Every stage acknowledges every step to the front, and the front
    /// starts the next stage (W10).
    #[default]
    Front,
    /// Each stage sends its state straight to the next over a
    /// `stage_link` connection; only the last stage answers the front
    /// (W12).
    Direct,
}

/// How the stages' direct links are bounded.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkPolicy {
    /// Frames a stage may queue for the next before it stops and fails
    /// the link.
    pub queue: u32,
    /// How long a stage waits for room in that queue.
    pub send_timeout_ms: u64,
}

impl Default for LinkPolicy {
    fn default() -> Self {
        Self {
            queue: 4,
            send_timeout_ms: 10_000,
        }
    }
}

/// One stage process and how to start it.
///
/// The front appends `--stage NAME --generation N --front ADDR --token T`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageSpec {
    pub name: String,
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// What a group must agree on, and its limits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StageGroupConfig {
    /// The complete model the group serves.
    pub model: String,
    /// The weights digest every stage must report.
    pub weights: String,
    /// The runtime every stage must report.
    pub runtime: String,
    /// The protocol version every stage must report.
    pub protocol: u32,
    /// The stages, in step order.
    pub stages: Vec<StageSpec>,
    /// Where stage state travels.
    pub transport: Transport,
    /// The direct links' bounds; unused by [`Transport::Front`].
    pub links: LinkPolicy,
    /// How long one stage may take to acknowledge one step; under
    /// [`Transport::Direct`], how long the whole chain may take.
    pub step_timeout: Duration,
    /// How long a stage may take to answer a health ping.
    pub health_timeout: Duration,
    /// How long a started or fenced stage may take to become ready.
    pub startup_timeout: Duration,
    /// How long a stage may take to stop or exit.
    pub stop_timeout: Duration,
    /// Consecutive failures allowed before the group stops (1–10, as for
    /// the serial worker).
    pub max_restarts: u32,
    pub backoff: Duration,
    pub max_backoff: Duration,
}

impl StageGroupConfig {
    /// Refuse invalid groups and limits before starting any stage.
    ///
    /// # Errors
    ///
    /// Says which setting is wrong.
    pub fn validate(&self) -> Result<(), String> {
        if !cfg!(unix) {
            return Err("stage groups require a Unix host".into());
        }
        if let Some(stage) = self.stages.iter().find(|s| !s.program.is_file()) {
            return Err(format!(
                "stage {} is missing its program at {}",
                stage.name,
                stage.program.display()
            ));
        }
        self.validate_settings()
    }

    /// As [`Self::validate`], for a group another machine heads: its
    /// stage programs are on that machine, so only the settings are checked
    /// here.
    ///
    /// # Errors
    ///
    /// Says which setting is wrong.
    pub fn validate_settings(&self) -> Result<(), String> {
        if !(1..=16).contains(&self.stages.len()) {
            return Err("a stage group needs 1–16 stages".into());
        }
        let names: BTreeSet<_> = self.stages.iter().map(|s| s.name.as_str()).collect();
        if names.len() != self.stages.len() || names.contains("") {
            return Err("every stage needs its own non-empty name".into());
        }
        let ms = Duration::from_millis;
        let secs = Duration::from_secs;
        if !(ms(10)..=secs(75)).contains(&self.step_timeout)
            || !(ms(10)..=secs(30)).contains(&self.health_timeout)
            || !(ms(10)..=secs(900)).contains(&self.startup_timeout)
            || !(ms(10)..=secs(5)).contains(&self.stop_timeout)
            || !(1..=10).contains(&self.max_restarts)
        {
            return Err("use step limits up to 75 seconds, health limits up to 30 seconds, startup limits up to 900 seconds, stop limits up to 5 seconds, and 1–10 restarts".into());
        }
        if !(1..=64).contains(&self.links.queue)
            || !(10..=60_000).contains(&self.links.send_timeout_ms)
        {
            return Err("a stage link queues 1–64 frames and waits 10 ms to 60 seconds".into());
        }
        if self.weights.is_empty() || self.runtime.is_empty() {
            return Err("a stage group needs its weights digest and runtime".into());
        }
        Ok(())
    }
}

/// One stage as the group status reports it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageStatus {
    pub name: String,
    /// The process group leader, while one runs.
    pub pid: Option<u32>,
    /// Ready at the group's current generation.
    pub ready: bool,
    /// Times this stage was replaced.
    pub replacements: u32,
    /// Steps this stage passed straight to the next stage, and their bytes
    /// on the wire (direct transport only).
    #[serde(default)]
    pub forwarded: u64,
    #[serde(default)]
    pub forwarded_bytes: u64,
}

/// The one readiness and generation status a beacon or router reads.
///
/// Advertise the group as a complete model only while `ready` is true. A
/// stage on its own is never a complete model.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StageGroupStatus {
    pub model: String,
    pub weights: String,
    pub runtime: String,
    pub protocol: u32,
    /// Where stage state travels.
    #[serde(default)]
    pub transport: Transport,
    /// Bumped whenever a stage is replaced.
    pub generation: u64,
    /// Every stage is loaded, checked, and agrees at `generation`.
    pub ready: bool,
    /// The one admission slot is held by a request or its cleanup.
    pub busy: bool,
    /// Set when the group stopped for good; restart the Pylon to retry.
    pub failed: Option<String>,
    /// Why the group is not ready, when it is not.
    pub reason: String,
    pub completed: u64,
    /// Stage messages refused by the fence or the stage checks.
    pub refused: u64,
    pub last_refusal: Option<String>,
    /// Consecutive failures counted against the restart budget.
    pub failures: u32,
    pub stages: Vec<StageStatus>,
}

impl StageGroupStatus {
    /// Whether a router may send this group a request now.
    pub fn admits(&self) -> bool {
        self.ready && !self.busy && self.failed.is_none()
    }

    /// The report the group head's beacon publishes (#12400). Generation 1
    /// is the first start; any later generation means a stage was replaced.
    /// A group that stopped for good reports `Recovering`, which no router
    /// accepts; stop publishing its beacon.
    pub fn head(&self) -> crate::group::GroupStatus {
        use nostr::pylon::Readiness;
        let readiness = if self.ready && self.failed.is_none() {
            Readiness::Ready
        } else if self.generation <= 1 && self.failed.is_none() {
            Readiness::Starting
        } else {
            Readiness::Recovering
        };
        crate::group::GroupStatus {
            readiness,
            generation: self.generation,
        }
    }
}

/// Why a request ended without completing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failure {
    pub reason: String,
    /// Tokens that reached the caller before the failure. The request is
    /// never replayed, so these stay the caller's only output.
    pub delivered: u64,
    /// The caller went away.
    pub cancelled: bool,
}

impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} after {} delivered tokens",
            self.reason, self.delivered
        )
    }
}

/// A completed request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Done {
    pub request: String,
    pub generation: u64,
    pub tokens: u64,
    /// How the last stage said the answer ended, when it parses answers.
    pub finish: Option<String>,
    /// Where the request's steps spent their time on each stage.
    pub profile: StepProfile,
}

/// Where one request's steps spent their time, stage by stage (W12,
/// #12399), from the times and bytes the stages report. A decode token's
/// time at the front minus the stages' summed compute is what the links
/// between stages and the head's own round trip cost.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StepProfile {
    /// The first step's (the prompt's) compute on each stage, µs.
    pub prefill_compute_us: Vec<u64>,
    /// Decode steps (every step after the first) each stage reported.
    pub decode_steps: Vec<u64>,
    /// Their summed compute on each stage, µs.
    pub decode_compute_us: Vec<u64>,
    /// The stage state each stage passed on in them, bytes on the wire
    /// (direct transport only).
    pub decode_bytes: Vec<u64>,
}

impl StepProfile {
    fn new(stages: usize) -> Self {
        Self {
            prefill_compute_us: vec![0; stages],
            decode_steps: vec![0; stages],
            decode_compute_us: vec![0; stages],
            decode_bytes: vec![0; stages],
        }
    }

    /// Count one step that `stage` finished.
    fn note(&mut self, stage: usize, step: u64, compute_us: Option<u64>, bytes: u64) {
        let Some(compute) = compute_us else {
            return;
        };
        if stage >= self.decode_steps.len() {
            return;
        }
        if step == 0 {
            self.prefill_compute_us[stage] += compute;
        } else {
            self.decode_steps[stage] += 1;
            self.decode_compute_us[stage] += compute;
            self.decode_bytes[stage] += bytes;
        }
    }

    /// Each stage's mean compute per decode step, ms; `None` when a stage
    /// reported none.
    #[must_use]
    pub fn decode_ms(&self) -> Option<Vec<f64>> {
        if self.decode_steps.is_empty() || self.decode_steps.contains(&0) {
            return None;
        }
        Some(
            self.decode_compute_us
                .iter()
                .zip(&self.decode_steps)
                .map(|(us, n)| *us as f64 / *n as f64 / 1_000.0)
                .collect(),
        )
    }

    /// The stage state passed between stages per decode token, bytes.
    #[must_use]
    pub fn bytes_per_token(&self) -> Option<f64> {
        let steps = *self.decode_steps.last()?;
        (steps > 0).then(|| self.decode_bytes.iter().sum::<u64>() as f64 / steps as f64)
    }
}

/// One admitted request. Dropping `tokens` cancels it.
#[derive(Debug)]
pub struct Run {
    pub request: String,
    pub generation: u64,
    /// Token ids in order, as the last stage sampled them.
    pub tokens: tokio::sync::mpsc::UnboundedReceiver<u32>,
    /// Each token's text, in order, when the last stage sends it.
    pub text: tokio::sync::mpsc::UnboundedReceiver<String>,
    /// Each token's reasoning and final-answer text, when the last stage
    /// parses the answer.
    pub answer: tokio::sync::mpsc::UnboundedReceiver<AnswerPiece>,
    pub done: tokio::sync::oneshot::Receiver<Result<Done, Failure>>,
}

#[cfg(unix)]
pub use runtime::StageGroup;

#[cfg(unix)]
mod runtime {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::tcp::OwnedWriteHalf;
    use tokio::net::{TcpListener, TcpStream};
    use tokio::sync::{mpsc, oneshot, watch};

    type Log = Arc<dyn Fn(String) + Send + Sync>;

    /// A stage's build as one line: its executable's SHA-256 and target.
    fn describe_build(build: Option<&serde_json::Value>) -> String {
        let Some(build) = build else {
            return "unstated".into();
        };
        match (build["sha256"].as_str(), build["cuda_arch"].as_str()) {
            (Some(sha), Some(arch)) => format!("{sha} ({arch})"),
            (Some(sha), None) => sha.into(),
            _ => build.to_string(),
        }
    }

    #[derive(Debug, Clone, Deserialize)]
    struct Hello {
        stage: String,
        token: String,
        generation: u64,
        weights: String,
        runtime: String,
        protocol: u32,
        /// Where the stage takes state from the stage before it. The front
        /// passes it to that stage with each step.
        #[serde(default)]
        listen: Option<String>,
        /// Where the previous stage reaches this one's link listener
        /// (direct transport).
        #[serde(default)]
        data: Option<String>,
        /// The executable the stage runs, as it reports it (its SHA-256 and
        /// target); every stage of a group must report the same (#12407).
        #[serde(default)]
        build: Option<serde_json::Value>,
        /// The numerical profile the stage runs, such as `cuda-device-sm_120`.
        #[serde(default)]
        profile: Option<String>,
    }

    #[derive(Debug, Clone, Deserialize)]
    #[serde(tag = "kind", rename_all = "snake_case")]
    enum Incoming {
        Hello(Hello),
        Pong {
            nonce: u64,
        },
        Checked {
            nonce: u64,
            output: u64,
        },
        Ack(StepAck),
        Stopped {
            request_id: String,
            attempt: u32,
        },
        /// The stage's link to the next stage is open at `generation`.
        Linked {
            generation: u64,
        },
        /// A link broke; `peer` is `next`, `prev`, or `self`.
        LinkFailed {
            generation: u64,
            peer: String,
            reason: String,
        },
        /// The stage passed `step` to the next stage in `bytes` bytes.
        Forwarded {
            generation: u64,
            request_id: String,
            attempt: u32,
            step: u64,
            #[serde(default)]
            bytes: u64,
            #[serde(default)]
            compute_us: Option<u64>,
        },
        /// The stage refused a frame from the previous stage.
        Refused {
            reason: String,
        },
    }

    enum Event {
        Joined {
            conn: u64,
            hello: Hello,
            writer: OwnedWriteHalf,
        },
        Message {
            conn: u64,
            message: Incoming,
        },
        Refused {
            reason: String,
        },
        Closed {
            conn: u64,
        },
    }

    enum Command {
        Run {
            prompt: Option<serde_json::Value>,
            steps: u64,
            sampling: Sampling,
            reply: oneshot::Sender<Result<Run, String>>,
        },
    }

    /// A running stage group. Dropping it stops and reaps every stage.
    pub struct StageGroup {
        commands: mpsc::UnboundedSender<Command>,
        status: watch::Receiver<StageGroupStatus>,
        kill: watch::Sender<bool>,
        task: std::sync::Mutex<Option<tokio::task::JoinHandle<()>>>,
    }

    impl StageGroup {
        /// Validate the group, open the front's loopback inbox, and start
        /// every stage at generation 1.
        ///
        /// # Errors
        ///
        /// When the configuration is invalid or the inbox cannot open.
        pub async fn start(config: StageGroupConfig, log: Log) -> Result<Self, String> {
            config.validate()?;
            let inbox = TcpListener::bind(("127.0.0.1", 0))
                .await
                .map_err(|e| format!("cannot open the stage inbox: {e}"))?;
            let front = inbox.local_addr().map_err(|e| e.to_string())?;
            let (events, received) = mpsc::unbounded_channel();
            tokio::spawn(accept(inbox, events));
            let (commands, command_rx) = mpsc::unbounded_channel();
            let (kill, kill_rx) = watch::channel(false);
            let front = Front::new(config, front.to_string(), log);
            let (status_tx, status) = watch::channel(front.status());
            let task = tokio::spawn(front.run(received, command_rx, kill_rx, status_tx));
            Ok(Self {
                commands,
                status,
                kill,
                task: std::sync::Mutex::new(Some(task)),
            })
        }

        /// The current readiness and generation.
        pub fn status(&self) -> StageGroupStatus {
            self.status.borrow().clone()
        }

        /// Follow status changes.
        pub fn watch(&self) -> watch::Receiver<StageGroupStatus> {
            self.status.clone()
        }

        /// Admit one request of `steps` tokens, sampled with `sampling`.
        ///
        /// # Errors
        ///
        /// When the group is not ready, is busy, or has stopped.
        pub async fn generate(&self, steps: u64, sampling: Sampling) -> Result<Run, String> {
            self.request(None, steps, sampling).await
        }

        /// Admit one request whose `prompt` (the conversation, as JSON)
        /// goes to the first stage with step zero; at most `steps` tokens,
        /// fewer when the last stage samples the end of the answer.
        ///
        /// # Errors
        ///
        /// As [`StageGroup::generate`].
        pub async fn generate_prompt(
            &self,
            prompt: serde_json::Value,
            steps: u64,
            sampling: Sampling,
        ) -> Result<Run, String> {
            self.request(Some(prompt), steps, sampling).await
        }

        async fn request(
            &self,
            prompt: Option<serde_json::Value>,
            steps: u64,
            sampling: Sampling,
        ) -> Result<Run, String> {
            let (reply, answer) = oneshot::channel();
            self.commands
                .send(Command::Run {
                    prompt,
                    steps,
                    sampling,
                    reply,
                })
                .map_err(|_| "the stage group stopped".to_string())?;
            answer
                .await
                .map_err(|_| "the stage group stopped".to_string())?
        }

        /// Stop every stage and wait until each process group is reaped.
        pub async fn stop(&self) {
            let _ = self.kill.send(true);
            let task = self
                .task
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .take();
            if let Some(task) = task {
                let _ = task.await;
            }
        }
    }

    impl crate::group::GroupReadiness for StageGroup {
        fn status(&self) -> crate::group::GroupStatus {
            self.status.borrow().head()
        }

        fn stages(&self) -> Vec<(String, bool)> {
            self.status
                .borrow()
                .stages
                .iter()
                .map(|s| (s.name.clone(), s.ready))
                .collect()
        }

        fn stopped(&self) -> Option<String> {
            self.status.borrow().failed.clone()
        }
    }

    impl Drop for StageGroup {
        fn drop(&mut self) {
            let _ = self.kill.send(true);
        }
    }

    static CONNECTIONS: AtomicU64 = AtomicU64::new(1);

    async fn accept(inbox: TcpListener, events: mpsc::UnboundedSender<Event>) {
        loop {
            let Ok((stream, _)) = inbox.accept().await else {
                continue;
            };
            if events.is_closed() {
                return;
            }
            let conn = CONNECTIONS.fetch_add(1, Ordering::Relaxed);
            tokio::spawn(read(conn, stream, events.clone()));
        }
    }

    /// Lines longer than this are refused rather than buffered.
    const LINE_LIMIT: u64 = 64 * 1024;

    async fn read(conn: u64, stream: TcpStream, events: mpsc::UnboundedSender<Event>) {
        let (reader, writer) = stream.into_split();
        let mut lines = BufReader::new(reader);
        let mut line = String::new();
        let mut writer = Some(writer);
        loop {
            line.clear();
            let read = tokio::io::AsyncReadExt::take(&mut lines, LINE_LIMIT)
                .read_line(&mut line)
                .await;
            if !matches!(read, Ok(n) if n > 0 && line.ends_with('\n')) {
                break;
            }
            let parsed = serde_json::from_str::<Incoming>(line.trim_end());
            let event = match (parsed, writer.take()) {
                (Ok(Incoming::Hello(hello)), Some(writer)) => Event::Joined {
                    conn,
                    hello,
                    writer,
                },
                (_, Some(_)) => {
                    let _ = events.send(Event::Refused {
                        reason: "a connection did not start by announcing a stage".into(),
                    });
                    return;
                }
                (Ok(message), None) => Event::Message { conn, message },
                (Err(error), None) => Event::Refused {
                    reason: format!("unreadable stage message: {error}"),
                },
            };
            if events.send(event).is_err() {
                return;
            }
        }
        let _ = events.send(Event::Closed { conn });
    }

    struct Slot {
        spec: StageSpec,
        child: Option<OwnedChild>,
        token: String,
        /// The generation this instance was started with.
        spawned: u64,
        conn: Option<u64>,
        writer: Option<OwnedWriteHalf>,
        joined: bool,
        checked: bool,
        check: Option<(u64, u64)>,
        ping: Option<(u64, Instant)>,
        next_ping: Instant,
        since: Instant,
        restart_at: Option<Instant>,
        broken: Option<String>,
        exited: Option<(Instant, String)>,
        replacements: u32,
        /// The address the joined instance announced for incoming state.
        listen: Option<String>,
        /// Its link listener's address, from its announcement.
        data: Option<String>,
        /// It was sent this generation's link key.
        link_sent: bool,
        /// Its link to the next stage is open (always true under the
        /// front transport).
        linked: bool,
        /// The last step of the running request it passed on.
        progress: Option<u64>,
        forwarded: u64,
        forwarded_bytes: u64,
    }

    impl Slot {
        fn ready(&self) -> bool {
            self.child.is_some()
                && self.conn.is_some()
                && self.joined
                && self.checked
                && self.linked
        }
        fn unlink(&mut self, direct: bool) {
            self.link_sent = false;
            self.linked = !direct;
            self.progress = None;
        }
        fn detach(&mut self, direct: bool) {
            self.conn = None;
            self.writer = None;
            self.joined = false;
            self.checked = false;
            self.check = None;
            self.ping = None;
            self.listen = None;
            self.data = None;
            self.unlink(direct);
        }
    }

    enum Phase {
        Running {
            sent: Instant,
        },
        /// The request ended; these stages have not yet stopped.
        Stopping {
            since: Instant,
            waiting: BTreeSet<usize>,
        },
    }

    struct Active {
        id: String,
        attempt: u32,
        prompt: Option<serde_json::Value>,
        steps: u64,
        sampling: Sampling,
        delivered: u64,
        /// The last token delivered, which the first stage embeds next.
        last_token: Option<u32>,
        tokens: mpsc::UnboundedSender<u32>,
        text: mpsc::UnboundedSender<String>,
        answer: mpsc::UnboundedSender<AnswerPiece>,
        done: Option<oneshot::Sender<Result<Done, Failure>>>,
        phase: Phase,
        profile: StepProfile,
    }

    struct Front {
        config: StageGroupConfig,
        address: String,
        log: Log,
        slots: Vec<Slot>,
        fence: Fence,
        budget: RestartBudget,
        active: Option<Active>,
        failed: Option<String>,
        reason: String,
        completed: u64,
        refused: u64,
        last_refusal: Option<String>,
        next_request: u64,
        nonce: u64,
        was_ready: bool,
        /// The link key for the current generation (direct transport).
        key: Option<(u64, stage_link::Key)>,
        /// The build and numerical profile the first stage to join
        /// announced. A stage that announces another, such as a replaced
        /// worker, never joins (#12407).
        identity: Option<(Option<serde_json::Value>, Option<String>)>,
    }

    impl Front {
        fn direct(&self) -> bool {
            self.config.transport == Transport::Direct
        }

        fn new(config: StageGroupConfig, address: String, log: Log) -> Self {
            let now = Instant::now();
            let slots = config
                .stages
                .iter()
                .map(|spec| Slot {
                    spec: spec.clone(),
                    child: None,
                    token: String::new(),
                    spawned: 0,
                    conn: None,
                    writer: None,
                    joined: false,
                    checked: false,
                    check: None,
                    ping: None,
                    next_ping: now,
                    since: now,
                    restart_at: Some(now),
                    broken: None,
                    exited: None,
                    replacements: 0,
                    listen: None,
                    data: None,
                    link_sent: false,
                    linked: config.transport == Transport::Front,
                    progress: None,
                    forwarded: 0,
                    forwarded_bytes: 0,
                })
                .collect();
            let stages = u16::try_from(config.stages.len()).expect("at most 16 stages");
            let fence = match config.transport {
                Transport::Front => Fence::new(1, stages),
                Transport::Direct => Fence::direct(1, stages),
            };
            Self {
                budget: RestartBudget::new(config.max_restarts, config.backoff, config.max_backoff),
                config,
                address,
                log,
                slots,
                fence,
                active: None,
                failed: None,
                reason: "stages are starting".into(),
                completed: 0,
                refused: 0,
                last_refusal: None,
                next_request: 0,
                nonce: 0,
                was_ready: false,
                key: None,
                identity: None,
            }
        }

        /// Every stage is loaded, checked, and agrees at this generation.
        fn ready(&self) -> bool {
            self.failed.is_none() && self.slots.iter().all(Slot::ready)
        }

        fn status(&self) -> StageGroupStatus {
            let ready = self.ready();
            StageGroupStatus {
                model: self.config.model.clone(),
                weights: self.config.weights.clone(),
                runtime: self.config.runtime.clone(),
                protocol: self.config.protocol,
                transport: self.config.transport,
                generation: self.fence.generation(),
                ready,
                busy: self.active.is_some(),
                failed: self.failed.clone(),
                reason: if ready {
                    String::new()
                } else {
                    self.reason.clone()
                },
                completed: self.completed,
                refused: self.refused,
                last_refusal: self.last_refusal.clone(),
                failures: self.budget.failures(),
                stages: self
                    .slots
                    .iter()
                    .map(|s| StageStatus {
                        name: s.spec.name.clone(),
                        pid: s.child.as_ref().map(OwnedChild::pid),
                        ready: s.ready(),
                        replacements: s.replacements,
                        forwarded: s.forwarded,
                        forwarded_bytes: s.forwarded_bytes,
                    })
                    .collect(),
            }
        }

        fn refuse(&mut self, reason: String) {
            self.refused += 1;
            (self.log)(format!("stage group refused a message: {reason}"));
            self.last_refusal = Some(reason);
        }

        async fn run(
            mut self,
            mut events: mpsc::UnboundedReceiver<Event>,
            mut commands: mpsc::UnboundedReceiver<Command>,
            mut kill: watch::Receiver<bool>,
            status: watch::Sender<StageGroupStatus>,
        ) {
            let tick = (self.config.health_timeout / 4)
                .clamp(Duration::from_millis(5), Duration::from_millis(100));
            loop {
                tokio::select! {
                    biased;
                    _ = kill.changed() => break,
                    command = commands.recv() => match command {
                        Some(command) => self.command(command).await,
                        None => break,
                    },
                    Some(event) = events.recv() => self.event(event).await,
                    _ = tokio::time::sleep(tick) => {}
                }
                self.poll().await;
                let next = self.status();
                status.send_if_modified(|s| {
                    let changed = *s != next;
                    *s = next;
                    changed
                });
            }
            self.shutdown("the stage group stopped").await;
            status.send_replace(self.status());
        }

        async fn command(&mut self, command: Command) {
            match command {
                Command::Run {
                    prompt,
                    steps,
                    sampling,
                    reply,
                } => {
                    let admitted = if let Some(failed) = &self.failed {
                        Err(format!("the stage group stopped: {failed}"))
                    } else if self.active.is_some() {
                        Err("the stage group is busy".into())
                    } else if !self.ready() {
                        Err(format!("the stage group is not ready: {}", self.reason))
                    } else if steps == 0 {
                        Err("a request needs at least one step".into())
                    } else {
                        Ok(())
                    };
                    if let Err(error) = admitted {
                        let _ = reply.send(Err(error));
                        return;
                    }
                    self.next_request += 1;
                    let id = self.next_request.to_string();
                    let (tokens, token_rx) = mpsc::unbounded_channel();
                    let (text, text_rx) = mpsc::unbounded_channel();
                    let (answer, answer_rx) = mpsc::unbounded_channel();
                    let (done, done_rx) = oneshot::channel();
                    self.fence.begin(&id, 1);
                    for slot in &mut self.slots {
                        slot.progress = None;
                    }
                    self.active = Some(Active {
                        id: id.clone(),
                        attempt: 1,
                        prompt,
                        steps,
                        sampling,
                        delivered: 0,
                        last_token: None,
                        tokens,
                        text,
                        answer,
                        done: Some(done),
                        phase: Phase::Running {
                            sent: Instant::now(),
                        },
                        profile: StepProfile::new(self.slots.len()),
                    });
                    let _ = reply.send(Ok(Run {
                        request: id,
                        generation: self.fence.generation(),
                        tokens: token_rx,
                        text: text_rx,
                        answer: answer_rx,
                        done: done_rx,
                    }));
                    self.send_step(0, 0).await;
                }
            }
        }

        async fn send(&mut self, index: usize, message: serde_json::Value) {
            let slot = &mut self.slots[index];
            let Some(writer) = slot.writer.as_mut() else {
                return;
            };
            let mut line = message.to_string();
            line.push('\n');
            let wrote = tokio::time::timeout(
                Duration::from_millis(500),
                writer.write_all(line.as_bytes()),
            )
            .await;
            if !matches!(wrote, Ok(Ok(()))) {
                slot.broken
                    .get_or_insert_with(|| "its connection to the front failed".into());
            }
        }

        async fn send_step(&mut self, index: usize, step: u64) {
            let Some(active) = &mut self.active else {
                return;
            };
            active.phase = Phase::Running {
                sent: Instant::now(),
            };
            let mut message = serde_json::json!({
                "kind": "step",
                "generation": self.fence.generation(),
                "request_id": active.id,
                "attempt": active.attempt,
                "stage": index,
                "step": step,
            });
            if index + 1 == self.slots.len() || self.config.transport == Transport::Direct {
                // The last stage samples with the front's settings; under
                // the direct transport the first stage passes them along.
                message["sampling"] = serde_json::json!(active.sampling);
                if step + 1 >= active.steps {
                    // The output limit ends the answer after this token.
                    message["final"] = serde_json::json!(true);
                }
            } else if let Some(next) = &self.slots[index + 1].listen {
                // Where this stage sends its state for the next one.
                message["next"] = serde_json::json!(next);
            }
            if index == 0
                && let Some(token) = active.last_token
            {
                // The first stage embeds the token the last stage sampled.
                message["token"] = serde_json::json!(token);
            }
            if index == 0
                && step == 0
                && let Some(prompt) = &active.prompt
            {
                // The first stage renders and encodes the conversation.
                message["prompt"] = prompt.clone();
            }
            self.send(index, message).await;
        }

        async fn event(&mut self, event: Event) {
            match event {
                Event::Joined {
                    conn,
                    hello,
                    writer,
                } => {
                    let Some(index) = self
                        .slots
                        .iter()
                        .position(|s| s.child.is_some() && s.token == hello.token)
                    else {
                        self.refuse(format!(
                            "stage {} at generation {} is not a running member of this group",
                            hello.stage, hello.generation
                        ));
                        return;
                    };
                    if self.slots[index].conn.is_some() {
                        self.refuse(format!("stage {} connected twice", hello.stage));
                        return;
                    }
                    self.slots[index].conn = Some(conn);
                    self.slots[index].writer = Some(writer);
                    self.announce(index, hello).await;
                }
                Event::Message { conn, message } => {
                    let Some(index) = self.slots.iter().position(|s| s.conn == Some(conn)) else {
                        self.refuse("a message arrived from a replaced stage".into());
                        return;
                    };
                    self.message(index, message).await;
                }
                Event::Refused { reason } => self.refuse(reason),
                Event::Closed { conn } => {
                    if let Some(slot) = self.slots.iter_mut().find(|s| s.conn == Some(conn)) {
                        slot.broken
                            .get_or_insert_with(|| "its connection to the front closed".into());
                    }
                }
            }
        }

        /// Check a stage's announcement and, if it agrees, send its check.
        async fn announce(&mut self, index: usize, hello: Hello) {
            let name = self.slots[index].spec.name.clone();
            let generation = self.fence.generation();
            let config = &self.config;
            let disagreement = if hello.stage != name {
                Some(format!("stage {name} announced itself as {}", hello.stage))
            } else if hello.generation < generation
                && hello.generation == self.slots[index].spawned
                && !self.slots[index].joined
            {
                // The current instance (its token proves it) started just
                // before the group moved on; bring it forward.
                self.send(
                    index,
                    serde_json::json!({"kind": "fence", "generation": generation}),
                )
                .await;
                return;
            } else if hello.generation != generation {
                self.refuse(format!(
                    "stage {name} announced generation {}; the group is at {generation}",
                    hello.generation
                ));
                return;
            } else if hello.weights != config.weights {
                Some(format!(
                    "stage {name} has weights {}; the group needs {}",
                    hello.weights, config.weights
                ))
            } else if hello.runtime != config.runtime {
                Some(format!(
                    "stage {name} runs {}; the group needs {}",
                    hello.runtime, config.runtime
                ))
            } else if hello.protocol != config.protocol {
                Some(format!(
                    "stage {name} speaks protocol {}; the group needs {}",
                    hello.protocol, config.protocol
                ))
            } else if config.transport == Transport::Direct && hello.data.is_none() {
                Some(format!(
                    "stage {name} has no link address; the group sends stage state directly"
                ))
            } else if let Some((build, profile)) = self
                .identity
                .as_ref()
                .filter(|(build, profile)| *build != hello.build || *profile != hello.profile)
            {
                Some(format!(
                    "stage {name} runs build {} with profile {}; the group runs build {} with profile {}",
                    describe_build(hello.build.as_ref()),
                    hello.profile.as_deref().unwrap_or("unstated"),
                    describe_build(build.as_ref()),
                    profile.as_deref().unwrap_or("unstated"),
                ))
            } else {
                None
            };
            if let Some(reason) = disagreement {
                // It never joins, so its startup deadline replaces it.
                self.reason = reason;
                return;
            }
            if self.identity.is_none() {
                (self.log)(format!(
                    "stage group runs build {} with profile {}",
                    describe_build(hello.build.as_ref()),
                    hello.profile.as_deref().unwrap_or("unstated")
                ));
                self.identity = Some((hello.build.clone(), hello.profile.clone()));
            }
            let direct = self.direct();
            let slot = &mut self.slots[index];
            slot.joined = true;
            slot.listen = hello.listen;
            slot.checked = false;
            slot.data = hello.data;
            slot.unlink(direct);
            self.nonce += 1;
            let (nonce, input) = (self.nonce, self.nonce.wrapping_mul(7919) % 1_000_003);
            self.slots[index].check = Some((nonce, input));
            if let Some(Active {
                phase: Phase::Stopping { waiting, .. },
                ..
            }) = &mut self.active
            {
                // A fenced stage announces itself only after dropping the
                // request's state.
                waiting.remove(&index);
            }
            self.send(
                index,
                serde_json::json!({"kind": "check", "nonce": nonce, "input": input}),
            )
            .await;
        }

        async fn message(&mut self, index: usize, message: Incoming) {
            match message {
                Incoming::Hello(hello) => {
                    if hello.token != self.slots[index].token {
                        self.refuse(format!("stage {} changed its identity", hello.stage));
                        return;
                    }
                    self.announce(index, hello).await;
                }
                Incoming::Pong { nonce } => {
                    let slot = &mut self.slots[index];
                    if slot.ping.is_some_and(|(n, _)| n == nonce) {
                        slot.ping = None;
                    }
                }
                Incoming::Checked { nonce, output } => {
                    let slot = &mut self.slots[index];
                    match slot.check {
                        Some((n, input)) if n == nonce && slot.joined => {
                            slot.check = None;
                            if output == input * 2 + 1 {
                                slot.checked = true;
                            } else {
                                slot.broken = Some("it failed its check".into());
                            }
                        }
                        _ => self.refuse(format!(
                            "stage {} answered a check it was not sent",
                            self.slots[index].spec.name
                        )),
                    }
                }
                Incoming::Ack(ack) => {
                    let name = self.slots[index].spec.name.clone();
                    if usize::from(ack.stage) != index {
                        self.refuse(format!("stage {name} acknowledged as stage {}", ack.stage));
                        return;
                    }
                    let last = index + 1 == self.slots.len();
                    if self.direct() && !last {
                        self.refuse(format!(
                            "stage {name} acknowledged step {} that it should pass to the next stage",
                            ack.step
                        ));
                        return;
                    }
                    if last != ack.token.is_some() {
                        self.refuse(format!(
                            "stage {name} {} a token",
                            if last { "did not return" } else { "returned" }
                        ));
                        return;
                    }
                    let accepted = self.fence.accept(&ack);
                    if accepted.is_ok()
                        && let Some(active) = &mut self.active
                    {
                        active.profile.note(index, ack.step, ack.compute_us, 0);
                    }
                    match accepted {
                        Ok(Accepted::Next(next)) => self.send_step(next, ack.step).await,
                        Ok(Accepted::StepDone(step)) => {
                            let token = ack.token.unwrap_or_default();
                            self.deliver(step, token, ack).await;
                        }
                        Err(refusal) => self.refuse(format!("stage {name}: {refusal}")),
                    }
                }
                Incoming::Stopped {
                    request_id: request,
                    attempt,
                } => {
                    if let Some(Active {
                        id,
                        attempt: current,
                        phase: Phase::Stopping { waiting, .. },
                        ..
                    }) = &mut self.active
                        && *id == request
                        && *current == attempt
                    {
                        waiting.remove(&index);
                    }
                }
                Incoming::Linked { generation } => {
                    let slot = &mut self.slots[index];
                    if generation == self.fence.generation() && slot.link_sent && slot.joined {
                        slot.linked = true;
                    } else {
                        let name = slot.spec.name.clone();
                        self.refuse(format!(
                            "stage {name} linked at generation {generation}, which it was not sent"
                        ));
                    }
                }
                Incoming::LinkFailed {
                    generation,
                    peer,
                    reason,
                } => {
                    let name = self.slots[index].spec.name.clone();
                    if generation != self.fence.generation() {
                        return self.refuse(format!(
                            "stage {name} reported a generation {generation} link: {reason}"
                        ));
                    }
                    // Replace the stage the report names: a stage that
                    // can't reach the next blames the next, and one whose
                    // inbound link broke blames the previous.
                    let blamed = match peer.as_str() {
                        "next" => (index + 1).min(self.slots.len() - 1),
                        "prev" => index.saturating_sub(1),
                        _ => index,
                    };
                    let why = format!("stage {name} lost its link: {reason}");
                    self.fail_stage(blamed, &why).await;
                }
                Incoming::Forwarded {
                    generation,
                    request_id,
                    attempt,
                    step,
                    bytes,
                    compute_us,
                } => {
                    let current = self.active.as_ref().is_some_and(|a| {
                        a.id == request_id
                            && a.attempt == attempt
                            && matches!(a.phase, Phase::Running { .. })
                    });
                    if generation != self.fence.generation() || !current {
                        let name = self.slots[index].spec.name.clone();
                        return self.refuse(format!(
                            "stage {name} passed on step {step} of request {request_id} at generation {generation}, which is not running"
                        ));
                    }
                    let slot = &mut self.slots[index];
                    slot.progress = Some(slot.progress.map_or(step, |p| p.max(step)));
                    slot.forwarded += 1;
                    slot.forwarded_bytes += bytes;
                    if let Some(active) = &mut self.active {
                        active.profile.note(index, step, compute_us, bytes);
                    }
                }
                Incoming::Refused { reason } => {
                    let name = self.slots[index].spec.name.clone();
                    self.refuse(format!("stage {name}: {reason}"));
                }
            }
        }

        /// Give every stage this generation's link key and the next
        /// stage's address, once every stage has joined and passed its
        /// check (direct transport).
        async fn link(&mut self) {
            if !self.direct()
                || self.failed.is_some()
                || !self.slots.iter().all(|s| s.joined && s.checked)
                || self.slots.iter().all(|s| s.link_sent)
            {
                return;
            }
            let generation = self.fence.generation();
            if self.key.as_ref().is_none_or(|(g, _)| *g != generation) {
                match stage_link::Key::generate() {
                    Ok(key) => self.key = Some((generation, key)),
                    Err(error) => {
                        return self
                            .give_up(format!("cannot make a link key: {error}"))
                            .await;
                    }
                }
            }
            let key = self
                .key
                .as_ref()
                .map(|(_, k)| k.to_hex())
                .unwrap_or_default();
            let stages = self.slots.len();
            for index in 0..stages {
                if self.slots[index].link_sent {
                    continue;
                }
                let next = self.slots.get(index + 1).and_then(|s| s.data.clone());
                self.slots[index].link_sent = true;
                let message = serde_json::json!({
                    "kind": "link",
                    "generation": generation,
                    "key": key,
                    "group": self.config.model,
                    "stage": index,
                    "stages": stages,
                    "next": next,
                    "queue": self.config.links.queue,
                    "send_timeout_ms": self.config.links.send_timeout_ms,
                });
                self.send(index, message).await;
            }
        }

        async fn deliver(&mut self, step: u64, token: u32, ack: StepAck) {
            let Some(active) = &mut self.active else {
                return;
            };
            let end = ack.end;
            if active.tokens.send(token).is_err() {
                return self.cancel().await;
            }
            if let Some(text) = ack.text {
                let _ = active.text.send(text);
            }
            if ack.reasoning.is_some() || ack.content.is_some() {
                let _ = active.answer.send(AnswerPiece {
                    reasoning: ack.reasoning.unwrap_or_default(),
                    content: ack.content.unwrap_or_default(),
                });
            }
            active.delivered += 1;
            active.last_token = Some(token);
            if active.delivered < active.steps && !end {
                return self.send_step(0, step + 1).await;
            }
            let done = Done {
                request: active.id.clone(),
                generation: self.fence.generation(),
                tokens: active.delivered,
                finish: ack.finish,
                profile: std::mem::take(&mut active.profile),
            };
            let reply = active.done.take();
            self.active = None;
            self.fence.end();
            self.completed += 1;
            self.budget.succeeded();
            if let Some(reply) = reply {
                let _ = reply.send(Ok(done));
            }
        }

        /// The caller went away: stop every stage before freeing the slot.
        async fn cancel(&mut self) {
            let Some(active) = &mut self.active else {
                return;
            };
            if !matches!(active.phase, Phase::Running { .. }) {
                return;
            }
            self.fence.end();
            let failure = Failure {
                reason: "the caller cancelled the request".into(),
                delivered: active.delivered,
                cancelled: true,
            };
            if let Some(reply) = active.done.take() {
                let _ = reply.send(Err(failure));
            }
            let waiting: BTreeSet<usize> = (0..self.slots.len())
                .filter(|&i| self.slots[i].conn.is_some())
                .collect();
            let (id, attempt) = (active.id.clone(), active.attempt);
            active.phase = Phase::Stopping {
                since: Instant::now(),
                waiting: waiting.clone(),
            };
            (self.log)(format!(
                "stage group request {id} cancelled; waiting for {} stages to stop",
                waiting.len()
            ));
            for index in waiting {
                self.send(
                    index,
                    serde_json::json!({"kind": "cancel", "request_id": id, "attempt": attempt}),
                )
                .await;
            }
        }

        /// Replace one stage: fail the request, fence the survivors, and
        /// kill and reap the stage's process group.
        async fn fail_stage(&mut self, index: usize, why: &str) {
            if self.slots[index].child.is_none() {
                return;
            }
            let reason = format!("stage {} failed: {why}", self.slots[index].spec.name);
            (self.log)(reason.clone());
            self.reason = reason.clone();
            let generation = self.fence.advance();
            let survivors: BTreeSet<usize> = (0..self.slots.len())
                .filter(|&i| i != index && self.slots[i].conn.is_some())
                .collect();
            if let Some(active) = &mut self.active {
                match &mut active.phase {
                    Phase::Running { .. } => {
                        let failure = Failure {
                            reason: reason.clone(),
                            delivered: active.delivered,
                            cancelled: false,
                        };
                        if let Some(reply) = active.done.take() {
                            let _ = reply.send(Err(failure));
                        }
                        active.phase = Phase::Stopping {
                            since: Instant::now(),
                            waiting: survivors.clone(),
                        };
                    }
                    Phase::Stopping { waiting, .. } => {
                        waiting.remove(&index);
                    }
                }
            }
            let now = Instant::now();
            let direct = self.direct();
            self.key = None;
            for &i in &survivors {
                let slot = &mut self.slots[i];
                slot.joined = false;
                slot.checked = false;
                slot.check = None;
                slot.unlink(direct);
                slot.since = now;
                self.send(
                    i,
                    serde_json::json!({"kind": "fence", "generation": generation}),
                )
                .await;
            }
            let slot = &mut self.slots[index];
            slot.detach(direct);
            slot.broken = None;
            slot.exited = None;
            slot.replacements += 1;
            let mut child = slot.child.take().expect("a running stage");
            let pid = child.pid();
            if let Err(error) = child.stop(self.config.stop_timeout).await {
                // Like the serial worker: if the old group cannot be
                // confirmed gone, serving stays paused.
                self.slots[index].child = Some(child);
                return self
                    .give_up(format!("stage {pid} did not exit: {error}"))
                    .await;
            }
            match self.budget.failed() {
                Some(wait) => {
                    (self.log)(format!(
                        "stage group moves to generation {generation}; restarting stage {} in {}s",
                        self.slots[index].spec.name,
                        wait.as_secs_f32()
                    ));
                    self.slots[index].restart_at = Some(Instant::now() + wait);
                }
                None => {
                    self.give_up(format!(
                        "the stage group stopped after repeated failures: {reason}; restart the pylon after fixing the stage"
                    ))
                    .await
                }
            }
        }

        async fn give_up(&mut self, reason: String) {
            (self.log)(reason.clone());
            self.failed = Some(reason.clone());
            self.shutdown(&reason).await;
        }

        /// Stop and reap every stage. The slot frees only once all are gone.
        async fn shutdown(&mut self, reason: &str) {
            self.reason = reason.into();
            self.fence.end();
            let direct = self.direct();
            for slot in &mut self.slots {
                slot.detach(direct);
                slot.restart_at = None;
                if let Some(mut child) = slot.child.take()
                    && let Err(error) = child.stop(self.config.stop_timeout).await
                {
                    (self.log)(format!("stage {} did not exit: {error}", slot.spec.name));
                    // Dropping the owner kills the group once more.
                }
            }
            if let Some(mut active) = self.active.take()
                && let Some(reply) = active.done.take()
            {
                let _ = reply.send(Err(Failure {
                    reason: reason.into(),
                    delivered: active.delivered,
                    cancelled: false,
                }));
            }
        }

        fn spawn(&mut self, index: usize) -> Result<(), String> {
            use sha2::{Digest, Sha256};
            static SPAWNS: AtomicU64 = AtomicU64::new(0);
            let generation = self.fence.generation();
            let slot = &mut self.slots[index];
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map_or(0, |d| d.as_nanos());
            let seed = format!(
                "{}:{}:{nanos}:{}:{generation}",
                std::process::id(),
                SPAWNS.fetch_add(1, Ordering::Relaxed),
                slot.spec.name
            );
            slot.token = format!("{:x}", Sha256::digest(seed.as_bytes()))[..32].to_string();
            slot.spawned = generation;
            let mut command = tokio::process::Command::new(&slot.spec.program);
            command
                .args(&slot.spec.args)
                .args(["--stage", &slot.spec.name])
                .args(["--generation", &generation.to_string()])
                .args(["--front", &self.address])
                .args(["--token", &slot.token])
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null());
            let child = OwnedChild::spawn(command)
                .map_err(|e| format!("stage {} did not start: {e}", slot.spec.name))?;
            (self.log)(format!(
                "stage {} started (process {}) at generation {generation}",
                slot.spec.name,
                child.pid()
            ));
            slot.child = Some(child);
            slot.since = Instant::now();
            slot.next_ping = slot.since;
            slot.restart_at = None;
            Ok(())
        }

        /// Watch processes, deadlines, and health; restart replaced stages.
        async fn poll(&mut self) {
            if self.failed.is_some() {
                return;
            }
            let now = Instant::now();
            for index in 0..self.slots.len() {
                if self.failed.is_some() {
                    return;
                }
                if self.slots[index]
                    .restart_at
                    .is_some_and(|at| at <= now && self.active.is_none())
                {
                    if let Err(error) = self.spawn(index) {
                        self.slots[index].restart_at = None;
                        self.reason = error.clone();
                        match self.budget.failed() {
                            Some(wait) => self.slots[index].restart_at = Some(now + wait),
                            None => {
                                return self
                                    .give_up(format!(
                                        "the stage group stopped after repeated failures: {error}"
                                    ))
                                    .await;
                            }
                        }
                    }
                    continue;
                }
                let Some(child) = self.slots[index].child.as_mut() else {
                    continue;
                };
                if let Ok(Some(status)) = child.child.try_wait() {
                    self.slots[index]
                        .exited
                        .get_or_insert_with(|| (now, format!("its process exited ({status})")));
                }
                let slot = &mut self.slots[index];
                // Read what an exited stage sent before its connection
                // closed, so an acknowledgment it made still counts.
                let failure = if let Some((at, why)) = slot.exited.clone()
                    && (slot.conn.is_none()
                        || slot.broken.is_some()
                        || at.elapsed() >= Duration::from_millis(250))
                {
                    Some(why)
                } else if let Some(broken) = self.slots[index].broken.take() {
                    Some(broken)
                } else if self.slots[index]
                    .ping
                    .is_some_and(|(_, sent)| sent.elapsed() >= self.config.health_timeout)
                {
                    Some("it stopped answering health checks".into())
                } else if !self.slots[index].ready()
                    && self.slots[index].since.elapsed() >= self.config.startup_timeout
                {
                    Some("it did not become ready in time".into())
                } else {
                    None
                };
                if let Some(why) = failure {
                    self.fail_stage(index, &why).await;
                    continue;
                }
                let slot = &mut self.slots[index];
                if slot.conn.is_some() && slot.ping.is_none() && slot.next_ping <= now {
                    self.nonce += 1;
                    let nonce = self.nonce;
                    let slot = &mut self.slots[index];
                    slot.ping = Some((nonce, now));
                    slot.next_ping = now + self.config.health_timeout / 4;
                    self.send(index, serde_json::json!({"kind": "ping", "nonce": nonce}))
                        .await;
                }
            }
            self.link().await;
            self.deadlines().await;
            let ready = self.ready();
            if ready && !self.was_ready {
                (self.log)(format!(
                    "stage group ready at generation {} with {} stages",
                    self.fence.generation(),
                    self.slots.len()
                ));
            }
            self.was_ready = ready;
            if !ready && self.reason.is_empty() {
                self.reason = "stages are starting".into();
            }
        }

        async fn deadlines(&mut self) {
            let Some(active) = &self.active else {
                return;
            };
            match &active.phase {
                Phase::Running { sent } => {
                    if active.tokens.is_closed() {
                        return self.cancel().await;
                    }
                    if sent.elapsed() >= self.config.step_timeout {
                        let (mut index, step) = self.fence.expected().unwrap_or((0, 0));
                        if self.direct() {
                            // The step stopped at the first stage that
                            // hasn't passed it on.
                            index = self
                                .slots
                                .iter()
                                .position(|s| s.progress.is_none_or(|p| p < step))
                                .unwrap_or(index)
                                .min(index);
                        }
                        self.fail_stage(
                            index,
                            &format!("step {step} made no progress before its deadline"),
                        )
                        .await;
                    }
                }
                Phase::Stopping { since, waiting } => {
                    if waiting.is_empty() {
                        (self.log)(format!(
                            "stage group request {} has stopped on every stage",
                            active.id
                        ));
                        self.active = None;
                        return;
                    }
                    if since.elapsed() >= self.config.stop_timeout {
                        let late: Vec<usize> = waiting.iter().copied().collect();
                        for index in late {
                            self.fail_stage(index, "it did not stop when asked").await;
                            if let Some(Active {
                                phase: Phase::Stopping { waiting, .. },
                                ..
                            }) = &mut self.active
                            {
                                waiting.remove(&index);
                            }
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ack(stage: u16, generation: u64, request: &str, attempt: u32, step: u64) -> StepAck {
        StepAck {
            stage,
            generation,
            request_id: request.into(),
            attempt,
            step,
            token: None,
            text: None,
            end: false,
            reasoning: None,
            content: None,
            finish: None,
            compute_us: None,
        }
    }

    fn fence() -> Fence {
        Fence::new(3, 2)
    }

    #[test]
    fn a_profile_splits_prefill_from_decode_per_stage() {
        let mut profile = StepProfile::new(2);
        assert_eq!(profile.decode_ms(), None);
        profile.note(0, 0, Some(900_000), 50_000);
        profile.note(1, 0, Some(800_000), 0);
        for step in 1..=4 {
            profile.note(0, step, Some(8_000), 44_000);
            profile.note(1, step, Some(9_000), 0);
        }
        // A stage that reports no time leaves the profile as it was.
        profile.note(1, 5, None, 0);
        profile.note(7, 5, Some(1), 0);
        assert_eq!(profile.prefill_compute_us, vec![900_000, 800_000]);
        assert_eq!(profile.decode_steps, vec![4, 4]);
        assert_eq!(profile.decode_ms(), Some(vec![8.0, 9.0]));
        assert_eq!(profile.bytes_per_token(), Some(44_000.0));
    }

    #[test]
    fn accepts_only_the_next_acknowledgment_in_stage_order() {
        let mut fence = fence();
        assert_eq!(fence.accept(&ack(0, 3, "1", 1, 0)), Err(Refusal::NoRequest));
        fence.begin("7", 1);
        assert_eq!(fence.expected(), Some((0, 0)));
        assert_eq!(fence.accept(&ack(0, 3, "7", 1, 0)), Ok(Accepted::Next(1)));
        assert!(matches!(
            fence.accept(&ack(0, 3, "7", 1, 0)),
            Err(Refusal::Duplicate { .. })
        ));
        assert_eq!(
            fence.accept(&ack(1, 3, "7", 1, 0)),
            Ok(Accepted::StepDone(0))
        );
        // A lost acknowledgment from `a` at step 1 leaves `b` out of order.
        assert!(matches!(
            fence.accept(&ack(1, 3, "7", 1, 1)),
            Err(Refusal::OutOfOrder { .. })
        ));
        assert!(matches!(
            fence.accept(&ack(0, 3, "7", 1, 2)),
            Err(Refusal::OutOfOrder { .. })
        ));
        assert_eq!(fence.accept(&ack(0, 3, "7", 1, 1)), Ok(Accepted::Next(1)));
        // A replay of an earlier step is a duplicate.
        assert!(matches!(
            fence.accept(&ack(1, 3, "7", 1, 0)),
            Err(Refusal::Duplicate { .. })
        ));
    }

    #[test]
    fn refuses_other_generations_requests_attempts_and_stages() {
        let mut fence = fence();
        fence.begin("7", 2);
        assert_eq!(
            fence.accept(&ack(0, 2, "7", 2, 0)),
            Err(Refusal::StaleGeneration { got: 2, current: 3 })
        );
        assert_eq!(
            fence.accept(&ack(0, 4, "7", 2, 0)),
            Err(Refusal::FutureGeneration { got: 4, current: 3 })
        );
        assert_eq!(
            fence.accept(&ack(0, 3, "6", 2, 0)),
            Err(Refusal::OtherRequest {
                got: "6".into(),
                current: "7".into()
            })
        );
        assert_eq!(
            fence.accept(&ack(0, 3, "7", 1, 0)),
            Err(Refusal::OtherAttempt { got: 1, current: 2 })
        );
        assert_eq!(
            fence.accept(&ack(5, 3, "7", 2, 0)),
            Err(Refusal::UnknownStage(5))
        );
        assert_eq!(fence.accept(&ack(0, 3, "7", 2, 0)), Ok(Accepted::Next(1)));
    }

    #[test]
    fn a_new_generation_invalidates_the_running_request() {
        let mut fence = fence();
        fence.begin("7", 1);
        assert_eq!(fence.accept(&ack(0, 3, "7", 1, 0)), Ok(Accepted::Next(1)));
        assert_eq!(fence.advance(), 4);
        assert_eq!(fence.expected(), None);
        // The old stage's late acknowledgment and a forged new-generation
        // one are both refused: the request is gone.
        assert!(matches!(
            fence.accept(&ack(1, 3, "7", 1, 0)),
            Err(Refusal::StaleGeneration { .. })
        ));
        assert_eq!(fence.accept(&ack(1, 4, "7", 1, 0)), Err(Refusal::NoRequest));
    }

    fn config(stages: Vec<StageSpec>) -> StageGroupConfig {
        StageGroupConfig {
            model: "m".into(),
            weights: "sha256:w".into(),
            runtime: "r".into(),
            protocol: PROTOCOL,
            stages,
            transport: Transport::Front,
            links: LinkPolicy::default(),
            step_timeout: Duration::from_secs(1),
            health_timeout: Duration::from_secs(1),
            startup_timeout: Duration::from_secs(5),
            stop_timeout: Duration::from_secs(1),
            max_restarts: 3,
            backoff: Duration::from_millis(20),
            max_backoff: Duration::from_millis(80),
        }
    }

    #[test]
    fn the_head_reports_starting_ready_and_recovering() {
        use nostr::pylon::Readiness;
        let mut status = StageGroupStatus {
            model: "m".into(),
            weights: "w".into(),
            runtime: "r".into(),
            protocol: PROTOCOL,
            transport: Transport::Front,
            generation: 1,
            ready: false,
            busy: false,
            failed: None,
            reason: String::new(),
            completed: 0,
            refused: 0,
            last_refusal: None,
            failures: 0,
            stages: vec![],
        };
        assert_eq!(status.head().readiness, Readiness::Starting);
        status.ready = true;
        assert_eq!(status.head().readiness, Readiness::Ready);
        status.ready = false;
        status.generation = 2;
        assert_eq!(status.head().readiness, Readiness::Recovering);
        assert_eq!(status.head().generation, 2);
        status.generation = 1;
        status.ready = true;
        status.failed = Some("stopped".into());
        assert_eq!(status.head().readiness, Readiness::Recovering);
    }

    #[cfg(unix)]
    #[test]
    fn validation_refuses_duplicate_stages_missing_programs_and_bad_limits() {
        let program = PathBuf::from("/bin/sh");
        let stage = |name: &str| StageSpec {
            name: name.into(),
            program: program.clone(),
            args: vec![],
        };
        assert!(config(vec![stage("a"), stage("b")]).validate().is_ok());
        assert!(config(vec![]).validate().is_err());
        assert!(config(vec![stage("a"), stage("a")]).validate().is_err());
        let mut missing = config(vec![stage("a")]);
        missing.stages[0].program = "/nonexistent/stage".into();
        assert!(missing.validate().is_err());
        let mut budget = config(vec![stage("a")]);
        budget.max_restarts = 11;
        assert!(budget.validate().is_err());
        let mut stop = config(vec![stage("a")]);
        stop.stop_timeout = Duration::from_secs(6);
        assert!(stop.validate().is_err());
    }
}
