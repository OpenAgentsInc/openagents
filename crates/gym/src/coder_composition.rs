//! Coder One's tunable composition, per Terminal-Bench attempt.
//!
//! A composed episode (`control.route`, `control.handoff`,
//! `control.horizon`, and `verify` in its policy manifest) writes
//! `artifacts/composition.json` (`openagents.coder-one.composition.v2`, or
//! v1 before persistence rounds recorded their deltas):
//! where the route started and why, the deadline each dispatch asked for,
//! every dispatch with its tier, status, time, and cost, each handoff and
//! its trigger, the checks after each dispatch with what
//! `generic.self-report` found, `verify.support`'s states, the repair, and
//! `verify.second`'s second executor, and each `control.persist` round with
//! its executor, own tests fixed and broken, and cost. This module reads it
//! from each attempt and renders it.

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
pub const SCHEMA: &str = "openagents.coder-one.composition.v2";

/// The schemas this module reads: v1 has no persistence deltas.
pub const SCHEMAS: [&str; 2] = ["openagents.coder-one.composition.v1", SCHEMA];

/// Whether a record carries a schema this module reads.
fn readable(record: &Value) -> bool {
    SCHEMAS.iter().any(|schema| record["schema"] == *schema)
}

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
            "second_kept": self.record["second"]["kept"],
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
            "  second: {} · {} · kept the {} candidate · {}",
            tier(&second["tier"]),
            words(&second["trigger"]),
            words(&second["kept"]),
            words(&second["why"])
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
                " · own tests +{} −{} · {} · {} escalation{}{}",
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
                " · failed {} → {}",
                round["before"]["failed"].as_u64().unwrap_or(0),
                round["after"]["failed"].as_u64().unwrap_or(0)
            )
        } else {
            String::new()
        };
        let delta = &round["delta"];
        let tests = if delta.is_object() && round["tests"].is_object() {
            format!(
                " · own tests +{} −{} ({} failing)",
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
        assert!(text.contains("1 file changed · failed 1 → 0"), "{text}");
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
        assert!(!readable(
            &json!({ "schema": "openagents.coder-one.composition.v3" })
        ));
        let text = detail_lines(&value).join("\n");
        assert!(
            text.contains("own tests +3 −1 · $1.2500 · 1 escalation · cap $3.00"),
            "{text}"
        );
        assert!(text.contains("codex/gpt-6-sol (high) [cheap]"), "{text}");
        assert!(
            text.contains("own tests +0 −0 (8 failing) · escalates"),
            "{text}"
        );
        assert!(text.contains("[escalated]"), "{text}");
    }

    #[test]
    fn no_composed_attempt_says_how_to_make_one() {
        let text = lines(&[], None).join("\n");
        assert!(text.contains("coder-one-tunable"));
    }
}
