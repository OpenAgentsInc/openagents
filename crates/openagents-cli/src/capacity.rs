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
use microcoder::logins;
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
  record PROVIDER [--login NAME] [--reset TIME] [--rate] [--store DIR]
        Record that PROVIDER refused work for a usage limit (a rate limit
        with --rate) until TIME. TIME is Unix seconds, an ISO 8601 UTC time,
        a duration from now (90m, 5h, 2d), or a clock time as the CLI
        printed it (3pm, \"resets 11:50am (UTC)\"), read as UTC. Without
        --reset the limit holds for 30 minutes. --login names the Claude
        login that hit it; without it, the signed-in login.
  logins [--store DIR]
        Each Claude login, the signed-in one first, and whether it has
        capacity now.
  add-login NAME [--secrets DIR] [--store DIR]
        Add a Claude login named NAME. Reads its token from standard input,
        such as `claude setup-token` printed, and saves it in
        DIR/claude-logins/NAME (DIR is OA_SECRETS_DIR, else ~/work/.secrets).
        New turns move to the next login with capacity when one runs out.
  remove-login NAME [--store DIR]
        Remove a Claude login and its saved token.
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
    Declared::computer("logins", Effect::ReadOnly),
    Declared::computer("add-login", Effect::LocalWrite),
    Declared::computer("remove-login", Effect::LocalWrite),
];

pub fn run(output: &Output, words: &[String]) -> u8 {
    run_with(
        output,
        words,
        coder::task::account::identify,
        now(),
        &mut || {
            let mut text = String::new();
            std::io::Read::read_to_string(
                &mut std::io::Read::take(std::io::stdin(), 64 * 1024),
                &mut text,
            )
            .ok()?;
            Some(text)
        },
    )
}

/// Reads the token text `add-login` saves, from standard input.
type ReadToken<'a> = &'a mut dyn FnMut() -> Option<String>;

fn now() -> u64 {
    coder::task::autostart::unix_now()
}

/// [`run`] with `identify` naming each provider's login and the clock at
/// `now`, so a test reads no real login.
fn run_with(
    output: &Output,
    words: &[String],
    identify: Identify,
    now: u64,
    read_token: ReadToken,
) -> u8 {
    let (command, rest) = match words.split_first() {
        Some((first, rest)) if !first.starts_with('-') => (first.as_str(), rest),
        _ => ("list", words),
    };
    if matches!(command, "help" | "-h" | "--help") {
        println!("{USAGE}");
        return 0;
    }
    let parsed = match Parsed::from(command, rest) {
        Ok(parsed) => parsed,
        Err(message) => return output.usage("capacity", &message, USAGE),
    };
    let Some(store) = parsed.store.clone().or_else(capacity::default_dir) else {
        return output.fail(
            "capacity",
            "Coder couldn't find your user folder; give --store DIR",
        );
    };
    if let Some(result) = login_command(output, command, &parsed, &store, identify, now, read_token)
    {
        return result;
    }
    if parsed.name.is_some() || parsed.secrets.is_some() {
        return output.usage("capacity", "unexpected argument", USAGE);
    }
    if parsed.login.is_some() && parsed.provider != Some(Provider::Claude) {
        return output.usage("capacity", "--login names a claude login", USAGE);
    }
    match command {
        "list" if parsed.provider.is_none() && parsed.reset.is_none() && !parsed.rate => {
            list(output, &store, identify, now)
        }
        "check" if parsed.reset.is_none() && !parsed.rate => match parsed.provider {
            Some(provider) => check(output, &store, provider, parsed.login.as_deref(), identify, now),
            None => output.usage("capacity", "check needs a PROVIDER", USAGE),
        },
        "record" => match parsed.provider {
            Some(provider) => record(output, &store, provider, &parsed, identify, now),
            None => output.usage("capacity", "record needs a PROVIDER", USAGE),
        },
        "list" | "check" => output.usage("capacity", "unexpected option", USAGE),
        other => output.usage(
            "capacity",
            &format!(
                "unknown command `{other}`; use list, check, record, logins, add-login, or remove-login"
            ),
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
    /// `--login NAME`: the Claude login a check or record is about.
    login: Option<String>,
    /// `--secrets DIR`: where `add-login` saves the token.
    secrets: Option<PathBuf>,
    /// The NAME `add-login` and `remove-login` take.
    name: Option<String>,
}

impl Parsed {
    fn from(command: &str, words: &[String]) -> Result<Parsed, String> {
        let mut parsed = Parsed::default();
        let names_a_login = matches!(command, "add-login" | "remove-login");
        let mut words = words.iter();
        while let Some(word) = words.next() {
            match word.as_str() {
                "--login" => {
                    let value = words.next().ok_or("--login needs a NAME")?;
                    parsed.login = Some(value.clone());
                }
                "--secrets" => {
                    let value = words.next().ok_or("--secrets needs a DIR")?;
                    parsed.secrets = Some(PathBuf::from(value));
                }
                name if names_a_login && parsed.name.is_none() && !name.starts_with('-') => {
                    parsed.name = Some(name.to_owned());
                }
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
    let mut row = held(book.blocking(provider, now));
    row["provider"] = json!(provider.as_str());
    if let Some(pick) = book.pick(provider, now)
        && book.logins_of(provider).next().is_some()
    {
        row["login"] = json!(pick.name().unwrap_or(logins::DEFAULT));
    }
    row
}

/// Whether a login has capacity, and the refusal that holds when not.
fn held(refusal: Option<&Refusal>) -> Value {
    match refusal {
        Some(refusal) => json!({
            "capacity": false,
            "kind": refusal.kind,
            "observed_at": refusal.observed_at,
            "resets_at": refusal.resets_at,
            "until": refusal.until,
            "until_utc": capacity::utc(refusal.until),
        }),
        None => json!({"capacity": true}),
    }
}

/// One Claude login's row: `default` is the signed-in login. `None` for a
/// name the book doesn't hold.
fn login_row(book: &Book, name: &str, now: u64) -> Option<Value> {
    let (login, file) = if name == logins::DEFAULT {
        (None, None)
    } else {
        let known = book
            .logins_of(Provider::Claude)
            .find(|login| login.name == name)?;
        (Some(name), Some(&known.token_file))
    };
    let mut row = held(book.blocking_login(Provider::Claude, login, now));
    row["provider"] = json!("claude");
    row["login"] = json!(name);
    if let Some(file) = file {
        row["token_saved"] = json!(file.is_file());
    }
    Some(row)
}

/// `logins`, `add-login`, and `remove-login`, or `None` for another
/// command.
fn login_command(
    output: &Output,
    command: &str,
    parsed: &Parsed,
    store: &Path,
    identify: Identify,
    now: u64,
    read_token: ReadToken,
) -> Option<u8> {
    let simple = parsed.provider.is_none()
        && parsed.reset.is_none()
        && !parsed.rate
        && parsed.login.is_none();
    let result = match command {
        "logins" if simple && parsed.name.is_none() && parsed.secrets.is_none() => {
            list_logins(output, store, identify, now)
        }
        "add-login" if simple => match &parsed.name {
            Some(name) => add_login(
                output,
                store,
                name,
                parsed.secrets.as_deref(),
                now,
                read_token,
            ),
            None => output.usage("capacity", "add-login needs a NAME", USAGE),
        },
        "remove-login" if simple && parsed.secrets.is_none() => match &parsed.name {
            Some(name) => remove_login(output, store, name),
            None => output.usage("capacity", "remove-login needs a NAME", USAGE),
        },
        "logins" | "add-login" | "remove-login" => {
            output.usage("capacity", "unexpected option", USAGE)
        }
        _ => return None,
    };
    Some(result)
}

fn list_logins(output: &Output, store: &Path, identify: Identify, now: u64) -> u8 {
    let book = Book::load_with(store, identify);
    let names: Vec<String> = std::iter::once(logins::DEFAULT.to_owned())
        .chain(
            book.logins_of(Provider::Claude)
                .map(|login| login.name.clone()),
        )
        .collect();
    let rows: Vec<Value> = names
        .iter()
        .filter_map(|name| login_row(&book, name, now))
        .collect();
    let next = book
        .pick(Provider::Claude, now)
        .map(|pick| pick.name().unwrap_or(logins::DEFAULT).to_owned());
    output.emit(
        &json!({"book": store.join(capacity::FILE), "now": now, "next": next, "logins": rows}),
        |value| {
            let rows: Vec<Vec<String>> = value["logins"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|row| {
                    let mut said = words(row);
                    if row["token_saved"] == false {
                        said.push_str("; its token is missing");
                    }
                    vec![row["login"].as_str().unwrap_or_default().to_owned(), said]
                })
                .collect();
            let next = match value["next"].as_str() {
                Some(name) => format!("New turns run on {name}."),
                None => "Every Claude login is out of capacity.".to_owned(),
            };
            format!("{}\n{next}", table(&rows))
        },
    );
    0
}

fn add_login(
    output: &Output,
    store: &Path,
    name: &str,
    secrets: Option<&Path>,
    now: u64,
    read_token: ReadToken,
) -> u8 {
    if let Err(why) = logins::check_name(name) {
        return output.usage("capacity", &why, USAGE);
    }
    let Some(dir) = secrets
        .map(|dir| dir.join("claude-logins"))
        .or_else(logins::token_dir)
    else {
        return output.fail(
            "capacity",
            "Coder couldn't find your user folder; give --secrets DIR",
        );
    };
    let Some(token) = read_token().as_deref().and_then(logins::Token::find) else {
        return output.fail(
            "capacity",
            "no Claude token on standard input; pipe in what `claude setup-token` printed",
        );
    };
    let file = match logins::save_token(&dir, name, &token) {
        Ok(file) => file,
        Err(why) => return output.fail("capacity", &why),
    };
    if let Err(why) = capacity::add_login(store, Provider::Claude, name, &file, now) {
        return output.fail("capacity", &why);
    }
    output.emit(
        &json!({"provider": "claude", "login": name, "token_file": file, "added": true}),
        |_| {
            format!(
                "Added the Claude login {name}. Its token is saved in {}.",
                file.display()
            )
        },
    );
    0
}

fn remove_login(output: &Output, store: &Path, name: &str) -> u8 {
    let removed = match capacity::remove_login(store, Provider::Claude, name) {
        Ok(removed) => removed,
        Err(why) => return output.fail("capacity", &why),
    };
    let _ = std::fs::remove_file(&removed.token_file);
    output.emit(
        &json!({"provider": "claude", "login": name, "removed": true}),
        |_| format!("Removed the Claude login {name} and its saved token."),
    );
    0
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

fn check(
    output: &Output,
    store: &Path,
    provider: Provider,
    login: Option<&str>,
    identify: Identify,
    now: u64,
) -> u8 {
    let book = Book::load_with(store, identify);
    let row = match login {
        Some(name) => match login_row(&book, name, now) {
            Some(row) => row,
            None => {
                return output.fail(
                    "capacity",
                    &format!("there is no claude login named `{name}`"),
                );
            }
        },
        None => row(&book, provider, now),
    };
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
    let login = parsed.login.clone().filter(|name| name != logins::DEFAULT);
    if let Some(name) = &login
        && Book::load_with(store, identify)
            .logins_of(provider)
            .all(|known| &known.name != name)
    {
        return output.fail(
            "capacity",
            &format!("there is no claude login named `{name}`"),
        );
    }
    let refusal = Refusal::new(provider, kind, now, resets_at).on_login(login);
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
            run_with(&output, &words, nobody, now, &mut || None)
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
                run_with(&output, &words, nobody, NOW, &mut || None),
                crate::EXIT_USAGE,
                "{list:?}"
            );
        }
        let mut words = words_of(&["record", "claude", "--reset", "1"]);
        words.extend(words_of(&["--store", store]));
        assert_eq!(
            run_with(&output, &words, nobody, NOW, &mut || None),
            EXIT_FAILURE
        );
        assert!(!dir.path().join(capacity::FILE).exists());
    }

    /// #12211: logins are added from what `claude setup-token` printed,
    /// listed with their capacity, and claude is out only when every login
    /// is. The token is saved in its own file, never in the book.
    #[test]
    fn logins_are_added_listed_and_rotated() {
        let dir = tempfile::tempdir().unwrap();
        let store = dir.path().join("tasks");
        let secrets = dir.path().join("secrets");
        let output = Output::new(true);
        let token = "sk-ant-oat01-fixture-token-0123456789";
        let run = |list: &[&str], now, input: Option<&str>| {
            let mut words = words_of(list);
            words.extend(words_of(&["--store", store.to_str().unwrap()]));
            let input = input.map(str::to_owned);
            run_with(&output, &words, nobody, now, &mut || input.clone())
        };
        let printed = format!("Your OAuth token (valid for 1 year):\n\n{token}\n");
        let add = |name: &str| {
            run(
                &["add-login", name, "--secrets", secrets.to_str().unwrap()],
                NOW,
                Some(&printed),
            )
        };
        assert_eq!(add("alt"), 0);
        assert_eq!(add("spare"), 0);
        assert_eq!(add("default"), crate::EXIT_USAGE);
        assert_eq!(
            run(
                &["add-login", "x", "--secrets", secrets.to_str().unwrap()],
                NOW,
                Some("nothing")
            ),
            EXIT_FAILURE
        );
        let saved = secrets.join("claude-logins/alt");
        assert_eq!(std::fs::read_to_string(&saved).unwrap().trim(), token);
        let book = std::fs::read_to_string(store.join(capacity::FILE)).unwrap();
        assert!(!book.contains(token));
        assert_eq!(run(&["logins"], NOW, None), 0);
        // The signed-in login and alt run out; claude still has capacity.
        assert_eq!(run(&["record", "claude", "--reset", "5h"], NOW, None), 0);
        assert_eq!(
            run(
                &["record", "claude", "--login", "alt", "--reset", "5h"],
                NOW,
                None
            ),
            0
        );
        assert_eq!(run(&["check", "claude"], NOW, None), 0);
        assert_eq!(
            run(&["check", "claude", "--login", "alt"], NOW, None),
            EXIT_FAILURE
        );
        assert_eq!(
            run(&["check", "claude", "--login", "default"], NOW, None),
            EXIT_FAILURE
        );
        assert_eq!(
            run(&["record", "claude", "--login", "nobody"], NOW, None),
            EXIT_FAILURE
        );
        assert_eq!(
            run(&["record", "codex", "--login", "alt"], NOW, None),
            crate::EXIT_USAGE
        );
        let book = Book::load_with(&store, nobody);
        assert_eq!(row(&book, Provider::Claude, NOW)["login"], "spare");
        let listed = login_row(&book, "alt", NOW).unwrap();
        assert_eq!(listed["capacity"], false);
        assert_eq!(listed["token_saved"], true);
        // The last login runs out: now the gate's check refuses.
        assert_eq!(
            run(
                &["record", "claude", "--login", "spare", "--reset", "2h"],
                NOW,
                None
            ),
            0
        );
        assert_eq!(run(&["check", "claude"], NOW, None), EXIT_FAILURE);
        let book = Book::load_with(&store, nobody);
        assert_eq!(row(&book, Provider::Claude, NOW)["until"], NOW + 7_200);
        // Removing a login removes its token file.
        assert_eq!(run(&["remove-login", "spare"], NOW, None), 0);
        assert!(!secrets.join("claude-logins/spare").exists());
        assert_eq!(run(&["remove-login", "spare"], NOW, None), EXIT_FAILURE);
    }
}
