//! Runs a held-out evidence-selection suite through the real decide
//! request: every item becomes a [`Select::request`] over its
//! candidates, the door answers it once (one retry on transport
//! failure), and [`Select::ranking`] reads the answer back — the same
//! code path a turn takes, so what this measures is what the turn
//! sends.
//!
//! ```text
//! TYPESAFE_API_KEY=... cargo run -p coder --example select_eval -- \
//!     crates/gym/suites/evidence-select-v1.json out.jsonl
//! ```

use std::time::Instant;

use coder::evidence::{Candidate, Candidates, Omitted, Span};
use coder::select::{Select, Verdict};
use jev::Client;
use serde::Deserialize;
use serde_json::{Map, Value, json};

#[derive(Deserialize)]
struct Suite {
    id: String,
    items: Vec<Item>,
}

#[derive(Deserialize)]
struct Item {
    id: String,
    task: String,
    #[serde(default)]
    candidates: Vec<Candidate>,
    #[serde(default)]
    omitted: Vec<Omitted>,
    truth: Truth,
}

#[derive(Deserialize)]
struct Truth {
    chosen: Option<String>,
    span: Option<Span>,
    any_relevant: bool,
    coverage: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let suite_path = std::env::args()
        .nth(1)
        .ok_or("usage: select_eval <suite.json> <out.jsonl>")?;
    let out_path = std::env::args().nth(2).ok_or("missing output path")?;
    let suite: Suite = serde_json::from_str(&std::fs::read_to_string(&suite_path)?)?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run(suite, &out_path))
}

async fn run(suite: Suite, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let mut lines = Vec::new();
    let mut recall = 0usize;
    let mut scored = 0usize;
    let mut abstain_tp = 0usize;
    let mut abstain_fp = 0usize;
    let mut abstain_fn = 0usize;
    let mut confident_errors = 0usize;
    let mut latencies = Vec::new();
    let mut errors = 0usize;
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;
    let mut noul_scored = 0usize;
    let mut noul_agree = 0usize;

    for item in &suite.items {
        let candidates = Candidates {
            candidates: item.candidates.clone(),
            omitted: item.omitted.clone(),
        };
        let request = Select::request(&item.task, &candidates);
        let mut record = Map::new();
        record.insert("item".to_string(), json!(item.id));
        record.insert("suite".to_string(), json!(suite.id));

        let started = Instant::now();
        let mut response = client.system_one(request.clone()).await;
        if response.is_err() {
            tokio::time::sleep(std::time::Duration::from_secs(3)).await;
            response = client.system_one(request).await;
        }
        let elapsed = started.elapsed().as_millis() as u64;
        record.insert("latency_ms".to_string(), json!(elapsed));

        match response {
            Ok(response) => {
                latencies.push(elapsed);
                record.insert("model".to_string(), json!(response.model));
                record.insert(
                    "input_tokens".to_string(),
                    json!(response.usage.input_tokens),
                );
                record.insert(
                    "output_tokens".to_string(),
                    json!(response.usage.output_tokens),
                );
                input_tokens += response.usage.input_tokens.unwrap_or_default();
                output_tokens += response.usage.output_tokens.unwrap_or_default();
                match Select::ranking(&response, &candidates) {
                    Ok(ranking) => {
                        scored += 1;
                        let (chosen_path, chosen_span) = match ranking.verdict {
                            Verdict::Chosen(index) => {
                                let c = &item.candidates[index];
                                (Some(c.path.as_str()), c.span)
                            }
                            Verdict::Abstained => (None, None),
                        };
                        record.insert("choice".to_string(), json!(ranking.choice.choice));
                        record.insert(
                            "probabilities".to_string(),
                            json!(ranking.choice.probabilities),
                        );
                        record.insert("chosen_path".to_string(), json!(chosen_path));
                        record.insert("chosen_span".to_string(), json!(chosen_span));
                        record.insert("unranked".to_string(), json!(ranking.unranked));

                        let correct = chosen_path == item.truth.chosen.as_deref()
                            && (item.truth.chosen.is_none() || chosen_span == item.truth.span);
                        record.insert("correct".to_string(), json!(correct));
                        if correct {
                            recall += 1;
                        }
                        match (chosen_path.is_none(), item.truth.chosen.is_none()) {
                            (true, true) => abstain_tp += 1,
                            (true, false) => abstain_fp += 1,
                            (false, true) => abstain_fn += 1,
                            (false, false) => {}
                        }
                        for (id, noul, truth) in [
                            (
                                "any_relevant",
                                ranking.any_relevant,
                                item.truth.any_relevant,
                            ),
                            ("coverage", ranking.coverage, item.truth.coverage),
                        ] {
                            match noul {
                                Some(probability) => {
                                    let agree = (probability >= 0.5) == truth;
                                    record.insert(format!("{id}_noul"), json!(probability));
                                    record.insert(format!("{id}_agree"), json!(agree));
                                    if agree {
                                        noul_agree += 1;
                                    }
                                    noul_scored += 1;
                                }
                                None => {
                                    record.insert(format!("{id}_noul"), Value::Null);
                                }
                            }
                        }
                        if !correct {
                            let top = ranking
                                .choice
                                .probabilities
                                .get(&ranking.choice.choice)
                                .copied()
                                .unwrap_or_default();
                            record.insert("top_probability".to_string(), json!(top));
                            if top >= 0.8 {
                                confident_errors += 1;
                            }
                        }
                    }
                    Err(fault) => {
                        errors += 1;
                        record.insert("fault".to_string(), json!(format!("{fault}")));
                    }
                }
            }
            Err(error) => {
                errors += 1;
                record.insert("error".to_string(), json!(format!("{error}")));
            }
        }
        lines.push(serde_json::to_string(&Value::Object(record))?);
        eprintln!(
            "{}: {}",
            item.id,
            lines.last().map(String::as_str).unwrap_or_default()
        );
    }

    latencies.sort_unstable();
    let mean = if latencies.is_empty() {
        Value::Null
    } else {
        json!(latencies.iter().sum::<u64>() as f64 / latencies.len() as f64)
    };
    let summary = json!({
        "summary": {
            "suite": suite.id,
            "items": suite.items.len(),
            "scored": scored,
            "errors": errors,
            "recall_at_1": if scored == 0 { Value::Null } else { json!(recall as f64 / scored as f64) },
            "correct": recall,
            "confident_errors": confident_errors,
            "abstention": {
                "true_positive": abstain_tp,
                "false_positive": abstain_fp,
                "false_negative": abstain_fn,
            },
            "latency_ms": {
                "mean": mean,
                "p50": latencies.get(latencies.len() / 2).copied(),
                "max": latencies.last().copied(),
            },
            "noul_agreement": if noul_scored == 0 { Value::Null } else { json!(noul_agree as f64 / noul_scored as f64) },
            "tokens": { "input": input_tokens, "output": output_tokens },
            "cost": "unknown",
        }
    });
    lines.push(serde_json::to_string(&summary)?);
    std::fs::write(out_path, lines.join("\n") + "\n")?;
    eprintln!("wrote {} lines to {out_path}", lines.len());
    Ok(())
}
