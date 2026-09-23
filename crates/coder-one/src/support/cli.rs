//! `coder-one support …`: build the labeled fixtures, evaluate the paired
//! judgments against the broad "done" baseline, and judge one check input.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::{Params, fit, fixtures};
use crate::checks::{self, Input};
use crate::component::{self, Fixture, JevChoice};
use crate::record::Recorder;

/// The support commands' usage.
pub const USAGE: &str = "usage: coder-one support evaluate [--fixtures DIR] [--jev recorded|live|off] [--save-jev]
                                [--write-checks [--checks-dir DIR]] [--out DIR | --no-record] [--json]
       coder-one support run --input FILE [--jev live|off] [--json]
       coder-one support fixtures [--components DIR] [--traces DIR] [--out DIR]

evaluate runs verify.support over the labeled fixtures (support--*), fits the
supports and contradicts cutoffs on the development split only, and reports
false accepts and false rejects apart for each split, beside the broad \"done\"
judgment. --write-checks also writes each recovered attempt's requirement states
to <checks-dir>/<job>/<trial>/support.json, beside the check the Gym shows
(default ~/.openagents/coder-one/checks). run checks a candidate in a checks input
file and judges its requirements with live Jev. fixtures rebuilds the labeled
set; it runs scenarios, which need python3, and asks nothing of Jev.";

struct Flags {
    fixtures: PathBuf,
    components: PathBuf,
    traces: PathBuf,
    out: Option<PathBuf>,
    input: Option<PathBuf>,
    checks_dir: Option<PathBuf>,
    write_checks: bool,
    jev: String,
    save_jev: bool,
    record: bool,
    json: bool,
}

fn parse(args: &[String]) -> Result<Flags, String> {
    let mut flags = Flags {
        fixtures: component::default_fixtures(),
        components: component::default_fixtures(),
        traces: Path::new(env!("CARGO_MANIFEST_DIR")).join("../../bench/terminal-bench/traces"),
        out: None,
        input: None,
        checks_dir: None,
        write_checks: false,
        jev: "recorded".to_string(),
        save_jev: false,
        record: true,
        json: false,
    };
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        let mut value = |name: &str| {
            iter.next()
                .cloned()
                .ok_or_else(|| format!("{name} needs a value"))
        };
        match arg.as_str() {
            "--fixtures" => flags.fixtures = value("--fixtures")?.into(),
            "--components" => flags.components = value("--components")?.into(),
            "--traces" => flags.traces = value("--traces")?.into(),
            "--out" => flags.out = Some(value("--out")?.into()),
            "--input" => flags.input = Some(value("--input")?.into()),
            "--checks-dir" => flags.checks_dir = Some(value("--checks-dir")?.into()),
            "--jev" => flags.jev = value("--jev")?,
            "--write-checks" => flags.write_checks = true,
            "--save-jev" => flags.save_jev = true,
            "--no-record" => flags.record = false,
            "--json" => flags.json = true,
            other => return Err(format!("unknown option {other}\n\n{USAGE}")),
        }
    }
    Ok(flags)
}

/// The labeled fixture directories under `root`: `support--*`.
#[must_use]
pub fn labeled(root: &Path) -> Vec<PathBuf> {
    component::fixtures_for(root, "verify.support")
        .into_iter()
        .filter(|dir| {
            dir.file_name()
                .is_some_and(|n| n.to_string_lossy().starts_with("support--"))
        })
        .collect()
}

/// Runs the suite over `dirs` and evaluates it.
///
/// # Errors
///
/// Returns a message when a fixture doesn't read.
pub async fn evaluate(
    dirs: &[PathBuf],
    choice: &JevChoice,
    recorder: &Recorder,
    save_live: bool,
) -> Result<(component::Suite, Value), String> {
    let support = component::find("verify.support")?;
    let suite = component::suite(support.as_ref(), dirs, choice, recorder, save_live).await?;
    let outputs: Vec<(String, Value)> = suite
        .runs
        .iter()
        .map(|run| (run.fixture.clone(), run.output.clone()))
        .collect();
    let mut evaluation = fit::evaluate(&fit::rows(&outputs));
    evaluation["jev"] = json!({
        "mode": choice.word(),
        "requests": suite.summary()["jev"],
        "cost_usd": suite.summary()["cost_usd"],
    });
    Ok((suite, evaluation))
}

/// Writes each retained fixture's full support report beside its check:
/// `<checks_dir>/<job>/<trial>/support.json`, judged with recorded Jev.
///
/// # Errors
///
/// Returns a message when a fixture or a file doesn't work.
pub async fn write_checks(dirs: &[PathBuf], checks_dir: &Path) -> Result<usize, String> {
    let mut written = 0;
    for dir in dirs {
        let fixture = Fixture::load(dir, "verify.support")?;
        let (Some(job), Some(trial)) = (
            fixture.source["job"].as_str(),
            fixture.source["trial"].as_str(),
        ) else {
            continue;
        };
        let task = fixtures::task_of(&fixture).ok_or("a fixture without a task")?;
        let evidence: Vec<super::Evidence> =
            serde_json::from_value(fixture.input["evidence"].clone()).map_err(|e| e.to_string())?;
        let skipped: Vec<super::Skipped> =
            serde_json::from_value(fixture.input["skipped"].clone()).unwrap_or_default();
        let mode = JevChoice::Recorded.mode(dir)?;
        let report = super::judge_evidence(
            &task,
            fixture
                .input
                .pointer("/candidate/digest")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            evidence,
            skipped,
            &mode,
            &Recorder::default(),
            Params::default(),
            None,
        )
        .await;
        let target = checks_dir.join(job).join(trial);
        std::fs::create_dir_all(&target)
            .map_err(|e| format!("cannot create {}: {e}", target.display()))?;
        let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
        crate::record::write_atomic(&target.join("support.json"), text.as_bytes())?;
        written += 1;
    }
    Ok(written)
}

fn pct(errors: &Value) -> String {
    format!(
        "{} false accepts of {} unmet · {} false rejects ({} unresolved) of {} met",
        errors["false_accepts"],
        errors["unmet"],
        errors["false_rejects"],
        errors["false_rejects_unresolved"],
        errors["n"].as_u64().unwrap_or(0) - errors["unmet"].as_u64().unwrap_or(0),
    )
}

/// The evaluation as text.
#[must_use]
pub fn lines(evaluation: &Value) -> Vec<String> {
    let c = &evaluation["cutoffs"];
    let mut lines = vec![format!(
        "verify.support · cutoffs fitted on development: supports ≥ {} · contradicts ≥ {} · done ≥ {} · checked in: {}",
        c["supports"],
        c["contradicts"],
        c["done"],
        if evaluation.pointer("/checked_in/matches") == Some(&json!(true)) {
            "matches"
        } else {
            "DIFFERS"
        }
    )];
    for split in ["development", "evaluation"] {
        let s = &evaluation[split];
        lines.push(format!(
            "{split}: {} candidates · {} labeled requirements",
            s["candidates"], s["labeled_requirements"]
        ));
        lines.push("  requirements".to_string());
        for (label, key) in [
            ("support pair, fitted", "support_fitted"),
            ("support pair at 0.5", "support_at_0.5"),
            ("checks alone", "checks_alone"),
            ("support and checks", "support_and_checks"),
        ] {
            lines.push(format!("    {label:<24} {}", pct(&s["requirements"][key])));
        }
        lines.push("  candidates".to_string());
        for (label, key) in [
            ("done at 0.5", "done_at_0.5"),
            ("done at dev cutoff", "done_at_dev_cutoff"),
            ("support pair, fitted", "support_fitted"),
            ("support and checks", "support_and_checks"),
        ] {
            lines.push(format!(
                "    {label:<24} {}",
                pct(&s["candidates_by_rule"][key])
            ));
        }
        let cal = &s["calibration"];
        lines.push(format!(
            "  Brier: supports {} · 1 − contradicts {} · done {}",
            cal.pointer("/supports/score/brier").unwrap_or(&Value::Null),
            cal.pointer("/one_minus_contradicts/score/brier")
                .unwrap_or(&Value::Null),
            cal.pointer("/done/score/brier").unwrap_or(&Value::Null),
        ));
    }
    lines.push(format!(
        "Jev: {} · requests {} · cost {}",
        evaluation.pointer("/jev/mode").unwrap_or(&Value::Null),
        evaluation.pointer("/jev/requests").unwrap_or(&Value::Null),
        evaluation.pointer("/jev/cost_usd").unwrap_or(&Value::Null),
    ));
    lines
}

/// Runs a support command and returns the exit code.
///
/// # Errors
///
/// Returns a message for bad arguments or unreadable inputs.
pub async fn command(args: &[String]) -> Result<i32, String> {
    let Some((verb, rest)) = args.split_first() else {
        return Err(USAGE.to_string());
    };
    let flags = parse(rest)?;
    match verb.as_str() {
        "fixtures" => {
            let out = flags
                .out
                .clone()
                .unwrap_or_else(component::default_fixtures);
            let names = fixtures::build(&flags.components, &flags.traces, &out).await?;
            for name in &names {
                println!("{}", out.join(name).display());
            }
            Ok(0)
        }
        "evaluate" => {
            let choice = JevChoice::parse(&flags.jev, component::cli::live_client)?;
            let dirs = labeled(&flags.fixtures);
            if dirs.is_empty() {
                return Err(format!(
                    "no support-- fixtures under {}",
                    flags.fixtures.display()
                ));
            }
            let recorder = if flags.record {
                component::recorder(
                    flags
                        .out
                        .clone()
                        .or_else(component::default_runs_dir)
                        .as_deref(),
                    "verify.support",
                )?
            } else {
                Recorder::default()
            };
            let (_, evaluation) = evaluate(&dirs, &choice, &recorder, flags.save_jev).await?;
            let mut written = None;
            if flags.write_checks {
                let dir = flags
                    .checks_dir
                    .clone()
                    .or_else(checks::cli::default_dir)
                    .ok_or("no --checks-dir and no HOME")?;
                written = Some((write_checks(&dirs, &dir).await?, dir));
            }
            if flags.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&evaluation).map_err(|e| e.to_string())?
                );
            } else {
                for line in lines(&evaluation) {
                    println!("{line}");
                }
                if let Some((n, dir)) = written {
                    println!("wrote {n} support reports under {}", dir.display());
                }
            }
            Ok(0)
        }
        "run" => {
            let path = flags
                .input
                .clone()
                .ok_or("support run needs --input FILE")?;
            let text = std::fs::read_to_string(&path)
                .map_err(|error| format!("cannot read {}: {error}", path.display()))?;
            let input: Input = serde_json::from_str(&text)
                .map_err(|error| format!("{} is not a checks input: {error}", path.display()))?;
            let mode = match flags.jev.as_str() {
                "live" => crate::component::jev::JevMode::Live(component::cli::live_client()?),
                "off" => crate::component::jev::JevMode::Off,
                other => return Err(format!("support run takes --jev live or off, not {other}")),
            };
            let recorder = Recorder::default();
            let scratch = std::env::temp_dir().join(format!(
                "coder-one-support-run-{}-{}",
                std::process::id(),
                atif::now_ms()
            ));
            let report = checks::check(&input, &recorder, &scratch).await;
            let support =
                super::judge(&input, &report, &mode, &recorder, Params::default(), None).await;
            if flags.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&json!({ "checks": report, "support": support }))
                        .map_err(|e| e.to_string())?
                );
            } else {
                for state in &support.states {
                    println!(
                        "{:<4} {:<13} supports {} · contradicts {} · scenarios {} · {}",
                        state.id,
                        state.state,
                        state
                            .judgment
                            .supports
                            .map_or("—".to_string(), |p| format!("{p:.2}")),
                        state
                            .judgment
                            .contradicts
                            .map_or("—".to_string(), |p| format!("{p:.2}")),
                        state.scenario_state,
                        state.why
                    );
                }
            }
            Ok(0)
        }
        _ => Err(USAGE.to_string()),
    }
}
