//! Escalation at checkpoints (#11474, phase 4 of #11451).
//!
//! Owner decision 1: route each new piece of work, keep follow-ups on their
//! harness, and move work to another harness only at a checkpoint, with a
//! briefing, never mid-turn. A checkpoint is a turn that ended with failed
//! checks (`checks_passed = false`, [`crate::routing_record::turn_checks`]),
//! an error, a usage limit, or a stall (the same failing step over and
//! over, [`crate::stuck`]), or a message that asks for the same thing again
//! (a redo).
//!
//! At a checkpoint one typed System One request (`escalate-v1`,
//! [`questions`]) asks what to do: stop and let the person decide, try
//! again here, or hand the work to a stronger harness, and why. It goes to
//! the gateway's H100 with Jev behind it, like the router's. [`policy`]
//! (`escalate-policy-v1`) then picks from that judgment, the admitted
//! harnesses and the Claude plan's window, in a short fixed order of steps
//! ([`Step`]). A usage limit needs no question: the work moves to another
//! admitted harness when there is one.
//!
//! A move carries a briefing ([`handoff`]): the task, what happened, the
//! failing check and its output, and the files changed so far, as the next
//! message. The transcript itself goes along the way a move to Claude Code
//! already seeds it ([`crate::live::Chat::claude_resume`]). The person sees
//! which agent took over in plain words. Each piece of work moves or retries
//! at most once each, so a checkpoint never loops.
//!
//! Every call writes an `escalated` routing line: the from and to
//! harnesses, the checkpoint, the probabilities and the step. `gym coder
//! routing` reads them for its escalation section.

use std::collections::BTreeMap;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

use crate::harness_plugin::Harness;
use crate::routing_record::Probability;

/// The question set's frozen name.
pub const QUESTION_SET: &str = "escalate-v1";

/// The policy's frozen name.
pub const POLICY: &str = "escalate-policy-v1";

/// How long a checkpoint waits for System One before it leaves the work
/// where it is.
pub const BUDGET: Duration = Duration::from_millis(1500);

/// What ended the turn.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Checkpoint {
    /// The checks the turn ran still fail.
    FailedChecks,
    /// The person asked for the same thing again.
    Redo,
    /// The turn ended with an error.
    Error,
    /// The harness reached a usage limit.
    Limit,
    /// The turn kept repeating the same failing step.
    Stall,
}

impl Checkpoint {
    /// What happened, in plain words.
    #[must_use]
    pub fn words(self) -> &'static str {
        match self {
            Self::FailedChecks => "the checks still fail",
            Self::Redo => "you asked for this again",
            Self::Error => "the turn ended with an error",
            Self::Limit => "it reached its usage limit",
            Self::Stall => "it kept repeating the same failing step",
        }
    }
}

/// The checkpoint a finished turn reached, if any: its error (a usage
/// limit or another one), a stall, or failed checks. `at_limit` says the
/// harness's plan reported a window at its limit; the gateway's own limit
/// is read from the error Coder wrote for it.
#[must_use]
pub fn reached(
    error: Option<&str>,
    at_limit: bool,
    stalled: bool,
    checks: Option<bool>,
) -> Option<Checkpoint> {
    if let Some(error) = error {
        return Some(
            if at_limit || crate::long_session::pause_seconds(error).is_some() {
                Checkpoint::Limit
            } else {
                Checkpoint::Error
            },
        );
    }
    if stalled {
        return Some(Checkpoint::Stall);
    }
    (checks == Some(false)).then_some(Checkpoint::FailedChecks)
}

/// What to do at a checkpoint.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Continue,
    Retry,
    Move,
}

/// Why.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    BeyondAgent,
    Slip,
    NeedsPerson,
    Outside,
}

const ACTIONS: [(&str, &str); 3] = [
    (
        "continue",
        "Stop here and tell the person what happened; they decide what comes next.",
    ),
    (
        "retry",
        "Try again on the same agent with what was learned: the failure looks like something it can fix.",
    ),
    (
        "move",
        "Hand the work to a stronger agent with a briefing: this agent is unlikely to finish it.",
    ),
];

const REASONS: [(&str, &str); 4] = [
    (
        "beyond_agent",
        "The work is beyond this agent: hard, long, or it needs deeper reasoning than it showed.",
    ),
    (
        "slip",
        "A small slip the agent can fix: a typo, a missed file, a wrong command or path.",
    ),
    (
        "needs_person",
        "It needs the person: missing access or a secret, an unclear or risky request, a decision only they can make.",
    ),
    (
        "outside",
        "The failure is not the agent's doing: a flaky or unrelated check, the network, or a service that is down.",
    ),
];

/// The `escalate-v1` questions.
#[must_use]
pub fn questions() -> jev::Questions {
    let choice = |instructions: &str, options: &[(&str, &str)]| {
        let mut choice = jev::Choice::default();
        for (name, about) in options {
            choice = choice.option(*name, *about);
        }
        choice.instructions = Some(instructions.into());
        choice
    };
    jev::Questions::new()
        .with(
            "action",
            choice(
                "The agent's turn just ended as described. What should happen next?",
                &ACTIONS,
            ),
        )
        .with("reason", choice("Why did the turn end this way?", &REASONS))
}

/// The most of a failing check's output the state and the briefing carry.
const DETAIL_CHARS: usize = 1500;

/// What the turn left behind: its failing check, if any, and the files it
/// changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Evidence {
    /// The last failing check command and the tail of its output.
    pub failed_check: Option<(String, String)>,
    /// The files the turn wrote or edited, in order.
    pub files: Vec<String>,
}

/// The evidence in a turn's entries (those after its message).
#[must_use]
pub fn evidence(entries: &[crate::live::Entry]) -> Evidence {
    let mut out = Evidence::default();
    for entry in entries {
        let crate::live::Entry::Tool {
            name,
            input,
            output,
            running: false,
        } = entry
        else {
            continue;
        };
        let command = input
            .get("command")
            .or_else(|| input.get("cmd"))
            .and_then(serde_json::Value::as_str);
        if let Some(command) = command.filter(|command| atif::activity::is_check_command(command)) {
            out.failed_check = crate::tools::output_failed(output).then(|| {
                let text = output
                    .get("text")
                    .or_else(|| output.get("output"))
                    .or_else(|| output.get("stdout"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                (command.to_owned(), tail(text, DETAIL_CHARS))
            });
            continue;
        }
        if matches!(name.as_str(), "Edit" | "Write")
            && let Some(path) = output
                .get("path")
                .or_else(|| input.get("path"))
                .or_else(|| input.get("file_path"))
                .and_then(serde_json::Value::as_str)
            && !out.files.iter().any(|file| file == path)
        {
            out.files.push(path.to_owned());
        }
    }
    out
}

fn tail(text: &str, max: usize) -> String {
    let count = text.chars().count();
    if count <= max {
        return text.trim().to_owned();
    }
    let tail: String = text.chars().skip(count - max).collect();
    format!("…{}", tail.trim())
}

/// The entries of the turn that just ended: everything after the last
/// message.
#[must_use]
pub fn last_turn(entries: &[crate::live::Entry]) -> &[crate::live::Entry] {
    let start = entries
        .iter()
        .rposition(|entry| matches!(entry, crate::live::Entry::User(_)))
        .map_or(0, |index| index + 1);
    &entries[start..]
}

/// The state System One reads at a checkpoint.
#[must_use]
pub fn state(
    earlier: &[crate::router::Said],
    task: &str,
    from: &str,
    checkpoint: Checkpoint,
    evidence: &Evidence,
) -> String {
    let mut text = crate::router::state(earlier, task);
    text.push_str(&format!(
        "\n\nThe agent working on this was {from}. Its turn just ended: {}.",
        checkpoint.words()
    ));
    if let Some((command, output)) = &evidence.failed_check {
        text.push_str(&format!(
            "\nThe failing check: {command}\nIts output ends:\n{output}"
        ));
    }
    if !evidence.files.is_empty() {
        text.push_str(&format!(
            "\nFiles it changed: {}",
            evidence.files.join(", ")
        ));
    }
    text
}

/// One checkpoint's answers as probabilities.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    pub action: BTreeMap<String, f64>,
    pub reason: BTreeMap<String, f64>,
}

/// The argmax reading of [`Recorded`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Judgment {
    pub action: Action,
    pub reason: Reason,
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
        Ok(Self {
            action: choice("action")?,
            reason: choice("reason")?,
        })
    }

    #[must_use]
    pub fn judgment(&self) -> Option<Judgment> {
        Some(Judgment {
            action: parse(argmax(&self.action)?)?,
            reason: parse(argmax(&self.reason)?)?,
        })
    }

    /// Every probability, with the model and door that gave it.
    #[must_use]
    pub fn probabilities(&self, model: &str, door: &str) -> Vec<Probability> {
        let one = |question: &str, answer: &str, p: f64| Probability {
            question: format!("{QUESTION_SET}.{question}"),
            answer: answer.to_owned(),
            p,
            model: model.to_owned(),
            door: door.to_owned(),
        };
        self.action
            .iter()
            .map(|(name, p)| one("action", name, *p))
            .chain(self.reason.iter().map(|(name, p)| one("reason", name, *p)))
            .collect()
    }
}

/// What the policy knows besides the judgment.
#[derive(Clone, Debug, PartialEq)]
pub struct Situation {
    /// The harness the turn ran on.
    pub current: Harness,
    /// The admitted harnesses, preferred first.
    pub admitted: Vec<Harness>,
    /// The share of the Claude plan's five-hour window used, when known.
    pub claude_five_hour: Option<u8>,
    /// This piece of work was already tried again once.
    pub retried: bool,
    /// This piece of work already moved once.
    pub moved: bool,
}

/// The stronger harness for work `current` could not finish: Claude Code
/// over Coder's own loop. Codex takes no chat turns yet.
#[must_use]
pub fn stronger(current: Harness, admitted: &[Harness]) -> Option<Harness> {
    (current == Harness::CoderLoop && admitted.contains(&Harness::ClaudeCode))
        .then_some(Harness::ClaudeCode)
}

/// Any other admitted harness that takes chat turns, for a usage limit.
#[must_use]
pub fn other(current: Harness, admitted: &[Harness]) -> Option<Harness> {
    admitted.iter().copied().find(|harness| {
        *harness != current && harness.declaration().turn != crate::harness_plugin::Turn::NotYet
    })
}

/// What the policy does with the work.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Call {
    /// Leave it where it is; the person decides.
    Stay,
    /// Try again on the same harness.
    Retry,
    /// Move it to this harness.
    To(Harness),
}

impl Call {
    /// `stay`, `retry` or `move`, as the record writes it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Self::Stay => "stay",
            Self::Retry => "retry",
            Self::To(_) => "move",
        }
    }
}

/// The policy's steps, in order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Step {
    /// A usage limit: the work moves to another admitted harness.
    LimitMove,
    /// This piece of work already moved once.
    MovedOnce,
    /// System One did not answer: the work stays.
    Silent,
    /// The failure needs the person.
    NeedsPerson,
    /// Claude's five-hour window is nearly used up.
    NearLimit,
    /// Judged beyond this harness, and a stronger one is admitted.
    JudgedMove,
    /// Judged a slip: tried again once.
    JudgedRetry,
    /// Judged to stop, or nothing else applies.
    JudgedStay,
    /// There is nowhere else to go.
    NowhereElse,
}

impl Step {
    #[must_use]
    pub fn name(self) -> String {
        serde_json::to_value(self)
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
            .unwrap_or_default()
    }
}

/// The policy's call and the step that made it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Decision {
    pub call: Call,
    pub step: Step,
}

/// `escalate-policy-v1`: the first step that applies decides.
#[must_use]
pub fn policy(judgment: Option<&Judgment>, checkpoint: Checkpoint, s: &Situation) -> Decision {
    let decide = |call, step| Decision { call, step };
    if s.moved {
        return decide(Call::Stay, Step::MovedOnce);
    }
    if checkpoint == Checkpoint::Limit {
        return match other(s.current, &s.admitted) {
            Some(to) => decide(Call::To(to), Step::LimitMove),
            None => decide(Call::Stay, Step::NowhereElse),
        };
    }
    let Some(judgment) = judgment else {
        return decide(Call::Stay, Step::Silent);
    };
    if judgment.reason == Reason::NeedsPerson {
        return decide(Call::Stay, Step::NeedsPerson);
    }
    match judgment.action {
        Action::Move => match stronger(s.current, &s.admitted) {
            Some(Harness::ClaudeCode)
                if s.claude_five_hour
                    .is_some_and(|used| used >= crate::router::NEAR_LIMIT_PERCENT) =>
            {
                decide(Call::Stay, Step::NearLimit)
            }
            Some(to) => decide(Call::To(to), Step::JudgedMove),
            None if !s.retried => decide(Call::Retry, Step::NowhereElse),
            None => decide(Call::Stay, Step::NowhereElse),
        },
        Action::Retry if !s.retried && checkpoint != Checkpoint::Redo => {
            decide(Call::Retry, Step::JudgedRetry)
        }
        _ => decide(Call::Stay, Step::JudgedStay),
    }
}

/// The message that hands the work to `to`: who takes over and why, the
/// task, the failing check, and the files changed so far.
#[must_use]
pub fn handoff(
    to: &str,
    from: &str,
    checkpoint: Checkpoint,
    task: &str,
    evidence: &Evidence,
) -> String {
    let mut text = format!(
        "{to} is taking over from {from}: {}.\n\nThe task: {}",
        checkpoint.words(),
        task.trim()
    );
    push_evidence(&mut text, evidence);
    text.push_str("\n\nFinish the task, and check that it works.");
    text
}

/// The message that tries the work again on the same harness.
#[must_use]
pub fn retry(checkpoint: Checkpoint, evidence: &Evidence) -> String {
    let mut text = format!("That did not work yet: {}.", checkpoint.words());
    push_evidence(&mut text, evidence);
    text.push_str("\n\nTry again: fix what went wrong, and check that it works.");
    text
}

fn push_evidence(text: &mut String, evidence: &Evidence) {
    if let Some((command, output)) = &evidence.failed_check {
        text.push_str(&format!(
            "\n\nThe failing check: `{command}`\n```\n{output}\n```"
        ));
    }
    if !evidence.files.is_empty() {
        text.push_str(&format!(
            "\n\nFiles changed so far: {}.",
            evidence.files.join(", ")
        ));
    }
}

/// A System One answer at one checkpoint.
#[derive(Clone, Debug, PartialEq)]
pub struct Answered {
    pub recorded: Recorded,
    pub model: String,
    pub door: String,
    pub ms: u64,
}

/// A checkpoint waiting for its answer.
pub struct Pending {
    pub receiver: Option<mpsc::Receiver<Result<Answered, String>>>,
    pub answer: Option<Result<Answered, String>>,
    pub started: Instant,
    pub checkpoint: Checkpoint,
    pub situation: Situation,
    /// The piece of work's task: its first message.
    pub task: String,
    pub evidence: Evidence,
    /// The routing record of the turn that reached the checkpoint.
    pub turn: Option<String>,
    /// The chat it belongs to.
    pub session: Option<String>,
}

impl Pending {
    /// Moves the answer in once there is one; returns whether it is in.
    pub fn settled(&mut self) -> bool {
        if self.answer.is_some() {
            return true;
        }
        let Some(receiver) = &self.receiver else {
            self.answer = Some(Err("No question was needed.".into()));
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

    /// Waits for the answer, for callers with no screen to keep drawing.
    pub fn settle(&mut self) {
        if self.answer.is_some() {
            return;
        }
        let Some(receiver) = &self.receiver else {
            self.answer = Some(Err("No question was needed.".into()));
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

    /// The policy's decision, with the probabilities and the escalation
    /// line's fields.
    #[must_use]
    pub fn decide(&self) -> (Decision, Vec<Probability>, u64, Option<String>) {
        let waited = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        match &self.answer {
            Some(Ok(answered)) => {
                let probabilities = answered
                    .recorded
                    .probabilities(&answered.model, &answered.door);
                let judgment = answered.recorded.judgment();
                (
                    policy(judgment.as_ref(), self.checkpoint, &self.situation),
                    probabilities,
                    answered.ms,
                    judgment.is_none().then(|| {
                        "An answer named an option this question set does not have.".into()
                    }),
                )
            }
            Some(Err(why)) => (
                policy(None, self.checkpoint, &self.situation),
                Vec::new(),
                waited,
                (self.receiver.is_some()).then(|| crate::routing_record::scrub(why)),
            ),
            None => (
                policy(None, self.checkpoint, &self.situation),
                Vec::new(),
                waited,
                Some("System One did not answer in time.".into()),
            ),
        }
    }
}

/// Asks System One about a checkpoint on a thread of its own.
#[must_use]
pub fn ask(state: String) -> mpsc::Receiver<Result<Answered, String>> {
    let (sender, receiver) = mpsc::channel();
    let request = jev::SystemOneRequest::new(state, questions());
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
        drop(request);
    }
    #[cfg(not(test))]
    {
        std::thread::spawn(move || {
            let _ = sender.send(ask_now(request));
        });
    }
    receiver
}

/// One checkpoint request, answered or given up on within [`BUDGET`].
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

/// Whether checkpoints escalate. `CODER_ESCALATE=off` turns it off. Unit
/// tests escalate only with a judge they set ([`set_test_judge`]).
#[must_use]
pub fn enabled() -> bool {
    #[cfg(test)]
    {
        TEST_JUDGE.with(|judge| judge.borrow().is_some())
    }
    #[cfg(not(test))]
    {
        std::env::var("CODER_ESCALATE").map_or(true, |value| value.trim() != "off")
    }
}

#[cfg(test)]
thread_local! {
    static TEST_JUDGE: std::cell::RefCell<Option<Option<Recorded>>> =
        const { std::cell::RefCell::new(None) };
}

/// Sets this test thread's System One for checkpoints.
#[cfg(test)]
pub(crate) fn set_test_judge(judge: Option<Option<Recorded>>) {
    TEST_JUDGE.with(|cell| *cell.borrow_mut() = judge);
}

/// A harness's name as the rail shows it: "Claude (Haiku 5.5)", or "Coder"
/// with the model the person chose.
#[must_use]
pub fn harness_words(app: &crate::App, harness: Harness) -> String {
    match harness {
        Harness::ClaudeCode => crate::ui::claude_rail_label(),
        Harness::CoderLoop => {
            let model = crate::models::label(&app.plugins.model);
            if model == crate::models::AUTO_LABEL {
                "Coder".to_owned()
            } else {
                format!("Coder ({model})")
            }
        }
        Harness::Codex => "Codex".to_owned(),
    }
}

impl crate::App {
    /// The piece of work's task: the message that started it.
    fn work_task(&self) -> String {
        let users: Vec<&str> = self
            .live
            .entries
            .iter()
            .filter_map(|entry| match entry {
                crate::live::Entry::User(text) => Some(text.as_str()),
                _ => None,
            })
            .collect();
        users
            .get(self.live.work_started.min(users.len().saturating_sub(1)))
            .map_or_else(String::new, |text| (*text).to_owned())
    }

    /// Whether this App escalates: a person's chat (the terminal or `coder
    /// chat`) with escalation on.
    fn escalates(&self) -> bool {
        enabled() && (cfg!(test) || self.routing_records)
    }

    fn escalation_situation(&self, current: Harness) -> Situation {
        let facts = crate::harness_plugin::Facts::read(self);
        let candidates = crate::harness_plugin::candidates(&facts);
        Situation {
            current,
            admitted: candidates.admitted,
            claude_five_hour: crate::harness_plugin::claude_five_hour_now(),
            retried: self.live.retried,
            moved: self.live.moved,
        }
    }

    /// Called when a chat's turn ends: when it reached a checkpoint, asks
    /// System One what to do on a thread of its own; [`App::poll_escalation`]
    /// acts on the answer.
    pub(crate) fn checkpoint_turn(&mut self, error: Option<&str>) {
        if self.mode != crate::Mode::Live
            || self.selected_agent.is_some()
            || self.pending_escalation.is_some()
            || !self.escalates()
        {
            return;
        }
        let Some(current) = self.live.runs_on else {
            return;
        };
        let turn = last_turn(&self.live.entries);
        let stalled = turn.iter().any(|entry| {
            matches!(entry, crate::live::Entry::Assistant { text, .. } if text.ends_with(crate::stuck::STOP_NOTICE))
        });
        let at_limit =
            current == Harness::ClaudeCode && crate::harness_plugin::claude_at_limit_now();
        let Some(checkpoint) = reached(
            error,
            at_limit,
            stalled,
            crate::routing_record::turn_checks(turn),
        ) else {
            return;
        };
        let evidence = evidence(turn);
        let situation = self.escalation_situation(current);
        if situation.moved && checkpoint != Checkpoint::Limit {
            return;
        }
        let task = self.work_task();
        // A usage limit needs no question.
        let receiver = (checkpoint != Checkpoint::Limit).then(|| {
            let earlier = crate::router::earlier(&self.live.entries);
            ask(state(
                &earlier,
                &task,
                &harness_words(self, current),
                checkpoint,
                &evidence,
            ))
        });
        self.pending_escalation = Some(Pending {
            receiver,
            answer: None,
            started: Instant::now(),
            checkpoint,
            situation,
            task,
            evidence,
            turn: self.last_routed.as_ref().map(|last| last.id.clone()),
            session: self.session_id().map(str::to_owned),
        });
    }

    /// Acts on a checkpoint once System One answered or the budget ran out.
    /// Called on every pass of the terminal's loop.
    pub fn poll_escalation(&mut self) {
        if self
            .pending_escalation
            .as_mut()
            .is_some_and(Pending::settled)
        {
            self.finish_escalation();
        }
    }

    /// Waits for a checkpoint's answer and acts on it (`coder chat`).
    pub fn settle_escalation(&mut self) {
        if let Some(pending) = self.pending_escalation.as_mut() {
            pending.settle();
            self.finish_escalation();
        }
    }

    /// Whether a checkpoint waits for its answer.
    #[must_use]
    pub fn escalation_pending(&self) -> bool {
        self.pending_escalation.is_some()
    }

    fn finish_escalation(&mut self) {
        let Some(pending) = self.pending_escalation.take() else {
            return;
        };
        // Another chat on screen, or the person already went on: nothing
        // moves.
        if pending.session.as_deref() != self.session_id()
            || self.live.busy
            || self.mode != crate::Mode::Live
        {
            return;
        }
        let (decision, probabilities, ms, missing) = pending.decide();
        let from = pending.situation.current;
        self.record_escalation(&pending, decision, probabilities.clone(), ms, missing);
        let message = match decision.call {
            Call::Stay => {
                // The work stays where it is; a plan goes on to its next
                // part.
                let error = matches!(pending.checkpoint, Checkpoint::Error | Checkpoint::Limit)
                    .then(|| pending.checkpoint.words());
                self.advance_plan(error);
                return;
            }
            Call::Retry => {
                self.live.retried = true;
                self.routing_rule = Some(crate::routing_record::Rule::Retried);
                retry(pending.checkpoint, &pending.evidence)
            }
            Call::To(to) => {
                self.live.moved = true;
                let text = handoff(
                    &harness_words(self, to),
                    &harness_words(self, from),
                    pending.checkpoint,
                    &pending.task,
                    &pending.evidence,
                );
                self.live.runs_on = Some(to);
                self.routing_rule = Some(crate::routing_record::Rule::Escalated);
                text
            }
        };
        self.routing_router = Some((
            probabilities,
            crate::router::Trace {
                question_set: QUESTION_SET.into(),
                policy: POLICY.into(),
                step: Some(decision.step.name()),
                follow_up: true,
                ms,
                missing: None,
            },
        ));
        self.send_as_turn(message);
    }

    /// Sends `text` as the chat's next message on the harness already
    /// chosen, keeping what the person was typing.
    pub(crate) fn send_as_turn(&mut self, text: String) {
        let typed = std::mem::take(&mut self.draft);
        let typed_composer = std::mem::take(&mut self.composer);
        self.draft.cursor = text.len();
        self.draft.text = text;
        self.route_settled = true;
        self.submit_live();
        self.route_settled = false;
        self.draft = typed;
        self.composer = typed_composer;
    }

    pub(crate) fn record_escalation(
        &mut self,
        pending: &Pending,
        decision: Decision,
        probabilities: Vec<Probability>,
        ms: u64,
        missing: Option<String>,
    ) {
        let Some(recorder) = self.routing_recorder() else {
            return;
        };
        recorder.escalated(crate::routing_record::EscalatedLine {
            id: pending.turn.clone().unwrap_or_default(),
            at_ms: atif::now_ms(),
            chat: Some(self.prompt_session()),
            from: pending.situation.current,
            to: match decision.call {
                Call::To(to) => Some(to),
                Call::Retry => Some(pending.situation.current),
                Call::Stay => None,
            },
            checkpoint: pending.checkpoint,
            action: decision.call.word().to_owned(),
            question_set: QUESTION_SET.into(),
            policy: POLICY.into(),
            step: decision.step.name(),
            probabilities,
            ms,
            missing,
        });
    }
}

#[cfg(test)]
#[path = "escalate_tests.rs"]
mod tests;
