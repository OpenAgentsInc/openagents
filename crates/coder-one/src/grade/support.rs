//! The support question: does a check's expected result follow from the
//! task's words, from the task's own program as the host ran it, or from
//! the standard definition of a named method?
//!
//! `accept.grade` ([`crate::grade`], issue #9635) asks it of each line of
//! the lean loop's session-written score script at the freeze. Each basis
//! is its own Noul over the same state ([`SUPPORT_SET`]), so a caller
//! reads the one that applies, and code, not Jev, turns the three
//! probabilities into a grade.
//!
//! It sits beside `accept.define`'s authority classes
//! ([`crate::accept::authority`], issue #9629) rather than inside them. A
//! class is set for a test in a frozen suite from its run on the untouched
//! workspace and two Jev questions about where its expected values come
//! from; a check line here is never run on the untouched workspace apart
//! from the script, and its grade names the basis its expectation rests
//! on. The support question's `task` and `standard` Nouls ask what the
//! classes' `expected_correct` question asks as one, split so the record
//! says which basis held.

use std::collections::BTreeMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::component::jev::{self as jev_component, JevMode};
use crate::record::Recorder;

/// Where a supported expectation comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// The task's own words.
    Task,
    /// What the task's own program printed on the untouched workspace
    /// (`evidence.baseline`, #9633).
    Baseline,
    /// The standard definition of a method the check or the task names.
    Standard,
}

impl Basis {
    /// Every basis, in the order ties resolve: the task's words first.
    pub const ALL: [Basis; 3] = [Basis::Task, Basis::Baseline, Basis::Standard];

    /// The basis as records and question IDs spell it.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Basis::Task => "task",
            Basis::Baseline => "baseline",
            Basis::Standard => "standard",
        }
    }
}

/// The support question set's file text.
pub const SUPPORT_TEXT: &str = include_str!("../../../../questions/expectation-support.json");

/// The support question set's ID.
pub const SUPPORT_SET: &str = "openagents.expectation-support.v1";

/// What the instructions write the item's state path into.
pub const ITEM: &str = "{item}";

/// The support question set as the repository holds it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SupportSet {
    pub id: String,
    /// `atif::digest` of `{"questions": …}`, the digest the question-set
    /// registry in `crates/coder` computes.
    pub digest: String,
    /// Each basis's instructions, with [`ITEM`] where the item's state
    /// path goes.
    pub instructions: BTreeMap<Basis, String>,
}

/// The support question set.
#[must_use]
pub fn support_set() -> &'static SupportSet {
    static SET: OnceLock<SupportSet> = OnceLock::new();
    SET.get_or_init(|| {
        let value: Value =
            serde_json::from_str(SUPPORT_TEXT).expect("expectation-support.json is JSON");
        let questions = value["questions"].clone();
        let instructions = Basis::ALL
            .iter()
            .map(|basis| {
                let text = questions[basis.word()]["instructions"]
                    .as_str()
                    .expect("each basis has instructions")
                    .to_string();
                (*basis, text)
            })
            .collect();
        SupportSet {
            id: value["id"].as_str().unwrap_or_default().to_string(),
            digest: atif::digest(&json!({ "questions": questions })),
            instructions,
        }
    })
}

/// The question for one basis of the item at state path `path`.
#[must_use]
pub fn question(basis: Basis, path: &str) -> String {
    support_set().instructions[&basis].replace(ITEM, path)
}

/// One check or test to ask about.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub id: String,
    /// The check as written.
    pub text: String,
    /// The lines before it that set up its inputs.
    pub context: String,
}

/// Jev's answers for one item, `None` where it didn't answer or wasn't
/// asked.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Support {
    pub task: Option<f64>,
    pub baseline: Option<f64>,
    pub standard: Option<f64>,
    /// How the answers came: `live`, `recorded`, `miss`, `off`,
    /// `failed`, or `skipped`.
    pub how: String,
    pub error: Option<String>,
}

impl Support {
    /// The probability for `basis`.
    #[must_use]
    pub fn p(&self, basis: Basis) -> Option<f64> {
        match basis {
            Basis::Task => self.task,
            Basis::Baseline => self.baseline,
            Basis::Standard => self.standard,
        }
    }

    /// The best-supported basis and its probability, or `None` when Jev
    /// answered none of them. Ties go to the earlier basis in
    /// [`Basis::ALL`].
    #[must_use]
    pub fn best(&self) -> Option<(Basis, f64)> {
        let mut best: Option<(Basis, f64)> = None;
        for basis in Basis::ALL {
            if let Some(p) = self.p(basis)
                && best.is_none_or(|(_, q)| p > q)
            {
                best = Some((basis, p));
            }
        }
        best
    }
}

/// Items per request, at most.
pub const BATCH: usize = 6;

/// The most characters of an item's text or context Jev reads.
pub const ITEM_CHARS: usize = 1_200;

/// The most characters of the task Jev reads.
pub const TASK_CHARS: usize = 6_000;

/// The most characters of baseline output Jev reads.
pub const BASELINE_CHARS: usize = 4_000;

/// Where the questions are asked from.
pub struct Context<'a> {
    pub component: &'a str,
    /// The Jev decision's name.
    pub name: &'a str,
    /// The call ID prefix.
    pub id: String,
    pub deadline: Option<crate::deadline::Deadline>,
}

/// What asking produced: one [`Support`] per item, in order, what it cost,
/// and one record per request.
#[derive(Clone, Debug, Default)]
pub struct Asked {
    pub supports: Vec<Support>,
    pub usd: f64,
    pub calls: Vec<Value>,
}

/// One request's state and questions for `items`, with the question ID
/// of each basis for each item. Without `baseline`, the baseline question
/// isn't asked.
#[must_use]
pub fn request(
    task: &str,
    baseline: Option<&str>,
    items: &[Item],
) -> (Value, ::jev::Questions, Vec<Vec<(Basis, String)>>) {
    let mut questions = ::jev::Questions::new();
    let mut ids = Vec::new();
    for j in 0..items.len() {
        let path = format!("checks[{j}]");
        let mut mine = Vec::new();
        for basis in Basis::ALL {
            if basis == Basis::Baseline && baseline.is_none() {
                continue;
            }
            let id = format!("{}_{j}", basis.word());
            questions = questions.with(
                id.clone(),
                ::jev::Noul::new(question(basis, &path).as_str()),
            );
            mine.push((basis, id));
        }
        ids.push(mine);
    }
    let mut state = serde_json::Map::new();
    state.insert(
        "task".to_string(),
        json!(crate::judge::clip(task.trim(), TASK_CHARS)),
    );
    if let Some(baseline) = baseline {
        state.insert(
            "baseline".to_string(),
            json!(crate::judge::clip(baseline.trim(), BASELINE_CHARS)),
        );
    }
    state.insert(
        "checks".to_string(),
        json!(
            items
                .iter()
                .map(|item| json!({
                    "text": crate::judge::clip(&item.text, ITEM_CHARS),
                    "context": crate::judge::clip(&item.context, ITEM_CHARS),
                }))
                .collect::<Vec<_>>()
        ),
    );
    (Value::Object(state), questions, ids)
}

/// Asks the support question of every item, [`BATCH`] at a time.
pub async fn ask(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    task: &str,
    baseline: Option<&str>,
    items: &[Item],
) -> Asked {
    let mut asked = Asked::default();
    for (n, chunk) in items.chunks(BATCH).enumerate() {
        let (state, questions, ids) = request(task, baseline, chunk);
        let answer = jev_component::ask(
            jev,
            recorder,
            jev_component::Ask {
                component: context.component,
                name: context.name,
                id: format!("{}-{n}", context.id),
                state,
                questions,
                parent: None,
                deadline: context.deadline.clone(),
            },
        )
        .await;
        asked.usd += answer.input_tokens.map_or(0.0, |t| {
            t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
        });
        asked.calls.push(json!({
            "how": answer.how,
            "error": answer.error,
            "input_tokens": answer.input_tokens,
        }));
        for mine in ids {
            let mut support = Support {
                how: answer.how.to_string(),
                error: answer.error.clone(),
                ..Support::default()
            };
            for (basis, id) in mine {
                let p = answer.noul(&id);
                match basis {
                    Basis::Task => support.task = p,
                    Basis::Baseline => support.baseline = p,
                    Basis::Standard => support.standard = p,
                }
            }
            asked.supports.push(support);
        }
    }
    asked
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_set_reads_and_names_every_basis() {
        let set = support_set();
        assert_eq!(set.id, SUPPORT_SET);
        assert_eq!(set.digest.len(), 64);
        for basis in Basis::ALL {
            let q = question(basis, "checks[2]");
            assert!(q.contains("`checks[2].text`"), "{q}");
            assert!(!q.contains(ITEM));
        }
    }

    #[test]
    fn without_a_baseline_the_baseline_question_is_not_asked() {
        let item = Item {
            id: "c1".to_string(),
            text: "check(mmd(x, x) == 0)".to_string(),
            context: "x = np.zeros((3, 2))".to_string(),
        };
        let (state, questions, ids) =
            request("Fix the monitor.", None, std::slice::from_ref(&item));
        assert!(state.get("baseline").is_none());
        let body = serde_json::to_value(&questions).unwrap_or_default();
        assert!(body.get("task_0").is_some());
        assert!(body.get("standard_0").is_some());
        assert!(body.get("baseline_0").is_none());
        assert_eq!(ids[0].len(), 2);
        let (state, _, ids) = request("Fix the monitor.", Some("exit 0"), &[item]);
        assert_eq!(state["baseline"], "exit 0");
        assert_eq!(ids[0].len(), 3);
    }

    #[test]
    fn the_best_basis_prefers_the_task_on_a_tie() {
        let support = Support {
            task: Some(0.7),
            baseline: None,
            standard: Some(0.7),
            ..Support::default()
        };
        assert_eq!(support.best(), Some((Basis::Task, 0.7)));
        assert_eq!(Support::default().best(), None);
    }
}
