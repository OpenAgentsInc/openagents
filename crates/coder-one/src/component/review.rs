//! `control.review` as a component, and its offline replay over retained
//! lean-loop dispatches (issue #9637).
//!
//! A fixture holds the rule's [`Evidence`] and [`Params`]; the component
//! returns the [`Decision`] the lean loop would make, on the same code the
//! loop calls.
//!
//! [`replay`] reads every retained lean-loop dispatch record
//! (`artifacts/microluna-N.json`) under the directories it is given and
//! asks, for each dispatch that reached its self-check, what the rule would
//! have decided there, from the records the trial kept:
//!
//! - the frozen score and the hard-coding answer of the reviewed
//!   candidate, from the loop's own record;
//! - `check-grades.json` and `executed-commands.jsonl`, when the trial has
//!   them (no retained trial does yet: #9635 and #9636 write them);
//! - the requirement map, from `artifacts/requirements.json`.
//!
//! Every row also carries a proxy for trigger 4 that the retained records
//! can answer: the frozen evaluation script (`lean-N/evaluator/score.sh`)
//! as the only executed check, so a requirement counts as touched when the
//! script's text names its path or command. The proxy is labeled as one
//! and never stands in for the rule's own reading.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use futures_util::future::LocalBoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};

use super::jev::JevMode;
use super::{Component, Fixture, Ran, input};
use crate::record::{Implementation, Recorder};
use crate::review_rule::{
    self as rule, COMPONENT, Decision, Evidence, Executed, Params, Reading, Target, Trigger,
};

/// The schema of a replay's rows.
pub const ROW_SCHEMA: &str = "openagents.coder-one.review-rule-row.v1";

/// The schema of a replay's summary.
pub const SUMMARY_SCHEMA: &str = "openagents.coder-one.review-rule-summary.v1";

/// A fixture's input.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ReviewInput {
    pub evidence: Evidence,
    #[serde(default)]
    pub params: Params,
}

/// `control.review`.
pub struct ReviewComponent;

impl Component for ReviewComponent {
    fn id(&self) -> &'static str {
        COMPONENT
    }
    fn implementation(&self) -> Implementation {
        rule::implementation(Params::default())
    }
    fn about(&self) -> &'static str {
        "Code starts the review session only when the score, an executed check, the hard-coding \
         question, or an uncovered requirement disagrees."
    }
    fn run<'a>(
        &'a self,
        fixture: &'a Fixture,
        _jev: &'a JevMode,
        _recorder: &'a Recorder,
    ) -> LocalBoxFuture<'a, Result<Ran, String>> {
        Box::pin(async move {
            let input: ReviewInput = input(fixture)?;
            let decision = rule::decide(&input.evidence, input.params);
            let mut metrics = Map::new();
            metrics.insert("review".to_string(), json!(decision.review));
            metrics.insert("trigger".to_string(), json!(decision.words()));
            if let Some(want) = fixture.retained.get("review").and_then(Value::as_bool) {
                let fired = fixture
                    .retained
                    .get("fired")
                    .is_none_or(|f| *f == json!(decision.fired));
                metrics.insert(
                    "matches_retained".to_string(),
                    json!(want == decision.review && fired),
                );
            }
            Ok(Ran {
                output: serde_json::to_value(&decision).map_err(|e| e.to_string())?,
                metrics,
            })
        })
    }
}

/// One dispatch's replay row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Row {
    pub schema: String,
    /// The directory the row came from, as given.
    pub root: String,
    pub job: String,
    pub trial: String,
    pub task: String,
    pub dispatch: String,
    /// The verifier's reward, or `None` when not known.
    pub reward: Option<f64>,
    /// Whether the loop reached its self-check.
    pub reviewed: bool,
    /// Why it didn't, when it didn't.
    pub not_reviewed: Option<String>,
    /// The self-check session's time, cost, and whether it changed the
    /// workspace.
    pub review_ms: Option<u64>,
    pub review_usd: Option<f64>,
    pub review_changed: Option<bool>,
    pub review_status: Option<String>,
    /// Every session's time and cost in the dispatch.
    pub sessions_ms: u64,
    pub sessions_usd: f64,
    /// Harbor's agent time, when recorded.
    pub agent_ms: Option<u64>,
    /// The session whose candidate the review read.
    pub session: Option<u32>,
    /// The rule on the retained records, `unknown_fires` on and off.
    pub decision: Option<Decision>,
    pub review_if_unknown_fires: Option<bool>,
    pub review_if_unknown_clear: Option<bool>,
    /// Trigger 4 with the frozen score script as the only executed check.
    pub uncovered_proxy: Option<rule::Reason>,
    /// The rule with that proxy for trigger 4 and trigger 2 still unknown,
    /// `unknown_fires` off.
    pub review_proxy: Option<bool>,
    /// Deliverable and check requirements that name a path or command.
    pub in_scope: Vec<String>,
}

/// The reward from a trial's records, or from `labels` by trial name.
fn reward(episode: &Path, trial: &str, labels: &BTreeMap<String, f64>) -> Option<f64> {
    if let Ok(text) = std::fs::read_to_string(episode.join("verifier/reward.txt")) {
        return text.trim().parse().ok();
    }
    for file in [
        episode.join("harbor-result.json"),
        episode.join("../../result.json"),
    ] {
        if let Some(r) = std::fs::read_to_string(file)
            .ok()
            .and_then(|t| serde_json::from_str::<Value>(&t).ok())
            .and_then(|v| v.pointer("/verifier_result/rewards/reward")?.as_f64())
        {
            return Some(r);
        }
    }
    labels.get(trial).copied()
}

/// Harbor's agent time for a trial, when recorded.
fn agent_ms(episode: &Path) -> Option<u64> {
    let text = std::fs::read_to_string(episode.join("harbor-result.json"))
        .or_else(|_| std::fs::read_to_string(episode.join("../../result.json")))
        .ok()?;
    let value: Value = serde_json::from_str(&text).ok()?;
    let at = |key: &str| -> Option<i64> {
        let text = value
            .pointer(&format!("/agent_execution/{key}"))?
            .as_str()?;
        time_ms(text)
    };
    u64::try_from(at("finished_at")? - at("started_at")?).ok()
}

/// Milliseconds since the epoch of an RFC 3339 time such as
/// `2026-09-24T22:37:54.854851Z`.
fn time_ms(text: &str) -> Option<i64> {
    let (date, time) = text.trim_end_matches('Z').split_once('T')?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let (hms, frac) = time.split_once('.').unwrap_or((time, "0"));
    let mut t = hms.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next()??);
    let ms: i64 = format!("{frac:0<3}")[..3].parse().ok()?;
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + h) * 60 + min) * 60_000 + s * 1_000 + ms)
}

/// Every episode directory under `root`: a `*.episode` directory, or a
/// trial's `agent/episode`.
fn episodes(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut pending = vec![(root.to_path_buf(), 0)];
    while let Some((dir, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let episode = path.extension().is_some_and(|e| e == "episode")
                || (path.file_name().is_some_and(|n| n == "episode")
                    && dir.file_name().is_some_and(|n| n == "agent"));
            if episode {
                out.push(path);
            } else if depth < 4 {
                pending.push((path, depth + 1));
            }
        }
    }
    out.sort();
    out
}

/// Whether a session record is the loop's self-check.
fn is_review(session: &Value) -> bool {
    session["group"] == "the self-check"
        || session["focus"]
            .as_array()
            .is_some_and(|f| f.iter().any(|x| x == "the self-check"))
}

/// The requirement map's targets from `artifacts/requirements.json`.
fn read_targets(artifacts: &Path) -> Option<Vec<Target>> {
    let text = std::fs::read_to_string(artifacts.join("requirements.json")).ok()?;
    let map: crate::requirements::RequirementMap = serde_json::from_str(&text).ok()?;
    Some(rule::targets(&map))
}

/// One dispatch record's row.
fn row_of(
    root: &Path,
    episode: &Path,
    record: &Value,
    dispatch: &str,
    labels: &BTreeMap<String, f64>,
) -> Row {
    let artifacts = episode.join("artifacts");
    let (job, trial) = if episode.file_name().is_some_and(|n| n == "episode") {
        let trial_dir = episode.parent().and_then(Path::parent);
        (
            trial_dir
                .and_then(Path::parent)
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            trial_dir
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    } else {
        (
            episode
                .parent()
                .and_then(Path::file_name)
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
            episode
                .file_stem()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default(),
        )
    };
    let task = trial.split("__").next().unwrap_or_default().to_string();
    let sessions: Vec<&Value> = record["sessions"]
        .as_array()
        .into_iter()
        .flatten()
        .collect();
    let moves: Vec<&Value> = record["moves"].as_array().into_iter().flatten().collect();
    let review = sessions.iter().find(|s| is_review(s));
    let mut row = Row {
        schema: ROW_SCHEMA.to_string(),
        root: root.display().to_string(),
        job,
        trial: trial.clone(),
        task,
        dispatch: dispatch.to_string(),
        reward: reward(episode, &trial, labels),
        reviewed: review.is_some(),
        not_reviewed: None,
        review_ms: review.and_then(|s| s["milliseconds"].as_u64()),
        review_usd: review.and_then(|s| s["cost_usd"].as_f64()),
        review_changed: review.and_then(|s| s["changed_workspace"].as_bool()),
        review_status: review.and_then(|s| s["status"].as_str().map(str::to_string)),
        sessions_ms: sessions
            .iter()
            .filter_map(|s| s["milliseconds"].as_u64())
            .sum(),
        sessions_usd: sessions.iter().filter_map(|s| s["cost_usd"].as_f64()).sum(),
        agent_ms: agent_ms(episode),
        session: None,
        decision: None,
        review_if_unknown_fires: None,
        review_if_unknown_clear: None,
        uncovered_proxy: None,
        review_proxy: None,
        in_scope: Vec::new(),
    };
    let Some(review) = review else {
        row.not_reviewed = Some(format!(
            "the loop never reached its self-check: {}",
            record["stopped"].as_str().unwrap_or("unknown")
        ));
        return row;
    };
    let review_number = review["number"].as_u64().unwrap_or(0);
    // The reviewed candidate, as the lean loop picks it: the best kept work
    // session when protected candidates restore it before the review, the
    // last work session otherwise.
    let protect = record["policy"]
        .pointer("/lean/protect_candidates")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let work: Vec<&&Value> = moves
        .iter()
        .filter(|m| {
            m["kind"] == "lean"
                && m["self_check"] != true
                && m["after_session"]
                    .as_u64()
                    .is_some_and(|n| n < review_number)
        })
        .collect();
    let candidate = work
        .iter()
        .rev()
        .find(|m| protect && m["kept"] == true)
        .or_else(|| work.last())
        .copied();
    let session = candidate
        .and_then(|m| m["after_session"].as_u64())
        .map_or(review_number.saturating_sub(1), |n| n);
    let session = u32::try_from(session).unwrap_or(0);
    row.session = Some(session);
    let score = candidate.and_then(|m| {
        Some((
            m["score"]["passed"].as_u64()?,
            m["score"]["total"].as_u64()?,
        ))
    });
    let hardcode_on = record["policy"]
        .pointer("/lean/hardcode_check")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let hardcoded = if hardcode_on {
        candidate.and_then(|m| rule::hardcoded_of(&m["hardcoded"]))
    } else {
        None
    };
    let groups: Vec<PathBuf> = std::fs::read_dir(&artifacts)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.is_dir()
                && p.file_name()
                    .is_some_and(|n| n.to_string_lossy().starts_with("lean-"))
        })
        .collect();
    let mut dirs: Vec<&Path> = groups.iter().map(PathBuf::as_path).collect();
    dirs.push(&artifacts);
    let targets = read_targets(&artifacts).unwrap_or_default();
    row.in_scope = targets
        .iter()
        .filter(|t| t.in_scope())
        .map(|t| t.id.clone())
        .collect();
    let evidence = Evidence {
        session,
        score,
        grades: rule::read_grades(&dirs),
        executed: rule::read_executed(&dirs),
        hardcoded,
        targets: targets.clone(),
    };
    let on = rule::decide(&evidence, Params::default());
    let off = rule::decide(
        &evidence,
        Params {
            unknown_fires: false,
        },
    );
    row.review_if_unknown_fires = Some(on.review);
    row.review_if_unknown_clear = Some(off.review);
    row.decision = Some(on.clone());
    let scorer = groups
        .iter()
        .map(|g| g.join("evaluator/score.sh"))
        .find(|p| p.is_file());
    if let Some(text) = scorer.and_then(|p| std::fs::read_to_string(p).ok()) {
        let proxy = [Executed {
            stage: "after_session".to_string(),
            session: Some(session),
            kind: "score".to_string(),
            command: text,
            ..Executed::default()
        }];
        let reason = rule::uncovered_reading(Some(&proxy), &targets, session);
        let triggers: Vec<rule::Reason> = on
            .triggers
            .iter()
            .map(|r| {
                if r.trigger == Trigger::Uncovered {
                    reason.clone()
                } else {
                    r.clone()
                }
            })
            .collect();
        row.review_proxy = Some(
            rule::decision(
                session,
                triggers,
                Params {
                    unknown_fires: false,
                },
            )
            .review,
        );
        row.uncovered_proxy = Some(reason);
    }
    row
}

/// Every retained lean-loop dispatch under `roots`, as rows. `labels`
/// gives rewards by trial name where the trial didn't keep its own.
#[must_use]
pub fn replay(roots: &[PathBuf], labels: &BTreeMap<String, f64>) -> Vec<Row> {
    let mut rows = Vec::new();
    for root in roots {
        for episode in episodes(root) {
            let mut files: Vec<PathBuf> = std::fs::read_dir(episode.join("artifacts"))
                .into_iter()
                .flatten()
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    let name = p.file_name().unwrap_or_default().to_string_lossy();
                    name.starts_with("microluna-")
                        && name.ends_with(".json")
                        && !name.ends_with(".atif.json")
                })
                .collect();
            files.sort();
            for file in files {
                let Some(record) = std::fs::read_to_string(&file)
                    .ok()
                    .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                else {
                    continue;
                };
                if record["mode"] != "lean" {
                    continue;
                }
                let dispatch = file
                    .file_stem()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                rows.push(row_of(root, &episode, &record, &dispatch, labels));
            }
        }
    }
    rows
}

/// Rewards by trial name from a labels file: an array, or an object with
/// `labels` or `predictions`, of `{trial, reward}`. A later entry for the
/// same trial replaces an earlier one.
///
/// # Errors
///
/// Returns a message when the file can't be read.
pub fn read_labels(path: &Path) -> Result<BTreeMap<String, f64>, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    let value: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let list = value["labels"]
        .as_array()
        .or(value["predictions"].as_array())
        .or(value.as_array());
    Ok(list
        .into_iter()
        .flatten()
        .filter_map(|l| Some((l["trial"].as_str()?.to_string(), l["reward"].as_f64()?)))
        .collect())
}

fn share(k: usize, n: usize) -> Value {
    json!({
        "k": k,
        "n": n,
        "rate": (n > 0).then(|| k as f64 / n as f64),
        "wilson95": super::finish::wilson(k, n).map(|(lo, hi)| [lo, hi]),
    })
}

/// A population's counts: how each trigger read, when the review would
/// have run, what skipping it saves, and whether a trigger fires on the
/// failures.
fn population(rows: &[&Row]) -> Value {
    let reviewed: Vec<&&Row> = rows.iter().filter(|r| r.reviewed).collect();
    let pass = |r: &Row| r.reward.is_some_and(|x| x >= 1.0);
    let fail = |r: &Row| r.reward.is_some_and(|x| x < 1.0);
    let mut triggers = Map::new();
    for trigger in Trigger::ALL {
        let mut counts = BTreeMap::new();
        for row in &reviewed {
            if let Some(d) = &row.decision {
                let reading = d
                    .triggers
                    .iter()
                    .find(|r| r.trigger == trigger)
                    .map_or(Reading::Unknown, |r| r.reading);
                let word = serde_json::to_value(reading)
                    .ok()
                    .and_then(|v| v.as_str().map(str::to_string))
                    .unwrap_or_default();
                *counts.entry(word).or_insert(0usize) += 1;
            }
        }
        triggers.insert(trigger.word().to_string(), json!(counts));
    }
    let proxy_fired = reviewed
        .iter()
        .filter(|r| {
            r.uncovered_proxy
                .as_ref()
                .is_some_and(|p| p.reading == Reading::Fired)
        })
        .count();
    let proxy_known = reviewed
        .iter()
        .filter(|r| r.uncovered_proxy.is_some())
        .count();
    let arm = |pick: &dyn Fn(&Row) -> Option<bool>| {
        let known: Vec<&&&Row> = reviewed.iter().filter(|r| pick(r).is_some()).collect();
        let skipped: Vec<&&&Row> = known
            .iter()
            .copied()
            .filter(|r| pick(r) == Some(false))
            .collect();
        let ms: u64 = skipped.iter().filter_map(|r| r.review_ms).sum();
        let usd: f64 = skipped.iter().filter_map(|r| r.review_usd).sum::<f64>() + 0.0;
        let all_ms: u64 = known.iter().map(|r| r.sessions_ms).sum();
        let all_usd: f64 = known.iter().map(|r| r.sessions_usd).sum();
        let agent: u64 = known.iter().filter_map(|r| r.agent_ms).sum();
        let failures: Vec<&&&Row> = known.iter().copied().filter(|r| fail(r)).collect();
        let passes: Vec<&&&Row> = known.iter().copied().filter(|r| pass(r)).collect();
        json!({
            "dispatches": known.len(),
            "review_runs": known.len() - skipped.len(),
            "skipped": share(skipped.len(), known.len()),
            "skipped_on_passes": share(passes.iter().filter(|r| pick(r) == Some(false)).count(), passes.len()),
            "fires_on_failures": share(failures.iter().filter(|r| pick(r) == Some(true)).count(), failures.len()),
            "saved_review_ms": ms,
            "saved_review_usd": usd,
            "saved_share_of_session_time": (all_ms > 0).then(|| ms as f64 / all_ms as f64),
            "saved_share_of_session_cost": (all_usd > 0.0).then(|| usd / all_usd),
            "saved_share_of_agent_time": (agent > 0 && known.iter().all(|r| r.agent_ms.is_some())).then(|| ms as f64 / agent as f64),
        })
    };
    let changed = reviewed
        .iter()
        .filter(|r| r.review_changed == Some(true))
        .count();
    json!({
        "dispatches": rows.len(),
        "reached_review": reviewed.len(),
        "passes": rows.iter().filter(|r| pass(r)).count(),
        "failures": rows.iter().filter(|r| fail(r)).count(),
        "reward_unknown": rows.iter().filter(|r| r.reward.is_none()).count(),
        "review_changed_workspace": share(changed, reviewed.len()),
        "review_ms": reviewed.iter().filter_map(|r| r.review_ms).sum::<u64>(),
        "review_usd": reviewed.iter().filter_map(|r| r.review_usd).sum::<f64>() + 0.0,
        "triggers": triggers,
        "uncovered_proxy_fired": share(proxy_fired, proxy_known),
        "rule_unknown_fires": arm(&|r: &Row| r.review_if_unknown_fires),
        "rule_unknown_clear": arm(&|r: &Row| r.review_if_unknown_clear),
        "rule_with_uncovered_proxy": arm(&|r: &Row| r.review_proxy),
    })
}

/// The replay's summary, by root and in all.
#[must_use]
pub fn summary(rows: &[Row]) -> Value {
    let mut roots: BTreeMap<String, Vec<&Row>> = BTreeMap::new();
    for row in rows {
        roots.entry(row.root.clone()).or_default().push(row);
    }
    json!({
        "schema": SUMMARY_SCHEMA,
        "implementation": rule::implementation(Params::default()),
        "all": population(&rows.iter().collect::<Vec<_>>()),
        "by_root": roots.iter().map(|(k, v)| (k.clone(), population(v))).collect::<Map<String, Value>>(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn harbor_times_read_as_milliseconds() {
        let a = time_ms("2026-09-24T22:37:54.854851Z").unwrap();
        let b = time_ms("2026-09-24T22:46:26.234211Z").unwrap();
        assert_eq!(b - a, 511_380);
        assert_eq!(time_ms("1970-01-01T00:00:01Z"), Some(1_000));
    }

    #[test]
    fn labels_read_from_either_shape() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("labels.json");
        std::fs::write(
            &path,
            json!({"labels": [{"trial": "a__1", "reward": 0.0}, {"trial": "b__2", "reward": null}]})
                .to_string(),
        )
        .unwrap();
        let labels = read_labels(&path).unwrap();
        assert_eq!(labels.get("a__1"), Some(&0.0));
        assert!(!labels.contains_key("b__2"));
    }

    #[test]
    fn the_replay_reads_a_retained_dispatch() {
        let dir = tempfile::tempdir().unwrap();
        let episode = dir.path().join("job-1/task__abc.episode");
        let artifacts = episode.join("artifacts");
        std::fs::create_dir_all(artifacts.join("lean-1/evaluator")).unwrap();
        std::fs::write(
            artifacts.join("lean-1/evaluator/score.sh"),
            "test -f out/report.csv && echo SCORE 1 1",
        )
        .unwrap();
        let map = crate::requirements::mechanical(
            "Write `/app/out/report.csv` with one row per input.\n\nThen write `/app/out/summary.json`.",
        );
        std::fs::write(
            artifacts.join("requirements.json"),
            serde_json::to_string(&map).unwrap(),
        )
        .unwrap();
        std::fs::write(
            episode.join("harbor-result.json"),
            json!({"verifier_result": {"rewards": {"reward": 0.0}}}).to_string(),
        )
        .unwrap();
        let record = json!({
            "mode": "lean",
            "policy": {"lean": {"hardcode_check": true}},
            "stopped": "session 1 ended done; the self-check ended done",
            "sessions": [
                {"number": 1, "group": "the whole task", "milliseconds": 300_000, "cost_usd": 0.01},
                {"number": 2, "group": "the self-check", "milliseconds": 80_000, "cost_usd": 0.003,
                 "changed_workspace": false, "status": "done"},
            ],
            "moves": [
                {"kind": "lean", "after_session": 1, "self_check": false, "kept": true,
                 "score": {"passed": 1, "total": 1}, "hardcoded": {"p": 0.02, "flagged": false}},
                {"kind": "lean", "after_session": 2, "self_check": true,
                 "score": {"passed": 1, "total": 1}, "hardcoded": {"p": 0.02, "flagged": false}},
            ],
        });
        std::fs::write(artifacts.join("microluna-1.json"), record.to_string()).unwrap();
        let rows = replay(&[dir.path().to_path_buf()], &BTreeMap::new());
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.trial, "task__abc");
        assert_eq!(row.reward, Some(0.0));
        assert_eq!(row.session, Some(1));
        assert_eq!(row.review_ms, Some(80_000));
        let decision = row.decision.as_ref().unwrap();
        // Score full and no hard-coding: clear. No executed record: the
        // regression and coverage triggers are unknown.
        assert_eq!(decision.fired, Vec::<Trigger>::new());
        assert_eq!(
            decision.unknown,
            vec![Trigger::Regressed, Trigger::Uncovered]
        );
        assert_eq!(row.review_if_unknown_fires, Some(true));
        assert_eq!(row.review_if_unknown_clear, Some(false));
        // The score script names the report but not the summary.
        let proxy = row.uncovered_proxy.as_ref().unwrap();
        assert_eq!(proxy.reading, Reading::Fired, "{proxy:?}");
        assert_eq!(row.review_proxy, Some(true));
        let summary = summary(&rows);
        assert_eq!(
            summary["all"]["rule_unknown_clear"]["saved_review_ms"],
            80_000
        );
        assert_eq!(summary["all"]["rule_unknown_fires"]["saved_review_ms"], 0);
    }
}
