//! `control.monitor` as a component, and its replay over retained streams.
//!
//! A fixture holds either a script with known stalls and loops, written
//! in as [`Onset`]s, or one retained native stream with its attempt's
//! outcome. A script runs through the scripted executor under the host
//! loop with the monitor watching in shadow mode; a retained stream
//! replays through the same monitor one event at a time. Either way each
//! judgment sees only the prefix available at its trigger.
//!
//! [`replay_tree`] replays every retained stream under a traces
//! directory, labels each judgment by hindsight, and reports trigger
//! precision, stale answers, and cost, for the rules alone and for Jev.

use std::path::{Path, PathBuf};

use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::jev::{JevMode, Recorded};
use super::{Component, Fixture, Ran, input};
use crate::delegate::{Briefing, BriefingInputs};
use crate::monitor::{
    Context, Judgment, Monitor, Onset, Params, Score, from_onsets, implementation,
    label_by_hindsight, paced, replay,
};
use crate::record::{Implementation, Recorder};
use crate::scripted::{Script, Scripted};
use crate::session::{self, Controls, Watch};

/// The schema of a retained-stream replay report.
pub const REPLAY_SCHEMA: &str = "openagents.coder-one.monitor-replay.v1";

/// A retained stream in a fixture.
#[derive(Clone, Debug, Deserialize)]
struct Stream {
    stream: String,
    /// How long the dispatch ran, which paces the stream's lines.
    milliseconds: u64,
}

#[derive(Clone, Debug, Deserialize)]
struct MonitorInput {
    task: String,
    #[serde(default)]
    briefing: String,
    #[serde(default)]
    params: Option<Params>,
    #[serde(default)]
    script: Option<Script>,
    #[serde(default)]
    controls: Option<Controls>,
    #[serde(default)]
    replay: Option<Stream>,
    /// Known intervals of each flag, for a script.
    #[serde(default)]
    onsets: Vec<Onset>,
    /// Whether the attempt passed, for hindsight labels.
    #[serde(default)]
    passed: Option<bool>,
}

/// `control.monitor`.
pub struct MonitorComponent;

fn briefing_of(text: &str) -> Briefing {
    Briefing::build(
        &BriefingInputs {
            instruction: text.to_string(),
            requirements: Vec::new(),
            files: Vec::new(),
            spans: Vec::new(),
            commands: Vec::new(),
            last_output: None,
            conclusion: String::new(),
            directions: String::new(),
        },
        12_000,
    )
}

/// Runs the monitor over one input and labels its judgments.
async fn watch(
    input: &MonitorInput,
    params: Params,
    jev: &JevMode,
    recorder: &Recorder,
) -> Result<Vec<Judgment>, String> {
    let context = Context::new(&input.task, &input.briefing);
    let mut monitor = Monitor::new(params, context.clone(), jev.clone());
    if let Some(script) = &input.script {
        let dir = std::env::temp_dir().join(format!(
            "coder-one-monitor-{}-{}",
            std::process::id(),
            atif::now_ms()
        ));
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let mut scripted = Scripted::new(script.clone(), dir.clone());
        scripted.recorder = recorder.clone();
        let controls = input.controls.clone().unwrap_or_default();
        let _ = session::drive_watched(
            &mut scripted,
            &briefing_of(&input.task),
            &controls,
            recorder,
            &mut session::virtual_time(),
            Some(&mut monitor as &mut dyn Watch),
        )
        .await;
        let _ = std::fs::remove_dir_all(&dir);
        let mut judgments = monitor.judgments.clone();
        if input.onsets.is_empty() {
            label_by_hindsight(&mut judgments, &monitor.tracker, input.passed, &context);
        } else {
            for judgment in &mut judgments {
                judgment.labels = Some(from_onsets(judgment, &input.onsets));
            }
        }
        return Ok(judgments);
    }
    let Some(stream) = &input.replay else {
        return Err("the input has neither a script nor a replay".to_string());
    };
    let lines = stream.stream.lines().count().max(1) as u64;
    let events = paced(&stream.stream, (stream.milliseconds / lines).max(1));
    let _ = replay(&events, &mut monitor, recorder).await;
    let mut judgments = monitor.judgments.clone();
    label_by_hindsight(&mut judgments, &monitor.tracker, input.passed, &context);
    Ok(judgments)
}

/// The metrics a run reports: trigger precision for each decider, stale
/// answers, and cost.
fn metrics(score: &Score) -> Map<String, Value> {
    let round = |value: Option<f64>| value.map_or(Value::Null, |v| json!(super::pack::round(v)));
    let mut metrics = Map::new();
    metrics.insert("triggers".to_string(), json!(score.triggers));
    for name in [
        "intervene",
        "stalled",
        "repeating",
        "rereading",
        "claims_done",
    ] {
        let rules = score.rules.get(name).copied().unwrap_or_default();
        metrics.insert(format!("rules_precision_{name}"), round(rules.precision()));
        metrics.insert(format!("rules_recall_{name}"), round(rules.recall()));
        if let Some(jev) = score.jev.get(name) {
            metrics.insert(format!("jev_precision_{name}"), round(jev.precision()));
            metrics.insert(format!("jev_recall_{name}"), round(jev.recall()));
        }
    }
    metrics.insert("jev_requests".to_string(), json!(score.jev_requests));
    metrics.insert("jev_answered".to_string(), json!(score.jev_answered));
    metrics.insert("stale".to_string(), json!(score.stale));
    metrics.insert(
        "monitor_cost_usd".to_string(),
        json!((score.cost_usd * 1e6).round() / 1e6),
    );
    metrics
}

impl Component for MonitorComponent {
    fn id(&self) -> &'static str {
        crate::monitor::COMPONENT
    }
    fn implementation(&self) -> Implementation {
        implementation(&Params::default())
    }
    fn about(&self) -> &'static str {
        "Rules and Jev judge a running session's progress, loops, re-reading, and completion claims, in shadow mode."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        jev: &'a JevMode,
        recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: MonitorInput = input(fixture)?;
            let params = input.params.clone().unwrap_or_default();
            let judgments = watch(&input, params, jev, recorder).await?;
            let mut score = Score::default();
            score.add(&judgments);
            Ok(Ran {
                output: json!({
                    "score": score.summary(),
                    "judgments": judgments.iter().map(|j| json!({
                        "n": j.n,
                        "trigger": j.trigger,
                        "seq": j.basis.seq,
                        "revision": j.basis.revision,
                        "basis_ms": j.basis_ms,
                        "rules": j.rules,
                        "jev": j.jev_flags,
                        "labels": j.labels,
                        "stale": j.stale,
                    })).collect::<Vec<_>>(),
                }),
                metrics: metrics(&score),
            })
        })
    }
}

/// One replayed stream.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Replayed {
    pub trace: String,
    pub episode: String,
    pub stream: String,
    pub arm: String,
    pub task: String,
    /// `luna`, `opus`, or another model family.
    pub family: String,
    pub passed: Option<bool>,
    pub events: usize,
    /// Milliseconds per native line, from the dispatch's duration.
    pub ms_per_line: u64,
    /// How Jev answered this stream: `live`, `recorded`, or `off`.
    pub jev: String,
    pub score: Value,
    /// The dispatch's own cost, beside the monitor's.
    pub delegate_usd: Option<f64>,
    pub monitor_usd: f64,
}

/// The replay over a traces directory.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub schema: String,
    pub implementation: Implementation,
    pub params: Params,
    /// What the report can and cannot measure.
    pub notes: Vec<String>,
    pub streams: usize,
    pub skipped: Vec<(String, String)>,
    pub replayed: Vec<Replayed>,
    /// Pooled over every stream: triggers, precision, stale, cost.
    pub totals: Value,
    /// Pooled per model family.
    pub by_family: Value,
    /// The streams Jev answered, pooled: the only place the rules and Jev
    /// are compared on the same judgments.
    pub compared: Value,
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Every retained stream under `traces`: `(episode dir, stream file)`.
fn streams(traces: &Path) -> Vec<(PathBuf, PathBuf)> {
    let mut out = Vec::new();
    let mut pending = vec![(traces.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let artifacts = path.join("artifacts");
            if path.join("manifest.json").is_file() && artifacts.is_dir() {
                for file in std::fs::read_dir(&artifacts)
                    .into_iter()
                    .flatten()
                    .flatten()
                {
                    let name = file.file_name().to_string_lossy().into_owned();
                    if name.starts_with("delegate-") && name.ends_with(".stream.jsonl") {
                        out.push((path.clone(), file.path()));
                    }
                }
            } else if depth < 3 {
                pending.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// The family of a model name.
fn family(model: &str) -> String {
    ["luna", "opus", "sonnet", "haiku", "fable"]
        .iter()
        .find(|name| model.contains(**name))
        .map_or_else(|| "other".to_string(), |name| (*name).to_string())
}

/// One stream's inputs, read from its episode directory.
fn stream_input(dir: &Path, file: &Path) -> Result<(MonitorInput, Value), String> {
    let manifest = read_json(&dir.join("manifest.json")).ok_or("no manifest")?;
    let state = read_json(&dir.join("artifacts/state.json")).ok_or("no state.json")?;
    let task = state
        .pointer("/issue/body")
        .and_then(Value::as_str)
        .ok_or("no task text in state.json")?
        .to_string();
    let stream = std::fs::read_to_string(file).map_err(|error| error.to_string())?;
    let name = file
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let briefing = std::fs::read_to_string(
        dir.join("artifacts")
            .join(name.replace(".stream.jsonl", ".briefing.md")),
    )
    .unwrap_or_default();
    let passed = std::fs::read_to_string(dir.join("verifier/reward.txt"))
        .ok()
        .and_then(|text| text.trim().parse::<f64>().ok())
        .map(|reward| reward > 0.0);
    let milliseconds = manifest
        .pointer("/delegate/delegation/milliseconds")
        .and_then(Value::as_u64)
        .unwrap_or(0);
    Ok((
        MonitorInput {
            task,
            briefing,
            params: None,
            script: None,
            controls: None,
            replay: Some(Stream {
                stream,
                milliseconds,
            }),
            onsets: Vec::new(),
            passed,
        },
        manifest,
    ))
}

/// How the replay asks Jev.
pub struct ReplayJev {
    /// A live client, used for the sampled streams until the limit.
    pub live: Option<jev::Client>,
    /// The most live requests the whole replay may make.
    pub live_limit: usize,
    /// Which traces are sampled for live answers: a trace name contains
    /// one of these.
    pub sample: Vec<String>,
    /// Recorded answers, replayed for every other stream.
    pub recorded: Recorded,
    /// Whether Jev is asked at all.
    pub on: bool,
}

/// The traces sampled for live answers by default: the first trial of
/// each development task under the Luna and Opus arms the router compares,
/// and under the v2 Luna arm, whose streams hold most of the stalls.
#[must_use]
pub fn default_sample() -> Vec<String> {
    vec![
        "coder-one-jevprobe3-luna--".to_string(),
        "coder-one-jevprobe2-opus-lean-low-5m--".to_string(),
        "coder-one-jevprobe2-luna--".to_string(),
    ]
}

fn first_trial(trace: &str) -> bool {
    !(trace.ends_with("-2") || trace.ends_with("-3"))
}

/// Replays every retained stream under `traces` through the monitor.
/// Returns the report and the recorder whose steps hold every Jev answer,
/// so a live run can be saved.
pub async fn replay_tree(traces: &Path, params: &Params, jev: &ReplayJev) -> (Report, Recorder) {
    replay_streams(traces, params, jev, &|_| true).await
}

/// Replays the retained streams under `traces` that `keep` accepts,
/// each named `<trace>/<stream file>`, as [`replay_tree`] replays them
/// all. A test pins a report to the population it was written over this
/// way, so newer retained traces don't move its numbers.
pub async fn replay_streams(
    traces: &Path,
    params: &Params,
    jev: &ReplayJev,
    keep: &dyn Fn(&str) -> bool,
) -> (Report, Recorder) {
    let recorder = Recorder::default();
    let mut skipped = Vec::new();
    let mut replayed = Vec::new();
    let mut totals = Score::default();
    let mut compared = Score::default();
    let mut families: std::collections::BTreeMap<String, Score> = Default::default();
    let mut live_used = 0usize;
    let trace_of = |dir: &Path| {
        dir.parent()
            .and_then(Path::file_name)
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default()
    };
    let label_of = |dir: &Path, file: &Path| {
        format!(
            "{}/{}",
            trace_of(dir),
            file.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
        )
    };
    let all: Vec<(PathBuf, PathBuf)> = streams(traces)
        .into_iter()
        .filter(|(dir, file)| keep(&label_of(dir, file)))
        .collect();
    for (dir, file) in &all {
        let trace = trace_of(dir);
        let label = label_of(dir, file);
        let (input, manifest) = match stream_input(dir, file) {
            Ok(read) => read,
            Err(why) => {
                skipped.push((label, why));
                continue;
            }
        };
        // Count this stream's triggers with the rules alone, to keep the
        // live sample inside its limit.
        let rules_only = Params {
            jev: false,
            ..params.clone()
        };
        let dry = match watch(
            &input,
            rules_only.clone(),
            &JevMode::Off,
            &Recorder::default(),
        )
        .await
        {
            Ok(dry) => dry,
            Err(why) => {
                skipped.push((label, why));
                continue;
            }
        };
        let sampled = jev.on
            && jev.live.is_some()
            && first_trial(&trace)
            && jev.sample.iter().any(|s| trace.contains(s.as_str()))
            && live_used + dry.len() <= jev.live_limit;
        let (mode, how) = if !jev.on {
            (JevMode::Off, "off")
        } else if sampled {
            live_used += dry.len();
            (
                JevMode::Live(jev.live.clone().expect("sampled only with a client")),
                "live",
            )
        } else {
            (JevMode::Recorded(jev.recorded.clone()), "recorded")
        };
        let used = if jev.on { params.clone() } else { rules_only };
        let judgments = match watch(&input, used, &mode, &recorder).await {
            Ok(judgments) => judgments,
            Err(why) => {
                skipped.push((label, why));
                continue;
            }
        };
        let mut score = Score::default();
        score.add(&judgments);
        let model = manifest
            .pointer("/delegate/model")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let fam = family(model);
        totals.merge(&score);
        families.entry(fam.clone()).or_default().merge(&score);
        if score.jev_answered > 0 {
            let answered: Vec<Judgment> = judgments
                .iter()
                .filter(|j| j.jev_flags.is_some())
                .cloned()
                .collect();
            let mut both = Score::default();
            both.add(&answered);
            compared.merge(&both);
        }
        let stream_text = input
            .replay
            .as_ref()
            .map(|s| s.stream.as_str())
            .unwrap_or("");
        let lines = stream_text.lines().count().max(1) as u64;
        let events = paced(stream_text, 1).len();
        replayed.push(Replayed {
            task: trace
                .rsplit("--")
                .next()
                .unwrap_or_default()
                .trim_end_matches("-2")
                .trim_end_matches("-3")
                .to_string(),
            arm: trace.split("--").nth(1).unwrap_or_default().to_string(),
            trace,
            episode: dir
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            stream: file
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            family: fam,
            passed: input.passed,
            events,
            ms_per_line: input
                .replay
                .as_ref()
                .map_or(0, |s| (s.milliseconds / lines).max(1)),
            jev: how.to_string(),
            monitor_usd: (score.cost_usd * 1e6).round() / 1e6,
            score: score.summary(),
            delegate_usd: manifest
                .pointer("/delegate/delegation/total_cost_usd")
                .and_then(Value::as_f64),
        });
    }
    let delegate_usd: f64 = replayed.iter().filter_map(|r| r.delegate_usd).sum();
    let mut total = totals.summary();
    total["delegate_usd"] = json!((delegate_usd * 1e4).round() / 1e4);
    total["monitor_share_of_delegate"] = if delegate_usd > 0.0 {
        json!(super::pack::round(totals.cost_usd / delegate_usd))
    } else {
        Value::Null
    };
    total["streams"] = json!(replayed.len());
    total["live_requests"] = json!(live_used);
    let report = Report {
        schema: REPLAY_SCHEMA.to_string(),
        implementation: implementation(params),
        params: params.clone(),
        notes: vec![
            "Each judgment sees only the prefix of the stream up to its trigger.".to_string(),
            "Labels are hindsight: stalled means the attempt failed and nothing changed after the trigger over at least two more commands; repeating means the last failing command failed again before any change; rereading is a fact of the prefix, so the rule defines it; claims_done means the claim was the session's last and no artifact changed after it.".to_string(),
            "Retained streams carry no per-line times, so lines are paced evenly over the dispatch's duration; silences never trigger on a replay, and stale answers follow that pacing.".to_string(),
            "Jev's cost is priced at the published input rate from reported tokens where an answer exists, and estimated at four characters a token otherwise.".to_string(),
        ],
        streams: all.len(),
        skipped,
        replayed,
        totals: total,
        by_family: families
            .iter()
            .map(|(name, score)| (name.clone(), score.summary()))
            .collect::<Map<String, Value>>()
            .into(),
        compared: compared.summary(),
    };
    (report, recorder)
}

/// The report's text form.
#[must_use]
pub fn lines(report: &Report) -> Vec<String> {
    let t = &report.totals;
    let mut out = vec![format!(
        "control.monitor replay · {} streams · {} replayed · {} triggers · {} Jev asks ({} answered, {} of them live now) · stale {}",
        report.streams,
        report.replayed.len(),
        t["triggers"],
        t["jev_requests"],
        t["jev_answered"],
        t["live_requests"],
        t["stale"],
    )];
    out.push(format!(
        "  monitor cost ${} at Jev's rate against ${} of executor spend ({} of it)",
        t["cost_usd"], t["delegate_usd"], t["monitor_share_of_delegate"]
    ));
    out.push("  question      labels  rules flagged/precision/recall   Jev flagged/precision/recall (answered streams)".to_string());
    let c = &report.compared;
    for name in [
        "intervene",
        "stalled",
        "repeating",
        "rereading",
        "claims_done",
    ] {
        let cell = |value: &Value| {
            format!(
                "{:>4} / {:>5} / {:>5}",
                value["flagged"],
                value["precision"]
                    .as_f64()
                    .map_or("—".to_string(), |p| format!("{p:.2}")),
                value["recall"]
                    .as_f64()
                    .map_or("—".to_string(), |p| format!("{p:.2}")),
            )
        };
        out.push(format!(
            "  {:<12} {:>6}  {:<30} {}",
            name,
            t["labels"][name].as_u64().unwrap_or(0),
            cell(&t["rules"][name]),
            if c["jev"][name].is_null() {
                "—".to_string()
            } else {
                format!(
                    "{}  (rules there: {})",
                    cell(&c["jev"][name]),
                    cell(&c["rules"][name])
                )
            }
        ));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn traces() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")
    }

    #[tokio::test]
    async fn the_recorded_replay_reproduces_the_checked_in_report() {
        let dir = crate::component::cli::monitor_dir();
        let jev = ReplayJev {
            live: None,
            live_limit: 0,
            sample: default_sample(),
            recorded: Recorded::load(&dir.join("jev-recorded.json")).unwrap(),
            on: true,
        };
        let checked_in: Value =
            serde_json::from_str(&std::fs::read_to_string(dir.join("replay.json")).unwrap())
                .unwrap();
        // The checked-in report documents the streams retained when it was
        // written; traces retained since then aren't part of it.
        let documented: std::collections::BTreeSet<String> = checked_in["replayed"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                format!(
                    "{}/{}",
                    r["trace"].as_str().unwrap(),
                    r["stream"].as_str().unwrap()
                )
            })
            .chain(
                checked_in["skipped"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|s| s[0].as_str().unwrap().to_string()),
            )
            .collect();
        assert_eq!(
            documented.len(),
            checked_in["streams"].as_u64().unwrap() as usize
        );
        let (report, _) = replay_streams(&traces(), &Params::default(), &jev, &|label| {
            documented.contains(label)
        })
        .await;
        // Every documented stream is still retained.
        assert_eq!(report.streams, documented.len());
        assert!(report.streams >= 200, "{}", report.streams);
        assert_eq!(report.replayed.len() + report.skipped.len(), report.streams);
        assert!(report.totals["triggers"].as_u64().unwrap() > 500);
        // Every live answer the replay recorded replays; the rest miss.
        assert!(report.compared["jev_answered"].as_u64().unwrap() >= 100);
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            checked_in,
            "rerun `coder-one component replay control.monitor --out bench/terminal-bench/monitor/replay.json`"
        );
    }

    /// Every retained stream, the documented ones and the newer ones,
    /// replays or is skipped with a reason; none crash the replay.
    #[tokio::test]
    async fn every_retained_stream_replays_or_is_skipped() {
        let jev = ReplayJev {
            live: None,
            live_limit: 0,
            sample: default_sample(),
            recorded: Recorded::default(),
            on: false,
        };
        let (report, _) = replay_tree(&traces(), &Params::default(), &jev).await;
        assert!(report.streams >= 234, "{}", report.streams);
        assert_eq!(report.replayed.len() + report.skipped.len(), report.streams);
        let dir = tempfile::tempdir().unwrap();
        crate::component::replay::tests::odd_tree(dir.path());
        let (report, _) = replay_tree(dir.path(), &Params::default(), &jev).await;
        assert_eq!(report.replayed.len() + report.skipped.len(), report.streams);
    }

    #[tokio::test]
    async fn the_scripted_suite_replays_recorded_answers_and_finds_its_loops() {
        let component = MonitorComponent;
        let root = super::super::default_fixtures();
        let dirs = super::super::fixtures_for(&root, "control.monitor");
        assert!(dirs.len() >= 5, "{dirs:?}");
        let recorder = Recorder::default();
        let suite = super::super::suite(
            &component,
            &dirs,
            &super::super::JevChoice::Recorded,
            &recorder,
            false,
        )
        .await
        .unwrap();
        for run in &suite.runs {
            assert!(run.error.is_none(), "{}: {:?}", run.fixture, run.error);
            assert!(
                !run.jev.contains_key("miss"),
                "{}: {:?}",
                run.fixture,
                run.jev
            );
        }
        let loop_run = suite
            .runs
            .iter()
            .find(|run| run.fixture.ends_with("scripted-loop"))
            .unwrap();
        assert_eq!(loop_run.metrics["rules_precision_repeating"], json!(1.0));
        let silence = suite
            .runs
            .iter()
            .find(|run| run.fixture.ends_with("scripted-silence"))
            .unwrap();
        let triggers: Vec<&str> = silence.output["judgments"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|j| j["trigger"].as_str())
            .collect();
        assert!(triggers.contains(&"silence"), "{triggers:?}");
        // A recorded suite reruns byte for byte.
        let again = super::super::suite(
            &component,
            &dirs,
            &super::super::JevChoice::Recorded,
            &Recorder::default(),
            false,
        )
        .await
        .unwrap();
        assert_eq!(suite.result(), again.result());
    }
}
