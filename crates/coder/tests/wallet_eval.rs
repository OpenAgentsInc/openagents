//! The wallet eval (#10170): every wallet request means the built-in
//! OpenAgents wallet. The router never asks which wallet, never names
//! another one, and in a terminal it descends the wallet's own commands,
//! so a balance or address request runs `openagents wallet balance` or
//! `openagents wallet address` there, whose answer is one plain sentence.
//! No answer names the x402 node's internals (the set's `technical` words).
//!
//! The set is `crates/coder/fixtures/chat-router/wallet-v1.json`. Each row
//! keeps Jev's recorded response body, and
//! `recorded_wallet_requests_reach_the_built_in_wallet` replays it through
//! the router's own reading and policy, so `cargo test -p coder` checks
//! the decision without a network. Two ignored tests run it live:
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a   # TYPESAFE_API_KEY for Jev
//! cargo test -p coder --test wallet_eval live_jev -- --ignored --nocapture
//! WALLET_EVAL_RECORD=1 cargo test -p coder --test wallet_eval live_jev -- --ignored
//!
//! # The hosted chat, through `openagents chat send --scratch --no-run`:
//! OPENAGENTS_BIN=$CARGO_TARGET_DIR/debug/openagents \
//!   cargo test -p coder --test wallet_eval live_hosted_chat -- --ignored --nocapture
//! ```
//!
//! The first asks live Jev and checks the same decisions (and, with
//! `WALLET_EVAL_RECORD=1`, rewrites the recorded bodies). The second sends
//! each terminal row to the hosted chat worker in a scratch thread with
//! `--no-run`, so nothing runs, and checks the reply: a terminal row is
//! answered with a wallet command, a send row with no command, and no
//! reply names a wallet in the set's `forbidden` list.

use std::path::PathBuf;

use coder::router::policy::{CLARIFY_NOTE, LATER_CLARIFY_NOTE, WALLET_NOTE};
use coder::router::{self, Bank, Context, Facts, Mode, Situation, Tier};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Set {
    forbidden: Vec<String>,
    technical: Vec<String>,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    id: String,
    surface: String,
    message: String,
    expect: String,
    recorded: Option<Value>,
}

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/chat-router/wallet-v1.json")
}

fn set() -> Set {
    serde_json::from_str(&std::fs::read_to_string(path()).expect("the wallet set"))
        .expect("the wallet set parses")
}

fn context(row: &Row) -> Context {
    Context::of(&serde_json::json!({ "surface": row.surface }))
}

fn base_facts() -> Facts {
    router::worker_facts(
        "google/gemini-3.8-flash",
        Some(coder::generate::DEFAULT_DOOR_URL),
        &router::Seams::default(),
    )
}

fn admitted() -> router::Admitted {
    let root = knowledge::product::repository();
    let corpus = knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
        .expect("the product corpus loads");
    router::Admitted::of(&coder::gym_kb::tools(&corpus), &[])
}

fn response(body: &Value) -> jev::SystemOneResponse {
    jev::SystemOneResponse::decode(jev::RawResponse {
        status: 200,
        headers: Default::default(),
        bytes: body.to_string().into_bytes(),
    })
    .expect("a readable recorded response")
}

/// The router's tier for `row` from Jev's `response`, as the worker
/// decides a turn from this surface.
fn decided(row: &Row, response: &jev::SystemOneResponse, admitted: &router::Admitted) -> Tier {
    let bank = Bank::builtin();
    let context = context(row);
    let facts = context.facts(&base_facts());
    let routing = router::reading(response, bank, &facts, admitted);
    router::decide(
        &routing,
        bank,
        &facts,
        &Situation {
            mode: Mode::Router,
            context: &context,
            personalize: true,
            draft: false,
            earlier: false,
            plugin: false,
        },
    )
}

/// What is wrong with `tier` for `row`, if anything.
fn judged(row: &Row, tier: &Tier, forbidden: &[String]) -> Option<String> {
    let canned = |tier: &Tier| match tier {
        Tier::CannedFinal { answer, text, .. } => Some((answer.id.clone(), text.clone())),
        Tier::CannedStem { answer, stem, .. } => Some((answer.id.clone(), stem.clone())),
        _ => None,
    };
    if let Some((_, text)) = canned(tier)
        && let Some(word) = forbidden.iter().find(|word| text.contains(word.as_str()))
    {
        return Some(format!("names {word}"));
    }
    if let Some((_, text)) = canned(tier)
        && let Some(word) = technical(&text, &set().technical)
    {
        return Some(format!("says {word}"));
    }
    let wallet_commands = matches!(tier, Tier::Cli { group, .. } if group == "wallet");
    let ok = match row.expect.as_str() {
        "command" => wallet_commands,
        "send" => canned(tier).is_some_and(|(id, _)| id == "wallet.send"),
        _ => {
            wallet_commands
                || matches!(tier, Tier::Model { note: Some(note), .. } if *note == WALLET_NOTE)
                || canned(tier).is_some_and(|(id, _)| id.starts_with("wallet."))
        }
    };
    let asks = matches!(tier, Tier::Model { note: Some(note), .. }
            if *note == CLARIFY_NOTE || *note == LATER_CLARIFY_NOTE)
        || canned(tier).is_some_and(|(id, _)| id.starts_with("clarify."));
    if asks {
        return Some(format!("asks what they mean: {tier:?}"));
    }
    (!ok).then(|| format!("expected {} but got {tier:?}", row.expect))
}

/// The first `technical` word `text` contains, ignoring case.
fn technical<'a>(text: &str, words: &'a [String]) -> Option<&'a String> {
    let text = text.to_lowercase();
    words
        .iter()
        .find(|word| text.contains(&word.to_lowercase()))
}

/// A wallet answer in a terminal is the plain balance or address: the
/// overview the router runs is `wallet balance`, and no command in the
/// wallet group, nor any prepared wallet answer, says a technical word.
#[test]
fn wallet_answers_are_plain() {
    let set = set();
    assert_eq!(
        coder::router::policy::wallet_overview().argv,
        vec!["wallet".to_owned(), "balance".to_owned()]
    );
    let tree = coder::cli_route::tree::bundled();
    let wallet = tree.group("wallet").expect("the wallet group");
    let mut commands = Vec::new();
    for leaf in wallet.leaves() {
        commands.push(leaf.path.join(" "));
        let text = format!("{} {}", leaf.summary, leaf.usage.join(" "));
        assert!(
            technical(&text, &set.technical).is_none(),
            "{} says {:?}",
            leaf.command(),
            technical(&text, &set.technical)
        );
    }
    for command in ["wallet balance", "wallet address"] {
        assert!(commands.iter().any(|c| c == command), "{command}");
    }
    assert!(
        technical(&wallet.summary, &set.technical).is_none(),
        "{}",
        wallet.summary
    );
    for answer in &Bank::builtin().answers {
        if answer.id.starts_with("wallet.") {
            assert!(
                technical(answer.text.as_deref().unwrap_or_default(), &set.technical).is_none(),
                "{} says {:?}",
                answer.id,
                technical(answer.text.as_deref().unwrap_or_default(), &set.technical)
            );
        }
    }
}

#[test]
fn the_set_has_the_owners_phrasings() {
    let set = set();
    assert!(set.rows.len() >= 10);
    for message in [
        "check my wallet balance",
        "check the wallet shit",
        "balance?",
        "my sats",
    ] {
        assert!(
            set.rows.iter().any(|row| row.message == message),
            "{message}"
        );
    }
    assert!(set.forbidden.iter().any(|word| word == "MetaMask"));
}

#[test]
fn recorded_wallet_requests_reach_the_built_in_wallet() {
    let set = set();
    let admitted = admitted();
    let mut wrong = Vec::new();
    for row in &set.rows {
        let body = row
            .recorded
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no recorded response", row.id));
        let tier = decided(row, &response(body), &admitted);
        if let Some(why) = judged(row, &tier, &set.forbidden) {
            wrong.push(format!(
                "{} {:?} ({}): {why}",
                row.id, row.message, row.surface
            ));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

fn judge() -> jev::Client {
    coder::decision::from_env()
        .expect("a decision profile")
        .expect("TYPESAFE_API_KEY or another profile")
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the live judge"]
async fn live_jev() {
    let judge = judge();
    let record = std::env::var_os("WALLET_EVAL_RECORD").is_some();
    let cli = coder::cli_route::CommandRoute::new(
        judge.clone(),
        std::sync::Arc::new(coder::cli_route::NoFill),
    );
    let groups = router::seams::CliRoute::groups(&cli);
    let root = knowledge::product::repository();
    let corpus = knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
        .expect("the product corpus loads");
    let tools = coder::gym_kb::tools(&corpus);
    let admitted = router::Admitted::of(&tools, &[]);
    let bank = Bank::builtin();
    let mut set: Value = serde_json::from_str(&std::fs::read_to_string(path()).unwrap()).unwrap();
    let rows = set::rows(&set);
    let forbidden = self::set().forbidden;
    let mut wrong = Vec::new();
    for (at, row) in rows.iter().enumerate() {
        let context = context(row);
        let facts = context.facts(&base_facts());
        let decks: &[openagents_deck::DeckEntry] = if row.surface == "desktop" {
            router::decks()
        } else {
            &[]
        };
        let asked = router::ask(
            &judge,
            router::split(
                &row.message,
                &[coder::generate::Message {
                    role: coder::generate::Role::User,
                    text: row.message.clone(),
                }],
                bank,
                &facts,
                &groups,
                &tools,
                &admitted,
                decks,
            ),
        )
        .await
        .expect("Jev answered");
        let tier = decided(row, &asked, &admitted);
        let why = judged(row, &tier, &forbidden);
        println!(
            "{} {:?} ({}): {}",
            row.id,
            row.message,
            row.surface,
            why.as_deref().unwrap_or("ok")
        );
        if let Some(why) = why {
            wrong.push(format!("{}: {why}", row.id));
        }
        if record {
            let body: Value = serde_json::from_slice(&asked.raw().bytes).expect("a JSON body");
            set["rows"][at]["recorded"] = body;
        }
    }
    if record {
        let mut text = serde_json::to_string_pretty(&set).unwrap();
        text.push('\n');
        std::fs::write(path(), text).unwrap();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

mod set {
    use super::Row;
    use serde_json::Value;

    pub fn rows(set: &Value) -> Vec<Row> {
        serde_json::from_value(set["rows"].clone()).expect("rows")
    }
}

#[test]
#[ignore = "talks to the hosted chat worker"]
fn live_hosted_chat() {
    let program = std::env::var("OPENAGENTS_BIN").unwrap_or_else(|_| "openagents".into());
    let set = set();
    let mut wrong = Vec::new();
    for row in set.rows.iter().filter(|row| row.surface == "terminal") {
        let out = std::process::Command::new(&program)
            .args([
                "--json",
                "chat",
                "send",
                "--scratch",
                "--no-run",
                "--timeout",
                "90",
                &row.message,
            ])
            .output()
            .expect("openagents runs");
        let events: Vec<Value> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .filter_map(|line| serde_json::from_str(line).ok())
            .collect();
        let text = events
            .iter()
            .find(|event| event["event"] == "result")
            .and_then(|event| event["text"].as_str())
            .unwrap_or_default()
            .to_owned();
        let command: Option<Vec<String>> = events
            .iter()
            .find(|event| event["event"] == "command")
            .and_then(|event| serde_json::from_value(event["argv"].clone()).ok());
        let mut why = set
            .forbidden
            .iter()
            .find(|word| text.to_lowercase().contains(&word.to_lowercase()))
            .map(|word| format!("names {word}"));
        let wallet = command
            .as_ref()
            .is_some_and(|argv| argv.first().is_some_and(|word| word == "wallet"));
        if let Some(word) = technical(&text, &set.technical) {
            why = why.or(Some(format!("says {word}")));
        }
        match row.expect.as_str() {
            "command" if !wallet => why = why.or(Some(format!("no wallet command: {text:?}"))),
            "send" if command.is_some() => why = why.or(Some("proposed a command".into())),
            _ => {}
        }
        println!(
            "{} {:?}: {} | {text:?} {command:?}",
            row.id,
            row.message,
            why.as_deref().unwrap_or("ok")
        );
        if let Some(why) = why {
            wrong.push(format!("{}: {why}", row.id));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
