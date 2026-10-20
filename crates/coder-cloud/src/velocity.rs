//! Daily velocity numbers (#12212): what the 2026-10-19 velocity audit
//! (`docs/audits/2026-10-19-issue-velocity.md`) computed by hand.
//!
//! [`collect`] reads the window's inputs: commits on `origin/main` (`git
//! log`), issues filed and closed (`gh issue list`), the landing queue's
//! entries, the cloud run records (`~/.openagents/cloud/runs.jsonl`), and the
//! coordinator's decision log (the app's `agent-wakes/<agent>/decisions.jsonl`).
//! [`measure`] turns an [`Input`] into a [`Report`] with no I/O, so tests
//! replay recorded inputs. An input that couldn't be read is reported as
//! unknown, never as zero.
//!
//! `openagents velocity` prints the report, and Foreman's first check-in
//! after 13:00 UTC each day carries [`Report::text`].

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The report's schema name.
pub const SCHEMA: &str = "openagents.velocity.v1";
/// The hour (UTC) after which Foreman's first check-in carries the report.
pub const DAILY_HOUR_UTC: u64 = 13;

/// One commit on `main`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Commit {
    pub sha: String,
    pub author: String,
    /// Unix seconds (committer time).
    pub at: u64,
    /// Names from `Co-Authored-By` trailers, without their addresses.
    #[serde(default)]
    pub co_authors: Vec<String>,
}

/// One issue, as `gh issue list --json number,createdAt,closedAt` prints it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub number: u64,
    pub created_at: String,
    #[serde(default)]
    pub closed_at: Option<String>,
}

/// Everything a report is computed from. `None` is an input that couldn't
/// be read.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Input {
    /// The window's end, Unix seconds.
    pub now: u64,
    pub hours: u64,
    #[serde(default)]
    pub commits: Option<Vec<Commit>>,
    #[serde(default)]
    pub issues: Option<Vec<Issue>>,
    /// Landing queue entries as their JSON objects.
    #[serde(default)]
    pub land: Option<Vec<Value>>,
    /// Cloud run records (`openagents.cloud.run.v1`), one per line of
    /// `runs.jsonl`.
    #[serde(default)]
    pub runs: Option<Vec<Value>>,
    /// The coordinator's decision log, oldest first.
    #[serde(default)]
    pub decisions: Option<Vec<Value>>,
    /// Why each unknown input couldn't be read.
    #[serde(default)]
    pub errors: Vec<String>,
}

/// The measured numbers.
#[derive(Clone, Debug, Default, Serialize)]
pub struct Report {
    pub schema: &'static str,
    pub since: u64,
    pub now: u64,
    pub hours: u64,
    pub commits: Value,
    pub issues: Value,
    pub land: Value,
    pub runs: Value,
    pub coordinator: Value,
    pub errors: Vec<String>,
}

/// Seconds since the Unix epoch of an RFC 3339 UTC time such as
/// `2026-10-19T21:07:00Z` (fractions and an offset of `+00:00` allowed).
#[must_use]
pub fn utc_seconds(text: &str) -> Option<u64> {
    let text = text.trim();
    let (date, time) = text.split_once(['T', ' '])?;
    let mut d = date.split('-').map(|p| p.parse::<i64>().ok());
    let (y, m, day) = (d.next()??, d.next()??, d.next()??);
    let time = time
        .trim_end_matches('Z')
        .trim_end_matches("+00:00")
        .split('.')
        .next()?;
    let mut t = time.split(':').map(|p| p.parse::<i64>().ok());
    let (h, min, s) = (t.next()??, t.next()??, t.next().flatten().unwrap_or(0));
    if !(1..=12).contains(&m) || !(1..=31).contains(&day) || h > 23 || min > 59 || s > 60 {
        return None;
    }
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if m <= 2 { y - 1 } else { y };
    let era = y.div_euclid(400);
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    u64::try_from(days * 86_400 + h * 3600 + min * 60 + s).ok()
}

/// `YYYY-MM-DD HH:MM UTC` for Unix seconds.
#[must_use]
pub fn utc_text(at: u64) -> String {
    let days = i64::try_from(at / 86_400).unwrap_or(0);
    let secs = at % 86_400;
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02} UTC",
        secs / 3600,
        secs % 3600 / 60
    )
}

/// The value at quantile `q` (0..=1) of sorted `values`, nearest rank.
fn quantile(sorted: &[f64], q: f64) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let rank = (q * sorted.len() as f64).ceil().max(1.0) as usize;
    Some(sorted[rank.min(sorted.len()) - 1])
}

fn median(sorted: &[f64]) -> Option<f64> {
    if sorted.is_empty() {
        return None;
    }
    let n = sorted.len();
    Some(if n % 2 == 1 {
        sorted[n / 2]
    } else {
        f64::midpoint(sorted[n / 2 - 1], sorted[n / 2])
    })
}

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

fn counted(map: BTreeMap<String, u64>) -> Value {
    let mut rows: Vec<(String, u64)> = map.into_iter().collect();
    rows.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
    Value::Array(
        rows.into_iter()
            .map(|(name, count)| json!({"name": name, "count": count}))
            .collect(),
    )
}

/// Why a landing queue entry bounced, as a cause.
#[must_use]
pub fn bounce_cause(reason: &str) -> &'static str {
    let r = reason.to_lowercase();
    if r.contains("conflict") {
        "conflict with main"
    } else if r.contains("bundled")
        || r.contains("generated")
        || r.contains("regenerate")
        || r.contains("stale")
    {
        "stale generated file"
    } else if r.contains("unresolved import")
        || r.contains("cannot find crate")
        || r.contains("use of undeclared crate")
        || r.contains("dependency")
    {
        "missing dependency"
    } else if r.contains("fmt") || r.contains("format") {
        "formatting"
    } else if r.contains("error[e") || r.contains("could not compile") {
        "build failed"
    } else if r.contains("test") || r.contains("failed") {
        "a test failed"
    } else if r.contains("push") {
        "push failed"
    } else {
        "other"
    }
}

/// Why a failed cloud run failed, from its record.
#[must_use]
pub fn run_cause(record: &Value) -> &'static str {
    let outcome = record["outcome"].as_str().unwrap_or_default();
    let message = record["message"]
        .as_str()
        .unwrap_or_default()
        .to_lowercase();
    if outcome == "preempted" || message.contains("preempted") || message.contains("is gone") {
        return "host lost";
    }
    if outcome == "not_started" {
        return "didn't start";
    }
    if message.contains("limit")
        && ["usage", "weekly", "rate", "hit your"]
            .iter()
            .any(|w| message.contains(w))
    {
        return "usage limit";
    }
    if message.contains("per-run cap") {
        return "cost cap";
    }
    if message.contains("initialization failed") || message.contains("didn't start") {
        return "didn't start";
    }
    if message.contains("without an outcome") || message.contains("without a result") {
        return "ended without a result";
    }
    if let Some(rest) = message.split("checks ").nth(1) {
        let mut parts = rest
            .split(|c: char| !c.is_ascii_digit())
            .filter(|p| !p.is_empty());
        if let (Some(passed), Some(total)) = (
            parts.next().and_then(|p| p.parse::<u64>().ok()),
            parts.next().and_then(|p| p.parse::<u64>().ok()),
        ) {
            if total == 0 {
                return "no change";
            }
            if passed < total {
                return "a check failed";
            }
        }
    }
    if message.contains("no change") || message.contains("made no change") {
        return "no change";
    }
    if message.contains("check") || message.contains("test") {
        return "a check failed";
    }
    if message.is_empty() {
        return "not recorded";
    }
    "other"
}

/// A route record's end time, Unix seconds.
fn run_at(record: &Value) -> Option<u64> {
    record["ended_at"]
        .as_u64()
        .or_else(|| record["lost_at"].as_u64())
}

fn commits_of(commits: &[Commit], since: u64, now: u64) -> Value {
    let mut by = BTreeMap::new();
    let mut total = 0;
    for commit in commits.iter().filter(|c| c.at >= since && c.at <= now) {
        total += 1;
        let name = match commit.co_authors.first() {
            Some(co) => format!("{} with {co}", commit.author),
            None => commit.author.clone(),
        };
        *by.entry(name).or_insert(0) += 1;
    }
    json!({"total": total, "by_author": counted(by)})
}

fn issues_of(issues: &[Issue], since: u64, now: u64) -> Value {
    let within = |at: Option<u64>| at.is_some_and(|at| at >= since && at <= now);
    let filed = issues
        .iter()
        .filter(|i| within(utc_seconds(&i.created_at)))
        .count();
    let mut hours: Vec<f64> = Vec::new();
    let mut closed = 0;
    for issue in issues {
        let closed_at = issue.closed_at.as_deref().and_then(utc_seconds);
        if !within(closed_at) {
            continue;
        }
        closed += 1;
        if let (Some(end), Some(start)) = (closed_at, utc_seconds(&issue.created_at)) {
            hours.push(end.saturating_sub(start) as f64 / 3600.0);
        }
    }
    hours.sort_by(f64::total_cmp);
    json!({"filed": filed, "closed": closed,
        "median_hours_to_close": median(&hours).map(round1),
        "p90_hours_to_close": quantile(&hours, 0.9).map(round1)})
}

fn land_of(entries: &[Value], since: u64, now: u64) -> Value {
    let within = |e: &Value| {
        let at = e["updated_at"].as_u64().or(e["submitted_at"].as_u64());
        at.is_some_and(|at| at >= since && at <= now)
            || e["submitted_at"]
                .as_u64()
                .is_some_and(|at| at >= since && at <= now)
    };
    let mut states = BTreeMap::new();
    let mut landed: Vec<f64> = Vec::new();
    let mut causes: BTreeMap<&'static str, (u64, String)> = BTreeMap::new();
    for entry in entries.iter().filter(|e| within(e)) {
        let state = entry["state"].as_str().unwrap_or("unknown").to_owned();
        if state == "landed"
            && let (Some(end), Some(start)) =
                (entry["updated_at"].as_u64(), entry["submitted_at"].as_u64())
        {
            landed.push(end.saturating_sub(start) as f64 / 60.0);
        }
        if state == "bounced" {
            let reason = entry["reason"].as_str().unwrap_or_default();
            let row = causes
                .entry(bounce_cause(reason))
                .or_insert((0, String::new()));
            row.0 += 1;
            if row.1.is_empty() {
                row.1 = reason.chars().take(160).collect();
            }
        }
        *states.entry(state).or_insert(0) += 1;
    }
    landed.sort_by(f64::total_cmp);
    let total: u64 = states.values().sum();
    let mut bounces: Vec<Value> = causes
        .into_iter()
        .map(
            |(cause, (count, example))| json!({"cause": cause, "count": count, "example": example}),
        )
        .collect();
    bounces.sort_by(|a, b| b["count"].as_u64().cmp(&a["count"].as_u64()));
    json!({"entries": total, "by_state": counted(states),
        "landed_median_minutes": median(&landed).map(round1), "bounces": bounces})
}

fn runs_of(records: &[Value], since: u64, now: u64) -> Value {
    let mut total = 0;
    let mut failed = 0;
    let mut undated = 0;
    let mut causes = BTreeMap::new();
    for record in records {
        let Some(at) = run_at(record) else {
            undated += 1;
            continue;
        };
        if at < since || at > now {
            continue;
        }
        total += 1;
        let outcome = record["outcome"].as_str().unwrap_or_default();
        if matches!(outcome, "failed" | "preempted" | "not_started") {
            failed += 1;
            *causes.entry(run_cause(record).to_owned()).or_insert(0) += 1;
        }
    }
    json!({"runs": total, "failed": failed, "by_cause": counted(causes),
        "undated": undated})
}

/// The coordinator's check-ins: how many ended errored or with no reply
/// (no decision recorded between the wake and its end), and the minutes
/// from the first such check-in of a run of them to the next good one.
fn coordinator_of(rows: &[Value], since: u64, now: u64) -> Value {
    struct Turn {
        at: u64,
        bad: bool,
        ended: bool,
    }
    let ms = |v: &Value| v["at"].as_u64().map(|at| at / 1000);
    let mut turns: Vec<Turn> = Vec::new();
    let mut open: BTreeMap<String, usize> = BTreeMap::new();
    let mut last: Option<usize> = None;
    for row in rows {
        match row["kind"].as_str() {
            Some("wake") => {
                let Some(at) = ms(row) else { continue };
                turns.push(Turn {
                    at,
                    bad: true,
                    ended: false,
                });
                let i = turns.len() - 1;
                if let Some(id) = row["id"].as_str() {
                    open.insert(id.to_owned(), i);
                }
                last = Some(i);
            }
            Some("decision") => {
                if let Some(i) = last
                    && !turns[i].ended
                {
                    turns[i].bad = false;
                }
            }
            Some("outcome") => {
                let Some(&i) = row["id"].as_str().and_then(|id| open.get(id)) else {
                    continue;
                };
                let status = row["status"].as_str().unwrap_or_default();
                if matches!(status, "errored" | "empty" | "no reply") {
                    turns[i].bad = true;
                    turns[i].ended = true;
                } else if matches!(status, "finished" | "stuck" | "not started") {
                    turns[i].ended = true;
                }
            }
            _ => {}
        }
    }
    // A check-in still going isn't counted either way.
    let ended: Vec<&Turn> = turns.iter().filter(|t| t.ended).collect();
    let in_window = ended
        .iter()
        .filter(|t| t.at >= since && t.at <= now)
        .count();
    let bad = ended
        .iter()
        .filter(|t| t.bad && t.at >= since && t.at <= now)
        .count();
    let mut down = 0u64;
    let mut start: Option<u64> = None;
    for turn in &ended {
        match (turn.bad, start) {
            (true, None) => start = Some(turn.at),
            (false, Some(s)) => {
                down += turn.at.min(now).saturating_sub(s.max(since));
                start = None;
            }
            _ => {}
        }
    }
    if let Some(s) = start {
        down += now.saturating_sub(s.max(since));
    }
    json!({"check_ins": in_window, "empty_or_errored": bad, "down_minutes": down / 60})
}

/// The report over `input`'s window.
#[must_use]
pub fn measure(input: &Input) -> Report {
    let now = input.now;
    let since = now.saturating_sub(input.hours * 3600);
    let unknown = || json!(null);
    Report {
        schema: SCHEMA,
        since,
        now,
        hours: input.hours,
        commits: input
            .commits
            .as_deref()
            .map_or_else(unknown, |c| commits_of(c, since, now)),
        issues: input
            .issues
            .as_deref()
            .map_or_else(unknown, |i| issues_of(i, since, now)),
        land: input
            .land
            .as_deref()
            .map_or_else(unknown, |l| land_of(l, since, now)),
        runs: input
            .runs
            .as_deref()
            .map_or_else(unknown, |r| runs_of(r, since, now)),
        coordinator: input
            .decisions
            .as_deref()
            .map_or_else(unknown, |d| coordinator_of(d, since, now)),
        errors: input.errors.clone(),
    }
}

fn names(rows: &Value, key: &str, limit: usize) -> String {
    let rows = rows.as_array().cloned().unwrap_or_default();
    let mut shown: Vec<String> = rows
        .iter()
        .take(limit)
        .map(|r| {
            format!(
                "{} {}",
                r[key].as_str().unwrap_or_default(),
                r["count"].as_u64().unwrap_or(0)
            )
        })
        .collect();
    if rows.len() > limit {
        let rest: u64 = rows[limit..]
            .iter()
            .filter_map(|r| r["count"].as_u64())
            .sum();
        shown.push(format!("others {rest}"));
    }
    shown.join(", ")
}

fn hours(v: &Value) -> String {
    v.as_f64()
        .map_or_else(|| "n/a".to_owned(), |h| format!("{h:.1} h"))
}

impl Report {
    /// The one-screen summary.
    #[must_use]
    pub fn text(&self) -> String {
        let mut out = vec![format!(
            "Velocity, the {} hours to {}",
            self.hours,
            utc_text(self.now)
        )];
        let unknown = "not known (couldn't be read)";
        out.push(if self.commits.is_null() {
            format!("Commits on main: {unknown}")
        } else {
            format!(
                "Commits on main: {} ({})",
                self.commits["total"],
                names(&self.commits["by_author"], "name", 5)
            )
        });
        out.push(if self.issues.is_null() {
            format!("Issues: {unknown}")
        } else {
            format!(
                "Issues: {} filed, {} closed; filing to close: median {}, p90 {}",
                self.issues["filed"],
                self.issues["closed"],
                hours(&self.issues["median_hours_to_close"]),
                hours(&self.issues["p90_hours_to_close"])
            )
        });
        if self.land.is_null() {
            out.push(format!("Landing queue: {unknown}"));
        } else {
            let median = self.land["landed_median_minutes"]
                .as_f64()
                .map_or_else(|| "n/a".to_owned(), |m| format!("{m:.0} min"));
            out.push(format!(
                "Landing queue: {} entries ({}); landed median {median}",
                self.land["entries"],
                names(&self.land["by_state"], "name", 5)
            ));
            for bounce in self.land["bounces"].as_array().into_iter().flatten() {
                out.push(format!(
                    "  bounced, {}: {} (e.g. {})",
                    bounce["cause"].as_str().unwrap_or_default(),
                    bounce["count"],
                    bounce["example"].as_str().unwrap_or_default()
                ));
            }
        }
        out.push(if self.runs.is_null() {
            format!("Cloud runs: {unknown}")
        } else if self.runs["failed"] == 0 {
            format!("Cloud runs: {}, none failed", self.runs["runs"])
        } else {
            format!(
                "Cloud runs: {} failed of {} ({})",
                self.runs["failed"],
                self.runs["runs"],
                names(&self.runs["by_cause"], "name", 6)
            )
        });
        out.push(if self.coordinator.is_null() {
            format!("Coordinator: {unknown}")
        } else {
            format!(
                "Coordinator: down {} min ({} of {} check-ins empty or errored)",
                self.coordinator["down_minutes"],
                self.coordinator["empty_or_errored"],
                self.coordinator["check_ins"]
            )
        });
        for error in &self.errors {
            out.push(format!("Unknown: {error}"));
        }
        out.join("\n")
    }
}

/// Whether a check-in at `now` (Unix seconds) is the first one after the
/// day's [`DAILY_HOUR_UTC`], given the day the report was last sent
/// (`days since the epoch`). Returns the day to remember when it is.
#[must_use]
pub fn daily_due(now: u64, last_day: Option<u64>) -> Option<u64> {
    let day = now / 86_400;
    let after = now % 86_400 >= DAILY_HOUR_UTC * 3600;
    (after && last_day != Some(day)).then_some(day)
}

// ---------------------------------------------------------------------------
// Reading the inputs.

/// Where [`collect`] reads.
#[derive(Clone, Debug, Default)]
pub struct Sources {
    /// A checkout of the repository (for `git log origin/main`).
    pub checkout: Option<PathBuf>,
    /// `OWNER/NAME` for `gh`.
    pub repo: String,
    /// The landing queue (`gs://...` or a folder).
    pub queue: String,
    /// `runs.jsonl`.
    pub runs: PathBuf,
    /// The coordinator's decision log, if known.
    pub decisions: Option<PathBuf>,
}

impl Sources {
    /// This computer's defaults for `repo`, with `checkout` and the
    /// decision log as given.
    #[must_use]
    pub fn here(repo: &str, checkout: Option<PathBuf>, decisions: Option<PathBuf>) -> Self {
        let home = user_home::home_os().map_or_else(|| PathBuf::from("."), PathBuf::from);
        let cloud = std::env::var_os("OPENAGENTS_CLOUD_HOME")
            .map_or_else(|| home.join(".openagents/cloud"), PathBuf::from);
        Self {
            checkout,
            repo: repo.to_owned(),
            queue: std::env::var("OPENAGENTS_LAND_QUEUE")
                .unwrap_or_else(|_| crate::machine_status::DEFAULT_QUEUE.into()),
            runs: cloud.join("runs.jsonl"),
            decisions: decisions.or_else(|| newest_decision_log(&home)),
        }
    }
}

/// The most recently written app decision log under `home`
/// (`.../agent-wakes/<agent>/decisions.jsonl` in each app data folder).
#[must_use]
pub fn newest_decision_log(home: &Path) -> Option<PathBuf> {
    let roots = [
        home.join("Library/Application Support"),
        home.join(".local/share"),
        home.join(".config"),
    ];
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;
    for root in roots {
        let Ok(apps) = std::fs::read_dir(&root) else {
            continue;
        };
        for app in apps.flatten() {
            let name = app.file_name().to_string_lossy().to_lowercase();
            if !name.starts_with("openagents") {
                continue;
            }
            let profiles = app.path().join("data/profiles");
            for profile in std::fs::read_dir(&profiles).into_iter().flatten().flatten() {
                let wakes = profile.path().join("agent-wakes");
                for agent in std::fs::read_dir(&wakes).into_iter().flatten().flatten() {
                    let log = agent.path().join("decisions.jsonl");
                    if let Ok(modified) = std::fs::metadata(&log).and_then(|m| m.modified())
                        && best.as_ref().is_none_or(|(at, _)| modified > *at)
                    {
                        best = Some((modified, log));
                    }
                }
            }
        }
    }
    best.map(|(_, log)| log)
}

async fn output(program: &str, args: &[String], cwd: Option<&Path>) -> Result<String, String> {
    let mut command = if program == "gcloud" {
        // The pool's pinned gcloud configuration, as every pool read uses.
        tokio::process::Command::from(crate::pool::gcloud_command())
    } else {
        tokio::process::Command::new(program)
    };
    command
        .args(args)
        .stdin(std::process::Stdio::null())
        .kill_on_drop(true);
    if let Some(cwd) = cwd {
        command.current_dir(cwd);
    }
    let done = tokio::time::timeout(Duration::from_secs(60), command.output())
        .await
        .map_err(|_| format!("{program} took over a minute"))?
        .map_err(|_| format!("{program} couldn't run"))?;
    if !done.status.success() {
        let err = String::from_utf8_lossy(&done.stderr);
        return Err(format!(
            "{program} failed: {}",
            err.trim().lines().last().unwrap_or_default()
        ));
    }
    Ok(String::from_utf8_lossy(&done.stdout).into_owned())
}

/// `git log` lines (`SHA\x1fAUTHOR\x1fTIME\x1fCO-AUTHORS`) as commits.
#[must_use]
pub fn parse_log(text: &str) -> Vec<Commit> {
    text.split('\x1e')
        .filter_map(|record| {
            let mut f = record.trim_matches(['\n', '\r']).split('\x1f');
            let sha = f.next()?.trim().to_owned();
            let author = f.next()?.trim().to_owned();
            let at = f.next()?.trim().parse().ok()?;
            let co_authors = f
                .next()
                .unwrap_or_default()
                .split(['\n', ';'])
                .map(|t| t.split('<').next().unwrap_or_default().trim().to_owned())
                .filter(|t| !t.is_empty())
                .collect();
            (!sha.is_empty()).then_some(Commit {
                sha,
                author,
                at,
                co_authors,
            })
        })
        .collect()
}

fn lines_of(text: &str) -> Vec<Value> {
    text.lines()
        .filter(|l| !l.trim().is_empty())
        .filter_map(|l| serde_json::from_str(l).ok())
        .collect()
}

/// Reads every input for the `hours` up to `now`. An input that can't be
/// read is left unknown, with why in `errors`.
pub async fn collect(sources: &Sources, hours: u64, now: u64) -> Input {
    let since = now.saturating_sub(hours * 3600);
    let mut input = Input {
        now,
        hours,
        ..Input::default()
    };
    let log_args: Vec<String> = [
        "log",
        "origin/main",
        &format!("--since=@{since}"),
        "--format=%H%x1f%an%x1f%ct%x1f%(trailers:key=Co-Authored-By,valueonly,separator=;)%x1e",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let created = utc_text(since);
    let date = created.split(' ').next().unwrap_or_default().to_owned();
    let gh_args: Vec<String> = [
        "issue",
        "list",
        "-R",
        &sources.repo,
        "--state",
        "all",
        "--limit",
        "1000",
        "--search",
        &format!("updated:>={date}"),
        "--json",
        "number,createdAt,closedAt",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    let git = async {
        match &sources.checkout {
            Some(dir) => output("git", &log_args, Some(dir))
                .await
                .map(|t| parse_log(&t)),
            None => Err("no checkout to read commits from".to_owned()),
        }
    };
    let gh = async {
        let text = output("gh", &gh_args, None).await?;
        serde_json::from_str::<Vec<Issue>>(&text).map_err(|e| format!("gh's issue list: {e}"))
    };
    let land = async {
        let queue = sources.queue.trim_end_matches('/');
        if queue.starts_with("gs://") {
            // Every entry in one read; the status reader's ten seconds are
            // too short for a day of entries.
            let args = vec![
                "storage".to_owned(),
                "cat".to_owned(),
                format!("{queue}/entries/*.json"),
                "--quiet".to_owned(),
            ];
            let text = output("gcloud", &args, None).await?;
            serde_json::Deserializer::from_str(&text)
                .into_iter::<Value>()
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("an entry doesn't read: {e}"))
        } else {
            crate::machine_status::landing_entries(queue).await
        }
    };
    let (commits, issues, land) = tokio::join!(git, gh, land);
    match commits {
        Ok(c) => input.commits = Some(c),
        Err(e) => input.errors.push(format!("commits: {e}")),
    }
    match issues {
        Ok(i) => input.issues = Some(i),
        Err(e) => input.errors.push(format!("issues: {e}")),
    }
    match land {
        Ok(l) => input.land = Some(l),
        Err(e) => input.errors.push(format!("landing queue: {e}")),
    }
    match std::fs::read_to_string(&sources.runs) {
        Ok(text) => input.runs = Some(lines_of(&text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => input.runs = Some(Vec::new()),
        Err(e) => input
            .errors
            .push(format!("cloud runs ({}): {e}", sources.runs.display())),
    }
    match &sources.decisions {
        Some(path) => match std::fs::read_to_string(path) {
            Ok(text) => input.decisions = Some(lines_of(&text)),
            Err(e) => input
                .errors
                .push(format!("coordinator ({}): {e}", path.display())),
        },
        None => input
            .errors
            .push("coordinator: no decision log on this computer (--decision-log FILE)".into()),
    }
    input
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_792_500_000; // 2026-10-20 12:40 UTC

    fn at(text: &str) -> u64 {
        utc_seconds(text).unwrap()
    }

    #[test]
    fn utc_times_read_and_print() {
        assert_eq!(utc_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(utc_seconds("2026-10-19T21:07:00Z"), Some(1_792_444_020));
        assert_eq!(
            utc_seconds("2026-10-19T21:07:00.123+00:00"),
            Some(1_792_444_020)
        );
        assert_eq!(utc_seconds("garbage"), None);
        assert_eq!(utc_text(1_792_444_020), "2026-10-19 21:07 UTC");
        assert_eq!(utc_text(0), "1970-01-01 00:00 UTC");
    }

    #[test]
    fn git_log_records_parse_with_their_co_authors() {
        let text = "a1\x1fAtlantisPleb\x1f100\x1fClaude Opus 5.5 <noreply@anthropic.com>\x1e\nb2\x1fCodex\x1f200\x1f\x1e\n";
        let commits = parse_log(text);
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[0].co_authors, vec!["Claude Opus 5.5".to_owned()]);
        assert_eq!((commits[1].author.as_str(), commits[1].at), ("Codex", 200));
    }

    #[test]
    fn causes_name_what_went_wrong() {
        assert_eq!(
            bounce_cause("tree::tests::the_bundled_tree_is_generated_from_this_help failed"),
            "stale generated file"
        );
        assert_eq!(
            bounce_cause("error[E0433]: unresolved import `futures_util`"),
            "missing dependency"
        );
        assert_eq!(
            bounce_cause("the change conflicts with main"),
            "conflict with main"
        );
        let run = |outcome: &str, message: &str| json!({"outcome": outcome, "message": message});
        assert_eq!(run_cause(&run("preempted", "")), "host lost");
        assert_eq!(
            run_cause(&run(
                "failed",
                "briefed agent on Codex, 847 s, cost unknown, checks 3/4"
            )),
            "a check failed"
        );
        assert_eq!(
            run_cause(&run("failed", "Claude Code, 2 s, cost unknown, checks 0/0")),
            "no change"
        );
        assert_eq!(
            run_cause(&run("failed", "You've hit your weekly limit")),
            "usage limit"
        );
        assert_eq!(
            run_cause(&run(
                "failed",
                "The flow on h ended without an outcome (exit 2)."
            )),
            "ended without a result"
        );
        assert_eq!(run_cause(&run("failed", "")), "not recorded");
    }

    fn recorded() -> Input {
        let fixture = include_str!("../tests/fixtures/velocity/2026-10-19.json");
        serde_json::from_str(fixture).unwrap()
    }

    #[test]
    fn a_recorded_day_measures_as_the_audit_counted_it() {
        let report = measure(&recorded());
        assert_eq!(report.commits["total"], 4);
        assert_eq!(
            report.commits["by_author"][0]["name"],
            "AtlantisPleb with Claude Opus 5.5"
        );
        assert_eq!(report.commits["by_author"][0]["count"], 2);
        // Two filed in the window; three closed (one filed the day before).
        assert_eq!(report.issues["filed"], 2);
        assert_eq!(report.issues["closed"], 3);
        assert_eq!(report.issues["median_hours_to_close"], 2.0);
        assert_eq!(report.issues["p90_hours_to_close"], 30.0);
        assert_eq!(report.land["entries"], 4);
        assert_eq!(report.land["landed_median_minutes"], 28.0);
        assert_eq!(report.land["bounces"][0]["cause"], "missing dependency");
        assert_eq!(report.land["bounces"][1]["cause"], "stale generated file");
        assert_eq!(report.runs["runs"], 3);
        assert_eq!(report.runs["failed"], 2);
        assert_eq!(report.runs["undated"], 1);
        // Three bad check-ins from 21:25 until the good one at 22:00.
        assert_eq!(report.coordinator["check_ins"], 5);
        assert_eq!(report.coordinator["empty_or_errored"], 3);
        assert_eq!(report.coordinator["down_minutes"], 35);
        let text = report.text();
        assert!(text.lines().count() <= 20, "{text}");
        assert!(text.contains("Commits on main: 4"), "{text}");
        assert!(text.contains("Coordinator: down 35 min"), "{text}");
        assert!(text.contains("Cloud runs: 2 failed of 3"), "{text}");
    }

    #[test]
    fn an_input_that_could_not_be_read_is_unknown_not_zero() {
        let report = measure(&Input {
            now: NOW,
            hours: 24,
            errors: vec!["issues: gh failed".into()],
            ..Input::default()
        });
        assert!(report.issues.is_null());
        let text = report.text();
        assert!(text.contains("Issues: not known"), "{text}");
        assert!(text.contains("Unknown: issues: gh failed"), "{text}");
    }

    #[test]
    fn the_daily_report_rides_the_first_check_in_after_13_utc() {
        let day = at("2026-10-20T00:00:00Z") / 86_400;
        assert_eq!(daily_due(at("2026-10-20T12:59:00Z"), None), None);
        assert_eq!(daily_due(at("2026-10-20T13:01:00Z"), None), Some(day));
        assert_eq!(daily_due(at("2026-10-20T13:11:00Z"), Some(day)), None);
        assert_eq!(
            daily_due(at("2026-10-21T13:00:00Z"), Some(day)),
            Some(day + 1)
        );
    }

    #[test]
    fn the_newest_decision_log_is_found_under_the_app_data() {
        let home = tempfile::tempdir().unwrap();
        let log = home
            .path()
            .join("Library/Application Support/OpenAgents/data/profiles/p1/agent-wakes/a1");
        std::fs::create_dir_all(&log).unwrap();
        std::fs::write(log.join("decisions.jsonl"), "{}\n").unwrap();
        assert_eq!(
            newest_decision_log(home.path()),
            Some(log.join("decisions.jsonl"))
        );
        assert_eq!(newest_decision_log(&home.path().join("none")), None);
    }
}
