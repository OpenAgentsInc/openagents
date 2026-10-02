//! The dispatch acknowledgement eval: a reply that starts work begins with
//! the verb, in its -ing form, and names no one doing it. The owner
//! (2026-10-02): "dont talk about Coder ... speak like 'Picking up one of
//! the...' just the verb". So "Looking through the latest commits.",
//! "Picking up issue #10178.", never "We'll have Coder ..." or "We'll
//! dispatch ...".
//!
//! The set is `crates/coder/fixtures/chat-router/dispatch-ack-v1.json`.
//! Each row names the bank entry the router picks for its message (and the
//! engine or capability it names) and keeps a continuation the
//! personalization model wrote for it. The fast tests build each
//! acknowledgement as the worker does (the bank's stem, then the
//! continuation through `personalize::check` and `close_stem`, or the
//! stem's generic end) and hold it to the rule, and hold every stem in the
//! bank that starts work to it too. Two ignored tests run it live:
//!
//! ```sh
//! # The personalization model (OPENROUTER_API_KEY or ~/.openagents/openrouter.json):
//! cargo test -p coder --test dispatch_ack_eval live_personalize -- --ignored --nocapture
//! DISPATCH_ACK_RECORD=1 cargo test -p coder --test dispatch_ack_eval live_personalize -- --ignored
//!
//! # The hosted chat, through `openagents chat send --scratch --no-run`:
//! OPENAGENTS_BIN=$CARGO_TARGET_DIR/debug/openagents \
//!   cargo test -p coder --test dispatch_ack_eval live_hosted_chat -- --ignored --nocapture
//! ```

use std::path::PathBuf;
use std::time::Duration;

use coder::router::personalize::{Personalizer, Provider, check, prompt_text};
use coder::router::seams::{Ask, Continuation};
use coder::router::{self, Bank, Facts, RouteId};
use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
struct Set {
    forbidden: Vec<String>,
    rows: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    id: String,
    message: String,
    answer: String,
    #[serde(default)]
    facts: std::collections::BTreeMap<String, String>,
    live: bool,
    recorded: Option<String>,
}

fn path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/chat-router/dispatch-ack-v1.json")
}

fn set() -> Set {
    serde_json::from_str(&std::fs::read_to_string(path()).expect("the dispatch set"))
        .expect("the dispatch set parses")
}

fn facts(row: &Row) -> Facts {
    row.facts
        .iter()
        .fold(Facts::default(), |facts, (key, value)| {
            facts.set(key, value.clone())
        })
}

/// What is wrong with an acknowledgement, if anything: it must begin
/// with a word ending in "ing", be one line ending with a period, and
/// carry none of the `forbidden` phrases.
fn judged(text: &str, forbidden: &[String]) -> Option<String> {
    let first: String = text
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .chars()
        .take_while(|c| c.is_alphabetic())
        .collect();
    if !(first.len() > 4 && first.ends_with("ing") && first.starts_with(char::is_uppercase)) {
        return Some(format!("does not start with an -ing verb: {text:?}"));
    }
    if let Some(phrase) = forbidden
        .iter()
        .find(|phrase| text.to_lowercase().contains(&phrase.to_lowercase()))
    {
        return Some(format!("says {phrase:?}: {text:?}"));
    }
    if text.contains('\n') || !text.ends_with('.') {
        return Some(format!("not one sentence: {text:?}"));
    }
    None
}

/// The acknowledgement the worker shows for `row` when the model wrote
/// `written` (or nothing): the stem and its checked continuation, or its
/// generic end; for an entry with whole text, that text.
fn acknowledgement(row: &Row, written: Option<&str>) -> Result<String, String> {
    let entry = Bank::builtin()
        .entry(&row.answer)
        .ok_or_else(|| format!("no bank entry {}", row.answer))?;
    let facts = facts(row);
    if let Some(text) = entry.render(&facts) {
        return Ok(text);
    }
    let (stem, generic_end) = entry
        .stem(&facts)
        .ok_or_else(|| format!("{} has no stem for {:?}", row.answer, row.facts))?;
    let continuation = written
        .map(|written| check(written, &stem, &row.message, false))
        .transpose()
        .map_err(|refusal| format!("the continuation is refused: {}", refusal.word()))?
        .map(|text| Continuation {
            text,
            model: "recorded".into(),
        });
    let (end, _) = router::close_stem(&generic_end, continuation.as_ref(), &row.message);
    Ok(format!("{stem}{end}"))
}

/// Every bank stem or text that starts work, filled and closed with its
/// generic end, starts with the verb and names no one doing it.
#[test]
fn every_bank_line_that_starts_work_starts_with_the_verb() {
    let forbidden = set().forbidden;
    let bank = Bank::builtin();
    let facts = Facts::default()
        .set("engine.name", "Claude Code")
        .set("capability.name", "Project map");
    let mut wrong = Vec::new();
    let mut seen = 0;
    for entry in &bank.answers {
        let line = if entry.answers(RouteId::WorkDispatch) {
            entry.stem(&facts).map(|(stem, end)| format!("{stem}{end}"))
        } else if entry.id == "cli.run" {
            entry.render(&facts)
        } else {
            None
        };
        let Some(line) = line else { continue };
        seen += 1;
        if let Some(why) = judged(&line, &forbidden) {
            wrong.push(format!("{}: {why}", entry.id));
        }
    }
    assert!(seen >= 6, "{seen} lines");
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn the_set_has_the_owners_phrasings() {
    let set = set();
    assert!(set.rows.len() >= 12);
    for answer in [
        "dispatch.stem",
        "dispatch.explore_stem",
        "dispatch.github_stem",
        "dispatch.capability_stem",
        "dispatch.engine_stem",
        "cli.run",
    ] {
        assert!(set.rows.iter().any(|row| row.answer == answer), "{answer}");
    }
    assert!(
        set.rows
            .iter()
            .any(|row| row.message.starts_with("pick one of the open issues"))
    );
    for phrase in ["We'll have", "We'll dispatch", "Coder will", "have Coder"] {
        assert!(set.forbidden.iter().any(|word| word == phrase), "{phrase}");
    }
}

/// Each row's recorded continuation passes the router's check and makes
/// an acknowledgement that starts with the verb; so does its generic end.
#[test]
fn recorded_acknowledgements_start_with_the_verb() {
    let set = set();
    let mut wrong = Vec::new();
    for row in &set.rows {
        for written in [row.recorded.as_deref(), None] {
            match acknowledgement(row, written) {
                Ok(text) => {
                    if let Some(why) = judged(&text, &set.forbidden) {
                        wrong.push(format!("{}: {why}", row.id));
                    }
                }
                Err(why) => wrong.push(format!("{}: {why}", row.id)),
            }
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

/// Stand-ins for what a model may still write the old way are refused,
/// so the generic end shows instead.
#[test]
fn a_continuation_that_names_the_worker_is_refused() {
    let set = set();
    let row = set
        .rows
        .iter()
        .find(|row| row.answer == "dispatch.stem")
        .expect("a plain dispatch row");
    for written in [
        "having Coder fix the flaky relay test.",
        "the flaky relay test, and Coder will report back.",
        "dispatching Coder to fix the flaky relay test.",
    ] {
        assert!(acknowledgement(row, Some(written)).is_err(), "{written}");
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "calls the live personalization model"]
async fn live_personalize() {
    let record = std::env::var_os("DISPATCH_ACK_RECORD").is_some();
    let provider = Personalizer::named("openrouter")
        .expect("an OpenRouter key")
        .expect("personalization on");
    let mut set: Value = serde_json::from_str(&std::fs::read_to_string(path()).unwrap()).unwrap();
    let rows: Vec<Row> = serde_json::from_value(set["rows"].clone()).unwrap();
    let forbidden = self::set().forbidden;
    let bank = Bank::builtin();
    let mut wrong = Vec::new();
    for (at, row) in rows.iter().enumerate() {
        let entry = bank.entry(&row.answer).unwrap();
        // The engine stem closes with its generic end, and whole text has
        // no continuation: the worker asks the model for neither.
        let Some((stem, _)) = entry
            .stem(&facts(row))
            .filter(|_| row.answer != "dispatch.engine_stem")
        else {
            continue;
        };
        let ask = Ask {
            route: RouteId::WorkDispatch,
            answer: row.answer.clone(),
            stem: stem.clone(),
            message: router::redact(&row.message),
        };
        let mut sink = |_: &str| {};
        let written = tokio::time::timeout(
            Duration::from_secs(20),
            provider.write(&prompt_text(&ask), &mut sink),
        )
        .await
        .expect("in time")
        .expect("the model wrote");
        let shown = acknowledgement(row, Some(&written.text));
        let why = match &shown {
            Ok(text) => judged(text, &forbidden),
            Err(why) => Some(format!("{why}: {:?}", written.text)),
        };
        println!(
            "{} {:?}: {} | {:?}",
            row.id,
            row.message,
            why.as_deref().unwrap_or("ok"),
            shown.as_deref().unwrap_or_default()
        );
        if let Some(why) = why {
            wrong.push(format!("{}: {why}", row.id));
        } else if record {
            set["rows"][at]["recorded"] = Value::from(written.text.trim());
        }
    }
    if record {
        let mut text = serde_json::to_string_pretty(&set).unwrap();
        text.push('\n');
        std::fs::write(path(), text).unwrap();
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
#[ignore = "talks to the hosted chat worker"]
fn live_hosted_chat() {
    let program = std::env::var("OPENAGENTS_BIN").unwrap_or_else(|_| "openagents".into());
    let set = set();
    let mut wrong = Vec::new();
    for row in set.rows.iter().filter(|row| row.live) {
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
        let result = events.iter().find(|event| event["event"] == "result");
        let text = result
            .and_then(|event| event["text"].as_str())
            .unwrap_or_default()
            .trim()
            .to_owned();
        let routed = events.iter().find(|event| event["event"] == "route");
        let route = routed
            .and_then(|event| event["route"].as_str())
            .unwrap_or_default()
            .to_owned();
        let tier = routed
            .and_then(|event| event["tier"].as_str())
            .unwrap_or_default()
            .to_owned();
        // Only a reply that starts work is held to the rule: a dispatch, or
        // a command the terminal runs now (not one offered to confirm). A
        // row the router answered another way is reported, not failed.
        let starts_work =
            route == "work.dispatch" || (tier == "cli" && !text.starts_with("Here's"));
        let why = if starts_work {
            judged(&text, &set.forbidden)
        } else {
            set.forbidden
                .iter()
                .find(|phrase| text.to_lowercase().contains(&phrase.to_lowercase()))
                .map(|phrase| format!("says {phrase:?}: {text:?}"))
        };
        println!(
            "{} {:?}: {} | {route}/{tier} {text:?}",
            row.id,
            row.message,
            why.as_deref()
                .unwrap_or(if starts_work { "ok" } else { "ok (not work)" })
        );
        if let Some(why) = why {
            wrong.push(format!("{}: {why}", row.id));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}
