//! Judging interview answers code doesn't check: self-knowledge,
//! reactions, and reflections (`docs/verse/generative-agents.md`, item 7).
//!
//! A [`Judge`] reads one answer beside the fixture records the item cites,
//! the item's reference answer, and the briefing the agent carried, and
//! returns a [`gi::Judgment`]: whether the records support the answer, and
//! whether it embellishes (states something neither the records nor the
//! briefing hold). [`JevJudge`] asks Jev `questions/interview-answer.json`;
//! [`ScriptedJudge`] is the deterministic stand-in a run with no model
//! uses, and its readings are labeled `scripted` in every row.

use std::collections::BTreeSet;
use std::sync::LazyLock;

use gym::interview as gi;
use gym::row::DoorIdentity;
use gym::suite::Item;

use super::{Briefing, DONT_REMEMBER};
use crate::questions::{Fill, Set};

/// The gate: the cited records support the answer.
pub const SUPPORTED: &str = "supported";
/// The answer states something neither the records nor the briefing hold.
pub const EMBELLISHED: &str = "embellished";

const SET_JSON: &str = include_str!("../../../../questions/interview-answer.json");

static SET: LazyLock<Set> = LazyLock::new(|| {
    let set: Set = serde_json::from_str(SET_JSON).expect("the interview-answer set parses");
    set.validate()
        .expect("the interview-answer set is one this host asks");
    set
});

/// The interview-answer question set.
#[must_use]
pub fn answer_set() -> &'static Set {
    &SET
}

/// The probability at or above which question `id` reads as yes.
#[must_use]
pub fn threshold(id: &str) -> f64 {
    SET.decisions
        .get(id)
        .and_then(|decision| decision.threshold)
        .map_or(0.5, jev::decision::Threshold::value)
}

/// The most briefing text a judge reads, bytes, so the state stays under
/// the set's bound.
pub const BRIEFING_MAX: usize = 8192;

/// One cited record, as the judge reads it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Evidence {
    pub reference: String,
    pub at: u64,
    pub text: String,
}

/// The item's cited records, read from the fixture.
///
/// # Errors
/// When a source doesn't resolve.
pub fn evidence(fixture: &gi::Fixture, item: &Item) -> Result<Vec<Evidence>, String> {
    let state = gi::ItemState::of(item)?;
    state
        .sources
        .iter()
        .map(|reference| {
            let (at, text) = fixture
                .record(reference)
                .ok_or_else(|| format!("item {} cites {reference}, which doesn't read", item.id))?;
            Ok(Evidence {
                reference: reference.clone(),
                at,
                text,
            })
        })
        .collect()
}

/// What a judge is asked about one answer.
#[derive(Clone, Copy, Debug)]
pub struct Case<'a> {
    pub agent: &'a str,
    pub item: &'a Item,
    pub evidence: &'a [Evidence],
    pub briefing: &'a Briefing,
    pub answer: &'a str,
}

/// Reads one interview answer.
pub trait Judge {
    /// # Errors
    /// When nothing judged; the row is recorded unjudged.
    fn judge(&mut self, case: &Case<'_>) -> Result<gi::Judgment, String>;
}

fn clip(text: &str, max: usize) -> &str {
    if text.len() <= max {
        return text;
    }
    let mut end = max;
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

/// The state Jev reads for one answer.
#[must_use]
pub fn state(case: &Case<'_>) -> serde_json::Value {
    let evidence: Vec<serde_json::Value> = case
        .evidence
        .iter()
        .map(|record| {
            serde_json::json!({
                "reference": record.reference,
                "date": &gym::eval::utc_from_unix(record.at)[..10],
                "text": record.text,
            })
        })
        .collect();
    serde_json::json!({
        "agent": case.agent,
        "question": gi::question(case.item),
        "answer": case.answer,
        "reference_answer": case.item.truth,
        "evidence": evidence,
        "briefing": clip(&case.briefing.text, BRIEFING_MAX),
    })
}

/// The decide request for one answer.
///
/// # Errors
/// When the state is larger than the set's policy admits.
pub fn request(case: &Case<'_>) -> Result<jev::SystemOneRequest, String> {
    let state = state(case);
    let size = serde_json::to_vec(&state).map_or(usize::MAX, |b| b.len());
    if let Some(max) = SET.policy.state_max_bytes
        && size as u64 > max
    {
        return Err(format!("the state is {size} bytes, over the set's {max}"));
    }
    Ok(jev::SystemOneRequest::new(state, SET.build(&Fill::None)?))
}

fn judgment(judge: &str, model: &str, supported: f64, embellished: f64) -> gi::Judgment {
    gi::Judgment {
        judge: judge.into(),
        identity: DoorIdentity::hosted(model),
        set: SET.id.clone(),
        supported,
        embellished,
        supported_at: threshold(SUPPORTED),
        embellished_at: threshold(EMBELLISHED),
    }
}

const STOP: &[&str] = &[
    "the", "and", "that", "this", "with", "for", "from", "was", "were", "are", "not", "but", "you",
    "your", "its", "it's", "has", "have", "had", "which", "what", "when", "who", "how", "why",
    "into", "then", "than", "them", "they", "their", "there", "would", "will", "can", "own", "out",
    "about", "after", "before", "each", "every", "one", "all", "any", "some", "said", "say",
    "says", "does", "did", "doesn", "don", "didn", "won", "isn", "hasn", "also", "only", "just",
    "rather", "instead", "too", "yet",
];

fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .map(|w| w.trim_matches('-').to_ascii_lowercase())
        .filter(|w| w.len() >= 3 && !STOP.contains(&w.as_str()))
        .collect()
}

/// A deterministic judge with no model, for scripted runs and tests.
///
/// Support is the share of the reference answer's content words the
/// answer holds. Embellishment is the share of the answer's content words
/// that appear in none of the cited records, the reference answer, the
/// question, or the briefing, doubled and capped at 1. An answer that says
/// it doesn't remember is neither. It is a stand-in, not a measurement:
/// only Jev's readings, calibrated by the owner's marks, count toward the
/// gate's embellishment rate in a real run.
#[derive(Clone, Copy, Debug, Default)]
pub struct ScriptedJudge;

impl ScriptedJudge {
    /// The identity rows record.
    pub const MODEL: &'static str = "overlap-judge-v1";
}

#[allow(clippy::cast_precision_loss)]
impl Judge for ScriptedJudge {
    fn judge(&mut self, case: &Case<'_>) -> Result<gi::Judgment, String> {
        let answer = words(case.answer);
        if answer.is_empty() || case.answer.trim() == DONT_REMEMBER {
            return Ok(judgment("scripted", Self::MODEL, 0.0, 0.0));
        }
        let reference = words(&case.item.truth);
        let supported = if reference.is_empty() {
            0.0
        } else {
            reference.intersection(&answer).count() as f64 / reference.len() as f64
        };
        let mut known = words(&case.item.truth);
        known.extend(words(gi::question(case.item)));
        known.extend(words(&case.briefing.text));
        for record in case.evidence {
            known.extend(words(&record.text));
        }
        let unknown = answer.difference(&known).count() as f64 / answer.len() as f64;
        Ok(judgment(
            "scripted",
            Self::MODEL,
            supported,
            (2.0 * unknown).min(1.0),
        ))
    }
}

/// Jev answers `questions/interview-answer.json`, on a runtime of its own.
pub struct JevJudge {
    client: jev::Client,
    runtime: tokio::runtime::Runtime,
}

impl JevJudge {
    /// Jev from the decision profile.
    ///
    /// # Errors
    /// When Jev isn't set up or no runtime starts.
    pub fn from_env() -> Result<Self, String> {
        let client = crate::decision::from_env()
            .map_err(|e| format!("Jev: {e}"))?
            .ok_or("Jev isn't set up, so interview answers can't be judged")?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| format!("cannot start a runtime: {e}"))?;
        Ok(Self { client, runtime })
    }
}

impl Judge for JevJudge {
    fn judge(&mut self, case: &Case<'_>) -> Result<gi::Judgment, String> {
        let request = request(case)?;
        let response = self
            .runtime
            .block_on(self.client.system_one(request))
            .map_err(|e| format!("Jev: {e}"))?;
        let noul = |id: &str| match response.answers.get(id) {
            Some(jev::Answer::Noul(answer)) => Ok(answer.noul),
            _ => Err(format!("Jev didn't answer `{id}` in {}", SET.id)),
        };
        Ok(judgment(
            "jev",
            &response.model,
            noul(SUPPORTED)?,
            noul(EMBELLISHED)?,
        ))
    }
}
