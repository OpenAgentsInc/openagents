//! Jev decides what the delegate's briefing says (issue #9746, series 6
//! and later).
//!
//! The host searches Coder's knowledge base with the task instruction and
//! hands the episode every candidate it found
//! ([`crate::briefing_knowledge`]). With `CODER_ONE_BRIEFING_JEV` set to a
//! question set's name, the episode asks Jev one request before
//! delegation:
//!
//! - one Noul per knowledge candidate, the set's entry question. The
//!   briefing keeps each candidate at the set's `keep` threshold or above,
//!   whole, in order of Jev's probability, up to the set's knowledge
//!   budget when it has one.
//! - one Noul per requirement the rule-based extraction
//!   ([`crate::requirements::mechanical`]) found in the instruction: whether
//!   a grader is likely to check it and a solver is likely to get it wrong
//!   or skip it. The briefing lists each requirement at the set's `flag`
//!   threshold or above under "Requirements Jev flags as easy to miss".
//!
//! This module holds every question set and its thresholds, so one file is
//! what a reviewer reads: [`V1`] is series 6's and [`V2`] is series 7's.
//! The record keeps every probability, kept or not. When Jev doesn't
//! answer every question, the selection fails: the episode never falls
//! back to the host's lexical ranking.

use serde_json::{Value, json};

use crate::briefing_knowledge::{Entry, Flagged, Knowledge};
use crate::component::jev::{Ask, Asked, JevMode};
use crate::record::{Implementation, Recorder};

/// The variable that names the question set: `on` or `v1` for [`V1`],
/// `v2` for [`V2`], and `off` or unset for none.
pub const ENV: &str = "CODER_ONE_BRIEFING_JEV";

/// The file the episode records the selection in, under its artifacts.
pub const ARTIFACT: &str = "briefing-jev.json";

/// The episode's outcome and exit code when Jev didn't answer: a fault,
/// not a run to grade.
pub const OUTCOME: &str = "briefing_jev_unavailable";
pub const EXIT_CODE: i32 = 7;

/// The most requirements one request asks about.
pub const MAX_REQUIREMENTS: usize = 12;

/// The record's schema.
pub const SCHEMA: &str = "openagents.coder_one.briefing_jev.v1";

/// The question asked of each requirement, in every set.
pub const REQUIREMENT_QUESTION: &str = "Is this requirement one a grader is likely to check \
and a solver is likely to get wrong or skip?";

/// The paragraph under "Requirements Jev flags as easy to miss".
pub const FLAG_NOTE: &str = "Jev, a decision model, judged each requirement below \
as one a grader is likely to check and a solver is likely to get wrong or skip. \
Verify each one before you finish.";

/// One question set: the questions, the thresholds, and how the kept
/// entries are bounded.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QuestionSet {
    /// The name `CODER_ONE_BRIEFING_JEV` takes and the record carries.
    pub name: &'static str,
    /// The question asked of each knowledge candidate.
    pub entry_question: &'static str,
    /// A candidate is kept when Jev's probability is at least this.
    pub keep: f64,
    /// A requirement is flagged when Jev's probability is at least this.
    pub flag: f64,
    /// The most characters of entry text the kept entries take, in order
    /// of Jev's probability; a kept entry that would pass it is skipped.
    /// `None` leaves only the briefing cap.
    pub budget: Option<usize>,
    /// The sentence the knowledge section adds under the host's note.
    pub kept_note: &'static str,
}

/// Series 6's set.
pub const V1: QuestionSet = QuestionSet {
    name: "v1",
    entry_question: "Does this entry apply to this task and change what a solver should do?",
    keep: 0.5,
    flag: 0.5,
    budget: None,
    kept_note: "Jev, a decision model, chose these entries from the candidates Coder's \
knowledge search found; each heading shows Jev's probability that the entry applies \
to this task.",
};

/// Series 7's set. In series 6, [`V1`]'s question ranked two generic
/// slips above the task's own edge case, every requirement passed its 0.5
/// flag, and the longer briefing cost more. This set asks about what the
/// task's outputs depend on, flags at 0.7, and holds the kept entries to
/// series 2's 16,000-character budget.
pub const V2: QuestionSet = QuestionSet {
    name: "v2",
    entry_question: "Does this entry state a method, formula, parameter, or edge case \
that this task's required outputs depend on?",
    keep: 0.5,
    flag: 0.7,
    budget: Some(16_000),
    kept_note: "Jev, a decision model, chose these entries from the candidates Coder's \
knowledge search found; each heading shows Jev's probability that the task's required \
outputs depend on what the entry states.",
};

/// Every set, by name.
pub const SETS: [QuestionSet; 2] = [V1, V2];

/// The question set the environment names, if any.
///
/// # Errors
///
/// A message when the variable names no set.
pub fn from_env() -> Result<Option<QuestionSet>, String> {
    match std::env::var(ENV)
        .ok()
        .map(|value| value.trim().to_ascii_lowercase())
        .as_deref()
    {
        None | Some("" | "off") => Ok(None),
        Some("on") => Ok(Some(V1)),
        Some(name) => SETS
            .iter()
            .find(|set| set.name == name)
            .copied()
            .map(Some)
            .ok_or_else(|| format!("{ENV} is {name:?}; it takes on, v1, v2, or off")),
    }
}

/// One candidate as Jev's state shows it: the fields of the entry's front
/// matter that say what it is and when it applies.
#[derive(Debug, Clone, PartialEq)]
pub struct Candidate {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub applies_when: String,
}

impl Candidate {
    /// Reads the candidate's fields from the entry's front matter. A field
    /// the entry lacks is empty.
    #[must_use]
    pub fn of(entry: &Entry) -> Self {
        let field = |key| front_matter_field(&entry.text, key).unwrap_or_default();
        Self {
            id: entry.id.clone(),
            title: field("title"),
            summary: field("summary"),
            applies_when: field("applies_when"),
        }
    }
}

/// One value from YAML front matter between `---` lines: a plain or quoted
/// scalar on the key's line, or a folded (`>`, `>-`) or literal (`|`)
/// block on the indented lines after it, joined with spaces.
#[must_use]
pub fn front_matter_field(text: &str, key: &str) -> Option<String> {
    let mut lines = text.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let block: Vec<&str> = lines.take_while(|line| line.trim() != "---").collect();
    let prefix = format!("{key}:");
    let at = block.iter().position(|line| line.starts_with(&prefix))?;
    let rest = block[at][prefix.len()..].trim();
    let value = if rest.is_empty() || rest.starts_with('>') || rest.starts_with('|') {
        block[at + 1..]
            .iter()
            .take_while(|line| line.starts_with(' ') || line.trim().is_empty())
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        rest.trim_matches('"').trim_matches('\'').to_string()
    };
    Some(value)
}

/// Jev's state and the question set: the instruction, each candidate, and
/// each requirement, with one Noul per candidate (`entry_{i}`) and one per
/// requirement (`requirement_{i}`).
#[must_use]
pub fn request(
    set: &QuestionSet,
    instruction: &str,
    candidates: &[Candidate],
    requirements: &[String],
) -> (Value, jev::Questions) {
    let mut questions = jev::Questions::new();
    for (i, candidate) in candidates.iter().enumerate() {
        questions = questions.with(
            format!("entry_{i}"),
            jev::Noul::new(format!(
                "Consider the knowledge entry `candidates[{i}]` (id `{}`). {}",
                candidate.id, set.entry_question
            )),
        );
    }
    for (i, _) in requirements.iter().enumerate() {
        questions = questions.with(
            format!("requirement_{i}"),
            jev::Noul::new(format!(
                "Consider the requirement `requirements[{i}]`. {REQUIREMENT_QUESTION}"
            )),
        );
    }
    let state = json!({
        "task": instruction.trim(),
        "candidates": candidates.iter().map(|c| json!({
            "id": c.id,
            "title": c.title,
            "summary": c.summary,
            "applies_when": c.applies_when,
        })).collect::<Vec<_>>(),
        "requirements": requirements,
    });
    (state, questions)
}

/// What Jev decided, and every probability behind it.
#[derive(Debug, Clone, PartialEq)]
pub struct Selection {
    /// The knowledge the briefing carries: the kept entries in order of
    /// Jev's probability, each with it, and the flagged requirements.
    pub knowledge: Knowledge,
    /// Each candidate in the host's order with Jev's probability.
    pub candidates: Vec<(Entry, f64)>,
    /// Kept candidates the set's budget left out, by id.
    pub over_budget: Vec<String>,
    /// Each requirement with Jev's probability.
    pub requirements: Vec<(String, f64)>,
}

/// Applies the set's thresholds and budget to Jev's answers. `noul` looks
/// up an answer by question ID.
///
/// # Errors
///
/// A message naming the first question Jev left unanswered: the selection
/// needs every answer, and never fills a gap from the host's ranking.
pub fn decide(
    set: &QuestionSet,
    host: &Knowledge,
    requirements: &[String],
    noul: impl Fn(&str) -> Option<f64>,
) -> Result<Selection, String> {
    let mut candidates = Vec::new();
    for (i, entry) in host.entries.iter().enumerate() {
        let p = noul(&format!("entry_{i}"))
            .ok_or_else(|| format!("Jev gave no answer for knowledge entry {}", entry.id))?;
        candidates.push((entry.clone(), p));
    }
    let mut judged = Vec::new();
    for (i, requirement) in requirements.iter().enumerate() {
        let p = noul(&format!("requirement_{i}"))
            .ok_or_else(|| format!("Jev gave no answer for requirement {}", i + 1))?;
        judged.push((requirement.clone(), p));
    }
    let mut kept: Vec<(usize, Entry, f64)> = candidates
        .iter()
        .enumerate()
        .filter(|(_, (_, p))| *p >= set.keep)
        .map(|(rank, (entry, p))| {
            let mut entry = entry.clone();
            entry.jev = Some(*p);
            (rank, entry, *p)
        })
        .collect();
    // Jev's probability first; the host's rank breaks a tie.
    kept.sort_by(|a, b| b.2.total_cmp(&a.2).then(a.0.cmp(&b.0)));
    let mut entries = Vec::new();
    let mut over_budget = Vec::new();
    let mut used = 0;
    for (_, entry, _) in kept {
        let size = entry.text.chars().count();
        if set.budget.is_some_and(|budget| used + size > budget) {
            over_budget.push(entry.id);
            continue;
        }
        used += size;
        entries.push(entry);
    }
    let knowledge = Knowledge {
        note: host.note.clone(),
        entries,
        kept_note: Some(set.kept_note),
        flagged: judged
            .iter()
            .filter(|(_, p)| *p >= set.flag)
            .map(|(text, p)| Flagged {
                text: text.clone(),
                p: *p,
            })
            .collect(),
    };
    Ok(Selection {
        knowledge,
        candidates,
        over_budget,
        requirements: judged,
    })
}

/// The selection's record: the set, its thresholds and questions, every
/// candidate and requirement with its probability and fate, and the
/// request's key. `outcome` is `selected` or `failed`.
#[must_use]
pub fn record(
    set: &QuestionSet,
    host: &Knowledge,
    requirements: &[String],
    asked: &Asked,
    selection: Option<&Selection>,
    error: Option<&str>,
) -> Value {
    let p = |id: String| asked.noul(&id);
    let fate = |entry: &Entry, p: Option<f64>| match (p, selection) {
        (None, _) => "unanswered",
        (Some(p), _) if p < set.keep => "below the threshold",
        (_, Some(s)) if s.over_budget.contains(&entry.id) => "kept, past the budget",
        _ => "kept",
    };
    json!({
        "schema": SCHEMA,
        "question_set": set.name,
        "outcome": if selection.is_some() { "selected" } else { "failed" },
        "error": error,
        "thresholds": { "keep": set.keep, "flag": set.flag, "budget_chars": set.budget },
        "questions": { "entry": set.entry_question, "requirement": REQUIREMENT_QUESTION },
        "request_key": asked.key,
        "how": asked.how,
        "input_tokens": asked.input_tokens,
        "milliseconds": asked.milliseconds,
        "candidates": host.entries.iter().enumerate().map(|(i, entry)| {
            let p = p(format!("entry_{i}"));
            json!({
                "rank": i + 1,
                "id": entry.id,
                "version": entry.version,
                "sha256": entry.sha256,
                "score": entry.score,
                "chars": entry.text.chars().count(),
                "title": Candidate::of(entry).title,
                "p": p,
                "kept": p.is_some_and(|p| p >= set.keep),
                "fate": fate(entry, p),
            })
        }).collect::<Vec<_>>(),
        "kept": selection.map(|s| s.knowledge.entries.iter().map(|e| json!({
            "id": e.id, "version": e.version, "sha256": e.sha256, "p": e.jev,
        })).collect::<Vec<_>>()),
        "requirements": requirements.iter().enumerate().map(|(i, text)| {
            let p = p(format!("requirement_{i}"));
            json!({ "text": text, "p": p, "flagged": p.is_some_and(|p| p >= set.flag) })
        }).collect::<Vec<_>>(),
    })
}

/// The selector's implementation record: its questions and thresholds.
#[must_use]
pub fn implementation(set: &QuestionSet) -> Implementation {
    Implementation::new(
        "task.briefing_jev",
        &format!(
            "Jev chooses knowledge and flags requirements ({})",
            set.name
        ),
        &json!({
            "keep": set.keep,
            "flag": set.flag,
            "budget_chars": set.budget,
            "entry_question": set.entry_question,
            "requirement_question": REQUIREMENT_QUESTION,
        }),
    )
}

/// Asks Jev the one request and applies the set.
///
/// # Errors
///
/// The record of the failed selection and why it failed, when Jev didn't
/// answer every question.
pub async fn select(
    set: &QuestionSet,
    mode: &JevMode,
    recorder: &Recorder,
    deadline: Option<crate::deadline::Deadline>,
    instruction: &str,
    host: &Knowledge,
    requirements: &[String],
) -> Result<(Selection, Value), (String, Value)> {
    let candidates: Vec<Candidate> = host.entries.iter().map(Candidate::of).collect();
    let (state, questions) = request(set, instruction, &candidates, requirements);
    let asked = crate::component::jev::ask(
        mode,
        recorder,
        Ask {
            component: "task.briefing_jev",
            name: "jev_briefing",
            id: "jev_briefing-1".to_string(),
            state,
            questions,
            parent: None,
            deadline,
        },
    )
    .await;
    let decided = if asked.answered() {
        decide(set, host, requirements, |id| {
            let requirement = id.starts_with("requirement_");
            asked.gate(
                id,
                if requirement {
                    "briefing.flag"
                } else {
                    "briefing.keep"
                },
                if requirement { set.flag } else { set.keep },
            )
        })
    } else {
        Err(format!(
            "Jev didn't answer the briefing request: {}",
            asked.error.as_deref().unwrap_or("no answers")
        ))
    };
    match decided {
        Ok(selection) => {
            let record = record(set, host, requirements, &asked, Some(&selection), None);
            Ok((selection, record))
        }
        Err(error) => {
            let record = record(set, host, requirements, &asked, None, Some(&error));
            Err((error, record))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::delegate::{BRIEFING_CAP, Briefing, BriefingInputs};

    fn candidate_entry(id: &str) -> Entry {
        use sha2::{Digest, Sha256};
        let text = format!(
            "---\nid: {id}\nversion: 2\ntitle: {id} title\nsummary: >-\n  What {id} says,\n  folded.\napplies_when: When {id} applies.\n---\n\nThe body of {id}.\n"
        );
        Entry {
            id: id.to_string(),
            version: 2,
            sha256: Sha256::digest(text.as_bytes())
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
            score: Some(0.9),
            text,
            jev: None,
        }
    }

    fn host() -> Knowledge {
        Knowledge {
            note: Some("Act on these.".to_string()),
            entries: vec![
                candidate_entry("a.first"),
                candidate_entry("b.second"),
                candidate_entry("c.third"),
            ],
            ..Knowledge::NONE
        }
    }

    fn requirements() -> Vec<String> {
        vec![
            "Write results.csv with two decimals.".to_string(),
            "Keep formulas intact.".to_string(),
        ]
    }

    fn answers<'a>(values: &'a [(&'a str, f64)]) -> impl Fn(&str) -> Option<f64> + 'a {
        move |id| values.iter().find(|(k, _)| *k == id).map(|(_, p)| *p)
    }

    #[test]
    fn front_matter_fields_read_plain_and_folded_values() {
        let text = "---\nid: x\ntitle: \"A title\"\nsummary: >-\n  line one\n  line two\napplies_when: >-\n  when it applies\nstatus: admitted\n---\n\nbody\n";
        assert_eq!(front_matter_field(text, "title").unwrap(), "A title");
        assert_eq!(
            front_matter_field(text, "summary").unwrap(),
            "line one line two"
        );
        assert_eq!(
            front_matter_field(text, "applies_when").unwrap(),
            "when it applies"
        );
        assert_eq!(front_matter_field(text, "missing"), None);
        assert_eq!(front_matter_field("no front matter", "title"), None);
    }

    #[test]
    fn the_request_asks_one_noul_per_candidate_and_requirement() {
        let host = host();
        let candidates: Vec<Candidate> = host.entries.iter().map(Candidate::of).collect();
        assert_eq!(candidates[1].summary, "What b.second says, folded.");
        let (state, questions) = request(&V1, "Do the task.", &candidates, &requirements());
        assert_eq!(questions.len(), 5);
        let body = serde_json::to_value(
            jev::SystemOneRequest::new(jev::Entry::from(state.clone()), questions)
                .body("jev")
                .unwrap(),
        )
        .unwrap();
        let q = &body["questions"];
        assert_eq!(q["entry_0"]["type"], "noul");
        let entry = q["entry_1"]["instructions"].as_str().unwrap();
        assert!(entry.contains("`candidates[1]`") && entry.contains("b.second"));
        assert!(entry.ends_with(V1.entry_question));
        let requirement = q["requirement_1"]["instructions"].as_str().unwrap();
        assert!(requirement.contains("`requirements[1]`"));
        assert!(requirement.ends_with(REQUIREMENT_QUESTION));
        assert_eq!(state["task"], "Do the task.");
        assert_eq!(state["candidates"][2]["id"], "c.third");
        assert_eq!(
            state["candidates"][2]["applies_when"],
            "When c.third applies."
        );
        assert_eq!(state["requirements"][0], requirements()[0]);
    }

    #[test]
    fn thresholds_keep_and_flag_at_or_above_one_half_ordered_by_p() {
        let values = [
            ("entry_0", 0.62),
            ("entry_1", 0.49),
            ("entry_2", 0.91),
            ("requirement_0", 0.5),
            ("requirement_1", 0.2),
        ];
        let selection = decide(&V1, &host(), &requirements(), answers(&values)).unwrap();
        let kept: Vec<_> = selection
            .knowledge
            .entries
            .iter()
            .map(|e| (e.id.as_str(), e.jev))
            .collect();
        assert_eq!(kept, [("c.third", Some(0.91)), ("a.first", Some(0.62))]);
        assert_eq!(selection.knowledge.kept_note, Some(V1.kept_note));
        assert!(selection.over_budget.is_empty());
        assert_eq!(selection.knowledge.note.as_deref(), Some("Act on these."));
        assert_eq!(
            selection.knowledge.flagged,
            [Flagged {
                text: requirements()[0].clone(),
                p: 0.5
            }]
        );
        assert_eq!(selection.candidates.len(), 3);
        assert_eq!(selection.requirements[1].1, 0.2);
    }

    #[test]
    fn a_missing_answer_fails_the_selection() {
        let values = [("entry_0", 0.9), ("entry_1", 0.9), ("requirement_0", 0.9)];
        let error = decide(&V1, &host(), &requirements(), answers(&values)).unwrap_err();
        assert!(error.contains("c.third"), "{error}");
        let values = [
            ("entry_0", 0.9),
            ("entry_1", 0.9),
            ("entry_2", 0.9),
            ("requirement_0", 0.9),
        ];
        let error = decide(&V1, &host(), &requirements(), answers(&values)).unwrap_err();
        assert!(error.contains("requirement 2"), "{error}");
    }

    #[test]
    fn the_briefing_lists_kept_entries_with_p_and_the_flagged_requirements() {
        let values = [
            ("entry_0", 0.62),
            ("entry_1", 0.1),
            ("entry_2", 0.91),
            ("requirement_0", 0.8),
            ("requirement_1", 0.3),
        ];
        let selection = decide(&V1, &host(), &requirements(), answers(&values)).unwrap();
        let inputs = BriefingInputs {
            instruction: "Do the task.".to_string(),
            requirements: requirements().into_iter().map(|r| (r, None)).collect(),
            files: Vec::new(),
            spans: Vec::new(),
            commands: Vec::new(),
            last_output: None,
            conclusion: crate::delegate::NO_EXPLORER.to_string(),
            directions: "Work in /app.".to_string(),
        };
        let briefing = Briefing::build_knowing(&inputs, &selection.knowledge, BRIEFING_CAP);
        let text = &briefing.text;
        assert!(text.contains("### c.third (version 2, sha256 "));
        assert!(text.contains(", Jev p=0.91)\n"));
        assert!(text.contains(", Jev p=0.62)\n"));
        assert!(!text.contains("b.second title"));
        assert!(text.find("c.third title").unwrap() < text.find("a.first title").unwrap());
        assert!(text.contains(&format!("Act on these. {}", V1.kept_note)));
        let flagged = text
            .find("## Requirements Jev flags as easy to miss")
            .unwrap();
        assert!(text[flagged..].contains(FLAG_NOTE));
        assert!(text[flagged..].contains("- Write results.csv with two decimals. (Jev p=0.80)"));
        assert!(
            !text[flagged..text.find(crate::briefing_knowledge::HEADING).unwrap()]
                .contains("Keep formulas intact.")
        );
        assert!(
            briefing
                .included
                .iter()
                .any(|i| i == "flagged requirement 1")
        );
    }

    fn recorded(host: &Knowledge, answers: Value) -> JevMode {
        let candidates: Vec<Candidate> = host.entries.iter().map(Candidate::of).collect();
        let (state, questions) = request(&V1, "Do the task.", &candidates, &requirements());
        let body = Value::Object(
            jev::SystemOneRequest::new(jev::Entry::from(state), questions)
                .body(crate::credentials::JEV_MODEL)
                .unwrap(),
        );
        let key = crate::component::jev::key(&body["state"], &body["questions"]);
        let mut set = crate::component::jev::Recorded::empty();
        set.entries.insert(
            key,
            crate::component::jev::RecordedAnswer {
                name: "jev_briefing".to_string(),
                model: crate::credentials::JEV_MODEL.to_string(),
                answers,
                input_tokens: Some(900),
                output_tokens: Some(5),
                milliseconds: Some(800),
                source: "test".to_string(),
            },
        );
        JevMode::Recorded(set)
    }

    #[tokio::test]
    async fn a_selection_is_one_recorded_decision_with_every_probability() {
        let host = host();
        let answers = json!({
            "entry_0": { "type": "noul", "noul": 0.7 },
            "entry_1": { "type": "noul", "noul": 0.2 },
            "entry_2": { "type": "noul", "noul": 0.55 },
            "requirement_0": { "type": "noul", "noul": 0.9 },
            "requirement_1": { "type": "noul", "noul": 0.1 },
        });
        let recorder = Recorder::default();
        let (selection, record) = select(
            &V1,
            &recorded(&host, answers),
            &recorder,
            None,
            "Do the task.",
            &host,
            &requirements(),
        )
        .await
        .unwrap();
        assert_eq!(selection.knowledge.entries.len(), 2);
        let steps = recorder.steps();
        let decisions: Vec<_> = steps
            .iter()
            .filter_map(|s| s.call.as_ref().filter(|c| c.is_decision()))
            .collect();
        assert_eq!(decisions.len(), 1);
        assert_eq!(record["outcome"], "selected");
        assert_eq!(record["thresholds"]["keep"], V1.keep);
        assert_eq!(record["question_set"], "v1");
        assert_eq!(candidates_fate(&record, 1), "below the threshold");
        let candidates = record["candidates"].as_array().unwrap();
        assert_eq!(candidates.len(), 3);
        assert_eq!(candidates[1]["p"], 0.2);
        assert_eq!(candidates[1]["kept"], false);
        assert_eq!(candidates[0]["title"], "a.first title");
        assert_eq!(record["kept"][0]["id"], "a.first");
        assert_eq!(record["requirements"][0]["flagged"], true);
        assert_eq!(record["requirements"][1]["p"], 0.1);
    }

    #[tokio::test]
    async fn jev_unavailable_fails_without_a_lexical_fallback() {
        let host = host();
        let recorder = Recorder::default();
        let (error, record) = select(
            &V1,
            &JevMode::Off,
            &recorder,
            None,
            "Do the task.",
            &host,
            &requirements(),
        )
        .await
        .unwrap_err();
        assert!(error.contains("Jev didn't answer"), "{error}");
        assert_eq!(record["outcome"], "failed");
        assert!(record["kept"].is_null());
        assert!(record["candidates"][0]["p"].is_null());
        // A partial answer fails too.
        let partial = json!({ "entry_0": { "type": "noul", "noul": 0.9 } });
        let (error, _) = select(
            &V1,
            &recorded(&host, partial),
            &Recorder::default(),
            None,
            "Do the task.",
            &host,
            &requirements(),
        )
        .await
        .unwrap_err();
        assert!(error.contains("b.second"), "{error}");
    }

    fn candidates_fate(record: &Value, i: usize) -> &str {
        record["candidates"][i]["fate"].as_str().unwrap()
    }

    #[test]
    fn the_environment_names_a_question_set() {
        assert_eq!(
            SETS.iter().map(|s| s.name).collect::<Vec<_>>(),
            ["v1", "v2"]
        );
        assert_ne!(V1.entry_question, V2.entry_question);
        assert_eq!((V2.keep, V2.flag, V2.budget), (0.5, 0.7, Some(16_000)));
    }

    #[test]
    fn v2_flags_at_seven_tenths_and_holds_kept_entries_to_its_budget() {
        let big = |id: &str, chars: usize| {
            use sha2::{Digest, Sha256};
            let text = format!(
                "---\nid: {id}\nversion: 1\ntitle: {id}\n---\n\n{}\n",
                "x".repeat(chars)
            );
            Entry {
                id: id.to_string(),
                version: 1,
                sha256: Sha256::digest(text.as_bytes())
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect(),
                score: None,
                text,
                jev: None,
            }
        };
        let host = Knowledge {
            entries: vec![big("a", 9_000), big("b", 8_000), big("c", 5_000)],
            ..Knowledge::NONE
        };
        // b is kept first, a would pass 16,000 characters, and c still fits.
        let values = [
            ("entry_0", 0.8),
            ("entry_1", 0.9),
            ("entry_2", 0.6),
            ("requirement_0", 0.69),
            ("requirement_1", 0.7),
        ];
        let selection = decide(&V2, &host, &requirements(), answers(&values)).unwrap();
        let ids: Vec<_> = selection
            .knowledge
            .entries
            .iter()
            .map(|e| e.id.as_str())
            .collect();
        assert_eq!(ids, ["b", "c"]);
        assert_eq!(selection.over_budget, ["a"]);
        assert_eq!(selection.knowledge.flagged.len(), 1);
        assert_eq!(selection.knowledge.flagged[0].text, requirements()[1]);
        assert!(selection.knowledge.note().ends_with(V2.kept_note));
    }
}
