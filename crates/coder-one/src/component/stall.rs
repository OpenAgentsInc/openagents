//! `control.stall` and `control.next` as components, and their offline
//! replay over retained Microluna dispatches (issue #9627).
//!
//! A fixture holds one [`Checkpoint`] with the task it was taken on and,
//! when it came from a retained attempt, its [`Hindsight`] label. The
//! components ask the one Jev request a checkpoint makes and report the
//! stall call or the next-step pick against the label.
//!
//! [`replay`] reads every retained dispatch under the traces it is given,
//! takes every checkpoint ([`crate::stall::checkpoints`]), and writes the
//! inputs, the labels by partition, and the rows of answers and calls
//! apart, so the evaluation labels can stay unread until the thresholds
//! are frozen. `--inputs` replays from a retained inputs file instead of
//! the traces. The partition of each task comes from a split file, never
//! from this code: the task names stay out of the binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::jev::{Ask, JevMode, Recorded, USD_PER_MILLION_INPUT};
use super::{Component, Fixture, Ran, input};
use crate::record::{Implementation, Recorder};
use crate::stall::{
    self, Answers, Checkpoint, Hindsight, NEXT_COMPONENT, STALL_COMPONENT, Session,
};

/// The schema of a replay's rows, inputs, and labels.
pub const ROW_SCHEMA: &str = "openagents.coder-one.stall-row.v1";

/// The schema of a split file.
pub const SPLIT_SCHEMA: &str = "openagents.coder-one.stall-split.v1";

/// A fixture's input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CheckpointInput {
    pub task: String,
    pub checkpoint: Checkpoint,
}

async fn answers(
    input: &CheckpointInput,
    jev: &JevMode,
    recorder: &Recorder,
    component: &str,
) -> (Answers, &'static str, Option<u64>) {
    let (state, questions) = stall::request(&input.task, &input.checkpoint);
    let asked = super::jev::ask(
        jev,
        recorder,
        Ask {
            component,
            name: stall::DECISION,
            id: format!(
                "jev-stall-{}-{}",
                input.checkpoint.session, input.checkpoint.turn
            ),
            state,
            questions,
            parent: None,
            deadline: None,
        },
    )
    .await;
    (Answers::from_asked(&asked), asked.how, asked.input_tokens)
}

fn hindsight_of(fixture: &Fixture) -> Option<Hindsight> {
    serde_json::from_value(fixture.retained.get("hindsight")?.clone()).ok()
}

/// `control.stall`.
pub struct StallComponent;

impl Component for StallComponent {
    fn id(&self) -> &'static str {
        STALL_COMPONENT
    }
    fn implementation(&self) -> Implementation {
        stall::implementation(stall::Params::default())
    }
    fn about(&self) -> &'static str {
        "Code features and Jev judge whether a Microluna session has stalled; code picks the action."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: CheckpointInput = input(fixture)?;
            let (answers, how, _) = answers(&input, jev, recorder, self.id()).await;
            let features = &input.checkpoint.features;
            let verdict = stall::decide(features, &answers, stall::Params::default());
            let mut metrics = Map::new();
            metrics.insert("suspect".to_string(), json!(verdict.suspect));
            metrics.insert("stalled".to_string(), json!(verdict.stalled));
            metrics.insert("jev_stalled".to_string(), json!(verdict.jev));
            metrics.insert("p_progress".to_string(), json!(answers.progress));
            metrics.insert("p_repeating".to_string(), json!(answers.repeating));
            metrics.insert("p_done_report_only".to_string(), json!(answers.done));
            if let Some(seen) = hindsight_of(fixture).filter(|h| h.labeled) {
                metrics.insert("label_stall".to_string(), json!(seen.stall));
                metrics.insert(
                    "matches_label".to_string(),
                    json!(verdict.stalled == seen.stall),
                );
            }
            Ok(Ran {
                output: json!({
                    "verdict": verdict,
                    "answers": answers,
                    "jev": how,
                    "rebrief": verdict.stalled.then(|| stall::rebrief_note(&input.checkpoint)),
                }),
                metrics,
            })
        })
    }
}

/// `control.next`.
pub struct NextComponent;

impl Component for NextComponent {
    fn id(&self) -> &'static str {
        NEXT_COMPONENT
    }
    fn implementation(&self) -> Implementation {
        stall::implementation(stall::Params::default())
    }
    fn about(&self) -> &'static str {
        "Code proposes next steps from the session's state and Jev's Choice picks one; Luna writes the edit."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: CheckpointInput = input(fixture)?;
            let (answers, how, _) = answers(&input, jev, recorder, self.id()).await;
            let mut metrics = Map::new();
            metrics.insert(
                "candidates".to_string(),
                json!(input.checkpoint.candidates.len()),
            );
            metrics.insert("pick".to_string(), json!(answers.next));
            metrics.insert("pick_p".to_string(), json!(answers.next_p));
            if let Some(seen) = hindsight_of(fixture) {
                metrics.insert("next_kind".to_string(), json!(seen.next_kind));
                metrics.insert(
                    "pick_matches_next".to_string(),
                    json!(answers.next.as_deref() == Some(seen.next_kind.as_str())),
                );
                metrics.insert("productive_next".to_string(), json!(seen.productive_next));
            }
            Ok(Ran {
                output: json!({
                    "pick": answers.next,
                    "p": answers.next_p,
                    "jev": how,
                    "note": stall::next_note(&input.checkpoint, &answers),
                }),
                metrics,
            })
        })
    }
}

/// Which tasks are calibration and which evaluation, from a split file.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Split {
    pub schema: String,
    pub calibration: Vec<String>,
    pub evaluation: Vec<String>,
}

impl Split {
    /// Reads a split file.
    ///
    /// # Errors
    ///
    /// Returns a message when the file doesn't read.
    pub fn load(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
        let split: Split = serde_json::from_str(&text)
            .map_err(|error| format!("{} is not a split file: {error}", path.display()))?;
        if split.schema != SPLIT_SCHEMA {
            return Err(format!("{} is not a {SPLIT_SCHEMA} file", path.display()));
        }
        Ok(split)
    }

    /// The partition of `task`: `calibration`, `evaluation`, or
    /// `excluded`.
    #[must_use]
    pub fn partition(&self, task: &str) -> &'static str {
        if self.calibration.iter().any(|t| t == task) {
            "calibration"
        } else if self.evaluation.iter().any(|t| t == task) {
            "evaluation"
        } else {
            "excluded"
        }
    }
}

/// One retained dispatch set: an attempt's Microluna sessions.
#[derive(Clone, Debug)]
pub struct Trial {
    /// The trial's directory name, such as `<task>__<id>`.
    pub id: String,
    pub task: String,
    pub passed: Option<bool>,
    pub sessions: Vec<Session>,
    /// The host's score after each session, by session from 1.
    pub host_scores: Vec<(usize, (u64, u64))>,
    /// Every file read, with its SHA-256.
    pub sources: Vec<(String, String)>,
}

/// `microluna-<d>-<n>.atif.jsonl` as `(d, n)`.
pub(crate) fn session_number(name: &str) -> Option<(u32, u32)> {
    let rest = name
        .strip_prefix("microluna-")?
        .strip_suffix(".atif.jsonl")?;
    let (d, n) = rest.split_once('-')?;
    Some((d.parse().ok()?, n.parse().ok()?))
}

/// The trial directory and name an `artifacts` directory belongs to.
pub(crate) fn trial_of(artifacts: &Path) -> Option<(PathBuf, String)> {
    let parent = artifacts.parent()?;
    let name = parent.file_name()?.to_string_lossy().into_owned();
    if name == "episode" && parent.parent()?.file_name()? == "agent" {
        let trial = parent.parent()?.parent()?;
        return Some((
            trial.to_path_buf(),
            trial.file_name()?.to_string_lossy().into_owned(),
        ));
    }
    let stem = name.strip_suffix(".episode")?;
    Some((parent.to_path_buf(), stem.to_string()))
}

/// Every `artifacts` directory under `root` holding Microluna session logs,
/// leaving out interrupted copies and live mirrors.
pub(crate) fn artifact_dirs(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if !path.is_dir()
                || name.contains("interrupt")
                || name == "live"
                || name == "produced"
                || name == "tbench"
            {
                continue;
            }
            if name == "artifacts"
                && std::fs::read_dir(&path)
                    .into_iter()
                    .flatten()
                    .flatten()
                    .any(|f| session_number(&f.file_name().to_string_lossy()).is_some())
            {
                out.push(path);
            } else if depth < 5 {
                pending.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

fn sha(bytes: &[u8]) -> String {
    crate::accept::sha256(bytes)
}

/// Reads one trial from its `artifacts` directory.
fn read_trial(artifacts: &Path) -> Option<Trial> {
    let (dir, id) = trial_of(artifacts)?;
    let task = id.split("__").next()?.to_string();
    let mut sources = Vec::new();
    let reward = dir.join("verifier/reward.txt");
    let passed = std::fs::read(&reward).ok().and_then(|bytes| {
        sources.push((reward.display().to_string(), sha(&bytes)));
        String::from_utf8_lossy(&bytes)
            .trim()
            .parse::<f64>()
            .ok()
            .map(|r| r >= 1.0)
    });
    let mut files: Vec<((u32, u32), PathBuf)> = std::fs::read_dir(artifacts)
        .ok()?
        .flatten()
        .filter_map(|f| Some((session_number(&f.file_name().to_string_lossy())?, f.path())))
        .collect();
    files.sort();
    let mut sessions = Vec::new();
    let mut index: BTreeMap<(u32, u32), usize> = BTreeMap::new();
    for (number, path) in &files {
        let bytes = std::fs::read(path).ok()?;
        sources.push((path.display().to_string(), sha(&bytes)));
        sessions.push(stall::parse_session(&String::from_utf8_lossy(&bytes)));
        index.insert(*number, sessions.len());
    }
    let mut host_scores = Vec::new();
    for entry in std::fs::read_dir(artifacts).ok()?.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let Some(d) = name
            .strip_prefix("microluna-")
            .and_then(|r| r.strip_suffix(".json"))
            .and_then(|r| r.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(bytes) = std::fs::read(entry.path()) else {
            continue;
        };
        sources.push((entry.path().display().to_string(), sha(&bytes)));
        let Ok(record) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        for item in record["moves"].as_array().into_iter().flatten() {
            let (Some(n), Some(p), Some(t)) = (
                item["after_session"].as_u64(),
                item["score"]["passed"].as_u64(),
                item["score"]["total"].as_u64(),
            ) else {
                continue;
            };
            if let Some(s) = u32::try_from(n).ok().and_then(|n| index.get(&(d, n))) {
                host_scores.push((*s, (p, t)));
            }
        }
    }
    sources.sort();
    Some(Trial {
        id,
        task,
        passed,
        sessions,
        host_scores,
        sources,
    })
}

/// Every retained trial under `roots`, the first copy of each trial id
/// kept.
#[must_use]
pub fn discover(roots: &[PathBuf]) -> Vec<Trial> {
    let mut seen = std::collections::BTreeSet::new();
    let mut out = Vec::new();
    for root in roots {
        for artifacts in artifact_dirs(root) {
            if let Some(trial) = read_trial(&artifacts)
                && seen.insert(trial.id.clone())
            {
                out.push(trial);
            }
        }
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    out
}

/// One checkpoint's input row: what a replay needs, no label.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct InputRow {
    pub schema: String,
    pub trial: String,
    pub task: String,
    pub partition: String,
    pub input: CheckpointInput,
}

/// One checkpoint's label row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct LabelRow {
    pub schema: String,
    pub trial: String,
    pub task: String,
    pub partition: String,
    pub session: usize,
    pub turn: usize,
    pub at: stall::At,
    pub passed: Option<bool>,
    pub hindsight: Hindsight,
}

/// How the replay asks Jev.
pub struct ReplayJev {
    pub recorded: Recorded,
    /// A live client for recorded misses, until the limit.
    pub live: Option<::jev::Client>,
    pub live_limit: usize,
    /// Whether Jev is asked at all.
    pub on: bool,
}

/// What a replay wrote and spent.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Replayed {
    pub trials: usize,
    pub checkpoints: usize,
    pub by_partition: BTreeMap<String, usize>,
    pub jev: BTreeMap<String, usize>,
    pub live_input_tokens: u64,
    pub live_usd: f64,
}

fn write_lines<T: Serialize>(path: &Path, rows: &[T]) -> Result<(), String> {
    let mut text = String::new();
    for row in rows {
        text.push_str(&serde_json::to_string(row).map_err(|e| e.to_string())?);
        text.push('\n');
    }
    crate::record::write_atomic(path, text.as_bytes())
}

/// The input and label rows of every trial's checkpoints.
#[must_use]
pub fn extract(trials: &[Trial], split: &Split) -> (Vec<InputRow>, Vec<LabelRow>) {
    let mut inputs = Vec::new();
    let mut labels = Vec::new();
    for trial in trials {
        let partition = split.partition(&trial.task).to_string();
        let task = trial
            .sessions
            .iter()
            .find(|s| !s.task.is_empty())
            .map_or(String::new(), |s| s.task.clone());
        for checkpoint in stall::checkpoints(&trial.sessions) {
            let hindsight = stall::hindsight(
                &trial.sessions,
                &checkpoint,
                trial.passed == Some(true),
                &trial.host_scores,
            );
            labels.push(LabelRow {
                schema: ROW_SCHEMA.to_string(),
                trial: trial.id.clone(),
                task: trial.task.clone(),
                partition: partition.clone(),
                session: checkpoint.session,
                turn: checkpoint.turn,
                at: checkpoint.at,
                passed: trial.passed,
                hindsight,
            });
            inputs.push(InputRow {
                schema: ROW_SCHEMA.to_string(),
                trial: trial.id.clone(),
                task: trial.task.clone(),
                partition: partition.clone(),
                input: CheckpointInput {
                    task: task.clone(),
                    checkpoint,
                },
            });
        }
    }
    (inputs, labels)
}

/// Asks Jev at every input in `partitions` and writes `rows.jsonl`: each
/// checkpoint's features, answers, and calls, with no label. Returns the
/// counts and the recorder whose steps hold the live answers.
pub async fn score(
    inputs: &[InputRow],
    partitions: &[String],
    jev: &ReplayJev,
    out: &Path,
) -> Result<(Replayed, Recorder), String> {
    let recorder = Recorder::default();
    let recorded = JevMode::Recorded(jev.recorded.clone());
    let live = jev.live.clone().map(JevMode::Live);
    let mut replayed = Replayed::default();
    let mut rows = Vec::new();
    let mut live_used = 0;
    for row in inputs
        .iter()
        .filter(|r| partitions.iter().any(|p| p == "all" || *p == r.partition))
    {
        replayed.checkpoints += 1;
        *replayed
            .by_partition
            .entry(row.partition.clone())
            .or_default() += 1;
        let (mut answers, mut how, mut tokens) = if jev.on {
            self::answers(&row.input, &recorded, &recorder, STALL_COMPONENT).await
        } else {
            (Answers::default(), "off", None)
        };
        if how == "miss"
            && let Some(live) = &live
            && live_used < jev.live_limit
        {
            live_used += 1;
            (answers, how, tokens) =
                self::answers(&row.input, live, &recorder, STALL_COMPONENT).await;
            if how == "live" {
                replayed.live_input_tokens += tokens.unwrap_or(0);
            }
        }
        *replayed.jev.entry(how.to_string()).or_default() += 1;
        let checkpoint = &row.input.checkpoint;
        let verdict = stall::decide(&checkpoint.features, &answers, stall::Params::default());
        rows.push(json!({
            "schema": ROW_SCHEMA,
            "trial": row.trial,
            "task": row.task,
            "partition": row.partition,
            "session": checkpoint.session,
            "turn": checkpoint.turn,
            "at": checkpoint.at,
            "features": checkpoint.features,
            "quiet": checkpoint.features.quiet(),
            "looping": checkpoint.features.looping(),
            "turned_back": checkpoint.features.turned_back(),
            "suspect": checkpoint.features.suspect(),
            "strong": checkpoint.features.strong(),
            "candidates": checkpoint.candidates.iter().map(|c| c.kind.clone()).collect::<Vec<_>>(),
            "answers": answers,
            "jev": how,
            "input_tokens": tokens,
            "verdict": verdict,
        }));
    }
    replayed.trials = rows
        .iter()
        .filter_map(|r| r["trial"].as_str())
        .collect::<std::collections::BTreeSet<_>>()
        .len();
    replayed.live_usd = replayed.live_input_tokens as f64 * USD_PER_MILLION_INPUT / 1_000_000.0;
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    write_lines(&out.join("rows.jsonl"), &rows)?;
    Ok((replayed, recorder))
}

/// Extracts the checkpoints of every trial under `roots` into `out`:
/// `inputs.jsonl`, one `labels-<partition>.jsonl` per partition, and
/// `sources.json` with each file's SHA-256.
///
/// # Errors
///
/// Returns a message when a file can't be written.
pub fn extract_to(roots: &[PathBuf], split: &Split, out: &Path) -> Result<Vec<InputRow>, String> {
    let trials = discover(roots);
    let (inputs, labels) = extract(&trials, split);
    std::fs::create_dir_all(out).map_err(|e| e.to_string())?;
    write_lines(&out.join("inputs.jsonl"), &inputs)?;
    for partition in ["calibration", "evaluation", "excluded"] {
        let rows: Vec<&LabelRow> = labels.iter().filter(|l| l.partition == partition).collect();
        write_lines(&out.join(format!("labels-{partition}.jsonl")), &rows)?;
    }
    let sources: Vec<Value> = trials
        .iter()
        .map(|t| {
            json!({
                "trial": t.id,
                "task": t.task,
                "partition": split.partition(&t.task),
                "passed": t.passed,
                "sessions": t.sessions.len(),
                "turns": t.sessions.iter().map(|s| s.turns).sum::<usize>(),
                "files": t.sources.iter().map(|(p, h)| json!({"path": p, "sha256": h})).collect::<Vec<_>>(),
            })
        })
        .collect();
    let text = serde_json::to_string_pretty(&json!({ "schema": ROW_SCHEMA, "trials": sources }))
        .map_err(|e| e.to_string())?;
    crate::record::write_atomic(&out.join("sources.json"), format!("{text}\n").as_bytes())?;
    Ok(inputs)
}

/// Reads an inputs file.
///
/// # Errors
///
/// Returns a message when a line doesn't read.
pub fn load_inputs(path: &Path) -> Result<Vec<InputRow>, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| serde_json::from_str(l).map_err(|e| format!("{}: {e}", path.display())))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn session_files_and_trial_directories_read_in_both_layouts() {
        assert_eq!(session_number("microluna-1-12.atif.jsonl"), Some((1, 12)));
        assert_eq!(session_number("microluna-1.json"), None);
        let (dir, id) = trial_of(Path::new("/j/job/task-a__X1/agent/episode/artifacts")).unwrap();
        assert_eq!(id, "task-a__X1");
        assert_eq!(dir, Path::new("/j/job/task-a__X1"));
        let (dir, id) = trial_of(Path::new("/t/job/task-a__X1.episode/artifacts")).unwrap();
        assert_eq!(id, "task-a__X1");
        assert_eq!(dir, Path::new("/t/job/task-a__X1.episode"));
    }

    #[test]
    fn a_split_names_each_partition() {
        let split = Split {
            schema: SPLIT_SCHEMA.to_string(),
            calibration: vec!["a".to_string()],
            evaluation: vec!["b".to_string()],
        };
        assert_eq!(split.partition("a"), "calibration");
        assert_eq!(split.partition("b"), "evaluation");
        assert_eq!(split.partition("c"), "excluded");
    }

    #[tokio::test]
    async fn the_fixtures_replay_without_a_call() {
        let root = super::super::default_fixtures();
        for id in [STALL_COMPONENT, NEXT_COMPONENT] {
            let dirs = super::super::fixtures_for(&root, id);
            assert!(!dirs.is_empty(), "no fixture for {id}");
            let component = super::super::find(id).unwrap();
            let suite = super::super::suite(
                component.as_ref(),
                &dirs,
                &super::super::JevChoice::Recorded,
                &Recorder::default(),
                false,
            )
            .await
            .unwrap();
            for run in &suite.runs {
                assert!(run.error.is_none(), "{:?}", run.error);
                assert_eq!(run.jev.get("miss"), None, "{}", run.fixture);
            }
        }
    }
}
