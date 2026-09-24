//! The acceptance suite against the verifier, for [`crate::runs_analysis`].
//!
//! A frozen acceptance suite says when Coder One's loop is done; the
//! verifier says whether the run passed. This module lines them up: each
//! acceptance test with its requirements, whether it was a guard (green on
//! the untouched workspace), and how it stood at the first and last suite
//! runs; and each verifier test with the acceptance tests that check it.
//!
//! Rules pick the candidates. They compare the functions and classes each
//! test calls, weighted by how rare each is among the verifier's tests,
//! and the words of the verifier test's name and docstring against the
//! acceptance test's description and script. A single strong candidate is
//! taken as it is, and a verifier test with no candidate is uncovered.
//! Every other pair is ambiguous, and Jev answers two Nouls about it: does
//! the acceptance test check what the verifier test checks, and does it
//! require behavior the verifier test forbids. Each request covers one
//! verifier test and its candidates, and [`Cache`] keeps each answer under
//! the digest of its state and questions, so a run is asked once.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::path::PathBuf;
use std::time::Instant;

use jev::{Entry, Noul, NoulCriteria, Questions, SystemOneRequest};
use serde::Serialize;
use serde_json::{Value, json};

use crate::runs::{read_json, text};
use crate::runs_analysis::{Anomaly, JevUse, Records, VerifierTest};
use crate::runs_learning::{Judge, Recorded, USD_PER_MILLION_INPUT};

/// The question set's version. Change it with any change to a question's
/// wording or to what [`state`] puts in a request.
pub const QUESTION_SET: &str = "runs-analysis-suite-v1";

/// A candidate at or above this rule score, with no rival above
/// [`RIVAL`], counts as checking the verifier test without asking Jev.
pub const CLEAR: f64 = 0.75;

/// A second candidate at or above this score makes a clear one ambiguous.
pub const RIVAL: f64 = 0.4;

/// Candidates below this rule score aren't candidates.
pub const FLOOR: f64 = 0.2;

/// The most candidates one request asks about.
pub const CANDIDATES: usize = 4;

/// A `checks` answer at or above this is yes; at or above [`PARTLY`],
/// partly.
pub const YES: f64 = 0.6;

/// See [`YES`].
pub const PARTLY: f64 = 0.35;

/// How many requests run at once.
pub const CONCURRENCY: usize = 6;

const SCRIPT_LIMIT: usize = 1_500;
const SOURCE_LIMIT: usize = 4_000;
const CACHE_SCHEMA: &str = "openagents.gym.run-analysis.suite-answer.v1";

/// One acceptance test.
#[derive(Clone, Debug, Default, Serialize)]
pub struct AcceptTest {
    pub id: String,
    pub requirements: Vec<String>,
    pub what: String,
    /// Green on the untouched workspace: it checks behavior that already
    /// worked.
    pub guard: bool,
    /// Whether it passed at the suite's first run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<bool>,
    /// Whether it passed at the last suite run that ran it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<String>,
    #[serde(skip)]
    pub script: String,
}

/// One acceptance test as a candidate for a verifier test.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Candidate {
    pub test: String,
    /// The rules' score, 0 to 1.
    pub score: f64,
    /// The functions and words both tests share.
    pub shared: Vec<String>,
    /// Jev: the acceptance test checks what the verifier test checks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checks: Option<f64>,
    /// Jev: the acceptance test requires behavior the verifier test
    /// forbids.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub contradicts: Option<f64>,
}

/// One verifier test against the suite.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Mapping {
    pub verifier_test: String,
    pub status: String,
    pub candidates: Vec<Candidate>,
    /// `yes`, `partly`, `no`, or `contradicted`.
    pub covered: String,
    /// The acceptance tests the verdict rests on.
    pub by: Vec<String>,
    /// `rules`, `jev`, or `rules only`, when Jev had no answer.
    pub decided_by: String,
}

/// The suite against the verifier.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Section {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    pub tests: Vec<AcceptTest>,
    pub verifier: Vec<Mapping>,
    pub uncovered: Vec<String>,
    pub contradicted: Vec<String>,
}

// ---------------------------------------------------------------------------
// Rules
// ---------------------------------------------------------------------------

const MODULES: [&str; 22] = [
    "np",
    "numpy",
    "os",
    "sys",
    "json",
    "subprocess",
    "pytest",
    "math",
    "random",
    "re",
    "pd",
    "pandas",
    "scipy",
    "stats",
    "Path",
    "shutil",
    "time",
    "self",
    "str",
    "path",
    "torch",
    "io",
];

const BUILTINS: [&str; 52] = [
    "assert",
    "len",
    "range",
    "abs",
    "float",
    "int",
    "list",
    "dict",
    "str",
    "print",
    "isinstance",
    "all",
    "any",
    "sorted",
    "min",
    "max",
    "sum",
    "open",
    "round",
    "zip",
    "enumerate",
    "set",
    "tuple",
    "bool",
    "type",
    "repr",
    "map",
    "filter",
    "getattr",
    "hasattr",
    "super",
    "format",
    "array",
    "allclose",
    "isnan",
    "isfinite",
    "zeros",
    "ones",
    "default_rng",
    "normal",
    "seed",
    "run",
    "check_output",
    "Popen",
    "join",
    "exists",
    "read_text",
    "write_text",
    "load",
    "loads",
    "dumps",
    "raises",
];

const STOPWORDS: [&str; 40] = [
    "test", "tests", "the", "and", "for", "with", "that", "this", "from", "when", "does", "not",
    "use", "uses", "must", "should", "into", "are", "has", "have", "its", "value", "values",
    "return", "returns", "check", "checks", "match", "matches", "handle", "handles", "bug", "was",
    "were", "than", "then", "only", "each", "all", "any",
];

/// The functions and classes a test's source calls or imports.
#[must_use]
pub fn symbols(source: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    for line in source.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("from ")
            && let Some((_, names)) = rest.split_once(" import ")
        {
            for name in names.split(',') {
                let name = name
                    .trim()
                    .trim_matches(['(', ')'])
                    .split(" as ")
                    .next()
                    .unwrap_or("")
                    .trim();
                if !name.is_empty() && !name.starts_with('_') {
                    found.insert(name.to_owned());
                }
            }
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let mut index = 0;
        while index < chars.len() {
            let c = chars[index];
            if !(c.is_alphabetic() || c == '_') {
                index += 1;
                continue;
            }
            let begin = index;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            let word: String = chars[begin..index].iter().collect();
            let called = chars.get(index) == Some(&'(');
            let object = (begin > 0 && chars[begin - 1] == '.').then(|| {
                let mut start = begin - 1;
                while start > 0 && (chars[start - 1].is_alphanumeric() || chars[start - 1] == '_') {
                    start -= 1;
                }
                chars[start..begin - 1].iter().collect::<String>()
            });
            if !called || word.starts_with('_') || BUILTINS.contains(&word.as_str()) {
                continue;
            }
            if object.as_deref().is_some_and(|o| MODULES.contains(&o)) {
                continue;
            }
            found.insert(word);
        }
    }
    found
}

/// The words of a text, lowercased, without the common ones.
#[must_use]
pub fn words(text: &str) -> BTreeSet<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .map(str::to_lowercase)
        .filter(|w| w.chars().count() >= 3 && !w.chars().all(|c| c.is_ascii_digit()))
        .filter(|w| !STOPWORDS.contains(&w.as_str()))
        .map(|w| {
            if w.len() > 4 && w.ends_with('s') && !w.ends_with("ss") {
                w[..w.len() - 1].to_owned()
            } else {
                w
            }
        })
        .collect()
}

/// Inverse document frequency over the verifier's tests.
fn weights(sets: &[BTreeSet<String>]) -> HashMap<String, f64> {
    let mut counts: HashMap<String, usize> = HashMap::new();
    for set in sets {
        for item in set {
            *counts.entry(item.clone()).or_default() += 1;
        }
    }
    let n = sets.len().max(1) as f64;
    counts
        .into_iter()
        .map(|(item, count)| (item, (1.0 + n / count as f64).ln()))
        .collect()
}

fn overlap(
    of: &BTreeSet<String>,
    other: &BTreeSet<String>,
    weights: &HashMap<String, f64>,
) -> (f64, Vec<String>) {
    let weight = |item: &String| weights.get(item).copied().unwrap_or(1.0);
    let total: f64 = of.iter().map(weight).sum();
    if total <= 0.0 {
        return (0.0, Vec::new());
    }
    let shared: Vec<String> = of.intersection(other).cloned().collect();
    (shared.iter().map(weight).sum::<f64>() / total, shared)
}

/// The rule score of each acceptance test against each verifier test.
#[must_use]
pub fn candidates(verifier: &[VerifierTest], tests: &[AcceptTest]) -> Vec<Vec<Candidate>> {
    let verifier_symbols: Vec<BTreeSet<String>> =
        verifier.iter().map(|test| symbols(&test.source)).collect();
    let verifier_words: Vec<BTreeSet<String>> = verifier
        .iter()
        .map(|test| {
            words(&format!(
                "{} {}",
                test.name.trim_start_matches("test_"),
                test.doc.as_deref().unwrap_or_default()
            ))
        })
        .collect();
    let symbol_weights = weights(&verifier_symbols);
    let word_weights = weights(&verifier_words);
    let accept: Vec<(BTreeSet<String>, BTreeSet<String>)> = tests
        .iter()
        .map(|test| {
            (
                symbols(&test.script),
                words(&format!("{} {}", test.what, test.script)),
            )
        })
        .collect();
    verifier
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let mut found: Vec<Candidate> = tests
                .iter()
                .zip(&accept)
                .map(|(test, (test_symbols, test_words))| {
                    let (by_symbol, mut shared) =
                        overlap(&verifier_symbols[index], test_symbols, &symbol_weights);
                    let (by_word, shared_words) =
                        overlap(&verifier_words[index], test_words, &word_weights);
                    for word in shared_words {
                        if !shared.contains(&word) {
                            shared.push(word);
                        }
                    }
                    let score = if verifier_symbols[index].is_empty() {
                        by_word
                    } else {
                        0.6 * by_symbol + 0.4 * by_word
                    };
                    Candidate {
                        test: test.id.clone(),
                        score: (score * 100.0).round() / 100.0,
                        shared,
                        checks: None,
                        contradicts: None,
                    }
                })
                .filter(|candidate| candidate.score >= FLOOR)
                .collect();
            found.sort_by(|a, b| {
                b.score
                    .partial_cmp(&a.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.test.cmp(&b.test))
            });
            found.truncate(CANDIDATES);
            found
        })
        .collect()
}

/// Whether the rules decide a verifier test's candidates on their own.
fn clear(candidates: &[Candidate]) -> bool {
    match candidates {
        [] => true,
        [only] => only.score >= CLEAR,
        [first, second, ..] => first.score >= CLEAR && second.score < RIVAL,
    }
}

// ---------------------------------------------------------------------------
// The suite
// ---------------------------------------------------------------------------

fn clip(text: &str, limit: usize) -> String {
    if text.chars().count() <= limit {
        text.to_owned()
    } else {
        let kept: String = text.chars().take(limit).collect();
        format!("{kept}\n…")
    }
}

/// The acceptance tests, from the suite record and its scripts.
#[must_use]
pub fn tests(records: &Records) -> Vec<AcceptTest> {
    let Some(accept) = &records.accept else {
        return Vec::new();
    };
    let start: HashMap<String, bool> = accept
        .pointer("/start/tests")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|t| Some((text(t, "/id")?, t["green"].as_bool()?)))
        .collect();
    let judged = accept.pointer("/detail/judged/tests");
    let runs = records.episode.iter().flat_map(|e| &e.suite_runs);
    let mut last: HashMap<String, (bool, String)> = HashMap::new();
    for run in runs {
        for (id, green, ..) in &run.tests {
            last.insert(id.clone(), (*green, run.label.clone()));
        }
    }
    accept["tests"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|test| {
            let id = text(test, "/id").unwrap_or_default();
            let script = records
                .suite_dir
                .as_ref()
                .and_then(|dir| {
                    let path = text(test, "/path").unwrap_or_else(|| format!("tests/{id}.sh"));
                    std::fs::read_to_string(dir.join(path)).ok()
                })
                .unwrap_or_default();
            let guard = start.get(&id).copied() == Some(true)
                || judged
                    .and_then(|j| j.get(&id))
                    .is_some_and(|j| j["green_at_start"] == true);
            AcceptTest {
                requirements: test["requirements"]
                    .as_array()
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect(),
                what: text(test, "/what").unwrap_or_default(),
                guard,
                start: start.get(&id).copied(),
                last: last.get(&id).map(|(green, _)| *green),
                last_run: last.get(&id).map(|(_, label)| label.clone()),
                id,
                script,
            }
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Jev
// ---------------------------------------------------------------------------

/// The state one request carries: the verifier test and its candidates.
#[must_use]
pub fn state(verifier: &VerifierTest, candidates: &[&AcceptTest]) -> Value {
    let mut acceptance = serde_json::Map::new();
    for test in candidates {
        acceptance.insert(
            test.id.clone(),
            json!({
                "what": test.what,
                "script": clip(&test.script, SCRIPT_LIMIT),
            }),
        );
    }
    json!({
        "verifier_test": {
            "name": verifier.name,
            "source": clip(&verifier.source, SOURCE_LIMIT),
        },
        "acceptance_tests": acceptance,
    })
}

/// The questions about `ids`: two Nouls each.
#[must_use]
pub fn questions(ids: &[&str]) -> Questions {
    let mut questions = Questions::new();
    for id in ids {
        questions = questions
            .with(
                format!("checks_{id}"),
                Noul::with_criteria(
                    format!(
                        "Does acceptance test `{id}` in `acceptance_tests` check the behavior that `verifier_test` checks, so that code which fails `verifier_test` would also fail `{id}`?"
                    ),
                    NoulCriteria::new()
                        .when_true(format!(
                            "`{id}` asserts the same property on inputs that expose it: a defect that makes `verifier_test` fail makes `{id}` fail too."
                        ))
                        .when_false(format!(
                            "`{id}` checks something else, or checks the same code with inputs or bounds that the defect `verifier_test` catches would still pass."
                        )),
                ),
            )
            .with(
                format!("contradicts_{id}"),
                Noul::with_criteria(
                    format!(
                        "Could a correct fix for what `verifier_test` checks make acceptance test `{id}` fail, because `{id}` pins behavior that the fix changes?"
                    ),
                    NoulCriteria::new()
                        .when_true(format!(
                            "`{id}` holds the code to behavior that a fix `verifier_test` accepts would change, so fixing the code can turn `{id}` red."
                        ))
                        .when_false(format!(
                            "Every fix `verifier_test` accepts keeps `{id}` passing, or the two tests check unrelated behavior."
                        )),
                ),
            );
    }
    questions
}

/// The answer key: the digest of the state and the questions.
#[must_use]
pub fn key(state: &Value, questions: &Questions) -> String {
    atif::digest(&json!({
        "set": QUESTION_SET,
        "state": state,
        "questions": serde_json::to_value(questions).unwrap_or(Value::Null),
    }))
}

/// Jev's answers, kept by key.
#[derive(Clone, Debug, Default)]
pub struct Cache {
    pub dir: Option<PathBuf>,
    answers: HashMap<String, Value>,
}

impl Cache {
    /// The cache under `dir`; `None` keeps it in memory.
    #[must_use]
    pub fn open(dir: Option<PathBuf>) -> Self {
        Cache {
            dir,
            answers: HashMap::new(),
        }
    }

    /// The answers under `key`, from memory or disk.
    pub fn get(&mut self, key: &str) -> Option<Value> {
        if let Some(found) = self.answers.get(key) {
            return Some(found.clone());
        }
        let path = self
            .dir
            .as_ref()?
            .join("answers")
            .join(format!("{key}.json"));
        let record = read_json(&path).filter(|r| r["schema"] == CACHE_SCHEMA)?;
        let answers = record.get("answers")?.clone();
        self.answers.insert(key.to_owned(), answers.clone());
        Some(answers)
    }

    /// Keeps `answers` under `key`.
    pub fn put(&mut self, key: &str, answers: &Value, input_tokens: Option<u64>) {
        self.answers.insert(key.to_owned(), answers.clone());
        let Some(dir) = &self.dir else { return };
        let dir = dir.join("answers");
        if std::fs::create_dir_all(&dir).is_err() {
            return;
        }
        let record = json!({
            "schema": CACHE_SCHEMA,
            "set": QUESTION_SET,
            "key": key,
            "answers": answers,
            "input_tokens": input_tokens,
        });
        let path = dir.join(format!("{key}.json"));
        let temporary = path.with_extension(format!("tmp{}", std::process::id()));
        if std::fs::write(&temporary, format!("{record:#}\n")).is_ok() {
            let _ = std::fs::rename(&temporary, &path);
        }
    }
}

struct Request {
    index: usize,
    key: String,
    state: Value,
    questions: Questions,
}

/// The suite against the verifier, asking Jev about the ambiguous pairs
/// the cache has no answer for. `None` when the run kept no suite.
pub async fn section(
    records: &Records,
    judge: &Judge,
    cache: &mut Cache,
    mut record: Option<&mut Recorded>,
) -> (Option<Section>, JevUse) {
    let mut jev = JevUse {
        mode: judge.word().to_owned(),
        ..JevUse::default()
    };
    let tests = tests(records);
    if tests.is_empty() || records.verifier.is_empty() {
        return (
            (!tests.is_empty()).then(|| Section {
                status: records.accept.as_ref().and_then(|a| text(a, "/status")),
                digest: records.accept.as_ref().and_then(|a| text(a, "/digest")),
                tests,
                ..Section::default()
            }),
            jev,
        );
    }
    let mut all = candidates(&records.verifier, &tests);
    let by_id: HashMap<&str, &AcceptTest> = tests.iter().map(|t| (t.id.as_str(), t)).collect();
    let mut requests = Vec::new();
    let mut answered: BTreeMap<usize, Value> = BTreeMap::new();
    for (index, found) in all.iter().enumerate() {
        if clear(found) {
            continue;
        }
        let chosen: Vec<&AcceptTest> = found
            .iter()
            .filter_map(|c| by_id.get(c.test.as_str()).copied())
            .collect();
        let ids: Vec<&str> = chosen.iter().map(|t| t.id.as_str()).collect();
        let state = state(&records.verifier[index], &chosen);
        let questions = questions(&ids);
        let key = key(&state, &questions);
        if let Some(answers) = cache.get(&key) {
            jev.cached += 1;
            if let Some(record) = record.as_deref_mut() {
                record.entries.insert(key.clone(), answers.clone());
            }
            answered.insert(index, answers);
            continue;
        }
        requests.push(Request {
            index,
            key,
            state,
            questions,
        });
    }
    match judge {
        Judge::Off(why) => {
            if !requests.is_empty() {
                jev.errors
                    .push(format!("{} pairs wait for Jev: {why}", requests.len()));
            }
        }
        Judge::Recorded(recorded) => {
            for request in requests {
                jev.asked += 1;
                match recorded.entries.get(&request.key) {
                    Some(answers) => {
                        cache.put(&request.key, answers, None);
                        if let Some(record) = record.as_deref_mut() {
                            record.entries.insert(request.key.clone(), answers.clone());
                        }
                        answered.insert(request.index, answers.clone());
                    }
                    None => {
                        jev.failed += 1;
                        jev.errors.push(format!(
                            "{}: no recorded answer",
                            records.verifier[request.index].name
                        ));
                    }
                }
            }
        }
        Judge::Live(client) => {
            let mut pending: std::collections::VecDeque<Request> = requests.into();
            let mut set = tokio::task::JoinSet::new();
            loop {
                while set.len() < CONCURRENCY
                    && let Some(request) = pending.pop_front()
                {
                    let client = client.clone();
                    jev.asked += 1;
                    set.spawn(async move {
                        let started = Instant::now();
                        let result = client
                            .system_one(SystemOneRequest::new(
                                Entry::from(request.state.clone()),
                                request.questions.clone(),
                            ))
                            .await;
                        (request, result, started.elapsed())
                    });
                }
                let Some(joined) = set.join_next().await else {
                    break;
                };
                let Ok((request, result, _elapsed)) = joined else {
                    jev.failed += 1;
                    continue;
                };
                match result {
                    Ok(response) => {
                        let answers = serde_json::from_str::<Value>(&response.raw().text())
                            .ok()
                            .and_then(|body| body.get("answers").cloned())
                            .unwrap_or(Value::Null);
                        let tokens = response.usage.input_tokens;
                        jev.input_tokens += tokens.unwrap_or(0);
                        cache.put(&request.key, &answers, tokens);
                        if let Some(record) = record.as_deref_mut() {
                            record.entries.insert(request.key.clone(), answers.clone());
                        }
                        answered.insert(request.index, answers);
                    }
                    Err(error) => {
                        jev.failed += 1;
                        if jev.errors.len() < 5 {
                            let message = error.to_string();
                            jev.errors.push(format!(
                                "{}: {}",
                                records.verifier[request.index].name,
                                message.lines().next().unwrap_or_default()
                            ));
                        }
                    }
                }
            }
        }
    }
    jev.cost_usd = jev.input_tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0;

    let mut mappings = Vec::new();
    for (index, verifier) in records.verifier.iter().enumerate() {
        let found = &mut all[index];
        let answers = answered.get(&index);
        for candidate in found.iter_mut() {
            let noul = |prefix: &str| {
                answers
                    .and_then(|a| a.pointer(&format!("/{prefix}_{}/noul", candidate.test)))
                    .and_then(Value::as_f64)
            };
            candidate.checks = noul("checks");
            candidate.contradicts = noul("contradicts");
        }
        let (covered, by, decided_by) = if clear(found) {
            match found.first() {
                Some(first) => ("yes", vec![first.test.clone()], "rules"),
                None => ("no", Vec::new(), "rules"),
            }
        } else if answers.is_some() {
            let contradicting: Vec<String> = found
                .iter()
                .filter(|c| c.contradicts.is_some_and(|p| p >= YES))
                .map(|c| c.test.clone())
                .collect();
            let yes: Vec<String> = found
                .iter()
                .filter(|c| c.checks.is_some_and(|p| p >= YES))
                .map(|c| c.test.clone())
                .collect();
            let partly: Vec<String> = found
                .iter()
                .filter(|c| c.checks.is_some_and(|p| (PARTLY..YES).contains(&p)))
                .map(|c| c.test.clone())
                .collect();
            if !contradicting.is_empty() {
                ("contradicted", contradicting, "jev")
            } else if !yes.is_empty() {
                ("yes", yes, "jev")
            } else if !partly.is_empty() {
                ("partly", partly, "jev")
            } else {
                ("no", Vec::new(), "jev")
            }
        } else {
            // Jev had no answer: the rules' best guess, labeled as such.
            match found.first() {
                Some(first) if first.score >= RIVAL => {
                    ("partly", vec![first.test.clone()], "rules only")
                }
                _ => ("no", Vec::new(), "rules only"),
            }
        };
        mappings.push(Mapping {
            verifier_test: verifier.name.clone(),
            status: verifier.status.clone(),
            candidates: found.clone(),
            covered: covered.to_owned(),
            by,
            decided_by: decided_by.to_owned(),
        });
    }
    let uncovered = mappings
        .iter()
        .filter(|m| m.covered == "no")
        .map(|m| m.verifier_test.clone())
        .collect();
    let contradicted = mappings
        .iter()
        .filter(|m| m.covered == "contradicted")
        .map(|m| m.verifier_test.clone())
        .collect();
    (
        Some(Section {
            status: records.accept.as_ref().and_then(|a| text(a, "/status")),
            digest: records.accept.as_ref().and_then(|a| text(a, "/digest")),
            tests,
            verifier: mappings,
            uncovered,
            contradicted,
        }),
        jev,
    )
}

/// What the mapping shows that went wrong.
#[must_use]
pub fn anomalies(section: &Section) -> Vec<Anomaly> {
    let mut found = Vec::new();
    for mapping in &section.verifier {
        if mapping.status != "passed" && mapping.covered == "no" {
            found.push(Anomaly {
                kind: "uncovered failure".to_owned(),
                text: format!(
                    "The verifier failed `{}`, and no acceptance test checks it",
                    mapping.verifier_test
                ),
                seconds: None,
            });
        }
        if mapping.covered == "contradicted" {
            found.push(Anomaly {
                kind: "contradicting test".to_owned(),
                text: format!(
                    "{} {} behavior `{}` forbids",
                    mapping.by.join(", "),
                    if mapping.by.len() == 1 {
                        "requires"
                    } else {
                        "require"
                    },
                    mapping.verifier_test
                ),
                seconds: None,
            });
        }
    }
    found
}
