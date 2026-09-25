//! `checks.metric_target` (issue #9657): measure the numeric goal a task
//! states for its finished work, and compare it with the threshold.
//!
//! Six of eleven mapped winning runs measured the task's stated goal, such
//! as a speedup, a latency, a cost, or a distance to a reference, with a
//! harness before they tried to improve it
//! (`docs/terminal-bench/2026-09-25-fable-pattern-map.md`). This component
//! is that measurement, in four parts:
//!
//! 1. **Extract.** Code finds every number in the instruction with its
//!    sentence ([`candidates`]) and the files the instruction names
//!    ([`references`]). One Jev request asks, per number, whether it's the
//!    threshold of a measured goal, which way the bound points, what
//!    quantity it bounds, and what the measurement is relative to
//!    (`questions/metric-target.json`). Code parses the threshold and its
//!    unit from the number ([`parse_number`]); Jev never writes a value.
//! 2. **Harness.** A provided script that measures the goal, which Jev
//!    picks from the workspace's scripts or none, or a harness a Luna
//!    session writes from the stated goal and the workspace's file names
//!    alone ([`HARNESS_GUIDANCE`]). The host freezes it.
//! 3. **Measure.** Code runs the harness with warmup runs and alternated
//!    repeats ([`measure`]): with a relative target, the candidate and the
//!    reference interleave, so drift in the machine's speed falls on both.
//!    The result is a median with its spread.
//! 4. **Compare.** [`Measurement::verdict`] compares the median with the
//!    threshold. [`refusal`] is `control.finish`'s rule: no `done` while a
//!    stated target is unmet or unmeasured.
//!
//! The harness contract: `sh harness.sh candidate` and, for a relative
//! target, `sh harness.sh reference`, each run from the workspace, print a
//! last `METRIC <number>` line. `METRIC_REFERENCE_DIR` names an untouched
//! copy of the workspace when the host keeps one. A provided script that
//! prints no such line is read by its wall time, for a quantity of time.

pub mod cli;
pub mod offline;
#[cfg(test)]
mod tests;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::component::jev::{self as jev_component, JevMode};
use crate::record::Recorder;

/// The Jev question set, embedded so its digest is the file's.
pub const QUESTION_SET: &str = include_str!("../../../../../questions/metric-target.json");

/// The most numbers one request asks about.
pub const MAX_CANDIDATES: usize = 20;

/// The most references one request offers.
pub const MAX_REFERENCES: usize = 10;

/// The most scripts the harness question offers.
pub const MAX_SCRIPTS: usize = 12;

/// Characters of a script's start the harness question reads.
pub const SCRIPT_HEAD: usize = 600;

/// The Noul above which a number is a goal's threshold.
pub const GOAL_P: f64 = 0.5;

/// The last-line prefix a harness prints its value with.
pub const METRIC: &str = "METRIC";

/// The file a written harness is.
pub const HARNESS_FILE: &str = "harness.sh";

/// What a measured goal bounds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Quantity {
    Speedup,
    Runtime,
    Latency,
    Throughput,
    Cost,
    Memory,
    Size,
    Error,
    Accuracy,
    Score,
    Count,
    Other,
}

impl Quantity {
    /// Every quantity, in the question's order.
    pub const ALL: [Quantity; 12] = [
        Quantity::Speedup,
        Quantity::Runtime,
        Quantity::Latency,
        Quantity::Throughput,
        Quantity::Cost,
        Quantity::Memory,
        Quantity::Size,
        Quantity::Error,
        Quantity::Accuracy,
        Quantity::Score,
        Quantity::Count,
        Quantity::Other,
    ];

    /// The quantity's option name.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Quantity::Speedup => "speedup",
            Quantity::Runtime => "runtime",
            Quantity::Latency => "latency",
            Quantity::Throughput => "throughput",
            Quantity::Cost => "cost",
            Quantity::Memory => "memory",
            Quantity::Size => "size",
            Quantity::Error => "error",
            Quantity::Accuracy => "accuracy",
            Quantity::Score => "score",
            Quantity::Count => "count",
            Quantity::Other => "other",
        }
    }

    /// The quantity an option name names.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Quantity> {
        Quantity::ALL.into_iter().find(|q| q.word() == word)
    }

    /// Whether a run's wall time measures it, so a harness that prints no
    /// value can still be read.
    #[must_use]
    pub fn timed(self) -> bool {
        matches!(
            self,
            Quantity::Speedup | Quantity::Runtime | Quantity::Latency
        )
    }
}

/// Which way a threshold bounds the measured value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// The value must reach the threshold: higher is better.
    AtLeast,
    /// The value must stay at or below it: lower is better.
    AtMost,
}

impl Direction {
    /// Whether `value` meets `threshold` in this direction.
    #[must_use]
    pub fn meets(self, value: f64, threshold: f64) -> bool {
        match self {
            Direction::AtLeast => value >= threshold,
            Direction::AtMost => value <= threshold,
        }
    }

    /// How much better `after` is than `before`: positive when it moved
    /// the way the bound points.
    #[must_use]
    pub fn gain(self, before: f64, after: f64) -> f64 {
        match self {
            Direction::AtLeast => after - before,
            Direction::AtMost => before - after,
        }
    }
}

/// What a target's measurement is compared with.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "name")]
pub enum Relative {
    /// An amount on its own.
    Absolute,
    /// The workspace as it was before any change.
    Original,
    /// A file or program the instruction names.
    Named(String),
}

impl Relative {
    /// Whether the measurement runs a reference beside the candidate.
    #[must_use]
    pub fn relative(&self) -> bool {
        !matches!(self, Relative::Absolute)
    }
}

/// A number in the instruction, with its sentence.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Candidate {
    /// The sentence that holds it, as written.
    pub sentence: String,
    /// The number with its unit, as written, such as `2.6x`.
    pub number: String,
    /// The value code parsed from it.
    pub value: f64,
    /// Its unit, normalized ([`parse_number`]); empty for none.
    pub unit: String,
}

/// A goal the task states for its finished work.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Target {
    pub quantity: Quantity,
    pub direction: Direction,
    pub threshold: f64,
    /// The threshold's unit, normalized: `x`, `%`, `s`, `ms`, `min`, `USD`,
    /// or the word the instruction used.
    pub unit: String,
    pub relative: Relative,
    /// The sentence and number it came from.
    pub sentence: String,
    pub number: String,
    /// Jev's probability that the number is a goal's threshold.
    pub p: f64,
}

impl Target {
    /// The target in a line a session reads.
    #[must_use]
    pub fn line(&self) -> String {
        let bound = match self.direction {
            Direction::AtLeast => "at least",
            Direction::AtMost => "at most",
        };
        let against = match &self.relative {
            Relative::Absolute => String::new(),
            Relative::Original => {
                ", measured against the code as it was before any change".to_string()
            }
            Relative::Named(name) => format!(", measured against {name}"),
        };
        format!(
            "{} {bound} {}{}{against} (from: \"{}\")",
            self.quantity.word(),
            number_text(self.threshold),
            if self.unit.is_empty() {
                String::new()
            } else {
                format!(" {}", self.unit)
            },
            crate::judge::clip(self.sentence.trim(), 300)
        )
    }
}

fn number_text(value: f64) -> String {
    if value.fract() == 0.0 && value.abs() < 1e15 {
        format!("{value:.0}")
    } else {
        let text = format!("{value:.4}");
        text.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

/// The question set's wording, loaded once.
pub struct QuestionSet {
    pub id: String,
    pub digest: String,
    value: Value,
}

/// The loaded question set.
///
/// # Panics
///
/// Panics when the embedded set isn't JSON, which a test rules out.
pub fn question_set() -> &'static QuestionSet {
    static SET: OnceLock<QuestionSet> = OnceLock::new();
    SET.get_or_init(|| {
        let value: Value = serde_json::from_str(QUESTION_SET).expect("the metric set is JSON");
        QuestionSet {
            id: value["id"].as_str().unwrap_or_default().to_string(),
            digest: atif::digest(&json!({
                "per_number": value["per_number"],
                "harness": value["harness"],
            })),
            value,
        }
    })
}

impl QuestionSet {
    fn text(&self, question: &str, field: &str) -> String {
        self.value["per_number"][question][field]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn criteria(&self, path: &[&str]) -> Vec<(String, String)> {
        let mut at = &self.value;
        for key in path {
            at = &at[*key];
        }
        at["criteria"]
            .as_object()
            .map(|map| {
                map.iter()
                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or_default().to_string()))
                    .collect()
            })
            .unwrap_or_default()
    }
}

/// The sentences of `text`: split at line breaks and at a `.`, `!`, or `?`
/// followed by white space.
#[must_use]
pub fn sentences(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let chars: Vec<char> = text.chars().collect();
    for (i, &c) in chars.iter().enumerate() {
        if c == '\n' {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            current.clear();
            continue;
        }
        current.push(c);
        if matches!(c, '.' | '!' | '?') && chars.get(i + 1).is_none_or(|n| n.is_whitespace()) {
            if !current.trim().is_empty() {
                out.push(current.trim().to_string());
            }
            current.clear();
        }
    }
    if !current.trim().is_empty() {
        out.push(current.trim().to_string());
    }
    out
}

fn number_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(
            r"(?P<cur>[$€£]\s?)?(?P<num>\d+(?:,\d{3})*(?:\.\d+)?(?:[eE][-+]?\d+)?)(?:\s*(?P<unit>[xX×%]|[A-Za-zµμ]+))?",
        )
        .expect("the number pattern compiles")
    })
}

/// A number token's value and its normalized unit: `2.6x` is `(2.6,
/// "x")`, `5 seconds` is `(5, "s")`, `$1,200` is `(1200, "USD")`.
#[must_use]
pub fn parse_number(token: &str) -> Option<(f64, String)> {
    let found = number_pattern().captures(token.trim())?;
    let value: f64 = found["num"].replace(',', "").parse().ok()?;
    let unit = if found.name("cur").is_some() {
        "USD".to_string()
    } else {
        found
            .name("unit")
            .map_or(String::new(), |u| normalize_unit(u.as_str()))
    };
    Some((value, unit))
}

fn normalize_unit(word: &str) -> String {
    let lower = word.to_lowercase();
    match lower.as_str() {
        "x" | "×" | "times" | "fold" => "x".to_string(),
        "%" | "percent" | "pct" => "%".to_string(),
        "s" | "sec" | "secs" | "second" | "seconds" => "s".to_string(),
        "ms" | "millisecond" | "milliseconds" => "ms".to_string(),
        "us" | "µs" | "μs" | "microsecond" | "microseconds" => "us".to_string(),
        "min" | "mins" | "minute" | "minutes" => "min".to_string(),
        "h" | "hr" | "hrs" | "hour" | "hours" => "h".to_string(),
        "usd" | "dollar" | "dollars" => "USD".to_string(),
        _ => lower,
    }
}

/// Every number in `instruction` with its sentence, in order, at most
/// [`MAX_CANDIDATES`]. A digit that continues a word or a path, such as
/// `python3` or `v2`, isn't a number.
#[must_use]
pub fn candidates(instruction: &str) -> Vec<Candidate> {
    let mut out: Vec<Candidate> = Vec::new();
    for sentence in sentences(instruction) {
        for found in number_pattern().captures_iter(&sentence) {
            let whole = found.get(0).expect("a match has a whole");
            let num = found.name("num").expect("a match has a number");
            let before = sentence[..whole.start()].chars().last();
            if before.is_some_and(|c| c.is_alphanumeric() || "_./-:".contains(c)) {
                continue;
            }
            // A number that ends a version or a path, such as `1.2.3` or
            // `3.11/`, isn't a threshold.
            let after = sentence[num.end()..].chars().next();
            if after.is_some_and(|c| "._/".contains(c))
                && sentence[num.end()..]
                    .chars()
                    .nth(1)
                    .is_some_and(char::is_alphanumeric)
            {
                continue;
            }
            let mut token = whole.as_str().trim().to_string();
            // A unit word longer than a unit is the next word, kept only
            // when it reads as a unit.
            if let Some(unit) = found.name("unit")
                && unit.as_str().chars().count() > 12
            {
                token = sentence[whole.start()..unit.start()].trim().to_string();
            }
            let Some((value, unit)) = parse_number(&token) else {
                continue;
            };
            let candidate = Candidate {
                sentence: crate::judge::clip(&sentence, 500),
                number: token,
                value,
                unit,
            };
            if !out
                .iter()
                .any(|c| c.sentence == candidate.sentence && c.number == candidate.number)
            {
                out.push(candidate);
            }
            if out.len() >= MAX_CANDIDATES {
                return out;
            }
        }
    }
    out
}

fn reference_pattern() -> &'static regex::Regex {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    PATTERN.get_or_init(|| {
        regex::Regex::new(r"`([^`\n]{1,80})`|\b([\w./-]+\.[A-Za-z][A-Za-z0-9]{0,5})\b")
            .expect("the reference pattern compiles")
    })
}

/// The files and programs `instruction` names, in order, at most
/// [`MAX_REFERENCES`]: code spans and words with a file extension.
#[must_use]
pub fn references(instruction: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for found in reference_pattern().captures_iter(instruction) {
        let name = found
            .get(1)
            .or_else(|| found.get(2))
            .map(|m| m.as_str().trim().to_string())
            .unwrap_or_default();
        // Numbers and versions aren't references.
        if name.is_empty()
            || name
                .chars()
                .all(|c| c.is_ascii_digit() || ".,eE-+x%".contains(c))
            || out.contains(&name)
        {
            continue;
        }
        out.push(name);
        if out.len() >= MAX_REFERENCES {
            break;
        }
    }
    out
}

/// A script the harness question offers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Script {
    /// Relative to the workspace.
    pub path: String,
    pub head: String,
}

/// The scripts a provided harness may be: shell and Python files at most
/// two levels down, outside test directories, at most [`MAX_SCRIPTS`], in
/// path order.
#[must_use]
pub fn scripts(workdir: &Path) -> Vec<Script> {
    let mut found = Vec::new();
    let mut stack = vec![(workdir.to_path_buf(), 0usize)];
    while let Some((dir, depth)) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(std::fs::DirEntry::file_name);
        for entry in entries {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') {
                continue;
            }
            if path.is_dir() {
                let skip = matches!(
                    name.as_str(),
                    "tests" | "test" | "node_modules" | "__pycache__" | "target" | "venv"
                );
                if depth < 2 && !skip {
                    stack.push((path, depth + 1));
                }
                continue;
            }
            let script = std::path::Path::new(&name)
                .extension()
                .is_some_and(|e| e == "sh" || e == "py");
            if !script || name.starts_with("test_") || name.ends_with("_test.py") {
                continue;
            }
            let Ok(relative) = path.strip_prefix(workdir) else {
                continue;
            };
            let text = std::fs::read_to_string(&path).unwrap_or_default();
            found.push(Script {
                path: relative.display().to_string(),
                head: crate::judge::clip(&text, SCRIPT_HEAD),
            });
        }
    }
    found.sort_by(|a, b| a.path.cmp(&b.path));
    found.truncate(MAX_SCRIPTS);
    found
}

/// The one Jev request that extracts the targets.
pub struct Request {
    pub state: Value,
    pub questions: ::jev::Questions,
}

/// The extraction request over `instruction`'s numbers, `references`, and
/// `scripts` (empty outside a workspace).
#[must_use]
pub fn request(
    instruction: &str,
    numbers: &[Candidate],
    references: &[String],
    scripts: &[Script],
) -> Request {
    let set = question_set();
    let mut questions = ::jev::Questions::new();
    for j in 0..numbers.len() {
        let finding = format!("numbers[{j}]");
        questions = questions.with(
            format!("goal_{j}"),
            ::jev::Noul::new(
                set.text("goal", "instructions")
                    .replace("{finding}", &finding),
            ),
        );
        for name in ["direction", "quantity"] {
            let mut choice = ::jev::Choice::new(
                set.text(name, "instructions")
                    .replace("{finding}", &finding)
                    .as_str(),
                indexmap::IndexMap::new(),
            );
            for (option, words) in set.criteria(&["per_number", name]) {
                choice = choice.option(option, words);
            }
            questions = questions.with(format!("{name}_{j}"), choice);
        }
        let mut choice = ::jev::Choice::new(
            set.text("relative", "instructions")
                .replace("{finding}", &finding)
                .as_str(),
            indexmap::IndexMap::new(),
        );
        for (k, name) in references.iter().enumerate() {
            choice = choice.option(format!("r{k}"), format!("`references[{k}]`: {name}"));
        }
        for (option, words) in set.criteria(&["per_number", "relative"]) {
            choice = choice.option(option, words);
        }
        questions = questions.with(format!("relative_{j}"), choice);
    }
    if !scripts.is_empty() {
        let mut choice = ::jev::Choice::new(
            set.value["harness"]["instructions"]
                .as_str()
                .unwrap_or_default(),
            indexmap::IndexMap::new(),
        );
        for (k, script) in scripts.iter().enumerate() {
            choice = choice.option(format!("s{k}"), format!("`scripts[{k}]`: {}", script.path));
        }
        for (option, words) in set.criteria(&["harness"]) {
            choice = choice.option(option, words);
        }
        questions = questions.with("harness", choice);
    }
    Request {
        state: json!({
            "task": crate::judge::clip(instruction, 6_000),
            "numbers": numbers
                .iter()
                .map(|c| json!({"sentence": c.sentence, "number": c.number}))
                .collect::<Vec<_>>(),
            "references": references,
            "scripts": scripts
                .iter()
                .map(|s| json!({"path": s.path, "start": s.head}))
                .collect::<Vec<_>>(),
        }),
        questions,
    }
}

/// The recorded-answer key of a request, as [`jev_component::ask`]
/// computes it.
#[must_use]
pub fn request_key(request: &Request) -> String {
    crate::checks::conformance::request_key(&request.state, &request.questions)
}

/// What extraction found.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Extracted {
    /// The numbers code found.
    pub candidates: Vec<Candidate>,
    pub references: Vec<String>,
    /// The goals, in the instruction's order.
    pub targets: Vec<Target>,
    /// The provided script Jev picked, relative to the workspace.
    pub harness: Option<String>,
    /// Whether Jev answered; `false` leaves the targets unknown, not
    /// absent.
    pub answered: bool,
    pub usd: f64,
    /// The request's record: how it was answered, its key, the answers,
    /// and the tokens, so a live run can be kept for replay.
    pub call: Value,
}

/// The targets `answers` name among `numbers`.
#[must_use]
pub fn targets_from(answers: &Value, numbers: &[Candidate], references: &[String]) -> Vec<Target> {
    let mut out = Vec::new();
    for (j, number) in numbers.iter().enumerate() {
        let p = answers[format!("goal_{j}")]["noul"].as_f64().unwrap_or(0.0);
        if p < GOAL_P {
            continue;
        }
        let direction = match answers[format!("direction_{j}")]["choice"].as_str() {
            Some("at_least") => Direction::AtLeast,
            Some("at_most") => Direction::AtMost,
            _ => continue,
        };
        let Some(quantity) = answers[format!("quantity_{j}")]["choice"]
            .as_str()
            .and_then(Quantity::from_word)
        else {
            continue;
        };
        let relative = match answers[format!("relative_{j}")]["choice"].as_str() {
            Some("original") => Relative::Original,
            Some(option) if option.starts_with('r') => option[1..]
                .parse::<usize>()
                .ok()
                .and_then(|k| references.get(k))
                .map_or(Relative::Absolute, |name| Relative::Named(name.clone())),
            _ => Relative::Absolute,
        };
        out.push(Target {
            quantity,
            direction,
            threshold: number.value,
            unit: number.unit.clone(),
            relative,
            sentence: number.sentence.clone(),
            number: number.number.clone(),
            p,
        });
    }
    out
}

/// Where extraction runs.
pub struct Context<'a> {
    pub component: &'a str,
    pub id: String,
    pub deadline: Option<crate::deadline::Deadline>,
}

/// Extracts the targets `instruction` states, and in `workdir`, when there
/// is one, the provided script that measures them.
pub async fn extract(
    jev: &JevMode,
    recorder: &Recorder,
    context: &Context<'_>,
    instruction: &str,
    workdir: Option<&Path>,
) -> Extracted {
    let numbers = candidates(instruction);
    let references = references(instruction);
    let scripts = workdir.map(scripts).unwrap_or_default();
    let mut out = Extracted {
        candidates: numbers.clone(),
        references: references.clone(),
        ..Extracted::default()
    };
    if numbers.is_empty() {
        out.answered = true;
        out.call = json!({"how": "none", "reason": "the instruction holds no number"});
        return out;
    }
    let asked_for = request(instruction, &numbers, &references, &scripts);
    let asked = jev_component::ask(
        jev,
        recorder,
        jev_component::Ask {
            component: context.component,
            name: "jev_metric_target",
            id: context.id.clone(),
            state: asked_for.state.clone(),
            questions: asked_for.questions.clone(),
            parent: None,
            deadline: context.deadline.clone(),
        },
    )
    .await;
    out.usd = asked.input_tokens.map_or(0.0, |t| {
        t as f64 * jev_component::USD_PER_MILLION_INPUT / 1_000_000.0
    });
    out.call = json!({
        "how": asked.how,
        "key": asked.key,
        "error": asked.error,
        "answers": asked.answers,
        "input_tokens": asked.input_tokens,
        "output_tokens": asked.output_tokens,
        "milliseconds": asked.milliseconds,
        "questions": question_set().id,
        "questions_digest": question_set().digest,
    });
    if let Some(answers) = &asked.answers {
        out.answered = true;
        out.targets = targets_from(answers, &numbers, &references);
        out.harness = answers["harness"]["choice"]
            .as_str()
            .and_then(|o| o.strip_prefix('s'))
            .and_then(|k| k.parse::<usize>().ok())
            .and_then(|k| scripts.get(k))
            .map(|s| s.path.clone());
    }
    out
}

/// Where a harness comes from.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    /// A script the workspace ships, run as it is.
    Provided,
    /// A harness a Luna session wrote from the goal.
    Written,
}

/// A frozen harness.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Harness {
    pub source: Source,
    /// The program and its arguments before the side, run from the
    /// workspace: `["sh", "/frozen/harness.sh"]`, or `["python3",
    /// "bench.py"]` for a provided script.
    pub command: Vec<String>,
    /// Whether the side (`candidate` or `reference`) is passed as the last
    /// argument; a provided script takes none.
    pub sided: bool,
    /// The frozen copy's directory and its digest, checked before every
    /// measurement; `None` for a provided script, which the workspace
    /// holds.
    pub frozen: Option<PathBuf>,
    pub digest: Option<BTreeMap<String, String>>,
}

impl Harness {
    /// A provided script, run as it is: `python3` for a `.py`, `sh`
    /// otherwise.
    #[must_use]
    pub fn provided(path: &str) -> Harness {
        let program = if path.ends_with(".py") {
            "python3"
        } else {
            "sh"
        };
        Harness {
            source: Source::Provided,
            command: vec![program.to_string(), path.to_string()],
            sided: false,
            frozen: None,
            digest: None,
        }
    }

    /// Whether the frozen copy is still what was frozen.
    #[must_use]
    pub fn intact(&self) -> bool {
        match (&self.frozen, &self.digest) {
            (Some(dir), Some(digest)) => {
                crate::micro::lean::evidence_tree(dir).as_ref() == Ok(digest)
            }
            _ => true,
        }
    }
}

/// Which side a run measures.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Candidate,
    Reference,
}

impl Side {
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Side::Candidate => "candidate",
            Side::Reference => "reference",
        }
    }
}

/// One harness run.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run {
    pub side: Side,
    /// Exited 0 within its bound.
    pub ok: bool,
    /// The last `METRIC` line's value, when it printed one.
    pub printed: Option<f64>,
    pub seconds: f64,
    /// The output's tail.
    pub tail: String,
}

/// The last `METRIC <number>` line's value in `output`.
#[must_use]
pub fn parse_metric(output: &str) -> Option<f64> {
    let line = output
        .lines()
        .rev()
        .find(|l| l.split_whitespace().next() == Some(METRIC))?;
    let mut words = line.split_whitespace().skip(1);
    let value: f64 = words.next()?.parse().ok()?;
    value.is_finite().then_some(value)
}

/// Runs a harness on one side. The lean loop's runner spawns it
/// ([`crate::micro::optimize`]); a test's answers from a script.
pub(crate) trait Runner {
    /// Runs the harness once on `side`, bounded by `wall`.
    async fn run(&self, side: Side, wall: Duration) -> Run;
}

/// How a target is measured.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, default)]
pub struct Protocol {
    /// Unrecorded runs first, of each side.
    pub warmup: u32,
    /// Recorded runs of each side, alternated.
    pub repeats: u32,
    /// One run's wall-time bound, in seconds.
    pub run_sec: u64,
    /// All of one measurement's runs together, in seconds.
    pub budget_sec: u64,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol {
            warmup: 1,
            repeats: 5,
            run_sec: 120,
            budget_sec: 600,
        }
    }
}

/// A target's measurement.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measurement {
    /// The median of the recorded values, in the target's unit; `None`
    /// when unmeasured.
    pub value: Option<f64>,
    /// Half the range of the recorded values: how far a value moves from
    /// run to run.
    pub spread: f64,
    /// Each recorded value, in order.
    pub values: Vec<f64>,
    /// Why there's no value, when there's none.
    pub error: Option<String>,
    /// The last candidate run's output tail: what the harness printed,
    /// such as the time of each phase.
    pub tail: String,
    /// Every run, the warmup included.
    pub runs: Vec<Run>,
}

/// How a measurement compares with the threshold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The median meets the threshold.
    Met,
    /// The median misses it.
    Unmet,
    /// No value.
    Unmeasured,
}

impl Measurement {
    /// A measurement with no value.
    #[must_use]
    pub fn unmeasured(error: &str, runs: Vec<Run>) -> Measurement {
        Measurement {
            value: None,
            spread: 0.0,
            values: Vec::new(),
            error: Some(error.to_string()),
            tail: runs.last().map(|r| r.tail.clone()).unwrap_or_default(),
            runs,
        }
    }

    /// How the median compares with `target`'s threshold.
    #[must_use]
    pub fn verdict(&self, target: &Target) -> Verdict {
        match self.value {
            None => Verdict::Unmeasured,
            Some(v) if target.direction.meets(v, target.threshold) => Verdict::Met,
            Some(_) => Verdict::Unmet,
        }
    }

    /// Whether every recorded value, not only the median, is on the same
    /// side of the threshold.
    #[must_use]
    pub fn confident(&self, target: &Target) -> bool {
        let met = self.verdict(target) == Verdict::Met;
        !self.values.is_empty()
            && self
                .values
                .iter()
                .all(|v| target.direction.meets(*v, target.threshold) == met)
    }

    /// The measurement in a line a session reads.
    #[must_use]
    pub fn line(&self, target: &Target) -> String {
        match self.value {
            Some(v) => format!(
                "{} {} ± {}{} over {} runs; the target is {}: {}",
                target.quantity.word(),
                number_text(v),
                number_text(self.spread),
                if target.unit.is_empty() {
                    String::new()
                } else {
                    format!(" {}", target.unit)
                },
                self.values.len(),
                number_text(target.threshold),
                match self.verdict(target) {
                    Verdict::Met => "met",
                    _ => "not met",
                }
            ),
            None => format!(
                "{} unmeasured: {}",
                target.quantity.word(),
                self.error.as_deref().unwrap_or("no value")
            ),
        }
    }
}

/// A run's value in the target's unit: the printed `METRIC`, or for a
/// quantity of time, the wall time.
fn reading(run: &Run, target: &Target) -> Option<f64> {
    if !run.ok {
        return None;
    }
    if let Some(value) = run.printed {
        return Some(value);
    }
    if !target.quantity.timed() {
        return None;
    }
    // A speedup's sides are times, compared as a ratio; any unit does.
    Some(match target.unit.as_str() {
        "ms" if target.quantity != Quantity::Speedup => run.seconds * 1_000.0,
        "us" if target.quantity != Quantity::Speedup => run.seconds * 1_000_000.0,
        "min" if target.quantity != Quantity::Speedup => run.seconds / 60.0,
        "h" if target.quantity != Quantity::Speedup => run.seconds / 3_600.0,
        _ => run.seconds,
    })
}

/// One recorded pair's value for a relative target: a speedup is the
/// reference's time over the candidate's; anything else is the candidate
/// over the reference. A `%` unit reads the ratio as a percentage: a
/// speedup's as the gain over 1.
fn ratio(candidate: f64, reference: f64, target: &Target) -> Option<f64> {
    let value = if target.quantity == Quantity::Speedup {
        (candidate > 0.0).then(|| reference / candidate)?
    } else {
        (reference != 0.0).then(|| candidate / reference)?
    };
    Some(match (target.quantity, target.unit.as_str()) {
        (Quantity::Speedup, "%") => (value - 1.0) * 100.0,
        (_, "%") => value * 100.0,
        _ => value,
    })
}

fn median(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    let n = sorted.len();
    if n % 2 == 1 {
        sorted[n / 2]
    } else {
        f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])
    }
}

/// Measures `target` with `runner` under `protocol`: the warmup runs,
/// then `repeats` recorded runs, alternating the reference and the
/// candidate for a relative target (reference first on even repeats,
/// candidate first on odd ones). Any failed recorded run, or a value it
/// can't read, leaves the target unmeasured; so does a budget that ends
/// before two recorded values.
pub(crate) async fn measure<R: Runner>(
    runner: &R,
    target: &Target,
    protocol: &Protocol,
    budget: Duration,
) -> Measurement {
    let started = std::time::Instant::now();
    let relative = target.relative.relative();
    let wall = |left: Duration| Duration::from_secs(protocol.run_sec).min(left);
    let left = || budget.saturating_sub(started.elapsed());
    let mut runs = Vec::new();
    for _ in 0..protocol.warmup {
        if relative {
            runs.push(runner.run(Side::Reference, wall(left())).await);
        }
        runs.push(runner.run(Side::Candidate, wall(left())).await);
        if left().is_zero() {
            return Measurement::unmeasured("the measurement budget ran out in the warmup", runs);
        }
    }
    let mut values = Vec::new();
    for i in 0..protocol.repeats {
        if left().is_zero() {
            break;
        }
        let order = if !relative {
            vec![Side::Candidate]
        } else if i % 2 == 0 {
            vec![Side::Reference, Side::Candidate]
        } else {
            vec![Side::Candidate, Side::Reference]
        };
        let mut pair: BTreeMap<&str, Option<f64>> = BTreeMap::new();
        for side in order {
            let run = runner.run(side, wall(left())).await;
            pair.insert(side.word(), reading(&run, target));
            let failed = !run.ok;
            runs.push(run);
            if failed {
                return Measurement::unmeasured(&format!("a {} run failed", side.word()), runs);
            }
        }
        let candidate = pair.get("candidate").copied().flatten();
        let value = if relative {
            match (candidate, pair.get("reference").copied().flatten()) {
                (Some(c), Some(r)) => ratio(c, r, target),
                _ => None,
            }
        } else {
            candidate
        };
        let Some(value) = value else {
            return Measurement::unmeasured(
                "the harness printed no METRIC line, and the quantity isn't a time",
                runs,
            );
        };
        values.push(value);
    }
    let need = protocol.repeats.clamp(1, 2) as usize;
    if values.len() < need {
        return Measurement::unmeasured("the measurement budget ran out before two runs", runs);
    }
    let lo = values.iter().copied().fold(f64::INFINITY, f64::min);
    let hi = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let tail = runs
        .iter()
        .rev()
        .find(|r| r.side == Side::Candidate)
        .map(|r| r.tail.clone())
        .unwrap_or_default();
    Measurement {
        value: Some(median(&values)),
        spread: (hi - lo) / 2.0,
        values,
        error: None,
        tail,
        runs,
    }
}

/// `control.optimize`'s keep rule on the metric: `after` is better than
/// `before` by more than either measurement's spread.
#[must_use]
pub fn improves(before: &Measurement, after: &Measurement, direction: Direction) -> bool {
    match (before.value, after.value) {
        (Some(b), Some(a)) => {
            let gain = direction.gain(b, a);
            gain > 0.0 && gain > before.spread.max(after.spread)
        }
        (None, Some(_)) => true,
        _ => false,
    }
}

/// `control.finish`'s rule for a stated target: why a `done` finish can't
/// stand, or `None` when it may. No target, no refusal.
#[must_use]
pub fn refusal(target: Option<&Target>, last: Option<&Measurement>) -> Option<String> {
    let target = target?;
    match last {
        None => Some(format!(
            "the task states a target ({}), and nothing measured it",
            target.line()
        )),
        Some(m) => match m.verdict(target) {
            Verdict::Met => None,
            Verdict::Unmet => Some(format!(
                "the task states a target ({}), and the host's measurement misses it: {}",
                target.line(),
                m.line(target)
            )),
            Verdict::Unmeasured => Some(format!(
                "the task states a target ({}), and the host couldn't measure it: {}",
                target.line(),
                m.error.as_deref().unwrap_or("no value")
            )),
        },
    }
}

/// What the harness session is told. It sees the goal and the
/// workspace's file names, and writes the harness only.
pub const HARNESS_GUIDANCE: &str = "Write a measurement harness, and nothing else. Don't change \
any file in the workspace: the host checks, and puts the workspace back if one changed.\n\n\
The harness measures the goal stated below, the way the task defines it, on the workspace as \
it will be once the work is done. Write it as a POSIX shell script at the path the state gives. \
The host runs it from the workspace as `sh harness.sh candidate` and, when the goal compares \
with a reference, as `sh harness.sh reference`; each run must measure one side once and print a \
last line `METRIC <number>`, the measured value of that side in the goal's unit (for a speedup, \
print each side's time in seconds; the host divides). The host runs the harness several times \
and alternates the sides, so make each run self-contained and deterministic in its inputs: fix \
any random seed, and use the task's own inputs or a workload the task describes. When the goal \
compares with the code as it was before any change, the environment variable \
`METRIC_REFERENCE_DIR` names an untouched copy of the workspace for the reference side. Print \
what each phase took before the METRIC line if the work has phases. Run the harness once on \
each side to check that it prints a METRIC line where the code already exists, then finish \
with status `done`.";

/// What an optimization round's session is told, after the task.
pub const OPTIMIZE_GUIDANCE: &str = "The workspace passes the acceptance check. Make one \
improvement to the measured quantity below and nothing else: find where the time or the cost \
goes from the measurement's output, change that, and rerun the harness and the acceptance check \
yourself. The host measures the workspace after you finish. It keeps your change only when the \
acceptance check still passes and the measurement improves by more than its spread; otherwise it \
puts the workspace back. Finish with status `done` when you made a change you measured, or \
`blocked` when you found no improvement to make.";

/// The workspace's file names, for the harness session: the interfaces
/// it may call, without their contents.
#[must_use]
pub fn interfaces(workdir: &Path, max: usize) -> String {
    let mut files = crate::micro::parallel::workspace_files(workdir);
    files.sort();
    let total = files.len();
    files.truncate(max);
    let mut text = files.join("\n");
    if total > max {
        text.push_str(&format!("\n… and {} more files", total - max));
    }
    text
}
