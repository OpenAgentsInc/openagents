//! The first response: one typed judgment the moment a message arrives.
//!
//! A conversation turn on the chat worker used to show nothing until the
//! model's first token, which on the Gemini Flash lane is three to four
//! seconds after the request reaches the worker. This module is what fills
//! that gap. For a turn that asks (`"opener": true`, or `"judge": true` for
//! the judgment alone), the worker asks one System One (Jev) request over
//! the user's message and the bounded transcript, in parallel with the
//! model call and never in front of it, and reads three independent
//! questions from the same state:
//!
//! - `action`: the turn's route, worded exactly as [`crate::classify`]'s
//!   measured `coder-turns-v2` question, so its answer means the same.
//! - `lane`: whether the request can be answered in the chat or needs a
//!   computer — a repository, files, commands, or changes.
//! - `opener`: which of the 21 [`OPENERS`] the reply should open with, or
//!   `none`.
//!
//! The highest-probability opener is shown at once as the reply's first
//! words, and the typed judgment goes out beside it for a client that
//! renders or routes on it (see [`feedback`]). Nothing here is keyword
//! matching: every reading is a Choice answer's argmax over options this
//! module lists, which is the typed semantic selector `AGENTS.md` asks for.
//!
//! [`rank_questions`] and [`ranking`] are the same judgment turned to the
//! phone's suggestions: the caller names its candidate repositories or
//! actions, and the answer orders them. Read
//! `docs/coder/measurements/2026-09-28-first-reply.md` for what this saves
//! and `nips/openagents/NIP-CJ.md` for the wire shapes.
//!
//! Everything here is pure: a state in, a request out; an answer in, a
//! reading out. The caller owns the HTTP.

use std::time::Duration;

use indexmap::IndexMap;
use jev::{Answer, Choice, ChoiceAnswer, Entry, Questions, RetryPolicy, SystemOneResponse};
use serde_json::{Value, json};

use crate::classify::{Route, route};
use crate::generate::Message;

/// The question set's identity, for evidence and for the wire.
pub const SET: &str = "coder-first-response-v1";

/// How long the worker waits for the judgment before it gives up on it.
/// Past this the model's own first words are close, and an opener that
/// arrives after them is not shown at all.
pub const BUDGET: Duration = Duration::from_millis(2_500);

/// The instruction the worker adds to the caller's, so the model does
/// not open with an acknowledgement of its own after the one shown.
pub const MODEL_NOTE: &str = "The user already sees a one-line acknowledgement above your \
reply, so do not open with one or with a greeting: begin directly with the substance.";

/// The openers the judgment chooses from: `(id, what the user sees, when it
/// fits)`. Each is short, true before any work has happened, and promises
/// nothing the turn may not do. They are worded for the chat worker, which
/// has no computer: `computer` is the one to pick when the work needs one.
pub const OPENERS: &[(&str, &str, &str)] = &[
    (
        "look_into",
        "I'll look into that now.",
        "A request to investigate or find something out",
    ),
    (
        "check",
        "Let me check on that.",
        "A question about the state or status of something",
    ),
    (
        "review_files",
        "I'll review the relevant files.",
        "A request about specific code, files, or a repository",
    ),
    (
        "bearings",
        "Let me get my bearings.",
        "A broad or open-ended task in an unfamiliar area",
    ),
    (
        "think",
        "Let me think about that.",
        "A question that needs reasoning, judgment, or a design decision",
    ),
    (
        "explain",
        "Here's how that works.",
        "A request to explain a concept, tool, or piece of code",
    ),
    (
        "dig_in",
        "Let me dig into this.",
        "A hard or detailed problem that needs careful work",
    ),
    (
        "reproduce",
        "Let me try to reproduce that.",
        "A report of a bug, crash, or unexpected behavior",
    ),
    (
        "trace",
        "Let me trace where that's coming from.",
        "An error message, stack trace, or failing command to diagnose",
    ),
    (
        "plan",
        "Let me sketch a plan.",
        "A multi-step feature, migration, or project to plan",
    ),
    (
        "write",
        "I'll write that up.",
        "A request to write code, a script, a document, or a message",
    ),
    (
        "compare",
        "Let me compare the options.",
        "A choice between tools, libraries, or approaches",
    ),
    (
        "summarize",
        "Let me summarize.",
        "A request to summarize or condense something",
    ),
    (
        "understand",
        "Let me make sure I understand.",
        "An ambiguous request that needs a clarifying question",
    ),
    (
        "on_it",
        "On it.",
        "A short, clear instruction the assistant can carry out in its reply",
    ),
    (
        "sure",
        "Sure.",
        "A simple, direct question with a short answer",
    ),
    ("welcome", "You're welcome!", "Thanks, praise, or a goodbye"),
    ("hello", "Hi!", "A greeting with no request yet"),
    (
        "sorry",
        "Sorry about that.",
        "A complaint that a previous answer was wrong or unhelpful",
    ),
    (
        "computer",
        "That needs your computer.",
        "Work this chat cannot do: running code or commands, or reading or changing the \
         user's repository or files",
    ),
    (
        "good_question",
        "Good question.",
        "A curious, conceptual, or 'why' question",
    ),
];

/// Where the judgment says the request belongs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lane {
    /// Answerable in the chat: no files, repository, or commands.
    Chat,
    /// Needs a computer: a repository, files, commands, tests, or changes.
    Computer,
    /// The judgment has no read.
    Unknown,
}

impl Lane {
    /// The word the wire carries.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Lane::Chat => "chat",
            Lane::Computer => "computer",
            Lane::Unknown => "unknown",
        }
    }

    fn parse(choice: &str) -> Self {
        match choice {
            "chat" => Lane::Chat,
            "computer" => Lane::Computer,
            _ => Lane::Unknown,
        }
    }
}

/// The three questions, from one state.
#[must_use]
pub fn questions() -> Questions {
    let action = crate::classify::questions()
        .get("action")
        .cloned()
        .expect("the turn set asks `action`");
    let mut openers: IndexMap<String, Option<Entry>> = OPENERS
        .iter()
        .map(|(id, text, fits)| {
            (
                (*id).to_string(),
                Some(Entry::from(format!("\"{text}\" — {fits}"))),
            )
        })
        .collect();
    openers.insert(
        "none".to_string(),
        Some(Entry::from("No listed opener fits this message")),
    );
    Questions::new()
        .with("action", action)
        .with(
            "lane",
            Choice::new(
                "Can the user's latest message be answered in a chat reply, or does it need \
                 work on a computer?",
                IndexMap::from([
                    (
                        "chat".to_string(),
                        Some(Entry::from(
                            "Answer in the chat: a question, explanation, advice, or a short \
                             snippet that needs no repository, files, or commands",
                        )),
                    ),
                    (
                        "computer".to_string(),
                        Some(Entry::from(
                            "Needs a computer: reading or changing a repository or files, \
                             running commands or tests, or opening a pull request",
                        )),
                    ),
                    (
                        "none".to_string(),
                        Some(Entry::from("Neither fits the message")),
                    ),
                ]),
            ),
        )
        .with(
            "opener",
            Choice::new(
                "Which short acknowledgement should the assistant show first, before its \
                 full reply to the user's latest message?",
                openers,
            ),
        )
}

/// The state the judgment reads: the same bounded shape Classify reads.
#[must_use]
pub fn state(task: &str, transcript: &[Message]) -> Value {
    crate::classify::state_of(task, transcript, &[])
}

/// A retry policy for a call that is only worth anything fast: one
/// attempt, bounded by [`BUDGET`].
#[must_use]
pub fn retry() -> RetryPolicy {
    RetryPolicy {
        max_retries: 0,
        budget: Some(BUDGET),
        ..RetryPolicy::default()
    }
}

/// The request the worker sends.
#[must_use]
pub fn request(task: &str, transcript: &[Message]) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(task, transcript), questions())
        .retry(retry())
        .timeout(BUDGET)
}

/// What the judgment read.
#[derive(Clone, Debug)]
pub struct Triage {
    pub route: Route,
    pub lane: Lane,
    /// The chosen opener's id and display text, or `None` for `none`.
    pub opener: Option<(&'static str, &'static str)>,
    /// The opener choice's confidence.
    pub confidence: f64,
}

fn choice<'a>(response: &'a SystemOneResponse, id: &str) -> Option<&'a ChoiceAnswer> {
    match response.answers.get(id) {
        Some(Answer::Choice(choice)) => Some(choice),
        _ => None,
    }
}

/// Reads a response into a [`Triage`]: each answer's argmax, nothing
/// more.
#[must_use]
pub fn triage_of(response: &SystemOneResponse) -> Triage {
    let judgment = crate::classify::Judgment {
        action: choice(response, "action").cloned(),
    };
    let lane = choice(response, "lane").map_or(Lane::Unknown, |lane| Lane::parse(&lane.choice));
    let opener = choice(response, "opener");
    Triage {
        route: route(&judgment),
        lane,
        opener: opener.and_then(|opener| {
            OPENERS
                .iter()
                .find(|(id, _, _)| *id == opener.choice)
                .map(|(id, text, _)| (*id, *text))
        }),
        confidence: opener.map_or(0.0, |opener| opener.confidence),
    }
}

impl Triage {
    /// The NIP-CJ verdict word.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match self.route {
            Route::Respond => "respond",
            Route::Clarify => "clarify",
            Route::End => "end_conversation",
            Route::Halt(_) => "unrouted",
        }
    }

    /// The display line: the opener when one was chosen, else the verdict.
    #[must_use]
    pub fn line(&self) -> String {
        match self.opener {
            Some((_, text)) => text.to_string(),
            None => self.verdict().to_string(),
        }
    }
}

/// The `27000` judgment feedback for `triage`, at payload `version`.
///
/// `verdict` and `line` are NIP-CJ's; `set`, `lane`, `opener`, and
/// `confidence` are this set's typed additions, each optional to a reader.
#[must_use]
pub fn feedback(version: u64, triage: &Triage) -> Value {
    json!({
        "v": version,
        "requires": [],
        "type": "judgment",
        "verdict": triage.verdict(),
        "line": triage.line(),
        "set": SET,
        "lane": triage.lane.word(),
        "opener": triage.opener.map(|(id, _)| id),
        "confidence": triage.confidence,
    })
}

/// The most candidates one ranking takes.
pub const MAX_CANDIDATES: usize = 16;
/// The longest candidate id, in bytes.
pub const MAX_ID_BYTES: usize = 64;
/// The longest candidate label, in bytes.
pub const MAX_LABEL_BYTES: usize = 200;

/// One suggestion a caller wants ranked: a repository or an action.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub id: String,
    pub label: String,
}

/// Reads `candidates` from a request payload: an array of
/// `{ "id", "label" }`, bounded.
///
/// # Errors
///
/// Names what is wrong: not an array, too many, an empty, long, repeated,
/// or reserved id, or a long label.
pub fn candidates_of(value: &Value) -> Result<Vec<Candidate>, String> {
    let list = value
        .as_array()
        .ok_or("candidates is not an array of { id, label }")?;
    if list.is_empty() || list.len() > MAX_CANDIDATES {
        return Err(format!("candidates must name 1 to {MAX_CANDIDATES}"));
    }
    let mut out: Vec<Candidate> = Vec::with_capacity(list.len());
    for item in list {
        let id = item["id"].as_str().unwrap_or_default();
        let label = item["label"].as_str().unwrap_or(id);
        if id.is_empty() || id.len() > MAX_ID_BYTES || id == "none" {
            return Err(format!(
                "a candidate id must be 1 to {MAX_ID_BYTES} bytes and not `none`"
            ));
        }
        if label.len() > MAX_LABEL_BYTES {
            return Err(format!(
                "a candidate label must be at most {MAX_LABEL_BYTES} bytes"
            ));
        }
        if out.iter().any(|seen| seen.id == id) {
            return Err(format!("the candidate id {id} is repeated"));
        }
        out.push(Candidate {
            id: id.to_string(),
            label: label.to_string(),
        });
    }
    Ok(out)
}

/// The ranking question: which candidate the user most likely wants next.
#[must_use]
pub fn rank_questions(candidates: &[Candidate]) -> Questions {
    let mut options: IndexMap<String, Option<Entry>> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.id.clone(),
                Some(Entry::from(candidate.label.clone())),
            )
        })
        .collect();
    options.insert(
        "none".to_string(),
        Some(Entry::from("None of these fits what the user is doing")),
    );
    Questions::new().with(
        "next",
        Choice::new(
            "Given the conversation so far, which of these repositories or actions is the user \
             most likely to want next?",
            options,
        ),
    )
}

/// The ranking request over a draft (possibly empty) and the transcript.
#[must_use]
pub fn rank_request(
    draft: &str,
    transcript: &[Message],
    candidates: &[Candidate],
) -> jev::SystemOneRequest {
    jev::SystemOneRequest::new(state(draft, transcript), rank_questions(candidates))
        .retry(retry())
        .timeout(BUDGET)
}

/// The candidates in the answer's order, most likely first, each with its
/// probability. A candidate the answer left out ranks last at zero, in the
/// caller's order; `none` is not a candidate and is dropped.
#[must_use]
pub fn ranking(response: &SystemOneResponse, candidates: &[Candidate]) -> Vec<(String, f64)> {
    let probabilities = choice(response, "next").map(|next| &next.probabilities);
    let mut ranked: Vec<(usize, String, f64)> = candidates
        .iter()
        .enumerate()
        .map(|(index, candidate)| {
            let p = probabilities
                .and_then(|p| p.get(&candidate.id))
                .copied()
                .filter(|p| p.is_finite())
                .unwrap_or(0.0);
            (index, candidate.id.clone(), p)
        })
        .collect();
    ranked.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    ranked.into_iter().map(|(_, id, p)| (id, p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn response(answers: Value) -> SystemOneResponse {
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: json!({ "model": "jev", "answers": answers })
                .to_string()
                .into_bytes(),
        })
        .expect("a readable response")
    }

    fn picked(choice: &str, options: &[&str]) -> Value {
        let probabilities: serde_json::Map<String, Value> = options
            .iter()
            .map(|option| {
                (
                    (*option).to_string(),
                    json!(if *option == choice { 1.0 } else { 0.0 }),
                )
            })
            .collect();
        json!({ "type": "choice", "choice": choice, "confidence": 1.0, "probabilities": probabilities })
    }

    #[test]
    fn the_set_asks_three_independent_questions_and_validates() {
        let questions = questions();
        questions.validate().expect("a valid set");
        let asked: Vec<&str> = questions.iter().map(|(id, _)| id).collect();
        assert_eq!(asked, ["action", "lane", "opener"]);
        // The action wording is Classify's, so its answer means the same.
        assert_eq!(
            serde_json::to_value(questions.get("action")).unwrap(),
            serde_json::to_value(crate::classify::questions().get("action")).unwrap()
        );
        // Twenty-one openers and the escape.
        let opener = serde_json::to_value(questions.get("opener")).unwrap();
        assert_eq!(opener["criteria"].as_object().unwrap().len(), 22);
        assert_eq!(OPENERS.len(), 21);
    }

    #[test]
    fn the_argmax_opener_is_the_line() {
        let ids: Vec<&str> = OPENERS
            .iter()
            .map(|(id, _, _)| *id)
            .chain(["none"])
            .collect();
        let triage = triage_of(&response(json!({
            "action": picked("respond", &["respond", "clarify", "end_conversation", "none"]),
            "lane": picked("computer", &["chat", "computer", "none"]),
            "opener": picked("look_into", &ids),
        })));
        assert_eq!(triage.route, Route::Respond);
        assert_eq!(triage.lane, Lane::Computer);
        assert_eq!(triage.line(), "I'll look into that now.");
        let body = feedback(2, &triage);
        assert_eq!(body["type"], "judgment");
        assert_eq!(body["verdict"], "respond");
        assert_eq!(body["lane"], "computer");
        assert_eq!(body["opener"], "look_into");
        assert_eq!(body["set"], SET);

        // `none` shows no opener; the verdict is the line.
        let quiet = triage_of(&response(json!({
            "action": picked("end_conversation", &["respond", "clarify", "end_conversation", "none"]),
            "lane": picked("none", &["chat", "computer", "none"]),
            "opener": picked("none", &ids),
        })));
        assert!(quiet.opener.is_none());
        assert_eq!(quiet.lane, Lane::Unknown);
        assert_eq!(quiet.line(), "end_conversation");
        assert!(feedback(2, &quiet)["opener"].is_null());
    }

    #[test]
    fn candidates_are_bounded_and_ranked_by_probability() {
        assert!(candidates_of(&json!("repo")).is_err());
        assert!(candidates_of(&json!([])).is_err());
        assert!(candidates_of(&json!([{ "id": "none" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "a" }, { "id": "a" }])).is_err());
        assert!(candidates_of(&json!([{ "id": "x".repeat(65) }])).is_err());
        let many: Vec<Value> = (0..17).map(|i| json!({ "id": format!("r{i}") })).collect();
        assert!(candidates_of(&Value::Array(many)).is_err());

        let candidates = candidates_of(&json!([
            { "id": "openagents", "label": "OpenAgentsInc/openagents" },
            { "id": "psionic", "label": "OpenAgentsInc/psionic" },
            { "id": "run_tests", "label": "Run the tests" },
        ]))
        .unwrap();
        rank_questions(&candidates).validate().expect("valid");
        let ranked = ranking(
            &response(json!({ "next": {
                "type": "choice", "choice": "psionic", "confidence": 0.6,
                "probabilities": { "openagents": 0.3, "psionic": 0.6, "run_tests": 0.0, "none": 0.1 }
            }})),
            &candidates,
        );
        let order: Vec<&str> = ranked.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(order, ["psionic", "openagents", "run_tests"]);
    }
}
