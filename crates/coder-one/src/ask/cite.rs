//! Checks an answer's citations against the Gym before anyone reads it.
//!
//! Every claim names the runs it rests on, and may name transcript steps,
//! Jev judgments, and repository files. Code checks each:
//!
//! - a run exists, named exactly as `job/trial` or by a job name that has
//!   one trial;
//! - a step exists in that run's transcript;
//! - a judgment is at or above [`REASON_AT`] in that run's stored answer,
//!   for every run the claim cites;
//! - a file exists under the repository, and a cited line is within it;
//! - a person's mark exists on a cited run, or on a cited step of it, in
//!   the Gym's marks store;
//! - a run card row exists on the cited run's card, with a known value.
//!
//! A claim with a citation that doesn't check, or with no citation at all,
//! is marked unverified. It's kept: a person decides what to make of it.

use std::collections::BTreeMap;
use std::path::Path;

use serde_json::{Value, json};

/// The threshold a cited judgment must meet, the Gym's `REASON_AT`.
pub const REASON_AT: f64 = 0.5;

/// What the Gym says about one run, from `gym runs show RUN --json`.
#[derive(Clone, Debug, PartialEq)]
pub struct RunFacts {
    /// `job/trial`.
    pub id: String,
    pub job: String,
    /// The task the run attempted, such as `cad-model`.
    pub task: String,
    /// How many transcript steps the run has.
    pub steps: usize,
    /// Each judgment's probability, when Jev judged the run.
    pub judgments: Option<BTreeMap<String, f64>>,
    /// A person's marks on the run: the step, or `None` for the whole run,
    /// and the verdict, `bad` or `clear`.
    pub marks: Vec<(Option<u64>, String)>,
    /// The run card's rows by ID, with the value as the card prints it, or
    /// `None` when the Gym computed no card. An unknown row is absent.
    pub card: Option<BTreeMap<String, String>>,
}

impl RunFacts {
    /// Reads `gym runs show RUN --json`.
    #[must_use]
    pub fn from_show(shown: &Value) -> Option<Self> {
        let job = shown.pointer("/run/job")?.as_str()?.to_string();
        let trial = shown.pointer("/run/trial")?.as_str()?;
        let judgments = shown["learning"]["judgments"].as_object().map(|map| {
            map.iter()
                .filter_map(|(id, p)| Some((id.clone(), p.as_f64()?)))
                .collect()
        });
        Some(RunFacts {
            id: format!("{job}/{trial}"),
            job,
            task: shown
                .pointer("/run/task")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            steps: shown["transcript"].as_array().map_or(0, Vec::len),
            judgments,
            marks: shown["marks"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|mark| {
                    (
                        mark["step"].as_u64(),
                        mark["verdict"].as_str().unwrap_or_default().to_string(),
                    )
                })
                .collect(),
            card: shown["card"]["rows"].as_array().map(|rows| {
                rows.iter()
                    .filter(|row| !row["value"].is_null())
                    .filter_map(|row| {
                        Some((
                            row["id"].as_str()?.to_string(),
                            row["text"].as_str().unwrap_or_default().to_string(),
                        ))
                    })
                    .collect()
            }),
        })
    }
}

/// One claim, checked.
#[derive(Clone, Debug, PartialEq)]
pub struct Claim {
    pub text: String,
    pub runs: Vec<String>,
    pub steps: Vec<(String, u64)>,
    pub judgments: Vec<String>,
    pub files: Vec<String>,
    /// Marks it rests on: `job/trial` for a run's mark, `job/trial/STEP`
    /// for a step's.
    pub marks: Vec<String>,
    /// For a highlights ask, the key of the highlight the claim drafts.
    pub highlight: Option<String>,
    /// Run card rows it rests on: the run and the row's ID.
    pub cards: Vec<(String, String)>,
    /// Why a citation didn't check, one line each.
    pub problems: Vec<String>,
    /// Citations checked, and how many held.
    pub citations: usize,
    pub valid: usize,
}

impl Claim {
    /// Whether every citation held and there was at least one.
    #[must_use]
    pub fn verified(&self) -> bool {
        self.citations > 0 && self.problems.is_empty()
    }

    /// The claim as the record and `--json` carry it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "claim": self.text,
            "runs": self.runs,
            "steps": self.steps.iter().map(|(run, step)| json!({"run": run, "step": step})).collect::<Vec<_>>(),
            "judgments": self.judgments,
            "files": self.files,
            "marks": self.marks,
            "highlight": self.highlight,
            "card_rows": self.cards.iter().map(|(run, row)| json!({"run": run, "row": row})).collect::<Vec<_>>(),
            "verified": self.verified(),
            "problems": self.problems,
            "citations": self.citations,
            "valid_citations": self.valid,
        })
    }
}

/// The answer's claims, read from the `answer` tool's arguments.
#[must_use]
pub fn claims(answer: &Value) -> Vec<Claim> {
    let strings = |value: &Value| -> Vec<String> {
        value
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    };
    answer["claims"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|claim| Claim {
            text: claim["claim"].as_str().unwrap_or_default().to_string(),
            runs: strings(&claim["runs"]),
            steps: claim["steps"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|s| Some((s["run"].as_str()?.trim().to_string(), s["step"].as_u64()?)))
                .collect(),
            judgments: strings(&claim["judgments"]),
            files: strings(&claim["files"]),
            marks: strings(&claim["marks"]),
            highlight: claim["highlight"]
                .as_str()
                .map(|key| key.trim().to_string())
                .filter(|key| !key.is_empty()),
            cards: claim["card_rows"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|c| {
                    Some((
                        c["run"].as_str()?.trim().to_string(),
                        c["row"].as_str()?.trim().to_string(),
                    ))
                })
                .collect(),
            problems: Vec::new(),
            citations: 0,
            valid: 0,
        })
        .collect()
}

/// Every run the claims cite, directly or through a step.
#[must_use]
pub fn cited_runs(claims: &[Claim]) -> Vec<String> {
    let mut runs: Vec<String> = claims
        .iter()
        .flat_map(|c| {
            c.runs
                .iter()
                .cloned()
                .chain(c.steps.iter().map(|(run, _)| run.clone()))
                .chain(c.marks.iter().map(|mark| split_mark(mark).0.to_string()))
                .chain(c.cards.iter().map(|(run, _)| run.clone()))
        })
        .collect();
    runs.sort();
    runs.dedup();
    runs
}

/// Checks every claim. `facts` maps a cited run name to what `gym runs
/// show` found for it, or `None` when the Gym found nothing.
pub fn check(claims: &mut [Claim], facts: &BTreeMap<String, Option<RunFacts>>, repo: &Path) {
    for claim in claims.iter_mut() {
        let mut problems = Vec::new();
        let mut citations = 0;
        let mut valid = 0;
        let mut found: Vec<&RunFacts> = Vec::new();
        for run in &claim.runs {
            citations += 1;
            match resolve(run, facts) {
                Ok(fact) => {
                    valid += 1;
                    found.push(fact);
                }
                Err(why) => problems.push(why),
            }
        }
        for (run, step) in &claim.steps {
            citations += 1;
            match resolve(run, facts) {
                Ok(fact) if *step >= 1 && (*step as usize) <= fact.steps => valid += 1,
                Ok(fact) => problems.push(format!(
                    "{run} has {} transcript steps; step {step} isn't one",
                    fact.steps
                )),
                Err(why) => problems.push(why),
            }
        }
        for id in &claim.judgments {
            citations += 1;
            if found.is_empty() {
                problems.push(format!("{id} is cited with no run to check it against"));
                continue;
            }
            let misses: Vec<String> = found
                .iter()
                .filter_map(|fact| match &fact.judgments {
                    None => Some(format!("{} has no Jev judgment", fact.id)),
                    Some(map) => match map.get(id) {
                        None => Some(format!("{id} isn't a judgment Jev made for {}", fact.id)),
                        Some(p) if *p < REASON_AT => Some(format!(
                            "{id} is {p:.2} for {}, under {REASON_AT:.2}",
                            fact.id
                        )),
                        Some(_) => None,
                    },
                })
                .collect();
            if misses.is_empty() {
                valid += 1;
            } else {
                problems.extend(misses);
            }
        }
        for mark in &claim.marks {
            citations += 1;
            let (run, step) = split_mark(mark);
            match resolve(run, facts) {
                Ok(fact) => match fact.marks.iter().find(|(at, _)| *at == step) {
                    Some(_) => valid += 1,
                    None => problems.push(match step {
                        Some(step) => format!("no person marked step {step} of {run}"),
                        None => format!("no person marked {run}"),
                    }),
                },
                Err(why) => problems.push(why),
            }
        }
        for (run, row) in &claim.cards {
            citations += 1;
            match resolve(run, facts) {
                Ok(fact) => match fact.card.as_ref().map(|card| card.contains_key(row)) {
                    Some(true) => valid += 1,
                    Some(false) => {
                        problems.push(format!("{row} isn't a known row on {}'s run card", fact.id))
                    }
                    None => problems.push(format!("the Gym has no run card for {}", fact.id)),
                },
                Err(why) => problems.push(why),
            }
        }
        for file in &claim.files {
            citations += 1;
            match check_file(file, repo) {
                Ok(()) => valid += 1,
                Err(why) => problems.push(why),
            }
        }
        if citations == 0 {
            problems.push("the claim cites nothing".to_string());
        }
        claim.problems = problems;
        claim.citations = citations;
        claim.valid = valid;
    }
}

/// A mark citation's run and step: `job/trial` or `job/trial/STEP`.
#[must_use]
pub fn split_mark(text: &str) -> (&str, Option<u64>) {
    match text.rsplit_once('/') {
        Some((run, step))
            if run.contains('/')
                && !step.is_empty()
                && step.bytes().all(|b| b.is_ascii_digit()) =>
        {
            (run, step.parse().ok())
        }
        _ => (text, None),
    }
}

fn resolve<'a>(
    run: &str,
    facts: &'a BTreeMap<String, Option<RunFacts>>,
) -> Result<&'a RunFacts, String> {
    match facts.get(run) {
        Some(Some(fact)) if fact.id == run || fact.job == run => Ok(fact),
        Some(Some(fact)) => Err(format!(
            "{run} isn't a run name; the Gym read it as {}, so cite that",
            fact.id
        )),
        _ => Err(format!("the Gym has no run {run}")),
    }
}

fn check_file(file: &str, repo: &Path) -> Result<(), String> {
    let (path, line) = match file.rsplit_once(':') {
        Some((path, line)) if line.chars().all(|c| c.is_ascii_digit()) && !line.is_empty() => {
            (path, line.parse::<usize>().ok())
        }
        _ => (file, None),
    };
    if Path::new(path).is_absolute() || path.split('/').any(|part| part == "..") {
        return Err(format!("{file} isn't a path inside the repository"));
    }
    let full = repo.join(path);
    let text = std::fs::read_to_string(&full)
        .map_err(|_| format!("{path} isn't a file in the repository"))?;
    match line {
        Some(line) if line == 0 || line > text.lines().count() => Err(format!(
            "{path} has {} lines; line {line} isn't one",
            text.lines().count()
        )),
        _ => Ok(()),
    }
}

/// Totals over checked claims: claims, verified claims, citations, and
/// valid citations.
#[must_use]
pub fn totals(claims: &[Claim]) -> Value {
    let citations: usize = claims.iter().map(|c| c.citations).sum();
    let valid: usize = claims.iter().map(|c| c.valid).sum();
    json!({
        "claims": claims.len(),
        "verified": claims.iter().filter(|c| c.verified()).count(),
        "unverified": claims.iter().filter(|c| !c.verified()).count(),
        "citations": citations,
        "valid_citations": valid,
        "validity": if citations == 0 { Value::Null } else { json!((valid as f64 / citations as f64 * 1000.0).round() / 1000.0) },
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts() -> BTreeMap<String, Option<RunFacts>> {
        let shown = json!({
            "run": {"job": "tb4--a", "trial": "a__1"},
            "transcript": [{"step": 1}, {"step": 2}, {"step": 3}],
            "marks": [{"run": "tb4--a/a__1", "step": null, "verdict": "bad"}, {"run": "tb4--a/a__1", "step": 2, "verdict": "bad"}],
            "learning": {"judgments": {"unearned_success": 0.97, "near_miss": 0.2}},
            "card": {"rows": [
                {"id": "session.1.model_share", "value": 0.641, "text": "64%"},
                {"id": "checks.line_grades", "value": null, "text": "not recorded"},
            ]},
        });
        let fact = RunFacts::from_show(&shown).unwrap();
        let unjudged = RunFacts {
            id: "tb4--b/b__1".to_string(),
            job: "tb4--b".to_string(),
            task: "b".to_string(),
            steps: 0,
            judgments: None,
            marks: Vec::new(),
            card: None,
        };
        BTreeMap::from([
            ("tb4--a/a__1".to_string(), Some(fact.clone())),
            ("tb4--a".to_string(), Some(fact.clone())),
            ("a".to_string(), Some(fact)),
            ("tb4--b/b__1".to_string(), Some(unjudged)),
            ("nope/x".to_string(), None),
        ])
    }

    #[test]
    fn citations_check_against_runs_steps_judgments_and_files() {
        let repo = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(repo.path().join("docs")).unwrap();
        std::fs::write(repo.path().join("docs/a.md"), "one\ntwo\n").unwrap();
        let answer = json!({"answer": "…", "claims": [
            {"claim": "good", "runs": ["tb4--a/a__1"], "steps": [{"run": "tb4--a/a__1", "step": 3}], "judgments": ["unearned_success"], "files": ["docs/a.md:2"]},
            {"claim": "job name", "runs": ["tb4--a"], "steps": [], "judgments": []},
            {"claim": "missing run", "runs": ["nope/x"], "steps": [], "judgments": []},
            {"claim": "bad step", "runs": ["tb4--a/a__1"], "steps": [{"run": "tb4--a/a__1", "step": 9}], "judgments": []},
            {"claim": "weak judgment", "runs": ["tb4--a/a__1"], "steps": [], "judgments": ["near_miss"]},
            {"claim": "unjudged", "runs": ["tb4--b/b__1"], "steps": [], "judgments": ["near_miss"]},
            {"claim": "a task name", "runs": ["a"], "steps": [], "judgments": []},
            {"claim": "uncited", "runs": [], "steps": [], "judgments": []},
            {"claim": "bad file", "runs": [], "steps": [], "judgments": [], "files": ["docs/a.md:9", "../x", "docs/none.md"]},
            {"claim": "marked", "runs": [], "steps": [], "judgments": [], "marks": ["tb4--a/a__1", "tb4--a/a__1/2"]},
            {"claim": "unmarked", "runs": [], "steps": [], "judgments": [], "marks": ["tb4--a/a__1/3", "tb4--b/b__1"]},
        ]});
        let mut claims = claims(&answer);
        assert_eq!(
            cited_runs(&claims),
            vec!["a", "nope/x", "tb4--a", "tb4--a/a__1", "tb4--b/b__1"]
        );
        check(&mut claims, &facts(), repo.path());
        let verified: Vec<bool> = claims.iter().map(Claim::verified).collect();
        assert_eq!(
            verified,
            vec![
                true, true, false, false, false, false, false, false, false, true, false
            ]
        );
        assert_eq!(
            claims[10].problems,
            vec![
                "no person marked step 3 of tb4--a/a__1",
                "no person marked tb4--b/b__1"
            ]
        );
        assert_eq!(split_mark("tb4--a/a__1/12"), ("tb4--a/a__1", Some(12)));
        assert_eq!(split_mark("tb4--a/a__1"), ("tb4--a/a__1", None));
        assert_eq!(split_mark("tb4--a"), ("tb4--a", None));
        assert_eq!(claims[0].citations, 4);
        assert!(
            claims[2].problems[0].contains("no run nope/x"),
            "{:?}",
            claims[2]
        );
        assert!(
            claims[3].problems[0].contains("3 transcript steps"),
            "{:?}",
            claims[3]
        );
        assert!(claims[4].problems[0].contains("0.20"), "{:?}", claims[4]);
        assert!(
            claims[5].problems[0].contains("no Jev judgment"),
            "{:?}",
            claims[5]
        );
        assert!(
            claims[6].problems[0].contains("cite that"),
            "{:?}",
            claims[6]
        );
        assert_eq!(claims[7].problems, vec!["the claim cites nothing"]);
        assert_eq!(claims[8].problems.len(), 3, "{:?}", claims[8]);
        let totals = totals(&claims);
        assert_eq!(totals["claims"], 11);
        assert_eq!(totals["verified"], 3);
        // Unverified claims are kept, not dropped.
        assert_eq!(claims.len(), 11);
    }

    #[test]
    fn card_row_citations_check_against_the_run_card() {
        let repo = tempfile::tempdir().unwrap();
        let answer = json!({"answer": "…", "claims": [
            {"claim": "Model latency was 64% of session 1.", "runs": [], "steps": [], "judgments": [], "files": [], "marks": [],
             "card_rows": [{"run": "tb4--a/a__1", "row": "session.1.model_share"}]},
            {"claim": "An unknown row.", "runs": [], "steps": [], "judgments": [], "files": [], "marks": [],
             "card_rows": [{"run": "tb4--a/a__1", "row": "checks.line_grades"}]},
            {"claim": "A run with no card.", "runs": [], "steps": [], "judgments": [], "files": [], "marks": [],
             "card_rows": [{"run": "tb4--b/b__1", "row": "session.1.turns"}]},
        ]});
        let mut claims = claims(&answer);
        assert_eq!(cited_runs(&claims), vec!["tb4--a/a__1", "tb4--b/b__1"]);
        check(&mut claims, &facts(), repo.path());
        assert!(claims[0].verified(), "{:?}", claims[0]);
        assert_eq!(
            claims[0].to_json()["card_rows"][0]["row"],
            "session.1.model_share"
        );
        assert!(
            claims[1].problems[0].contains("isn't a known row"),
            "{:?}",
            claims[1]
        );
        assert!(
            claims[2].problems[0].contains("no run card"),
            "{:?}",
            claims[2]
        );
    }
}
