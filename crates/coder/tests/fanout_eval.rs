//! The fan-out eval (#10183): a request for the same work on several
//! coding engines, from a terminal on the computer, is a dispatch plan of
//! one run per engine ("1 per agent", "ask all three agents") or per named
//! engine ("have codex and claude both look"), read-only when the work
//! only reads; a single-run ask, or one that names engines only as its
//! subject, stays one run.
//!
//! The set is `crates/coder/fixtures/chat-router/fanout-v1.json`. Each row
//! keeps Jev's recorded response body, and
//! `recorded_fan_out_requests_plan_as_labeled` replays it through the
//! router's reading and policy, so `cargo test -p coder` checks the
//! decision without a network. The ignored `live_jev` asks live Jev:
//!
//! ```sh
//! set -a; . ~/work/.secrets/typesafe.env; set +a   # TYPESAFE_API_KEY for Jev
//! cargo test -p coder --test fanout_eval live_jev -- --ignored --nocapture
//! FANOUT_EVAL_RECORD=1 cargo test -p coder --test fanout_eval live_jev -- --ignored
//! ```

use std::path::PathBuf;

use coder::router::{self, Bank, Context, Facts, Mode, Offer, Situation, Tier};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Row {
    id: String,
    message: String,
    /// `fan_out` or `single`.
    expect: String,
    #[serde(default)]
    runs: Vec<String>,
    read_only: Option<bool>,
    summarize: Option<bool>,
    recorded: Option<Value>,
    /// Why the recorded response misses, when it does: the replay checks
    /// it still misses the same way rather than skipping it.
    known_miss: Option<String>,
}

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/chat-router/fanout-v1.json")
}

fn rows(set: &Value) -> Vec<Row> {
    serde_json::from_value(set["rows"].clone()).expect("rows")
}

fn set() -> Value {
    serde_json::from_str(&std::fs::read_to_string(path()).expect("the fan-out set"))
        .expect("the fan-out set parses")
}

/// A terminal on a computer with Codex, Claude Code, and Grok Build ready.
fn context() -> Context {
    Context::of(&serde_json::json!({
        "surface": "terminal",
        "computer_ready": true,
        "computer": {"place": "here", "engines": [
            {"engine": "codex", "state": "ready"},
            {"engine": "claude", "state": "ready"},
            {"engine": "grok", "state": "ready"},
        ]},
    }))
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

fn decided(response: &jev::SystemOneResponse, admitted: &router::Admitted) -> Tier {
    let bank = Bank::builtin();
    let context = context();
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

/// The plan a tier's offer carries, when it fans out.
fn plan(tier: &Tier) -> Option<router::DispatchPlan> {
    let offer = match tier {
        Tier::CannedFinal { offer, .. } | Tier::CannedStem { offer, .. } => offer.as_ref(),
        _ => None,
    };
    match offer {
        Some(Offer::RunCoder { plan, .. }) if !plan.is_single() => Some(plan.clone()),
        _ => None,
    }
}

/// What is wrong with `tier` for `row`, if anything. The plan's shape and
/// engines are the fan-out's accuracy; read-only and summary are reported
/// beside it.
fn judged(row: &Row, tier: &Tier) -> (Option<String>, Vec<String>) {
    let planned = plan(tier);
    let mut notes = Vec::new();
    let wrong = match (row.expect.as_str(), &planned) {
        ("fan_out", None) => Some(format!("one run, not a plan: {tier:?}")),
        ("fan_out", Some(plan)) => {
            let runs: Vec<&str> = plan.runs.iter().map(|engine| engine.word()).collect();
            if runs != row.runs.iter().map(String::as_str).collect::<Vec<_>>() {
                Some(format!("runs {runs:?}, wanted {:?}", row.runs))
            } else {
                if let Some(read_only) = row.read_only
                    && read_only != plan.read_only
                {
                    notes.push(format!(
                        "read_only {} (labeled {read_only})",
                        plan.read_only
                    ));
                }
                if let Some(summarize) = row.summarize
                    && summarize != plan.summarize
                {
                    notes.push(format!(
                        "summarize {} (labeled {summarize})",
                        plan.summarize
                    ));
                }
                None
            }
        }
        (_, Some(plan)) => Some(format!("planned {plan:?} for a single-run ask")),
        _ => None,
    };
    (wrong, notes)
}

#[test]
fn the_set_has_the_owners_request_and_near_misses() {
    let set = set();
    let rows = rows(&set);
    assert!(rows.iter().any(|row| row.message
        == "do 3 readonly delegations, 1 per agent, explore repo and summarize briefly"));
    assert!(rows.iter().filter(|row| row.expect == "fan_out").count() >= 10);
    assert!(rows.iter().filter(|row| row.expect == "single").count() >= 10);
}

#[test]
fn recorded_fan_out_requests_plan_as_labeled() {
    let set = set();
    let admitted = admitted();
    let mut wrong = Vec::new();
    for row in rows(&set) {
        let body = row
            .recorded
            .as_ref()
            .unwrap_or_else(|| panic!("{} has no recorded response", row.id));
        let tier = decided(&response(body), &admitted);
        match (judged(&row, &tier), &row.known_miss) {
            ((Some(why), _), None) => wrong.push(format!("{} {:?}: {why}", row.id, row.message)),
            ((None, _), Some(_)) => {
                wrong.push(format!("{} now passes; drop its known_miss", row.id))
            }
            _ => {}
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the live judge"]
async fn live_jev() {
    let judge = coder::decision::from_env()
        .expect("a decision profile")
        .expect("TYPESAFE_API_KEY or another profile");
    let record = std::env::var_os("FANOUT_EVAL_RECORD").is_some();
    let root = knowledge::product::repository();
    let corpus = knowledge::product::Corpus::load(&knowledge::product::default_dir(), Some(&root))
        .expect("the product corpus loads");
    let tools = coder::gym_kb::tools(&corpus);
    let admitted = router::Admitted::of(&tools, &[]);
    let bank = Bank::builtin();
    let context = context();
    let facts = context.facts(&base_facts());
    let mut set = set();
    let mut wrong = Vec::new();
    let (mut fan_rows, mut fan_ok, mut single_rows, mut single_ok) = (0, 0, 0, 0);
    for (at, row) in rows(&set).iter().enumerate() {
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
                &[],
                &tools,
                &admitted,
                &[],
            )
            .for_surface(context.surface()),
        )
        .await
        .expect("Jev answered");
        let tier = decided(&asked, &admitted);
        let (why, notes) = judged(row, &tier);
        if row.expect == "fan_out" {
            fan_rows += 1;
            fan_ok += usize::from(why.is_none());
        } else {
            single_rows += 1;
            single_ok += usize::from(why.is_none());
        }
        println!(
            "{} {:?}: {}{}",
            row.id,
            row.message,
            why.as_deref().unwrap_or("ok"),
            if notes.is_empty() {
                String::new()
            } else {
                format!(" ({})", notes.join("; "))
            }
        );
        if let Some(why) = why.filter(|_| row.known_miss.is_none()) {
            wrong.push(format!("{}: {why}", row.id));
        }
        if record {
            let body: Value = serde_json::from_slice(&asked.raw().bytes).expect("a JSON body");
            set["rows"][at]["recorded"] = body;
        }
    }
    println!(
        "fan-out: {fan_ok}/{fan_rows} planned as labeled; single: {single_ok}/{single_rows} stayed one run"
    );
    if record {
        let mut text = serde_json::to_string_pretty(&set).unwrap();
        text.push('\n');
        std::fs::write(path(), text).unwrap();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
