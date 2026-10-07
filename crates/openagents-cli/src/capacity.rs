//! `openagents capacity`: the shared usage-limit book (#10765).
//!
//! Coder, Microcoder, and the auto-start policy record each provider's
//! usage or rate-limit refusal in `capacity.json` in the task store, until
//! its reset. These commands let an orchestrator outside Coder, such as a
//! session that launches Claude Code subagents, read the same book before
//! it starts work and record a limit it hit. `docs/coder/runtime/capacity.md`
//! is the guide.

use std::path::{Path, PathBuf};

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder::task::capacity::{self, Book, Kind, Provider, Refusal};
use serde_json::{Value, json};

use crate::Output;
use crate::out::{EXIT_FAILURE, table};

pub(crate) const USAGE: &str = "usage: openagents capacity COMMAND [OPTIONS]
  list [--store DIR]
        Each provider, whether it has capacity now, and when a recorded
        limit resets. Bare `openagents capacity` is list.
  check PROVIDER [--store DIR]
        Exit 0 when PROVIDER has capacity now, 1 when a recorded limit
        holds; says until when.
  record PROVIDER [--reset TIME] [--rate] [--store DIR]
        Record that PROVIDER refused work for a usage limit (a rate limit
        with --rate) until TIME. TIME is Unix seconds, an ISO 8601 UTC time,
        a duration from now (90m, 5h, 2d), or a clock time as the CLI
        printed it (3pm, \"resets 11:50am (UTC)\"), read as UTC. Without
        --reset the limit holds for 30 minutes.
PROVIDER is codex, claude, vertex, devin, opencode, or grok. The book is
capacity.json in the task store (~/.openagents/tasks, or OPENAGENTS_TASKS);
--store names another. Coder's delegations, Microcoder, and the auto-start
policy read and write the same book, and the policy resumes tasks a limit
stopped once a provider has capacity again.";

/// What each command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("check", Effect::ReadOnly),
    Declared::computer("record", Effect::LocalWrite),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    run_with(output, words, coder::task::account::identify, now())
}

fn now() -> u64 {
    coder::task::autostart::unix_now()
}

/// [`run`] with `identify` naming each provider's login and the clock at
/// `now`, so a test reads no real login.
fn run_with(output: &Output, words: &[String], identify: Identify, now: u64) -> u8 {
    let (command, rest) = match words.split_first() {
        Some((first, rest)) if !first.starts_with('-') => (first.as_str(), rest),
        _ => ("list", words),
    };
    if matches!(command, "help" | "-h" | "--help") {
        println!("{USAGE}");
        return 0;
    }
    let parsed = match Parsed::from(rest) {
        Ok(parsed) => parsed,
        Err(message) => return output.usage("capacity", &message, USAGE),
    };
    let Some(store) = parsed.store.clone().or_else(capacity::default_dir) else {
        return output.fail("capacity", "HOME is unset; give --store DIR");
    };
    match command {
        "list" if parsed.provider.is_none() && parsed.reset.is_none() && !parsed.rate => {
            list(output, &store, identify, now)
        }
        "check" if parsed.reset.is_none() && !parsed.rate => match parsed.provider {
            Some(provider) => check(output, &store, provider, identify, now),
            None => output.usage("capacity", "check needs a PROVIDER", USAGE),
        },
        "record" => match parsed.provider {
            Some(provider) => record(output, &store, provider, &parsed, identify, now),
            None => output.usage("capacity", "record needs a PROVIDER", USAGE),
        },
        "list" | "check" => output.usage("capacity", "unexpected option", USAGE),
        other => output.usage(
            "capacity",
            &format!("unknown command `{other}`; use list, check, or record"),
            USAGE,
        ),
    }
}

type Identify = coder::task::account::Identify;

#[derive(Default)]
struct Parsed {
    provider: Option<Provider>,
    reset: Option<String>,
    rate: bool,
    store: Option<PathBuf>,
}

impl Parsed {
    fn from(words: &[String]) -> Result<Parsed, String> {
        let mut parsed = Parsed::default();
        let mut words = words.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--reset" => {
                    let value = words.next().ok_or("--reset needs a TIME")?;
                    parsed.reset = Some(value.clone());
                }
                "--store" => {
                    let value = words.next().ok_or("--store needs a DIR")?;
                    parsed.store = Some(PathBuf::from(value));
                }
                "--rate" => parsed.rate = true,
                provider if parsed.provider.is_none() && !provider.starts_with('-') => {
                    parsed.provider = Some(Provider::from_config(provider).ok_or_else(|| {
                        format!(
                            "unknown provider `{provider}`; use codex, claude, vertex, devin, opencode, or grok"
                        )
                    })?);
                }
                other => return Err(format!("unexpected argument `{other}`")),
            }
        }
        Ok(parsed)
    }
}

/// One provider's row: whether it has capacity at `now`, and the refusal
/// that holds when one does.
fn row(book: &Book, provider: Provider, now: u64) -> Value {
    match book.blocking(provider, now) {
        Some(refusal) => json!({
            "provider": provider.as_str(),
            "capacity": false,
            "kind": refusal.kind,
            "observed_at": refusal.observed_at,
            "resets_at": refusal.resets_at,
            "until": refusal.until,
            "until_utc": capacity::utc(refusal.until),
        }),
        None => json!({"provider": provider.as_str(), "capacity": true}),
    }
}

fn words(row: &Value) -> String {
    if row["capacity"] == true {
        return "capacity".to_owned();
    }
    let kind = if row["kind"] == "rate_limit" {
        "rate limit"
    } else {
        "usage limit"
    };
    format!(
        "no capacity: {kind} until {}",
        row["until_utc"].as_str().unwrap_or_default()
    )
}

fn list(output: &Output, store: &Path, identify: Identify, now: u64) -> u8 {
    let book = Book::load_with(store, identify);
    let rows: Vec<Value> = Provider::ALL
        .iter()
        .map(|provider| row(&book, *provider, now))
        .collect();
    output.emit(
        &json!({"book": store.join(capacity::FILE), "now": now, "providers": rows}),
        |value| {
            let rows: Vec<Vec<String>> = value["providers"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| {
                    vec![
                        row["provider"].as_str().unwrap_or_default().to_owned(),
                        words(row),
                    ]
                })
                .collect();
            table(&rows)
        },
    );
    0
}

fn check(output: &Output, store: &Path, provider: Provider, identify: Identify, now: u64) -> u8 {
    let row = row(&Book::load_with(store, identify), provider, now);
    let has = row["capacity"] == true;
    output.emit(&row, |row| format!("{provider}: {}", words(row)));
    if has { 0 } else { EXIT_FAILURE }
}

fn record(
    output: &Output,
    store: &Path,
    provider: Provider,
    parsed: &Parsed,
    identify: Identify,
    now: u64,
) -> u8 {
    let resets_at = match parsed.reset.as_deref() {
        Some(text) => match capacity::parse_reset(text, now) {
            Some(at) if at > now => Some(at),
            Some(_) => return output.fail("capacity", "--reset names a time that has passed"),
            None => {
                return output.usage(
                    "capacity",
                    &format!("cannot read the reset time `{text}`"),
                    USAGE,
                );
            }
        },
        None => None,
    };
    let kind = if parsed.rate {
        Kind::RateLimit
    } else {
        Kind::UsageLimit
    };
    let refusal = Refusal::new(provider, kind, now, resets_at);
    if let Err(why) = capacity::record_with(store, refusal, identify) {
        return output.fail("capacity", &why);
    }
    let row = row(&Book::load_with(store, identify), provider, now);
    output.emit(&row, |row| format!("{provider}: recorded; {}", words(row)));
    0
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_790_163_058;

    fn nobody(_: Provider) -> Option<String> {
        None
    }

    fn words_of(list: &[&str]) -> Vec<String> {
        list.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn record_then_check_until_the_reset() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().to_str().unwrap();
        let output = Output::new(true);
        let run = |list: &[&str], now| {
            let mut words = words_of(list);
            words.extend(words_of(&["--store", store]));
            run_with(&output, &words, nobody, now)
        };
        assert_eq!(run(&["check", "claude"], NOW), 0);
        assert_eq!(run(&["record", "claude", "--reset", "1790164200"], NOW), 0);
        assert_eq!(run(&["check", "claude"], NOW), EXIT_FAILURE);
        assert_eq!(run(&["check", "codex"], NOW), 0);
        assert_eq!(run(&["check", "claude"], 1_790_164_200), 0);
        assert_eq!(run(&[], NOW), 0);
        assert_eq!(run(&["list"], NOW), 0);
        let book = Book::load_with(dir.path(), nobody);
        let held = book.blocking(Provider::Claude, NOW).unwrap();
        assert_eq!(held.kind, Kind::UsageLimit);
        assert_eq!(held.resets_at, Some(1_790_164_200));
        // A duration and a rate limit.
        assert_eq!(run(&["record", "codex", "--reset", "5h", "--rate"], NOW), 0);
        let book = Book::load_with(dir.path(), nobody);
        let held = book.blocking(Provider::Codex, NOW).unwrap();
        assert_eq!(held.kind, Kind::RateLimit);
        assert_eq!(held.until, NOW + 18_000);
    }

    #[test]
    fn the_listing_names_every_provider() {
        let dir = tempfile::tempdir().unwrap();
        capacity::record_with(
            dir.path(),
            Refusal::new(Provider::Claude, Kind::UsageLimit, NOW, Some(NOW + 60)),
            nobody,
        )
        .unwrap();
        let book = Book::load_with(dir.path(), nobody);
        let claude = row(&book, Provider::Claude, NOW);
        assert_eq!(claude["capacity"], false);
        assert_eq!(claude["until"], NOW + 60);
        assert!(words(&claude).starts_with("no capacity: usage limit until"));
        assert_eq!(row(&book, Provider::Grok, NOW)["capacity"], true);
    }

    #[test]
    fn mistakes_are_usage_errors() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().to_str().unwrap();
        let output = Output::new(true);
        for list in [
            &["check"][..],
            &["record", "gemini"],
            &["record", "claude", "--reset", "soon"],
            &["frobnicate"],
            &["check", "claude", "--rate"],
        ] {
            let mut words = words_of(list);
            words.extend(words_of(&["--store", store]));
            assert_eq!(
                run_with(&output, &words, nobody, NOW),
                crate::EXIT_USAGE,
                "{list:?}"
            );
        }
        let mut words = words_of(&["record", "claude", "--reset", "1"]);
        words.extend(words_of(&["--store", store]));
        assert_eq!(run_with(&output, &words, nobody, NOW), EXIT_FAILURE);
        assert!(!dir.path().join(capacity::FILE).exists());
    }
}
