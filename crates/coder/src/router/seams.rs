//! The router's integration seams: the traits other modules implement so
//! the router can personalize a stem (T1), ground a reply in the product
//! or codebase knowledge base (T2), read the Gym's verified records (the
//! `gym.*` and `eval.*` routes), take one step of the authoring interview
//! (`eval.author`), and propose an `openagents` command (T4 CLI), without
//! the router depending on how any of them work.
//!
//! Each seam has a no-op implementation, and [`Seams::default`] holds only
//! no-ops, so the router works, and falls back to today's behavior, before
//! any real implementation lands.
//!
//! The router, not the implementation, owns the invariants around a seam:
//!
//! - It decides whether a seam is called at all (the policy table).
//! - It builds the input. A [`Ask`] carries only the route, the answer
//!   id, the stem, and the latest message, already redacted and bounded to
//!   [`MESSAGE_CHARS`] characters; nothing from earlier turns, no device
//!   or worker key, and no `context` field.
//! - It bounds the call with the seam's budget ([`PERSONALIZE_BUDGET`],
//!   [`KB_BUDGET`], [`CLI_BUDGET`]) and treats a late answer as a failure.
//! - It validates what comes back before any of it is shown: a
//!   continuation must pass [`super::validate_continuation`], a passage
//!   below [`super::RELEVANCE_FLOOR`] is dropped, and a CLI proposal passes
//!   [`super::gate`] for the surface or is not offered.
//! - It names every service a message reaches in the privacy answer, from
//!   each seam's [`Personalize::recipients`] (and the others').
//!
//! Every method returns a boxed future so a seam can be held as
//! `Arc<dyn …>` by the worker.

use std::sync::Arc;
use std::time::Duration;

use futures_util::future::BoxFuture;

use super::{Effect, RouteId, RunsOn, Surface};
use crate::generate::Message;

/// The longest latest message, in characters, a seam receives.
pub const MESSAGE_CHARS: usize = 600;

/// How long the router waits for a continuation before it closes the stem
/// with the entry's `generic_end`.
pub const PERSONALIZE_BUDGET: Duration = Duration::from_millis(1_200);

/// How long the router waits for retrieval before it answers with the
/// model alone.
pub const KB_BUDGET: Duration = Duration::from_millis(2_000);

/// How long the router waits for a CLI proposal before it answers with the
/// model alone.
pub const CLI_BUDGET: Duration = Duration::from_millis(3_000);

/// How long the router waits for the Gym's records before it answers with
/// the model alone, told it has none.
pub const GYM_BUDGET: Duration = Duration::from_millis(2_000);

/// How long the router waits for one step of the authoring interview (a
/// model turn behind the seam) before it falls back to the bank.
pub const AUTHOR_BUDGET: Duration = Duration::from_millis(12_000);

/// Why a seam did not answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SeamError {
    /// Nothing is configured behind the seam. The router falls back
    /// silently, as if the tier did not exist.
    Unavailable,
    /// The seam tried and failed; the text is for the worker's log and must
    /// not contain message text.
    Failed(String),
}

impl std::fmt::Display for SeamError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SeamError::Unavailable => f.write_str("unavailable"),
            SeamError::Failed(why) => write!(f, "failed: {why}"),
        }
    }
}

impl std::error::Error for SeamError {}

// ---------------------------------------------------------------------------
// T1: personalization
// ---------------------------------------------------------------------------

/// What a personalization call may see: the whole prompt, nothing else.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Ask {
    /// The route the router chose.
    pub route: RouteId,
    /// The bank entry whose stem is being continued, as `id` (no version).
    pub answer: String,
    /// The stem, exactly as shown to the user. The continuation follows it.
    pub stem: String,
    /// The user's latest message, redacted of bounded secret shapes and
    /// cut to [`MESSAGE_CHARS`] characters.
    pub message: String,
}

/// A continuation of a stem, as the provider wrote it. The router
/// validates it before it is shown.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Continuation {
    /// The words that follow the stem, starting with the space or
    /// punctuation that joins them.
    pub text: String,
    /// The model that wrote them, for the result's `model` field.
    pub model: String,
}

/// Writes the rest of a stem's sentence from the user's latest message.
pub trait Personalize: Send + Sync {
    /// Whether a provider is configured. When `false` the router never
    /// calls [`Personalize::continuation`] and closes stems with their
    /// `generic_end`.
    fn available(&self) -> bool;

    /// The services a message reaches through this seam, named for a
    /// person ("OpenRouter"), for the privacy answer. Empty when
    /// unavailable.
    fn recipients(&self) -> Vec<String>;

    /// The continuation of `ask.stem`.
    fn continuation<'a>(&'a self, ask: &'a Ask) -> BoxFuture<'a, Result<Continuation, SeamError>>;
}

/// No personalization: every stem closes with its `generic_end`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoPersonalize;

impl Personalize for NoPersonalize {
    fn available(&self) -> bool {
        false
    }

    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }

    fn continuation<'a>(&'a self, _: &'a Ask) -> BoxFuture<'a, Result<Continuation, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

// ---------------------------------------------------------------------------
// T2: product and codebase knowledge
// ---------------------------------------------------------------------------

/// What a retrieval may see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Lookup {
    /// The user's latest message, redacted and cut to [`MESSAGE_CHARS`].
    pub message: String,
    /// The bounded transcript the turn carried, oldest first, for
    /// resolving what "it" refers to. An implementation that sends text to
    /// an embedding provider must name that provider in `recipients`.
    pub transcript: Vec<Message>,
}

/// One retrieved passage.
#[derive(Clone, Debug, PartialEq)]
pub struct Passage {
    /// The entry's stable id, with its version when it has one
    /// (`product.connect-a-mac@2`); cited in the reply.
    pub id: String,
    /// A short title for the citation.
    pub title: String,
    /// The passage the model may answer from.
    pub text: String,
    /// Where it comes from: a repository path or a public URL.
    pub source: String,
    /// Jev's relevance for this message, in `[0, 1]`. The router drops a
    /// passage below [`super::RELEVANCE_FLOOR`].
    pub relevance: f64,
    /// A reviewed short answer in the plural voice, when the entry has one.
    /// The router may serve it whole (T0) at relevance at least
    /// [`super::KB_ANSWER_CONFIDENCE`] when the message needs no specifics.
    pub answer: Option<String>,
    /// The reviewed answer assumes the chat is not on a computer (the
    /// entry's `off-computer` tag), so a chat on the computer Coder runs on
    /// never shows it whole; the passage still grounds the model (#10077).
    pub off_computer: bool,
    /// The reviewed answer walks through screens of the OpenAgents phone
    /// or desktop app (the entry's `in-app` tag), so the website's chat,
    /// where those screens are not, never shows it whole; the passage
    /// still grounds the model.
    pub in_app: bool,
}

/// What a retrieval found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Grounding {
    /// The passages, most relevant first; at most six are used.
    pub passages: Vec<Passage>,
    /// The commit the corpus was read at, named in the reply ("as of
    /// `820bc02`"); required for the codebase corpus.
    pub commit: Option<String>,
    /// The corpus cannot answer this without running or reading more code
    /// than it holds: the router offers to dispatch Coder instead.
    pub needs_dispatch: bool,
}

/// What one job on the caller's own provider keys (BYOK, NIP-CJ
/// "Caller-paid model calls") lends a seam for that job: their keys, and
/// Jev on them. A seam that grounds on these embeds the message and asks
/// for judgments on the caller's keys only; the records it reads (a corpus,
/// an index, the Gym's verified results) are ours and stay shared.
pub struct TheirKeys {
    /// The caller's keys for this job ([`model_access::Access::theirs`]).
    pub access: model_access::Access,
    /// Jev on the caller's keys.
    pub judge: std::sync::Arc<jev::Client>,
}

impl std::fmt::Debug for TheirKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TheirKeys").finish_non_exhaustive()
    }
}

impl TheirKeys {
    /// The embedder on the caller's keys, or `None` when none of their keys
    /// embeds. Never one of ours.
    #[must_use]
    pub fn embedder(&self) -> Option<knowledge::search::Embedder> {
        knowledge::search::Embedder::theirs(&self.access).and_then(Result::ok)
    }
}

/// The OpenAgents product knowledge base (`knowledge/openagents/`).
pub trait ProductKb: Send + Sync {
    /// Whether a corpus is configured.
    fn available(&self) -> bool;
    /// The services a message reaches through this seam (an embedding
    /// provider, for example), for the privacy answer.
    fn recipients(&self) -> Vec<String>;
    /// The admitted passages relevant to `lookup`.
    fn ground<'a>(&'a self, lookup: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>>;
    /// This knowledge base for one job on the caller's own keys: the same
    /// corpus, embedding and judging on their keys. `None` turns the seam
    /// off for that job; it never runs on ours.
    fn on_their_keys(&self, _theirs: &TheirKeys) -> Option<std::sync::Arc<dyn ProductKb>> {
        None
    }
}

/// Knowledge of the public OpenAgents codebase at a pinned commit.
pub trait CodebaseKb: Send + Sync {
    /// Whether an index is configured.
    fn available(&self) -> bool;
    /// The services a message reaches through this seam.
    fn recipients(&self) -> Vec<String>;
    /// The passages relevant to `lookup`, with the commit they were read at.
    fn ground<'a>(&'a self, lookup: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>>;
    /// Keep the connections a lookup uses open, with a fixed text and no
    /// message, so the first question after a quiet spell fits the budget.
    fn warm(&self) -> BoxFuture<'_, ()> {
        Box::pin(async {})
    }
    /// This index for one job on the caller's own keys, as
    /// [`ProductKb::on_their_keys`].
    fn on_their_keys(&self, _theirs: &TheirKeys) -> Option<std::sync::Arc<dyn CodebaseKb>> {
        None
    }
}

/// No knowledge base: the router answers such routes with the model alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoKb;

impl ProductKb for NoKb {
    fn available(&self) -> bool {
        false
    }
    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
    fn ground<'a>(&'a self, _: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

impl CodebaseKb for NoKb {
    fn available(&self) -> bool {
        false
    }
    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
    fn ground<'a>(&'a self, _: &'a Lookup) -> BoxFuture<'a, Result<Grounding, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

// ---------------------------------------------------------------------------
// The Gym: verified records for the `gym.*` and `eval.*` routes
// ---------------------------------------------------------------------------

/// What a Gym lookup may see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GymLookup {
    /// The route the router chose; the seam retrieves news only for
    /// `gym.news`.
    pub route: RouteId,
    /// The user's latest message, redacted and cut to [`MESSAGE_CHARS`],
    /// for the news retrieval's embedding and relevance judgment.
    pub message: String,
    /// The bounded transcript, for what "it" refers to.
    pub transcript: Vec<Message>,
}

/// The Gym's verified records (`crate::gym_kb`): published results and
/// checks, published test sets, adoptions, the app's changelog, and our
/// Gym product notes, each admitted only after it was verified.
pub trait GymKb: Send + Sync {
    /// Whether any records are configured.
    fn available(&self) -> bool;
    /// The services a message reaches through this seam (an embedding
    /// provider), for the privacy answer.
    fn recipients(&self) -> Vec<String>;
    /// The tool catalog, which the router's `tool` question offers; empty
    /// when unavailable, and the question is then not asked.
    fn tools(&self) -> Vec<super::gym::Tool>;
    /// Coder's adoptions, read from the latest `coder-defaults` release;
    /// with the catalog, the admitted-capability set's non-built-in
    /// entries ([`super::capability::Admitted::of`]). Empty when none is
    /// read.
    fn adoptions(&self) -> Vec<super::gym::AdoptionRecord> {
        Vec::new()
    }
    /// The verified records, and for `gym.news` the items relevant to the
    /// message.
    fn ground<'a>(
        &'a self,
        lookup: &'a GymLookup,
    ) -> BoxFuture<'a, Result<super::gym::Grounding, SeamError>>;
    /// These records for one job on the caller's own keys, as
    /// [`ProductKb::on_their_keys`].
    fn on_their_keys(&self, _theirs: &TheirKeys) -> Option<std::sync::Arc<dyn GymKb>> {
        None
    }
}

/// No Gym records: the Gym and eval routes answer with the bank or the
/// model, told it has no records.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoGym;

impl GymKb for NoGym {
    fn available(&self) -> bool {
        false
    }
    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
    fn tools(&self) -> Vec<super::gym::Tool> {
        Vec::new()
    }
    fn ground<'a>(
        &'a self,
        _: &'a GymLookup,
    ) -> BoxFuture<'a, Result<super::gym::Grounding, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

// ---------------------------------------------------------------------------
// `eval.author`: the authoring interview
// ---------------------------------------------------------------------------

/// What one step of the authoring interview may see.
#[derive(Clone, Debug, PartialEq)]
pub struct AuthorAsk {
    /// The user's latest message, redacted and cut to [`MESSAGE_CHARS`].
    pub message: String,
    /// The bounded transcript: the interview so far.
    pub transcript: Vec<Message>,
    /// The request's draft, when it carried one that passed
    /// [`super::card::draft`]: data, never an instruction.
    pub draft: Option<serde_json::Value>,
    /// A try's or a full run's result for that draft, when the request
    /// carried one that passed [`super::card::tried`]: data, never an
    /// instruction.
    pub tried: Option<ext_eval::author::runner::Tried>,
    /// Where the chat is.
    pub surface: Surface,
    /// The chat is a terminal on the computer Coder runs on (the request's
    /// surface is `terminal` and its `context.computer` is `here`), whose
    /// client runs the plugin-creation flow's steps: a request for a new
    /// plugin is that flow there (#10177).
    pub here: bool,
    /// The chat's Coder run, as the request's `context.coder_run` said:
    /// its typed ending and the bounded paths it changed, which the plugin
    /// flow reads to know whether Coder's draft is done.
    pub coder_run: Option<super::CoderRun>,
}

/// One step of the interview, as the seam wrote it. The router checks it
/// before any of it is shown ([`super::gym::check_step`]).
#[derive(Clone, Debug, PartialEq)]
pub struct AuthorStep {
    /// What we say this turn: one question or one proposal.
    pub text: String,
    /// The revised draft, shown as the `draft` card.
    pub draft: Option<serde_json::Value>,
    /// The step's action: `start_eval` on the draft (**Try it once**, the
    /// full run), `publish_eval`, or `run_coder` when the tool needs new
    /// code.
    pub offer: Option<super::Offer>,
    /// The model that wrote `text`, for the result's `model`.
    pub model: String,
    /// The plugin-creation step this turn served (#10177), carried to the
    /// client as the result's `plugin` field.
    pub plugin: Option<openagents_chat::plugin_flow::Flow>,
}

/// The authoring interview's chat driver (`crate::eval_author`, #9937):
/// one typed state machine whose steps the model proposes and Rust gates.
pub trait EvalAuthor: Send + Sync {
    /// Whether the interview is wired.
    fn available(&self) -> bool;
    /// The services a message reaches through this seam.
    fn recipients(&self) -> Vec<String>;
    /// The next step for `ask`.
    fn step<'a>(&'a self, ask: &'a AuthorAsk) -> BoxFuture<'a, Result<AuthorStep, SeamError>>;
}

/// No interview yet: `eval.author` answers with the bank's
/// `eval.author.soon`.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoAuthor;

impl EvalAuthor for NoAuthor {
    fn available(&self) -> bool {
        false
    }
    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
    fn step<'a>(&'a self, _: &'a AuthorAsk) -> BoxFuture<'a, Result<AuthorStep, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

// ---------------------------------------------------------------------------
// T4: the `openagents` command
// ---------------------------------------------------------------------------

/// One top-level command group, as the `cli_group` question lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliGroup {
    /// The group's name as typed (`computer`, `verse`, `kb`).
    pub id: String,
    /// Its one-line summary from the help table.
    pub summary: String,
    /// The group's commands as a structured subtree (`{summary, commands:
    /// {name: …}}`), which Jev reads in place of `summary` when present.
    pub tree: Option<serde_json::Value>,
}

/// What a CLI proposal may see.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliAsk {
    /// The group the router's `cli_group` question chose.
    pub group: String,
    /// Other groups the question found likely, most likely first: a route
    /// that keeps a beam descends these too and proposes the best path.
    pub also: Vec<String>,
    /// The user's latest message, redacted and cut to [`MESSAGE_CHARS`].
    pub message: String,
    /// The bounded transcript, for free-text parameters.
    pub transcript: Vec<Message>,
    /// Where the chat is, which bounds what may be proposed.
    pub surface: Surface,
}

/// A proposed command. It is an offer, not permission: it runs only on a
/// tap, under the device's own authority and the command's own checks.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CliProposal {
    /// The arguments after `openagents`, validated by the command's own
    /// parser.
    pub argv: Vec<String>,
    /// The leaf's declared effect class.
    pub effect: Effect,
    /// Where the leaf can run for this surface.
    pub runs_on: RunsOn,
}

/// What the descent found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CliAnswer {
    /// A complete, validated command.
    Proposal(CliProposal),
    /// A required parameter the message does not supply, named for a
    /// person ("which computer"); the router asks for it.
    Missing(String),
    /// No command fits (a `none` at some level of the descent).
    NoCommand,
}

/// Descends the `openagents` command tree and fills a leaf's parameters.
pub trait CliRoute: Send + Sync {
    /// The top-level groups the `cli_group` question offers, from the help
    /// table. Empty when unavailable, and the question is then not asked.
    fn groups(&self) -> Vec<CliGroup>;
    /// The services a message reaches through this seam.
    fn recipients(&self) -> Vec<String>;
    /// A command for `ask`, or why there is none.
    fn propose<'a>(&'a self, ask: &'a CliAsk) -> BoxFuture<'a, Result<CliAnswer, SeamError>>;
}

/// No command tree: the `cli` route answers with the model alone.
#[derive(Clone, Copy, Debug, Default)]
pub struct NoCli;

impl CliRoute for NoCli {
    fn groups(&self) -> Vec<CliGroup> {
        Vec::new()
    }
    fn recipients(&self) -> Vec<String> {
        Vec::new()
    }
    fn propose<'a>(&'a self, _: &'a CliAsk) -> BoxFuture<'a, Result<CliAnswer, SeamError>> {
        Box::pin(async { Err(SeamError::Unavailable) })
    }
}

// ---------------------------------------------------------------------------

/// The seams the worker holds, one of each.
#[derive(Clone)]
pub struct Seams {
    pub personalize: Arc<dyn Personalize>,
    pub product: Arc<dyn ProductKb>,
    pub codebase: Arc<dyn CodebaseKb>,
    pub cli: Arc<dyn CliRoute>,
    pub gym: Arc<dyn GymKb>,
    pub author: Arc<dyn EvalAuthor>,
}

impl Default for Seams {
    fn default() -> Self {
        Self {
            personalize: Arc::new(NoPersonalize),
            product: Arc::new(NoKb),
            codebase: Arc::new(NoKb),
            cli: Arc::new(NoCli),
            gym: Arc::new(NoGym),
            author: Arc::new(NoAuthor),
        }
    }
}

impl std::fmt::Debug for Seams {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Seams")
            .field("personalize", &self.personalize.available())
            .field("product", &self.product.available())
            .field("codebase", &self.codebase.available())
            .field("cli_groups", &self.cli.groups().len())
            .field("gym_tools", &self.gym.tools().len())
            .field("author", &self.author.available())
            .finish()
    }
}

impl Seams {
    /// Every service beyond the model door and Jev that a message may
    /// reach through these seams, deduplicated, in seam order.
    #[must_use]
    pub fn recipients(&self) -> Vec<String> {
        let mut all: Vec<String> = Vec::new();
        for name in self
            .personalize
            .recipients()
            .into_iter()
            .chain(self.product.recipients())
            .chain(self.codebase.recipients())
            .chain(self.cli.recipients())
            .chain(self.gym.recipients())
            .chain(self.author.recipients())
        {
            if !all.contains(&name) {
                all.push(name);
            }
        }
        all
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn the_default_seams_are_all_unavailable() {
        let seams = Seams::default();
        assert!(!seams.personalize.available());
        assert!(!seams.product.available());
        assert!(!seams.codebase.available());
        assert!(seams.cli.groups().is_empty());
        assert!(seams.recipients().is_empty());
        let ask = Ask {
            route: RouteId::WorkDispatch,
            answer: "dispatch.stem".into(),
            stem: "Working on".into(),
            message: "fix it".into(),
        };
        assert_eq!(
            seams.personalize.continuation(&ask).await,
            Err(SeamError::Unavailable)
        );
        let lookup = Lookup {
            message: "how do I connect my Mac".into(),
            transcript: Vec::new(),
        };
        assert_eq!(
            seams.product.ground(&lookup).await,
            Err(SeamError::Unavailable)
        );
        assert_eq!(
            seams.codebase.ground(&lookup).await,
            Err(SeamError::Unavailable)
        );
        let cli = CliAsk {
            also: Vec::new(),
            group: "computer".into(),
            message: "list my computers".into(),
            transcript: Vec::new(),
            surface: Surface::Phone,
        };
        assert_eq!(seams.cli.propose(&cli).await, Err(SeamError::Unavailable));
        assert!(!seams.gym.available());
        assert!(seams.gym.tools().is_empty());
        let gym = GymLookup {
            route: RouteId::GymNews,
            message: "what's new in the gym".into(),
            transcript: Vec::new(),
        };
        assert_eq!(seams.gym.ground(&gym).await, Err(SeamError::Unavailable));
        assert!(!seams.author.available());
        let author = AuthorAsk {
            message: "help me make a tool".into(),
            transcript: Vec::new(),
            draft: None,
            tried: None,
            surface: Surface::Phone,
            here: false,
            coder_run: None,
        };
        assert_eq!(
            seams.author.step(&author).await,
            Err(SeamError::Unavailable)
        );
    }
}
