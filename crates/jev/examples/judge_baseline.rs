//! Measures one coordination question set on labelled examples.
//!
//! ```text
//! cargo run -p jev --example judge_baseline -- run-progress \
//!     docs/decision-models/measurements/2026-10-18-coordination/run-progress.jsonl \
//!     > results.jsonl
//! ```
//!
//! Each example line is `{"id", "state", "label": {question: answer}}`; a
//! Noul label is `true`/`false` (read at 0.5), a Choice label is the
//! option. Every answer is one line on stdout; agreement, error rates,
//! and latency go to stderr. The Decisions API is
//! `OPENAGENTS_DECISIONS_URL`, else `https://openagents.com/api`, asked
//! for `openagents/decide` (Clef first) with no key.

use std::collections::BTreeMap;
use std::time::Instant;

use jev::decisions::{DecisionAnswer, DecisionsClient};
use jev::question_set::{Judgment, MODEL, coordination};
use serde_json::{Value, json};

#[derive(Default)]
struct Tally {
    asked: u32,
    right: u32,
    false_yes: u32,
    false_no: u32,
    yes: u32,
    no: u32,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let (Some(name), Some(path)) = (args.next(), args.next()) else {
        return Err("usage: judge_baseline SET EXAMPLES.jsonl".into());
    };
    let set = coordination::set(&name).ok_or_else(|| format!("no set named {name}"))?;
    let url = std::env::var("OPENAGENTS_DECISIONS_URL")
        .ok()
        .filter(|url| !url.trim().is_empty())
        .unwrap_or_else(|| jev::decisions::OPENAGENTS.to_owned());
    let client = DecisionsClient::new(url)?;
    let mut tallies: BTreeMap<String, Tally> = BTreeMap::new();
    let mut latencies = Vec::new();
    let mut failed = 0u32;
    for line in std::fs::read_to_string(&path)?.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let example: Value = serde_json::from_str(line)?;
        let id = example["id"].as_str().unwrap_or("?").to_owned();
        let request = set.request(MODEL, &example["state"])?;
        let key = format!("baseline-{}-{id}", set.id);
        let started = Instant::now();
        let response = match client.create_with(&request, Some(&key), &[]).await {
            Ok(response) => response,
            Err(error) => {
                failed += 1;
                println!("{}", json!({"id": id, "error": error.to_string()}));
                continue;
            }
        };
        latencies.push(started.elapsed().as_millis());
        let judgment = Judgment::from_response(set, response)?;
        let mut verdicts = serde_json::Map::new();
        for (question, label) in example["label"].as_object().into_iter().flatten() {
            let tally = tallies.entry(question.clone()).or_default();
            let (right, answer) = match (judgment.answer(question), label) {
                (Some(DecisionAnswer::Predicate { probability, .. }), Value::Bool(expected)) => {
                    let said = *probability >= 0.5;
                    if *expected {
                        tally.yes += 1;
                    } else {
                        tally.no += 1;
                    }
                    if said && !expected {
                        tally.false_yes += 1;
                    }
                    if !said && *expected {
                        tally.false_no += 1;
                    }
                    (said == *expected, json!(probability))
                }
                (Some(DecisionAnswer::Choice { choice, .. }), Value::String(expected)) => {
                    (choice == expected, json!(choice))
                }
                (answer, _) => (false, json!(answer)),
            };
            tally.asked += 1;
            tally.right += u32::from(right);
            verdicts.insert(
                question.clone(),
                json!({"label": label, "answer": answer, "right": right}),
            );
        }
        println!(
            "{}",
            json!({"id": id, "verdicts": verdicts, "receipt": judgment.receipt})
        );
    }
    latencies.sort_unstable();
    let p50 = latencies.get(latencies.len() / 2).copied().unwrap_or(0);
    eprintln!("{} ({} examples could not be judged)", set.id, failed);
    for (question, t) in &tallies {
        eprintln!(
            "  {question}: {}/{} right ({:.0}%); false yes {}/{} labelled no, false no {}/{} labelled yes",
            t.right,
            t.asked,
            100.0 * f64::from(t.right) / f64::from(t.asked.max(1)),
            t.false_yes,
            t.no,
            t.false_no,
            t.yes,
        );
    }
    eprintln!("  latency p50 {p50} ms over {} calls", latencies.len());
    Ok(())
}
