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

/// The state Classify reads: the latest message and a bounded transcript.
/// Named fields, not a concatenated string — the questions can point at
/// `task` and `transcript` directly.
pub fn state_of(task: &str, transcript: &[Message], repo: &[String]) -> Value {
    let turns: Vec<Value> = transcript
        .iter()
        .rev()
        .take(12)
        .rev()
        .map(|message| {
            json!({
                "role": match message.role {
                    crate::generate::Role::User => "user",
                    crate::generate::Role::Assistant => "assistant",
                },
                "text": message.text,
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
/// no separate confidence gate: the argmax choice rules, and a turn the
/// judge genuinely has no read on routes through `none`.
///
/// - `action` missing or naming nothing listed → `Halt`
/// - `action` is `none` → `Halt`
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
        Action::None => Route::Halt("no listed step fit".to_string()),
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

/// What the judge read of a shell round.
#[derive(Clone, Debug)]
pub struct ShellVerdict {
    /// The `outcome` choice and its confidence.
    pub outcome: Option<jev::ChoiceAnswer>,
    /// The `useful` Noul probability.
    pub useful: Option<f64>,
    /// The `damage` Noul probability.
    pub damage: Option<f64>,
}

impl ShellVerdict {
    /// The route the verdict means: `damage` forces `Stop` whatever the
    /// choice says; a missing or unlisted choice means `Pass`, the least
    /// forceful reading.
    pub fn route(&self) -> ShellRoute {
        if self.damage.is_some_and(|damage| damage >= 0.7) {
            return ShellRoute::Stop;
        }
        match self.outcome.as_ref().map(|outcome| outcome.choice.as_str()) {
            Some("retry") => ShellRoute::Retry,
            Some("stop") => ShellRoute::Stop,
            _ => ShellRoute::Pass,
        }
    }

    /// The display line: `pass 0.91 · useful 0.8 · damage 0.0`.
    pub fn line(&self) -> String {
        let route = match self.route() {
            ShellRoute::Pass => "pass",
            ShellRoute::Retry => "retry",
            ShellRoute::Stop => "stop",
        };
        let confidence = self
            .outcome
            .as_ref()
            .map_or(0.0, |outcome| outcome.confidence);
        format!(
            "{route} {confidence:.2} · useful {:.1} · damage {:.1}",
            self.useful.unwrap_or(0.0),
            self.damage.unwrap_or(0.0),
        )
    }
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
    fn none_and_unknown_actions_halt() {
        assert!(matches!(route(&judgment("none", 0.99)), Route::Halt(_)));
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
        assert!(matches!(route(&judgment("none", 0.3)), Route::Halt(_)));
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

    #[test]
    fn every_action_names_and_parses() {
        for action in [Action::Respond, Action::Clarify, Action::End, Action::None] {
            assert_eq!(Action::parse(action.name()), Some(action));
        }
    }
}
