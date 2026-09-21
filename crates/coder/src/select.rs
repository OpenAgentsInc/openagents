//! Selection: where the observed evidence meets a measured judgment.
//!
//! [`crate::evidence`] builds the candidate set — what the host saw,
//! bounded and digested, never a path a model invented. This module binds
//! that set to `questions/evidence-relevance.json`: it renders the decide
//! request a caller sends and reads the typed answer back into a
//! [`Ranking`]. The model ranks what the host showed it and nothing
//! else — the gate's options are exactly the observed paths, so an answer
//! cannot name a file nobody observed, and the `none` the set declares
//! stays an honest answer when none of them helps.
//!
//! Everything here is pure: candidates and a task in, a request out; an
//! answer and the same candidates in, a ranking out. The module makes no
//! model call of its own — the caller owns the HTTP — reads no
//! filesystem, and keeps no clock, so the same inputs always render the
//! same request and read the same ranking.
//!
//! A ranking keeps the question set's identity — its id and the digest
//! of its wording — beside the answer, so the judgment is attributable
//! to the wording that produced it and two runs over the same set are
//! comparable. Refusals and omissions are part of what was shown: a
//! refused candidate goes into the state as a name without content, an
//! omitted input as `omitted — reason`, and the ranking records the
//! omissions the model was shown.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::LazyLock;

use jev::{Answer, ChoiceAnswer, Question, Questions, SystemOneRequest, SystemOneResponse};
use serde_json::{Map, Value, json};

use crate::evidence::{Candidate, Candidates, Readness};
use crate::questions::Set;

/// The wording this binding asks from, vendored at build time so the
/// request a caller sends is always rendered from the same set. The
/// digest the state and the ranking carry is computed from this parse,
/// the way [`Set::digest`] computes it for a file on disk.
const SET_JSON: &str = include_str!("../../../questions/evidence-relevance.json");

/// The value an `options` field takes when the run fills the options in —
/// the marker `questions::Set::build` reads, restated here because this
/// module fills its own.
const SUPPLIED: &str = "supplied";

/// The set, parsed once: an invalid vendored set is a defect of the
/// build, not a condition a caller can answer.
static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set =
        serde_json::from_str(SET_JSON).expect("the vendored evidence-relevance set parses");
    set.validate()
        .expect("the vendored evidence-relevance set is one this host asks");
    set
});

/// The binder between observed candidates and the measured judgment:
/// [`Select::request`] renders the decide request, [`Select::ranking`]
/// reads the answer back. A namespace rather than a value — the
/// functions are the surface.
pub struct Select;

impl Select {
    /// The decide request the set asks over these candidates.
    ///
    /// The state carries the task, every candidate's observable record —
    /// its option name, path, span, readness, and size, plus the
    /// disclosure a refused candidate makes and never a byte it does not
    /// hold — every input the bounds omitted, listed as
    /// `omitted — reason`, and the set's own id and digest. The gate's
    /// options are exactly the candidate names plus the `none` the set
    /// declares: nothing a model could invent, and abstention an honest
    /// answer. With no candidates at all the request still asks, and
    /// `none` is the only option there is to give.
    ///
    /// The caller owns the call; what comes back is read by
    /// [`Select::ranking`] against the same candidates.
    #[must_use]
    pub fn request(task: &str, candidates: &Candidates) -> SystemOneRequest {
        SystemOneRequest::new(state(task, candidates), questions(candidates))
    }

    /// Maps the typed answer back onto the candidates it ranked.
    ///
    /// The gate's pick is read against the options the request offered:
    /// a candidate's name picks that candidate by index, a name the set
    /// itself declares — `none` — is preserved as abstention, and
    /// anything else is an [`Fault::UnknownOption`], reported rather
    /// than clamped to a listed candidate. The `any_relevant` and
    /// `coverage` probabilities are carried as supplied, the options the
    /// distribution left unranked are recorded, and the ranking keeps
    /// the set's id and digest so the judgment is attributable and two
    /// runs over the same set are comparable.
    ///
    /// # Errors
    ///
    /// Returns [`Fault::Unanswered`] when the gate went unanswered or
    /// answered in another type — an abstention is an answer, silence is
    /// not — and [`Fault::UnknownOption`] when the gate named an option
    /// the request never offered.
    pub fn ranking(
        response: &SystemOneResponse,
        candidates: &Candidates,
    ) -> Result<Ranking, Fault> {
        let Some(Answer::Choice(choice)) = response.answers.get(SET.gate.as_str()) else {
            return Err(Fault::Unanswered);
        };
        let names = option_names(candidates);
        let verdict = if declared(&choice.choice) {
            Verdict::Abstained
        } else {
            match names.iter().position(|name| name == &choice.choice) {
                Some(index) => Verdict::Chosen(index),
                None => {
                    return Err(Fault::UnknownOption {
                        option: choice.choice.clone(),
                    });
                }
            }
        };
        Ok(Ranking {
            choice: choice.clone(),
            verdict,
            any_relevant: noul(response, "any_relevant"),
            coverage: noul(response, "coverage"),
            unranked: unranked(&names, choice),
            omitted: candidates
                .omitted
                .iter()
                .map(|omitted| omitted.path.clone())
                .collect(),
            question_set: SET.id.clone(),
            set_digest: SET.digest(),
        })
    }
}

/// What the gate's pick means for the candidate set.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The model named a candidate: its index in
    /// `Candidates::candidates`.
    Chosen(usize),
    /// The model answered `none`: no listed candidate helps the task. An
    /// abstention is an answer, not a failure to rank.
    Abstained,
}

/// What the answer says about the candidate set, kept whole enough for a
/// caller to act on and a reviewer to audit.
#[derive(Clone, Debug, PartialEq)]
pub struct Ranking {
    /// The gate answer as it arrived: the option named, the confidence,
    /// and the distribution over every option offered.
    pub choice: ChoiceAnswer,
    /// What the gate's pick means for the set.
    pub verdict: Verdict,
    /// The `any_relevant` probability, carried as supplied — `None` when
    /// the door supplied none.
    pub any_relevant: Option<f64>,
    /// Whether the model judged the listed candidates to cover the
    /// task's needs, as the `coverage` probability it supplied.
    pub coverage: Option<f64>,
    /// The options the gate's distribution never named: the candidates
    /// the model left unranked.
    pub unranked: Vec<String>,
    /// The paths the state listed as `omitted — reason`, so a caller
    /// reading the ranking can tell the model saw the omissions beside
    /// the candidates.
    pub omitted: Vec<String>,
    /// The identifier of the question set the answer was produced under.
    pub question_set: String,
    /// The digest of that set's wording, so two runs that asked from the
    /// same file say so whatever the run supplied.
    pub set_digest: String,
}

impl Ranking {
    /// The candidate the verdict names, when it names one — `None` on an
    /// abstention.
    #[must_use]
    pub fn selected<'a>(&self, candidates: &'a Candidates) -> Option<&'a Candidate> {
        match self.verdict {
            Verdict::Chosen(index) => candidates.candidates.get(index),
            Verdict::Abstained => None,
        }
    }

    /// Whether the model abstained.
    #[must_use]
    pub fn abstained(&self) -> bool {
        matches!(self.verdict, Verdict::Abstained)
    }
}

/// Why a response cannot be read as a [`Ranking`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The gate question went unanswered, or its answer is not a Choice
    /// — there is no pick to rank by.
    Unanswered,
    /// The gate named an option the request never offered: not a
    /// candidate's name and not `none`. It is reported as supplied,
    /// never clamped to a listed candidate.
    UnknownOption {
        /// The name the door answered.
        option: String,
    },
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Fault::Unanswered => write!(f, "the gate question went unanswered"),
            Fault::UnknownOption { option } => write!(
                f,
                "the gate named {option:?}, which the request never offered"
            ),
        }
    }
}

impl std::error::Error for Fault {}

/// The state the set judges: the task, every candidate's observable
/// record, every omission the bounds produced, and the set's own
/// identity.
fn state(task: &str, candidates: &Candidates) -> Map<String, Value> {
    let mut state = Map::new();
    state.insert("task".to_string(), json!(task));
    state.insert(
        "question_set".to_string(),
        json!({ "id": SET.id, "digest": SET.digest() }),
    );
    let names = option_names(candidates);
    state.insert(
        "candidates".to_string(),
        Value::Array(
            candidates
                .candidates
                .iter()
                .zip(&names)
                .map(|(candidate, name)| entry(candidate, name))
                .collect(),
        ),
    );
    state.insert(
        "omitted".to_string(),
        Value::Array(
            candidates
                .omitted
                .iter()
                .map(|omitted| {
                    json!({
                        "path": omitted.path,
                        "status": format!("omitted — {}", omitted.reason.word()),
                    })
                })
                .collect(),
        ),
    );
    state
}

/// One candidate's record in the state: the same disclosure `evidence`
/// produced. A refused candidate carries its name, its span, and the
/// reason no byte was admitted — never a digest, which would imply
/// content.
fn entry(candidate: &Candidate, option: &str) -> Value {
    let mut entry = Map::new();
    entry.insert("option".to_string(), json!(option));
    entry.insert("path".to_string(), json!(candidate.path));
    if let Some(span) = candidate.span {
        entry.insert(
            "span".to_string(),
            json!({ "start": span.start, "end": span.end }),
        );
    }
    entry.insert("readness".to_string(), json!(candidate.readness.word()));
    entry.insert("bytes".to_string(), json!(candidate.bytes));
    if let Some(base) = &candidate.base {
        entry.insert("base".to_string(), json!(base));
    }
    if let Some(digest) = &candidate.digest {
        entry.insert("digest".to_string(), json!(digest));
    }
    entry.insert(
        "observation".to_string(),
        json!(candidate.observation.word()),
    );
    entry.insert("present".to_string(), json!(candidate.present));
    if let Some(withheld) = &candidate.withheld {
        entry.insert("withheld".to_string(), json!(withheld));
    }
    Value::Object(entry)
}

/// The questions one request asks: the set's wording with the candidate
/// names written in wherever the set declares `options: "supplied"`.
fn questions(candidates: &Candidates) -> Questions {
    let names = option_names(candidates);
    SET.questions
        .iter()
        .map(|(id, question)| {
            (
                id.clone(),
                Question::Raw(filled(question, candidates, &names)),
            )
        })
        .collect()
}

/// One question with the run's options written in — the same fill
/// `questions::Set::build` performs, except that an empty candidate list
/// is an honest ask rather than an error: the set's own `none` is then
/// the only option, and it answers.
fn filled(question: &Value, candidates: &Candidates, names: &[String]) -> Value {
    if question.get("options").and_then(Value::as_str) != Some(SUPPLIED) {
        return question.clone();
    }
    let mut body = question.as_object().cloned().unwrap_or_default();
    body.remove("options");
    let mut criteria: Map<String, Value> = body
        .get("criteria")
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    for (candidate, name) in candidates.candidates.iter().zip(names) {
        criteria.insert(name.clone(), json!(describe(candidate)));
    }
    body.insert("criteria".to_string(), Value::Object(criteria));
    Value::Object(body)
}

/// The option name one candidate answers to: the bare path, qualified by
/// the span it covers only when another candidate shares the path, and
/// counted up only when a literal path spells a name already taken. An
/// option no candidate answers to would be a name the model could never
/// honestly pick; a name two candidates share would pick neither
/// honestly.
fn option_names(candidates: &Candidates) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for candidate in &candidates.candidates {
        *counts.entry(candidate.path.as_str()).or_default() += 1;
    }
    let mut used = declared_options();
    candidates
        .candidates
        .iter()
        .map(|candidate| {
            let mut name = candidate.path.clone();
            if counts[candidate.path.as_str()] > 1 || used.contains(&name) {
                name = match candidate.span {
                    Some(span) => {
                        format!("{} (lines {}-{})", candidate.path, span.start, span.end)
                    }
                    None => format!("{} (the whole file)", candidate.path),
                };
            }
            let mut extra = 2;
            while used.contains(&name) {
                name = format!("{name} ({extra})");
                extra += 1;
            }
            used.insert(name.clone());
            name
        })
        .collect()
}

/// The options the set's gate question declares on its own — `none` —
/// which no supplied option may shadow.
fn declared_options() -> BTreeSet<String> {
    SET.questions
        .get(&SET.gate)
        .and_then(|gate| gate.get("criteria"))
        .and_then(Value::as_object)
        .map(|criteria| criteria.keys().cloned().collect())
        .unwrap_or_default()
}

/// Whether a gate answer names an option the set itself declares rather
/// than a candidate — the abstention the wording keeps honest.
fn declared(option: &str) -> bool {
    declared_options().contains(option)
}

/// What an option says about its candidate: the facts the state lists,
/// compressed to a clause. A refused candidate is a name only, and its
/// description says so rather than implying content.
fn describe(candidate: &Candidate) -> String {
    let covered = match candidate.span {
        Some(span) => format!("lines {}-{}", span.start, span.end),
        None => "the whole file".to_string(),
    };
    match candidate.readness {
        Readness::Refused => {
            let why = candidate
                .withheld
                .as_deref()
                .unwrap_or("no reason recorded");
            format!("{covered} — a name only; no content was admitted: {why}")
        }
        readness => format!(
            "{covered} — {} read, {} bytes",
            readness.word(),
            candidate.bytes
        ),
    }
}

/// A noul probability, carried as supplied — an answer that is absent or
/// of another type is `None`, never a number made up.
fn noul(response: &SystemOneResponse, id: &str) -> Option<f64> {
    match response.answers.get(id) {
        Some(Answer::Noul(answer)) => Some(answer.noul),
        _ => None,
    }
}

/// The options the gate's distribution never named: the candidates the
/// model left unranked, in the order the set lists them.
fn unranked(names: &[String], choice: &ChoiceAnswer) -> Vec<String> {
    names
        .iter()
        .filter(|name| !choice.probabilities.contains_key(*name))
        .cloned()
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::{Observation, Omission, Omitted, Span};

    /// A read candidate: bytes admitted, digest naming them.
    fn read(path: &str, span: Option<Span>, readness: Readness) -> Candidate {
        Candidate {
            path: path.to_string(),
            span,
            base: Some("base-commit".to_string()),
            digest: Some("ab".repeat(32)),
            bytes: 128,
            readness,
            observation: Observation::Read,
            present: true,
            withheld: None,
        }
    }

    /// A named-only candidate: a path the host recorded without reading
    /// a byte of it.
    fn refused(path: &str) -> Candidate {
        Candidate {
            path: path.to_string(),
            span: None,
            base: Some("base-commit".to_string()),
            digest: None,
            bytes: 0,
            readness: Readness::Refused,
            observation: Observation::Exists,
            present: true,
            withheld: Some("named for existence; no content was requested".to_string()),
        }
    }

    /// The set the tests rank over: a full read, a truncated read, and a
    /// refused name, plus one input the bounds omitted.
    fn candidates() -> Candidates {
        Candidates {
            candidates: vec![
                read("src/a.rs", Some(Span { start: 4, end: 9 }), Readness::Full),
                read("src/b.rs", None, Readness::Truncated),
                refused("src/secret.rs"),
            ],
            omitted: vec![Omitted {
                path: "src/big.rs".to_string(),
                reason: Omission::OverCount,
            }],
        }
    }

    /// A door's response body, decoded the way the client decodes it.
    fn response(choice: &str, probabilities: &[(&str, f64)]) -> SystemOneResponse {
        let body = json!({
            "model": "test-door",
            "answers": {
                "most_relevant": {
                    "type": "choice",
                    "choice": choice,
                    "confidence": 0.8,
                    "probabilities": Map::from_iter(
                        probabilities.iter().map(|(name, p)| ((*name).to_string(), json!(p))),
                    ),
                },
                "any_relevant": { "type": "noul", "noul": 0.7 },
                "coverage": { "type": "noul", "noul": 0.4 },
            },
        });
        SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: body.to_string().into_bytes(),
        })
        .expect("a readable response")
    }

    /// The state a request renders, as JSON.
    fn state_of(request: &SystemOneRequest) -> Value {
        request.state.to_value()
    }

    /// The criteria the gate question offers, in order.
    fn options_of(request: &SystemOneRequest) -> Vec<String> {
        let Some(Question::Raw(gate)) = request.questions.get("most_relevant") else {
            panic!("the gate question renders raw");
        };
        gate["criteria"]
            .as_object()
            .expect("the gate names criteria")
            .keys()
            .cloned()
            .collect()
    }

    #[test]
    fn the_request_carries_every_candidates_record() {
        let request = Select::request("find where the quota ledger is written", &candidates());
        let state = state_of(&request);

        assert_eq!(state["task"], "find where the quota ledger is written");
        assert_eq!(
            state["question_set"]["id"],
            "openagents.evidence-relevance.v1"
        );
        assert_eq!(state["question_set"]["digest"], json!(SET.digest()));

        let listed = state["candidates"].as_array().expect("candidates list");
        assert_eq!(listed.len(), 3);
        assert_eq!(listed[0]["path"], "src/a.rs");
        assert_eq!(listed[0]["span"], json!({"start": 4, "end": 9}));
        assert_eq!(listed[0]["readness"], "full");
        assert_eq!(listed[0]["bytes"], 128);
        assert_eq!(listed[1]["path"], "src/b.rs");
        assert!(listed[1].get("span").is_none());
        assert_eq!(listed[1]["readness"], "truncated");

        // A refused candidate is a name only: the request says so and
        // nothing in its record implies content.
        assert_eq!(listed[2]["path"], "src/secret.rs");
        assert_eq!(listed[2]["readness"], "refused");
        assert_eq!(listed[2]["bytes"], 0);
        assert!(
            listed[2].get("digest").is_none(),
            "a refused candidate holds no bytes and names no digest"
        );
        assert!(
            listed[2]["withheld"]
                .as_str()
                .is_some_and(|why| why.contains("no content")),
            "the request says the content was withheld: {:?}",
            listed[2]
        );
    }

    #[test]
    fn the_options_are_exactly_the_candidate_paths_and_none() {
        let request = Select::request("a task", &candidates());
        assert_eq!(
            options_of(&request),
            ["none", "src/a.rs", "src/b.rs", "src/secret.rs"],
            "the gate offers the declared none and the observed paths — nothing invented"
        );
    }

    #[test]
    fn an_unlisted_answer_is_an_unknown_option_never_clamped() {
        let response = response(
            "src/invented.rs",
            &[("src/invented.rs", 0.9), ("src/a.rs", 0.05), ("none", 0.05)],
        );
        let fault = Select::ranking(&response, &candidates())
            .expect_err("a path the host never observed is a fault");
        assert_eq!(
            fault,
            Fault::UnknownOption {
                option: "src/invented.rs".to_string(),
            }
        );
    }

    #[test]
    fn none_is_an_abstention_not_a_failure_to_rank() {
        let response = response(
            "none",
            &[("none", 0.85), ("src/a.rs", 0.1), ("src/b.rs", 0.05)],
        );
        let ranking = Select::ranking(&response, &candidates()).expect("none is an answer");
        assert!(ranking.abstained());
        assert_eq!(ranking.verdict, Verdict::Abstained);
        assert!(ranking.selected(&candidates()).is_none());
        assert_eq!(ranking.choice.choice, "none");
    }

    #[test]
    fn a_named_candidate_ranks_by_index() {
        let set = candidates();
        let response = response(
            "src/b.rs",
            &[("none", 0.1), ("src/a.rs", 0.2), ("src/b.rs", 0.7)],
        );
        let ranking = Select::ranking(&response, &set).expect("a listed candidate ranks");
        assert_eq!(ranking.verdict, Verdict::Chosen(1));
        assert_eq!(
            ranking
                .selected(&set)
                .map(|candidate| candidate.path.as_str()),
            Some("src/b.rs")
        );
    }

    #[test]
    fn the_probabilities_are_carried_as_supplied() {
        let response = response(
            "src/a.rs",
            &[("none", 0.1), ("src/a.rs", 0.8), ("src/b.rs", 0.1)],
        );
        let ranking = Select::ranking(&response, &candidates()).expect("a ranking");
        assert_eq!(ranking.any_relevant, Some(0.7));
        assert_eq!(ranking.coverage, Some(0.4));

        let mut body = serde_json::from_slice::<Value>(&response.raw().bytes).unwrap();
        body["answers"].as_object_mut().unwrap().remove("coverage");
        let partial = SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: body.to_string().into_bytes(),
        })
        .unwrap();
        let ranking = Select::ranking(&partial, &candidates()).expect("a ranking");
        assert_eq!(
            ranking.coverage, None,
            "an unsupplied noul is absent, not invented"
        );
        assert_eq!(ranking.any_relevant, Some(0.7));
    }

    #[test]
    fn omissions_are_listed_and_the_ranking_remembers_them() {
        let set = candidates();
        let request = Select::request("a task", &set);
        let omitted = &state_of(&request)["omitted"];
        assert_eq!(omitted[0]["path"], "src/big.rs");
        assert_eq!(
            omitted[0]["status"], "omitted — over-count",
            "an omitted input is listed as omitted, with its bound, never as content"
        );

        let response = response("none", &[("none", 1.0)]);
        let ranking = Select::ranking(&response, &set).expect("a ranking");
        assert_eq!(
            ranking.omitted,
            ["src/big.rs"],
            "a caller can tell the model saw the omission"
        );
    }

    #[test]
    fn candidates_the_distribution_skips_are_unranked() {
        let response = response(
            "src/a.rs",
            &[("none", 0.1), ("src/a.rs", 0.8), ("src/b.rs", 0.1)],
        );
        let ranking = Select::ranking(&response, &candidates()).expect("a ranking");
        assert_eq!(
            ranking.unranked,
            ["src/secret.rs"],
            "the distribution gave the refused candidate no entry"
        );
        assert_eq!(ranking.set_digest, SET.digest());
        assert_eq!(ranking.question_set, "openagents.evidence-relevance.v1");
    }

    #[test]
    fn a_repeated_path_names_each_candidate_by_its_span() {
        let set = Candidates {
            candidates: vec![
                read("src/f.rs", Some(Span { start: 1, end: 2 }), Readness::Full),
                read(
                    "src/f.rs",
                    Some(Span { start: 3, end: 4 }),
                    Readness::Truncated,
                ),
            ],
            omitted: Vec::new(),
        };
        let request = Select::request("a task", &set);
        assert_eq!(
            options_of(&request),
            ["none", "src/f.rs (lines 1-2)", "src/f.rs (lines 3-4)"]
        );
        let answered = response(
            "src/f.rs (lines 3-4)",
            &[("none", 0.1), ("src/f.rs (lines 3-4)", 0.9)],
        );
        let ranking = Select::ranking(&answered, &set).expect("a ranking");
        assert_eq!(ranking.verdict, Verdict::Chosen(1));

        // The bare shared path names neither candidate.
        let ambiguous = response("src/f.rs", &[("none", 0.1), ("src/f.rs", 0.9)]);
        assert!(matches!(
            Select::ranking(&ambiguous, &set),
            Err(Fault::UnknownOption { .. })
        ));
    }

    #[test]
    fn an_unanswered_gate_cannot_rank() {
        let body = json!({
            "model": "test-door",
            "answers": {
                "any_relevant": { "type": "noul", "noul": 0.7 },
            },
        });
        let response = SystemOneResponse::decode(jev::RawResponse {
            status: 200,
            headers: Default::default(),
            bytes: body.to_string().into_bytes(),
        })
        .expect("a readable response");
        assert_eq!(
            Select::ranking(&response, &candidates()),
            Err(Fault::Unanswered)
        );
    }

    #[test]
    fn an_empty_candidate_set_still_asks_honestly() {
        let set = Candidates::default();
        let request = Select::request("a task", &set);
        assert_eq!(options_of(&request), ["none"]);
        let response = response("none", &[("none", 1.0)]);
        let ranking = Select::ranking(&response, &set).expect("none is an answer");
        assert!(ranking.abstained());
    }

    #[test]
    fn the_request_renders_deterministically() {
        let set = candidates();
        let first = Select::request("a task", &set);
        let second = Select::request("a task", &set);
        assert_eq!(
            first.body("test-door").expect("a valid body"),
            second.body("test-door").expect("a valid body"),
        );
    }
}
