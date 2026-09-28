//! The published data contract: `openagents.gym.leaderboard.v1` and
//! `openagents.gym.trace-bundle.v1`.
//!
//! Every number in these types is computed from a committed evidence file
//! by this crate. None is typed by hand. A board carries the digests of
//! the files it read, the labels its claims must travel with, and its
//! caveats, so a reader (the Verse Gym, a web page, or a person with
//! `jq`) can't show a number without them.
//!
//! Money is list-price US dollars unless a [`CostBasis`] says otherwise.
//! Durations are seconds. Missing values are `null`, never zero.

use serde::{Deserialize, Serialize};

/// The leaderboard schema.
pub const LEADERBOARD_SCHEMA: &str = "openagents.gym.leaderboard.v1";

/// The trace bundle schema.
pub const TRACE_BUNDLE_SCHEMA: &str = "openagents.gym.trace-bundle.v1";

/// The publication index schema.
pub const INDEX_SCHEMA: &str = "openagents.gym.leaderboard-index.v1";

/// Every board the generator knows, with the digest of their content.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Leaderboard {
    pub schema: String,
    pub generator: Generator,
    /// The ATIF-rule digest of `boards`: object keys sorted at every
    /// depth, then SHA-256. Two builds from the same evidence agree.
    pub digest: String,
    pub boards: Vec<Board>,
}

/// What wrote a publication.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Generator {
    pub name: String,
    pub version: String,
}

/// One result set: one pre-registered or declared study, one population.
///
/// A board never pools attempts from different arms, references, or
/// benchmarks. Two studies are two boards.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Board {
    /// Stable, URL-safe, e.g. `tb4-fable-delegate-repro-9776`.
    pub id: String,
    pub title: String,
    pub benchmark: Benchmark,
    pub kind: BoardKind,
    /// The question the study asked, from its report.
    pub question: String,
    /// One sentence built by code from the tallies below.
    pub headline: String,
    pub provenance: Provenance,
    pub subject: Subject,
    pub reference: Reference,
    /// Labels that apply to every claim on the board.
    pub labels: Vec<Label>,
    pub caveats: Vec<Caveat>,
    pub totals: Tally,
    /// Named subsets of the attempts, such as each pass, or attempts on
    /// tasks with and without their own knowledge.
    pub splits: Vec<Split>,
    pub spend: Spend,
    pub tasks: Vec<TaskRow>,
    pub attempts: Vec<Attempt>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Benchmark {
    /// `Terminal-Bench`.
    pub name: String,
    /// `4.0`, `2.1`.
    pub version: String,
}

/// What a beat means on the board.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoardKind {
    /// A beat is a pass whose known total cost is below the reference's
    /// cheapest win and whose whole-trial time is below its fastest win.
    BeatCheapestAndFastestWin,
    /// A beat is a pass whose cost is below the reference's cost per
    /// trial on the same task. Time isn't part of the rule.
    CostBelowReferencePerTrial,
}

/// Where the board's numbers come from.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Provenance {
    /// GitHub issues in `OpenAgentsInc/openagents`.
    pub issues: Vec<u32>,
    /// The dated report, repository-relative.
    pub report: String,
    /// The commit that froze the inputs before any run, when there was one.
    pub frozen_commit: Option<String>,
    /// Every committed file the board was computed from.
    pub evidence: Vec<EvidenceFile>,
}

/// A committed file and its SHA-256 at generation time.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFile {
    /// Repository-relative.
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// The agent under test.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Subject {
    /// `Coder One`, `Microcoder`.
    pub agent: String,
    /// The arm's name in the harness.
    pub arm: String,
    /// The executor model, as its records name it.
    pub model: String,
    pub effort: Option<String>,
    /// The binary's identity, when the report pins one.
    pub artifact: Option<String>,
}

/// What the subject was measured against.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Reference {
    /// `Fable 5.1 low`.
    pub name: String,
    /// The bar in words.
    pub rule: String,
    /// Where the reference's runs came from and how that differs.
    pub conditions: String,
}

/// A label a claim must carry wherever it's shown.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Label {
    /// The inputs were committed before the first run.
    PreRegistered,
    /// Knowledge entries were in the prompt.
    KnowledgeAssisted,
    /// The knowledge base was off.
    KnowledgeOff,
    /// A knowledge entry the attempt kept was written from earlier runs
    /// on the same task.
    InSample,
    /// The tasks weren't used to develop the subject.
    OutOfSample,
    /// Costs are list prices on reported tokens, not a bill.
    ListPrice,
    /// Some costs are unknown; only an estimated bound is shown for them.
    CostBound,
    /// The beat's cost or time margin is under five percent.
    ThinMargin,
    /// One or two attempts per task: no per-task rate is estimated.
    FewAttempts,
    /// The reference ran on another host under other conditions.
    ReferenceOtherConditions,
}

impl Label {
    /// The label as a reader sees it.
    #[must_use]
    pub fn text(self) -> &'static str {
        match self {
            Self::PreRegistered => "pre-registered",
            Self::KnowledgeAssisted => "knowledge-assisted",
            Self::KnowledgeOff => "knowledge off",
            Self::InSample => "in-sample",
            Self::OutOfSample => "out-of-sample",
            Self::ListPrice => "list price",
            Self::CostBound => "cost is a bound",
            Self::ThinMargin => "thin margin",
            Self::FewAttempts => "few attempts",
            Self::ReferenceOtherConditions => "reference under other conditions",
        }
    }
}

/// A limit the board states in words, keyed so a reader can style it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Caveat {
    pub code: String,
    pub text: String,
}

/// Counts over a population of attempts. Faults aren't results and aren't
/// in `attempts`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tally {
    pub attempts: u32,
    pub passes: u32,
    pub beats: u32,
    pub faults: u32,
    /// Attempts whose cost is unknown. They can't beat.
    pub cost_unknown: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Split {
    pub name: String,
    pub tally: Tally,
}

/// What the whole board spent.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Spend {
    pub basis: CostBasis,
    /// The sum of costs the tools reported.
    pub reported_usd: f64,
    /// The sum of lower-bound estimates for attempts whose cost is unknown.
    pub estimated_lower_bound_usd: Option<f64>,
    /// The sum of upper bounds for attempts whose cost is unknown.
    pub estimated_upper_bound_usd: Option<f64>,
}

/// How a cost figure was produced.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostBasis {
    /// List price applied to reported tokens.
    ListPrice,
}

/// One task's row.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
    pub task: String,
    pub bar: Bar,
    pub knowledge: TaskKnowledge,
    /// Attempt IDs on this board, in run order.
    pub attempts: Vec<String>,
    pub passes: u32,
    pub beats: u32,
    pub status: TaskStatus,
}

/// The reference's bar on one task.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Bar {
    /// The cost a pass must come in under.
    pub cost_usd: Option<f64>,
    /// The whole-trial time a pass must come in under, when time counts.
    pub seconds: Option<f64>,
    /// The reference trial that set the cost bar.
    pub cost_trial: Option<String>,
    /// The reference trial that set the time bar.
    pub time_trial: Option<String>,
    /// The subject's deadline, when the study derived one from the bar.
    pub deadline_seconds: Option<u32>,
    /// The reference's passes and trials on the task.
    pub reference_passes: Option<u32>,
    pub reference_trials: Option<u32>,
}

/// Whether the task had knowledge of its own.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum TaskKnowledge {
    /// Candidates written from earlier runs on this task were offered.
    Own { candidates: u32 },
    /// Candidates were offered, none written from this task.
    OtherTasksOnly,
    /// The knowledge base was off.
    Off,
}

/// A task's standing, from its attempts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// At least one attempt beat the bar.
    Beat,
    /// A study's confirmation rule held (for example 2 of 3 runs won).
    Confirmed,
    /// Some runs beat the bar, but the study's confirmation rule didn't
    /// hold.
    NotConfirmed,
    /// Passed, but no pass beat the bar.
    PassedWithoutBeat,
    /// No attempt passed.
    NeverPassed,
}

/// One graded attempt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Attempt {
    /// Unique on the board, e.g. `coq-block-bound.p2`.
    pub id: String,
    pub task: String,
    /// The split it belongs to in run order, e.g. `pass 2`.
    pub series: String,
    /// The harness trial or run record.
    pub trial: String,
    pub reward: Option<f64>,
    pub passed: bool,
    /// Whole-trial seconds (the time the bar compares against).
    pub seconds: Option<f64>,
    pub phases: Option<Phases>,
    pub cost: Cost,
    /// Cost over the cost bar, when both are known.
    pub cost_ratio: Option<f64>,
    /// Whether `cost_ratio` compares a bound rather than a known cost.
    pub cost_ratio_is_bound: bool,
    /// Seconds over the time bar, when the board has one.
    pub time_ratio: Option<f64>,
    pub beat: bool,
    /// Why it isn't a beat, in the order the rule checks. Empty on a beat.
    pub misses: Vec<Miss>,
    pub labels: Vec<Label>,
    pub how_it_ended: Option<String>,
    pub jev: Option<JevSummary>,
    pub verifier: Option<VerifierSummary>,
    /// The scrubbed, bounded trace bundle for this attempt, when one was
    /// generated. The path is relative to the leaderboard file.
    pub trace: Option<TraceRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Phases {
    pub environment_setup: Option<f64>,
    pub agent_setup: Option<f64>,
    pub agent_execution: Option<f64>,
    pub verifier: Option<f64>,
}

/// An attempt's cost.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Cost {
    /// The tool reported it (for Claude Code, its own `total_cost_usd`).
    Reported { usd: f64 },
    /// Unknown. A bound may be estimated from the stream; it can't beat.
    Unknown {
        lower_bound_usd: Option<f64>,
        upper_bound_usd: Option<f64>,
    },
}

impl Cost {
    #[must_use]
    pub fn known(&self) -> Option<f64> {
        match self {
            Self::Reported { usd } => Some(*usd),
            Self::Unknown { .. } => None,
        }
    }
}

/// Why an attempt isn't a beat.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Miss {
    Failed,
    CostUnknown,
    Cost,
    Time,
}

/// What Jev decided before the attempt.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JevSummary {
    pub question_set: String,
    pub outcome: String,
    pub milliseconds: Option<u64>,
    pub candidates: u32,
    pub kept: u32,
    /// Kept entries written from this task.
    pub kept_own: u32,
    pub requirements: u32,
    pub flagged: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerifierSummary {
    pub summary: Option<String>,
    pub failed_tests: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TraceRef {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
}

/// One attempt's trace, scrubbed and bounded for a phone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceBundle {
    pub schema: String,
    pub board: String,
    pub attempt: String,
    pub task: String,
    /// The retained files the bundle was read from.
    pub sources: Vec<EvidenceFile>,
    /// The task instruction as the agent received it.
    pub instruction: Text,
    pub jev: Option<JevDecision>,
    /// The briefing the delegate received.
    pub briefing: Option<Text>,
    /// The episode, in time order. `at_ms` is from the episode's start.
    pub steps: Vec<TraceStep>,
    pub verifier: Option<VerifierDetail>,
    pub outcome: TraceOutcome,
    pub scrub: ScrubReport,
}

/// A bounded string. `original_bytes` is set when the text was cut.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Text {
    pub text: String,
    pub original_bytes: Option<u64>,
}

/// Every candidate and requirement Jev judged, with its probability.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JevDecision {
    pub question_set: String,
    pub questions: Vec<String>,
    pub keep_threshold: f64,
    pub flag_threshold: f64,
    pub budget_chars: Option<u64>,
    pub milliseconds: Option<u64>,
    pub input_tokens: Option<u64>,
    pub candidates: Vec<JevCandidate>,
    pub requirements: Vec<JevRequirement>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JevCandidate {
    pub rank: u32,
    pub id: String,
    pub version: u32,
    pub title: Option<String>,
    pub sha256: String,
    /// The retrieval score that ranked it.
    pub score: f64,
    /// Jev's probability that the entry bears on the task.
    pub p: f64,
    pub kept: bool,
    pub fate: String,
    pub written_from_this_task: Option<bool>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JevRequirement {
    pub text: String,
    pub p: f64,
    pub flagged: bool,
}

/// One step on the trace's clock.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceStep {
    pub at_ms: Option<u64>,
    #[serde(flatten)]
    pub kind: StepKind,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum StepKind {
    /// The host's own step: an invocation starting or ending, a note.
    Host { text: Text },
    /// A decision-model call (Jev) the host made.
    Decision {
        name: String,
        duration_ms: Option<u64>,
    },
    /// The delegate started.
    DelegateStarted {
        agent: String,
        model: Option<String>,
    },
    /// The agent said something.
    Say { text: Text },
    /// The agent ran a command.
    Command { command: Text },
    /// The command's result.
    CommandResult {
        exit_code: Option<i64>,
        output: Text,
    },
    /// Cumulative token usage at this point in the session.
    Usage {
        input_tokens: u64,
        cache_write_tokens: u64,
        cache_read_tokens: u64,
        output_tokens: u64,
    },
    /// The delegate ended on its own.
    DelegateEnded { error: bool, result: Text },
}

/// The verifier's result.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct VerifierDetail {
    pub reward: Option<f64>,
    pub tests: Vec<TestResult>,
    pub passed: u32,
    pub failed: u32,
    /// The end of the verifier's output.
    pub output_tail: Text,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestResult {
    pub name: String,
    pub status: String,
}

/// The attempt's numbers, repeated from the board so a bundle stands alone.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TraceOutcome {
    pub passed: bool,
    pub beat: bool,
    pub seconds: Option<f64>,
    pub cost: Cost,
    pub bar: Bar,
    pub how_it_ended: Option<String>,
}

/// What scrubbing and bounding changed.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScrubReport {
    /// Redactions by rule name. Empty means nothing matched.
    pub redactions: std::collections::BTreeMap<String, u32>,
    /// Text fields cut to their bound.
    pub truncated_fields: u32,
    /// Steps dropped to fit the bundle bound (none, normally).
    pub dropped_steps: u32,
    /// The per-field bound finally used, in bytes.
    pub field_bound: u32,
}

/// The append-only list of publications.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Index {
    pub schema: String,
    pub publications: Vec<Publication>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Publication {
    /// The leaderboard's content digest.
    pub digest: String,
    /// The git commit the evidence was read at, when the caller gave one.
    pub commit: Option<String>,
    pub boards: Vec<String>,
}
