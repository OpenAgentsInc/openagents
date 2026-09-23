//! Coder One's tunable composition, per Terminal-Bench attempt.
//!
//! A composed episode (`control.route`, `control.handoff`,
//! `control.horizon`, and `verify` in its policy manifest) writes
//! `artifacts/composition.json` (`openagents.coder-one.composition.v1`):
//! where the route started and why, the deadline each dispatch asked for,
//! every dispatch with its tier, status, time, and cost, each handoff and
//! its trigger, the checks after each dispatch, `verify.support`'s states,
//! and the repair. This module reads it from each attempt and renders it.

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
pub const SCHEMA: &str = "openagents.coder-one.composition.v1";

/// The schema of this module's JSON.
pub const VIEW_SCHEMA: &str = "openagents.gym.coder-composition.v1";

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
        (record["schema"] == SCHEMA).then(|| Row {
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
            short(&self.roles().join("→"), 34),
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
            "dispatches": self.roles(),
            "escalated": self.record["escalated"],
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
    let mut lines =
        vec!["Composition (control.route → exec → verify → handoff → repair)".to_owned()];
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
    }
    for handoff in record["handoffs"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  handoff {}: {} → {} · {}",
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

const HELP: &str = "gym coder composition [QUERY] [--json]

Each Terminal-Bench attempt that ran Coder One's tunable composition: where
control.route started and why, the deadline each dispatch asked for, every
dispatch (planner, primary, escalation) with its tier, status, time, and
cost, each handoff and its trigger, verify.checks after each dispatch,
verify.support's states, and the repair. QUERY picks the attempts whose task,
job, or trial contains it for detail; the newest is shown otherwise.

  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --no-jobs | --no-traces  omit one source
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
    let rows = rows(&records);
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
        assert!(text.contains("primary→escalation"), "{text}");
        assert!(
            text.contains(
                "handoff escalate: codex/gpt-6-luna → claude-code/claude-opus-5-5 · a check failed"
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
    fn no_composed_attempt_says_how_to_make_one() {
        let text = lines(&[], None).join("\n");
        assert!(text.contains("coder-one-tunable"));
    }
}
