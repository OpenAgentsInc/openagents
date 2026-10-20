//! The Classify side of the agent: one state, a map of typed questions,
//! and the table that routes the answers.
//!
//! The question set and the table that consumes it live in this one
//! module so they review together — a route means nothing without the
//! question it reads. Every `Choice` carries a `none` outcome: choice
//! probabilities always sum to one, so without an escape the model must
//! name an action however poorly any fits.
//!
//! The set is `coder-turns-v2`: one question per turn, `action`, and one
//! per round of shell commands, `outcome`. `coder-turns-v1` also asked
//! `needs_code`, `risk`, and `progress` on the turn and `useful` and
//! `damage` on the round, and
//! `docs/decision-models/measurements/2026-09-20-coder-question-baselines.md` retired
//! the five: on real turns each scores no better than the constant that
//! would replace it, and no decision read any of them. The v1 text stays
//! in `crates/gym/questions/coder-turns-v1.json`, where the rows that
//! measured it still point.

use indexmap::IndexMap;
use jev::{Answer, Choice, Entry, Questions, SystemOneResponse};
use serde_json::{Value, json};

use crate::generate::Message;

/// The actions Classify can name, in the order they are offered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// Answer the user directly.
    Respond,
    /// Ask one clarifying question before answering.
    Clarify,
    /// The conversation is over: the user said so.
    End,
    /// No listed action fits the state.
    None,
}

impl Action {
    /// The option name the Choice answer carries.
    pub fn name(self) -> &'static str {
        match self {
            Action::Respond => "respond",
            Action::Clarify => "clarify",
            Action::End => "end_conversation",
            Action::None => "none",
        }
    }

    /// The action an answer's `choice` string names, if it names one.
    pub fn parse(choice: &str) -> Option<Self> {
        Some(match choice {
            "respond" => Action::Respond,
            "clarify" => Action::Clarify,
            "end_conversation" => Action::End,
            "none" => Action::None,
            _ => return None,
        })
    }
}

/// What Classify read, kept whole so the terminal can show it.
#[derive(Clone, Debug)]
pub struct Judgment {
    /// The `action` answer: choice, confidence, and the full distribution.
    pub action: Option<jev::ChoiceAnswer>,
}

/// Where the router sends a turn.
#[derive(Clone, Debug, PartialEq)]
pub enum Route {
    /// Generate an answer now.
    Respond,
    /// Generate one clarifying question.
    Clarify,
    /// Close the conversation; no generation.
    End,
    /// Do not act; the reason is for the transcript.
    Halt(String),
}

impl Route {
    /// The word a trace records this route as. A halt's reason is on the
    /// judgment beside it, so the word names the route and nothing else.
    pub fn word(&self) -> &'static str {
        match self {
            Route::Respond => "respond",
            Route::Clarify => "clarify",
            Route::End => "end",
            Route::Halt(_) => "halt",
        }
    }
}

/// `coder-turns-v2`: the measured family the turn and round questions
/// belong to — the stable identity both functions' trace records name,
/// the name the retained baselines and the retired `coder-turns-v1`
/// suite answer under.
pub const TURNS: &str = "coder-turns-v2";

/// What a trace records about the turn question: the same
/// set-identity, wording-digest, and gate record a file-defined
/// question set's provenance emits, over the wording this host builds
/// in code.
#[must_use]
pub fn questions_provenance() -> Value {
    crate::questions::function(
        TURNS,
        "action",
        crate::questions::wording_digest(&questions()),
    )
}

/// The same record for the round question [`shell_questions`] builds.
#[must_use]
pub fn shell_provenance() -> Value {
    crate::questions::function(
        TURNS,
        "outcome",
        crate::questions::wording_digest(&shell_questions()),
    )
}

/// The turn question: `action`, the one answer [`route`] reads.
pub fn questions() -> Questions {
    Questions::new()
        .with(
            "action",
            Choice::new(
                "Based on the conversation so far, what is the single next best step?",
                IndexMap::from([
                    (
                        "respond".to_string(),
                        Some(Entry::from(
                            "Answer the user's message directly — the intent is clear and no code is needed yet",
                        )),
                    ),
                    (
                        "clarify".to_string(),
                        Some(Entry::from(
                            "Ask one short clarifying question — the request is ambiguous and a wrong answer wastes the turn",
                        )),
                    ),
                    (
                        "end_conversation".to_string(),
                        Some(Entry::from(
                            "The user is done: a goodbye, a thanks, or an explicit quit",
                        )),
                    ),
                    (
                        "none".to_string(),
                        Some(Entry::from(
                            "No listed step fits: the state does not support a next action",
                        )),
                    ),
                ]),
            ),
        )
}

/// The bounds [`state_of`] holds a transcript to before a door reads it.
///
/// The state has to fit the smallest door `coder` intends to serve, which
/// is Apple's on-device model behind `lev`. On `coder-turns-v1` that door
/// answered states up to 12,101 bytes and refused `branch_too_long` from
/// 10,704 bytes, so the budget is [`STATE_BUDGET`], and these caps are the
/// rung of the measured ladder where every real turn state lands under it.
/// `docs/decision-models/measurements/2026-09-20-state-budget.md` holds the ladder, the
/// sizes, and what hosted Jev's accuracy did at each rung.
///
/// The caps take the largest contributor first. Command output was
/// assumed to be it, and on real turns it is not: over the 40 turn states
/// the assistant's own text is 54% of the bytes and shell records 41%, so
/// a cap on output alone leaves the median turn over the budget. A cap on
/// turns and one on each message's text are what bring the state under.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Caps {
    /// How many of the latest transcript turns the state carries.
    pub turns: usize,
    /// How many bytes of one message's text the state carries. A shell
    /// record is bounded by `commands` and `output_bytes` first, then held
    /// to this too.
    pub message_bytes: usize,
    /// How many commands of one shell record the state carries.
    pub commands: usize,
    /// How many bytes of one command's output the state carries.
    pub output_bytes: usize,
}

/// The bytes a `classify` state may reach, as `serde_json` writes it.
///
/// Apple's runtime refused the smallest real state at 10,704 bytes, so the
/// budget sits at three quarters of that: tokens per byte vary with what
/// the text is, and the margin is what keeps a shell-heavy state from
/// tokenizing past the window the byte count says it fits.
pub const STATE_BUDGET: usize = 10_704 / 4 * 3;

impl Caps {
    /// The bounds production applies: six turns, 768 bytes of text per
    /// message, three commands per shell record, and 256 bytes of output
    /// per command. Every one of the 40 real turn states in
    /// `coder-turns-v1` lands under [`STATE_BUDGET`] at these bounds, and
    /// hosted Jev's accuracy on the development partition is within noise
    /// of the unbudgeted state.
    pub const PRODUCTION: Caps = Caps {
        turns: 6,
        message_bytes: 768,
        commands: 3,
        output_bytes: 256,
    };

    /// The bounds the state had before it was budgeted: twelve turns,
    /// whole messages, ten commands, and 2,048 bytes of output. Kept so a
    /// sweep has a baseline to pair against.
    pub const UNBUDGETED: Caps = Caps {
        turns: 12,
        message_bytes: usize::MAX,
        commands: crate::shell::COMMANDS_MAX,
        output_bytes: crate::shell::HEAD_MAX,
    };
}

/// The line a shell record opens with; `shell::transcript_of` writes it.
const SHELL_RECORD: &str = "ran shell commands:\n";

/// The longest prefix of `text` within `bytes`, on a character boundary.
fn head(text: &str, bytes: usize) -> &str {
    if text.len() <= bytes {
        return text;
    }
    let mut end = bytes;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// One message's text, held to `caps`.
///
/// A shell record is the one message the agent writes for itself, in the
/// shape `shell::transcript_of` gives it: a heading, then one block per
/// command of the command line, its status, and its output. It is bounded
/// block by block so a command is never cut mid-line — the first
/// `caps.commands` blocks stay, each with `caps.output_bytes` of output,
/// and a line counts the blocks dropped, and the record then keeps its
/// first `caps.message_bytes` like any other message. Caps no tighter
/// than the shell's own leave the record as written.
pub fn bounded_text(text: &str, caps: &Caps) -> String {
    let shell_bounded =
        caps.commands >= crate::shell::COMMANDS_MAX && caps.output_bytes >= crate::shell::HEAD_MAX;
    let Some(records) = text.strip_prefix(SHELL_RECORD).filter(|_| !shell_bounded) else {
        return head(text, caps.message_bytes).to_string();
    };
    let blocks: Vec<&str> = records.split("\n$ ").skip(1).collect();
    let mut out = String::from(SHELL_RECORD);
    for block in blocks.iter().take(caps.commands) {
        let mut lines = block.splitn(3, '\n');
        let command = lines.next().unwrap_or_default();
        let status = lines.next().unwrap_or_default();
        let output = head(lines.next().unwrap_or_default(), caps.output_bytes);
        out.push_str(&format!("\n$ {command}\n{status}\n{output}"));
        if !output.ends_with('\n') {
            out.push('\n');
        }
    }
    let dropped = blocks.len().saturating_sub(caps.commands);
    if dropped > 0 {
        out.push_str(&format!("\n{dropped} more commands not shown\n"));
    }
    head(&out, caps.message_bytes).to_string()
}

/// The state Classify reads: the latest message and a transcript bounded
/// by [`Caps::PRODUCTION`]. Named fields, not a concatenated string — the
/// questions can point at `task` and `transcript` directly.
pub fn state_of(task: &str, transcript: &[Message], repo: &[String]) -> Value {
    state_within(task, transcript, repo, &Caps::PRODUCTION)
}

/// [`state_of`] under explicit bounds, so a sweep can hold the same
/// transcript to a ladder of caps and pair the answers.
pub fn state_within(task: &str, transcript: &[Message], repo: &[String], caps: &Caps) -> Value {
    let turns: Vec<Value> = transcript
        .iter()
        .rev()
        .take(caps.turns)
        .rev()
        .map(|message| {
            json!({
                "role": match message.role {
                    crate::generate::Role::User => "user",
                    crate::generate::Role::Assistant => "assistant",
                },
                "text": bounded_text(&message.text, caps),
            })
        })
        .collect();
    json!({ "task": task, "transcript": turns, "repo_members": repo })
}

/// Reads a response into a [`Judgment`].
pub fn judgment_of(response: &SystemOneResponse) -> Judgment {
    let action = response
        .answers
        .get("action")
        .and_then(|answer| match answer {
            Answer::Choice(choice) => Some(choice.clone()),
            _ => None,
        });
    Judgment { action }
}

/// The routing table: judgment in, next step out.
///
/// The `none` option is the escape hatch — the choice distribution sums
/// to one, so when nothing listed fits, `none` is meant to win. There is
/// no separate confidence gate: the argmax choice rules, and that rule is
/// measured. On `coder-turns-v1` the incumbent door's two wrong `action`
/// answers came at 0.72 and 0.48 while its right ones ran from 0.52 up, so
/// no floor separates them and a floor that caught both would have halted
/// more than half the turns that routed correctly.
///
/// **`none` answers, it does not halt**, and that is a correction. It used
/// to route to `Halt`, which replies that there is no confident next step
/// and generates nothing. Two things measured on real turns say that is
/// wrong. `none` is never the truth: 40 turns harvested from recorded
/// sessions, and every one of them had a next step the agent took. And a
/// door that misreads the state answers `none` freely — `kev-8b` chose it
/// on 14 of 16 real turns, which under the old table halted seven turns in
/// eight. A judge with no read is the same situation as a judge that could
/// not be reached, and [`crate::agent::Classified::Skipped`] already
/// answers that by generating unrouted. The judgment is kept whole either
/// way, so the terminal still shows that `none` won.
///
/// - `action` missing or naming nothing listed → `Halt`
/// - `action` is `none` → `Respond`, unrouted
/// - `end_conversation` → `End`
/// - `clarify` → `Clarify`
/// - otherwise → `Respond`
pub fn route(judgment: &Judgment) -> Route {
    let Some(action) = &judgment.action else {
        return Route::Halt("the action question went unanswered".to_string());
    };
    let Some(action) = Action::parse(&action.choice) else {
        return Route::Halt(format!(
            "the action answer names nothing listed: {}",
            action.choice
        ));
    };
    match action {
        Action::None => Route::Respond,
        Action::End => Route::End,
        Action::Clarify => Route::Clarify,
        Action::Respond => Route::Respond,
    }
}

/// Where the judge sends a round of shell outcomes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShellRoute {
    /// Hand the outputs to the model; the loop continues.
    Pass,
    /// A command failed or missed; the model corrects and tries again.
    Retry,
    /// Stop running commands: damage, a stuck loop, or nothing to learn.
    Stop,
}

impl ShellRoute {
    /// The word a trace and the transcript record this route as.
    pub fn word(self) -> &'static str {
        match self {
            ShellRoute::Pass => "pass",
            ShellRoute::Retry => "retry",
            ShellRoute::Stop => "stop",
        }
    }
}

/// What the judge read of a shell round.
#[derive(Clone, Debug)]
pub struct ShellVerdict {
    /// The `outcome` choice and its confidence.
    pub outcome: Option<jev::ChoiceAnswer>,
}

impl ShellVerdict {
    /// The route the verdict means: the choice, read as an argmax like
    /// the turn's `action`. A missing or unlisted choice means `Pass`,
    /// the least forceful reading.
    ///
    /// No probability gates this route. `coder-turns-v1` also read a
    /// `damage` Noul against a stop threshold, and that gate left with the
    /// question: its truth was `no` on 55 of 55 labelled rounds, hosted
    /// Jev never answered above 0.13, and the gate read only a calibrated
    /// number, which no door `coder` ships with served.
    pub fn route(&self) -> ShellRoute {
        match self.outcome.as_ref().map(|outcome| outcome.choice.as_str()) {
            Some("retry") => ShellRoute::Retry,
            Some("stop") => ShellRoute::Stop,
            _ => ShellRoute::Pass,
        }
    }

    /// The display line: `pass 0.91`, the route and the choice's
    /// confidence.
    pub fn line(&self) -> String {
        let route = self.route().word();
        let confidence = self
            .outcome
            .as_ref()
            .map_or(0.0, |outcome| outcome.confidence);
        format!("{route} {confidence:.2}")
    }
}

/// The round question: `outcome`, where the loop goes next.
pub fn shell_questions() -> Questions {
    Questions::new()
        .with(
            "outcome",
            Choice::new(
                "These shell commands ran for the user's task. What should the agent do next?",
                IndexMap::from([
                    (
                        "pass".to_string(),
                        Some(Entry::from(
                            "Hand the outputs to the model — they answer the task or move it forward",
                        )),
                    ),
                    (
                        "retry".to_string(),
                        Some(Entry::from(
                            "A command failed or missed the point — the model should correct the approach and try again",
                        )),
                    ),
                    (
                        "stop".to_string(),
                        Some(Entry::from(
                            "Stop running commands — the outputs show damage, a stuck loop, or nothing left to learn",
                        )),
                    ),
                ]),
            ),
        )
}

/// Reads a response into a [`ShellVerdict`].
pub fn shell_verdict_of(response: &SystemOneResponse) -> ShellVerdict {
    let outcome = response
        .answers
        .get("outcome")
        .and_then(|answer| match answer {
            Answer::Choice(choice) => Some(choice.clone()),
            _ => None,
        });
    ShellVerdict { outcome }
}

#[cfg(test)]
mod tests {
    use super::*;
    use jev::ChoiceAnswer;

    fn judgment(choice: &str, confidence: f64) -> Judgment {
        Judgment {
            action: Some(ChoiceAnswer {
                choice: choice.to_string(),
                confidence,
                probabilities: IndexMap::new(),
            }),
        }
    }

    #[test]
    fn a_confident_respond_routes_to_respond() {
        assert_eq!(route(&judgment("respond", 0.9)), Route::Respond);
        assert_eq!(route(&judgment("clarify", 0.8)), Route::Clarify);
        assert_eq!(route(&judgment("end_conversation", 0.7)), Route::End);
    }

    #[test]
    fn an_unreadable_answer_halts_and_none_answers() {
        // `none` is the judge saying it has no read, which is the same
        // situation as a judge that could not be reached. An answer that
        // names nothing listed is a door that broke its own contract.
        assert_eq!(route(&judgment("none", 0.99)), Route::Respond);
        assert!(matches!(route(&judgment("fly", 0.99)), Route::Halt(_)));
        assert!(matches!(route(&Judgment { action: None }), Route::Halt(_)));
    }

    #[test]
    fn the_choice_rules_at_any_confidence() {
        assert_eq!(route(&judgment("respond", 0.2)), Route::Respond);
        assert_eq!(route(&judgment("clarify", 0.1)), Route::Clarify);
        assert_eq!(route(&judgment("none", 0.3)), Route::Respond);
    }

    #[test]
    fn the_state_is_a_structured_object() {
        let state = state_of(
            "what time is it",
            &[Message {
                role: crate::generate::Role::User,
                text: "hi".to_string(),
            }],
            &["coder".to_string()],
        );
        assert_eq!(state["repo_members"][0], "coder");
        assert_eq!(state["task"], "what time is it");
        assert_eq!(state["transcript"][0]["role"], "user");
    }

    /// A shell verdict whose choice is `choice`.
    fn judged(choice: &str) -> ShellVerdict {
        ShellVerdict {
            outcome: Some(ChoiceAnswer {
                choice: choice.to_string(),
                confidence: 0.9,
                probabilities: IndexMap::new(),
            }),
        }
    }

    /// A door's response body, decoded the way the client decodes it.
    fn response(body: Value) -> SystemOneResponse {
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: body.to_string().into_bytes(),
        })
        .expect("a readable response")
    }

    #[test]
    fn the_choice_is_the_shell_route() {
        assert_eq!(judged("pass").route(), ShellRoute::Pass);
        assert_eq!(judged("retry").route(), ShellRoute::Retry);
        assert_eq!(judged("stop").route(), ShellRoute::Stop);
        assert_eq!(judged("fly").route(), ShellRoute::Pass);
        assert_eq!(ShellVerdict { outcome: None }.route(), ShellRoute::Pass);
        assert_eq!(judged("retry").line(), "retry 0.90");
    }

    #[test]
    fn a_damage_answer_is_not_read() {
        // A door that still answers the retired v1 questions changes
        // nothing: the round's choice is the only answer the verdict reads.
        let answers = json!({
            "outcome": { "type": "choice", "choice": "pass", "confidence": 0.9,
                         "probabilities": { "pass": 0.9, "retry": 0.1, "stop": 0.0 } },
            "damage": { "type": "noul", "noul": 1.0 },
            "useful": { "type": "noul", "noul": 0.0 },
        });
        let body = response(json!({
            "model": "lev-adapted", "answers": answers,
            "extensions": { "calibration": { "state": "calibrated", "family": "damage" } },
        }));
        assert_eq!(shell_verdict_of(&body).route(), ShellRoute::Pass);
    }

    #[test]
    fn the_sets_ask_one_question_each() {
        let turn: Value = serde_json::to_value(questions()).unwrap();
        assert_eq!(
            turn.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["action"]
        );
        let shell: Value = serde_json::to_value(shell_questions()).unwrap();
        assert_eq!(
            shell.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["outcome"]
        );
    }

    #[test]
    fn every_action_names_and_parses() {
        for action in [Action::Respond, Action::Clarify, Action::End, Action::None] {
            assert_eq!(Action::parse(action.name()), Some(action));
        }
    }

    /// The code-built functions record the same provenance shape a
    /// file-defined set emits — identity, the wording's digest, the
    /// gate — so a trace reader needs no second path for them.
    #[test]
    fn the_code_built_functions_carry_the_sets_provenance_shape() {
        let turn = questions_provenance();
        assert_eq!(turn["question_set"], TURNS);
        assert_eq!(turn["gate"], "action");
        assert_eq!(
            turn["set_digest"].as_str().unwrap().len(),
            64,
            "the same digest form a Set records"
        );
        assert!(turn["policy_version"].is_null());

        // The round question is the same family under its own gate, and
        // its wording digests to a different record than the turn's —
        // which is what a digest is for.
        let shell = shell_provenance();
        assert_eq!(shell["question_set"], TURNS);
        assert_eq!(shell["gate"], "outcome");
        assert_ne!(shell["set_digest"], turn["set_digest"]);

        // Asking twice digests twice identically: the wording is fixed
        // in code, so the record is stable across runs.
        assert_eq!(questions_provenance()["set_digest"], turn["set_digest"]);
    }
}
