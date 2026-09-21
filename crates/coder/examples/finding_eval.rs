//! Runs a labeled review-finding suite through the real per-finding
//! request: every item becomes the state a `per_finding` decide step
//! builds — the recorded diff, the captured scope, and the anchored
//! findings under their `f`-style names — asked once through
//! `openagents.review-finding.v1` with `Fill::Findings`. The answer's
//! nouls are judged at 0.5 against the suite's truth labels, so what
//! this measures is what a review run sends and reads.
//!
//! ```text
//! TYPESAFE_API_KEY=... cargo run -p coder --example finding_eval -- \
//!     crates/gym/suites/review-finding-v1.json out.jsonl
//! ```

use std::collections::BTreeMap;
use std::path::Path;
use std::time::Instant;

use coder::questions::{Fill, Set};
use jev::{Answer, Client, SystemOneRequest};
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
    diff: Vec<DiffFile>,
    findings: Vec<LabeledFinding>,
    truth: BTreeMap<String, bool>,
}

#[derive(Deserialize)]
struct DiffFile {
    path: String,
    truncated: bool,
    text: String,
}

#[derive(Deserialize)]
struct LabeledFinding {
    id: String,
    path: String,
    #[serde(default)]
    span: Option<Value>,
    severity: String,
    summary: String,
    #[serde(default)]
    evidence: Option<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let suite_path = std::env::args()
        .nth(1)
        .ok_or("usage: finding_eval <suite.json> <out.jsonl>")?;
    let out_path = std::env::args().nth(2).ok_or("missing output path")?;
    let set_path = std::env::args()
        .nth(3)
        .unwrap_or_else(|| "questions/review-finding.json".to_string());
    let suite: Suite = serde_json::from_str(&std::fs::read_to_string(&suite_path)?)?;
    let set = Set::load(Path::new(&set_path))?;

    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?
        .block_on(run(&suite, &set, &out_path))
}

async fn run(suite: &Suite, set: &Set, out_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let client = Client::from_env()?;
    let mut lines = Vec::new();
    let mut judged = 0usize;
    let mut agreed = 0usize;
    let mut confident_errors = 0usize;
    let mut unanswered = 0usize;
    let mut errors = 0usize;
    let mut latencies = Vec::new();
    let mut input_tokens = 0u64;
    let mut output_tokens = 0u64;

    for item in &suite.items {
        let mut record = Map::new();
        record.insert("item".to_string(), json!(item.id));
        record.insert("suite".to_string(), json!(suite.id));

        // The state a `per_finding` step sends: the host's own pins, the
        // captured diff, and the anchored findings under their ids.
        let ids: Vec<String> = item.findings.iter().map(|f| f.id.clone()).collect();
        let mut findings = serde_json::Map::new();
        for finding in &item.findings {
            let mut body = json!({
                "path": finding.path,
                "severity": finding.severity,
                "summary": finding.summary,
            });
            if let Some(span) = &finding.span {
                body["span"] = span.clone();
            }
            if let Some(evidence) = &finding.evidence {
                body["evidence"] = json!(evidence);
            }
            findings.insert(finding.id.clone(), body);
        }
        let state = json!({
            "revision": {
                "base": format!("{}:base", item.id),
                "tip": format!("{}:tip", item.id),
                "input_digest": item.id,
                "diff_digest": item.id,
            },
            "scope": {
                "changed": item.diff.iter().map(|f| f.path.clone()).collect::<Vec<_>>(),
                "excluded": [],
            },
            "diff": item.diff.iter().map(|f| json!({
                "path": f.path,
                "truncated": f.truncated,
                "text": f.text,
            })).collect::<Vec<_>>(),
            "findings": findings,
        });

        let questions = set.build(&Fill::Findings(ids.clone()))?;
        let request = SystemOneRequest::new(state, questions);
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
                input_tokens += response.usage.input_tokens.unwrap_or_default();
                output_tokens += response.usage.output_tokens.unwrap_or_default();
                let mut reads = Map::new();
                for finding in &item.findings {
                    let truth = item.truth.get(&finding.id).copied();
                    match response.answers.get(&finding.id) {
                        Some(Answer::Noul(noul)) => {
                            judged += 1;
                            let genuine = noul.noul >= 0.5;
                            let agree = truth.is_some_and(|t| genuine == t);
                            if agree {
                                agreed += 1;
                            }
                            if truth == Some(false) && noul.noul >= 0.8
                                || truth == Some(true) && noul.noul <= 0.2
                            {
                                confident_errors += 1;
                            }
                            reads.insert(
                                finding.id.clone(),
                                json!({
                                    "noul": noul.noul,
                                    "truth": truth,
                                    "agree": agree,
                                }),
                            );
                        }
                        _ => {
                            unanswered += 1;
                            reads.insert(
                                finding.id.clone(),
                                json!({ "noul": Value::Null, "truth": truth }),
                            );
                        }
                    }
                }
                record.insert("findings".to_string(), Value::Object(reads));
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
    let summary = json!({
        "summary": {
            "suite": suite.id,
            "set": set.id,
            "set_digest": set.digest(),
            "items": suite.items.len(),
            "findings_judged": judged,
            "findings_unanswered": unanswered,
            "agreement": if judged == 0 { Value::Null } else { json!(agreed as f64 / judged as f64) },
            "confident_errors": confident_errors,
            "errors": errors,
            "latency_ms": {
                "mean": if latencies.is_empty() { Value::Null } else {
                    json!(latencies.iter().sum::<u64>() as f64 / latencies.len() as f64)
                },
                "max": latencies.last().copied(),
            },
            "tokens": { "input": input_tokens, "output": output_tokens },
            "cost": "unknown",
        }
    });
    lines.push(serde_json::to_string(&summary)?);
    std::fs::write(out_path, lines.join("\n") + "\n")?;
    eprintln!("wrote {} lines to {out_path}", lines.len());
    Ok(())
}
