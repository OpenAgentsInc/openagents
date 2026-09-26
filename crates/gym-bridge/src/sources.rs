//! Bounded, read-only projections of explicitly admitted host source roots.
use crate::{protocol::*, *};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::os::unix::fs::MetadataExt;
use std::{
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

const MAX_SCAN: usize = 512;
const MAX_FILE: u64 = 1024 * 1024;
const MAX_READ: u64 = 8 * 1024 * 1024;
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceKind {
    Microcoder,
    TerminalBench,
    TrainingSummaries,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub root: PathBuf,
    pub label: String,
    pub kind: SourceKind,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Root {
    source: Source,
    device: u64,
    inode: u64,
}
impl Root {
    pub fn admit(mut source: Source) -> Result<Self> {
        text(&source.label, 160)?;
        source.root = source
            .root
            .canonicalize()
            .map_err(|_| error(ErrorCode::Unavailable, "Gym source root is unavailable"))?;
        let m = std::fs::symlink_metadata(&source.root)
            .map_err(|_| error(ErrorCode::Unavailable, "Gym source root is unavailable"))?;
        if !m.is_dir() {
            return Err(error(
                ErrorCode::Forbidden,
                "Gym source root is not a directory",
            ));
        }
        Ok(Self {
            source,
            device: m.dev(),
            inode: m.ino(),
        })
    }
    pub fn current(&self) -> Result<()> {
        let m = std::fs::symlink_metadata(&self.source.root)
            .map_err(|_| error(ErrorCode::SourceChanged, "Gym source root is unavailable"))?;
        if !m.is_dir()
            || m.dev() != self.device
            || m.ino() != self.inode
            || self.source.root.canonicalize().ok().as_deref() != Some(&self.source.root)
        {
            return Err(error(ErrorCode::SourceChanged, "Gym source root changed"));
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TrainingSummary {
    pub v: String,
    pub observed_at: u64,
    pub run: Run,
}
pub const TRAINING_SCHEMA: &str = "openagents.gym-training-summary.v1";
struct Scan {
    anchor: Option<crate::confined::Anchor>,
    entries: usize,
    bytes: u64,
    partial: bool,
}
impl Scan {
    fn dirs(&mut self, path: &Path) -> Vec<PathBuf> {
        self.children(path, true)
    }
    fn children(&mut self, path: &Path, directories: bool) -> Vec<PathBuf> {
        let mut found = Vec::new();
        let Some(anchor) = &self.anchor else {
            return found;
        };
        let Ok((names, complete)) = anchor.names(path, MAX_SCAN.saturating_sub(self.entries))
        else {
            self.partial = true;
            return found;
        };
        self.partial |= !complete;
        self.entries += names.len();
        for name in names {
            let path = path.join(name);
            let Ok(file) = anchor.open(&path, directories) else {
                continue;
            };
            let Ok(meta) = file.metadata() else {
                self.partial = true;
                continue;
            };
            if (directories && meta.is_dir()) || (!directories && meta.is_file()) {
                found.push(path);
            }
        }
        found.sort_by_key(|p| std::cmp::Reverse(self.modified(p)));
        found
    }
    fn modified(&self, path: &Path) -> u64 {
        self.anchor
            .as_ref()
            .and_then(|a| a.open(path, false).ok())
            .and_then(|f| f.metadata().ok())
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_secs())
    }
    fn json(&mut self, path: &Path, tail: bool) -> Option<Value> {
        let mut file = match self.anchor.as_ref()?.open(path, false) {
            Ok(file) => file,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return None,
            Err(_) => {
                self.partial = true;
                return None;
            }
        };
        let m = file.metadata().ok()?;
        if !m.is_file() {
            self.partial = true;
            return None;
        }
        let cap = if tail { 64 * 1024 } else { MAX_FILE };
        if (!tail && m.len() > cap) || self.bytes.saturating_add(m.len().min(cap)) > MAX_READ {
            self.partial = true;
            return None;
        }
        let start = if tail { m.len().saturating_sub(cap) } else { 0 };
        if file.seek(SeekFrom::Start(start)).is_err() {
            self.partial = true;
            return None;
        }
        let mut bytes = Vec::new();
        if file.take(cap + 1).read_to_end(&mut bytes).is_err() || bytes.len() as u64 > cap {
            self.partial = true;
            return None;
        }
        self.bytes += bytes.len() as u64;
        if tail {
            if start > 0 {
                self.partial = true;
            }
            let mut events = bytes
                .split(|b| *b == b'\n')
                .enumerate()
                .filter(|(i, line)| !line.is_empty() && (start == 0 || *i > 0))
                .filter_map(|(_, line)| {
                    nostr::contracts::parse_strict_bounded(line, 64 * 1024).ok()
                })
                .collect::<Vec<_>>();
            if events.len() > 256 {
                events.drain(..events.len() - 256);
            }
            Some(Value::Array(events))
        } else {
            match nostr::contracts::parse_strict_bounded(&bytes, MAX_FILE as usize) {
                Ok(v) => Some(v),
                Err(_) => {
                    self.partial = true;
                    None
                }
            }
        }
    }
}
fn label(value: &str, max: usize) -> String {
    let mut out = String::new();
    for c in value.chars().filter(|c| !c.is_control()) {
        if out.len() + c.len_utf8() > max {
            break;
        }
        out.push(c);
    }
    if out.is_empty() {
        "Unnamed run".into()
    } else {
        out
    }
}
fn number(value: &Value, key: &str) -> Option<f64> {
    value[key].as_f64().filter(|n| n.is_finite() && *n >= 0.0)
}
fn id(root: &Root, path: &Path) -> String {
    nostr::contracts::digest_bytes(
        format!(
            "{}:{}:{}:{}",
            root.source.root.display(),
            root.device,
            root.inode,
            path.display()
        )
        .as_bytes(),
    )
    .trim_start_matches("sha256:")
    .into()
}
fn base(root: &Root, path: &Path, category: Category) -> Run {
    Run {
        id: id(root, path),
        title: label(
            path.file_name().and_then(|s| s.to_str()).unwrap_or("Run"),
            160,
        ),
        category,
        status: Status::Unknown,
        completed: None,
        total: None,
        cost_usd: None,
        elapsed_ms: None,
        metrics: vec![],
        source: root.source.label.clone(),
        provenance: "Host file projection; process liveness and independent outcome are unverified"
            .into(),
    }
}
fn state(recent: u64, now: u64) -> Status {
    if now.saturating_sub(recent) > 120 {
        Status::Stale
    } else {
        Status::Running
    }
}
fn microcoder(root: &Root, path: &Path, scan: &mut Scan, now: u64) -> Option<Run> {
    let summary = scan.json(&path.join("summary.json"), false);
    let events = scan.json(&path.join("events.jsonl"), true);
    let event = events
        .as_ref()
        .and_then(Value::as_array)
        .and_then(|v| v.last());
    if summary.is_none() && event.is_none() {
        return None;
    }
    let mut run = base(root, path, Category::Agent);
    let value = summary.as_ref().or(event)?;
    if let Some(task) = value["task"].as_str() {
        run.title = label(task, 160);
    }
    if let Some(s) = &summary {
        let o = &s["outcome"];
        run.status = if o["error"].as_str().is_some() || o["success"] == false {
            Status::Failed
        } else if o["ending"]["reason"].as_str().is_some_and(|r| {
            matches!(
                r,
                "finished"
                    | "tests_held"
                    | "step_limit"
                    | "time_limit"
                    | "spend_limit"
                    | "bad_replies"
                    | "idle"
                    | "unaccepted"
            )
        }) && number(o, "seconds").is_some()
        {
            Status::Completed
        } else {
            Status::Unknown
        };
        run.completed = o["steps"].as_u64();
        run.elapsed_ms = number(o, "seconds").map(|s| (s * 1000.0) as u64);
        let unpriced = o["cost_unknown"].as_array().is_some_and(|a| !a.is_empty())
            || o["model_cost"]["unknown_calls"].as_u64().unwrap_or(0) > 0
            || o["cost"]["unknown_calls"].as_u64().unwrap_or(0) > 0;
        run.cost_usd = number(o, "model_usd")
            .zip(number(o, "jev_usd"))
            .zip(number(o, "embedding_usd"))
            .filter(|_| !unpriced)
            .map(|((model, jev), embedding)| model + jev + embedding);
        let basis = s["cost_basis"]
            .as_str()
            .or_else(|| o["cost_basis"].as_str())
            .unwrap_or("unknown");
        run.provenance = format!(
            "Host Microcoder summary; cost basis {}; completion is not an independent pass",
            label(basis, 64)
        );
        if let Some(reward) = s["reward"].as_f64().filter(|v| v.is_finite()) {
            run.metrics.push(Metric {
                name: "Recorded reward".into(),
                unit: "score".into(),
                points: vec![Point {
                    step: 0,
                    value: reward,
                }],
            });
        }
    } else {
        run.status = state(scan.modified(&path.join("events.jsonl")), now);
        run.completed = value["step"].as_u64();
    }
    if let Some(events) = events.as_ref().and_then(Value::as_array) {
        for (name, unit, key) in [
            ("Recorded model call cost", "USD/call", "usd"),
            ("Model call duration", "ms/call", "milliseconds"),
        ] {
            let mut points = Vec::<Point>::new();
            for e in events.iter().filter(|e| e["event"] == "generated") {
                let Some(step) = e["step"].as_u64() else {
                    continue;
                };
                let Some(value) = number(&e["generated"], key) else {
                    continue;
                };
                if key == "usd" && e["generated"]["cost_unknown"].is_string() {
                    continue;
                }
                if points.last().is_none_or(|old| old.step < step) {
                    points.push(Point { step, value });
                }
            }
            if points.len() > 64 {
                points.drain(..points.len() - 64);
            }
            if !points.is_empty() {
                run.metrics.push(Metric {
                    name: name.into(),
                    unit: unit.into(),
                    points,
                });
            }
        }
        if !events.is_empty() {
            run.provenance.push_str(
                "; series use the last bounded 64 KiB event tail, never a complete-cost claim",
            );
        }
    }
    Some(run)
}
fn terminal(root: &Root, path: &Path, scan: &mut Scan, now: u64) -> Option<Run> {
    let result = scan
        .json(&path.join("result.json"), false)
        .or_else(|| scan.json(&path.join("harbor-result.json"), false));
    let config = scan.json(&path.join("config.json"), false);
    if result.is_none() && config.is_none() {
        return None;
    }
    let mut run = base(root, path, Category::Evaluation);
    if let Some(c) = config
        && let Some(name) = c["task"]["path"]
            .as_str()
            .and_then(|p| Path::new(p).file_name())
            .and_then(|n| n.to_str())
    {
        run.title = label(name, 160);
    }
    if let Some(result) = result {
        run.status = if result["exception_info"].is_object() {
            Status::Failed
        } else if result["finished_at"].is_string() && result["started_at"].is_string() {
            Status::Completed
        } else {
            Status::Unknown
        };
        run.cost_usd = number(&result["agent_result"], "cost_usd");
        let reward = result["verifier_result"]["rewards"]["reward"]
            .as_f64()
            .filter(|v| v.is_finite());
        if let Some(reward) = reward {
            run.metrics.push(Metric {
                name: "Recorded reward".into(),
                unit: "score".into(),
                points: vec![Point {
                    step: 0,
                    value: reward,
                }],
            });
        }
        let time = |key: &str| result[key].as_str().and_then(timestamp_ms);
        run.elapsed_ms = time("finished_at")
            .zip(time("started_at"))
            .and_then(|(a, b)| u64::try_from(a - b).ok());
        run.completed = (run.status != Status::Unknown).then_some(1);
        run.total = Some(1);
        run.provenance="Host Harbor result; recorded outcome, unverified retained evidence; reported cost only".into();
    } else {
        run.status = state(scan.modified(&path.join("trial.log")), now);
    }
    Some(run)
}
pub(crate) fn snapshot(roots: &[Root], now: u64) -> Result<Snapshot> {
    let mut scan = Scan {
        anchor: None,
        entries: 0,
        bytes: 0,
        partial: false,
    };
    let mut rows = Vec::<(u64, Run)>::new();
    for root in roots {
        root.current()?;
        scan.anchor = Some(crate::confined::Anchor::new(
            &root.source.root,
            root.device,
            root.inode,
        )?);
        match root.source.kind {
            SourceKind::Microcoder => {
                for path in scan.dirs(&root.source.root) {
                    if let Some(run) = microcoder(root, &path, &mut scan, now) {
                        rows.push((scan.modified(&path), run));
                    }
                }
            }
            SourceKind::TerminalBench => {
                for job in scan.dirs(&root.source.root) {
                    for record in scan.children(&job.join("tbench/attempts"), false) {
                        if record.extension().and_then(|s| s.to_str()) == Some("json")
                            && let Some(value) = scan.json(&record, false)
                            && let Some(run) = attempt_record(root, &job, &value)
                        {
                            rows.push((scan.modified(&record), run));
                        }
                    }
                    for path in scan.dirs(&job) {
                        if let Some(run) = terminal(root, &path, &mut scan, now) {
                            rows.push((scan.modified(&path), run));
                        }
                    }
                }
            }
            SourceKind::TrainingSummaries => {
                for path in scan.children(&root.source.root, false) {
                    if path.extension().and_then(|s| s.to_str()) != Some("json") {
                        continue;
                    }
                    let Some(value) = scan.json(&path, false) else {
                        continue;
                    };
                    let Ok(mut summary) = serde_json::from_value::<TrainingSummary>(value) else {
                        scan.partial = true;
                        continue;
                    };
                    if summary.v != TRAINING_SCHEMA
                        || summary.observed_at > now
                        || summary.run.category != Category::Training
                        || summary.run.validate().is_err()
                    {
                        scan.partial = true;
                        continue;
                    }
                    summary.run.id = id(root, &path);
                    summary.run.source = root.source.label.clone();
                    summary.run.provenance = format!(
                        "Operator training summary observed at {}; unverified declaration",
                        summary.observed_at
                    );
                    if summary.run.status == Status::Running
                        && now.saturating_sub(summary.observed_at) > 120
                    {
                        summary.run.status = Status::Stale;
                    }
                    rows.push((summary.observed_at, summary.run));
                }
            }
        }
        root.current()?;
    }
    let mut seen = std::collections::BTreeSet::new();
    rows.retain(|(_, run)| seen.insert(run.id.clone()));
    rows.sort_by_key(|(time, _)| std::cmp::Reverse(*time));
    let truncated = rows.len() > MAX_RUNS;
    let mut snapshot = Snapshot {
        observed_at: now,
        runs: rows
            .into_iter()
            .take(MAX_RUNS)
            .map(|(_, run)| run)
            .collect(),
        recipes: vec![],
        notices: vec![],
    };
    if scan.partial {
        snapshot.notices.push(
            "Some sources exceeded scan bounds or were unreadable; this board is incomplete".into(),
        );
    }
    if truncated {
        snapshot
            .notices
            .push("Showing the most recent 64 runs within the admitted bounded scan".into());
    }
    snapshot.validate(now)?;
    Ok(snapshot)
}

fn timestamp_ms(text: &str) -> Option<i64> {
    let core = text
        .strip_suffix('Z')
        .or_else(|| text.strip_suffix("+00:00"))?;
    let (whole, fraction) = core.split_once('.').unwrap_or((core, ""));
    let seconds = gym::views::rfc3339_seconds(&format!("{whole}Z"))?;
    if fraction.len() > 9 || !fraction.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let ms = format!("{fraction:0<3}").get(..3)?.parse::<u64>().ok()?;
    i64::try_from(seconds.checked_mul(1000)?.checked_add(ms)?).ok()
}

fn attempt_record(root: &Root, job: &Path, value: &Value) -> Option<Run> {
    if value["schema"] != "openagents.tbench.attempt.v1" {
        return None;
    }
    let trial = value["attempt"]["trial"].as_str()?;
    if trial.is_empty()
        || trial.len() > 256
        || Path::new(trial).components().count() != 1
        || trial == "."
        || trial == ".."
    {
        return None;
    }
    let mut run = base(root, &job.join(trial), Category::Evaluation);
    run.title = label(value["task"]["name"].as_str().unwrap_or(trial), 160);
    let terminal = value["outcome"]["terminal_status"]
        .as_str()
        .unwrap_or("unknown");
    run.status = match terminal {
        "passed" | "completed" => Status::Completed,
        "failed" | "agent_error" | "verifier_error" | "usage_limited" | "timeout" => Status::Failed,
        "cancelled" => Status::Cancelled,
        _ => Status::Unknown,
    };
    run.completed = (!matches!(run.status, Status::Unknown)).then_some(1);
    run.total = Some(1);
    run.cost_usd = number(&value["cost"], "amount_usd");
    run.elapsed_ms = value["timing"]["total_ms"].as_u64();
    if terminal != "usage_limited"
        && let Some(reward) = value["outcome"]["reward"]
            .as_f64()
            .filter(|v| v.is_finite())
    {
        run.metrics.push(Metric {
            name: "Recorded reward".into(),
            unit: "score".into(),
            points: vec![Point {
                step: 0,
                value: reward,
            }],
        });
    }
    run.provenance = format!(
        "Host Terminal-Bench attempt record; cost basis {}; evidence not independently verified",
        label(
            value["cost"]["provenance"].as_str().unwrap_or("unknown"),
            80
        )
    );
    Some(run)
}
