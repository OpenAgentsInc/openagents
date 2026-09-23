//! `coder-one study …`: run a hill-climbing study over policy manifests.

use std::path::PathBuf;

use serde_json::Value;

use super::Tier;
use super::harness::TerminalBench;
use super::pack::{Options, run};

/// The study commands' usage.
pub const USAGE: &str =
    "usage: coder-one study run evidence.pack [--through replay|mini|screen|measure|confirm]
                            [--operators swap,grid,climb,random,reflect,router-refit]
                            [--reflection FILE] [--seed N] [--climb-moves N] [--random-draws N]
                            [--traces DIR] [--fixtures DIR] [--out DIR] [--retain [DIR]]
                            [--allow-terminal-bench --artifact FILE --artifact-sha256 HEX] [--json]
       coder-one study list [--out DIR] [--json]

A study screens candidates cheapest first. Tier 0 (replay) always runs: every
candidate packs the retained briefings of the search partition, the best third
moves to the selection partition, and the best third of those is promoted.
--through mini also runs every mini-task with the scripted executor under each
promoted candidate. The Terminal-Bench tiers (screen, measure, confirm) cost
money and time: they run only with --allow-terminal-bench and the arm's artifact,
and otherwise the result lists the commands they would run.
Studies are recorded under ~/.openagents/coder-one/studies unless --out names
another directory. --retain copies the plan, candidates, and result into
bench/terminal-bench/studies, or DIR.";

/// Runs `coder-one study …` and returns the exit code.
///
/// # Errors
///
/// Returns a message for a malformed command or a study that can't run.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let mut positional = Vec::new();
    let mut out: Option<PathBuf> = None;
    let mut json_output = false;
    let mut through = Tier::Replay;
    let mut allow = false;
    let mut artifact: Option<PathBuf> = None;
    let mut artifact_sha256: Option<String> = None;
    let mut operators: Option<Vec<String>> = None;
    let mut reflection = None;
    let mut seed = None;
    let mut climb_moves = None;
    let mut random_draws = None;
    let mut traces = None;
    let mut fixtures = None;
    let mut retain: Option<PathBuf> = None;
    let mut iter = args.iter().peekable();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        let number = |name: &str, text: String| {
            text.parse::<u64>()
                .map_err(|_| format!("{name} takes a whole number, not {text}"))
        };
        match arg.as_str() {
            "--out" => out = Some(value("--out")?.into()),
            "--json" => json_output = true,
            "--through" => through = Tier::parse(&value("--through")?)?,
            "--allow-terminal-bench" => allow = true,
            "--artifact" => artifact = Some(value("--artifact")?.into()),
            "--artifact-sha256" => artifact_sha256 = Some(value("--artifact-sha256")?),
            "--operators" => {
                operators = Some(
                    value("--operators")?
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect(),
                );
            }
            "--reflection" => reflection = Some(PathBuf::from(value("--reflection")?)),
            "--seed" => seed = Some(number("--seed", value("--seed")?)?),
            "--climb-moves" => {
                climb_moves = Some(number("--climb-moves", value("--climb-moves")?)?)
            }
            "--random-draws" => {
                random_draws = Some(number("--random-draws", value("--random-draws")?)?);
            }
            "--traces" => traces = Some(PathBuf::from(value("--traces")?)),
            "--fixtures" => fixtures = Some(PathBuf::from(value("--fixtures")?)),
            "--retain" => {
                retain = Some(match iter.peek() {
                    Some(next) if !next.starts_with("--") => PathBuf::from(iter.next().unwrap()),
                    _ => super::retained_dir(),
                });
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown option {other}\n{USAGE}"));
            }
            other => positional.push(other.to_string()),
        }
    }
    let out = out
        .or_else(super::default_dir)
        .ok_or("no --out and no HOME to record under")?;
    match positional.first().map(String::as_str) {
        Some("run") => {
            if positional.get(1).map(String::as_str) != Some(super::pack::COMPONENT) {
                return Err(format!(
                    "study run takes {}, the one study this build defines\n{USAGE}",
                    super::pack::COMPONENT
                ));
            }
            if through >= Tier::Screen && !allow {
                return Err(format!(
                    "--through {} runs Terminal-Bench trials; add --allow-terminal-bench with \
                     --artifact and --artifact-sha256 to spend them",
                    through.word()
                ));
            }
            let mut options = Options::new(out);
            options.through = through;
            if allow {
                let (Some(artifact), Some(sha)) = (artifact, artifact_sha256) else {
                    return Err(
                        "--allow-terminal-bench needs --artifact FILE and --artifact-sha256 HEX"
                            .to_string(),
                    );
                };
                options.terminal_bench = Some(TerminalBench::new(artifact, sha));
            }
            if let Some(operators) = operators {
                options.operators = operators;
            }
            options.reflection = reflection;
            if let Some(seed) = seed {
                options.seed = seed;
            }
            if let Some(moves) = climb_moves {
                options.climb_moves = usize::try_from(moves).unwrap_or(usize::MAX);
            }
            if let Some(draws) = random_draws {
                options.random_draws = usize::try_from(draws).unwrap_or(usize::MAX);
            }
            if let Some(traces) = traces {
                options.traces = traces;
            }
            if let Some(fixtures) = fixtures {
                options.fixtures = fixtures;
            }
            options.retain = retain;
            let finished = run(&options).await?;
            if json_output {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&finished.result)
                        .map_err(|error| error.to_string())?
                );
            } else {
                print_result(&finished.result);
                println!("recorded under {}", finished.dir.display());
            }
            Ok(0)
        }
        Some("list") => {
            let mut rows = Vec::new();
            if let Ok(entries) = std::fs::read_dir(&out) {
                for entry in entries.flatten() {
                    let path = entry.path().join("result.json");
                    if let Some(result) = std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|t| serde_json::from_str::<Value>(&t).ok())
                    {
                        rows.push(result);
                    }
                }
            }
            rows.sort_by(|a, b| a["study"].as_str().cmp(&b["study"].as_str()));
            if json_output {
                let brief: Vec<Value> = rows
                    .iter()
                    .map(|r| {
                        serde_json::json!({
                            "study": r["study"],
                            "component": r["component"],
                            "status": r["status"],
                            "candidates": r["candidates"].as_array().map_or(0, Vec::len),
                            "selected": r["selected"],
                            "beats_baseline": r.pointer("/confirmation/beats_baseline"),
                        })
                    })
                    .collect();
                println!(
                    "{}",
                    serde_json::to_string_pretty(&brief).map_err(|error| error.to_string())?
                );
            } else if rows.is_empty() {
                println!("no studies under {}", out.display());
            } else {
                for r in &rows {
                    println!(
                        "{}  {}  {} candidates  {}",
                        r["study"].as_str().unwrap_or("?"),
                        r["status"].as_str().unwrap_or("?"),
                        r["candidates"].as_array().map_or(0, Vec::len),
                        r.pointer("/confirmation/statement")
                            .and_then(Value::as_str)
                            .unwrap_or("no candidate reached confirmation"),
                    );
                }
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}

fn print_result(result: &Value) {
    let short = |v: &Value| {
        v.as_str()
            .map_or("none".to_string(), |d| d[..12.min(d.len())].to_string())
    };
    println!(
        "study {} · {} · {}",
        result["study"].as_str().unwrap_or("?"),
        result["component"].as_str().unwrap_or("?"),
        result["status"].as_str().unwrap_or("?"),
    );
    let split = &result["split"];
    println!(
        "split: development {} (search {} · selection {} briefings) · held-out {} ({} briefings)",
        split["development_tasks"]
            .as_array()
            .map(|t| t
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
        split["search"],
        split["selection"],
        split["held_out_tasks"]
            .as_array()
            .map(|t| t
                .iter()
                .filter_map(Value::as_str)
                .collect::<Vec<_>>()
                .join(", "))
            .unwrap_or_default(),
        split["confirmation"],
    );
    for operator in result["operators"].as_array().into_iter().flatten() {
        println!(
            "  {:<13} {:>4} proposals · {:>4} built · {:>3} duplicates · {:>2} refused · best search J {}",
            operator["operator"].as_str().unwrap_or("?"),
            operator["proposals"],
            operator["built"],
            operator["duplicates"],
            operator["refused"],
            operator
                .pointer("/best_search/j")
                .map_or("—".to_string(), Value::to_string),
        );
    }
    for rung in result["rungs"].as_array().into_iter().flatten() {
        println!(
            "  rung {}/{}: {} evaluated, {} kept",
            rung["tier"].as_str().unwrap_or("?"),
            rung["phase"].as_str().unwrap_or("?"),
            rung["evaluated"],
            rung["kept"],
        );
    }
    println!(
        "baseline {} · selected {}",
        short(&result["baseline"]),
        short(&result["selected"])
    );
    match result["confirmation"]
        .pointer("/statement")
        .and_then(Value::as_str)
    {
        Some(statement) => println!("{statement}"),
        None => println!("No candidate beat the baseline on selection, so nothing was confirmed."),
    }
    println!(
        "spend: ${} over {} ms, {} model or Jev calls",
        result["spend"]["spend_usd"], result["spend"]["wall_ms"], result["spend"]["calls"]
    );
    for tier in result["higher_tiers"].as_array().into_iter().flatten() {
        if tier["run"] == false {
            println!(
                "  {}: not run ({})",
                tier["tier"].as_str().unwrap_or("?"),
                tier["why"].as_str().unwrap_or("")
            );
        }
    }
}
