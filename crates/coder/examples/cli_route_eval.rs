//! Run the CLI route's labeled set against live Jev and the chat model.
//!
//! ```sh
//! set -a; . ~/work/.secrets/coder-chat-worker.env; set +a   # CODER_AI_GATEWAY_KEY
//! cargo run -p coder --example cli_route_eval
//! ```
//!
//! Jev comes from the decision profile (`coder::decision::from_env`, which
//! also reads `~/.openagents/jev.json`); free text is written by the chat
//! worker's door (`ResponsesDoor::from_env`: the Vercel AI Gateway and
//! Gemini 3.8 Flash by default). Each row first asks the level-0 group
//! question, as the router's `cli_group` does, then descends from the
//! group it picked. Prints one line per row and the totals as JSON;
//! `CLI_ROUTE_EVAL_OUT=FILE` also writes them. Read
//! `docs/coder/measurements/2026-09-28-cli-route.md`.

use std::sync::Arc;
use std::time::Instant;

use coder::cli_route::eval::{Totals, labeled, score};
use coder::cli_route::tree::bundled;
use coder::cli_route::{CommandRoute, Fill, ModelFill, NoFill, Outcome, descend};
use coder::generate::ResponsesDoor;
use coder::router::Surface;
use coder::router::seams::CliAsk;
use serde_json::{Value, json};

#[tokio::main]
async fn main() -> Result<(), String> {
    let jev = coder::decision::from_env()?.ok_or("no Jev profile is configured")?;
    let fill: Arc<dyn Fill> = match ResponsesDoor::from_env() {
        Some(door) => Arc::new(ModelFill {
            door,
            service: "Vercel AI Gateway".to_string(),
        }),
        None => {
            eprintln!("no model door (CODER_AI_GATEWAY_KEY); free text will be missing");
            Arc::new(NoFill)
        }
    };
    let set = labeled();
    let tree = bundled();
    let hosts: Vec<_> = set.hosts.iter().map(|host| host.host()).collect();
    let phone = CommandRoute::new(jev.clone(), Arc::clone(&fill));
    let desk = CommandRoute::new(jev, fill).with_hosts(Arc::new(move || hosts.clone()));
    let only: Option<String> = std::env::args().nth(1);
    let mut totals = Totals::default();
    let mut rows: Vec<Value> = Vec::new();
    let mut groups_right = 0usize;
    let mut groups_asked = 0usize;
    let mut millis: Vec<u128> = Vec::new();
    for row in &set.rows {
        if only.as_ref().is_some_and(|id| id != &row.id) {
            continue;
        }
        let surface = row.surface();
        let route = if surface == Surface::Phone {
            &phone
        } else {
            &desk
        };
        let mut ask = CliAsk {
            also: Vec::new(),
            group: String::new(),
            message: row.message.clone(),
            transcript: Vec::new(),
            surface,
        };
        let started = Instant::now();
        let result: Result<(Option<(String, f64)>, Outcome), String> = async {
            let ranked = route.group_ranked(&ask).await.map_err(|e| e.to_string())?;
            let group = ranked.first().cloned();
            let taken = group
                .as_ref()
                .filter(|(name, p)| name != "none" && *p >= descend::GROUP_CONFIDENCE);
            let outcome = match taken {
                Some((name, _)) => {
                    ask.group.clone_from(name);
                    // The router's beam: the next likely groups, as
                    // `Routing::cli_alternatives` reads them.
                    ask.also = ranked
                        .iter()
                        .skip(1)
                        .filter(|(g, p)| g != "none" && *p >= coder::router::judge::CLI_BEAM_FLOOR)
                        .take(coder::router::judge::CLI_BEAM)
                        .map(|(g, _)| g.clone())
                        .collect();
                    route.outcome(&ask).await.map_err(|e| e.to_string())?
                }
                None => Outcome::NoCommand { trail: Vec::new() },
            };
            Ok((group, outcome))
        }
        .await;
        let elapsed = started.elapsed().as_millis();
        millis.push(elapsed);
        match result {
            Ok((group, outcome)) => {
                if !row.command.is_empty() {
                    groups_asked += 1;
                    if group.as_ref().is_some_and(|(g, p)| {
                        g == &row.command[0] && *p >= descend::GROUP_CONFIDENCE
                    }) {
                        groups_right += 1;
                    }
                }
                let scored = score(tree, row, &outcome);
                totals.add(row, Some(&scored));
                let shown = match &outcome {
                    Outcome::Proposal { argv, effect, .. } => {
                        format!("proposal {} [{}]", argv.join(" "), effect.word())
                    }
                    Outcome::Missing { path, what, .. } => {
                        format!("missing {} ({what})", path.join(" "))
                    }
                    Outcome::NotOffered { path, effect, .. } => {
                        format!("not offered {} [{}]", path.join(" "), effect.word())
                    }
                    Outcome::NoCommand { .. } => "no command".to_string(),
                };
                println!(
                    "{} {:<8} {} {:>5}ms  {:<60} -> {}",
                    row.id,
                    row.surface,
                    if scored.correct { "ok " } else { "BAD" },
                    elapsed,
                    row.message.chars().take(60).collect::<String>(),
                    shown
                );
                rows.push(json!({
                    "id": row.id, "correct": scored.correct, "valid": scored.valid,
                    "group": group.map(|(g, p)| json!({"group": g, "p": p})),
                    "outcome": outcome.evidence(), "ms": elapsed,
                }));
            }
            Err(error) => {
                totals.add(row, None);
                println!("{} ERROR {error}", row.id);
                rows.push(json!({ "id": row.id, "error": error }));
            }
        }
    }
    millis.sort_unstable();
    let percentile = |q: f64| {
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let at = ((millis.len().saturating_sub(1)) as f64 * q).round() as usize;
        millis.get(at).copied().unwrap_or(0)
    };
    let mut report = totals.report();
    report["group_correct"] = json!(if groups_asked == 0 {
        0.0
    } else {
        #[allow(clippy::cast_precision_loss)]
        {
            groups_right as f64 / groups_asked as f64
        }
    });
    report["ms_p50"] = json!(percentile(0.5));
    report["ms_p95"] = json!(percentile(0.95));
    report["set"] = json!(set.set);
    report["tree"] = json!(tree.schema);
    println!(
        "{}",
        serde_json::to_string_pretty(&report).unwrap_or_default()
    );
    if let Ok(path) = std::env::var("CLI_ROUTE_EVAL_OUT") {
        let document = json!({ "totals": report, "rows": rows });
        std::fs::write(
            &path,
            serde_json::to_string_pretty(&document).unwrap_or_default(),
        )
        .map_err(|e| format!("{path}: {e}"))?;
    }
    Ok(())
}
