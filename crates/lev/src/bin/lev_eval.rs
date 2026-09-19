//! Scores a door against a labelled suite, and fits a calibration map.
//!
//! It talks to any System One door through the `crates/jev` client, so the
//! same command measures hosted Jev, `kev-serve`, and `lev-serve`.
//!
//! ```text
//! cargo run -p lev --features serve --bin lev-eval -- \
//!     --door lev=http://127.0.0.1:11436 --fit
//! ```

use std::collections::BTreeMap;

use indexmap::IndexMap;
use jev::{Answer, Client, Config, Questions, SystemOneRequest};
use lev::calibrate::{Map, Observation, Record, admit, score};
use lev::suite::{Item, Suite};

const SUITE: &str = include_str!("../../suites/support-v2.json");

/// What one item produced on one door.
struct Scored {
    family: String,
    split: String,
    raw_top: f64,
    correct: bool,
    distribution: IndexMap<String, f64>,
}

fn question_for(item: &Item) -> Questions {
    // The suite stores questions in Lev's own shape; re-encode through the
    // wire so the client sends exactly what a caller would.
    let wire = serde_json::to_value(&item.question).expect("a question serializes");
    let raw = jev::Question::Raw(wire);
    Questions::new().with("q", raw)
}

async fn run_item(client: &Client, item: &Item) -> Option<Scored> {
    let request = SystemOneRequest::new(
        serde_json::to_value(&item.state).ok()?,
        question_for(item),
    );
    let response = match client.system_one(request).await {
        Ok(response) => response,
        Err(error) => {
            eprintln!("{}: {error}", item.id);
            return None;
        }
    };
    let answer = response.answers.get("q")?;
    let (chosen, distribution) = match answer {
        Answer::Noul(noul) => {
            let yes = noul.noul;
            let chosen = if yes >= 0.5 { "yes" } else { "no" };
            let distribution: IndexMap<String, f64> =
                [("no".to_string(), 1.0 - yes), ("yes".to_string(), yes)].into_iter().collect();
            (chosen.to_string(), distribution)
        }
        Answer::Choice(choice) => (choice.choice.clone(), choice.probabilities.clone()),
        Answer::Score(score) => {
            let distribution: IndexMap<String, f64> = score
                .probabilities
                .iter()
                .map(|(level, probability)| (level.to_string(), *probability))
                .collect();
            let chosen = distribution
                .iter()
                .max_by(|a, b| a.1.total_cmp(b.1))
                .map(|(key, _)| key.clone())
                .unwrap_or_default();
            (chosen, distribution)
        }
    };
    let raw_top = distribution.values().copied().fold(0.0_f64, f64::max);
    Some(Scored {
        family: item.family.clone(),
        split: item.split.clone(),
        raw_top,
        correct: chosen == item.truth,
        distribution,
    })
}

fn row(label: &str, metrics: lev::calibrate::Metrics) -> String {
    format!(
        "| {label} | {:.2} | {:.3} | {:.3} | {:.3} | {} | {} |",
        metrics.accuracy,
        metrics.ece,
        metrics.brier,
        metrics.nll,
        metrics.confident_errors,
        metrics.items
    )
}

#[tokio::main]
async fn main() {
    let mut doors: Vec<(String, Client)> = Vec::new();
    let mut fit = false;
    let mut out: Option<String> = None;
    let mut dump: Option<String> = None;
    let mut args = std::env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--fit" => fit = true,
            "--out" => out = args.next(),
            "--dump" => dump = args.next(),
            "--jev" => match Client::from_env() {
                Ok(client) => doors.push(("jev (hosted)".to_string(), client)),
                Err(error) => eprintln!("skipping hosted Jev: {error}"),
            },
            "--door" => {
                let Some(spec) = args.next() else { continue };
                let Some((name, url)) = spec.split_once('=') else { continue };
                let config = Config::new()
                    .api_key("unused-by-a-local-door")
                    .base_url(url.to_string())
                    .default_model(name.to_string());
                match Client::new(config) {
                    Ok(client) => doors.push((name.to_string(), client)),
                    Err(error) => eprintln!("skipping {name}: {error}"),
                }
            }
            other => eprintln!("unknown flag {other}"),
        }
    }
    if doors.is_empty() {
        eprintln!("no doors; pass --jev or --door name=url");
        std::process::exit(2);
    }

    let suite = Suite::load(SUITE).expect("the shipped suite loads");
    println!("# Suite scores: `{}`\n", suite.name);
    println!(
        "{} items, {} in the calibration split and {} in the evaluation split, digest `{}`.\n",
        suite.items.len(),
        suite.split("calibration").count(),
        suite.split("evaluation").count(),
        &suite.digest[..16]
    );
    println!("{}\n", suite.description);

    for (name, client) in &doors {
        println!("## {name}\n");
        let mut scored = Vec::new();
        for item in &suite.items {
            if let Some(result) = run_item(client, item).await {
                scored.push(result);
            }
        }

        // Overall, on the evaluation split, before any map.
        let evaluation: Vec<Observation> = scored
            .iter()
            .filter(|s| s.split == "evaluation")
            .map(|s| Observation { raw: s.raw_top, correct: s.correct })
            .collect();

        // Per-item observations, so a change to the gate can be re-scored
        // without asking the models again.
        if let Some(path) = dump.as_deref() {
            let rows: Vec<serde_json::Value> = scored
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "door": name,
                        "family": s.family,
                        "split": s.split,
                        "raw_top": s.raw_top,
                        "correct": s.correct,
                        "distribution": s.distribution,
                    })
                })
                .collect();
            let file = format!("{path}/{}.json", name.replace([' ', '(', ')'], "_"));
            let _ = std::fs::create_dir_all(path);
            if let Ok(text) = serde_json::to_string_pretty(&rows) {
                let _ = std::fs::write(&file, text + "\n");
            }
        }

        println!("| Set | Accuracy | ECE | Brier | NLL | Confident errors | Items |");
        println!("| --- | --- | --- | --- | --- | --- | --- |");
        println!("{}", row("evaluation, raw", score(&evaluation)));

        if fit {
            // One map per family, fitted on calibration and applied to
            // evaluation. Fitting and scoring never share an item.
            let mut calibrated = Vec::new();
            #[allow(clippy::type_complexity)]
            let mut per_family: BTreeMap<
                String,
                (lev::calibrate::Metrics, lev::calibrate::Metrics, usize, bool, String),
            > = BTreeMap::new();
            for family in suite.families() {
                let fit_on: Vec<Observation> = scored
                    .iter()
                    .filter(|s| s.family == family && s.split == "calibration")
                    .map(|s| Observation { raw: s.raw_top, correct: s.correct })
                    .collect();
                let map = Map::fit_auto(&fit_on);
                let raw_family: Vec<Observation> = scored
                    .iter()
                    .filter(|s| s.family == family && s.split == "evaluation")
                    .map(|s| Observation { raw: s.raw_top, correct: s.correct })
                    .collect();
                let mapped: Vec<Observation> = scored
                    .iter()
                    .filter(|s| s.family == family && s.split == "evaluation")
                    .map(|s| Observation {
                        raw: map
                            .apply_distribution(&s.distribution)
                            .values()
                            .copied()
                            .fold(0.0_f64, f64::max),
                        correct: s.correct,
                    })
                    .collect();
                let raw_metrics = score(&raw_family);
                let calibrated_metrics = score(&mapped);
                let (admitted, verdict) = admit(raw_metrics, calibrated_metrics, map.fitted_on);
                per_family.insert(
                    family.clone(),
                    (raw_metrics, calibrated_metrics, map.fitted_on, admitted, verdict.clone()),
                );
                if let Some(dir) = out.as_deref() {
                    let record = Record {
                        family: family.clone(),
                        estimator: "l2".to_string(),
                        samples: 8,
                        suite: suite.name.clone(),
                        suite_digest: suite.digest.clone(),
                        os_build: std::env::var("LEV_OS_BUILD").unwrap_or_else(|_| "25E246".to_string()),
                        fitted: "2026-09-19".to_string(),
                        map: map.clone(),
                        raw_metrics,
                        calibrated_metrics,
                        admitted,
                        verdict,
                    };
                    // Namespaced by door: three doors scoring the same suite
                    // would otherwise overwrite each other's records, and the
                    // last one to run would silently win.
                    let slug = name.replace([' ', '(', ')'], "_");
                    let _ = std::fs::create_dir_all(format!("{dir}/{slug}"));
                    let path = format!("{dir}/{slug}/{family}.json");
                    match serde_json::to_string_pretty(&record) {
                        Ok(text) => {
                            if let Err(error) = std::fs::write(&path, text + "\n") {
                                eprintln!("could not write {path}: {error}");
                            }
                        }
                        Err(error) => eprintln!("could not encode {family}: {error}"),
                    }
                }
                // Only an admitted map contributes to the calibrated total.
                if admitted {
                    calibrated.extend(mapped);
                } else {
                    calibrated.extend(raw_family);
                }
            }
            println!("{}", row("evaluation, admitted maps only", score(&calibrated)));
            println!(
                "\n| Family | Fitted on | Raw ECE | Mapped ECE | Raw NLL | Mapped NLL | Raw Brier | Mapped Brier | Accuracy | Verdict |"
            );
            println!("| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |");
            for (family, (raw, mapped, fitted_on, _, verdict)) in &per_family {
                println!(
                    "| `{family}` | {fitted_on} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.3} | {:.2} | {verdict} |",
                    raw.ece, mapped.ece, raw.nll, mapped.nll, raw.brier, mapped.brier, raw.accuracy
                );
            }
        }
        println!();
    }
}
