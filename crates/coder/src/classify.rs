//! The Classify side of the agent: one state, a map of typed questions,
//! and the table that routes the answers.
//!
//! The question set and the thresholds that consume it live in this one
//! module so they review together — a threshold means nothing without the
//! question it reads. Every `Choice` carries a `none` outcome: choice
//! probabilities always sum to one, so without an escape the model must
//! name an action however poorly any fits.

use indexmap::IndexMap;
use jev::{Answer, Choice, Entry, Noul, Questions, Score, SystemOneResponse};
use serde_json::{Map, Value, json};

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
    /// The `needs_code` Noul probability.
    pub needs_code: Option<f64>,
    /// The `risk` Score, 0–2.
    pub risk: Option<f64>,
    /// The `progress` Score, 0–2.
    pub progress: Option<f64>,
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

/// The question set, in the order the state object names them.
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
        .with(
            "needs_code",
            Noul::new(
                "Does the user's request need code written, or files in a repository inspected or changed?",
            ),
        )
        .with(
            "risk",
            Score::new(
                "How much can the chosen next step damage?",
                vec![
                    Some(Entry::from("0: answer in prose; nothing changes")),
                    Some(Entry::from("1: reads files or runs a reversible command")),
                    Some(Entry::from("2: writes files or could break a build")),
                ],
            ),
        )
        .with(
            "progress",
            Score::new(
                "How close is the conversation to a resolved end?",
                vec![
                    Some(Entry::from("0: just started")),
                    Some(Entry::from("1: underway, intent understood")),
                    Some(Entry::from("2: resolved or resolving now")),
                ],
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
/// `docs/decision-models/2026-09-20-state-budget.md` holds the ladder, the
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
    let noul = |id| {
        response.answers.get(id).and_then(|answer| match answer {
            Answer::Noul(noul) => Some(noul.noul),
            _ => None,
        })
    };
    let score = |id| {
        response.answers.get(id).and_then(|answer| match answer {
            Answer::Score(score) => Some(score.score),
            _ => None,
        })
    };
    Judgment {
        action,
        needs_code: noul("needs_code"),
        risk: score("risk"),
        progress: score("progress"),
    }
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

/// The question family the `damage` Noul belongs to, as a calibration
/// record names it. `docs/decision-models/2026-09-19-coder-turns.md`
/// labels the family under this name.
pub const DAMAGE_FAMILY: &str = "damage";

/// The calibrated probability of harm at which a round stops.
///
/// This is the one place in the module where a probability gates an
/// action rather than an argmax being taken, so the number has to be a
/// probability. It reads as one only from a door that applied an admitted
/// calibration map for [`DAMAGE_FAMILY`]; [`ShellVerdict::damage_gate`]
/// refuses to read it from any other door.
///
/// Against a sampled estimator the same number means something else. A
/// door that counts `N` seeded draws answers in steps of `1/N`, so with
/// eight draws the threshold is "six of eight draws agreed" (`0.75`, the
/// first step at or above `0.7`), and a unanimous sample reads `1.0`
/// however often the model is wrong. On `coder-turns-v1` the uncalibrated
/// `lev-base` door crossed this value on 3 of 21 rounds that damaged
/// nothing, while hosted Jev never rose above `0.1`.
pub const DAMAGE_STOP: f64 = 0.7;

/// What the serving door said about the numbers it returned.
#[derive(Clone, Debug, PartialEq)]
pub enum Calibration {
    /// The door applied an admitted calibration map for the family the
    /// request named, so its probabilities are probabilities.
    Calibrated,
    /// The door said its numbers are raw: frequencies over seeded samples,
    /// at a resolution of one over `samples` when it reported them.
    Uncalibrated {
        /// How many draws the estimator counted, when the door said.
        samples: Option<u64>,
    },
    /// The door said nothing about calibration.
    Unstated,
}

impl Calibration {
    /// Reads the `extensions` a door attaches to its response body: the
    /// calibration state and, for the `damage` question, the estimator's
    /// sample count. A body without them is [`Calibration::Unstated`].
    pub fn of(response: &SystemOneResponse) -> Self {
        let Some(jev::ResponseBody::Json(body)) = response.raw().body() else {
            return Calibration::Unstated;
        };
        let extensions = &body["extensions"];
        let calibration = &extensions["calibration"];
        match calibration["state"].as_str() {
            Some("calibrated") if calibration["family"] == DAMAGE_FAMILY => Calibration::Calibrated,
            Some(_) => Calibration::Uncalibrated {
                samples: extensions["estimator"]["damage"]["samples"].as_u64(),
            },
            None => Calibration::Unstated,
        }
    }

    /// The word the display line and the trace carry.
    pub fn word(&self) -> String {
        match self {
            Calibration::Calibrated => "calibrated".to_string(),
            Calibration::Uncalibrated {
                samples: Some(samples),
            } => format!("uncalibrated, steps of 1/{samples}"),
            Calibration::Uncalibrated { samples: None } => "uncalibrated".to_string(),
            Calibration::Unstated => "calibration unstated".to_string(),
        }
    }
}

/// What the `damage` probability did to the route.
#[derive(Clone, Debug, PartialEq)]
pub enum DamageGate {
    /// A calibrated probability reached [`DAMAGE_STOP`]; the round stops.
    Stop,
    /// A calibrated probability stayed below [`DAMAGE_STOP`], or the
    /// question went unanswered; the choice decides.
    Clear,
    /// The number is not a probability this gate can read, so it did not
    /// gate. The choice decides, and the reason goes to the trace.
    Unread(String),
}

/// What the judge read of a shell round.
#[derive(Clone, Debug)]
pub struct ShellVerdict {
    /// The `outcome` choice and its confidence.
    pub outcome: Option<jev::ChoiceAnswer>,
    /// The `useful` Noul probability.
    pub useful: Option<f64>,
    /// The `damage` Noul probability.
    pub damage: Option<f64>,
    /// What the door said the `damage` number is.
    pub calibration: Calibration,
}

impl ShellVerdict {
    /// Whether `damage` stops the round.
    ///
    /// The gate reads only a calibrated probability. A door that serves a
    /// raw frequency, or one that does not say, has its `damage` number
    /// recorded and displayed but not acted on: the number could be
    /// "every draw agreed" as easily as "seventy percent likely," and
    /// stopping an agent on it would be stopping on a guess. Refusing to
    /// read it is typed and explicit so a reader of the trace sees that
    /// the gate was there and did not fire, rather than that it fired
    /// low.
    pub fn damage_gate(&self) -> DamageGate {
        let Some(damage) = self.damage else {
            return DamageGate::Clear;
        };
        match &self.calibration {
            Calibration::Calibrated if damage >= DAMAGE_STOP => DamageGate::Stop,
            Calibration::Calibrated => DamageGate::Clear,
            other => DamageGate::Unread(format!(
                "damage {damage:.3} not routed: the door serves `{DAMAGE_FAMILY}` {} and \
                 the stop threshold {DAMAGE_STOP} reads a calibrated probability",
                other.word()
            )),
        }
    }

    /// The route the verdict means: a calibrated `damage` at or above
    /// [`DAMAGE_STOP`] forces `Stop` whatever the choice says; a missing
    /// or unlisted choice means `Pass`, the least forceful reading.
    pub fn route(&self) -> ShellRoute {
        if self.damage_gate() == DamageGate::Stop {
            return ShellRoute::Stop;
        }
        match self.outcome.as_ref().map(|outcome| outcome.choice.as_str()) {
            Some("retry") => ShellRoute::Retry,
            Some("stop") => ShellRoute::Stop,
            _ => ShellRoute::Pass,
        }
    }

    /// The display line: `pass 0.91 · useful 0.8 · damage 0.0`, with the
    /// door's word on the number when it is not a calibrated probability:
    /// `damage 1.0 (uncalibrated, steps of 1/8)`.
    pub fn line(&self) -> String {
        let route = self.route().word();
        let confidence = self
            .outcome
            .as_ref()
            .map_or(0.0, |outcome| outcome.confidence);
        let calibration = match (&self.damage, &self.calibration) {
            (None, _) | (_, Calibration::Calibrated) => String::new(),
            (Some(_), other) => format!(" ({})", other.word()),
        };
        format!(
            "{route} {confidence:.2} · useful {:.1} · damage {:.1}{calibration}",
            self.useful.unwrap_or(0.0),
            self.damage.unwrap_or(0.0),
        )
    }
}

/// The `extensions` a shell-round request carries: the family the
/// `damage` question belongs to, so a door holding an admitted map for it
/// applies the map, and a request for the estimator's record, so the
/// door says at what resolution it answered. A door that serves no
/// extensions ignores the field, and the response then says nothing,
/// which [`Calibration::of`] reads as [`Calibration::Unstated`].
pub fn shell_extensions() -> Map<String, Value> {
    let mut extra = Map::new();
    extra.insert(
        "extensions".to_string(),
        json!({ "family": DAMAGE_FAMILY, "estimator": true }),
    );
    extra
}

/// The questions for a round of shell outcomes: where the loop goes, and
/// whether the outputs helped or harmed.
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
        .with(
            "useful",
            Noul::new("Do the outputs help answer the user's request?"),
        )
        .with(
            "damage",
            Noul::new(
                "Does any output suggest the commands harmed the user's files, state, or secrets?",
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
    let noul = |id| {
        response.answers.get(id).and_then(|answer| match answer {
            Answer::Noul(noul) => Some(noul.noul),
            _ => None,
        })
    };
    ShellVerdict {
        outcome,
        useful: noul("useful"),
        damage: noul("damage"),
        calibration: Calibration::of(response),
    }
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
            needs_code: None,
            risk: None,
            progress: None,
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
        assert!(matches!(
            route(&Judgment {
                action: None,
                needs_code: None,
                risk: None,
                progress: None
            }),
            Route::Halt(_)
        ));
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

    /// A shell verdict with `damage` at `damage`, from a door that said
    /// `calibration` about it.
    fn judged(choice: &str, damage: f64, calibration: Calibration) -> ShellVerdict {
        ShellVerdict {
            outcome: Some(ChoiceAnswer {
                choice: choice.to_string(),
                confidence: 0.9,
                probabilities: IndexMap::new(),
            }),
            useful: Some(0.5),
            damage: Some(damage),
            calibration,
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
    fn a_calibrated_door_routes_damage_to_stop() {
        let verdict = judged("pass", 0.75, Calibration::Calibrated);
        assert_eq!(verdict.damage_gate(), DamageGate::Stop);
        assert_eq!(verdict.route(), ShellRoute::Stop);
        let clear = judged("retry", 0.6, Calibration::Calibrated);
        assert_eq!(clear.damage_gate(), DamageGate::Clear);
        assert_eq!(clear.route(), ShellRoute::Retry);
        assert!(!clear.line().contains('('), "{}", clear.line());
    }

    #[test]
    fn an_uncalibrated_door_does_not_fire_the_stop() {
        // Eight draws that all said yes read 1.0 on a sampled estimator,
        // which is the estimator's ceiling and not a probability of one.
        let unanimous = judged("pass", 1.0, Calibration::Uncalibrated { samples: Some(8) });
        let DamageGate::Unread(why) = unanimous.damage_gate() else {
            panic!(
                "an uncalibrated number was routed: {:?}",
                unanimous.damage_gate()
            );
        };
        assert!(why.contains("1/8") && why.contains("0.7"), "{why}");
        assert_eq!(unanimous.route(), ShellRoute::Pass);
        assert!(
            unanimous.line().contains("steps of 1/8"),
            "{}",
            unanimous.line()
        );

        let unstated = judged("retry", 0.9, Calibration::Unstated);
        assert!(matches!(unstated.damage_gate(), DamageGate::Unread(_)));
        assert_eq!(unstated.route(), ShellRoute::Retry);
        assert!(unstated.line().contains("unstated"), "{}", unstated.line());

        let stop = judged("stop", 0.0, Calibration::Unstated);
        assert_eq!(stop.route(), ShellRoute::Stop, "the choice still stops");
    }

    #[test]
    fn a_missing_damage_answer_clears_the_gate() {
        let mut verdict = judged("pass", 0.0, Calibration::Unstated);
        verdict.damage = None;
        assert_eq!(verdict.damage_gate(), DamageGate::Clear);
        assert_eq!(verdict.route(), ShellRoute::Pass);
    }

    #[test]
    fn the_calibration_is_read_from_the_response_extensions() {
        let answers = json!({
            "outcome": { "type": "choice", "choice": "pass", "confidence": 0.9,
                         "probabilities": { "pass": 0.9, "retry": 0.1, "stop": 0.0 } },
            "damage": { "type": "noul", "noul": 1.0 },
        });
        let hosted = response(json!({ "model": "jev", "answers": answers }));
        assert_eq!(Calibration::of(&hosted), Calibration::Unstated);

        let raw = response(json!({
            "model": "lev-base", "answers": answers,
            "extensions": {
                "calibration": { "state": "uncalibrated" },
                "estimator": { "damage": { "samples": 8, "resolution": 0.125 } },
            },
        }));
        assert_eq!(
            Calibration::of(&raw),
            Calibration::Uncalibrated { samples: Some(8) }
        );
        let verdict = shell_verdict_of(&raw);
        assert_eq!(verdict.damage, Some(1.0));
        assert_eq!(verdict.route(), ShellRoute::Pass);

        let fitted = response(json!({
            "model": "lev-adapted", "answers": answers,
            "extensions": { "calibration": { "state": "calibrated", "family": DAMAGE_FAMILY } },
        }));
        assert_eq!(Calibration::of(&fitted), Calibration::Calibrated);
        assert_eq!(shell_verdict_of(&fitted).route(), ShellRoute::Stop);

        // A map fitted for another family is not a map for this one.
        let borrowed = response(json!({
            "model": "lev-adapted", "answers": answers,
            "extensions": { "calibration": { "state": "calibrated", "family": "urgency" } },
        }));
        assert_eq!(
            Calibration::of(&borrowed),
            Calibration::Uncalibrated { samples: None }
        );
    }

    #[test]
    fn the_request_names_the_family() {
        let extra = shell_extensions();
        assert_eq!(extra["extensions"]["family"], DAMAGE_FAMILY);
        assert_eq!(extra["extensions"]["estimator"], true);
    }

    #[test]
    fn every_action_names_and_parses() {
        for action in [Action::Respond, Action::Clarify, Action::End, Action::None] {
            assert_eq!(Action::parse(action.name()), Some(action));
        }
    }
}
