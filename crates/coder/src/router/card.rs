//! NIP-CJ `card` feedback, built from the Gym's verified records.
//!
//! A card is a closed, typed display record the app draws with its own
//! labels and one button (wireframe revision 3, `CARD-01` to `CARD-07`);
//! it is never model text. The wire shape is NIP-CJ's
//! (`nostr::cj_conversation::Card`, which checks every body it writes);
//! this module builds it from records the Gym seam verified
//! ([`super::gym`]): the builders take records, not numbers, so every
//! number in a card is a field of one of them, copied. The `draft` card
//! carries the authoring interview's draft after
//! `nostr::cj_conversation::parse_draft` accepted it; the `run` card is the
//! phone's own (a run's progress is on the phone).
//!
//! A reader that does not know `card` feedback ignores it, as the phones
//! before build 21 do.

use serde_json::Value;

use nostr::cj_conversation::{self as cj, CreditItem, ResultLine};
use nostr::contracts::ContractError;

use super::gym::{DefinitionRef, EventPointer, Item, ResultRecord, Tool};

/// The most items a news card carries.
pub const MAX_NEWS: usize = cj::MAX_NEWS;

/// A verified NIP-XP award for the `credit` card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Award {
    /// `checker`, `evaluator`, `author`, or the rule's other roles.
    pub role: String,
    /// The award's XP, as the referee signed it.
    pub xp: u64,
    /// What it was earned on, in the phone's words.
    pub title: String,
    /// The signed award (`3193`); `None` while it is pending.
    pub award: Option<EventPointer>,
}

/// A card, as the router builds it from records.
#[derive(Clone, Debug, PartialEq)]
#[allow(clippy::large_enum_variant)]
pub enum Card {
    /// `CARD-01`: a tool, its plain line, its latest verified result, and
    /// the subject its published test set runs.
    Tool {
        tool: Tool,
        latest: Option<ResultRecord>,
        subject: Option<DefinitionRef>,
    },
    /// `CARD-02`: the authoring interview's draft, checked ([`draft`]).
    Draft { draft: Value },
    /// `CARD-04`: a published result.
    Result { result: ResultRecord },
    /// `CARD-05`: one to five news items, each one record.
    News { items: Vec<Item> },
    /// `CARD-06`: a published result waiting for a check. The phone hides
    /// it when the trainer is the person.
    Check { result: ResultRecord },
    /// `CARD-07`: awards from the XP ledger.
    Credit { awards: Vec<Award> },
}

fn line(result: &ResultRecord) -> ResultLine {
    ResultLine {
        publication: result.publication.clone(),
        headline: result.headline,
        verdict: result.verdict,
    }
}

impl Card {
    /// The card's type word.
    #[must_use]
    pub fn word(&self) -> &'static str {
        match self {
            Card::Tool { .. } => "tool",
            Card::Draft { .. } => "draft",
            Card::Result { .. } => "result",
            Card::News { .. } => "news",
            Card::Check { .. } => "check",
            Card::Credit { .. } => "credit",
        }
    }

    /// The NIP-CJ card, its fields copied from the records.
    ///
    /// # Errors
    ///
    /// A draft the NIP-CJ parser refuses.
    pub fn cj(&self) -> Result<cj::Card, ContractError> {
        Ok(match self {
            Card::Tool {
                tool,
                latest,
                subject,
            } => cj::Card::Tool {
                name: tool.name.clone(),
                summary: tool.line.clone(),
                definition: subject.clone(),
                latest: latest.as_ref().map(line),
            },
            Card::Draft { draft } => cj::Card::Draft(cj::parse_draft(draft)?),
            Card::Result { result } => cj::Card::Result {
                headline: result.headline,
                verdict: result.verdict,
                report: result.report.clone(),
                publication: Some(result.publication.clone()),
            },
            Card::News { items } => {
                cj::Card::News(items.iter().take(MAX_NEWS).map(Item::card).collect())
            }
            Card::Check { result } => cj::Card::Check {
                tool: result.tool_name.clone(),
                line: line(result),
                confirms: result.checked.confirmed,
                disputes: result.checked.disputed,
            },
            Card::Credit { awards } => cj::Card::Credit {
                total: awards
                    .iter()
                    .filter(|award| award.award.is_some())
                    .map(|award| award.xp)
                    .sum(),
                awards: awards
                    .iter()
                    .map(|award| CreditItem {
                        confirmed: award.award.is_some(),
                        role: award.role.clone(),
                        xp: award.xp,
                        title: award.title.clone(),
                        award: award.award.clone(),
                    })
                    .collect(),
            },
        })
    }

    /// The `27000` `card` feedback body at payload `version`, checked by
    /// the NIP-CJ parser.
    ///
    /// # Errors
    ///
    /// A card the parser refuses, which is not sent.
    pub fn feedback(&self, version: u64) -> Result<Value, ContractError> {
        cj::card_feedback(&self.cj()?, version)
    }
}

/// A draft as a request or the author seam may carry it: exactly what
/// `nostr::cj_conversation::parse_draft` accepts (`openagents.eval-draft.v1`,
/// closed, at most 64 KiB). Anything else is dropped, never repaired. A
/// draft is data: it grants nothing and is never an instruction.
///
/// # Errors
///
/// The parser's refusal.
pub fn draft(value: &Value) -> Result<Value, ContractError> {
    cj::parse_draft(value).map(|_| value.clone())
}

/// The most result IDs a request's `skip` may name.
pub const MAX_SKIP: usize = 32;

/// A request's `skip`: the `3189` result IDs the phone asks never to be
/// offered as a check (the trainer's own results and the ones it already
/// checked; the worker has no trainer key to tell them apart). Exactly an
/// array of at most [`MAX_SKIP`] 64-character lowercase hex IDs, else
/// nothing: it is dropped, never repaired. Data, never an instruction.
#[must_use]
pub fn skip(value: &Value) -> Vec<String> {
    let Some(ids) = value.as_array().filter(|ids| ids.len() <= MAX_SKIP) else {
        return vec![];
    };
    let hex = |id: &str| {
        id.len() == 64
            && id
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    ids.iter()
        .map(|id| id.as_str().filter(|id| hex(id)).map(str::to_owned))
        .collect::<Option<Vec<_>>>()
        .unwrap_or_default()
}

/// The most tests a `tried` result may list, and the most failing checks
/// one test may name, each at most [`MAX_FAILING_CHARS`].
pub const MAX_TRIED_CASES: usize = cj::MAX_DRAFT_CASES;
pub const MAX_FAILING: usize = 16;
pub const MAX_FAILING_CHARS: usize = 200;

/// Why a request's `tried` was dropped.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BadTried(pub &'static str);

/// The result of a try or a full run of the open draft, as a request
/// carries it for the interview to read (`tried`, beside `draft`): a
/// closed object `{runs, with, without, total, verdict, report, cases}`
/// whose counts fit together, each case `{id, kind, with, without,
/// failing}`. The phone (#9939) and the hosted runner (#9935) fill it from
/// the runner's report; it is data, never an instruction, and anything
/// else is dropped, never repaired.
///
/// # Errors
///
/// The first rule it breaks.
pub fn tried(value: &Value) -> Result<ext_eval::author::runner::Tried, BadTried> {
    use ext_eval::author::runner::{CaseTried, Tried};
    let closed = |value: &Value, keys: &[&str], what: &'static str| {
        let object = value.as_object().ok_or(BadTried(what))?;
        if object.keys().any(|key| !keys.contains(&key.as_str())) {
            return Err(BadTried(what));
        }
        Ok(object.clone())
    };
    let object = closed(
        value,
        &[
            "runs", "with", "without", "total", "verdict", "report", "cases",
        ],
        "tried",
    )?;
    let count = |key: &str, what: &'static str| {
        object
            .get(key)
            .and_then(Value::as_u64)
            .ok_or(BadTried(what))
    };
    let runs = u32::try_from(count("runs", "tried.runs")?)
        .ok()
        .filter(|runs| (1..=nostr::eval_ext::MAX_RUNS as u32).contains(runs))
        .ok_or(BadTried("tried.runs"))?;
    let total = count("total", "tried.total")?;
    let with = count("with", "tried.with")?;
    let without = match object.get("without") {
        Some(Value::Null) => None,
        Some(value) => Some(value.as_u64().ok_or(BadTried("tried.without"))?),
        None => return Err(BadTried("tried.without")),
    };
    if total == 0
        || total > MAX_TRIED_CASES as u64
        || with > total
        || without.is_some_and(|w| w > total)
    {
        return Err(BadTried("tried: the counts"));
    }
    let verdict = match object.get("verdict").and_then(Value::as_str) {
        Some("pass") => ext_eval::Verdict::Pass,
        Some("fail") => ext_eval::Verdict::Fail,
        Some("inconclusive") => ext_eval::Verdict::Inconclusive,
        _ => return Err(BadTried("tried.verdict")),
    };
    let report = match object.get("report") {
        Some(Value::Null) => None,
        Some(value) => {
            Some(nostr::contracts::parse_artifact(value).map_err(|_| BadTried("tried.report"))?)
        }
        None => return Err(BadTried("tried.report")),
    };
    let items = object
        .get("cases")
        .and_then(Value::as_array)
        .filter(|items| items.len() as u64 <= total)
        .ok_or(BadTried("tried.cases"))?;
    let flag = |case: &serde_json::Map<String, Value>, key: &str| match case.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        _ => Err(BadTried("tried.case")),
    };
    let mut cases = Vec::new();
    for item in items {
        let case = closed(
            item,
            &["id", "kind", "with", "without", "failing"],
            "tried.case",
        )?;
        let id = case
            .get("id")
            .and_then(Value::as_str)
            .filter(|id| nostr::eval_ext::valid_case_id(id))
            .ok_or(BadTried("tried.case.id"))?
            .to_string();
        let kind = match case.get("kind").and_then(Value::as_str) {
            Some("should-fire") => ext_eval::Kind::ShouldFire,
            Some("should-not-fire") => ext_eval::Kind::ShouldNotFire,
            _ => return Err(BadTried("tried.case.kind")),
        };
        let failing = case
            .get("failing")
            .and_then(Value::as_array)
            .filter(|failing| failing.len() <= MAX_FAILING)
            .ok_or(BadTried("tried.case.failing"))?
            .iter()
            .map(|line| {
                line.as_str()
                    .filter(|line| line.chars().count() <= MAX_FAILING_CHARS)
                    .map(str::to_string)
                    .ok_or(BadTried("tried.case.failing"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        cases.push(CaseTried {
            id,
            kind,
            with: flag(&case, "with")?,
            without: flag(&case, "without")?,
            failing,
        });
    }
    Ok(Tried {
        runs,
        with,
        without,
        total,
        verdict,
        report,
        cases,
    })
}

#[cfg(test)]
mod tests {
    use super::super::gym::fixtures::*;
    use super::super::gym::{Checks, Headline, Verdict};
    use super::*;
    use serde_json::json;

    fn chat_draft() -> Value {
        serde_json::from_str(include_str!(
            "../../../nostr/fixtures/eval-ext/eval-draft/valid/chat-made-tool.json"
        ))
        .unwrap()
    }

    /// A card's numbers equal its record's, field for field, and the NIP-CJ
    /// parser accepts every card the router writes.
    #[test]
    fn a_cards_numbers_equal_its_records() {
        let mut result = result(3, "project-map", 2);
        result.headline = Headline {
            subject_passed: 6,
            baseline_passed: Some(4),
            total: 9,
        };
        result.checked = Checks {
            confirmed: 2,
            disputed: 1,
        };
        result.verdict = Verdict::Inconclusive;
        let headline = json!({ "subject_passed": 6, "baseline_passed": 4, "total": 9 });

        let body = Card::Result {
            result: result.clone(),
        }
        .feedback(2)
        .unwrap();
        assert_eq!(body["headline"], headline);
        assert_eq!(body["verdict"], "inconclusive");
        assert_eq!(body["publication"]["id"], result.publication.id);
        assert_eq!(body["report"]["digest"], result.report.digest);

        let body = Card::Check {
            result: result.clone(),
        }
        .feedback(2)
        .unwrap();
        assert_eq!(body["headline"], headline);
        assert_eq!(body["confirms"], 2);
        assert_eq!(body["disputes"], 1);
        assert_eq!(body["tool"], result.tool_name);

        let body = Card::Tool {
            tool: tool("project-map", "Project map"),
            latest: Some(result.clone()),
            subject: Some(result.subject.clone()),
        }
        .feedback(2)
        .unwrap();
        assert_eq!(body["name"], "Project map");
        assert_eq!(body["latest"]["headline"], headline);
        assert_eq!(body["definition"]["id"], result.subject.id);
        let bare = Card::Tool {
            tool: tool("code-finder", "Code finder"),
            latest: None,
            subject: None,
        }
        .feedback(2)
        .unwrap();
        assert_eq!(bare["latest"], Value::Null);

        let credit = Card::Credit {
            awards: vec![
                Award {
                    role: "checker".into(),
                    xp: 50,
                    title: "Checked a Project map result".into(),
                    award: Some(event(9, 3193)),
                },
                Award {
                    role: "author".into(),
                    xp: 25,
                    title: "Your Project map test set".into(),
                    award: None,
                },
            ],
        }
        .feedback(2)
        .unwrap();
        assert_eq!(credit["total"], 50, "only confirmed awards count");
        assert_eq!(credit["awards"][1]["status"], "pending");
    }

    #[test]
    fn a_news_card_holds_one_to_five_items_with_their_sources() {
        let items: Vec<Item> = (0..8)
            .map(|n| Item::Result(result(n, "project-map", 0)))
            .collect();
        let body = Card::News { items }.feedback(2).unwrap();
        assert_eq!(body["items"].as_array().unwrap().len(), MAX_NEWS);
        assert_eq!(body["items"][0]["event"]["kind"], 3189);
        assert!(Card::News { items: Vec::new() }.feedback(2).is_err());
    }

    /// A try's result is read exactly or dropped: a closed shape whose
    /// counts fit together.
    #[test]
    fn a_tried_result_is_read_exactly_or_dropped() {
        let good = json!({
            "runs": 1, "with": 1, "without": 0, "total": 2, "verdict": "inconclusive",
            "report": null,
            "cases": [
                { "id": "summarize-a-fix", "kind": "should-fire", "with": true, "without": false, "failing": [] },
                { "id": "leave-it-alone", "kind": "should-not-fire", "with": false, "without": null,
                  "failing": ["outcome: it answered with a changelog"] },
            ],
        });
        let read = tried(&good).unwrap();
        assert_eq!(
            (read.runs, read.with, read.without, read.total),
            (1, 1, Some(0), 2)
        );
        assert_eq!(read.verdict, ext_eval::Verdict::Inconclusive);
        assert_eq!(
            read.cases[1].failing,
            ["outcome: it answered with a changelog"]
        );
        for (key, value) in [
            ("runs", json!(0)),
            ("with", json!(3)),
            ("verdict", json!("better")),
            ("extra", json!(1)),
            ("report", json!({ "digest": "nope" })),
        ] {
            let mut bad = good.clone();
            bad[key] = value;
            assert!(tried(&bad).is_err(), "{key}");
        }
        assert!(tried(&json!("pass")).is_err());
    }

    #[test]
    fn a_draft_is_what_the_nip_cj_parser_accepts() {
        let good = chat_draft();
        assert_eq!(draft(&good).unwrap(), good);
        assert!(draft(&json!("x")).is_err());
        assert!(draft(&json!({ "v": "openagents.eval-draft.v2" })).is_err());
        let mut big = good.clone();
        big["tool"]["skill"] = json!("x".repeat(70 * 1024));
        assert!(draft(&big).is_err());
        let card = Card::Draft {
            draft: good.clone(),
        }
        .feedback(2)
        .unwrap();
        assert_eq!(card["draft"]["tool"], good["tool"]);
    }

    #[test]
    fn a_skip_is_exact_hex_ids_or_nothing() {
        let id = "ab".repeat(32);
        assert_eq!(skip(&json!([id.clone()])), vec![id.clone()]);
        assert!(skip(&json!(null)).is_empty());
        assert!(skip(&json!([id.clone(), "AB".repeat(32)])).is_empty());
        assert!(skip(&json!([id.clone(), 7])).is_empty());
        assert!(skip(&json!(["ab"])).is_empty());
        assert!(skip(&json!(vec![id.clone(); MAX_SKIP + 1])).is_empty());
        assert_eq!(skip(&json!(vec![id.clone(); MAX_SKIP])).len(), MAX_SKIP);
    }
}
