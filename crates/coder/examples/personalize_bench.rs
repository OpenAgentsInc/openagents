//! Measure T1 personalization providers on the ~20-word continuation.
//!
//! Sends each of a fixed set of work requests, under the dispatch stems
//! the chat router design names, to each provider given on the command
//! line (in `CODER_PERSONALIZE` syntax), and prints one line per call and
//! a summary per provider: time to the complete continuation (the provider
//! streams; nothing is shown before the whole continuation validates),
//! time to its first delta, validation pass rate, and the share within
//! `coder::router::seams::PERSONALIZE_BUDGET`. The budget is not applied here, so the
//! slow tail is measured rather than cut off.
//!
//! ```sh
//! cargo run -p coder --example personalize_bench -- \
//!   openrouter:google/gemini-2.5-flash-lite gateway:glm
//! ```
//!
//! `PERSONALIZE_ROUNDS` repeats the whole set (default 1). The OpenRouter
//! key comes from `OPENROUTER_API_KEY` or `~/.openagents/openrouter.json`;
//! the gateway key from `CODER_DOOR_KEY` or `CODER_AI_GATEWAY_KEY`. Read
//! `docs/coder/design/2026-09-28-chat-router.md` for the recorded numbers.

use std::time::{Duration, Instant};

use coder::router::personalize::{Personalizer, Provider, check, prompt_text};
use coder::router::seams::{Ask, PERSONALIZE_BUDGET as BUDGET};
use coder::router::{RouteId, redact};

/// `(stem, message)`: what a user on the `work.dispatch` route writes.
const CASES: &[(&str, &str)] = &[
    (
        "We'll dispatch Coder to",
        "fix the flaky retry test in crates/coder and open a PR",
    ),
    (
        "We'll dispatch Coder to",
        "make the relay's retry timeout configurable",
    ),
    (
        "We'll dispatch Coder to",
        "bump the version to 1.4.2 and tag a release",
    ),
    (
        "We'll dispatch Coder to",
        "can you add dark mode to my settings page",
    ),
    (
        "We'll dispatch Coder to",
        "rename the UserService class to AccountService everywhere",
    ),
    (
        "We'll dispatch Coder to",
        "upgrade tokio to the latest version and fix whatever breaks",
    ),
    (
        "We'll dispatch Coder to",
        "write unit tests for the quota module, it has none",
    ),
    (
        "We'll dispatch Coder to",
        "the build is failing on main with a linker error, please fix it",
    ),
    (
        "We'll dispatch Coder to",
        "add a --json flag to the list command",
    ),
    (
        "We'll dispatch Coder to",
        "refactor my python script to use async requests instead of threads",
    ),
    (
        "We'll dispatch Coder to",
        "delete the dead code in src/legacy and make sure everything still compiles",
    ),
    (
        "We'll dispatch Coder to",
        "set up a GitHub Action... actually no, just add a Makefile with build and test targets",
    ),
    (
        "We'll have Coder look through",
        "look through my rails repo and tell me how auth works",
    ),
    (
        "We'll have Coder look through",
        "what's in the README of OpenAgentsInc/psionic",
    ),
    (
        "We'll have Coder look through",
        "find where the chat worker's quota is implemented",
    ),
    (
        "We'll have Coder look through",
        "explore the codebase and tell me which crates depend on nostr",
    ),
    (
        "We'll have Coder look through",
        "how does our deploy script decide which binary to ship?",
    ),
    (
        "We'll have Coder pick up",
        "work on issue #9920 in OpenAgentsInc/openagents",
    ),
    (
        "We'll have Coder pick up",
        "review PR 412 on my repo and leave comments",
    ),
    (
        "We'll have Coder pick up",
        "take the top open bug in my tracker and fix it",
    ),
];

/// The router's ask for a `work.dispatch` stem, its message redacted.
fn ask(stem: &str, message: &str) -> Ask {
    Ask {
        route: RouteId::WorkDispatch,
        answer: "dispatch.stem".to_string(),
        stem: stem.to_string(),
        message: redact(message),
    }
}

struct Row {
    total_ms: u64,
    first_ms: Option<u64>,
    valid: bool,
}

fn percentile(sorted: &[u64], p: f64) -> u64 {
    if sorted.is_empty() {
        return 0;
    }
    let rank = (p * (sorted.len() - 1) as f64).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

#[tokio::main]
async fn main() -> Result<(), String> {
    let providers: Vec<String> = std::env::args().skip(1).collect();
    if providers.is_empty() {
        return Err("name providers: openrouter:<model> gateway:<lane>".to_string());
    }
    let rounds: usize = std::env::var("PERSONALIZE_ROUNDS")
        .ok()
        .and_then(|n| n.parse().ok())
        .unwrap_or(1);
    let mut summary = Vec::new();
    for asked in &providers {
        let Some(provider) = Personalizer::named(asked)? else {
            continue;
        };
        // One unmeasured call opens the provider's connection, as a
        // running worker's would already be.
        {
            let prompt = prompt_text(&ask(CASES[0].0, CASES[0].1));
            let mut sink = |_: &str| {};
            let _ =
                tokio::time::timeout(Duration::from_secs(10), provider.write(&prompt, &mut sink))
                    .await;
        }
        let mut rows = Vec::new();
        for _ in 0..rounds {
            for (stem, message) in CASES {
                let ask = ask(stem, message);
                let prompt = prompt_text(&ask);
                let started = Instant::now();
                let mut first: Option<u64> = None;
                let mut sink = |_: &str| {
                    if first.is_none() {
                        first = Some(started.elapsed().as_millis() as u64);
                    }
                };
                let written = tokio::time::timeout(
                    Duration::from_secs(10),
                    provider.write(&prompt, &mut sink),
                )
                .await;
                let total_ms = started.elapsed().as_millis() as u64;
                let (shown, valid) = match written {
                    Err(_) => ("(timed out at 10 s)".to_string(), false),
                    Ok(Err(error)) => (format!("(failed: {error})"), false),
                    Ok(Ok(written)) => {
                        match check(&written.text, &ask.stem, &ask.message, written.cut_off) {
                            Ok(text) => (format!("{stem}{text}"), true),
                            Err(invalid) => (
                                format!("(invalid {}: {:?})", invalid.word(), written.text),
                                false,
                            ),
                        }
                    }
                };
                println!(
                    "{asked}\t{total_ms} ms\tfirst {}\t{shown}",
                    first.map_or("-".to_string(), |ms| format!("{ms} ms"))
                );
                rows.push(Row {
                    total_ms,
                    first_ms: first,
                    valid,
                });
            }
        }
        let mut totals: Vec<u64> = rows.iter().map(|row| row.total_ms).collect();
        totals.sort_unstable();
        let mut firsts: Vec<u64> = rows.iter().filter_map(|row| row.first_ms).collect();
        firsts.sort_unstable();
        let valid = rows.iter().filter(|row| row.valid).count();
        let within = rows
            .iter()
            .filter(|row| row.valid && row.total_ms <= BUDGET.as_millis() as u64)
            .count();
        summary.push(format!(
            "| {asked} | {} | {} | {} | {} | {} | {}/{} | {}/{} |",
            percentile(&firsts, 0.5),
            percentile(&totals, 0.5),
            percentile(&totals, 0.9),
            totals.last().copied().unwrap_or(0),
            provider.model(),
            valid,
            rows.len(),
            within,
            rows.len(),
        ));
    }
    println!(
        "\n| Provider | First delta p50 (ms) | Complete p50 (ms) | Complete p90 (ms) | Max (ms) | Model | Valid | Valid within {} ms |",
        BUDGET.as_millis()
    );
    println!("| --- | --- | --- | --- | --- | --- | --- | --- |");
    for line in summary {
        println!("{line}");
    }
    Ok(())
}
