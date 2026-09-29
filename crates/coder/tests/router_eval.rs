//! The chat router's live eval over the labeled route set (#9925).
//!
//! Ignored by default: each test asks a live service. Run one with
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a    # TYPESAFE_API_KEY for Jev
//! CODER_AI_GATEWAY_KEY=... \                      # embeddings for the baseline
//! ROUTER_EVAL_SPLIT=held_out \
//!   cargo test -p coder --test router_eval -- --ignored --nocapture --test-threads 1
//! ```
//!
//! `ROUTER_EVAL_SPLIT` is `held_out` (the default), `tune`, or `all`.
//! `ROUTER_EVAL_ROWS` is `all` (the default), `v1` (the rows of
//! `routes-v1.json`, for comparing with the v1 measurements), or `gym` (the
//! rows `routes-v2.json` added).
//! Each system prints a Markdown report ([`coder::router_eval::Report`]) and
//! writes its JSON to `ROUTER_EVAL_OUT` (default `target/router-eval/`).
//!
//! Systems:
//!
//! - `chat-router-v2` (`live_router`): the router's Jev question set and
//!   policy table, in `Mode::Router`, with the `tool` question over the
//!   product corpus's tool catalog as the deployed worker asks it
//!   (`ROUTER_EVAL_GYM=off` leaves it out).
//! - `first-response-legacy`: the same judgment decided in `Mode::Legacy`,
//!   which is what a turn that asks only for `opener` gets: today's
//!   opener-and-prepared-answer path, the baseline the router must beat.
//! - `embedding`: nearest neighbor over the bank's `when` texts and the
//!   route descriptions, with its canned threshold fit on the tune split.
//!
//! Rows run one at a time, so each latency is one request's.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Instant;

use coder::generate::{DEFAULT_DOOR_URL, Message, Role};
use coder::router;
use coder::router_eval::{
    CANNED_TARGET, Reading, Report, Row, Set, fit_threshold, nearest_reading, route_descriptions,
    routed_reading,
};
use knowledge::search::{Embed, cosine};

fn split() -> String {
    std::env::var("ROUTER_EVAL_SPLIT").unwrap_or_else(|_| "held_out".to_string())
}

/// The rows of `split` that `ROUTER_EVAL_ROWS` keeps, and the label the
/// report's split carries.
fn rows<'a>(set: &'a Set, split: &str) -> (Vec<&'a Row>, String) {
    let which = std::env::var("ROUTER_EVAL_ROWS").unwrap_or_else(|_| "all".to_string());
    let rows = set.rows(split);
    let gym = |row: &Row| row.tags.iter().any(|tag| tag == "gym");
    match which.as_str() {
        "v1" => (
            rows.into_iter().filter(|r| !gym(r)).collect(),
            format!("{split}-v1-rows"),
        ),
        "gym" => (
            rows.into_iter().filter(|r| gym(r)).collect(),
            format!("{split}-gym-rows"),
        ),
        _ => (rows, split.to_string()),
    }
}

fn transcript(row: &Row) -> Vec<Message> {
    row.messages
        .iter()
        .map(|m| Message {
            role: if m.role == "assistant" {
                Role::Assistant
            } else {
                Role::User
            },
            text: m.text.clone(),
        })
        .collect()
}

fn publish(report: &Report) {
    println!("{}", report.markdown());
    let dir = std::env::var_os("ROUTER_EVAL_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/router-eval"),
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    let path = dir.join(format!("{}-{}.json", report.system, report.split));
    let _ = std::fs::write(
        &path,
        serde_json::to_string_pretty(&report.json()).unwrap_or_default(),
    );
    println!("wrote {}", path.display());
}

/// Asks the `chat-router-v1` set for every row of the split and decides
/// each tier in `mode`, as the deployed worker does for a turn with no
/// computer state: the CLI route is wired, so its command groups are asked
/// as `cli_group` (`ROUTER_EVAL_CLI=off` leaves them out, as before the
/// worker wired it).
async fn run_router(name: &str, mode: router::Mode) {
    let judge = coder::decision::from_env()
        .expect("a decision profile")
        .expect("TYPESAFE_API_KEY or another profile");
    let bank = router::Bank::builtin();
    let mut seams = router::Seams::default();
    if std::env::var("ROUTER_EVAL_CLI").as_deref() != Ok("off") {
        seams.cli = std::sync::Arc::new(coder::cli_route::CommandRoute::new(
            judge.clone(),
            std::sync::Arc::new(coder::cli_route::NoFill),
        ));
    }
    let facts = router::worker_facts(
        "google/gemini-3.8-flash",
        Some(DEFAULT_DOOR_URL),
        Some((6, 40)),
        &seams,
    );
    let tools = if std::env::var("ROUTER_EVAL_GYM").as_deref() == Ok("off") {
        Vec::new()
    } else {
        let root = knowledge::product::repository();
        let corpus =
            knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
                .expect("the product corpus loads");
        coder::gym_kb::tools(&corpus)
    };
    let context = router::Context::default();
    let situation = router::Situation {
        mode,
        context: &context,
        personalize: true,
        draft: false,
    };
    let set = Set::fixture();
    let (rows, split_label) = rows(&set, &split());
    let mut readings = Vec::new();
    let mut traces = Vec::new();
    for row in &rows {
        let started = Instant::now();
        let asked = judge
            .system_one(router::request(
                row.latest(),
                &transcript(row),
                bank,
                &facts,
                &seams.cli.groups(),
                &tools,
            ))
            .await;
        let ms = started.elapsed().as_millis();
        readings.push(match asked {
            Ok(response) => {
                let routing = router::reading(&response, bank, &facts);
                let tier = router::decide(&routing, bank, &facts, &situation);
                traces.push(trace(row, &routing, &tier));
                routed_reading(&row.id, &routing, &tier, ms)
            }
            Err(error) => Reading {
                id: row.id.clone(),
                error: Some(error.to_string()),
                ms,
                ..Reading::default()
            },
        });
    }
    publish(&Report::of(name, &split_label, &rows, &readings));
    write_traces(name, &split_label, &traces);
}

/// One row's reading in full, for tuning on the tune split: ids and
/// probabilities, and the row's own labels.
fn trace(row: &Row, routing: &router::Routing, tier: &router::Tier) -> serde_json::Value {
    serde_json::json!({
        "id": row.id,
        "message": row.latest(),
        "label": { "route": row.route, "answer": row.answer, "also": row.also,
                   "tier": row.tier, "cli_group": row.cli_group },
        "route": routing.route.word(),
        "route_p": routing.route_p,
        "runner_up": routing.runner_up.map(|(r, p)| (r.word(), p)),
        "answer": routing.answer.as_ref().map(|(e, p)| (e.id.clone(), *p)),
        "needs_specifics": routing.needs_specifics,
        "lane": format!("{:?}", routing.lane),
        "lane_p": routing.lane_p,
        "cli_group": routing.cli_group,
        "tool": routing.tool,
        "risk": routing.risk.word(),
        "risk_p": routing.risk_p,
        "tier": tier.word(),
        "served": tier.answer().map(|e| e.id.clone()),
    })
}

fn write_traces(name: &str, split: &str, traces: &[serde_json::Value]) {
    let dir = std::env::var_os("ROUTER_EVAL_OUT").map_or_else(
        || PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/router-eval"),
        PathBuf::from,
    );
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(
        dir.join(format!("{name}-{split}-rows.json")),
        serde_json::to_string_pretty(traces).unwrap_or_default(),
    );
}

#[tokio::test]
#[ignore = "calls the live judge"]
async fn live_router() {
    run_router("chat-router-v2", router::Mode::Router).await;
}

#[tokio::test]
#[ignore = "calls the live judge"]
async fn live_first_response_baseline() {
    run_router("first-response-legacy", router::Mode::Legacy).await;
}

/// Embeds `texts` in batches.
async fn embed_all<E: Embed>(embedder: &E, texts: Vec<String>) -> Vec<Vec<f32>> {
    let mut out = Vec::with_capacity(texts.len());
    for batch in texts.chunks(96) {
        let (vectors, _) = embedder
            .embed(batch.to_vec())
            .await
            .expect("the embeddings call");
        out.extend(vectors);
    }
    out
}

#[tokio::test]
#[ignore = "calls a live embeddings provider"]
async fn live_embedding_baseline() {
    let embedder = coder::codebase::embedder().expect("an embeddings key");
    let bank: Vec<(String, String, String)> = router::Bank::builtin()
        .answers
        .iter()
        .filter_map(|a| Some((a.id.clone(), a.routes.first()?.clone(), a.when.clone())))
        .collect();
    let routes = route_descriptions();
    let bank_vectors = embed_all(&embedder, bank.iter().map(|b| b.2.clone()).collect()).await;
    let route_vectors = embed_all(
        &embedder,
        routes.iter().map(|(_, d)| (*d).to_string()).collect(),
    )
    .await;
    let set = Set::fixture();
    let mut scored = BTreeMap::new();
    let mut latency = BTreeMap::new();
    for row in &set.rows {
        let started = Instant::now();
        let (vectors, _) = embedder
            .embed(vec![row.latest().to_string()])
            .await
            .expect("the embeddings call");
        latency.insert(row.id.clone(), started.elapsed().as_millis());
        let query = &vectors[0];
        let answers: Vec<(String, String, f64)> = bank
            .iter()
            .zip(&bank_vectors)
            .map(|((id, route, _), v)| (id.clone(), route.clone(), cosine(v, query)))
            .collect();
        let near_routes: Vec<(String, f64)> = routes
            .iter()
            .zip(&route_vectors)
            .map(|((route, _), v)| ((*route).to_string(), cosine(v, query)))
            .collect();
        scored.insert(row.id.clone(), (answers, near_routes));
    }
    let tune = set.rows("tune");
    let threshold = fit_threshold(&tune, &scored, CANNED_TARGET);
    println!("embedding baseline: canned threshold {threshold:.2}, fit on the tune split");
    let (rows, split) = rows(&set, &split());
    let readings: Vec<Reading> = rows
        .iter()
        .map(|row| {
            let (answers, near) = &scored[&row.id];
            let mut reading = nearest_reading(&row.id, answers, near, threshold);
            reading.ms = latency[&row.id];
            reading
        })
        .collect();
    publish(&Report::of(
        &format!("embedding-nn@{threshold:.2}"),
        &split,
        &rows,
        &readings,
    ));
}
