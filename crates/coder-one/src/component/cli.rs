//! `coder-one component …`: run one component alone, on fixtures.

use std::path::PathBuf;

use serde_json::{Value, json};

use super::{JevChoice, Suite, default_fixtures, default_runs_dir, extract, find, registry};

/// The component commands' usage.
pub const USAGE: &str = "usage: coder-one component list [--json]
       coder-one component run ID --fixture DIR [--jev recorded|live|off] [--save-jev]
                               [--out DIR | --no-record] [--json]
       coder-one component suite ID [--fixtures DIR] [--jev recorded|live|off] [--save-jev]
                                 [--out DIR | --no-record] [--export FILE] [--json]
       coder-one component extract --traces DIR --arm TEXT --out DIR
       coder-one component replay evidence.pack [--traces DIR] [--out FILE] [--json]
       coder-one component replay control.monitor [--traces DIR] [--jev recorded|live|off]
                                  [--recorded FILE] [--save-jev] [--live-limit N] [--sample TEXT]
                                  [--out FILE] [--json]

--jev defaults to recorded: answers replay from each fixture's jev-recorded.json,
and a changed state or question set misses. live calls Jev with TYPESAFE_API_KEY;
--save-jev adds its answers to the fixture. off leaves every judgment unknown.
--export writes each fixture's task and output to FILE, such as the task features
the Gym's router reads: bench/terminal-bench/profiles/task-features.json.
Runs record their invocations under ~/.openagents/coder-one/components unless
--out names another directory or --no-record is given.

replay control.monitor feeds every retained native stream through the monitor
one event at a time and labels each judgment by hindsight. Its Jev answers
replay from --recorded (bench/terminal-bench/monitor/jev-recorded.json); with
--jev live, the first trial of each task under the traces --sample names (the
v3 Luna, v2 Opus, and v2 Luna arms by default) is asked live, at most
--live-limit requests (40), and --save-jev adds the answers to that file.";

/// A live Jev client from `TYPESAFE_API_KEY` or `~/.openagents/jev.json`.
///
/// # Errors
///
/// Returns a message when there is no key.
pub fn live_client() -> Result<::jev::Client, String> {
    let dir = crate::credentials::openagents_dir().unwrap_or_else(|| PathBuf::from("/nonexistent"));
    let env = |name: &str| {
        std::env::var(name)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
    };
    let key = crate::credentials::jev_key(env, &dir)
        .map_err(|error| format!("--jev live needs a Jev key: {error}"))?;
    crate::credentials::jev_client(&key.secret)
}

struct Flags {
    positional: Vec<String>,
    fixture: Option<PathBuf>,
    fixtures: Option<PathBuf>,
    traces: Option<PathBuf>,
    arm: Option<String>,
    out: Option<PathBuf>,
    export: Option<PathBuf>,
    jev: String,
    save_jev: bool,
    record: bool,
    json: bool,
    recorded: Option<PathBuf>,
    live_limit: usize,
    sample: Vec<String>,
}

impl Flags {
    fn parse(args: &[String]) -> Result<Self, String> {
        let mut flags = Flags {
            positional: Vec::new(),
            fixture: None,
            fixtures: None,
            traces: None,
            arm: None,
            out: None,
            export: None,
            jev: "recorded".to_string(),
            save_jev: false,
            record: true,
            json: false,
            recorded: None,
            live_limit: 40,
            sample: Vec::new(),
        };
        let mut args = args.iter();
        while let Some(arg) = args.next() {
            let mut value = |name: &str| {
                args.next()
                    .cloned()
                    .ok_or_else(|| format!("{name} needs a value"))
            };
            match arg.as_str() {
                "--fixture" => flags.fixture = Some(value("--fixture")?.into()),
                "--fixtures" => flags.fixtures = Some(value("--fixtures")?.into()),
                "--traces" => flags.traces = Some(value("--traces")?.into()),
                "--arm" => flags.arm = Some(value("--arm")?),
                "--out" => flags.out = Some(value("--out")?.into()),
                "--export" => flags.export = Some(value("--export")?.into()),
                "--jev" => flags.jev = value("--jev")?,
                "--save-jev" => flags.save_jev = true,
                "--recorded" => flags.recorded = Some(value("--recorded")?.into()),
                "--sample" => flags.sample.push(value("--sample")?),
                "--live-limit" => {
                    flags.live_limit = value("--live-limit")?
                        .parse()
                        .map_err(|_| "--live-limit takes a count".to_string())?;
                }
                "--no-record" => flags.record = false,
                "--json" => flags.json = true,
                other if other.starts_with("--") => return Err(format!("unknown option {other}")),
                other => flags.positional.push(other.to_string()),
            }
        }
        Ok(flags)
    }

    fn choice(&self) -> Result<JevChoice, String> {
        JevChoice::parse(&self.jev, live_client)
    }

    fn out(&self) -> Option<PathBuf> {
        if !self.record {
            return None;
        }
        self.out.clone().or_else(default_runs_dir)
    }
}

/// Runs a component command and returns the exit code: 0 when every
/// fixture ran, 1 when one failed.
///
/// # Errors
///
/// Returns a message for bad arguments or unreadable fixtures.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let flags = Flags::parse(rest)?;
    match verb.as_str() {
        "list" => {
            let components: Vec<Value> = registry()
                .iter()
                .map(|component| {
                    json!({
                        "id": component.id(),
                        "implementation": component.implementation(),
                        "about": component.about(),
                        "fixtures": super::fixtures_for(&default_fixtures(), component.id()).len(),
                    })
                })
                .collect();
            if flags.json {
                print_json(&json!({ "components": components }))?;
            } else {
                for component in &components {
                    println!(
                        "{:<17} {:>3} fixtures  {}  ({}, {})",
                        component["id"].as_str().unwrap_or_default(),
                        component["fixtures"],
                        component["about"].as_str().unwrap_or_default(),
                        component["implementation"]["name"]
                            .as_str()
                            .unwrap_or_default(),
                        &component["implementation"]["digest"]
                            .as_str()
                            .unwrap_or_default()[..12]
                    );
                }
            }
            Ok(0)
        }
        "run" | "suite" => {
            let [id] = flags.positional.as_slice() else {
                return Err(format!("component {verb} needs one component ID\n{USAGE}"));
            };
            let component = find(id)?;
            let dirs = if verb == "run" {
                vec![
                    flags
                        .fixture
                        .clone()
                        .ok_or_else(|| format!("component run needs --fixture DIR\n{USAGE}"))?,
                ]
            } else {
                let root = flags.fixtures.clone().unwrap_or_else(default_fixtures);
                let dirs = super::fixtures_for(&root, id);
                if dirs.is_empty() {
                    return Err(format!(
                        "no fixture under {} has an input for {id}",
                        root.display()
                    ));
                }
                dirs
            };
            let choice = flags.choice()?;
            let recorder = super::recorder(flags.out().as_deref(), id)?;
            let suite = super::suite(
                component.as_ref(),
                &dirs,
                &choice,
                &recorder,
                flags.save_jev,
            )
            .await?;
            if let Some(path) = &flags.export {
                let text = format!(
                    "{}\n",
                    serde_json::to_string_pretty(&super::export(&suite, &dirs)?)
                        .map_err(|error| error.to_string())?
                );
                crate::record::write_atomic(path, text.as_bytes())?;
            }
            if flags.json {
                print_json(&suite.report())?;
            } else {
                print_text(&suite);
            }
            Ok(i32::from(suite.runs.iter().any(|run| run.error.is_some())))
        }
        "extract" => {
            let traces = flags
                .traces
                .clone()
                .ok_or("component extract needs --traces DIR")?;
            let arm = flags
                .arm
                .clone()
                .ok_or("component extract needs --arm TEXT")?;
            let out = flags
                .out
                .clone()
                .ok_or("component extract needs --out DIR")?;
            let extracted = extract::extract_tree(&traces, &arm, &out)?;
            for one in &extracted {
                println!(
                    "{}  {}  {} Jev answers{}",
                    one.dir.display(),
                    one.components.join(", "),
                    one.recorded,
                    match one.reproduces {
                        Some(true) => "  briefing rebuilds exactly",
                        Some(false) => "  briefing does NOT rebuild",
                        None => "",
                    }
                );
            }
            println!("{} attempts extracted", extracted.len());
            Ok(i32::from(
                extracted.iter().any(|one| one.reproduces == Some(false)),
            ))
        }
        "replay" if flags.positional.first().map(String::as_str) == Some("control.monitor") => {
            replay_monitor(&flags).await
        }
        "replay" => {
            if flags.positional.first().map(String::as_str) != Some("evidence.pack") {
                return Err(format!(
                    "component replay takes evidence.pack or control.monitor\n{USAGE}"
                ));
            }
            let traces = flags.traces.clone().unwrap_or_else(|| {
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join("../../bench/terminal-bench/traces")
            });
            let report = super::replay::replay_tree(&traces, crate::pack::Params::default());
            let value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
            let out = flags.out.clone().or_else(|| {
                default_runs_dir()
                    .map(|dir| dir.join(format!("pack-replay-{}.json", atif::now_ms())))
            });
            if let Some(path) = &out {
                if let Some(parent) = path.parent() {
                    std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
                }
                let text =
                    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
                crate::record::write_atomic(path, format!("{text}\n").as_bytes())?;
            }
            if flags.json {
                print_json(&value)?;
            } else {
                print_replay(&report);
                if let Some(path) = &out {
                    println!("written to {}", path.display());
                }
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

/// The checkout's monitor directory: the replay report and its recorded
/// Jev answers.
#[must_use]
pub fn monitor_dir() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/monitor")
}

async fn replay_monitor(flags: &Flags) -> Result<i32, String> {
    let traces = flags.traces.clone().unwrap_or_else(|| {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces")
    });
    let recorded_path = flags
        .recorded
        .clone()
        .unwrap_or_else(|| monitor_dir().join("jev-recorded.json"));
    let recorded = super::jev::Recorded::load(&recorded_path)?;
    let live = match flags.jev.as_str() {
        "live" => match flags.choice()? {
            JevChoice::Live(client) => Some(client),
            _ => None,
        },
        "recorded" | "off" => None,
        other => return Err(format!("--jev takes live, recorded, or off, not {other}")),
    };
    let jev = super::monitor::ReplayJev {
        live,
        live_limit: flags.live_limit,
        sample: if flags.sample.is_empty() {
            super::monitor::default_sample()
        } else {
            flags.sample.clone()
        },
        recorded: recorded.clone(),
        on: flags.jev != "off",
    };
    let params = crate::monitor::Params::default();
    let (report, recorder) = super::monitor::replay_tree(&traces, &params, &jev).await;
    if flags.save_jev && flags.jev == "live" {
        let mut recorded = recorded;
        let added = super::jev::record_answers(
            &recorder.steps(),
            "live control.monitor replay",
            &mut recorded,
        );
        if added > 0 {
            recorded.save(&recorded_path)?;
        }
        eprintln!("recorded {added} answers in {}", recorded_path.display());
    }
    let value = serde_json::to_value(&report).map_err(|error| error.to_string())?;
    if let Some(path) = &flags.out {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let text = serde_json::to_string_pretty(&value).map_err(|error| error.to_string())?;
        crate::record::write_atomic(path, format!("{text}\n").as_bytes())?;
    }
    if flags.json {
        print_json(&value)?;
    } else {
        for line in super::monitor::lines(&report) {
            println!("{line}");
        }
        if let Some(path) = &flags.out {
            println!("written to {}", path.display());
        }
    }
    Ok(0)
}

fn print_replay(report: &super::replay::Report) {
    let totals = &report.totals;
    println!(
        "evidence.pack replay · {} manifests · {} first-packer briefings ({} coverage-packed left out) · {} replayed · {} rebuild exactly · {} skipped",
        report.manifests,
        report.briefings,
        report.coverage_packed,
        totals["replayed"],
        totals["reproduces"],
        report.skipped.len()
    );
    println!("{:<52} {:>12} {:>12}", "", "before", "after");
    for (label, key) in [
        ("Jev-selected items", "selected"),
        ("  delivered, whole or trimmed", "selected_delivered"),
        ("  dropped", "selected_dropped"),
        (
            "  dropped although larger than the cap",
            "selected_over_cap_dropped",
        ),
        ("items left out", "omitted_items"),
        ("duplicate listing bytes", "duplicate_bytes"),
        ("data-file characters delivered", "data_chars"),
        (
            "episodes dropping selected items",
            "episodes_dropping_selected",
        ),
        (
            "  while keeping duplicate listings",
            "episodes_dropping_selected_while_duplicates_kept",
        ),
        ("briefings over the cap", "over_cap"),
    ] {
        println!(
            "{label:<52} {:>12} {:>12}",
            short(&totals["before"][key]),
            short(&totals["after"][key])
        );
    }
    println!(
        "requirements naming a path or constant with no delivered evidence after: {} of {}",
        totals["keyed_uncovered_after"], totals["keyed_requirements"]
    );
    for (dir, why) in &report.skipped {
        println!("skipped {dir}: {why}");
    }
}

fn print_json(value: &Value) -> Result<(), String> {
    println!(
        "{}",
        serde_json::to_string_pretty(value).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn short(value: &Value) -> String {
    match value {
        Value::Null => "—".to_string(),
        Value::Number(number) => number.to_string(),
        Value::Bool(flag) => flag.to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn print_text(suite: &Suite) {
    println!(
        "{} · {} ({}) · jev {}",
        suite.component,
        suite.implementation.name,
        &suite.implementation.digest[..12],
        suite.jev_mode
    );
    for run in &suite.runs {
        let metrics: Vec<String> = run
            .metrics
            .iter()
            .map(|(name, value)| format!("{name} {}", short(value)))
            .collect();
        let jev: Vec<String> = run
            .jev
            .iter()
            .map(|(how, count)| format!("{how} {count}"))
            .collect();
        println!(
            "  {:<64} {:>5} ms  jev {}",
            run.fixture,
            run.milliseconds,
            if jev.is_empty() {
                "none".to_string()
            } else {
                jev.join(", ")
            }
        );
        if let Some(error) = &run.error {
            println!("    error: {error}");
        }
        println!("    {}", metrics.join(" · "));
    }
    let summary = suite.summary();
    println!(
        "{} fixtures in {} ms · {} errors · Jev cost {}",
        suite.runs.len(),
        suite.milliseconds,
        summary["errors"],
        summary["cost_usd"]
            .as_f64()
            .map_or("unknown".to_string(), |usd| format!("${usd:.6}"))
    );
    if let Some(metrics) = summary["metrics"].as_object() {
        for (name, value) in metrics {
            if let Some(mean) = value.get("mean") {
                println!("  {name}: mean {} over {}", short(mean), value["of"]);
            } else {
                println!("  {name}: {} of {} true", value["true"], value["of"]);
            }
        }
    }
    if let Some(log) = &suite.log {
        println!("recorded to {}", log.display());
    }
}
