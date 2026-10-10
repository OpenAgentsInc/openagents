//! Own coding capacity (`docs/inference/gateway.md`, sections 4 and 5;
//! issue #11080): `openagents/code` under `pay: "mine"` run by the key
//! owner's own Coder, on their own linked computers, with their own Codex
//! or Claude Code subscriptions.
//!
//! - **Own capacity only.** The gateway builds these upstreams per caller
//!   from the linked computers of the account the caller's key resolves to
//!   ([`own_upstreams`]) and hands them to the router as the caller's own
//!   ([`crate::run::Caller::own`]), so they are offered only to that
//!   caller's `pay: "mine"` requests. Never another person's request,
//!   never pooled, never resold.
//! - **Capacity is a quantity.** Each subscription account on a linked
//!   computer reports how many more sessions it can take now
//!   ([`Linked::free_sessions`]). Each account is its own upstream,
//!   `coder:<computer>/<account>`; one with no free session is left out,
//!   and the rest are offered most free sessions first. An account that
//!   turns out to be spent answers [`ErrorClass::Payment`], so the router
//!   benches it and falls over to the owner's next account before any
//!   output (episode 246).
//! - **The class is judged, never matched.** These upstreams join
//!   `openagents/code`, which a request names or `openagents/auto` reaches
//!   by its typed judgment; nothing here reads the prompt to decide.
//! - **Progress, then the answer.** A run streams what it is doing as
//!   Open Responses events ([`super::progress`]). The first line names the
//!   computer and the account doing the work.
//! - **Price.** Zero: the owner's subscription pays, and the gateway adds
//!   no fee. Attempts are metered like every other, under the caller's own
//!   account.
//! - **Privacy.** The run goes from the owner's computer to their own
//!   subscription's provider, under that provider's terms, which we have
//!   not verified as zero retention. Like a direct own key, it takes only
//!   `openagents.privacy: "standard"`.
//!
//! [`Runs`] starts a run on one account of one computer and reports its
//! progress. The gateway's reaches the computer through the owner's
//! Coder link; tests use a stub.

use std::sync::Arc;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};

use super::progress::{Brief, Progress, brief};
pub use super::progress::{Report, Reports};
use super::{
    Account, AttemptError, AttemptMeter, BoxFuture, Capabilities, CostBasis, ErrorClass,
    EventStream, ModelRow, Price, PrivacyTerms, Sent, Upstream, check,
};
use crate::event::Event;
use crate::request::CreateResponse;

/// The model id prefix of own coding capacity: `coder/codex`,
/// `coder/claude-code`.
pub const MODEL_PREFIX: &str = "coder/";

/// The upstream name prefix: `coder:<computer>/<account>`.
pub const UPSTREAM_PREFIX: &str = "coder:";

/// The most conversation text a run is handed, in bytes.
pub const MAX_BRIEF: usize = 256 * 1024;

/// Whether `model` is an own-capacity model id.
#[must_use]
pub fn is_own_capacity(model: &str) -> bool {
    Agent::of_model(model).is_some()
}

/// The coding agent a subscription runs.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Agent {
    Codex,
    ClaudeCode,
}

impl Agent {
    pub const ALL: [Self; 2] = [Self::Codex, Self::ClaudeCode];

    /// The public model id.
    #[must_use]
    pub fn model(self) -> &'static str {
        match self {
            Self::Codex => "coder/codex",
            Self::ClaudeCode => "coder/claude-code",
        }
    }

    /// The agent's name as people know it.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::Codex => "Codex",
            Self::ClaudeCode => "Claude Code",
        }
    }

    /// The agent a public model id names.
    #[must_use]
    pub fn of_model(model: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|agent| agent.model() == model)
    }
}

/// One subscription account on one of the owner's linked computers.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Linked {
    /// The account the caller's key resolves to; only its own requests
    /// reach this computer.
    pub owner: String,
    /// The linked computer's id (its host key).
    pub computer: String,
    /// The computer's name, as the owner sees it ("Studio").
    #[serde(default)]
    pub computer_label: String,
    /// The subscription account's id on that computer.
    pub account: String,
    /// The account's name, as the owner sees it ("Codex (work)").
    #[serde(default)]
    pub account_label: String,
    pub agent: Agent,
    /// How many more sessions the account can take now.
    pub free_sessions: u32,
}

fn plain_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
}

impl Linked {
    /// The upstream's name.
    #[must_use]
    pub fn upstream(&self) -> String {
        format!("{UPSTREAM_PREFIX}{}/{}", self.computer, self.account)
    }

    /// Whether the record is usable: an owner, and plain ids for the
    /// computer and the account.
    ///
    /// # Errors
    ///
    /// A sentence naming what is wrong.
    pub fn check(&self) -> Result<(), String> {
        if self.owner.is_empty() {
            return Err("a linked computer needs its owner".into());
        }
        if !plain_id(&self.computer) || !plain_id(&self.account) {
            return Err("computer and account ids are letters, digits, `-`, `_`, and `.`".into());
        }
        Ok(())
    }

    fn computer_name(&self) -> &str {
        if self.computer_label.is_empty() {
            &self.computer
        } else {
            &self.computer_label
        }
    }

    fn account_name(&self) -> &str {
        if self.account_label.is_empty() {
            &self.account
        } else {
            &self.account_label
        }
    }
}

/// One run to start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    pub owner: String,
    pub computer: String,
    pub account: String,
    pub agent: Agent,
    pub brief: Brief,
}

/// Starts runs on the owner's linked computers.
pub trait Runs: Send + Sync {
    /// Starts `run`.
    ///
    /// # Errors
    ///
    /// Before any progress: [`ErrorClass::Payment`] when the account's
    /// subscription is spent, [`ErrorClass::RateLimited`] when it has no
    /// free session after all, [`ErrorClass::Connection`] when the
    /// computer cannot be reached, [`ErrorClass::Auth`] when the agent is
    /// signed out.
    fn start(&self, run: Run) -> BoxFuture<'_, Result<Reports, AttemptError>>;
}

/// The capabilities an own-capacity model is offered with: a coding run
/// takes text and answers text, with no function tools, images, or schema.
const CAPABILITIES: Capabilities = Capabilities {
    tools: false,
    reasoning: true,
    reasoning_always_on: true,
    json_schema: false,
    images: false,
    context: 1_000_000,
    max_output: 128_000,
};

/// The model row for `agent`, at no charge.
#[must_use]
pub fn row(agent: Agent) -> ModelRow {
    ModelRow {
        id: agent.model().to_owned(),
        upstream_model: match agent {
            Agent::Codex => "codex",
            Agent::ClaudeCode => "claude",
        }
        .to_owned(),
        capabilities: CAPABILITIES,
        price: Price::default(),
        price_source: "the caller's own subscription: no charge",
    }
}

/// One subscription account on one of the owner's linked computers, as
/// an upstream.
pub struct OwnCoder {
    name: String,
    linked: Linked,
    account: Account,
    privacy: PrivacyTerms,
    models: Vec<ModelRow>,
    runs: Arc<dyn Runs>,
}

impl OwnCoder {
    /// The upstream for `linked`, starting runs through `runs`.
    ///
    /// # Errors
    ///
    /// The record's problem ([`Linked::check`]).
    pub fn new(linked: Linked, runs: Arc<dyn Runs>) -> Result<Self, String> {
        linked.check()?;
        let name = linked.upstream();
        Ok(Self {
            account: Account {
                id: name.clone(),
                basis: CostBasis::FreeCapacity,
            },
            privacy: PrivacyTerms::unverified(
                "the caller's own subscription on their own computer; its provider's terms apply",
            ),
            models: vec![row(linked.agent)],
            name,
            linked,
            runs,
        })
    }

    /// The linked account this upstream runs on.
    #[must_use]
    pub fn linked(&self) -> &Linked {
        &self.linked
    }
}

/// The caller's own-capacity upstreams from `linked`: only the records of
/// `owner`, usable ones, with a free session, most free sessions first
/// (ties keep their order). Each is its own upstream, so the router fails
/// over from one account to the next.
#[must_use]
pub fn own_upstreams(
    owner: &str,
    mut linked: Vec<Linked>,
    runs: &Arc<dyn Runs>,
) -> Vec<Arc<dyn Upstream>> {
    linked.retain(|record| record.owner == owner && record.free_sessions > 0);
    linked.sort_by(|a, b| b.free_sessions.cmp(&a.free_sessions));
    linked
        .into_iter()
        .filter_map(|record| OwnCoder::new(record, runs.clone()).ok())
        .map(|upstream| Arc::new(upstream) as Arc<dyn Upstream>)
        .collect()
}

impl Upstream for OwnCoder {
    fn name(&self) -> &str {
        &self.name
    }

    fn account(&self) -> &Account {
        &self.account
    }

    fn privacy(&self) -> &PrivacyTerms {
        &self.privacy
    }

    fn models(&self) -> &[ModelRow] {
        &self.models
    }

    fn configured(&self) -> bool {
        self.linked.free_sessions > 0
    }

    fn send<'a>(
        &'a self,
        request: &'a CreateResponse,
        model: &'a str,
    ) -> BoxFuture<'a, Result<Sent, AttemptError>> {
        Box::pin(async move {
            let row = check(self, request, model)?;
            if self.linked.free_sessions == 0 {
                return Err(AttemptError::new(
                    ErrorClass::RateLimited,
                    format!("{} has no free session", self.name),
                ));
            }
            let brief = brief(request, MAX_BRIEF)?;
            let meter = AttemptMeter::start(self, row);
            let run = Run {
                owner: self.linked.owner.clone(),
                computer: self.linked.computer.clone(),
                account: self.linked.account.clone(),
                agent: self.linked.agent,
                brief,
            };
            let reports = match self.runs.start(run).await {
                Ok(reports) => reports,
                Err(error) => {
                    meter.fail(&error);
                    return Err(error);
                }
            };
            meter.status(200);
            let mut progress = Progress::new(request, &row.id);
            let opening = progress.start(&format!(
                "Working on {} with your {} account {}.",
                self.linked.computer_name(),
                self.linked.agent.label(),
                self.linked.account_name(),
            ));
            let events = stream(progress, opening, reports);
            Ok(Sent {
                events: meter.wrap(events),
                meter,
            })
        })
    }
}

/// The run's reports as events after `opening`.
fn stream(progress: Progress, opening: Vec<Event>, reports: Reports) -> EventStream {
    let opening = futures_util::stream::iter(opening.into_iter().map(Ok::<Event, AttemptError>));
    let rest = futures_util::stream::unfold(
        (progress, Some(reports)),
        |(mut progress, reports)| async move {
            let Some(mut reports) = reports else {
                return None;
            };
            let events = match reports.next().await {
                Some(Report::Step(line)) => progress.step(&line),
                Some(Report::Done { text, usage }) if !text.is_empty() => {
                    progress.finish(&text, usage)
                }
                Some(Report::Done { .. }) => {
                    progress.fail("run_empty", "The run finished without an answer.")
                }
                Some(Report::Failed(why)) => progress.fail("run_failed", &why),
                None => progress.fail("run_failed", "The run stopped before it finished."),
            };
            let next = (!progress.ended()).then_some(reports);
            Some((events, (progress, next)))
        },
    );
    Box::pin(opening.chain(rest.flat_map(|events| {
        futures_util::stream::iter(events.into_iter().map(Ok::<Event, AttemptError>))
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::openagents::Privacy;
    use crate::response::Usage;
    use crate::sse::StreamItem;
    use crate::stream::{Accumulator, StreamCheck};
    use serde_json::json;
    use std::sync::Mutex;

    fn linked(computer: &str, account: &str, agent: Agent, free: u32) -> Linked {
        Linked {
            owner: "acct_owner".into(),
            computer: computer.into(),
            computer_label: "Studio".into(),
            account: account.into(),
            account_label: "Codex (work)".into(),
            agent,
            free_sessions: free,
        }
    }

    /// Answers each run from a script, keyed by account, and keeps the
    /// runs it was asked for.
    struct Script {
        spent: Vec<String>,
        started: Mutex<Vec<Run>>,
    }

    impl Runs for Script {
        fn start(&self, run: Run) -> BoxFuture<'_, Result<Reports, AttemptError>> {
            Box::pin(async move {
                if self.spent.contains(&run.account) {
                    return Err(AttemptError::new(
                        ErrorClass::Payment,
                        "the subscription is spent",
                    ));
                }
                let account = run.account.clone();
                self.started.lock().unwrap().push(run);
                let reports: Reports = Box::pin(futures_util::stream::iter(vec![
                    Report::Step("Reading the code.".into()),
                    Report::Done {
                        text: format!("Done on {account}."),
                        usage: Some(Usage::new(100, 0, 20, 0)),
                    },
                ]));
                Ok(reports)
            })
        }
    }

    fn request() -> CreateResponse {
        serde_json::from_value(json!({
            "model": "openagents/code",
            "input": "fix the login bug",
            "openagents": {"pay": "mine", "privacy": "standard"}
        }))
        .unwrap()
    }

    async fn collect(sent: Sent) -> Vec<Event> {
        sent.events.map(|event| event.unwrap()).collect().await
    }

    #[test]
    fn records_are_checked_and_named() {
        let mut record = linked("host-1", "codex-1", Agent::Codex, 2);
        assert!(record.check().is_ok());
        assert_eq!(record.upstream(), "coder:host-1/codex-1");
        record.account = "../x".into();
        assert!(record.check().is_err());
        assert!(is_own_capacity("coder/codex") && is_own_capacity("coder/claude-code"));
        assert!(!is_own_capacity("openai/gpt-5.6-sol"));
    }

    #[test]
    fn only_the_owners_accounts_with_free_sessions_are_offered_most_free_first() {
        let runs: Arc<dyn Runs> = Arc::new(Script {
            spent: Vec::new(),
            started: Mutex::new(Vec::new()),
        });
        let mut other = linked("host-9", "codex-9", Agent::Codex, 9);
        other.owner = "acct_someone_else".into();
        let upstreams = own_upstreams(
            "acct_owner",
            vec![
                linked("host-1", "codex-1", Agent::Codex, 1),
                linked("host-1", "claude-1", Agent::ClaudeCode, 0),
                linked("host-2", "codex-2", Agent::Codex, 3),
                other,
            ],
            &runs,
        );
        let names: Vec<&str> = upstreams.iter().map(|upstream| upstream.name()).collect();
        assert_eq!(names, ["coder:host-2/codex-2", "coder:host-1/codex-1"]);
        let offering = &upstreams[0].offerings()[0];
        assert_eq!(offering.model, "coder/codex");
        // Standard only: the provider's terms are the owner's, not verified.
        assert!(!offering.zero_retention);
        assert!(upstreams[0].privacy().allows(&Privacy::Standard));
        assert_eq!(upstreams[0].rate_rows()[0].output, 0);
    }

    #[tokio::test]
    async fn a_run_streams_progress_naming_the_computer_and_account_then_the_answer() {
        let script = Arc::new(Script {
            spent: Vec::new(),
            started: Mutex::new(Vec::new()),
        });
        let runs: Arc<dyn Runs> = script.clone();
        let upstream = OwnCoder::new(linked("host-1", "codex-1", Agent::Codex, 1), runs).unwrap();
        let sent = upstream.send(&request(), "coder/codex").await.unwrap();
        let meter = sent.meter.clone();
        let events = collect(sent).await;
        let items: Vec<StreamItem> = events.iter().cloned().map(StreamItem::Event).collect();
        let violations = StreamCheck::run(items.iter().chain([&StreamItem::Done]));
        assert!(violations.is_empty(), "{violations:?}");
        let mut folded = Accumulator::new();
        for event in &events {
            folded.push(event);
        }
        let response = folded.finish().unwrap();
        assert_eq!(response.output_text(), "Done on codex-1.");
        let summary = match &response.output[0] {
            crate::item::Item::Reasoning(reasoning) => reasoning.summary_text(),
            other => panic!("expected progress, got {other:?}"),
        };
        assert_eq!(
            summary,
            "Working on Studio with your Codex account Codex (work).\nReading the code."
        );
        let started = script.started.lock().unwrap();
        assert_eq!(started[0].brief.task, "fix the login bug");
        assert_eq!(started[0].owner, "acct_owner");
        let measure = meter.snapshot();
        assert_eq!(measure.tokens.input, 100);
        assert_eq!(measure.stage, super::super::Stage::Completed);
    }

    #[tokio::test]
    async fn a_spent_account_refuses_before_any_output_so_the_router_falls_over() {
        let runs: Arc<dyn Runs> = Arc::new(Script {
            spent: vec!["codex-1".into()],
            started: Mutex::new(Vec::new()),
        });
        let upstream = OwnCoder::new(linked("host-1", "codex-1", Agent::Codex, 1), runs).unwrap();
        let error = upstream
            .send(&request(), "coder/codex")
            .await
            .err()
            .unwrap();
        assert_eq!(error.class, ErrorClass::Payment);
        assert!(error.class.benches() && error.class.falls_back());
    }

    #[tokio::test]
    async fn strict_privacy_and_other_models_are_refused_before_sending() {
        let runs: Arc<dyn Runs> = Arc::new(Script {
            spent: Vec::new(),
            started: Mutex::new(Vec::new()),
        });
        let upstream = OwnCoder::new(linked("host-1", "codex-1", Agent::Codex, 1), runs).unwrap();
        let strict: CreateResponse = serde_json::from_value(json!({
            "model": "openagents/code", "input": "x", "openagents": {"pay": "mine"}
        }))
        .unwrap();
        let error = upstream.send(&strict, "coder/codex").await.err().unwrap();
        assert_eq!(error.class, ErrorClass::PrivacyRefused);
        let error = upstream
            .send(&request(), "coder/claude-code")
            .await
            .err()
            .unwrap();
        assert_eq!(error.class, ErrorClass::Unsupported);
    }

    #[tokio::test]
    async fn a_run_that_stops_early_fails_in_plain_words() {
        struct Stops;
        impl Runs for Stops {
            fn start(&self, _run: Run) -> BoxFuture<'_, Result<Reports, AttemptError>> {
                Box::pin(async {
                    let reports: Reports =
                        Box::pin(futures_util::stream::iter(vec![Report::Step(
                            "Started.".into(),
                        )]));
                    Ok(reports)
                })
            }
        }
        let upstream = OwnCoder::new(
            linked("host-1", "codex-1", Agent::Codex, 1),
            Arc::new(Stops),
        )
        .unwrap();
        let events = collect(upstream.send(&request(), "coder/codex").await.unwrap()).await;
        let last = events.last().unwrap();
        assert_eq!(last.type_name(), "response.failed");
        let message = last
            .body
            .response()
            .and_then(|response| response.error.as_ref())
            .map(|error| error.message.clone());
        assert_eq!(
            message.as_deref(),
            Some("The run stopped before it finished.")
        );
    }
}
