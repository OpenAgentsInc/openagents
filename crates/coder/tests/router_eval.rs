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
//! Each system prints a Markdown report ([`coder::router_eval::Report`]) and
//! writes its JSON to `ROUTER_EVAL_OUT` (default `target/router-eval/`).
//!
//! Systems:
//!
//! - `chat-router-v1` (`live_router`): the router's Jev question set and
//!   policy table, in `Mode::Router`.
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
/// each tier in `mode`, as the worker does for a turn with no computer
/// state and the default seams (so no CLI group is asked).
async fn run_router(name: &str, mode: router::Mode) {
    let judge = coder::decision::from_env()
        .expect("a decision profile")
        .expect("TYPESAFE_API_KEY or another profile");
    let bank = router::Bank::builtin();
    let seams = router::Seams::default();
    let facts = router::worker_facts(
        "google/gemini-3.8-flash",
        Some(DEFAULT_DOOR_URL),
        Some((6, 40)),
        &seams,
    );
    let context = router::Context::default();
    let situation = router::Situation {
        mode,
        context: &context,
        personalize: true,
    };
    let set = Set::fixture();
    let split = split();
    let rows = set.rows(&split);
    let mut readings = Vec::new();
    for row in &rows {
        let started = Instant::now();
        let asked = judge
            .system_one(router::request(
                row.latest(),
                &transcript(row),
                bank,
                &facts,
                &seams.cli.groups(),
            ))
            .await;
        let ms = started.elapsed().as_millis();
        readings.push(match asked {
            Ok(response) => {
                let routing = router::reading(&response, bank, &facts);
                let tier = router::decide(&routing, bank, &facts, &situation);
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
    publish(&Report::of(name, &split, &rows, &readings));
}

#[tokio::test]
#[ignore = "calls the live judge"]
async fn live_router() {
    run_router("chat-router-v1", router::Mode::Router).await;
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
    let split = split();
    let rows = set.rows(&split);
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
