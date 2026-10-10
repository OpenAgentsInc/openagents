//! `openagents settings provider-key`: the person's own OpenRouter, Vercel
//! AI Gateway, and TypeSafe keys (BYOK, #10176,
//! `docs/byok/2026-10-02-byok-openrouter.md`).
//!
//! A key is read from a hidden prompt or from stdin, never from argv,
//! tested with the provider's cheapest call, and kept in the login keychain
//! or a `0600` file (`model_access::store`). A refused key is not kept.
//! Nothing here prints a key: output carries the provider, the last four
//! characters, and the key's fingerprint.

use std::io::{BufRead, IsTerminal, Write};

use model_access::check::{self, State};
use model_access::{ApiKey, Keys, Mode, PROVIDERS, Provider, store};
use serde_json::{Value, json};

use crate::Output;
use coder::task::settings::{self, Settings};

pub(crate) const USAGE: &str =
    "usage: openagents settings provider-key COMMAND [PROVIDER] [OPTIONS]
  set PROVIDER [--use] [--skip-test]
                          Read your key from a hidden prompt or stdin, test it,
                          and keep it; --use also runs everything on your keys.
  show [PROVIDER]         Each key you added: the last four characters, the
                          label, what it has spent, and whether it works.
  test [PROVIDER]         Test your keys now, and whether each OpenRouter or
                          Vercel key may call Jev (one small decision).
  connect [openrouter] [--use]
                          Sign in to OpenRouter in your browser and approve a
                          key for OpenAgents; nothing to copy or paste.
  clear PROVIDER          Remove a key; removing your last OpenRouter or Vercel
                          key returns model calls to OpenAgents.
PROVIDER is openrouter, vercel, or typesafe. Keys live in the login keychain, or
else in 0600 files under ~/.openagents. Never type a key on the command line:
it lands in shell history and `ps`. For one command only, set
OPENAGENTS_OPENROUTER_KEY, OPENAGENTS_VERCEL_KEY, or OPENAGENTS_TYPESAFE_KEY.";

fn dir() -> std::path::PathBuf {
    store::openagents_dir().unwrap_or_else(|| std::path::PathBuf::from(".openagents"))
}

/// The person's stored keys on this computer.
pub(crate) fn stored() -> Keys {
    store::load_all(&store::all(&dir()))
}

/// Read a key: a hidden prompt on a terminal, else one line of stdin.
fn read_key(provider: Provider) -> Result<ApiKey, String> {
    let stdin = std::io::stdin();
    let mut line = String::new();
    if stdin.is_terminal() {
        eprint!("Paste your {} key (it won't show): ", provider.name());
        let _ = std::io::stderr().flush();
        let _hidden = NoEcho::enter();
        stdin
            .lock()
            .read_line(&mut line)
            .map_err(|_| "couldn't read the key".to_owned())?;
        eprintln!();
    } else {
        stdin
            .lock()
            .read_line(&mut line)
            .map_err(|_| "couldn't read the key from stdin".to_owned())?;
    }
    let key = ApiKey::new(line.as_str());
    // Scrub the line buffer.
    // SAFETY: zero bytes keep the string valid UTF-8.
    unsafe { line.as_bytes_mut().fill(0) };
    if key.is_empty() {
        return Err("no key was given".into());
    }
    Ok(key)
}

/// This terminal without echo until dropped.
pub(crate) struct NoEcho {
    #[cfg(unix)]
    saved: Option<libc::termios>,
}

impl NoEcho {
    pub(crate) fn enter() -> Self {
        #[cfg(unix)]
        {
            // SAFETY: termios is plain data; tcgetattr fills it and
            // tcsetattr reads it, both on stdin.
            let saved = unsafe {
                let mut term: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(libc::STDIN_FILENO, &raw mut term) != 0 {
                    None
                } else {
                    let saved = term;
                    term.c_lflag &= !libc::ECHO;
                    term.c_lflag |= libc::ECHONL;
                    libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const term);
                    Some(saved)
                }
            };
            NoEcho { saved }
        }
        #[cfg(not(unix))]
        {
            NoEcho {}
        }
    }
}

impl Drop for NoEcho {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(saved) = self.saved {
            // SAFETY: restores the termios tcgetattr returned.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const saved);
            }
        }
    }
}

/// One key's row, never the key.
fn row(provider: Provider, key: &ApiKey, state: Option<&State>, place: &str) -> Value {
    let mut row = json!({
        "provider": provider.word(),
        "name": provider.name(),
        "last_four": key.last_four(),
        "fingerprint": key.fingerprint(),
        "stored_in": place,
        "state": state.map_or("unchecked", State::word),
        "label": provider.name(),
    });
    if let Some(State::Works {
        label: _,
        remaining_usd,
        spent_usd,
    }) = state
    {
        row["remaining_usd"] = json!(remaining_usd);
        row["spent_usd"] = json!(spent_usd);
    }
    if let Some(state) = state {
        row["line"] = json!(state.line(provider));
    }
    row
}

fn render_row(row: &Value) -> String {
    let mut line = format!(
        "{:<18} …{}  {}",
        row["name"].as_str().unwrap_or(""),
        row["last_four"].as_str().unwrap_or(""),
        row["state"].as_str().unwrap_or("")
    );
    if let Some(label) = row["label"].as_str() {
        line.push_str(&format!("  label {label}"));
    }
    if let Some(spent) = row["spent_usd"].as_f64() {
        line.push_str(&format!("  spent ${spent:.2}"));
    }
    if let Some(left) = row["remaining_usd"].as_f64() {
        line.push_str(&format!("  left ${left:.2}"));
    }
    match row["jev"].as_bool() {
        Some(true) => line.push_str("  Jev works"),
        Some(false) => line.push_str("  Jev refused"),
        None => {}
    }
    line
}

/// The tester: the real one, or a fixture for tests
/// (`OPENAGENTS_PROVIDER_CHECK=works|refused|no-credits`).
fn tester() -> Box<dyn check::Send> {
    struct Fixture(Option<u16>, &'static str);
    impl check::Send for Fixture {
        fn send(&self, _: &check::Request, _: &ApiKey) -> (Option<u16>, Vec<u8>) {
            (self.0, self.1.as_bytes().to_vec())
        }
    }
    match std::env::var("OPENAGENTS_PROVIDER_CHECK").as_deref() {
        Ok("works") => Box::new(Fixture(
            Some(200),
            r#"{"data":{"label":"fixture"},"answers":{"ok":{"type":"noul","noul":0.9}}}"#,
        )),
        Ok("refused") => Box::new(Fixture(Some(401), "{}")),
        Ok("no-credits") => Box::new(Fixture(Some(402), "{}")),
        _ => Box::new(check::Http),
    }
}

fn parse_provider(output: &Output, word: Option<&String>) -> Result<Option<Provider>, u8> {
    match word {
        None => Ok(None),
        Some(word) => Provider::parse(word)
            .map(Some)
            .map_err(|message| output.usage("settings provider-key", &message, USAGE)),
    }
}

fn load_settings(output: &Output) -> Result<(std::path::PathBuf, Settings), u8> {
    let file = settings::path();
    match Settings::load(&file) {
        Ok(loaded) => Ok((file, loaded)),
        Err(message) => Err(output.fail("settings", &message)),
    }
}

pub(crate) fn run(output: &Output, words: &[String]) -> u8 {
    let Some((command, rest)) = words.split_first() else {
        return output.usage("settings provider-key", "a command is required", USAGE);
    };
    if matches!(command.as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return 0;
    }
    let flags: Vec<&String> = rest.iter().filter(|w| w.starts_with("--")).collect();
    let positional: Vec<&String> = rest.iter().filter(|w| !w.starts_with("--")).collect();
    for flag in &flags {
        if !matches!(flag.as_str(), "--use" | "--skip-test") {
            return output.usage(
                "settings provider-key",
                &format!("unknown option `{flag}`; a key is never taken from the command line"),
                USAGE,
            );
        }
    }
    if positional.len() > 1 {
        return output.usage(
            "settings provider-key",
            "a key is never taken from the command line; it is read from a hidden prompt or stdin",
            USAGE,
        );
    }
    let provider = match parse_provider(output, positional.first().copied()) {
        Ok(provider) => provider,
        Err(code) => return code,
    };
    let dir = dir();
    match command.as_str() {
        "set" => {
            let Some(provider) = provider else {
                return output.usage("settings provider-key", "set needs a PROVIDER", USAGE);
            };
            let key = match read_key(provider) {
                Ok(key) => key,
                Err(message) => return output.fail("settings provider-key", &message),
            };
            let skip_test = flags.iter().any(|f| *f == "--skip-test");
            let wants_use = flags.iter().any(|f| *f == "--use");
            keep_and_report(output, provider, &key, skip_test, wants_use, "Kept")
        }
        "connect" => {
            if provider.is_some_and(|p| p != Provider::OpenRouter) {
                return output.usage(
                    "settings provider-key",
                    "only OpenRouter connects by signing in; add a Vercel AI Gateway or TypeSafe key with `set`",
                    USAGE,
                );
            }
            let key = match connect(&|url| {
                eprintln!(
                    "Sign in to OpenRouter in your browser and approve a key for OpenAgents.\nIf no browser opens, visit:\n{url}"
                );
                true
            }) {
                Ok(key) => key,
                Err(message) => return output.fail("settings provider-key", &message),
            };
            let skip_test = flags.iter().any(|f| *f == "--skip-test");
            let wants_use = flags.iter().any(|f| *f == "--use");
            keep_and_report(
                output,
                Provider::OpenRouter,
                &key,
                skip_test,
                wants_use,
                "Connected",
            )
        }
        "show" | "test" => {
            let keys = stored();
            let testing = command == "test";
            let stores = store::all(&dir);
            let mut rows = Vec::new();
            for p in PROVIDERS {
                if provider.is_some_and(|only| only != p) {
                    continue;
                }
                let Some(key) = keys.get(p) else { continue };
                let place = stores
                    .iter()
                    .find(|s| s.load(p).ok().flatten().is_some())
                    .map(|s| s.describe(p))
                    .unwrap_or_default();
                // `show` tests the free checks; TypeSafe's costs a decision,
                // so only `test` makes it.
                let state = (testing || p != Provider::TypeSafe)
                    .then(|| check::test(tester().as_ref(), p, key));
                let mut shown = row(p, key, state.as_ref(), &place);
                // `test` also asks whether this key may call Jev at its own
                // provider's door, so a person knows whether decisions run
                // on it under mine.
                if testing && let Some(jev) = check::jev(tester().as_ref(), p, key) {
                    shown["jev"] = json!(jev);
                }
                rows.push(shown);
            }
            let (_, loaded) = match load_settings(output) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            let status = model_access::status_line(loaded.models.payer, &keys, None);
            output.emit(
                &json!({ "payer": loaded.models.payer.as_str(), "keys": rows, "status": status }),
                |value| {
                    let rows = value["keys"].as_array().cloned().unwrap_or_default();
                    let mut lines: Vec<String> = if rows.is_empty() {
                        vec![match provider {
                            Some(only) if testing => format!(
                                "You have no {} key to test. Add one with `openagents settings provider-key set {}`.",
                                only.name(),
                                only.word()
                            ),
                            Some(only) => format!("You have no {} key.", only.name()),
                            None => "No provider keys added.".into(),
                        }]
                    } else {
                        rows.iter().map(render_row).collect()
                    };
                    // Keep guidance after the data rows.
                    for row in &rows {
                        if row["state"] == "no credits"
                            && let Some(line) = row["line"].as_str()
                        {
                            lines.push(line.to_owned());
                        }
                    }
                    lines.push(value["status"].as_str().unwrap_or("").to_owned());
                    lines.join("\n")
                },
            );
            0
        }
        "clear" => {
            let Some(provider) = provider else {
                return output.usage("settings provider-key", "clear needs a PROVIDER", USAGE);
            };
            let had = stored().get(provider).is_some();
            if let Err(message) = store::delete_everywhere(&store::all(&dir), provider) {
                return output.fail("settings provider-key", &message);
            }
            if !had {
                output.emit(
                    &json!({ "provider": provider.word(), "cleared": false }),
                    |_| format!("You have no {} key; nothing was removed.", provider.name()),
                );
                return 0;
            }
            let (file, mut loaded) = match load_settings(output) {
                Ok(pair) => pair,
                Err(code) => return code,
            };
            let keys = stored();
            let reset = loaded.settle_payer(&keys);
            if reset && let Err(message) = loaded.save(&file) {
                return output.fail("settings", &message);
            }
            let status = model_access::status_line(loaded.models.payer, &keys, None);
            output.emit(
                &json!({ "provider": provider.word(), "cleared": true, "payer": loaded.models.payer.as_str(), "status": status }),
                |value| {
                    format!(
                        "Removed your {} key.\n{}",
                        provider.name(),
                        value["status"].as_str().unwrap_or("")
                    )
                },
            );
            0
        }
        other => output.usage(
            "settings provider-key",
            &format!("unknown command `{other}`"),
            USAGE,
        ),
    }
}

/// Test `key`, keep it, and ask the one question (or take `--use`): the
/// tail of `set` and `connect`. `verb` starts the line ("Kept",
/// "Connected").
fn keep_and_report(
    output: &Output,
    provider: Provider,
    key: &ApiKey,
    skip_test: bool,
    wants_use: bool,
    verb: &str,
) -> u8 {
    let dir = dir();
    let state = (!skip_test).then(|| check::test(tester().as_ref(), provider, key));
    if let Some(state) = &state
        && !state.storable()
    {
        return output.fail("settings provider-key", &state.line(provider));
    }
    let target = store::preferred(&dir);
    if let Err(message) = target.save(provider, key) {
        return output.fail("settings provider-key", &message);
    }
    let place = target.describe(provider);
    let mut value = row(provider, key, state.as_ref(), &place);
    // Adding a key never switches the mode by itself; `--use`, or a yes to
    // the one question, does.
    let (file, mut loaded) = match load_settings(output) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let ask = !output.json()
        && std::io::stdin().is_terminal()
        && provider.chat_capable()
        && loaded.models.payer == Mode::Ours;
    if wants_use || (ask && confirm()) {
        if let Err(message) = loaded.set_payer(Mode::Mine, &stored()) {
            return output.fail("settings provider-key", &message);
        }
        if let Err(message) = loaded.save(&file) {
            return output.fail("settings", &message);
        }
    }
    value["payer"] = json!(loaded.models.payer.as_str());
    output.emit(&value, |value| {
        let mut text = format!(
            "{verb} your {} key.\n{}",
            provider.name(),
            render_row(value)
        );
        if let Some(line) = value["line"].as_str() {
            text.push('\n');
            text.push_str(line);
        }
        text.push('\n');
        text.push_str(&model_access::status_line(
            loaded.models.payer,
            &stored(),
            None,
        ));
        text
    });
    0
}

/// Connect OpenRouter: sign in in the browser and get a key back
/// (`model_access::connect`). `announce` gets the sign-in page's link,
/// which carries no secret, and says whether the person can see it; when
/// they can't and no browser opens, the sign-in stops at once. A test sets
/// `OPENAGENTS_PROVIDER_CONNECT_FIXTURE` to the key the sign-in returns,
/// and no browser opens.
///
/// # Errors
/// The sign-in failed, was declined, or took too long: one line.
pub(crate) fn connect(announce: &dyn Fn(&str) -> bool) -> Result<ApiKey, String> {
    if let Ok(key) = std::env::var("OPENAGENTS_PROVIDER_CONNECT_FIXTURE") {
        return Ok(ApiKey::new(key));
    }
    model_access::connect::connect(
        &|url| {
            let shown = announce(url);
            if model_access::connect::open_browser(url) || shown {
                Ok(())
            } else {
                Err("Couldn't open a browser here. Run `openagents settings provider-key connect` to get the sign-in link.".into())
            }
        },
        &|| false,
    )
}

/// Test `key` for `provider` and keep it (a settings screen's paste): a
/// refused key is not kept, and the line says so.
///
/// # Errors
/// The provider refused it, or it cannot be kept.
pub(crate) fn keep(provider: Provider, key: &ApiKey) -> Result<(), String> {
    let state = check::test(tester().as_ref(), provider, key);
    if !state.storable() {
        return Err(state.line(provider));
    }
    store::preferred(&dir()).save(provider, key)?;
    model_access::install(coder::task::settings::access());
    Ok(())
}

/// Remove `provider`'s key from every store.
///
/// # Errors
/// A store refused.
pub(crate) fn forget(provider: Provider) -> Result<(), String> {
    store::delete_everywhere(&store::all(&dir()), provider)
}

/// The one question after a chat-capable key is added.
fn confirm() -> bool {
    eprint!("Use your keys for everything? [y/N] ");
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err() {
        return false;
    }
    matches!(answer.trim(), "y" | "Y" | "yes" | "Yes")
}

/// `openagents settings set models.payer VALUE`: `mine` needs a key that
/// can answer chat.
pub(crate) fn set_payer(loaded: &mut Settings, value: &str) -> Result<(), String> {
    let mode = Mode::parse(value)?;
    loaded.set_payer(mode, &stored())
}
