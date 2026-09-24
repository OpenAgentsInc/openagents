//! Coder One's tunable composition, per Terminal-Bench attempt.
//!
//! A composed episode (`control.route`, `control.handoff`,
//! `control.horizon`, and `verify` in its policy manifest) writes
//! `artifacts/composition.json` (`openagents.coder-one.composition.v3`; v2
//! before `verify.second` named its triggers and recorded its outcome and
//! cost, and v1 before persistence rounds recorded their deltas):
//! where the route started and why, the deadline each dispatch asked for,
//! every dispatch with its tier, status, time, and cost, each handoff and
//! its trigger, the checks after each dispatch with what
//! `generic.self-report` found, `verify.support`'s states, the repair, and
//! `verify.second`'s second executor, and each `control.persist` round with
//! its executor, own tests fixed and broken, and cost. This module reads it
//! from each attempt and renders it, and `--escalations` reports each
//! escalation's trigger, executor, and outcome, and the conditional success
//! of the escalated attempts.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::terminal_bench::{Attempt, Records};

/// `text` cut to `width` characters, with an ellipsis when cut.
fn short(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        text.to_owned()
    } else {
        let kept: String = text.chars().take(width.saturating_sub(1)).collect();
        format!("{kept}…")
    }
}

/// The schema a composition record carries.
pub const SCHEMA: &str = "openagents.coder-one.composition.v3";

/// The schemas this module reads: v1 has no persistence deltas, and v2 no
/// escalation triggers by name.
pub const SCHEMAS: [&str; 3] = [
    "openagents.coder-one.composition.v1",
    "openagents.coder-one.composition.v2",
    SCHEMA,
];

/// Whether a record carries a schema this module reads.
fn readable(record: &Value) -> bool {
    SCHEMAS.iter().any(|schema| record["schema"] == *schema)
}

/// The schema of this module's JSON.
pub const VIEW_SCHEMA: &str = "openagents.gym.coder-composition.v1";

/// The schema of `--escalations --json`.
pub const ESCALATIONS_SCHEMA: &str = "openagents.gym.coder-escalations.v1";

/// The schema of `--best-of --json`.
pub const BEST_OF_SCHEMA: &str = "openagents.gym.coder-best-of.v1";

/// Where an episode keeps the verifier's grade of each `control.best_of`
/// candidate, relative to the episode: `{"schema": BEST_OF_GRADES_SCHEMA,
/// "rewards": [1.0, 0.0, null]}`, one reward per candidate number, `null`
/// where the verifier gave none. The loader attaches it to the
/// composition record as `best_of.grades`.
pub const BEST_OF_GRADES: &str = "best-of/grades.json";

/// The schema of [`BEST_OF_GRADES`].
pub const BEST_OF_GRADES_SCHEMA: &str = "openagents.coder-one.best-of-grades.v1";

fn words(value: &Value) -> String {
    value.as_str().map_or_else(|| "?".to_owned(), str::to_owned)
}

fn money(value: &Value) -> String {
    value
        .as_f64()
        .map_or_else(|| "unknown".to_owned(), |usd| format!("${usd:.4}"))
}

fn seconds(ms: &Value) -> String {
    ms.as_u64().map_or_else(
        || "?".to_owned(),
        |ms| format!("{:.0}s", ms as f64 / 1000.0),
    )
}

fn tier(value: &Value) -> String {
    let label = format!(
        "{}/{}",
        value["agent"].as_str().unwrap_or("?"),
        value["model"].as_str().unwrap_or("?")
    );
    match value["effort"].as_str() {
        Some(effort) => format!("{label} ({effort})"),
        None => label,
    }
}

/// The verdict counts of a checks summary, such as `2 passed · 1 failed`.
fn verdicts(summary: &Value) -> String {
    let counts: Vec<String> = summary["verdicts"]
        .as_object()
        .into_iter()
        .flatten()
        .map(|(word, n)| format!("{n} {word}"))
        .collect();
    if counts.is_empty() {
        "no scenario ran".to_owned()
    } else {
        counts.join(" · ")
    }
}

/// One composed attempt.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub job: String,
    pub trial: String,
    pub task: String,
    pub arm: String,
    pub reward: Option<f64>,
    pub cost_usd: Option<f64>,
    pub agent_ms: Option<u64>,
    pub record: Value,
}

impl Row {
    fn of(attempt: &Attempt) -> Option<Self> {
        let record = attempt.composition.clone()?;
        readable(&record).then(|| Row {
            job: attempt.job.clone(),
            trial: attempt.trial.clone(),
            task: attempt.task.clone(),
            arm: attempt.arm.clone(),
            reward: attempt.reward,
            cost_usd: attempt.cost_usd,
            agent_ms: attempt.phases_ms[2],
            record,
        })
    }

    /// Where the route started: `cheap`, `strong`, or `manifest`.
    #[must_use]
    pub fn start(&self) -> String {
        words(&self.record["first"]["start"])
    }

    /// The dispatches' roles, in order.
    #[must_use]
    pub fn roles(&self) -> Vec<String> {
        self.record["branches"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|b| words(&b["role"]))
            .collect()
    }

    /// The last checks' summary.
    #[must_use]
    pub fn last_checks(&self) -> Option<&Value> {
        self.record["checks"]
            .as_array()
            .and_then(|checks| checks.last())
            .map(|c| &c["summary"])
    }

    fn line(&self) -> String {
        format!(
            "{:<28} {:<24} {:>6} {:>9} {:>7}  {:<8} {:<34} {:<24} {}",
            short(&self.task, 28),
            short(&self.arm, 24),
            self.reward
                .map_or_else(|| "—".to_owned(), |r| format!("{r:.1}")),
            self.cost_usd
                .map_or_else(|| "—".to_owned(), |usd| format!("${usd:.3}")),
            self.agent_ms.map_or_else(
                || "—".to_owned(),
                |ms| format!("{:.0}s", ms as f64 / 1000.0)
            ),
            self.start(),
            short(&self.roles().join(", "), 34),
            short(
                &self
                    .last_checks()
                    .map_or_else(|| "no checks".to_owned(), verdicts),
                24
            ),
            repair_word(&self.record["repair"]),
        )
    }

    fn to_json(&self) -> Value {
        json!({
            "job": self.job,
            "trial": self.trial,
            "task": self.task,
            "arm": self.arm,
            "reward": self.reward,
            "cost_usd": self.cost_usd,
            "agent_ms": self.agent_ms,
            "start": self.start(),
            "effort": self.record["effort"]["effort"],
            "effort_score": self.record["effort"]["score"],
            "dispatches": self.roles(),
            "escalated": self.record["escalated"],
            "second_kept": self.record["second"]["kept"],
            "second_fired": fired(&self.record["second"]),
            "best_of_n": self.record["best_of"]["n"],
            "best_of_kept": self.record["best_of"]["kept"],
            "best_of_verdicts": self.record["best_of"]["verdicts"],
            "best_of_oracle": BestOf::of(self).oracle,
            "persist_rounds": self.record["persist"]["rounds"].as_array().map(Vec::len),
            "persist_stopped": self.record["persist"]["stopped"],
            "persist_totals": self.record["persist"]["totals"],
            "last_checks": self.last_checks(),
            "composition": self.record,
        })
    }
}

fn repair_word(repair: &Value) -> String {
    if repair.is_null() {
        "no repair".to_owned()
    } else if repair["ran"] == true {
        format!(
            "repaired{}",
            if repair["changed"] == true {
                ", changed"
            } else {
                ", unchanged"
            }
        )
    } else {
        format!(
            "repair skipped: {}",
            repair["skipped"].as_str().unwrap_or("not triggered")
        )
    }
}

/// Every composed attempt, newest first.
#[must_use]
pub fn rows(records: &Records) -> Vec<Row> {
    let mut rows: Vec<(Option<String>, Row)> = records
        .attempts
        .iter()
        .filter_map(|a| Row::of(a).map(|row| (a.started_at.clone(), row)))
        .collect();
    rows.sort_by(|a, b| b.0.cmp(&a.0));
    rows.into_iter().map(|(_, row)| row).collect()
}

/// One composition record as text: the route, the horizon, each dispatch,
/// each handoff, the checks after each dispatch, support, and the repair.
#[must_use]
pub fn detail_lines(record: &Value) -> Vec<String> {
    let mut lines = vec!["Composition: route, run, check, hand off, and repair".to_owned()];
    let first = &record["first"];
    let route = &record["route"];
    let profile = &route["profile"];
    lines.push(format!(
        "  route: start {} with {} · {}{}",
        words(&first["start"]),
        tier(&first["tier"]),
        words(&first["reason"]),
        profile["difficulty"]
            .as_f64()
            .filter(|_| !words(&first["reason"]).contains("difficulty"))
            .map_or(String::new(), |d| format!(" · difficulty {d:.2}"))
    ));
    if let Some(family) = route["family"].as_object() {
        lines.push(format!(
            "  route family: {} · {}",
            family
                .get("family")
                .and_then(Value::as_str)
                .unwrap_or("none"),
            family.get("why").and_then(Value::as_str).unwrap_or("")
        ));
    }
    if let Some(effort) = record["effort"].as_object() {
        let features: Vec<String> = effort
            .get("features")
            .and_then(Value::as_object)
            .into_iter()
            .flatten()
            .filter_map(|(id, p)| p.as_f64().map(|p| format!("{id} {p:.2}")))
            .collect();
        lines.push(format!(
            "  effort: {} · {}{}",
            effort.get("effort").map_or_else(|| "?".to_owned(), words),
            effort.get("reason").map_or_else(|| "?".to_owned(), words),
            if features.is_empty() {
                String::new()
            } else {
                format!(" · {}", features.join(", "))
            }
        ));
    }
    let horizon = &record["horizon"];
    lines.push(format!(
        "  horizon: episode deadline {} · first dispatch asked {}s · checks {}s, {}s per command{}",
        horizon["episode_deadline_sec"]
            .as_u64()
            .map_or_else(|| "none".to_owned(), |s| format!("{s}s")),
        horizon["first_dispatch_sec"].as_u64().unwrap_or(0),
        horizon["check_budget"]["seconds"].as_u64().unwrap_or(0),
        horizon["command_sec"].as_u64().unwrap_or(0),
        if horizon["long"] == true {
            " · long task"
        } else {
            ""
        }
    ));
    if let Some(planner) = record["planner"].as_object() {
        lines.push(format!(
            "  planner: {}",
            planner.get("skipped").and_then(Value::as_str).map_or_else(
                || format!(
                    "{} · plan {} characters",
                    words(planner.get("status").unwrap_or(&Value::Null)),
                    planner
                        .get("plan_chars")
                        .and_then(Value::as_u64)
                        .unwrap_or(0)
                ),
                |why| format!("skipped: {why}")
            )
        ));
    }
    for branch in record["branches"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  dispatch {:<10} {:<42} {:<10} {:>6} of {:>6}s asked · {}{}",
            words(&branch["role"]),
            tier(&branch["tier"]),
            words(&branch["status"]),
            seconds(&branch["milliseconds"]),
            branch["requested_sec"].as_u64().unwrap_or(0),
            money(&branch["usd"]),
            branch["stopped_by"]
                .as_str()
                .map_or(String::new(), |by| format!(" · stopped by {by}"))
        ));
        lines.extend(parallel_lines(&branch["parallel"]));
    }
    for handoff in record["handoffs"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  handoff {}: from {} to {} · {}",
            words(&handoff["action"]),
            handoff["from"].as_str().unwrap_or("-"),
            handoff["to"].as_str().unwrap_or("-"),
            handoff["trigger"]
                .as_str()
                .or_else(|| handoff["why"].as_str())
                .unwrap_or("")
        ));
    }
    for checks in record["checks"].as_array().into_iter().flatten() {
        let summary = &checks["summary"];
        let states: Vec<String> = summary["requirements"]
            .as_object()
            .into_iter()
            .flatten()
            .map(|(state, n)| format!("{n} {state}"))
            .collect();
        lines.push(format!(
            "  checks after {}: {} scenarios · {} · requirements {} · {} packets",
            words(&checks["after"]),
            summary["scenarios"].as_u64().unwrap_or(0),
            verdicts(summary),
            if states.is_empty() {
                "none".to_owned()
            } else {
                states.join(", ")
            },
            summary["packets"].as_u64().unwrap_or(0)
        ));
        let reported = &checks["self_report"];
        if let Some(verdict) = reported["verdict"].as_str() {
            let findings = reported["findings"].as_array().cloned().unwrap_or_default();
            lines.push(format!(
                "    self-report {verdict}{}",
                reported["requirement"]
                    .as_str()
                    .map_or(String::new(), |r| format!(" on {r}"))
            ));
            for finding in findings.iter().take(3) {
                lines.push(format!(
                    "      {}: {}",
                    words(&finding["signal"]),
                    short(finding["evidence"].as_str().unwrap_or_default(), 140)
                ));
            }
        }
    }
    if let Some(support) = record["support"].as_object() {
        lines.push(format!(
            "  support: {}",
            [
                "judged",
                "supported",
                "contradicted",
                "unresolved",
                "skipped"
            ]
            .iter()
            .map(|key| format!(
                "{} {key}",
                support.get(*key).and_then(Value::as_u64).unwrap_or(0)
            ))
            .collect::<Vec<_>>()
            .join(" · ")
        ));
    }
    let repair = &record["repair"];
    let second = &record["second"];
    let second_line = if second.is_null() {
        None
    } else if let Some(why) = second["skipped"].as_str() {
        Some(format!("  second: skipped: {why}"))
    } else {
        Some(format!(
            "  second: {} · {} · kept the {} candidate · {}{}",
            tier(&second["tier"]),
            words(&second["trigger"]),
            words(&second["kept"]),
            words(&second["why"]),
            if second.get("cost_usd").is_some() {
                format!(
                    " · {} · {}",
                    money(&second["cost_usd"]),
                    seconds(&second["milliseconds"])
                )
            } else {
                String::new()
            }
        ))
    };
    lines.push(format!(
        "  repair: {}{}",
        repair_word(repair),
        if repair["ran"] == true {
            format!(
                " · {} · {}",
                tier(&repair["tier"]),
                money(&repair["cost_usd"])
            )
        } else {
            String::new()
        }
    ));
    lines.extend(second_line);
    lines.extend(persist_lines(&record["persist"]));
    lines.extend(best_of_lines(&record["best_of"]));
    lines
}

/// `control.best_of`: each candidate's status, cost, time, checks, and
/// verdict, the verifier's grade when the candidates were graded, and
/// which one was kept and why.
/// A Microluna suite loop's timeline under its dispatch: the summary, then
/// one line per session or writer with when it ran and what beside it.
#[must_use]
pub fn parallel_lines(parallel: &Value) -> Vec<String> {
    if !parallel.is_object() {
        return Vec::new();
    }
    let secs = |ms: &Value| {
        ms.as_u64().map_or_else(
            || "?".to_owned(),
            |ms| format!("{:.1}s", ms as f64 / 1000.0),
        )
    };
    let mut lines = vec![format!(
        "    parallel: {} sessions · wall {} · session time {} · concurrency {} · peak {} · critical path {} · saved {} · suite {} ({} on the critical path) · {} merges, {} conflicts",
        parallel["sessions"].as_u64().unwrap_or(0),
        secs(&parallel["wall_ms"]),
        secs(&parallel["session_ms"]),
        parallel["concurrency"]
            .as_f64()
            .map_or("?".to_owned(), |c| format!("{c:.2}×")),
        parallel["peak"].as_u64().unwrap_or(0),
        secs(&parallel["critical_path_ms"]),
        secs(&parallel["saved_ms"]),
        secs(&parallel["suite_ms"]),
        secs(&parallel["suite_on_critical_path_ms"]),
        parallel["merges"].as_u64().unwrap_or(0),
        parallel["conflicts"].as_u64().unwrap_or(0),
    )];
    let tracks = parallel["tracks"].as_array().cloned().unwrap_or_default();
    for track in &tracks {
        let label = track["label"].as_str().unwrap_or("?");
        let (start, end) = (
            track["start_ms"].as_u64().unwrap_or(0),
            track["end_ms"].as_u64().unwrap_or(0),
        );
        let beside: Vec<&str> = tracks
            .iter()
            .filter(|other| other["label"] != track["label"])
            .filter(|other| {
                let nested = (track["kind"] == "define" && other["kind"] == "writer")
                    || (track["kind"] == "writer" && other["kind"] == "define");
                !nested
                    && other["start_ms"].as_u64().unwrap_or(0) < end
                    && other["end_ms"].as_u64().unwrap_or(0) > start
            })
            .filter_map(|other| other["label"].as_str())
            .collect();
        lines.push(format!(
            "      {label:<22} {:>7} to {:<7} {}{}{}",
            secs(&track["start_ms"]),
            secs(&track["end_ms"]),
            track["batch"].as_str().unwrap_or(""),
            track["group"]
                .as_str()
                .map_or(String::new(), |g| format!(" · {g}")),
            if beside.is_empty() {
                String::new()
            } else {
                format!(" · alongside {}", beside.join(", "))
            }
        ));
    }
    lines
}

fn best_of_lines(best: &Value) -> Vec<String> {
    if best.is_null() {
        return Vec::new();
    }
    if let Some(why) = best["skipped"].as_str() {
        return vec![format!(
            "  best of {}: one candidate ran: {why}",
            best["n"].as_u64().unwrap_or(0)
        )];
    }
    let rewards = best["grades"]["rewards"].as_array();
    let mut lines = vec![format!(
        "  best of {}: kept candidate {} · {}{}",
        best["n"].as_u64().unwrap_or(0),
        best["kept"].as_u64().unwrap_or(0),
        words(&best["why"]),
        if best["leaked"] == true {
            " · the real workspace changed while the candidates ran"
        } else {
            ""
        }
    )];
    for candidate in best["candidates"].as_array().into_iter().flatten() {
        let number = candidate["number"].as_u64().unwrap_or(0);
        let verdict = &candidate["verdict"];
        let grade = rewards
            .and_then(|r| r.get(usize::try_from(number).unwrap_or(0).saturating_sub(1)))
            .map(|reward| {
                reward.as_f64().map_or_else(
                    || " · verifier: none".to_owned(),
                    |r| format!(" · verifier {r:.1}"),
                )
            })
            .unwrap_or_default();
        lines.push(format!(
            "    candidate {number}{} {:<10} {:>6} · {} · {} · verdict {}{}{grade}",
            if best["kept"] == number { "*" } else { " " },
            words(&candidate["status"]),
            seconds(&candidate["milliseconds"]),
            money(&candidate["cost_usd"]),
            if candidate["checks"].is_object() {
                verdicts(&candidate["checks"])
            } else {
                "no checks".to_owned()
            },
            verdict["call"].as_str().unwrap_or("not asked"),
            verdict["p_fail"]
                .as_f64()
                .map_or_else(String::new, |p| format!(" (p_fail {p:.2})")),
        ));
    }
    lines
}

/// One attempt's `control.best_of` outcome, or a single candidate's.
#[derive(Clone, Debug, PartialEq)]
pub struct BestOf {
    /// Candidates that ran: `best_of.n`, or 1 without `best_of` or when it
    /// was skipped.
    pub candidates: usize,
    /// Whether any candidate passed the verifier: the kept one's reward for
    /// a single candidate, the graded candidates' otherwise; `None` when
    /// the candidates weren't all graded.
    pub oracle: Option<bool>,
    /// The kept candidate's verdict call.
    pub verdict: Option<String>,
}

impl BestOf {
    /// The outcome of `row`.
    #[must_use]
    pub fn of(row: &Row) -> Self {
        let best = &row.record["best_of"];
        let ran = best["candidates"]
            .as_array()
            .filter(|_| best["skipped"].is_null());
        let passed = row.reward.map(|r| r >= 1.0);
        match ran {
            Some(candidates) => {
                let rewards = best["grades"]["rewards"].as_array();
                let oracle = rewards.and_then(|rewards| {
                    let graded: Vec<f64> = rewards.iter().filter_map(Value::as_f64).collect();
                    if graded.iter().any(|r| *r >= 1.0) {
                        Some(true)
                    } else if graded.len() == candidates.len() {
                        Some(false)
                    } else {
                        None
                    }
                });
                // The kept candidate passing is enough to know one did.
                let oracle = if passed == Some(true) {
                    Some(true)
                } else {
                    oracle
                };
                let kept = best["kept"].as_u64().unwrap_or(0);
                let verdict = candidates
                    .iter()
                    .find(|c| c["number"] == kept)
                    .and_then(|c| c["verdict"]["call"].as_str())
                    .map(str::to_owned);
                BestOf {
                    candidates: candidates.len(),
                    oracle,
                    verdict,
                }
            }
            None => BestOf {
                candidates: 1,
                oracle: passed,
                verdict: row.record["verdict"]["first"]["call"]
                    .as_str()
                    .map(str::to_owned),
            },
        }
    }
}

/// `--best-of`: per arm, the pass rate beside the oracle rate (whether any
/// candidate passed), the selection's accuracy on the attempts where one
/// did, and the cost and time per attempt and per pass.
#[derive(Clone, Debug, PartialEq)]
pub struct BestOfArm {
    pub arm: String,
    pub attempts: usize,
    pub graded: usize,
    pub passed: usize,
    /// Attempts whose every candidate was graded, or whose kept one passed.
    pub oracle_known: usize,
    pub oracle_passed: usize,
    /// Of the attempts where some candidate passed, those whose kept
    /// candidate passed.
    pub selected_right: usize,
    pub candidates: usize,
    pub cost_usd: f64,
    pub cost_unknown: usize,
    pub agent_ms: u64,
    /// The kept candidates' verdict calls.
    pub verdicts: std::collections::BTreeMap<String, usize>,
}

impl BestOfArm {
    /// Every arm among `rows`, in name order. An experiment's job,
    /// `PROFILE--ARM--TASK--RUN`, names the arm; one profile can run
    /// several arms with different policies, so the job's arm wins over
    /// the attempt's profile.
    #[must_use]
    pub fn of(rows: &[Row]) -> Vec<BestOfArm> {
        let mut arms: std::collections::BTreeMap<String, BestOfArm> =
            std::collections::BTreeMap::new();
        for row in rows {
            let parts: Vec<&str> = row.job.split("--").collect();
            let name = if parts.len() >= 4 {
                parts[1].to_owned()
            } else {
                row.arm.clone()
            };
            let arm = arms.entry(name.clone()).or_insert_with(|| BestOfArm {
                arm: name,
                attempts: 0,
                graded: 0,
                passed: 0,
                oracle_known: 0,
                oracle_passed: 0,
                selected_right: 0,
                candidates: 0,
                cost_usd: 0.0,
                cost_unknown: 0,
                agent_ms: 0,
                verdicts: std::collections::BTreeMap::new(),
            });
            let outcome = BestOf::of(row);
            let passed = row.reward.is_some_and(|r| r >= 1.0);
            arm.attempts += 1;
            arm.graded += usize::from(row.reward.is_some());
            arm.passed += usize::from(passed);
            if let Some(oracle) = outcome.oracle {
                arm.oracle_known += 1;
                arm.oracle_passed += usize::from(oracle);
                arm.selected_right += usize::from(oracle && passed);
            }
            arm.candidates += outcome.candidates;
            match row.cost_usd {
                Some(usd) => arm.cost_usd += usd,
                None => arm.cost_unknown += 1,
            }
            arm.agent_ms += row.agent_ms.unwrap_or(0);
            *arm.verdicts
                .entry(outcome.verdict.unwrap_or_else(|| "none".to_owned()))
                .or_default() += 1;
        }
        arms.into_values().collect()
    }

    fn rate(part: usize, whole: usize) -> String {
        if whole == 0 {
            "—".to_owned()
        } else {
            format!(
                "{part}/{whole} ({:.0}%)",
                100.0 * part as f64 / whole as f64
            )
        }
    }

    fn to_json(&self) -> Value {
        json!({
            "arm": self.arm,
            "attempts": self.attempts,
            "graded": self.graded,
            "passed": self.passed,
            "oracle_known": self.oracle_known,
            "oracle_passed": self.oracle_passed,
            "selected_right": self.selected_right,
            "candidates": self.candidates,
            "cost_usd": self.cost_usd,
            "cost_unknown": self.cost_unknown,
            "cost_per_pass_usd": (self.passed > 0).then(|| self.cost_usd / self.passed as f64),
            "agent_ms": self.agent_ms,
            "kept_verdicts": self.verdicts,
        })
    }
}

/// `--best-of` as text.
#[must_use]
pub fn best_of_summary(arms: &[BestOfArm]) -> Vec<String> {
    let mut lines = vec![
        "Best of N · the pass rate beside the oracle rate (any candidate passed)".to_owned(),
        format!(
            "{:<34} {:>5} {:>12} {:>12} {:>12} {:>10} {:>11} {:>9}  {}",
            "arm",
            "cands",
            "passed",
            "oracle",
            "selection",
            "cost",
            "cost/pass",
            "agent/att",
            "kept verdicts"
        ),
    ];
    for arm in arms {
        lines.push(format!(
            "{:<34} {:>5} {:>12} {:>12} {:>12} {:>10} {:>11} {:>9}  {}",
            short(&arm.arm, 34),
            arm.candidates,
            BestOfArm::rate(arm.passed, arm.graded),
            BestOfArm::rate(arm.oracle_passed, arm.oracle_known),
            BestOfArm::rate(arm.selected_right, arm.oracle_passed),
            format!(
                "${:.3}{}",
                arm.cost_usd,
                if arm.cost_unknown > 0 { "+" } else { "" }
            ),
            if arm.passed > 0 {
                format!("${:.3}", arm.cost_usd / arm.passed as f64)
            } else {
                "—".to_owned()
            },
            if arm.attempts > 0 {
                format!("{:.0}s", arm.agent_ms as f64 / 1000.0 / arm.attempts as f64)
            } else {
                "—".to_owned()
            },
            arm.verdicts
                .iter()
                .map(|(call, n)| format!("{call} {n}"))
                .collect::<Vec<_>>()
                .join(", "),
        ));
    }
    if arms.iter().any(|a| a.oracle_known < a.attempts) {
        lines.push(format!(
            "The oracle is unknown where a candidate wasn't graded: grade each archived candidate with `tbench verify --candidate` and write {BEST_OF_GRADES} in the episode."
        ));
    }
    lines
}

/// `control.persist`: each round's executor, time, cost, change, and
/// checks, and why the rounds stopped.
fn persist_lines(persist: &Value) -> Vec<String> {
    if persist.is_null() {
        return Vec::new();
    }
    if let Some(why) = persist["skipped"].as_str() {
        return vec![format!("  persist: skipped: {why}")];
    }
    let rounds = persist["rounds"].as_array().cloned().unwrap_or_default();
    let totals = &persist["totals"];
    let mut lines = vec![format!(
        "  persist: {} round{} · stopped: {}{}",
        rounds.len(),
        if rounds.len() == 1 { "" } else { "s" },
        persist["stopped"].as_str().unwrap_or("-"),
        if totals.is_object() {
            format!(
                " · own tests {} fixed, {} broken · {} · {} escalation{}{}",
                totals["tests_fixed"].as_u64().unwrap_or(0),
                totals["tests_broken"].as_u64().unwrap_or(0),
                money(&totals["cost_usd"]),
                totals["escalations"].as_u64().unwrap_or(0),
                if totals["escalations"] == 1 { "" } else { "s" },
                persist["spend"]["cap_usd"]
                    .as_f64()
                    .map_or_else(String::new, |cap| format!(" · cap ${cap:.2}"))
            )
        } else {
            String::new()
        }
    )];
    for round in &rounds {
        let files = round["files_changed"].as_array().map_or(0, Vec::len);
        let checks = if round["after"].is_object() {
            format!(
                " · failing checks {} before, {} after",
                round["before"]["failed"].as_u64().unwrap_or(0),
                round["after"]["failed"].as_u64().unwrap_or(0)
            )
        } else {
            String::new()
        };
        let delta = &round["delta"];
        let tests = if delta.is_object() && round["tests"].is_object() {
            format!(
                " · own tests {} fixed, {} broken ({} failing)",
                delta["tests_fixed"].as_u64().unwrap_or(0),
                delta["tests_broken"].as_u64().unwrap_or(0),
                round["tests"]["failed"].as_u64().unwrap_or(0)
            )
        } else {
            String::new()
        };
        let class = match round["class"].as_str() {
            Some(class) if class != "strong" => format!(" [{class}]"),
            _ => String::new(),
        };
        lines.push(format!(
            "    round {} {:<42} {:<10} {:>6} · {} · {}{}{}{}{}",
            round["round"],
            format!("{}{class}", tier(&round["tier"])),
            words(&round["status"]),
            seconds(&round["milliseconds"]),
            money(&round["cost_usd"]),
            if round["changed"] == true {
                format!("{files} file{} changed", if files == 1 { "" } else { "s" })
            } else {
                "no change".to_owned()
            },
            checks,
            tests,
            if round["kept"] == false {
                " · put back"
            } else {
                ""
            },
            if round["next"] == "escalate" {
                " · escalates"
            } else {
                ""
            }
        ));
    }
    lines
}

/// The composed attempts as text: one row each, then the detail of the
/// newest, or of each one `query` names.
#[must_use]
pub fn lines(rows: &[Row], query: Option<&str>) -> Vec<String> {
    let mut lines = vec![format!(
        "Coder One composition · {} composed attempts",
        rows.len()
    )];
    if rows.is_empty() {
        lines.push(
            "No attempt recorded artifacts/composition.json. Run a coder-one-tunable arm, or one whose manifest sets control.route, control.handoff, control.horizon, or verify.".to_owned(),
        );
        return lines;
    }
    lines.push(format!(
        "{:<28} {:<24} {:>6} {:>9} {:>7}  {:<8} {:<34} {:<24} {}",
        "task", "arm", "reward", "cost", "agent", "start", "dispatches", "last checks", "repair"
    ));
    for row in rows {
        lines.push(row.line());
    }
    let chosen: Vec<&Row> = match query {
        Some(query) => rows
            .iter()
            .filter(|r| r.task.contains(query) || r.job.contains(query) || r.trial.contains(query))
            .collect(),
        None => rows.iter().take(1).collect(),
    };
    for row in chosen {
        lines.push(String::new());
        lines.push(format!("{} / {} ({})", row.job, row.trial, row.task));
        lines.extend(detail_lines(&row.record));
    }
    lines
}

/// The composed attempts as versioned JSON.
#[must_use]
pub fn to_json(rows: &[Row]) -> Value {
    json!({
        "schema": VIEW_SCHEMA,
        "attempts": rows.iter().map(Row::to_json).collect::<Vec<_>>(),
    })
}

/// The trigger names that fired for a `verify.second` record: `fired`
/// when the record has it (composition v3), else the words before each
/// `:` of its `trigger` text.
fn fired(second: &Value) -> Vec<String> {
    if let Some(names) = second["fired"].as_array() {
        return names
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect();
    }
    let text = match &second["trigger"] {
        Value::String(text) => text.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(Value::as_str)
            .collect::<Vec<_>>()
            .join("; "),
        _ => String::new(),
    };
    text.split("; ")
        .filter_map(|part| part.split_once(':').map(|(name, _)| name.trim().to_owned()))
        .filter(|name| !name.is_empty())
        .collect()
}

/// One escalation: an attempt whose `verify.second` ran a second executor.
#[derive(Clone, Debug, PartialEq)]
pub struct Escalation {
    pub job: String,
    pub trial: String,
    pub task: String,
    pub arm: String,
    pub reward: Option<f64>,
    /// The trigger names that fired.
    pub fired: Vec<String>,
    /// The second executor.
    pub executor: String,
    /// Whether the host kept the second candidate.
    pub kept_second: bool,
    /// The second executor's cost, when it reported one.
    pub cost_usd: Option<f64>,
    pub milliseconds: Option<u64>,
}

impl Escalation {
    fn of(row: &Row) -> Option<Self> {
        let second = &row.record["second"];
        if second.is_null() || second.get("skipped").is_some() || second["kept"].is_null() {
            return None;
        }
        let branch = row.record["branches"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|b| b["role"] == "second");
        let cost_usd = second["cost_usd"]
            .as_f64()
            .or_else(|| branch.and_then(|b| b["usd"].as_f64()));
        let milliseconds = second["milliseconds"]
            .as_u64()
            .or_else(|| branch.and_then(|b| b["milliseconds"].as_u64()));
        Some(Escalation {
            job: row.job.clone(),
            trial: row.trial.clone(),
            task: row.task.clone(),
            arm: row.arm.clone(),
            reward: row.reward,
            fired: fired(second),
            executor: tier(&second["tier"]),
            kept_second: second["kept"] == "second",
            cost_usd,
            milliseconds,
        })
    }

    /// Whether the verifier passed the attempt.
    #[must_use]
    pub fn passed(&self) -> Option<bool> {
        self.reward.map(|r| r >= 1.0)
    }

    fn to_json(&self) -> Value {
        json!({
            "job": self.job,
            "trial": self.trial,
            "task": self.task,
            "arm": self.arm,
            "reward": self.reward,
            "fired": self.fired,
            "executor": self.executor,
            "outcome": if self.kept_second { "kept_second" } else { "kept_first" },
            "cost_usd": self.cost_usd,
            "milliseconds": self.milliseconds,
        })
    }
}

/// The escalations over a set of composed attempts.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EscalationSummary {
    /// Composed attempts read.
    pub attempts: usize,
    /// Attempts where a trigger fired but the second executor was skipped,
    /// with why.
    pub skipped_after_trigger: Vec<(String, String)>,
    pub escalations: Vec<Escalation>,
}

impl EscalationSummary {
    /// Summarizes `rows`.
    #[must_use]
    pub fn of(rows: &[Row]) -> Self {
        let mut summary = EscalationSummary {
            attempts: rows.len(),
            ..EscalationSummary::default()
        };
        for row in rows {
            if let Some(escalation) = Escalation::of(row) {
                summary.escalations.push(escalation);
                continue;
            }
            let second = &row.record["second"];
            if let Some(why) = second["skipped"].as_str()
                && !fired(second).is_empty()
            {
                summary
                    .skipped_after_trigger
                    .push((row.task.clone(), why.to_owned()));
            }
        }
        summary
    }

    /// The escalated attempts the verifier graded.
    fn graded(&self) -> impl Iterator<Item = &Escalation> {
        self.escalations.iter().filter(|e| e.passed().is_some())
    }

    /// Escalated, graded attempts that kept the second candidate and
    /// passed: the first candidate, which a check flagged, was set aside.
    #[must_use]
    pub fn rescued(&self) -> usize {
        self.graded()
            .filter(|e| e.kept_second && e.passed() == Some(true))
            .count()
    }

    /// `(passed, failed)` among graded escalations that kept `second` or
    /// not.
    fn outcomes(&self, second: bool) -> (usize, usize) {
        let kept: Vec<&Escalation> = self.graded().filter(|e| e.kept_second == second).collect();
        let passed = kept.iter().filter(|e| e.passed() == Some(true)).count();
        (passed, kept.len() - passed)
    }

    /// Per trigger name, how many escalations it fired in.
    #[must_use]
    pub fn by_trigger(&self) -> std::collections::BTreeMap<String, usize> {
        let mut out = std::collections::BTreeMap::new();
        for e in &self.escalations {
            for name in &e.fired {
                *out.entry(name.clone()).or_insert(0) += 1;
            }
        }
        out
    }

    /// The escalations' reported cost, and how many reported none.
    #[must_use]
    pub fn cost(&self) -> (f64, usize) {
        // A float sum of nothing is -0.0; start from 0.0 so it prints as $0.
        let known: f64 = self
            .escalations
            .iter()
            .filter_map(|e| e.cost_usd)
            .fold(0.0, |sum, usd| sum + usd);
        let unknown = self
            .escalations
            .iter()
            .filter(|e| e.cost_usd.is_none())
            .count();
        (known, unknown)
    }

    /// The report as text.
    #[must_use]
    pub fn lines(&self) -> Vec<String> {
        let graded = self.graded().count();
        let mut lines = vec![format!(
            "Escalation (verify.second) · {} composed attempts · {} escalated · {} graded",
            self.attempts,
            self.escalations.len(),
            graded
        )];
        if self.escalations.is_empty() {
            lines.push("  No attempt ran a second executor.".to_owned());
        }
        let triggers: Vec<String> = self
            .by_trigger()
            .iter()
            .map(|(name, n)| format!("{name} {n}"))
            .collect();
        if !triggers.is_empty() {
            lines.push(format!("  triggers: {}", triggers.join(" · ")));
        }
        let rescued = self.rescued();
        let percent = |n: usize| {
            if graded == 0 {
                "—".to_owned()
            } else {
                format!("{:.0}%", 100.0 * n as f64 / graded as f64)
            }
        };
        lines.push(format!(
            "  conditional success: {rescued} of {graded} graded escalations ({}) kept the second candidate and passed",
            percent(rescued)
        ));
        let (second_passed, second_failed) = self.outcomes(true);
        let (first_passed, first_failed) = self.outcomes(false);
        lines.push(format!(
            "  kept the second candidate: {} ({second_passed} passed, {second_failed} failed)",
            second_passed + second_failed
        ));
        lines.push(format!(
            "  kept the first candidate: {} ({first_passed} passed, {first_failed} failed)",
            first_passed + first_failed
        ));
        let (cost, unknown) = self.cost();
        let priced = self.escalations.len() - unknown;
        lines.push(format!(
            "  escalation cost: ${cost:.4} over {priced} priced{}{}",
            if priced > 0 {
                format!(" · mean ${:.4}", cost / priced as f64)
            } else {
                String::new()
            },
            if unknown > 0 {
                format!(" · {unknown} unreported")
            } else {
                String::new()
            }
        ));
        if graded > 0 {
            lines.push(format!(
                "  cost per rescue: {}",
                if rescued == 0 {
                    "no rescue".to_owned()
                } else {
                    format!("${:.4}", cost / rescued as f64)
                }
            ));
        }
        if !self.skipped_after_trigger.is_empty() {
            lines.push(format!(
                "  triggered but skipped: {}",
                self.skipped_after_trigger.len()
            ));
            for (task, why) in &self.skipped_after_trigger {
                lines.push(format!("    {task}: {why}"));
            }
        }
        if !self.escalations.is_empty() {
            lines.push(String::new());
            lines.push(format!(
                "  {:<28} {:<24} {:>6}  {:<20} {:<24} {:<12} {:>9} {:>7}",
                "task", "arm", "reward", "fired", "executor", "outcome", "cost", "time"
            ));
            for e in &self.escalations {
                lines.push(format!(
                    "  {:<28} {:<24} {:>6}  {:<20} {:<24} {:<12} {:>9} {:>7}",
                    short(&e.task, 28),
                    short(&e.arm, 24),
                    e.reward
                        .map_or_else(|| "—".to_owned(), |r| format!("{r:.1}")),
                    short(&e.fired.join("+"), 20),
                    short(&e.executor, 24),
                    if e.kept_second {
                        "kept second"
                    } else {
                        "kept first"
                    },
                    e.cost_usd
                        .map_or_else(|| "—".to_owned(), |usd| format!("${usd:.3}")),
                    e.milliseconds.map_or_else(
                        || "—".to_owned(),
                        |ms| format!("{:.0}s", ms as f64 / 1000.0)
                    ),
                ));
            }
        }
        lines
    }

    /// The report as versioned JSON.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let graded = self.graded().count();
        let (second_passed, second_failed) = self.outcomes(true);
        let (first_passed, first_failed) = self.outcomes(false);
        let (cost, unknown) = self.cost();
        json!({
            "schema": ESCALATIONS_SCHEMA,
            "attempts": self.attempts,
            "escalated": self.escalations.len(),
            "graded": graded,
            "rescued": self.rescued(),
            "conditional_success": (graded > 0).then(|| self.rescued() as f64 / graded as f64),
            "kept_second": { "passed": second_passed, "failed": second_failed },
            "kept_first": { "passed": first_passed, "failed": first_failed },
            "by_trigger": self.by_trigger(),
            "cost_usd": cost,
            "cost_unreported": unknown,
            "skipped_after_trigger": self.skipped_after_trigger.iter().map(|(task, why)| json!({ "task": task, "why": why })).collect::<Vec<_>>(),
            "escalations": self.escalations.iter().map(Escalation::to_json).collect::<Vec<_>>(),
        })
    }
}

const HELP: &str = "gym coder composition [QUERY] [--json]

Each Terminal-Bench attempt that ran Coder One's tunable composition: where
control.route started and why, the deadline each dispatch asked for, every
dispatch (planner, primary, escalation) with its tier, status, time, and
cost, each handoff and its trigger, verify.checks after each dispatch,
verify.support's states, the repair, verify.second, and each control.persist
round. QUERY picks the attempts whose task,
job, or trial contains it for detail; the newest is shown otherwise.

  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --no-jobs, --no-traces   leave out that source
  --arm NAME               only attempts of this arm
  --job TEXT               only attempts whose job name contains TEXT
  --escalations            report verify.second instead: each escalation's
                           triggers, executor, outcome, and cost, and the
                           conditional success, the graded escalations that
                           kept the second candidate and passed
  --best-of                report control.best_of instead: per arm, the
                           candidates run, the pass rate beside the oracle
                           rate (any candidate passed, from each episode's
                           best-of/grades.json), the selection's accuracy
                           where a candidate passed, and cost and time
  --json                   print versioned JSON instead of text";

/// `gym coder composition`.
///
/// # Errors
///
/// Returns a message for an unknown option or a failed write.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut jobs = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".openagents/terminal-bench/jobs"));
    let mut traces = Some(repo.join("traces"));
    let mut json_out = false;
    let mut escalations = false;
    let mut best_of = false;
    let mut arm: Option<String> = None;
    let mut job: Option<String> = None;
    let mut query = None;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_out = true,
            "--escalations" => escalations = true,
            "--best-of" => best_of = true,
            "--arm" | "--job" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{argument} needs a value"))?
                    .clone();
                if argument == "--arm" {
                    arm = Some(value);
                } else {
                    job = Some(value);
                }
                index += 1;
            }
            "--no-jobs" => jobs = None,
            "--no-traces" => traces = None,
            "--jobs-dir" | "--traces-dir" => {
                let value = args
                    .get(index + 1)
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                if argument == "--jobs-dir" {
                    jobs = Some(value.into());
                } else {
                    traces = Some(value.into());
                }
                index += 1;
            }
            other if other.starts_with('-') => return Err(format!("unknown option {other}")),
            other => query = Some(other.to_owned()),
        }
        index += 1;
    }
    let records = Records::load(jobs.as_deref(), traces.as_deref(), None);
    let rows: Vec<Row> = rows(&records)
        .into_iter()
        .filter(|row| arm.as_ref().is_none_or(|arm| &row.arm == arm))
        .filter(|row| {
            job.as_ref()
                .is_none_or(|job| row.job.contains(job.as_str()))
        })
        .collect();
    if best_of {
        let arms = BestOfArm::of(&rows);
        if json_out {
            let value = json!({
                "schema": BEST_OF_SCHEMA,
                "arms": arms.iter().map(BestOfArm::to_json).collect::<Vec<_>>(),
            });
            serde_json::to_writer_pretty(&mut *out, &value).map_err(|error| error.to_string())?;
            writeln!(out).map_err(|error| error.to_string())?;
        } else {
            for line in best_of_summary(&arms) {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
        return Ok(0);
    }
    if escalations {
        let summary = EscalationSummary::of(&rows);
        if json_out {
            serde_json::to_writer_pretty(&mut *out, &summary.to_json())
                .map_err(|error| error.to_string())?;
            writeln!(out).map_err(|error| error.to_string())?;
        } else {
            for line in summary.lines() {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
        return Ok(0);
    }
    if json_out {
        serde_json::to_writer_pretty(&mut *out, &to_json(&rows))
            .map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in lines(&rows, query.as_deref()) {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record() -> Value {
        json!({
            "schema": SCHEMA,
            "first": { "start": "cheap", "tier": { "agent": "codex", "model": "gpt-6-luna" }, "reason": "difficulty 0.25 is below 0.50" },
            "route": { "profile": { "difficulty": 0.25 } },
            "horizon": { "episode_deadline_sec": 780, "first_dispatch_sec": 429, "check_budget": { "seconds": 180 }, "command_sec": 60, "long": false },
            "planner": null,
            "branches": [
                { "role": "primary", "tier": { "agent": "codex", "model": "gpt-6-luna" }, "status": "answered", "milliseconds": 51000, "requested_sec": 429, "usd": 0.003 },
                { "role": "escalation", "tier": { "agent": "claude-code", "model": "claude-opus-5-5", "effort": "low" }, "status": "answered", "milliseconds": 24000, "requested_sec": 262, "usd": 0.05 }
            ],
            "handoffs": [ { "action": "escalate", "from": "codex/gpt-6-luna", "to": "claude-code/claude-opus-5-5", "trigger": "a check failed" } ],
            "escalated": true,
            "checks": [
                { "after": "primary", "summary": { "scenarios": 3, "verdicts": { "failed": 1, "passed": 2 }, "requirements": { "contradicted": 1, "observed": 1 }, "packets": 1 } },
                { "after": "escalation", "summary": { "scenarios": 3, "verdicts": { "passed": 3 }, "requirements": { "observed": 2 }, "packets": 0 } }
            ],
            "support": null,
            "repair": { "ran": false, "skipped": "no check contradicted a requirement" }
        })
    }

    #[test]
    fn a_composed_attempt_shows_its_route_dispatches_checks_and_repair() {
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.task = "fix-git".to_owned();
        attempt.arm = "coder-one-tunable".to_owned();
        attempt.reward = Some(1.0);
        attempt.composition = Some(record());
        let records = Records {
            attempts: vec![attempt, crate::terminal_bench::test_attempt()],
            ..Records::default()
        };
        let rows = rows(&records);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].start(), "cheap");
        assert_eq!(rows[0].roles(), ["primary", "escalation"]);
        let text = lines(&rows, None).join("\n");
        assert!(text.contains("primary, escalation"), "{text}");
        assert!(
            text.contains(
                "handoff escalate: from codex/gpt-6-luna to claude-code/claude-opus-5-5 · a check failed"
            ),
            "{text}"
        );
        assert!(
            text.contains("checks after primary: 3 scenarios · 1 failed · 2 passed"),
            "{text}"
        );
        assert!(
            text.contains("repair skipped: no check contradicted a requirement"),
            "{text}"
        );
        let value = to_json(&rows);
        assert_eq!(value["schema"], VIEW_SCHEMA);
        assert_eq!(
            value["attempts"][0]["dispatches"],
            json!(["primary", "escalation"])
        );
        assert_eq!(value["attempts"][0]["escalated"], true);
    }

    #[test]
    fn a_v4_attempt_shows_its_family_self_report_and_second_executor() {
        let mut value = record();
        value["route"]["family"] = json!({ "family": "cad-from-drawing", "why": "astra passed 10 of 10 on cad-from-drawing against the rule's 0.80", "picked": "astra" });
        value["checks"][0]["self_report"] = json!({
            "verdict": "failed",
            "requirement": "R1",
            "findings": [{ "source": "report", "signal": "underdetermined", "evidence": "The drawing doesn't pin this down exactly." }],
        });
        value["second"] = json!({
            "tier": { "agent": "codex", "model": "gpt-6-astra", "effort": "xhigh" },
            "trigger": "failed: 1 failed scenario(s) and 0 contradicted requirement(s) remain",
            "kept": "second",
            "why": "the second candidate has 0 failure(s)",
        });
        let text = detail_lines(&value).join("\n");
        assert!(text.contains("route family: cad-from-drawing"), "{text}");
        assert!(text.contains("self-report failed on R1"), "{text}");
        assert!(
            text.contains("underdetermined: The drawing doesn't pin this down exactly."),
            "{text}"
        );
        assert!(text.contains("second: codex/gpt-6-astra (xhigh)"), "{text}");
        assert!(text.contains("kept the second candidate"), "{text}");
        value["second"] = json!({ "skipped": "the checks confirmed the result" });
        let text = detail_lines(&value).join("\n");
        assert!(
            text.contains("second: skipped: the checks confirmed the result"),
            "{text}"
        );
    }

    #[test]
    fn a_v5_attempt_shows_each_persist_round() {
        let mut value = record();
        value["persist"] = json!({
            "stopped": "round 2 changed nothing",
            "rounds": [
                { "round": 1, "tier": { "agent": "claude-code", "model": "claude-opus-5-5", "effort": "xhigh" }, "status": "answered", "milliseconds": 412000, "cost_usd": 0.91, "changed": true, "files_changed": [{ "path": "out.step", "change": "modified" }], "before": { "failed": 1 }, "after": { "failed": 0 }, "kept": true },
                { "round": 2, "tier": { "agent": "claude-code", "model": "claude-opus-5-5", "effort": "xhigh" }, "status": "answered", "milliseconds": 95000, "cost_usd": 0.2, "changed": false, "files_changed": [], "kept": true },
            ],
        });
        let text = detail_lines(&value).join("\n");
        assert!(
            text.contains("persist: 2 rounds · stopped: round 2 changed nothing"),
            "{text}"
        );
        assert!(
            text.contains("1 file changed · failing checks 1 before, 0 after"),
            "{text}"
        );
        assert!(text.contains("no change"), "{text}");
        value["persist"] = json!({ "skipped": "not a long task", "rounds": [] });
        let text = detail_lines(&value).join("\n");
        assert!(text.contains("persist: skipped: not a long task"), "{text}");
    }

    #[test]
    fn a_v8_attempt_shows_each_rounds_delta_and_the_ladder() {
        let mut value = record();
        value["schema"] = json!("openagents.coder-one.composition.v2");
        value["persist"] = json!({
            "stopped": "round 3 on the strong executor made no progress",
            "totals": { "tests_fixed": 3, "tests_broken": 1, "cost_usd": 1.25, "escalations": 1 },
            "spend": { "spent_before_usd": 4.0, "cap_usd": 3.0 },
            "rounds": [
                { "round": 1, "class": "strong", "tier": { "agent": "claude-code", "model": "claude-opus-5-5", "effort": "xhigh" }, "status": "answered", "milliseconds": 412000, "cost_usd": 0.91, "changed": true, "files_changed": [{ "path": "src/main.rs", "change": "modified" }], "before": { "failed": 1 }, "after": { "failed": 1 }, "kept": true, "tests": { "passed": 19, "failed": 8 }, "delta": { "tests_fixed": 0, "tests_broken": 0, "tests_added": 27, "progress": null } },
                { "round": 2, "class": "cheap", "tier": { "agent": "codex", "model": "gpt-6-sol", "effort": "high" }, "status": "answered", "milliseconds": 200000, "cost_usd": 0.2, "changed": true, "files_changed": [{ "path": "src/main.rs", "change": "modified" }], "before": { "failed": 1 }, "after": { "failed": 1 }, "kept": true, "tests": { "passed": 19, "failed": 8 }, "delta": { "tests_fixed": 0, "tests_broken": 0, "progress": false }, "next": "escalate" },
                { "round": 3, "class": "escalated", "tier": { "agent": "claude-code", "model": "claude-opus-5-5", "effort": "xhigh" }, "status": "answered", "milliseconds": 300000, "cost_usd": 0.14, "changed": true, "files_changed": [], "before": { "failed": 1 }, "after": { "failed": 1 }, "kept": true, "tests": { "passed": 21, "failed": 6 }, "delta": { "tests_fixed": 3, "tests_broken": 1, "progress": true } },
            ],
        });
        assert!(readable(&value), "a v2 record is read");
        assert!(readable(
            &json!({ "schema": "openagents.coder-one.composition.v1" })
        ));
        assert!(readable(
            &json!({ "schema": "openagents.coder-one.composition.v2" })
        ));
        assert!(!readable(
            &json!({ "schema": "openagents.coder-one.composition.v4" })
        ));
        let text = detail_lines(&value).join("\n");
        assert!(
            text.contains("own tests 3 fixed, 1 broken · $1.2500 · 1 escalation · cap $3.00"),
            "{text}"
        );
        assert!(text.contains("codex/gpt-6-sol (high) [cheap]"), "{text}");
        assert!(
            text.contains("own tests 0 fixed, 0 broken (8 failing) · escalates"),
            "{text}"
        );
        assert!(text.contains("[escalated]"), "{text}");
    }

    #[test]
    fn a_v9_attempt_shows_the_effort_it_chose_and_why() {
        let mut value = record();
        value["effort"] = json!({
            "rule": "sensitivity-v1",
            "features": { "faithful_reproduction": 0.91, "long_reasoning": 0.4, "close_reading": null },
            "score": 0.672,
            "at": 0.4,
            "effort": "xhigh",
            "reason": "sensitivity 0.672 is at or above 0.400",
            "jev": "live",
        });
        let text = detail_lines(&value).join("\n");
        assert!(
            text.contains("effort: xhigh · sensitivity 0.672 is at or above 0.400 · faithful_reproduction 0.91, long_reasoning 0.40"),
            "{text}"
        );
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.composition = Some(value);
        let rows = rows(&Records {
            attempts: vec![attempt],
            ..Records::default()
        });
        let json = to_json(&rows);
        assert_eq!(json["attempts"][0]["effort"], "xhigh");
        assert_eq!(json["attempts"][0]["effort_score"], 0.672);
        // An attempt without control.effort shows no effort line.
        assert!(!detail_lines(&record()).join("\n").contains("effort:"));
    }

    #[test]
    fn no_composed_attempt_says_how_to_make_one() {
        let text = lines(&[], None).join("\n");
        assert!(text.contains("coder-one-tunable"));
    }

    /// A composed attempt on `task` whose `verify.second` record is
    /// `second`, graded `reward`.
    fn escalated(task: &str, reward: Option<f64>, second: Value) -> crate::terminal_bench::Attempt {
        let mut value = record();
        value["second"] = second;
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.task = task.to_owned();
        attempt.arm = "coder-one-tunable-v9-escalate".to_owned();
        attempt.reward = reward;
        attempt.composition = Some(value);
        attempt
    }

    #[test]
    fn escalations_report_their_triggers_outcomes_and_conditional_success() {
        let astra = json!({ "agent": "codex", "model": "gpt-6-astra", "effort": "xhigh" });
        let attempts = vec![
            // Rescued: a check fired, the second candidate stayed, it passed.
            escalated(
                "atrx-vep-crispr",
                Some(1.0),
                json!({ "tier": astra, "trigger": "check: 1 scenario(s) failed", "fired": ["check"], "outcome": "kept_second", "kept": "second", "cost_usd": 2.5, "milliseconds": 600_000 }),
            ),
            // The second candidate stayed and still failed.
            escalated(
                "ks-solver-cpp",
                Some(0.0),
                json!({ "tier": astra, "trigger": "self_report: the executor reported that the result fails", "fired": ["self_report"], "outcome": "kept_second", "kept": "second", "cost_usd": 1.5, "milliseconds": 300_000 }),
            ),
            // The first stayed and passed: the flag was wrong.
            escalated(
                "wal-recovery-ordering",
                Some(1.0),
                json!({ "tier": astra, "trigger": "check: 2 scenario(s) failed; self_report: the executor reported that the result fails", "fired": ["check", "self_report"], "outcome": "kept_first", "kept": "first", "cost_usd": null, "milliseconds": 100_000 }),
            ),
            // A v4 record: the trigger text only, and the cost on the branch.
            escalated(
                "mvcc-lsm-compaction",
                None,
                json!({ "tier": astra, "trigger": "unconfirmed: no scenario confirmed the result", "kept": "second" }),
            ),
            // Triggered, but too little time was left.
            escalated(
                "production-planning",
                Some(0.0),
                json!({ "skipped": "less than 1800 s left in the episode", "trigger": ["check: 1 scenario(s) failed"], "fired": ["check"] }),
            ),
            // Nothing fired.
            escalated(
                "cad-model",
                Some(1.0),
                json!({ "skipped": "no check failed and the executor reported no failure" }),
            ),
        ];
        let rows = rows(&Records {
            attempts,
            ..Records::default()
        });
        let summary = EscalationSummary::of(&rows);
        assert_eq!(summary.attempts, 6);
        assert_eq!(summary.escalations.len(), 4);
        assert_eq!(summary.rescued(), 1);
        assert_eq!(
            summary.by_trigger(),
            [
                ("check".to_owned(), 2),
                ("self_report".to_owned(), 2),
                ("unconfirmed".to_owned(), 1)
            ]
            .into_iter()
            .collect()
        );
        assert_eq!(summary.cost(), (4.0, 2));
        assert_eq!(summary.skipped_after_trigger.len(), 1);
        let text = summary.lines().join("\n");
        assert!(
            text.contains("6 composed attempts · 4 escalated · 3 graded"),
            "{text}"
        );
        assert!(
            text.contains("conditional success: 1 of 3 graded escalations (33%)"),
            "{text}"
        );
        assert!(
            text.contains("kept the second candidate: 2 (1 passed, 1 failed)"),
            "{text}"
        );
        assert!(
            text.contains("kept the first candidate: 1 (1 passed, 0 failed)"),
            "{text}"
        );
        assert!(text.contains("cost per rescue: $4.0000"), "{text}");
        assert!(
            text.contains("production-planning: less than 1800 s left"),
            "{text}"
        );
        let value = summary.to_json();
        assert_eq!(value["schema"], ESCALATIONS_SCHEMA);
        assert_eq!(value["rescued"], 1);
        assert_eq!(value["graded"], 3);
        assert_eq!(value["escalations"][0]["outcome"], "kept_second");
        assert_eq!(
            value["escalations"][2]["fired"],
            json!(["check", "self_report"])
        );
        // The row JSON carries the fired names too.
        assert_eq!(
            to_json(&rows)["attempts"][0]["second_fired"],
            json!(["check"])
        );
        // The detail line shows the escalation's cost and time.
        let detail = detail_lines(&rows[0].record).join("\n");
        assert!(detail.contains("$2.5000 · 600s"), "{detail}");
    }

    /// A composed attempt of `arm` graded `reward`, with `best_of`.
    fn picked(arm: &str, reward: Option<f64>, best_of: Value) -> Row {
        let mut value = record();
        value["best_of"] = best_of;
        let mut attempt = crate::terminal_bench::test_attempt();
        attempt.arm = arm.to_owned();
        attempt.reward = reward;
        attempt.cost_usd = Some(0.1);
        attempt.phases_ms[2] = Some(60_000);
        attempt.composition = Some(value);
        Row::of(&attempt).unwrap()
    }

    fn candidates(calls: &[&str]) -> Value {
        json!(
            calls
                .iter()
                .enumerate()
                .map(|(i, call)| json!({
                    "number": i + 1,
                    "status": "answered",
                    "milliseconds": 30_000,
                    "cost_usd": 0.02,
                    "checks": { "verdicts": { "passed": 2 } },
                    "verdict": { "call": call, "p_fail": 0.4 },
                }))
                .collect::<Vec<_>>()
        )
    }

    #[test]
    fn best_of_separates_what_selection_loses_from_what_generation_lacks() {
        let rows = vec![
            // Kept a passing candidate.
            picked(
                "luna-best-of-3",
                Some(1.0),
                json!({ "n": 3, "kept": 2, "candidates": candidates(&["fail", "pass", "unknown"]), "grades": { "rewards": [0.0, 1.0, 1.0] } }),
            ),
            // One candidate passed, but the selection kept another.
            picked(
                "luna-best-of-3",
                Some(0.0),
                json!({ "n": 3, "kept": 1, "candidates": candidates(&["unknown", "fail", "unknown"]), "grades": { "rewards": [0.0, 1.0, 0.0] } }),
            ),
            // None passed.
            picked(
                "luna-best-of-3",
                Some(0.0),
                json!({ "n": 3, "kept": 3, "candidates": candidates(&["unknown", "unknown", "unknown"]), "grades": { "rewards": [0.0, 0.0, 0.0] } }),
            ),
            // Not graded: the oracle is unknown.
            picked(
                "luna-best-of-3",
                Some(0.0),
                json!({ "n": 3, "kept": 1, "candidates": candidates(&["unknown", "unknown", "unknown"]) }),
            ),
            // A single candidate is its own oracle.
            picked("luna-best-of-1", Some(1.0), Value::Null),
        ];
        let arms = BestOfArm::of(&rows);
        assert_eq!(arms.len(), 2);
        let single = &arms[0];
        assert_eq!(single.arm, "luna-best-of-1");
        assert_eq!(
            (
                single.oracle_known,
                single.oracle_passed,
                single.selected_right
            ),
            (1, 1, 1)
        );
        let three = &arms[1];
        assert_eq!(three.attempts, 4);
        assert_eq!(three.candidates, 12);
        assert_eq!(three.passed, 1);
        assert_eq!(three.oracle_known, 3);
        assert_eq!(three.oracle_passed, 2);
        assert_eq!(three.selected_right, 1);
        assert_eq!(three.verdicts.get("pass"), Some(&1));
        let text = best_of_summary(&arms).join("\n");
        assert!(text.contains("2/3 (67%)"), "{text}");
        assert!(text.contains("1/2 (50%)"), "{text}");
        assert!(text.contains("tbench verify --candidate"), "{text}");
        // The detail shows each candidate, the kept one starred, and its
        // grade.
        let detail = detail_lines(&rows[1].record).join("\n");
        assert!(detail.contains("best of 3: kept candidate 1"), "{detail}");
        assert!(detail.contains("candidate 1* answered"), "{detail}");
        assert!(detail.contains("candidate 2  answered"), "{detail}");
        assert!(
            detail.contains("verdict fail (p_fail 0.40) · verifier 1.0"),
            "{detail}"
        );
        // An experiment's job names the arm, whatever profile ran it.
        let mut named = picked("coder-one-tunable-luna-pack", Some(0.0), Value::Null);
        named.job = "tb4--luna-bo1--mvcc-lsm-compaction--best-of-9587-r1".to_owned();
        assert_eq!(BestOfArm::of(&[named])[0].arm, "luna-bo1");
        let skipped = json!({ "n": 5, "skipped": "the workspace is over 256 MiB" });
        assert_eq!(
            best_of_lines(&skipped),
            ["  best of 5: one candidate ran: the workspace is over 256 MiB"]
        );
    }

    #[test]
    fn a_parallel_loop_lists_which_sessions_overlapped() {
        let parallel = json!({
            "sessions": 3, "wall_ms": 10_000, "session_ms": 16_000, "concurrency": 1.6,
            "peak": 2, "critical_path_ms": 9_000, "saved_ms": 6_000, "suite_ms": 5_000,
            "suite_on_critical_path_ms": 1_000, "merges": 1, "conflicts": 0,
            "tracks": [
                { "label": "accept.define", "kind": "define", "batch": "suite", "start_ms": 0, "end_ms": 5_000 },
                { "label": "accept-writer-1", "kind": "writer", "batch": "writer round 1", "start_ms": 0, "end_ms": 4_000 },
                { "label": "session 1", "kind": "edit", "batch": "suite", "start_ms": 0, "end_ms": 4_000 },
                { "label": "session 2", "kind": "edit", "batch": "round 1", "group": "group 1 of 2: T1", "start_ms": 5_000, "end_ms": 9_000 },
                { "label": "session 3", "kind": "edit", "batch": "round 1", "group": "group 2 of 2: T2", "start_ms": 5_000, "end_ms": 8_000 },
            ],
        });
        let lines = parallel_lines(&parallel);
        assert!(lines[0].contains("concurrency 1.60×"), "{lines:#?}");
        let line = |label: &str| {
            lines
                .iter()
                .find(|l| l.trim_start().starts_with(label))
                .unwrap()
                .clone()
        };
        assert!(
            line("session 2").ends_with("· alongside session 3"),
            "{lines:#?}"
        );
        assert!(
            line("accept.define").ends_with("alongside session 1"),
            "{lines:#?}"
        );
        assert!(
            line("accept-writer-1").ends_with("alongside session 1"),
            "{lines:#?}"
        );
        assert!(parallel_lines(&Value::Null).is_empty());
    }

    #[test]
    fn no_escalation_says_so() {
        let text = EscalationSummary::of(&[]).lines().join("\n");
        assert!(text.contains("No attempt ran a second executor."), "{text}");
        assert!(text.contains("0 of 0 graded escalations (—)"), "{text}");
        assert!(
            text.contains("escalation cost: $0.0000 over 0 priced"),
            "{text}"
        );
    }
}
