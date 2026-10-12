//! Router v0: System One judges the request, a plain ordered policy picks
//! the harness (#11460, phase 2 of #11451).
//!
//! Code first: [`crate::harness_plugin::candidates`] keeps the harnesses
//! that are on, signed in, under their limits and allowed by the person's
//! routing preferences. Then one typed System One request per routing
//! point (the `route-v1` question set, [`questions`]) asks about the
//! request and the chat, never about words in it: what kind of work it is,
//! how big, whether it needs the repository or a shell, whether it needs
//! the person's own Claude Code setup (skills, hooks, MCP servers), how
//! risky it is, and, for a follow-up, whether it is a new piece of work or
//! a continuation. The request goes to our gateway's H100 (`/v1/systemone`
//! with `X-Decision-Order: hosted`, Clef-Flash), with Jev and our decision
//! API behind it ([`crate::issue_run::Decider::Auto`]).
//!
//! [`policy`] (`route-policy-v1`) then reads the argmax of each answer,
//! together with who pays for each harness, the Claude plan's five-hour
//! window and λ from the cost and speed sliders, and walks a short list of
//! steps in a fixed order ([`Step`]). The first step that applies picks.
//! It is not a learned optimizer and it holds no probability cutoffs: the
//! calibration note (`docs/research/typesafe/2026-10-03-calibration.md`)
//! asks for every probability to be recorded next to its outcome before any
//! threshold is fitted, so the routing record keeps them all.
//!
//! When System One does not answer within [`BUDGET`], [`fallback`] decides
//! instead: a follow-up stays on its chat's harness, a new chat takes the
//! first admitted candidate (the phase-1 fixed rule). The record then says
//! `fallback`.
//!
//! The request never blocks typing: it runs on its own thread
//! ([`ask`]) while the terminal keeps drawing, and the message waits for the
//! answer or the budget, whichever comes first.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::harness_plugin::{Harness, Payer, Turn};
use crate::routing_record::Probability;

/// The question set's frozen name. A changed question is a new version.
pub const QUESTION_SET: &str = "route-v1";

/// The policy's frozen name. A changed step or order is a new version.
pub const POLICY: &str = "route-policy-v1";

/// How long a message waits for System One before the fallback decides.
/// The gateway door answered the labelled set in 150 to 320 ms (median
/// 269 ms) from a home connection on 2026-10-12.
pub const BUDGET: Duration = Duration::from_millis(1000);

/// The share of Claude's five-hour window at which new work leans away
/// from Claude Code. This reads the plan's own usage report, not a model
/// probability.
pub const NEAR_LIMIT_PERCENT: u8 = 90;

/// What kind of work the request is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Answer,
    RepoChange,
    IssueToPr,
    Research,
}

/// How big the request is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Size {
    Small,
    Medium,
    Large,
}

/// How much harm a mistake could do.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Risk {
    Low,
    Medium,
    High,
}

const KINDS: [(&str, &str); 4] = [
    (
        "answer",
        "A reply in words: explain, define, advise, compare or chat. Nothing is changed or run.",
    ),
    (
        "repo_change",
        "Change files in the person's project: write, fix, rename, refactor, configure or deploy it.",
    ),
    (
        "issue_to_pr",
        "Take a tracked issue (a GitHub issue number or link) through to a pull request or a landed commit.",
    ),
    (
        "research",
        "Investigate and report back without changing files: read code, run commands or tests, look things up.",
    ),
];

const SIZES: [(&str, &str); 3] = [
    (
        "small",
        "A minute or two: one short answer, or a one-line change.",
    ),
    (
        "medium",
        "Several steps or a few files: minutes to under an hour.",
    ),
    (
        "large",
        "Many files or steps, a long investigation, or a multi-part project.",
    ),
];

const RISKS: [(&str, &str); 3] = [
    (
        "low",
        "Reading, answering, or a local edit that is easy to undo.",
    ),
    (
        "medium",
        "Local changes that are hard to undo: deleting files, rewriting history, large rewrites.",
    ),
    (
        "high",
        "Effects beyond this computer or hard to reverse: deploys, merges or pushes to main, payments, secrets, production data.",
    ),
];

const NEEDS_REPO: &str = "Does doing what the person asks need their repository, their files, or a shell on their computer?";
const OWN_TOOLS: &str = "Does the person ask for their own Claude Code setup: one of their skills or custom slash commands, their hooks, their MCP servers, or their Claude Code settings?";
const NEW_WORK: &str = "Is the person's latest message a new piece of work, rather than a continuation, correction, answer or reaction to what the chat is already doing?";

/// The `route-v1` questions. `follow_up` adds the new-work question, which
/// only means something when the chat already holds turns.
#[must_use]
pub fn questions(follow_up: bool) -> jev::Questions {
    let choice = |instructions: &str, options: &[(&str, &str)]| {
        let mut choice = jev::Choice::default();
        for (name, about) in options {
            choice = choice.option(*name, *about);
        }
        choice.instructions = Some(instructions.into());
        choice
    };
    let mut questions = jev::Questions::new()
        .with(
            "kind",
            choice(
                "What kind of work does the person's latest message ask for?",
                &KINDS,
            ),
        )
        .with(
            "size",
            choice(
                "How big is what the person's latest message asks for?",
                &SIZES,
            ),
        )
        .with("needs_repo", jev::Noul::new(NEEDS_REPO))
        .with("own_tools", jev::Noul::new(OWN_TOOLS))
        .with(
            "risk",
            choice(
                "If doing what the person asks went wrong, how much harm could it do?",
                &RISKS,
            ),
        );
    if follow_up {
        questions = questions.with("new_work", jev::Noul::new(NEW_WORK));
    }
    questions
}

/// The most of the chat the state carries: the latest turns, each cut
/// short.
const EARLIER_TURNS: usize = 6;
const EARLIER_CHARS: usize = 500;
const LATEST_CHARS: usize = 2000;

/// One earlier turn of the chat, for the state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Said {
    /// `user` or `assistant`.
    pub role: String,
    pub text: String,
}

/// The earlier turns of `entries` that the state shows: the person's
/// messages and the replies, oldest first; tool calls are left out.
#[must_use]
pub fn earlier(entries: &[crate::live::Entry]) -> Vec<Said> {
    entries
        .iter()
        .filter_map(|entry| match entry {
            crate::live::Entry::User(text) => Some(Said {
                role: "user".into(),
                text: text.clone(),
            }),
            crate::live::Entry::Assistant { text, .. } if !text.trim().is_empty() => Some(Said {
                role: "assistant".into(),
                text: text.clone(),
            }),
            _ => None,
        })
        .collect()
}

fn cut(text: &str, max: usize) -> String {
    let text = text.trim();
    if text.chars().count() <= max {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(max).collect();
    cut.push('…');
    cut
}

/// The state System One reads: what Coder is, the latest few turns of the
/// chat, and the person's latest message.
#[must_use]
pub fn state(earlier: &[Said], latest: &str) -> String {
    let mut text = String::from(
        "A person is talking to Coder, a coding assistant running in a terminal in their project folder.\n",
    );
    let shown = &earlier[earlier.len().saturating_sub(EARLIER_TURNS)..];
    if !shown.is_empty() {
        text.push_str("\nEarlier in this chat, oldest first:\n");
        for said in shown {
            let who = if said.role == "user" {
                "Person"
            } else {
                "Coder"
            };
            text.push_str(&format!("{who}: {}\n", cut(&said.text, EARLIER_CHARS)));
        }
    }
    text.push_str("\nThe person's latest message:\n");
    text.push_str(&cut(latest, LATEST_CHARS));
    text
}

/// One request's answers as probabilities: a distribution for each Choice
/// and the probability of yes for each Noul. This is what the routing
/// record keeps and what the fixtures replay.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    pub kind: BTreeMap<String, f64>,
    pub size: BTreeMap<String, f64>,
    pub needs_repo: f64,
    pub own_tools: f64,
    pub risk: BTreeMap<String, f64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub new_work: Option<f64>,
}

/// The argmax reading of [`Recorded`], which the policy reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Judgment {
    pub kind: Kind,
    pub size: Size,
    pub needs_repo: bool,
    pub own_tools: bool,
    pub risk: Risk,
    /// For a follow-up: whether it is a new piece of work.
    #[serde(default)]
    pub new_work: Option<bool>,
}

fn argmax(options: &BTreeMap<String, f64>) -> Option<&str> {
    options
        .iter()
        .filter(|(_, p)| p.is_finite())
        .max_by(|a, b| a.1.total_cmp(b.1))
        .map(|(name, _)| name.as_str())
}

fn parse<T: serde::de::DeserializeOwned>(name: &str) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(name.to_owned())).ok()
}

impl Recorded {
    /// Reads a System One response to [`questions`].
    ///
    /// # Errors
    /// A question went unanswered.
    pub fn from_response(response: &jev::SystemOneResponse) -> Result<Self, String> {
        let choice = |id: &str| {
            response
                .choice(id)
                .map(|answer| {
                    answer
                        .probabilities
                        .iter()
                        .map(|(name, p)| (name.clone(), *p))
                        .collect::<BTreeMap<_, _>>()
                })
                .map_err(|error| format!("{id}: {error}"))
        };
        let noul = |id: &str| {
            response
                .noul(id)
                .map(|answer| answer.noul)
                .map_err(|error| format!("{id}: {error}"))
        };
        Ok(Self {
            kind: choice("kind")?,
            size: choice("size")?,
            needs_repo: noul("needs_repo")?,
            own_tools: noul("own_tools")?,
            risk: choice("risk")?,
            new_work: response.noul("new_work").ok().map(|answer| answer.noul),
        })
    }

    /// The argmax of every answer; a yes-or-no question reads yes at one
    /// half or above. `None` when an answer names nothing this set knows.
    #[must_use]
    pub fn judgment(&self) -> Option<Judgment> {
        Some(Judgment {
            kind: parse(argmax(&self.kind)?)?,
            size: parse(argmax(&self.size)?)?,
            needs_repo: self.needs_repo >= 0.5,
            own_tools: self.own_tools >= 0.5,
            risk: parse(argmax(&self.risk)?)?,
            new_work: self.new_work.map(|p| p >= 0.5),
        })
    }

    /// Every probability, each with the model and door that gave it, as
    /// the routing record keeps them.
    #[must_use]
    pub fn probabilities(&self, model: &str, door: &str) -> Vec<Probability> {
        let one = |question: &str, answer: &str, p: f64| Probability {
            question: format!("{QUESTION_SET}.{question}"),
            answer: answer.to_owned(),
            p,
            model: model.to_owned(),
            door: door.to_owned(),
        };
        let mut out = Vec::new();
        for (question, options) in [("kind", &self.kind), ("size", &self.size)] {
            out.extend(options.iter().map(|(name, p)| one(question, name, *p)));
        }
        out.push(one("needs_repo", "yes", self.needs_repo));
        out.push(one("own_tools", "yes", self.own_tools));
        out.extend(self.risk.iter().map(|(name, p)| one("risk", name, *p)));
        if let Some(p) = self.new_work {
            out.push(one("new_work", "yes", p));
        }
        out
    }
}

/// What the policy knows besides the judgment: the admitted harnesses in
/// the person's preference order, who pays for each, the chat's harness
/// for a follow-up, the plan window and λ.
#[derive(Clone, Debug, PartialEq)]
pub struct Situation {
    /// The admitted candidates, preferred first.
    pub admitted: Vec<Harness>,
    /// The admitted harness the person's routing preferences put ahead of
    /// every other admitted one, when they do.
    pub preferred: Option<Harness>,
    /// Who pays for each admitted harness.
    pub payers: Vec<(Harness, Payer)>,
    /// The harness the chat already runs on, for a follow-up.
    pub current: Option<Harness>,
    /// The share of the Claude plan's five-hour window used, when Claude
    /// Code reported it.
    pub claude_five_hour: Option<u8>,
    /// λ, dollars per minute of waiting ([`crate::preferences::time_weight`]).
    pub time_weight: f64,
}

impl Situation {
    /// Whether this routing point is a follow-up in a chat that already
    /// runs on a harness.
    #[must_use]
    pub fn follow_up(&self) -> bool {
        self.current.is_some()
    }

    fn has(&self, harness: Harness) -> bool {
        self.admitted.contains(&harness)
    }

    fn payer(&self, harness: Harness) -> Option<Payer> {
        self.payers
            .iter()
            .find(|(h, _)| *h == harness)
            .map(|(_, payer)| *payer)
    }

    /// The first admitted harness a subscription pays for: no per-turn
    /// dollars.
    fn on_a_plan(&self) -> Option<Harness> {
        self.admitted.iter().copied().find(|harness| {
            matches!(
                self.payer(*harness),
                Some(Payer::ClaudePlan | Payer::ChatGptLogin)
            )
        })
    }

    /// The first admitted harness whose turn is one request over the
    /// transcript: no session to start, so the quickest and cheapest turn.
    fn one_request(&self) -> Option<Harness> {
        self.admitted
            .iter()
            .copied()
            .find(|harness| harness.declaration().turn == Turn::Transcript)
    }
}

/// The policy's steps, in the order it tries them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// Only one harness is admitted.
    OnlyOne,
    /// A follow-up judged a continuation stays on the chat's harness, so
    /// its session and prompt cache carry on (owner decision 1).
    Continue,
    /// The request needs the person's own Claude Code setup.
    OwnTools,
    /// The person's routing preferences put one admitted harness ahead.
    Leaning,
    /// Claude's five-hour window is nearly used up: new work goes
    /// elsewhere.
    NearLimit,
    /// A small answer that needs no repository: the cheap path.
    CheapAnswer,
    /// Time is dear (λ at least twice the balanced amount) and the work is
    /// small and safe: the quickest turn.
    QuickTurn,
    /// Repository work, an issue, large or risky work: the harness a plan
    /// pays for.
    PlanWork,
    /// Nothing else applied: the first admitted candidate.
    StandingOrder,
}

impl Step {
    /// The step's stable name, as records and fixtures write it.
    #[must_use]
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

/// The policy's pick and the step that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub harness: Harness,
    pub step: Step,
}

/// `route-policy-v1`: the first step that applies picks.
#[must_use]
pub fn policy(judgment: &Judgment, situation: &Situation) -> Decision {
    let pick = |harness, step| Decision { harness, step };
    if situation.admitted.len() <= 1 {
        return pick(
            situation
                .admitted
                .first()
                .copied()
                .unwrap_or(Harness::CoderLoop),
            Step::OnlyOne,
        );
    }
    if let Some(current) = situation.current
        && situation.has(current)
        && judgment.new_work == Some(false)
    {
        return pick(current, Step::Continue);
    }
    if judgment.own_tools && situation.has(Harness::ClaudeCode) {
        return pick(Harness::ClaudeCode, Step::OwnTools);
    }
    if let Some(preferred) = situation.preferred {
        return pick(preferred, Step::Leaning);
    }
    if situation
        .claude_five_hour
        .is_some_and(|used| used >= NEAR_LIMIT_PERCENT)
        && let Some(other) = situation
            .admitted
            .iter()
            .copied()
            .find(|harness| *harness != Harness::ClaudeCode)
    {
        return pick(other, Step::NearLimit);
    }
    if judgment.kind == Kind::Answer
        && judgment.size == Size::Small
        && !judgment.needs_repo
        && let Some(harness) = situation.one_request()
    {
        return pick(harness, Step::CheapAnswer);
    }
    if situation.time_weight >= 2.0 * crate::preferences::BALANCED_TIME_WEIGHT
        && judgment.size == Size::Small
        && judgment.risk == Risk::Low
        && let Some(harness) = situation.one_request()
    {
        return pick(harness, Step::QuickTurn);
    }
    if (matches!(judgment.kind, Kind::RepoChange | Kind::IssueToPr)
        || judgment.needs_repo
        || judgment.size == Size::Large
        || judgment.risk == Risk::High)
        && let Some(harness) = situation.on_a_plan()
    {
        return pick(harness, Step::PlanWork);
    }
    pick(situation.admitted[0], Step::StandingOrder)
}

/// The decision when System One did not answer: a follow-up stays on its
/// chat's harness while it is admitted; otherwise the first admitted
/// candidate (the phase-1 fixed rule).
#[must_use]
pub fn fallback(situation: &Situation) -> Harness {
    situation
        .current
        .filter(|current| situation.has(*current))
        .or_else(|| situation.admitted.first().copied())
        .unwrap_or(Harness::CoderLoop)
}

/// A System One answer to one routing request.
#[derive(Clone, Debug, PartialEq)]
pub struct Answered {
    pub recorded: Recorded,
    /// The model that answered.
    pub model: String,
    /// The door it answered through.
    pub door: String,
    /// How long the request took.
    pub ms: u64,
}

/// What the router did at one routing point, as the record keeps it.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Trace {
    pub question_set: String,
    pub policy: String,
    /// The policy step that picked, or `None` for the fallback.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    pub follow_up: bool,
    /// How long System One took to answer, or how long the message waited
    /// for it.
    pub ms: u64,
    /// Why there was no answer, scrubbed, when there was none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub missing: Option<String>,
}

/// The router's call for one routing point.
#[derive(Clone, Debug, PartialEq)]
pub struct Routed {
    pub harness: Harness,
    pub rule: crate::routing_record::Rule,
    pub probabilities: Vec<Probability>,
    pub trace: Trace,
}

/// Decides from System One's answer, or from the fallback when there is
/// none or it cannot be read.
#[must_use]
pub fn decide(answer: Result<Answered, String>, situation: &Situation, waited: Duration) -> Routed {
    use crate::routing_record::Rule;
    let follow_up = situation.follow_up();
    let mut trace = Trace {
        question_set: QUESTION_SET.into(),
        policy: POLICY.into(),
        step: None,
        follow_up,
        ms: u64::try_from(waited.as_millis()).unwrap_or(u64::MAX),
        missing: None,
    };
    match answer {
        Ok(answered) => {
            trace.ms = answered.ms;
            let probabilities = answered
                .recorded
                .probabilities(&answered.model, &answered.door);
            if let Some(judgment) = answered.recorded.judgment() {
                let decision = policy(&judgment, situation);
                trace.step = Some(decision.step.name());
                return Routed {
                    harness: decision.harness,
                    rule: Rule::Router,
                    probabilities,
                    trace,
                };
            }
            trace.missing =
                Some("An answer named an option this question set does not have.".into());
            Routed {
                harness: fallback(situation),
                rule: Rule::Fallback,
                probabilities,
                trace,
            }
        }
        Err(why) => {
            trace.missing = Some(crate::routing_record::scrub(&why));
            Routed {
                harness: fallback(situation),
                rule: Rule::Fallback,
                probabilities: Vec::new(),
                trace,
            }
        }
    }
}

/// Asks System One about the latest message on a thread of its own and
/// hands back where the answer will arrive. The thread gives up at
/// [`BUDGET`].
#[must_use]
pub fn ask(
    earlier: &[Said],
    latest: &str,
    follow_up: bool,
) -> mpsc::Receiver<Result<Answered, String>> {
    let (sender, receiver) = mpsc::channel();
    let request = jev::SystemOneRequest::new(state(earlier, latest), questions(follow_up));
    #[cfg(test)]
    {
        if let Some(Some(recorded)) = TEST_JUDGE.with(|judge| judge.borrow().clone()) {
            let _ = sender.send(Ok(Answered {
                recorded,
                model: "test".into(),
                door: "test".into(),
                ms: 0,
            }));
        }
        // A judge that never answers drops the sender: the message falls
        // back at once.
        drop(request);
        receiver
    }
    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            let _ = sender.send(ask_now(request));
        });
        receiver
    }
}

/// One routing request, answered or given up on within [`BUDGET`].
///
/// # Errors
/// No door answered in time, or the answer could not be read.
pub fn ask_now(request: jev::SystemOneRequest) -> Result<Answered, String> {
    let started = Instant::now();
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| error.to_string())?;
    let (response, door) = runtime
        .block_on(async {
            tokio::time::timeout(
                BUDGET,
                crate::issue_run::decide::ask(crate::issue_run::Decider::Auto, request),
            )
            .await
        })
        .map_err(|_| "System One did not answer in time.".to_owned())??;
    Ok(Answered {
        recorded: Recorded::from_response(&response)?,
        model: response.model.clone(),
        door,
        ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
    })
}

/// Whether routing points ask System One. `CODER_ROUTER=off` turns it off,
/// and then the fixed rule routes as in phase 1. Unit tests ask only a
/// judge they set ([`set_test_judge`]).
#[must_use]
pub fn enabled() -> bool {
    #[cfg(test)]
    {
        TEST_JUDGE.with(|judge| judge.borrow().is_some())
    }
    #[cfg(not(test))]
    {
        std::env::var("CODER_ROUTER").map_or(true, |value| value.trim() != "off")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_JUDGE: std::cell::RefCell<Option<Option<Recorded>>> =
        const { std::cell::RefCell::new(None) };
}

/// Sets this test thread's System One: `None` turns routing requests off,
/// `Some(None)` is a System One that never answers, `Some(Some(answers))`
/// answers every request with `answers`.
#[cfg(test)]
pub(crate) fn set_test_judge(judge: Option<Option<Recorded>>) {
    TEST_JUDGE.with(|cell| *cell.borrow_mut() = judge);
}

/// A message held while System One judges it: where it runs, what its
/// briefing carries ([`crate::context`]), or both.
pub struct Pending {
    /// The route question, when one was asked.
    pub receiver: Option<mpsc::Receiver<Result<Answered, String>>>,
    /// Its answer, or why there is none, once that is known.
    pub answer: Option<Result<Answered, String>>,
    /// The briefing's selection, when one is under way.
    pub context: Option<crate::context::Asking>,
    /// Whether a selection was considered after the route's answer, so a
    /// follow-up starts at most one.
    pub context_considered: bool,
    pub started: Instant,
    pub situation: Situation,
    /// The chat the message was sent in: the answer lands there or
    /// nowhere, never in a chat opened while it waited.
    pub session: Option<String>,
    /// The message and its attachments, put back when the answer comes.
    pub draft: crate::Draft,
    pub composer: crate::composer_state::ComposerState,
}

impl Pending {
    /// Moves the route's answer in once there is one, or the budget ran
    /// out. Returns whether the route side is settled (or was never asked).
    pub fn poll_route(&mut self) -> bool {
        if self.answer.is_some() {
            return true;
        }
        let Some(receiver) = &self.receiver else {
            return true;
        };
        self.answer = match receiver.try_recv() {
            Ok(answer) => Some(answer),
            Err(mpsc::TryRecvError::Disconnected) => {
                Some(Err("System One could not be asked.".into()))
            }
            Err(mpsc::TryRecvError::Empty) if self.started.elapsed() >= BUDGET => {
                Some(Err("System One did not answer in time.".into()))
            }
            Err(mpsc::TryRecvError::Empty) => None,
        };
        self.answer.is_some()
    }

    /// Waits for the route's answer up to what is left of the budget, for
    /// callers with no screen to keep drawing.
    pub fn wait_route(&mut self) {
        if self.answer.is_some() {
            return;
        }
        let Some(receiver) = &self.receiver else {
            return;
        };
        let left = BUDGET.saturating_sub(self.started.elapsed());
        self.answer = Some(match receiver.recv_timeout(left) {
            Ok(answer) => answer,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                Err("System One did not answer in time.".into())
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("System One could not be asked.".into())
            }
        });
    }

    /// Whether the route's answer judged this follow-up a new piece of
    /// work.
    #[must_use]
    pub fn new_work(&self) -> bool {
        matches!(
            &self.answer,
            Some(Ok(answered)) if answered
                .recorded
                .judgment()
                .and_then(|judgment| judgment.new_work)
                == Some(true)
        )
    }
}

#[cfg(test)]
#[path = "router_tests.rs"]
mod tests;
