//! Coder One's components, as isolated runs and episodes recorded them.
//!
//! `coder-one component suite` and `component run` record each run as a
//! session log of invocation records under
//! `~/.openagents/coder-one/components/`: a `suite` invocation with one
//! child per fixture, whose end event holds the fixture's output, metrics,
//! and Jev mode, and each Jev request below that. Episodes record the same
//! invocation records, or have them derived from their trajectories. This
//! module puts the two side by side, per component: the isolated suites
//! with their metrics, cost, and latency, and the episode invocations with
//! their count, latency, and cost.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value, json};

use crate::terminal_bench::Records;
use crate::timeline::{self, Entry, Timeline};

/// The schema of the components report.
pub const SCHEMA: &str = "openagents.gym.coder-components.v1";

/// The components Coder One runs today, in episode order.
pub const KNOWN: &[&str] = &[
    "task.requirements",
    "evidence.setup",
    "evidence.probes",
    "evidence.probes.planner",
    "host.operation",
    "evidence.probes.selector",
    "evidence.select",
    "evidence.pack",
    "exec.explore",
    "exec.system",
    "exec.session",
    "control.monitor",
    "verify.close",
];

/// One fixture's run inside a suite.
#[derive(Clone, Debug, PartialEq)]
pub struct FixtureRun {
    pub fixture: String,
    pub outcome: String,
    pub milliseconds: Option<u64>,
    pub metrics: Map<String, Value>,
    pub output_digest: Option<String>,
    /// Jev requests by how they were answered.
    pub jev: Map<String, Value>,
    pub cost_usd: Option<f64>,
    pub error: Option<String>,
}

/// One isolated suite: a component over its fixtures.
#[derive(Clone, Debug, PartialEq)]
pub struct Suite {
    pub log: PathBuf,
    pub component: String,
    pub implementation: Option<String>,
    pub jev_mode: String,
    pub started_at: Option<u64>,
    pub complete: bool,
    pub runs: Vec<FixtureRun>,
    /// The suite's own metric summary, as its end event recorded it.
    pub summary: Value,
}

impl Suite {
    /// The suite's Jev cost, when every request's cost is known.
    #[must_use]
    pub fn cost_usd(&self) -> Option<f64> {
        self.runs
            .iter()
            .map(|run| run.cost_usd)
            .sum::<Option<f64>>()
            .map(|usd| usd + 0.0)
    }

    /// Mean milliseconds per fixture.
    #[must_use]
    pub fn mean_ms(&self) -> Option<f64> {
        let times: Vec<u64> = self
            .runs
            .iter()
            .filter_map(|run| run.milliseconds)
            .collect();
        (!times.is_empty()).then(|| times.iter().sum::<u64>() as f64 / times.len() as f64)
    }
}

/// A component's invocations across episodes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EpisodeUse {
    pub invocations: usize,
    pub attempts: usize,
    pub durations: Vec<u64>,
    /// Summed cost; `None` once an invocation's cost is unknown.
    pub cost_usd: Option<f64>,
    pub outcomes: BTreeMap<String, usize>,
    /// Invocation counts by timeline source: log, trajectory, or derived.
    pub sources: BTreeMap<String, usize>,
    /// Leaf invocations by cost provenance, such as `price_estimate`,
    /// `none` for a known zero, or `unknown`.
    pub provenances: BTreeMap<String, usize>,
    /// Leaf invocations whose cost is unknown: work that may have been
    /// billed and was never reported. Never counted as zero.
    pub unknown_cost: usize,
}

impl EpisodeUse {
    #[must_use]
    pub fn mean_ms(&self) -> Option<f64> {
        (!self.durations.is_empty())
            .then(|| self.durations.iter().sum::<u64>() as f64 / self.durations.len() as f64)
    }
}

/// One component's row.
#[derive(Clone, Debug, PartialEq)]
pub struct Row {
    pub component: String,
    /// Isolated suites, newest first.
    pub suites: Vec<Suite>,
    pub episodes: EpisodeUse,
}

/// Every component's isolated runs and episode invocations.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Report {
    pub rows: Vec<Row>,
    pub runs_dir: Option<PathBuf>,
    pub errors: Vec<String>,
    /// How the attempts used their episode deadlines.
    pub deadlines: Deadlines,
}

/// Episode deadline use across attempts, from each manifest's `deadline`.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Deadlines {
    /// Attempts that recorded a deadline, bounded or not.
    pub recorded: usize,
    /// Attempts with a hard deadline.
    pub bounded: usize,
    /// Each bounded attempt's elapsed share of its deadline.
    pub used_fractions: Vec<f64>,
    /// Cuts by what asked: shortened, then skipped.
    pub cuts: BTreeMap<String, (usize, usize)>,
}

impl Deadlines {
    fn of(records: &Records) -> Self {
        let mut deadlines = Deadlines::default();
        for deadline in records.attempts.iter().filter_map(|a| a.deadline.as_ref()) {
            deadlines.recorded += 1;
            if deadline["kind"].as_str() == Some("hard") {
                deadlines.bounded += 1;
                if let (Some(elapsed), Some(total)) = (
                    deadline["elapsed_ms"].as_u64(),
                    deadline["total_ms"].as_u64().filter(|total| *total > 0),
                ) {
                    deadlines.used_fractions.push(elapsed as f64 / total as f64);
                }
            }
            for cut in deadline["cuts"].as_array().into_iter().flatten() {
                // Name a cut by its kind, not its number: `delegate-1` and
                // `delegate-2` are both dispatches.
                let what = cut["what"].as_str().unwrap_or("?");
                let what = what
                    .trim_end_matches(|c: char| c.is_ascii_digit())
                    .trim_end_matches('-');
                let slot = deadlines.cuts.entry(what.to_owned()).or_default();
                if cut["skipped"].as_bool() == Some(true) {
                    slot.1 += 1;
                } else {
                    slot.0 += 1;
                }
            }
        }
        deadlines
    }

    fn mean_used(&self) -> Option<f64> {
        (!self.used_fractions.is_empty())
            .then(|| self.used_fractions.iter().sum::<f64>() / self.used_fractions.len() as f64)
    }
}

/// Where component runs record themselves by default.
#[must_use]
pub fn default_runs_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .filter(|home| !home.is_empty())
        .map(|home| PathBuf::from(home).join(".openagents/coder-one/components"))
}

/// Reads one run log into its suites.
fn suites_in(timeline: &Timeline) -> Vec<Suite> {
    let entries = &timeline.entries;
    let children = |id: &str| -> Vec<&Entry> {
        entries
            .iter()
            .filter(|entry| entry.parent.as_deref() == Some(id))
            .collect()
    };
    entries
        .iter()
        .filter(|entry| entry.component == "suite")
        .map(|root| {
            let component = root.name.clone().unwrap_or_default();
            let runs = children(&root.id)
                .into_iter()
                .filter(|child| child.component == component)
                .map(|child| {
                    let text = |key: &str| child.output.get(key).cloned().unwrap_or(Value::Null);
                    FixtureRun {
                        fixture: child.name.clone().unwrap_or_default(),
                        outcome: child.outcome.clone(),
                        milliseconds: child.duration_ms,
                        metrics: text("metrics").as_object().cloned().unwrap_or_default(),
                        output_digest: child.output_digest.clone(),
                        jev: text("jev").as_object().cloned().unwrap_or_default(),
                        cost_usd: children(&child.id)
                            .iter()
                            .map(|request| request.cost_usd)
                            .sum::<Option<f64>>()
                            .map(|usd| usd + 0.0),
                        error: text("error").as_str().map(str::to_owned),
                    }
                })
                .collect::<Vec<_>>();
            Suite {
                log: timeline.path.clone(),
                implementation: root.implementation.clone(),
                jev_mode: root
                    .output
                    .get("jev_mode")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown")
                    .to_owned(),
                started_at: root.started_at,
                complete: timeline.complete && root.outcome != "unknown",
                summary: root.output.get("summary").cloned().unwrap_or(Value::Null),
                component,
                runs,
            }
        })
        .collect()
}

/// Reads every run log in `dir`.
#[must_use]
pub fn load_suites(dir: &Path) -> (Vec<Suite>, Vec<String>) {
    let mut suites = Vec::new();
    let mut errors = Vec::new();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .is_some_and(|name| name.to_string_lossy().ends_with(".atif.jsonl"))
        })
        .collect();
    paths.sort();
    for path in paths {
        match timeline::read_log(&path) {
            Ok(timeline) => suites.extend(suites_in(&timeline)),
            Err(error) => errors.push(error),
        }
    }
    (suites, errors)
}

/// Every component's invocations across the attempts in `records`.
#[must_use]
pub fn episode_use(records: &Records) -> (BTreeMap<String, EpisodeUse>, Vec<String>) {
    let mut uses: BTreeMap<String, EpisodeUse> = BTreeMap::new();
    let mut unreadable = 0;
    for attempt in &records.attempts {
        let timeline = match timeline::for_attempt(attempt) {
            Some(Ok(timeline)) => timeline,
            // The attempt view already notes an unreadable trajectory.
            Some(Err(_)) => {
                unreadable += 1;
                continue;
            }
            None => continue,
        };
        let mut seen = std::collections::BTreeSet::new();
        for entry in &timeline.entries {
            if entry.component == "episode" || entry.component.is_empty() {
                continue;
            }
            let usage = uses
                .entry(entry.component.clone())
                .or_insert_with(|| EpisodeUse {
                    cost_usd: Some(0.0),
                    ..EpisodeUse::default()
                });
            usage.invocations += 1;
            if seen.insert(entry.component.clone()) {
                usage.attempts += 1;
            }
            usage.durations.extend(entry.duration_ms);
            // Only leaves carry cost; a parent's cost is its children's.
            let leaf = !timeline
                .entries
                .iter()
                .any(|other| other.parent.as_deref() == Some(&entry.id));
            if leaf {
                let provenance = entry.cost_provenance.clone().unwrap_or_else(|| {
                    if entry.cost_usd.is_some() {
                        "reported".to_owned()
                    } else {
                        "none".to_owned()
                    }
                });
                *usage.provenances.entry(provenance).or_default() += 1;
                if entry.cost_usd.is_none() && entry.cost_provenance.is_some() {
                    usage.unknown_cost += 1;
                }
                usage.cost_usd = match (
                    usage.cost_usd,
                    entry.cost_usd,
                    entry.cost_provenance.as_deref(),
                ) {
                    (Some(sum), Some(cost), _) => Some(sum + cost),
                    (Some(sum), None, None) => Some(sum),
                    _ => None,
                };
            }
            *usage.outcomes.entry(entry.outcome.clone()).or_default() += 1;
            *usage
                .sources
                .entry(timeline.source.label().to_owned())
                .or_default() += 1;
        }
    }
    let errors = if unreadable == 0 {
        Vec::new()
    } else {
        vec![format!(
            "{unreadable} attempts have an unreadable trajectory and no invocation log; their invocations are not counted"
        )]
    };
    (uses, errors)
}

/// The report: runs from `runs_dir` and episodes from `records`.
#[must_use]
pub fn report(runs_dir: Option<&Path>, records: &Records) -> Report {
    let (mut suites, mut errors) = runs_dir.map(load_suites).unwrap_or_default();
    let (mut uses, episode_errors) = episode_use(records);
    errors.extend(episode_errors);
    suites.sort_by(|a, b| b.started_at.cmp(&a.started_at).then(b.log.cmp(&a.log)));
    let mut ids: Vec<String> = KNOWN.iter().map(|id| (*id).to_owned()).collect();
    for id in suites
        .iter()
        .map(|suite| suite.component.clone())
        .chain(uses.keys().cloned())
    {
        if !ids.contains(&id) {
            ids.push(id);
        }
    }
    let rows = ids
        .into_iter()
        .map(|component| Row {
            suites: suites
                .iter()
                .filter(|suite| suite.component == component)
                .cloned()
                .collect(),
            episodes: uses.remove(&component).unwrap_or_default(),
            component,
        })
        .collect();
    Report {
        rows,
        runs_dir: runs_dir.map(Path::to_path_buf),
        errors,
        deadlines: Deadlines::of(records),
    }
}

fn money(value: Option<f64>) -> String {
    value.map_or("unknown".to_owned(), |usd| format!("${usd:.6}"))
}

fn millis(value: Option<f64>) -> String {
    value.map_or("—".to_owned(), |ms| format!("{ms:.1} ms"))
}

fn short(value: &Value) -> String {
    match value {
        Value::Null => "—".to_owned(),
        Value::String(text) => text.clone(),
        Value::Number(number) => number
            .as_f64()
            .filter(|n| n.fract() != 0.0)
            .map_or_else(|| number.to_string(), |n| format!("{n:.4}")),
        other => other.to_string(),
    }
}

fn stamp(ms: Option<u64>) -> String {
    ms.map_or("—".to_owned(), |ms| {
        let seconds = ms / 1000;
        let (days, rest) = (seconds / 86_400, seconds % 86_400);
        // Civil date from days since 1970-01-01.
        let z = days as i64 + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = doy - (153 * mp + 2) / 5 + 1;
        let month = if mp < 10 { mp + 3 } else { mp - 9 };
        let year = yoe + era * 400 + i64::from(month <= 2);
        format!(
            "{year:04}-{month:02}-{day:02} {:02}:{:02}",
            rest / 3_600,
            rest % 3_600 / 60
        )
    })
}

impl Report {
    fn selected(&self, component: Option<&str>) -> Vec<&Row> {
        self.rows
            .iter()
            .filter(|row| component.is_none_or(|id| row.component == id))
            .collect()
    }

    /// The report as versioned JSON, for one component or all.
    #[must_use]
    pub fn to_json(&self, component: Option<&str>) -> Value {
        json!({
            "schema": SCHEMA,
            "runs_dir": self.runs_dir.as_ref().map(|dir| dir.display().to_string()),
            "components": self.selected(component).iter().map(|row| json!({
                "component": row.component,
                "isolated": {
                    "suites": row.suites.len(),
                    "latest": row.suites.first().map(|suite| json!({
                        "log": suite.log.display().to_string(),
                        "implementation": suite.implementation,
                        "jev_mode": suite.jev_mode,
                        "started_at": suite.started_at,
                        "complete": suite.complete,
                        "fixtures": suite.runs.len(),
                        "errors": suite.runs.iter().filter(|run| run.error.is_some() || run.outcome != "completed").count(),
                        "mean_ms": suite.mean_ms(),
                        "cost_usd": suite.cost_usd(),
                        "summary": suite.summary,
                        "runs": suite.runs.iter().map(|run| json!({
                            "fixture": run.fixture,
                            "outcome": run.outcome,
                            "milliseconds": run.milliseconds,
                            "output_digest": run.output_digest,
                            "metrics": run.metrics,
                            "jev": run.jev,
                            "cost_usd": run.cost_usd,
                            "error": run.error,
                        })).collect::<Vec<_>>(),
                    })),
                    "history": row.suites.iter().map(|suite| json!({
                        "log": suite.log.display().to_string(),
                        "implementation": suite.implementation,
                        "jev_mode": suite.jev_mode,
                        "started_at": suite.started_at,
                        "fixtures": suite.runs.len(),
                        "cost_usd": suite.cost_usd(),
                    })).collect::<Vec<_>>(),
                },
                "episodes": {
                    "invocations": row.episodes.invocations,
                    "attempts": row.episodes.attempts,
                    "mean_ms": row.episodes.mean_ms(),
                    "cost_usd": if row.episodes.invocations == 0 { Value::Null } else { json!(row.episodes.cost_usd) },
                    "outcomes": row.episodes.outcomes,
                    "sources": row.episodes.sources,
                    "cost_provenances": row.episodes.provenances,
                    "unknown_cost_invocations": row.episodes.unknown_cost,
                },
            })).collect::<Vec<_>>(),
            "deadlines": {
                "attempts_recorded": self.deadlines.recorded,
                "attempts_bounded": self.deadlines.bounded,
                "mean_used_fraction": self.deadlines.mean_used(),
                "cuts": self.deadlines.cuts.iter().map(|(what, (cut, skipped))| json!({
                    "what": what,
                    "shortened": cut,
                    "skipped": skipped,
                })).collect::<Vec<_>>(),
            },
            "read_errors": self.errors,
        })
    }

    /// The report as text rows, for the CLI and the terminal.
    #[must_use]
    pub fn lines(&self, component: Option<&str>) -> Vec<String> {
        let mut lines = vec![format!(
            "Coder One components · isolated runs from {} · episodes from Terminal-Bench evidence",
            self.runs_dir
                .as_ref()
                .map_or("nowhere".to_owned(), |dir| dir.display().to_string())
        )];
        for row in self.selected(component) {
            lines.push(String::new());
            lines.push(row.component.clone());
            match row.suites.first() {
                None => lines.push("  isolated: no recorded runs".to_owned()),
                Some(suite) => {
                    lines.push(format!(
                        "  isolated: {} suites · latest {} · Jev {} · {} fixtures · {} errors · {} per fixture · Jev cost {}{}",
                        row.suites.len(),
                        stamp(suite.started_at),
                        suite.jev_mode,
                        suite.runs.len(),
                        suite
                            .runs
                            .iter()
                            .filter(|run| run.error.is_some() || run.outcome != "completed")
                            .count(),
                        millis(suite.mean_ms()),
                        money(suite.cost_usd()),
                        if suite.complete { "" } else { " · INCOMPLETE" }
                    ));
                    lines.push(format!(
                        "    implementation {}",
                        suite.implementation.as_deref().unwrap_or("—")
                    ));
                    if let Some(metrics) =
                        suite.summary.pointer("/metrics").and_then(Value::as_object)
                    {
                        let parts: Vec<String> = metrics
                            .iter()
                            .map(|(name, value)| match value.get("mean") {
                                Some(mean) => format!("{name} {}", short(mean)),
                                None => format!(
                                    "{name} {}/{}",
                                    short(&value["true"]),
                                    short(&value["of"])
                                ),
                            })
                            .collect();
                        lines.push(format!("    metrics: {}", parts.join(" · ")));
                    }
                    for run in &suite.runs {
                        let metrics: Vec<String> = run
                            .metrics
                            .iter()
                            .take(6)
                            .map(|(name, value)| format!("{name} {}", short(value)))
                            .collect();
                        lines.push(format!(
                            "    {:<62} {:>9}  out {}  {}",
                            run.fixture.chars().take(62).collect::<String>(),
                            millis(run.milliseconds.map(|ms| ms as f64)),
                            run.output_digest
                                .as_deref()
                                .map_or("—".to_owned(), |digest| digest
                                    .chars()
                                    .take(10)
                                    .collect()),
                            run.error.clone().unwrap_or_else(|| metrics.join(" · "))
                        ));
                    }
                }
            }
            let episodes = &row.episodes;
            lines.push(if episodes.invocations == 0 {
                "  episodes: no recorded invocations".to_owned()
            } else {
                format!(
                    "  episodes: {} invocations in {} attempts · {} mean · cost {} · {} · {}",
                    episodes.invocations,
                    episodes.attempts,
                    millis(episodes.mean_ms()),
                    money(episodes.cost_usd),
                    episodes
                        .outcomes
                        .iter()
                        .map(|(outcome, n)| format!("{outcome} {n}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    episodes
                        .sources
                        .iter()
                        .map(|(source, n)| format!("{source} {n}"))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            });
            if !episodes.provenances.is_empty() {
                lines.push(format!(
                    "    cost by provenance: {} · {} unknown",
                    episodes
                        .provenances
                        .iter()
                        .map(|(provenance, n)| format!("{provenance} {n}"))
                        .collect::<Vec<_>>()
                        .join(", "),
                    episodes.unknown_cost
                ));
            }
        }
        if component.is_none() {
            let deadlines = &self.deadlines;
            lines.push(String::new());
            lines.push(if deadlines.bounded == 0 {
                format!(
                    "Episode deadlines: none of {} recorded attempts had one; the harness timeout was the only limit",
                    deadlines.recorded
                )
            } else {
                format!(
                    "Episode deadlines: {} of {} recorded attempts bounded · {} of the deadline used on average",
                    deadlines.bounded,
                    deadlines.recorded,
                    deadlines
                        .mean_used()
                        .map_or("—".to_owned(), |used| format!("{:.0}%", used * 100.0))
                )
            });
            for (what, (cut, skipped)) in &deadlines.cuts {
                lines.push(format!("  cut {what}: {cut} shortened, {skipped} skipped"));
            }
        }
        lines.extend(
            self.errors
                .iter()
                .map(|error| format!("READ ERROR: {error}")),
        );
        lines
    }
}

const HELP: &str = "\
gym coder components  isolated component runs and episode invocations, side by side

  --component ID           one component, such as evidence.pack
  --runs-dir PATH          component run logs (default ~/.openagents/coder-one/components)
  --jobs-dir PATH          local Harbor jobs (default ~/.openagents/terminal-bench/jobs)
  --traces-dir PATH        retained checkout traces
  --no-runs | --no-jobs | --no-traces  omit one source
  --json                   print versioned JSON instead of text

Record isolated runs with `coder-one component suite ID`.";

/// `gym coder components …`.
///
/// # Errors
///
/// Returns a message for an unknown option.
pub fn command(args: &[String], out: &mut impl std::io::Write) -> Result<i32, String> {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench");
    let mut runs = default_runs_dir();
    let mut jobs = std::env::var_os("HOME")
        .map(PathBuf::from)
        .map(|home| home.join(".openagents/terminal-bench/jobs"));
    let mut traces = Some(repo.join("traces"));
    let mut component = None;
    let mut json_output = false;
    let mut index = 0;
    while index < args.len() {
        let argument = args[index].as_str();
        match argument {
            "help" | "--help" | "-h" => {
                writeln!(out, "{HELP}").map_err(|error| error.to_string())?;
                return Ok(0);
            }
            "--json" => json_output = true,
            "--no-runs" => runs = None,
            "--no-jobs" => jobs = None,
            "--no-traces" => traces = None,
            "--component" | "--runs-dir" | "--jobs-dir" | "--traces-dir" => {
                let value = args
                    .get(index + 1)
                    .filter(|value| !value.starts_with("--"))
                    .ok_or_else(|| format!("{argument} needs a value"))?;
                match argument {
                    "--component" => component = Some(value.clone()),
                    "--runs-dir" => runs = Some(value.into()),
                    "--jobs-dir" => jobs = Some(value.into()),
                    _ => traces = Some(value.into()),
                }
                index += 1;
            }
            other => return Err(format!("unknown option {other}\n\n{HELP}")),
        }
        index += 1;
    }
    let records = Records::load(jobs.as_deref(), traces.as_deref(), None);
    let report = report(runs.as_deref(), &records);
    if let Some(id) = &component
        && !report.rows.iter().any(|row| &row.component == id)
    {
        return Err(format!("no component {id} in the runs or episodes"));
    }
    // `exec.system` variants compare on their captured first request and
    // the attempts that ran them.
    let prompts = matches!(component.as_deref(), None | Some("exec.system"))
        .then(|| crate::coder_prompt::comparison(&records));
    if json_output {
        let mut value = report.to_json(component.as_deref());
        if let Some(prompts) = &prompts {
            value["system_variants"] = prompts.to_json();
        }
        serde_json::to_writer_pretty(&mut *out, &value).map_err(|error| error.to_string())?;
        writeln!(out).map_err(|error| error.to_string())?;
    } else {
        for line in report.lines(component.as_deref()) {
            writeln!(out, "{line}").map_err(|error| error.to_string())?;
        }
        if let Some(prompts) = &prompts {
            writeln!(out).map_err(|error| error.to_string())?;
            for line in prompts.lines() {
                writeln!(out, "{line}").map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        kind: &str,
        id: &str,
        parent: Option<&str>,
        component: &str,
        name: Option<&str>,
        at: u64,
        extra: Value,
    ) -> String {
        let mut record = json!({
            "schema": timeline::INVOCATION_SCHEMA, "event": kind, "id": id, "parent": parent,
            "component": component, "name": name, "at": at,
            "implementation": { "name": "test", "digest": "0123456789abcdef" },
        });
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        json!({ "record": "step", "step": { "at": at, "source": "System", "message": "", "extensions": { "invocation": record } } }).to_string()
    }

    /// A suite log shaped like the one `coder-one component suite` writes.
    fn suite_log(dir: &Path) -> PathBuf {
        let path = dir.join("component-evidence.pack-1000.atif.jsonl");
        let session = atif::Session::opening(
            "component-evidence.pack-1000",
            "jev-1.13.0",
            "component-runner",
            "/r",
            "v",
        );
        let mut log = atif::Log::create_at(&path, &session).unwrap();
        let lines = [
            event(
                "start",
                "inv-1",
                None,
                "suite",
                Some("evidence.pack"),
                1000,
                json!({}),
            ),
            event(
                "start",
                "inv-2",
                Some("inv-1"),
                "evidence.pack",
                Some("fixture-a"),
                1001,
                json!({}),
            ),
            event(
                "end",
                "inv-2",
                None,
                "evidence.pack",
                None,
                1002,
                json!({
                    "outcome": "completed", "milliseconds": 1,
                    "output": { "digest": "abcdef0123456789", "summary": {
                        "fixture": "fixture-a", "jev_mode": "recorded",
                        "metrics": { "reproduces_retained": true, "omitted_items": 3 },
                        "jev": {}, "error": null } },
                }),
            ),
            event(
                "end",
                "inv-1",
                None,
                "suite",
                None,
                1003,
                json!({
                    "outcome": "completed", "milliseconds": 3,
                    "output": { "digest": "x", "summary": { "component": "evidence.pack", "jev_mode": "recorded",
                        "summary": { "metrics": { "reproduces_retained": { "true": 1, "of": 1 }, "omitted_items": { "mean": 3.0, "of": 1 } } } } },
                }),
            ),
        ];
        log.finish(atif::log::ENDED).unwrap();
        // Insert the events before the end record.
        let text = std::fs::read_to_string(&path).unwrap();
        let (head, end) = text.trim_end().rsplit_once('\n').unwrap();
        std::fs::write(&path, format!("{head}\n{}\n{end}\n", lines.join("\n"))).unwrap();
        path
    }

    #[test]
    fn isolated_runs_and_retained_episodes_show_side_by_side() {
        let dir = std::env::temp_dir().join(format!("gym-components-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        suite_log(&dir);
        let traces =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces");
        let records = Records::load(None, Some(&traces), None);
        let report = report(Some(&dir), &records);
        let pack = report
            .rows
            .iter()
            .find(|row| row.component == "evidence.pack")
            .unwrap();
        assert_eq!(pack.suites.len(), 1);
        let suite = &pack.suites[0];
        assert_eq!(suite.jev_mode, "recorded");
        assert_eq!(suite.runs[0].metrics["omitted_items"], json!(3));
        assert!(suite.complete);
        // Every retained v3 Luna trial derived an `evidence.pack` invocation.
        assert!(pack.episodes.invocations >= 24);
        let value = report.to_json(Some("evidence.pack"));
        assert_eq!(value["components"].as_array().unwrap().len(), 1);
        assert_eq!(value["components"][0]["isolated"]["latest"]["fixtures"], 1);
        let text = report.lines(Some("evidence.pack")).join("\n");
        assert!(text.contains("isolated: 1 suites"), "{text}");
        assert!(text.contains("reproduces_retained 1/1"), "{text}");
        assert!(text.contains("episodes:"), "{text}");
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn deadline_use_and_cuts_are_summed_across_attempts() {
        let mut bounded = crate::terminal_bench::test_attempt();
        bounded.deadline = Some(json!({
            "kind": "hard", "total_ms": 1_000_000, "elapsed_ms": 800_000,
            "cuts": [
                {"what": "delegate-1", "skipped": false},
                {"what": "jev_close", "skipped": true},
            ],
        }));
        let mut open = crate::terminal_bench::test_attempt();
        open.deadline = Some(json!({ "kind": "none", "elapsed_ms": 5_000, "cuts": [] }));
        let records = Records {
            attempts: vec![bounded, open],
            ..Records::default()
        };
        let report = report(None, &records);
        assert_eq!(report.deadlines.recorded, 2);
        assert_eq!(report.deadlines.bounded, 1);
        assert_eq!(report.deadlines.cuts["delegate"], (1, 0));
        assert_eq!(report.deadlines.cuts["jev_close"], (0, 1));
        let value = report.to_json(None);
        assert_eq!(value["deadlines"]["mean_used_fraction"], 0.8);
        let text = report.lines(None).join("\n");
        assert!(text.contains("1 of 2 recorded attempts bounded"), "{text}");
        assert!(text.contains("80% of the deadline used"), "{text}");
        assert!(
            text.contains("cut delegate: 1 shortened, 0 skipped"),
            "{text}"
        );
    }
}
