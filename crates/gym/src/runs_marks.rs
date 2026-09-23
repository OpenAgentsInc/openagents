//! Marks: a person's word on a run, or on one step of its transcript, and
//! how well Jev's judgments agree with it.
//!
//! Agents do things nobody sees until a person reads the run. A mark
//! records what that person saw: this run, or this step, is bad, with an
//! optional one-line note and the `runs-learning-v1` judgment IDs that name
//! what went wrong. A person can also clear a run: they read it and found
//! nothing wrong.
//!
//! - [`Marks`] is the store: an append-only `marks.jsonl` under
//!   `~/.openagents/gym/marks/`. Each line is a [`Record`] that names the
//!   run, the step, the digest of the evidence Jev reads for the run, the
//!   author, and the time, and carries the digest of the line before it.
//!   Removing a mark appends an `unmark` record; no line is ever rewritten,
//!   and a mark never edits a run's evidence.
//! - [`agreement`] builds a suite from the marks. Each tag is a positive
//!   label for its judgment on that run, and each cleared run is a negative
//!   label for every judgment. It reports agreement, precision, and recall
//!   for each judgment with 95% Wilson intervals and their denominators,
//!   and it says when too few labels exist to support a number.
//! - [`command`] is `gym runs mark`, `unmark`, `marks`, and `agreement`.
//!
//! The marks don't depend on the question set's wording: rewording a
//! judgment produces a new question set, and the same marks measure it.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::io::Write;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::coder_matrix::wilson;
use crate::runs::{Catalog, Run, Sources, clip_words, date, now_ms};
use crate::runs_learning::{self as learning, Answer, Context, JUDGMENTS, REASON_AT, Store};
use crate::runs_story::Detail;

/// The schema of one line in `marks.jsonl`.
pub const RECORD_SCHEMA: &str = "openagents.gym.runs-mark.v1";

/// The schema of `gym runs marks --json`.
pub const MARKS_SCHEMA: &str = "openagents.gym.runs-marks.v1";

/// The schema of `gym runs agreement --json`.
pub const AGREEMENT_SCHEMA: &str = "openagents.gym.runs-agreement.v1";

/// The file the marks are kept in, inside the marks directory.
pub const FILE: &str = "marks.jsonl";

/// The fewest labels of each kind, positive and negative, with Jev answers,
/// that a judgment needs before the report gives its numbers.
pub const MIN_LABELS: usize = 5;

/// The ID of the row that asks whether Jev gives any reason at all for the
/// runs a person marked bad, and none for the runs a person cleared.
pub const ANY_REASON: &str = "any_reason";

/// Where the marks are kept: `~/.openagents/gym/marks`.
#[must_use]
pub fn default_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/gym/marks"))
}

/// Who is marking: `$USER`, or `unknown`.
#[must_use]
pub fn default_author() -> String {
    std::env::var("USER")
        .ok()
        .filter(|user| !user.trim().is_empty())
        .unwrap_or_else(|| "unknown".to_owned())
}

/// What a mark says about its run or step.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// Something in the run or step is bad.
    Bad,
    /// A person read the run and found nothing wrong. Only a whole run can
    /// be cleared.
    Clear,
}

impl Verdict {
    /// The verdict's word.
    #[must_use]
    pub fn word(self) -> &'static str {
        match self {
            Verdict::Bad => "bad",
            Verdict::Clear => "cleared",
        }
    }

    /// The flag the list draws before a marked run's task.
    #[must_use]
    pub fn flag(self) -> char {
        match self {
            Verdict::Bad => '⚑',
            Verdict::Clear => '⚐',
        }
    }
}

/// Whether a record places a mark or removes one.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Mark,
    Unmark,
}

/// One line of `marks.jsonl`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Record {
    pub schema: String,
    pub action: Action,
    /// The run, `job/trial`.
    pub run: String,
    /// The transcript step, counted from 1, or `None` for the whole run.
    #[serde(default)]
    pub step: Option<usize>,
    /// What the mark says; `None` on an `unmark` record.
    #[serde(default)]
    pub verdict: Option<Verdict>,
    /// Judgment IDs from the question set the mark names.
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub note: Option<String>,
    /// The question set whose IDs the tags are.
    pub questions: String,
    /// The digest of the evidence Jev reads for the run as it stood when
    /// the mark was made, the key its answer is stored under. `None` when
    /// the run's records couldn't be read.
    #[serde(default)]
    pub evidence: Option<String>,
    pub author: String,
    /// When, in milliseconds since the epoch.
    pub at_ms: i64,
    /// The digest of the line before this one, or `None` for the first.
    #[serde(default)]
    pub previous: Option<String>,
    /// The digest of this record without this field.
    pub digest: String,
}

impl Record {
    /// The digest of everything in the record but the digest itself.
    #[must_use]
    pub fn compute_digest(&self) -> String {
        let mut value = serde_json::to_value(self).unwrap_or(Value::Null);
        if let Some(object) = value.as_object_mut() {
            object.remove("digest");
        }
        atif::digest(&value)
    }
}

/// A mark as it stands: the latest `mark` record for its run and step that
/// no later `unmark` removed.
#[derive(Clone, Debug, PartialEq)]
pub struct Mark {
    pub run: String,
    pub step: Option<usize>,
    pub verdict: Verdict,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub evidence: Option<String>,
    pub author: String,
    pub at_ms: i64,
    /// The digest of the record that placed it.
    pub digest: String,
}

impl Mark {
    fn from_record(record: &Record) -> Option<Self> {
        Some(Mark {
            run: record.run.clone(),
            step: record.step,
            verdict: record.verdict?,
            tags: record.tags.clone(),
            note: record.note.clone(),
            evidence: record.evidence.clone(),
            author: record.author.clone(),
            at_ms: record.at_ms,
            digest: record.digest.clone(),
        })
    }

    /// `run` or `run/12`.
    #[must_use]
    pub fn target(&self) -> String {
        match self.step {
            Some(step) => format!("{}/{step}", self.run),
            None => self.run.clone(),
        }
    }

    /// The mark in words, without the run: `step 12 marked bad: looped —
    /// "ran the same test 9 times" (chris, Sep 23 14:05)`.
    #[must_use]
    pub fn describe(&self) -> String {
        let what = match (self.step, self.verdict) {
            (Some(step), verdict) => format!("Step {step} marked {}", verdict.word()),
            (None, Verdict::Bad) => "Marked bad".to_owned(),
            (None, Verdict::Clear) => "Cleared: nothing wrong".to_owned(),
        };
        let mut text = what;
        if !self.tags.is_empty() {
            text.push_str(&format!(": {}", self.tags.join(", ")));
        }
        if let Some(note) = &self.note {
            text.push_str(&format!(" — \"{note}\""));
        }
        text.push_str(&format!(" ({}, {})", self.author, date(self.at_ms)));
        text
    }

    /// The mark as JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        json!({
            "run": self.run,
            "step": self.step,
            "verdict": self.verdict.word(),
            "tags": self.tags,
            "note": self.note,
            "evidence": self.evidence,
            "author": self.author,
            "at_ms": self.at_ms,
            "digest": self.digest,
        })
    }
}

/// The marks store.
#[derive(Clone, Debug, Default)]
pub struct Marks {
    /// Where the store lives; `None` keeps it in memory.
    pub dir: Option<PathBuf>,
    records: Vec<Record>,
    current: BTreeMap<(String, Option<usize>), Mark>,
    /// Lines that couldn't be read or whose digests don't hold.
    pub errors: Vec<String>,
}

impl Marks {
    /// Reads the store under `dir`; a missing file is an empty store.
    #[must_use]
    pub fn open(dir: Option<PathBuf>) -> Self {
        let mut marks = Marks {
            dir,
            ..Marks::default()
        };
        marks.reload();
        marks
    }

    /// The store's file, when it has a directory.
    #[must_use]
    pub fn path(&self) -> Option<PathBuf> {
        self.dir.as_ref().map(|dir| dir.join(FILE))
    }

    /// Reads the file again, so records another process appended count.
    /// A store with no directory keeps what it holds.
    pub fn reload(&mut self) {
        let Some(path) = self.path() else {
            return;
        };
        self.records.clear();
        self.errors.clear();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut previous: Option<String> = None;
        for (number, line) in text.lines().enumerate() {
            if line.trim().is_empty() {
                continue;
            }
            let record = match serde_json::from_str::<Record>(line) {
                Ok(record) if record.schema == RECORD_SCHEMA => record,
                Ok(record) => {
                    self.errors.push(format!(
                        "{} line {}: unknown schema {}",
                        path.display(),
                        number + 1,
                        record.schema
                    ));
                    continue;
                }
                Err(error) => {
                    self.errors
                        .push(format!("{} line {}: {error}", path.display(), number + 1));
                    continue;
                }
            };
            if record.compute_digest() != record.digest {
                self.errors.push(format!(
                    "{} line {}: the record's digest doesn't match its content",
                    path.display(),
                    number + 1
                ));
            } else if record.previous != previous {
                self.errors.push(format!(
                    "{} line {}: the chain breaks; the line before it isn't the one it names",
                    path.display(),
                    number + 1
                ));
            }
            previous = Some(record.digest.clone());
            self.records.push(record);
        }
        self.rebuild();
    }

    fn rebuild(&mut self) {
        self.current.clear();
        for record in &self.records {
            let target = (record.run.clone(), record.step);
            match record.action {
                Action::Mark => {
                    if let Some(mark) = Mark::from_record(record) {
                        self.current.insert(target, mark);
                    }
                }
                Action::Unmark => {
                    self.current.remove(&target);
                }
            }
        }
    }

    /// Every record, oldest first.
    #[must_use]
    pub fn records(&self) -> &[Record] {
        &self.records
    }

    /// Appends a record, chained to the last one on disk.
    fn append(&mut self, mut record: Record) -> Result<(), String> {
        self.reload();
        record.previous = self.records.last().map(|last| last.digest.clone());
        record.digest = record.compute_digest();
        if let Some(path) = self.path() {
            if let Some(dir) = &self.dir {
                std::fs::create_dir_all(dir)
                    .map_err(|error| format!("cannot create {}: {error}", dir.display()))?;
            }
            let line = serde_json::to_string(&record).map_err(|e| e.to_string())?;
            let mut file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .map_err(|error| format!("cannot open {}: {error}", path.display()))?;
            file.write_all(format!("{line}\n").as_bytes())
                .map_err(|error| format!("cannot write {}: {error}", path.display()))?;
        }
        self.records.push(record);
        self.rebuild();
        Ok(())
    }

    /// Marks `run`, or one step of it, and returns the mark.
    ///
    /// # Errors
    ///
    /// Returns a message when a tag isn't a judgment ID, a step is
    /// cleared, or the file can't be written.
    pub fn mark(&mut self, new: NewMark) -> Result<Mark, String> {
        if new.verdict == Verdict::Clear && new.step.is_some() {
            return Err("only a whole run can be cleared, not one step".to_owned());
        }
        if new.verdict == Verdict::Clear && !new.tags.is_empty() {
            return Err("a cleared run takes no tags; tags name what went wrong".to_owned());
        }
        let mut tags = Vec::new();
        for tag in new.tags {
            if learning::judgment(&tag).is_none() {
                return Err(format!(
                    "{tag} isn't a judgment in {}; the IDs are {}",
                    learning::QUESTION_SET,
                    JUDGMENTS.map(|j| j.id).join(", ")
                ));
            }
            if !tags.contains(&tag) {
                tags.push(tag);
            }
        }
        let note = new
            .note
            .map(|note| note.split_whitespace().collect::<Vec<_>>().join(" "))
            .filter(|note| !note.is_empty());
        let target = (new.run.clone(), new.step);
        self.append(Record {
            schema: RECORD_SCHEMA.to_owned(),
            action: Action::Mark,
            run: new.run,
            step: new.step,
            verdict: Some(new.verdict),
            tags,
            note,
            questions: learning::QUESTION_SET.to_owned(),
            evidence: new.evidence,
            author: new.author,
            at_ms: new.at_ms,
            previous: None,
            digest: String::new(),
        })?;
        self.current
            .get(&target)
            .cloned()
            .ok_or_else(|| "the mark wasn't kept".to_owned())
    }

    /// Removes the mark on `run`, or on one step of it. Returns whether
    /// there was one.
    ///
    /// # Errors
    ///
    /// Returns a message when the file can't be written.
    pub fn unmark(
        &mut self,
        run: &str,
        step: Option<usize>,
        author: &str,
        at_ms: i64,
    ) -> Result<bool, String> {
        self.reload();
        if !self.current.contains_key(&(run.to_owned(), step)) {
            return Ok(false);
        }
        self.append(Record {
            schema: RECORD_SCHEMA.to_owned(),
            action: Action::Unmark,
            run: run.to_owned(),
            step,
            verdict: None,
            tags: Vec::new(),
            note: None,
            questions: learning::QUESTION_SET.to_owned(),
            evidence: None,
            author: author.to_owned(),
            at_ms,
            previous: None,
            digest: String::new(),
        })?;
        Ok(true)
    }

    /// Every mark as it stands, by run and then step, the run's own first.
    pub fn all(&self) -> impl Iterator<Item = &Mark> {
        self.current.values()
    }

    /// How many marks stand.
    #[must_use]
    pub fn len(&self) -> usize {
        self.current.len()
    }

    /// Whether no mark stands.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.current.is_empty()
    }

    /// The marks on `run`: the run's own first, then its steps in order.
    #[must_use]
    pub fn of_run(&self, run: &str) -> Vec<&Mark> {
        self.current
            .range((run.to_owned(), None)..=(run.to_owned(), Some(usize::MAX)))
            .map(|(_, mark)| mark)
            .collect()
    }

    /// The mark on one target, when there is one.
    #[must_use]
    pub fn get(&self, run: &str, step: Option<usize>) -> Option<&Mark> {
        self.current.get(&(run.to_owned(), step))
    }

    /// Whether `run` or any of its steps is marked.
    #[must_use]
    pub fn is_marked(&self, run: &str) -> bool {
        !self.of_run(run).is_empty()
    }

    /// What the run's marks add up to: bad when any mark says bad, cleared
    /// when the run is cleared and nothing on it is marked bad.
    #[must_use]
    pub fn verdict(&self, run: &str) -> Option<Verdict> {
        let marks = self.of_run(run);
        if marks.iter().any(|mark| mark.verdict == Verdict::Bad) {
            Some(Verdict::Bad)
        } else if marks.iter().any(|mark| mark.verdict == Verdict::Clear) {
            Some(Verdict::Clear)
        } else {
            None
        }
    }

    /// The marks on `run`'s steps, by step number.
    #[must_use]
    pub fn steps(&self, run: &str) -> BTreeMap<usize, &Mark> {
        self.of_run(run)
            .into_iter()
            .filter_map(|mark| Some((mark.step?, mark)))
            .collect()
    }
}

/// What [`Marks::mark`] records.
#[derive(Clone, Debug)]
pub struct NewMark {
    pub run: String,
    pub step: Option<usize>,
    pub verdict: Verdict,
    pub tags: Vec<String>,
    pub note: Option<String>,
    pub evidence: Option<String>,
    pub author: String,
    pub at_ms: i64,
}

/// The digest of the evidence Jev reads for `detail`'s run: the key its
/// answer is stored under.
#[must_use]
pub fn evidence_key(detail: &Detail, context: &Context) -> String {
    learning::key(&learning::evidence(detail, context))
}

/// Splits `RUN[/STEP]`: a last piece that is a number is a step.
#[must_use]
pub fn parse_target(text: &str) -> (String, Option<usize>) {
    let text = text.trim().trim_end_matches('/');
    if let Some((run, step)) = text.rsplit_once('/')
        && !step.is_empty()
        && step.chars().all(|c| c.is_ascii_digit())
        && let Ok(step) = step.parse::<usize>()
    {
        return (run.to_owned(), Some(step));
    }
    (text.to_owned(), None)
}

// ---------------------------------------------------------------------------
// Agreement
// ---------------------------------------------------------------------------

/// A proportion with its numerator, denominator, and 95% Wilson interval.
/// With no denominator the value is unknown, never zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Proportion {
    pub hits: usize,
    pub of: usize,
}

impl Proportion {
    /// The share, or `None` with nothing to divide by.
    #[must_use]
    pub fn value(self) -> Option<f64> {
        (self.of > 0).then(|| self.hits as f64 / self.of as f64)
    }

    /// The 95% Wilson interval, or `None` with nothing to divide by.
    #[must_use]
    pub fn interval(self) -> Option<(f64, f64)> {
        (self.of > 0).then(|| wilson(self.hits, self.of))
    }

    fn to_json(self) -> Value {
        json!({
            "value": self.value().map(round3),
            "hits": self.hits,
            "of": self.of,
            "wilson_95": self.interval().map(|(low, high)| [round3(low), round3(high)]),
        })
    }

    /// `0.82 9/11 [0.52, 0.95]`, or `— 0/0`.
    #[must_use]
    pub fn text(self) -> String {
        match (self.value(), self.interval()) {
            (Some(value), Some((low, high))) => {
                format!("{value:.2} {}/{} [{low:.2}, {high:.2}]", self.hits, self.of)
            }
            _ => format!("— {}/{}", self.hits, self.of),
        }
    }
}

fn round3(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// One judgment measured against the marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    /// The judgment's ID, or [`ANY_REASON`].
    pub id: String,
    pub tag: String,
    /// Labels whose run has a Jev answer.
    pub positive: usize,
    pub negative: usize,
    /// Labels whose run has no Jev answer, so they count nowhere.
    pub unjudged: usize,
    pub true_positive: usize,
    pub false_positive: usize,
    pub false_negative: usize,
    pub true_negative: usize,
}

impl Row {
    fn new(id: &str, tag: &str) -> Self {
        Row {
            id: id.to_owned(),
            tag: tag.to_owned(),
            positive: 0,
            negative: 0,
            unjudged: 0,
            true_positive: 0,
            false_positive: 0,
            false_negative: 0,
            true_negative: 0,
        }
    }

    fn count(&mut self, label: bool, said: Option<bool>) {
        let Some(said) = said else {
            self.unjudged += 1;
            return;
        };
        match (label, said) {
            (true, true) => self.true_positive += 1,
            (true, false) => self.false_negative += 1,
            (false, true) => self.false_positive += 1,
            (false, false) => self.true_negative += 1,
        }
        if label {
            self.positive += 1;
        } else {
            self.negative += 1;
        }
    }

    /// The share of labels Jev's answer matches.
    #[must_use]
    pub fn agreement(&self) -> Proportion {
        Proportion {
            hits: self.true_positive + self.true_negative,
            of: self.positive + self.negative,
        }
    }

    /// Of the labeled runs Jev said yes to, the share a person marked.
    #[must_use]
    pub fn precision(&self) -> Proportion {
        Proportion {
            hits: self.true_positive,
            of: self.true_positive + self.false_positive,
        }
    }

    /// Of the runs a person marked, the share Jev said yes to.
    #[must_use]
    pub fn recall(&self) -> Proportion {
        Proportion {
            hits: self.true_positive,
            of: self.positive,
        }
    }

    /// Whether there are enough labels to support a number.
    #[must_use]
    pub fn supported(&self) -> bool {
        self.positive >= MIN_LABELS && self.negative >= MIN_LABELS
    }

    /// Why the row's numbers aren't supported, or `None` when they are.
    #[must_use]
    pub fn too_few(&self) -> Option<String> {
        (!self.supported()).then(|| {
            format!(
                "too few labels: {} positive and {} negative with Jev answers; a number needs {MIN_LABELS} of each",
                self.positive, self.negative
            )
        })
    }

    fn to_json(&self) -> Value {
        json!({
            "id": self.id,
            "tag": self.tag,
            "labels": {
                "positive": self.positive,
                "negative": self.negative,
                "unjudged": self.unjudged,
            },
            "confusion": {
                "true_positive": self.true_positive,
                "false_positive": self.false_positive,
                "false_negative": self.false_negative,
                "true_negative": self.true_negative,
            },
            "supported": self.supported(),
            "too_few": self.too_few(),
            "agreement": self.agreement().to_json(),
            "precision": self.precision().to_json(),
            "recall": self.recall().to_json(),
        })
    }
}

/// Every judgment against the marks.
#[derive(Clone, Debug, PartialEq)]
pub struct Agreement {
    /// Runs with any mark.
    pub marked_runs: usize,
    /// Runs a person marked bad, on the run or on a step.
    pub bad_runs: usize,
    /// Runs a person cleared and marked nothing bad on.
    pub cleared_runs: usize,
    /// Runs marked bad with no tag: they count only toward [`ANY_REASON`].
    pub untagged_runs: usize,
    /// Labeled runs with no Jev answer.
    pub unjudged_runs: usize,
    /// Labeled runs whose Jev answer is to the evidence the person saw.
    pub same_evidence: usize,
    /// Labeled runs whose Jev answer is to evidence that changed after the
    /// mark.
    pub changed_evidence: usize,
    /// [`ANY_REASON`] first, then each judgment in the question set's order.
    pub rows: Vec<Row>,
}

impl Agreement {
    /// How many rows have too few labels.
    #[must_use]
    pub fn unsupported(&self) -> usize {
        self.rows.iter().filter(|row| !row.supported()).count()
    }
}

/// Jev's answer for a marked run: the answer to the evidence the person
/// saw when there is one, else the run's current answer. The flag says
/// whether it's the evidence the person saw.
fn answer_for<'a>(
    marks: &[&Mark],
    store: &'a Store,
    current: &HashMap<String, &'a Answer>,
    run: &str,
) -> Option<(&'a Answer, bool)> {
    let mut seen: Vec<&Mark> = marks
        .iter()
        .copied()
        .filter(|mark| mark.evidence.is_some())
        .collect();
    seen.sort_by_key(|mark| std::cmp::Reverse(mark.at_ms));
    for mark in &seen {
        if let Some(answer) = mark.evidence.as_deref().and_then(|key| store.get(key)) {
            return Some((answer, true));
        }
    }
    let answer = current.get(run).copied()?;
    let same = seen
        .first()
        .is_some_and(|mark| mark.evidence.as_deref() == Some(answer.key.as_str()));
    Some((answer, same))
}

/// Measures Jev's judgments against the marks. `current` is each run's
/// answer to its evidence as it stands now.
#[must_use]
pub fn agreement(marks: &Marks, store: &Store, current: &HashMap<String, &Answer>) -> Agreement {
    let mut by_run: BTreeMap<&str, Vec<&Mark>> = BTreeMap::new();
    for mark in marks.all() {
        by_run.entry(mark.run.as_str()).or_default().push(mark);
    }
    let mut any = Row::new(ANY_REASON, "any reason at all");
    let mut rows: Vec<Row> = JUDGMENTS.iter().map(|j| Row::new(j.id, j.tag)).collect();
    let mut result = Agreement {
        marked_runs: by_run.len(),
        bad_runs: 0,
        cleared_runs: 0,
        untagged_runs: 0,
        unjudged_runs: 0,
        same_evidence: 0,
        changed_evidence: 0,
        rows: Vec::new(),
    };
    for (run, run_marks) in &by_run {
        let bad = run_marks.iter().any(|mark| mark.verdict == Verdict::Bad);
        let tags: BTreeSet<&str> = run_marks
            .iter()
            .filter(|mark| mark.verdict == Verdict::Bad)
            .flat_map(|mark| mark.tags.iter().map(String::as_str))
            .collect();
        if bad {
            result.bad_runs += 1;
            if tags.is_empty() {
                result.untagged_runs += 1;
            }
        } else {
            result.cleared_runs += 1;
        }
        let answer = answer_for(run_marks, store, current, run);
        match answer {
            None => result.unjudged_runs += 1,
            Some((_, true)) => result.same_evidence += 1,
            Some((_, false)) => result.changed_evidence += 1,
        }
        let answer = answer.map(|(answer, _)| answer);
        any.count(
            bad,
            answer.map(|answer| answer.nouls.values().any(|p| *p >= REASON_AT)),
        );
        for row in &mut rows {
            // An answer that lacks this judgment counts as no answer.
            let said = answer.and_then(|answer| answer.nouls.get(&row.id).map(|p| *p >= REASON_AT));
            if tags.contains(row.id.as_str()) {
                row.count(true, said);
            } else if !bad {
                row.count(false, said);
            }
        }
    }
    result.rows = std::iter::once(any).chain(rows).collect();
    result
}

/// The agreement as JSON for `gym runs agreement --json`.
#[must_use]
pub fn agreement_json(agreement: &Agreement) -> Value {
    json!({
        "schema": AGREEMENT_SCHEMA,
        "questions": learning::QUESTION_SET,
        "questions_digest": learning::questions_digest(),
        "model": learning::JEV_MODEL,
        "reason_at": REASON_AT,
        "min_labels": MIN_LABELS,
        "labels": "a tag on a bad mark is a positive label for its judgment on that run; a cleared run is a negative label for every judgment; any_reason takes every bad run as positive",
        "marked_runs": agreement.marked_runs,
        "bad_runs": agreement.bad_runs,
        "cleared_runs": agreement.cleared_runs,
        "untagged_runs": agreement.untagged_runs,
        "unjudged_runs": agreement.unjudged_runs,
        "same_evidence": agreement.same_evidence,
        "changed_evidence": agreement.changed_evidence,
        "unsupported": agreement.unsupported(),
        "rows": agreement.rows.iter().map(Row::to_json).collect::<Vec<_>>(),
    })
}

/// The agreement in words. The first lines say how many judgments have
/// too few labels, before any number.
#[must_use]
pub fn agreement_text(agreement: &Agreement) -> Vec<String> {
    let mut lines = Vec::new();
    let unsupported = agreement.unsupported();
    if agreement.marked_runs == 0 {
        lines.push(
            "No marks yet, so there is nothing to measure Jev against. Mark runs with `gym runs mark RUN --tag ID` or clear them with `gym runs mark RUN --clear`."
                .to_owned(),
        );
        return lines;
    }
    if unsupported > 0 {
        lines.push(format!(
            "Too few labels: {unsupported} of {} rows lack the {MIN_LABELS} positive and {MIN_LABELS} negative labels with Jev answers a number needs, so they show counts only.",
            agreement.rows.len()
        ));
    } else {
        lines.push(format!(
            "Every row has at least {MIN_LABELS} positive and {MIN_LABELS} negative labels with Jev answers."
        ));
    }
    lines.push(format!(
        "Jev ({}, {} {}) against the marks on {} runs: {} marked bad ({} with no tag), {} cleared, {} with no Jev answer.",
        learning::JEV_MODEL,
        learning::QUESTION_SET,
        &learning::questions_digest()[..12],
        agreement.marked_runs,
        agreement.bad_runs,
        agreement.untagged_runs,
        agreement.cleared_runs,
        agreement.unjudged_runs,
    ));
    if agreement.changed_evidence > 0 {
        lines.push(format!(
            "{} of the answers are to evidence that changed after the mark; {} are to the evidence the person saw.",
            agreement.changed_evidence, agreement.same_evidence
        ));
    }
    lines.push(format!(
        "Jev says yes at {REASON_AT:.2} or above. A tag is a positive label for its judgment; a cleared run is a negative for every judgment. Intervals are 95% Wilson."
    ));
    lines.push(String::new());
    lines.push(format!(
        "{:<26} {:>4} {:>4}  {:<24} {:<24} {:<24}",
        "judgment", "pos", "neg", "agreement", "precision", "recall"
    ));
    for row in &agreement.rows {
        let head = format!(
            "{:<26} {:>4} {:>4}  ",
            clip_words(&row.id, 26),
            row.positive,
            row.negative
        );
        if row.supported() {
            lines.push(format!(
                "{head}{:<24} {:<24} {:<24}",
                row.agreement().text(),
                row.precision().text(),
                row.recall().text()
            ));
        } else if row.positive + row.negative + row.unjudged > 0 {
            lines.push(format!(
                "{head}too few labels{}",
                if row.unjudged > 0 {
                    format!("; {} labeled runs have no Jev answer", row.unjudged)
                } else {
                    String::new()
                }
            ));
        } else {
            lines.push(format!("{head}no labels"));
        }
    }
    lines
}

// ---------------------------------------------------------------------------
// The command line
// ---------------------------------------------------------------------------

/// The usage of the marking subcommands.
pub const USAGE: &str = "\
gym runs mark, unmark, marks, agreement: a person's marks on runs, and Jev against them.

Usage:
  gym runs mark RUN[/STEP] [--tag ID]... [--note TEXT] [--author NAME]
  gym runs mark RUN --clear [--note TEXT] [--author NAME]
  gym runs unmark RUN[/STEP]
  gym runs marks [--json]
  gym runs agreement [--json]
  gym runs --marked

`mark` records that a run, or step STEP of its transcript, is bad. STEP
counts from 1, as `gym runs show RUN --json` numbers the steps. Each --tag
names a runs-learning-v1 judgment the mark shows, such as unearned_success,
looped, or harness_fault. --note keeps a one-line note. --clear records
that you read the run and found nothing wrong. A new mark on the same run or
step replaces the old one; `unmark` removes it. --author defaults to $USER.

Marks are appended to ~/.openagents/gym/marks/marks.jsonl, each with the
digest of the evidence Jev reads for the run, and never edit the run.
--marks-dir PATH keeps them elsewhere.

`agreement` measures Jev's judgments against the marks: a tag is a positive
label for its judgment on that run, and a cleared run is a negative label
for every judgment. It reports agreement, precision, and recall with 95%
Wilson intervals and their denominators, and gives no number for a judgment
with fewer than 5 positive and 5 negative labels. It reads the answers
`gym runs rank` keeps and asks Jev nothing.

The source flags --jobs-dir, --traces-dir, --no-jobs, --no-traces,
--no-tasks, --learning-dir, and --no-reference work as they do for
`gym runs`.";

/// Whether `word` names one of this module's subcommands.
#[must_use]
pub fn handles(word: &str) -> bool {
    matches!(word, "mark" | "unmark" | "marks" | "agreement")
}

/// `gym runs mark`, `unmark`, `marks`, and `agreement`.
///
/// # Errors
///
/// Returns the usage text when the arguments don't parse, and a message
/// when the run isn't found or the store can't be written.
pub fn command(args: &[String], out: &mut impl Write) -> Result<i32, String> {
    let Some(sub) = args
        .first()
        .map(String::as_str)
        .filter(|word| handles(word))
    else {
        return Err(USAGE.to_owned());
    };
    let mut sources = Sources::standard();
    let mut marks_dir = default_dir();
    let mut learning_dir = learning::default_dir();
    let mut reference = true;
    let mut target: Option<String> = None;
    let mut tags = Vec::new();
    let mut note: Option<String> = None;
    let mut author = default_author();
    let (mut clear, mut json_out) = (false, false);
    let mut index = 1;
    let value = |index: usize| {
        args.get(index + 1)
            .cloned()
            .ok_or_else(|| format!("{} needs a value\n\n{USAGE}", args[index]))
    };
    while index < args.len() {
        match args[index].as_str() {
            "--tag" => {
                tags.extend(
                    value(index)?
                        .split(',')
                        .map(|tag| tag.trim().to_owned())
                        .filter(|tag| !tag.is_empty()),
                );
                index += 1;
            }
            "--note" => {
                note = Some(value(index)?);
                index += 1;
            }
            "--author" => {
                author = value(index)?;
                index += 1;
            }
            "--clear" => clear = true,
            "--json" => json_out = true,
            "--marks-dir" => {
                marks_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--learning-dir" => {
                learning_dir = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--jobs-dir" => {
                sources.jobs = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--traces-dir" => {
                sources.traces = Some(PathBuf::from(value(index)?));
                index += 1;
            }
            "--no-jobs" => sources.jobs = None,
            "--no-traces" => sources.traces = None,
            "--no-tasks" => sources.tasks.clear(),
            "--no-reference" => reference = false,
            "--help" | "-h" => {
                writeln!(out, "{USAGE}").map_err(|e| e.to_string())?;
                return Ok(0);
            }
            other if !other.starts_with("--") && target.is_none() => {
                target = Some(other.to_owned());
            }
            other => return Err(format!("unknown argument {other}\n\n{USAGE}")),
        }
        index += 1;
    }
    let write =
        |out: &mut dyn Write, text: &str| writeln!(out, "{text}").map_err(|e| e.to_string());
    let mut marks = Marks::open(marks_dir);
    for error in &marks.errors {
        eprintln!("gym runs: {error}");
    }
    match sub {
        "marks" => {
            let catalog = Catalog::load(sources);
            if json_out {
                let value = json!({
                    "schema": MARKS_SCHEMA,
                    "file": marks.path(),
                    "records": marks.records().len(),
                    "errors": marks.errors,
                    "marks": marks.all().map(|mark| {
                        let mut value = mark.to_json();
                        value["task"] = json!(catalog.runs.iter().find(|run| run.id() == mark.run).map(|run| run.task.clone()));
                        value
                    }).collect::<Vec<_>>(),
                });
                write(
                    out,
                    &serde_json::to_string_pretty(&value).map_err(|e| e.to_string())?,
                )?;
                return Ok(0);
            }
            for line in marks_text(&marks, &catalog) {
                write(out, &line)?;
            }
            Ok(0)
        }
        "agreement" => {
            let catalog = Catalog::load(sources);
            let context = Context::new(
                &catalog,
                reference
                    .then(crate::terminal_bench_reference::Reference::checked)
                    .flatten(),
            );
            let store = Store::open(learning_dir);
            let current = learning::answers(&catalog, &store, &context);
            let result = agreement(&marks, &store, &current);
            if json_out {
                write(
                    out,
                    &serde_json::to_string_pretty(&agreement_json(&result))
                        .map_err(|e| e.to_string())?,
                )?;
            } else {
                for line in agreement_text(&result) {
                    write(out, &line)?;
                }
            }
            Ok(0)
        }
        "unmark" => {
            let target = target.ok_or_else(|| format!("unmark needs a run\n\n{USAGE}"))?;
            let (name, step) = parse_target(&target);
            // A mark can outlive its run's records, so an exact id works
            // without the catalog.
            let run = if marks.is_marked(&name) {
                name
            } else {
                let catalog = Catalog::load(sources);
                catalog
                    .find(&name)
                    .map(Run::id)
                    .ok_or_else(|| format!("no run matches {name}"))?
            };
            let shown = match step {
                Some(step) => format!("{run} step {step}"),
                None => run.clone(),
            };
            if marks.unmark(&run, step, &author, now_ms())? {
                write(out, &format!("Removed the mark on {shown}."))?;
            } else {
                write(out, &format!("{shown} has no mark to remove."))?;
            }
            Ok(0)
        }
        _ => {
            let target = target.ok_or_else(|| format!("mark needs a run\n\n{USAGE}"))?;
            let (name, step) = parse_target(&target);
            let catalog = Catalog::load(sources);
            let run = catalog
                .find(&name)
                .ok_or_else(|| format!("no run matches {name}"))?;
            let detail = Detail::load(run);
            let steps = detail.transcript.blocks.len();
            if let Some(step) = step
                && (step == 0 || step > steps)
            {
                return Err(format!(
                    "{} has {steps} transcript steps, counted from 1; there is no step {step}",
                    run.id()
                ));
            }
            let context = Context::new(
                &catalog,
                reference
                    .then(crate::terminal_bench_reference::Reference::checked)
                    .flatten(),
            );
            let mark = marks.mark(NewMark {
                run: run.id(),
                step,
                verdict: if clear { Verdict::Clear } else { Verdict::Bad },
                tags,
                note,
                evidence: Some(evidence_key(&detail, &context)),
                author,
                at_ms: now_ms(),
            })?;
            if json_out {
                write(
                    out,
                    &serde_json::to_string_pretty(&mark.to_json()).map_err(|e| e.to_string())?,
                )?;
            } else {
                write(out, &format!("{}: {}", run.id(), mark.describe()))?;
            }
            Ok(0)
        }
    }
}

/// The marks in words, newest first, for `gym runs marks`.
#[must_use]
pub fn marks_text(marks: &Marks, catalog: &Catalog) -> Vec<String> {
    let mut lines = Vec::new();
    if marks.is_empty() {
        lines.push(format!(
            "No marks yet{}. `gym runs mark RUN[/STEP] --tag ID --note TEXT` adds one.",
            marks
                .path()
                .map(|path| format!(" in {}", path.display()))
                .unwrap_or_default()
        ));
        return lines;
    }
    let mut all: Vec<&Mark> = marks.all().collect();
    all.sort_by(|a, b| b.at_ms.cmp(&a.at_ms).then(a.target().cmp(&b.target())));
    let runs: BTreeSet<&str> = all.iter().map(|mark| mark.run.as_str()).collect();
    lines.push(format!(
        "{} marks on {} runs, newest first.",
        all.len(),
        runs.len()
    ));
    lines.push(String::new());
    for mark in all {
        let task = catalog
            .runs
            .iter()
            .find(|run| run.id() == mark.run)
            .map_or("(run no longer found)", |run| run.task.as_str());
        lines.push(format!("{} {task}  {}", mark.verdict.flag(), mark.target()));
        lines.push(format!("    {}", mark.describe()));
    }
    lines
}

/// The marks on one run as lines for its story: nothing when unmarked.
#[must_use]
pub fn story_lines(marks: &Marks, run: &str) -> Vec<String> {
    marks.of_run(run).into_iter().map(Mark::describe).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    fn new(run: &str, step: Option<usize>, verdict: Verdict, tags: &[&str]) -> NewMark {
        NewMark {
            run: run.to_owned(),
            step,
            verdict,
            tags: tags.iter().map(|tag| (*tag).to_owned()).collect(),
            note: None,
            evidence: None,
            author: "tester".to_owned(),
            at_ms: 1_790_173_207_539,
        }
    }

    #[test]
    fn the_store_appends_replaces_removes_and_survives_a_reopen() {
        let dir = tempfile::tempdir().unwrap();
        let mut marks = Marks::open(Some(dir.path().to_path_buf()));
        assert!(marks.is_empty());
        let mut first = new("job/trial", None, Verdict::Bad, &["looped"]);
        first.note = Some("  ran the   same test nine times ".to_owned());
        let mark = marks.mark(first).unwrap();
        assert_eq!(mark.note.as_deref(), Some("ran the same test nine times"));
        marks
            .mark(new("job/trial", Some(3), Verdict::Bad, &["harness_fault"]))
            .unwrap();
        // A new mark on the same target replaces the old one.
        marks
            .mark(new("job/trial", None, Verdict::Bad, &["near_miss"]))
            .unwrap();
        assert_eq!(marks.len(), 2);
        assert_eq!(
            marks.get("job/trial", None).unwrap().tags,
            vec!["near_miss"]
        );

        let again = Marks::open(Some(dir.path().to_path_buf()));
        assert!(again.errors.is_empty(), "{:?}", again.errors);
        assert_eq!(again.records().len(), 3);
        assert_eq!(again.of_run("job/trial").len(), 2);
        assert_eq!(again.of_run("job/trial")[0].step, None);
        assert_eq!(
            again.steps("job/trial").keys().copied().collect::<Vec<_>>(),
            vec![3]
        );
        assert_eq!(again.verdict("job/trial"), Some(Verdict::Bad));

        // Unmarking appends; nothing is rewritten.
        assert!(marks.unmark("job/trial", Some(3), "tester", 1).unwrap());
        assert!(!marks.unmark("job/trial", Some(3), "tester", 2).unwrap());
        let again = Marks::open(Some(dir.path().to_path_buf()));
        assert_eq!(again.records().len(), 4);
        assert_eq!(again.len(), 1);
        let text = std::fs::read_to_string(dir.path().join(FILE)).unwrap();
        assert_eq!(text.lines().count(), 4);
        assert!(text.contains("\"action\":\"unmark\""), "{text}");
    }

    #[test]
    fn an_edited_line_breaks_its_digest() {
        let dir = tempfile::tempdir().unwrap();
        let mut marks = Marks::open(Some(dir.path().to_path_buf()));
        marks
            .mark(new("job/trial", None, Verdict::Bad, &["looped"]))
            .unwrap();
        marks
            .mark(new("job/other", None, Verdict::Clear, &[]))
            .unwrap();
        let path = dir.path().join(FILE);
        let text = std::fs::read_to_string(&path).unwrap();
        std::fs::write(&path, text.replacen("looped", "near_miss", 1)).unwrap();
        let again = Marks::open(Some(dir.path().to_path_buf()));
        assert_eq!(again.errors.len(), 1, "{:?}", again.errors);
        assert!(again.errors[0].contains("digest"), "{:?}", again.errors);
    }

    #[test]
    fn marks_refuse_what_they_cannot_mean() {
        let mut marks = Marks::open(None);
        assert!(marks.mark(new("r", None, Verdict::Bad, &["nope"])).is_err());
        assert!(marks.mark(new("r", Some(2), Verdict::Clear, &[])).is_err());
        assert!(
            marks
                .mark(new("r", None, Verdict::Clear, &["looped"]))
                .is_err()
        );
        let mark = marks
            .mark(new("r", None, Verdict::Bad, &["looped", "looped"]))
            .unwrap();
        assert_eq!(mark.tags, vec!["looped"]);
    }

    #[test]
    fn a_target_splits_off_a_numbered_step() {
        assert_eq!(
            parse_target("job/trial/12"),
            ("job/trial".to_owned(), Some(12))
        );
        assert_eq!(parse_target("job/trial"), ("job/trial".to_owned(), None));
        assert_eq!(
            parse_target("coq-block-bound/3"),
            ("coq-block-bound".to_owned(), Some(3))
        );
        assert_eq!(
            parse_target("coq-block-bound"),
            ("coq-block-bound".to_owned(), None)
        );
    }

    fn answer(run: &str, key: &str, yes: &[&str]) -> Answer {
        let mut answer = Answer::from_answers(run, key.to_owned(), Value::Null, &json!({}));
        for judgment in &JUDGMENTS {
            let p = if yes.contains(&judgment.id) { 0.9 } else { 0.1 };
            answer.nouls.insert(judgment.id.to_owned(), p);
        }
        answer
    }

    #[test]
    fn agreement_counts_labels_and_says_when_they_are_too_few() {
        let mut marks = Marks::open(None);
        let mut store = Store::open(None);
        // Six runs tagged looped: Jev agrees on five.
        for n in 0..6 {
            let run = format!("bad/{n}");
            let mut mark = new(&run, None, Verdict::Bad, &["looped"]);
            mark.evidence = Some(format!("key-{run}"));
            marks.mark(mark).unwrap();
            let yes: &[&str] = if n < 5 { &["looped"] } else { &[] };
            store
                .insert(answer(&run, &format!("key-{run}"), yes))
                .unwrap();
        }
        // Six cleared runs: Jev says looped on one.
        for n in 0..6 {
            let run = format!("ok/{n}");
            let mut mark = new(&run, None, Verdict::Clear, &[]);
            mark.evidence = Some(format!("key-{run}"));
            marks.mark(mark).unwrap();
            let yes: &[&str] = if n == 0 { &["looped"] } else { &[] };
            store
                .insert(answer(&run, &format!("key-{run}"), yes))
                .unwrap();
        }
        // A step mark with a tag, no Jev answer at all.
        marks
            .mark(new("unjudged/0", Some(4), Verdict::Bad, &["near_miss"]))
            .unwrap();
        // A run whose answer is to newer evidence than the mark saw.
        let mut mark = new("moved/0", None, Verdict::Bad, &[]);
        mark.evidence = Some("old-key".to_owned());
        marks.mark(mark).unwrap();
        let moved = answer("moved/0", "new-key", &["near_miss"]);
        let current: HashMap<String, &Answer> = [("moved/0".to_owned(), &moved)].into();

        let result = agreement(&marks, &store, &current);
        assert_eq!(result.marked_runs, 14);
        assert_eq!(result.bad_runs, 8);
        assert_eq!(result.cleared_runs, 6);
        assert_eq!(result.untagged_runs, 1);
        assert_eq!(result.unjudged_runs, 1);
        assert_eq!(result.same_evidence, 12);
        assert_eq!(result.changed_evidence, 1);

        let row = |id: &str| result.rows.iter().find(|row| row.id == id).unwrap();
        let looped = row("looped");
        assert!(looped.supported());
        assert_eq!((looped.positive, looped.negative), (6, 6));
        assert_eq!(looped.agreement(), Proportion { hits: 10, of: 12 });
        assert_eq!(looped.precision(), Proportion { hits: 5, of: 6 });
        assert_eq!(looped.recall(), Proportion { hits: 5, of: 6 });
        let (low, high) = looped.agreement().interval().unwrap();
        assert!(low > 0.5 && low < 0.6 && high > 0.95, "{low} {high}");

        // near_miss has one unjudged positive and six negatives.
        let near = row("near_miss");
        assert!(!near.supported());
        assert_eq!((near.positive, near.negative, near.unjudged), (0, 6, 1));
        assert_eq!(near.recall().value(), None, "unknown is not zero");
        assert!(
            near.too_few()
                .unwrap()
                .contains("0 positive and 6 negative")
        );

        let any = row(ANY_REASON);
        assert_eq!((any.positive, any.negative, any.unjudged), (7, 6, 1));
        assert!(any.supported());

        let text = agreement_text(&result);
        assert!(text[0].starts_with("Too few labels:"), "{text:#?}");
        let looped_line = text
            .iter()
            .find(|line| line.starts_with("looped "))
            .unwrap();
        assert!(looped_line.contains("0.83 10/12 ["), "{looped_line}");
        let near_line = text
            .iter()
            .find(|line| line.starts_with("near_miss "))
            .unwrap();
        assert!(near_line.contains("too few labels"), "{near_line}");
        assert!(!near_line.contains("0.00"), "{near_line}");

        let value = agreement_json(&result);
        assert_eq!(value["schema"], AGREEMENT_SCHEMA);
        let near = value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "near_miss")
            .unwrap();
        assert_eq!(near["supported"], false);
        assert_eq!(near["recall"]["value"], Value::Null);
        assert_eq!(near["recall"]["of"], 0);
    }

    #[test]
    fn the_command_line_marks_filters_shows_and_measures() {
        let (dir, _) = crate::runs::fixture_sources();
        let state = tempfile::tempdir().unwrap();
        let recorded = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures/runs-learning/recorded.json")
            .display()
            .to_string();
        let base = [
            "--jobs-dir".to_owned(),
            dir.path().join("jobs").display().to_string(),
            "--traces-dir".to_owned(),
            dir.path().join("traces").display().to_string(),
            "--no-tasks".to_owned(),
            "--no-reference".to_owned(),
            "--learning-dir".to_owned(),
            state.path().join("learning").display().to_string(),
            "--marks-dir".to_owned(),
            state.path().join("marks").display().to_string(),
        ];
        let run = |extra: &[&str]| -> Result<String, String> {
            let mut args: Vec<String> = extra.iter().map(|s| (*s).to_owned()).collect();
            args.extend(base.iter().cloned());
            let mut out = Vec::new();
            crate::runs::command(&args, &mut out)?;
            Ok(String::from_utf8(out).unwrap())
        };
        let json =
            |extra: &[&str]| -> Value { serde_json::from_str(&run(extra).unwrap()).unwrap() };
        let text = run(&["rank", "--recorded", &recorded]).unwrap();
        assert!(text.contains("4 answered"), "{text}");

        let text = run(&["agreement"]).unwrap();
        assert!(text.starts_with("No marks yet"), "{text}");

        let text = run(&[
            "mark",
            "wal-recovery-ordering",
            "--tag",
            "unearned_success",
            "--tag",
            "near_miss",
            "--note",
            "said the tests passed; two failed",
            "--author",
            "tester",
        ])
        .unwrap();
        assert!(
            text.contains("Marked bad: unearned_success, near_miss — \"said the tests passed; two failed\" (tester,"),
            "{text}"
        );
        run(&[
            "mark",
            "coq-block-bound/2",
            "--tag",
            "looped",
            "--author",
            "tester",
        ])
        .unwrap();
        run(&[
            "mark",
            "cancel-async-tasks",
            "--clear",
            "--author",
            "tester",
        ])
        .unwrap();
        let error = run(&["mark", "coq-block-bound/999"]).unwrap_err();
        assert!(error.contains("no step 999"), "{error}");
        assert!(run(&["mark", "coq-block-bound", "--tag", "nope"]).is_err());
        assert!(run(&["mark", "coq-block-bound/2", "--clear"]).is_err());

        // The list filters to marked runs and shows each mark.
        let value = json(&["--marked", "--json"]);
        assert_eq!(value["shown"], 3, "{value}");
        assert_eq!(value["marked"], 3, "{value}");
        let text = run(&["--marked"]).unwrap();
        assert!(text.contains("Showing: marked runs only"), "{text}");
        assert!(
            text.contains("⚑ Step 2 marked bad: looped (tester,"),
            "{text}"
        );
        assert!(text.contains("⚐ Cleared: nothing wrong (tester,"), "{text}");

        // The story and the transcript show the marks.
        let text = run(&["show", "coq-block-bound", "--transcript"]).unwrap();
        assert!(
            text.contains("Marks\n  Step 2 marked bad: looped"),
            "{text}"
        );
        let lines: Vec<&str> = text.lines().collect();
        let at = lines
            .iter()
            .position(|line| line.trim_start().starts_with("⚑ Step 2 marked bad"))
            .expect("the mark under its step");
        let transcript = lines.iter().position(|line| *line == "Transcript").unwrap();
        assert!(at > transcript, "{text}");
        let value = json(&["show", "wal-recovery-ordering", "--json"]);
        assert_eq!(
            value["marks"][0]["tags"],
            json!(["unearned_success", "near_miss"])
        );
        assert_eq!(
            value["marks"][0]["evidence"], value["learning"]["key"],
            "{value}"
        );

        let value = json(&["marks", "--json"]);
        assert_eq!(value["schema"], MARKS_SCHEMA);
        assert_eq!(value["marks"].as_array().unwrap().len(), 3);
        let text = run(&["marks"]).unwrap();
        assert!(text.contains("3 marks on 3 runs, newest first."), "{text}");

        // Jev against the marks: every answer is to the evidence marked.
        let value = json(&["agreement", "--json"]);
        assert_eq!(value["schema"], AGREEMENT_SCHEMA);
        assert_eq!(value["marked_runs"], 3);
        assert_eq!(value["bad_runs"], 2);
        assert_eq!(value["cleared_runs"], 1);
        assert_eq!(value["same_evidence"], 3, "{value}");
        let unearned = value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "unearned_success")
            .unwrap()
            .clone();
        assert_eq!(unearned["labels"]["positive"], 1, "{unearned}");
        assert_eq!(unearned["labels"]["negative"], 1, "{unearned}");
        assert_eq!(unearned["confusion"]["true_positive"], 1, "{unearned}");
        assert_eq!(unearned["supported"], false);
        let text = run(&["agreement"]).unwrap();
        assert!(text.starts_with("Too few labels:"), "{text}");

        // Unmarking removes the mark; a second unmark finds nothing.
        let text = run(&["unmark", "coq-block-bound/2"]).unwrap();
        assert!(text.contains("Removed the mark on"), "{text}");
        let text = run(&["unmark", "coq-block-bound/2"]).unwrap();
        assert!(text.contains("has no mark to remove"), "{text}");
        assert_eq!(json(&["--marked", "--json"])["shown"], 2);
    }

    #[test]
    fn with_no_marks_the_report_says_so() {
        let result = agreement(&Marks::open(None), &Store::open(None), &HashMap::new());
        let text = agreement_text(&result);
        assert!(text[0].starts_with("No marks yet"), "{text:?}");
    }
}
