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
use lev::calibrate::{DoorIdentity, Map, Observation, Record, admit, score};
use lev::suite::{Item, Suite};

const SUITE: &str = include_str!("../../suites/support-v2.json");

/// Why an item produced no score.
///
/// The distinction matters and collapsing it flatters a door. A guardrail
/// refusal is a real property of the model — it declined to judge — and it
/// belongs in the record with its own outcome. A connection reset is a
/// property of the harness and leaves the record set entirely. Returning
/// `None` for both, which this binary used to do, means a door whose
/// guardrails fire on the hard items quietly scores better.
enum Outcome {
    /// The door answered.
    Scored(Box<Scored>),
    /// The door refused, with its typed reason.
    Refused(String),
    /// The harness failed. Not the door's fault and not its credit.
    Infra(String),
}

/// What one item produced on one door.
struct Scored {
    id: String,
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

async fn run_item(client: &Client, item: &Item) -> Outcome {
    let Ok(state) = serde_json::to_value(&item.state) else {
        return Outcome::Infra("the item's state did not encode".to_string());
    };
    let request = SystemOneRequest::new(state, question_for(item));
    let response = match client.system_one(request).await {
        Ok(response) => response,
        Err(error) => {
            // A 4xx that names a refusal code is the door declining to
            // judge. Anything else is the harness.
            let text = error.to_string();
            return if let jev::Error::Api(api) = &error {
                let body = format!("{:?}", api.body);
                let code = [
                    "guardrail",
                    "uncalibrated",
                    "branch_too_long",
                    "too_many_options",
                    "unsupported_guide",
                    "model_unavailable",
                ]
                .into_iter()
                .find(|code| body.contains(code));
                match code {
                    Some(code) => Outcome::Refused(code.to_string()),
                    None => Outcome::Infra(text),
                }
            } else {
                Outcome::Infra(text)
            };
        }
    };
    let Some(answer) = response.answers.get("q") else {
        return Outcome::Infra("the door answered without the question".to_string());
    };
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
    Outcome::Scored(Box::new(Scored {
        id: item.id.clone(),
        family: item.family.clone(),
        split: item.split.clone(),
        raw_top,
        correct: chosen == item.truth,
        distribution,
    }))
}

/// Reads `GET /v1/models` as raw JSON.
///
/// `jev::ModelCard` carries only name, description, and release date, and
/// drops the adapter and base signature a Lev door already publishes.
async fn door_identity(name: &str, client: &Client) -> DoorIdentity {
    let Ok(response) = client.models().list_raw(jev::ListOptions::default()).await else {
        return DoorIdentity { model: name.to_string(), verified: false, ..Default::default() };
    };
    let Ok(body) = serde_json::from_slice::<serde_json::Value>(&response.bytes) else {
        return DoorIdentity { model: name.to_string(), verified: false, ..Default::default() };
    };
    let first = body.get("models").and_then(|models| models.get(0));
    let text = |key: &str| {
        first
            .and_then(|model| model.get(key))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let signature = text("base_model_signature");
    let adapter = text("adapter");
    DoorIdentity {
        model: if text("name").is_empty() { name.to_string() } else { text("name") },
        verified: !signature.is_empty(),
        base_model_signature: signature,
        adapter,
    }
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
        // Ask the door what it is, so a record is attributable. A door that
        // publishes nothing verifiable is recorded as such rather than
        // credited with an identity it did not give.
        let identity = door_identity(name, client).await;
        if identity.verified {
            println!(
                "Identity: base `{}`{}.\n",
                identity.base_model_signature,
                if identity.adapter.is_empty() {
                    String::new()
                } else {
                    format!(", adapter `{}`", identity.adapter)
                }
            );
        } else {
            println!("Identity: not verifiable for this door.\n");
        }
        let mut scored = Vec::new();
        let mut refusals: BTreeMap<String, usize> = BTreeMap::new();
        let mut infra = 0_usize;
        for item in &suite.items {
            match run_item(client, item).await {
                Outcome::Scored(result) => scored.push(*result),
                Outcome::Refused(code) => {
                    *refusals.entry(code).or_insert(0) += 1;
                }
                Outcome::Infra(reason) => {
                    infra += 1;
                    eprintln!("{}: {reason}", item.id);
                }
            }
        }

        // Say what did not get scored, and why. A shrinking Items column with
        // no explanation is how a door gets credit for declining.
        let refused: usize = refusals.values().sum();
        println!(
            "{} of {} items scored; {refused} refused by the door, {infra} lost to the harness.\n",
            scored.len(),
            suite.items.len()
        );
        if !refusals.is_empty() {
            let detail: Vec<String> =
                refusals.iter().map(|(code, count)| format!("`{code}` x{count}")).collect();
            println!("Door refusals: {}.\n", detail.join(", "));
        }

        // Overall, on the evaluation split, before any map.
        let evaluation: Vec<Observation> = scored
            .iter()
            .filter(|s| s.split == "evaluation")
            .map(|s| Observation::new(s.raw_top, s.correct))
            .collect();

        // Per-item observations, so a change to the gate can be re-scored
        // without asking the models again.
        if let Some(path) = dump.as_deref() {
            let rows: Vec<serde_json::Value> = scored
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "door": name,
                        "id": s.id,
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
                    .map(|s| Observation::new(s.raw_top, s.correct))
                    .collect();
                let map = Map::fit_auto(&fit_on);
                let raw_family: Vec<Observation> = scored
                    .iter()
                    .filter(|s| s.family == family && s.split == "evaluation")
                    .map(|s| Observation::new(s.raw_top, s.correct))
                    .collect();
                let mapped: Vec<Observation> = scored
                    .iter()
                    .filter(|s| s.family == family && s.split == "evaluation")
                    .map(|s| {
                        Observation::new(
                            map.apply_distribution(&s.distribution)
                                .values()
                                .copied()
                                .fold(0.0_f64, f64::max),
                            s.correct,
                        )
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
                        language: "en".to_string(),
                        estimator: "l2".to_string(),
                        samples: 8,
                        suite: suite.name.clone(),
                        suite_digest: suite.digest.clone(),
                        os_build: std::env::var("LEV_OS_BUILD").unwrap_or_else(|_| "25E246".to_string()),
                        door: name.clone(),
                        door_identity: identity.clone(),
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
