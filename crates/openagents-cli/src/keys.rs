//! `openagents keys` and `openagents login --key` (#11305): the account's
//! sign-in keys (`oa_sk_…`, an agent acting as you) and API keys (`oak_…`,
//! model calls), from the command line, through the website's `/v1/keys`
//! (the routes Settings uses too).
//!
//! Calls run under the account's own sign-in, as `openagents mac` does:
//! `OPENAGENTS_TOKEN` (a sign-in key; `OPENAGENTS_APP_TOKEN` also works)
//! with `OPENAGENTS_ORIGIN`, else `--account-dir DIR`, else Coder's
//! (`coder login`). No secret is ever printed except a new key's, once.

use std::io::{BufRead, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Duration;

use coder::cli_route::tree::{Declared, Effect};
use openagents_login::Saved;
use serde_json::{Value, json};

use crate::{Args, Output};

pub(crate) const USAGE: &str =
    "usage: openagents keys COMMAND [--kind signin|api] [--account-dir DIR]
  list        Your keys: name, id, made, last used, and expiry. Never a secret.
  create --name NAME [--expires never|30d|90d]
              Make a key and print it once, with the line to use it.
  show KEY    One key, by id, the start of its id, or its name.
  revoke KEY  Revoke a key, by id, the start of its id, or its name.
  rotate KEY  Make a new key with the same name and expiry, print it, then
              revoke the old one.
--kind signin (the default) is a sign-in key: an agent acting as you, as
coder login does. --kind api is an API key for calling models
(https://api.openagents.com/v1); API keys don't expire. Signs in as
openagents mac does: OPENAGENTS_TOKEN, --account-dir, or coder login.";

pub(crate) const LOGIN_USAGE: &str = "usage: openagents login --key [--account-dir DIR]
Sign in with a sign-in key from Settings (Sign-in keys for agents), read from
standard input or pasted at the prompt, never typed on the command line, and
save it where Coder keeps its sign-in (or in DIR). To skip saving, set it in
the environment instead: OPENAGENTS_TOKEN=oa_sk_... openagents ...";

/// What each command does, for the chat router's command tree.
pub(crate) const EFFECTS: &[Declared] = &[
    Declared::computer("list", Effect::ReadOnly),
    Declared::computer("create", Effect::Secret),
    Declared::computer("show", Effect::ReadOnly),
    Declared::computer("revoke", Effect::Grants),
    Declared::computer("rotate", Effect::Secret),
];

pub(crate) const LOGIN_EFFECTS: &[Declared] = &[Declared::computer("", Effect::LocalWrite)];

const DAY: u64 = 86_400;

pub fn run(output: &Output, words: &[String]) -> u8 {
    if words.is_empty() || matches!(words[0].as_str(), "--help" | "-h" | "help") {
        println!("{USAGE}");
        return if words.is_empty() {
            crate::EXIT_USAGE
        } else {
            0
        };
    }
    let args = match Args::parse(words, &[]) {
        Ok(args) => args,
        Err(message) => return output.usage("keys", &message, USAGE),
    };
    let positional = args.positional();
    let command = positional.first().map(String::as_str).unwrap_or_default();
    let known: &[&str] = match command {
        "list" | "show" | "revoke" | "rotate" => &["kind", "account-dir"],
        "create" => &["name", "expires", "kind", "account-dir"],
        _ => return output.usage("keys", &format!("unknown command `{command}`"), USAGE),
    };
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !known.contains(name))
    {
        return output.usage("keys", &format!("unknown option `--{name}`"), USAGE);
    }
    let kind = match args.option("kind").unwrap_or("signin") {
        "signin" | "sign-in" => "signin",
        "api" => "api",
        other => {
            return output.usage(
                "keys",
                &format!("--kind is signin or api, not `{other}`"),
                USAGE,
            );
        }
    };
    let target = positional.get(1).map(String::as_str);
    if matches!(command, "show" | "revoke" | "rotate") && target.is_none() {
        return output.usage(
            "keys",
            &format!("{command} takes a key's id or name"),
            USAGE,
        );
    }
    let site = match Site::open(args.option("account-dir")) {
        Ok(site) => site,
        Err(message) => return output.fail("keys", &message),
    };
    let result = match command {
        "list" => site.list(kind).map(|listed| {
            output.emit(&listed, |value| list_text(value, kind));
        }),
        "create" => {
            let Some(name) = args.option("name") else {
                return output.usage("keys", "create takes --name NAME", USAGE);
            };
            let days = match expires_days(args.option("expires").unwrap_or("never")) {
                Ok(days) => days,
                Err(message) => return output.usage("keys", &message, USAGE),
            };
            if kind == "api" && days.is_some() {
                return output.usage("keys", "API keys don't expire; leave out --expires", USAGE);
            }
            site.create(kind, name, days).map(|made| {
                output.emit(&with_use(&site, made), |value| made_text(value, "Made"));
            })
        }
        "show" => site.find(kind, target.unwrap_or_default()).map(|(_, key)| {
            output.emit(&key, |value| key_line(value, kind));
        }),
        "revoke" => site
            .find(kind, target.unwrap_or_default())
            .and_then(|(_, key)| {
                let id = key["id"].as_str().unwrap_or_default().to_owned();
                site.revoke(kind, &id).map(|answer| (answer, key))
            })
            .map(|(answer, key)| {
                output.emit(&answer, |_| {
                    format!(
                        "Revoked {} ({}).",
                        quoted(&key),
                        short(key["id"].as_str().unwrap_or_default())
                    )
                });
            }),
        "rotate" => site.rotate(kind, target.unwrap_or_default()).map(|made| {
            output.emit(&made, |value| made_text(value, "Rotated: made"));
        }),
        _ => unreachable!("checked above"),
    };
    match result {
        Ok(()) => 0,
        Err(message) => output.fail("keys", &message),
    }
}

/// `openagents login --key [--account-dir DIR]`.
pub fn login(output: &Output, words: &[String]) -> u8 {
    if words
        .first()
        .is_some_and(|w| matches!(w.as_str(), "--help" | "-h" | "help"))
    {
        println!("{LOGIN_USAGE}");
        return 0;
    }
    if words.iter().any(|w| w.starts_with("--key=")) {
        return output.refuse(
            "login",
            "Don't put the key on the command line. Run openagents login --key and paste it, or pipe it in: openagents login --key < key.txt",
        );
    }
    let args = match Args::parse(words, &["key"]) {
        Ok(args) => args,
        Err(message) => return output.usage("login", &message, LOGIN_USAGE),
    };
    if !args.switch("key") || !args.positional().is_empty() {
        return output.usage(
            "login",
            "sign in with --key, or with coder login for a code to approve",
            LOGIN_USAGE,
        );
    }
    if let Some(name) = args
        .option_names()
        .into_iter()
        .find(|name| !["key", "account-dir"].contains(name))
    {
        return output.usage("login", &format!("unknown option `--{name}`"), LOGIN_USAGE);
    }
    let dir = account_dir(args.option("account-dir"));
    let key = match read_key_input() {
        Ok(key) => key,
        Err(message) => return output.fail("login", &message),
    };
    let origin = openagents_login::origin_from(|name| std::env::var(name).ok());
    let signed = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Couldn't start sign-in.".to_owned())
        .and_then(|runtime| {
            runtime
                .block_on(openagents_login::sign_in_with_key(&origin, &key))
                .map_err(|error| error.to_string())
        });
    let saved = match signed {
        Ok(saved) => saved,
        Err(message) => return output.fail("login", &message),
    };
    if let Err(message) = saved.store(&dir) {
        return output.fail("login", &message);
    }
    output.emit(
        &json!({"account": {"id": saved.account, "label": saved.label}, "origin": saved.origin, "saved": dir.join(openagents_login::FILE).display().to_string()}),
        |value| {
            format!(
                "Signed in to OpenAgents as {} with a sign-in key.",
                value["account"]["label"].as_str().unwrap_or("your account")
            )
        },
    );
    0
}

/// The key from piped standard input, or a hidden prompt on a terminal.
fn read_key_input() -> Result<String, String> {
    let stdin = std::io::stdin();
    if !stdin.is_terminal() {
        return openagents_login::read_key(&mut stdin.lock());
    }
    eprint!("Paste your sign-in key (from Settings, Sign-in keys for agents): ");
    let _ = std::io::stderr().flush();
    let stty = |arg: &str| {
        std::process::Command::new("stty")
            .arg(arg)
            .stdin(std::process::Stdio::inherit())
            .status()
            .is_ok_and(|status| status.success())
    };
    let hidden = cfg!(unix) && stty("-echo");
    let mut line = String::new();
    let read = stdin.lock().read_line(&mut line);
    if hidden {
        stty("echo");
        eprintln!();
    }
    read.map_err(|_| "Couldn't read the key.".to_owned())?;
    openagents_login::read_key(&mut std::io::Cursor::new(line))
}

/// Coder's sign-in folder, or the one named.
fn account_dir(dir: Option<&str>) -> PathBuf {
    dir.map_or_else(
        || {
            std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_default()
                .join(".openagents")
                .join("coder")
        },
        PathBuf::from,
    )
}

/// The folder a sign-in is read from: `account_dir(dir)`, else the
/// `coder-new` folder when Coder's data hasn't moved to `coder` here
/// (#11361) and it alone still holds one.
fn sign_in_dir(dir: Option<&str>) -> PathBuf {
    if dir.is_some() {
        return account_dir(dir);
    }
    let dir = account_dir(None);
    let older = dir.with_file_name("coder-new");
    if dir.join(openagents_login::FILE).is_file() || !older.join(openagents_login::FILE).is_file() {
        dir
    } else {
        older
    }
}

/// `never`, `30d`, `90d` (or a bare number of days, 1 to 365).
fn expires_days(text: &str) -> Result<Option<u64>, String> {
    if text == "never" {
        return Ok(None);
    }
    text.trim_end_matches('d')
        .parse::<u64>()
        .ok()
        .filter(|days| (1..=365).contains(days))
        .map(Some)
        .ok_or_else(|| format!("--expires is never, 30d, or 90d, not `{text}`"))
}

/// The website, as the signed-in account (also `openagents projects`).
pub(crate) struct Site {
    pub(crate) saved: Saved,
    /// The folder the sign-in came from, when it wasn't the environment.
    dir: Option<PathBuf>,
    pub(crate) http: reqwest::blocking::Client,
}

impl Site {
    pub(crate) fn open(dir: Option<&str>) -> Result<Self, String> {
        let from_env = dir
            .is_none()
            .then(|| Saved::from_env(|name| std::env::var(name).ok()))
            .flatten();
        let (saved, dir) = match from_env {
            Some(saved) => (saved, None),
            None => {
                let dir = sign_in_dir(dir);
                let saved = Saved::load_file(&dir).ok_or_else(|| {
                    format!(
                        "Not signed in here ({}). Sign in with: coder login, or set OPENAGENTS_TOKEN to a sign-in key.",
                        dir.display()
                    )
                })?;
                (saved, Some(dir))
            }
        };
        let http = reqwest::blocking::Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|error| format!("Couldn't start the web client: {error}"))?;
        Ok(Self { saved, dir, http })
    }

    pub(crate) fn answer(
        response: reqwest::Result<reqwest::blocking::Response>,
    ) -> Result<Value, String> {
        let response = response.map_err(|_| "The website couldn't be reached.".to_owned())?;
        let status = response.status();
        let body: Value = response.json().unwrap_or(Value::Null);
        if status.is_success() {
            return Ok(body);
        }
        if status.as_u16() == 401 {
            return Err(
                "Not signed in: the sign-in was revoked or expired. Sign in again with coder login, or a new key in OPENAGENTS_TOKEN.".into(),
            );
        }
        Err(body["error"]["message"]
            .as_str()
            .map_or_else(|| format!("The website answered {status}."), str::to_owned))
    }

    pub(crate) fn url(&self, path: &str) -> String {
        format!("{}{path}", self.saved.origin)
    }

    fn list(&self, kind: &str) -> Result<Value, String> {
        Self::answer(
            self.http
                .get(self.url(&format!("/v1/keys?kind={kind}")))
                .bearer_auth(self.saved.token())
                .send(),
        )
    }

    fn create(&self, kind: &str, name: &str, days: Option<u64>) -> Result<Value, String> {
        Self::answer(
            self.http
                .post(self.url("/v1/keys"))
                .bearer_auth(self.saved.token())
                .json(&json!({"kind": kind, "name": name, "expires_in_days": days}))
                .send(),
        )
    }

    fn revoke(&self, kind: &str, id: &str) -> Result<Value, String> {
        if !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err("That key id isn't valid.".into());
        }
        Self::answer(
            self.http
                .delete(self.url(&format!("/v1/keys/{id}?kind={kind}")))
                .bearer_auth(self.saved.token())
                .send(),
        )
    }

    /// The one key `target` names: its whole id, the start of it (6
    /// characters or more), or its name.
    fn find(&self, kind: &str, target: &str) -> Result<(Value, Value), String> {
        let listed = self.list(kind)?;
        let keys = listed["keys"].as_array().cloned().unwrap_or_default();
        let id_of = |key: &Value| key["id"].as_str().unwrap_or_default().to_owned();
        if let Some(key) = keys.iter().find(|key| id_of(key) == target) {
            return Ok((listed.clone(), key.clone()));
        }
        let mut found: Vec<&Value> = keys
            .iter()
            .filter(|key| target.len() >= 6 && id_of(key).starts_with(target))
            .collect();
        if found.is_empty() {
            found = keys.iter().filter(|key| key["name"] == target).collect();
        }
        match found.as_slice() {
            [key] => Ok((listed.clone(), (*key).clone())),
            [] => Err(format!(
                "No key is named or has the id `{target}`. See them with: openagents keys list"
            )),
            many => Err(format!(
                "More than one key matches `{target}`: {}. Use the id.",
                many.iter()
                    .map(|key| short(&id_of(key)))
                    .collect::<Vec<_>>()
                    .join(", ")
            )),
        }
    }

    /// Make the replacement first, keep it where the old one was saved
    /// when this command was running on the old one, then revoke the old.
    fn rotate(&self, kind: &str, target: &str) -> Result<Value, String> {
        let (_, old) = self.find(kind, target)?;
        let name = old["name"].as_str().unwrap_or("Agent");
        let days = match (old["expires_at"].as_u64(), old["created_at"].as_u64()) {
            (Some(expires), Some(created)) => {
                Some((expires.saturating_sub(created) + DAY / 2) / DAY).filter(|d| *d > 0)
            }
            _ => None,
        };
        let made = self.create(kind, name, days)?;
        let mut answer = with_use(self, made);
        let old_id = old["id"].as_str().unwrap_or_default().to_owned();
        // A command running on the key it rotates keeps working after.
        if old["current"] == json!(true)
            && let (Some(dir), Some(token)) = (&self.dir, answer["token"].as_str())
        {
            let renewed = Saved::with_token(
                &self.saved.origin,
                &self.saved.account,
                &self.saved.label,
                answer["key"]["expires_at"].as_u64().unwrap_or(u64::MAX),
                token,
            );
            renewed.store(dir)?;
            answer["saved"] = json!(dir.join(openagents_login::FILE).display().to_string());
        }
        self.revoke(kind, &old_id)?;
        answer["revoked"] = json!(old_id);
        Ok(answer)
    }
}

/// A create answer with the line that uses the new key.
fn with_use(site: &Site, mut made: Value) -> Value {
    if let Some(token) = made["token"].as_str() {
        let origin = &site.saved.origin;
        let line = if made["kind"] == "api" {
            format!("OPENAGENTS_API_KEY={token} openagents inference ...")
        } else if origin == openagents_login::DEFAULT_ORIGIN {
            format!("OPENAGENTS_TOKEN={token} coder")
        } else {
            format!("OPENAGENTS_ORIGIN={origin} OPENAGENTS_TOKEN={token} coder")
        };
        made["use"] = json!(line);
    }
    made
}

fn short(id: &str) -> String {
    id.chars().take(12).collect()
}

fn quoted(key: &Value) -> String {
    format!("\"{}\"", key["name"].as_str().unwrap_or("Unnamed key"))
}

/// One key in one line: id, name, and its dates.
fn key_line(key: &Value, kind: &str) -> String {
    let id = short(key["id"].as_str().unwrap_or_default());
    if kind == "api" {
        return format!(
            "{id}  {}  made {}  {}",
            quoted(key),
            key["created"].as_str().unwrap_or("-"),
            key["status"].as_str().unwrap_or("active")
        );
    }
    let at = |field: &str, none: &str| key[field].as_u64().map_or_else(|| none.to_owned(), date);
    format!(
        "{id}  {}  made {}  last used {}  expires {}{}",
        quoted(key),
        at("created_at", "-"),
        at("last_used_at", "never"),
        at("expires_at", "never"),
        if key["current"] == json!(true) {
            "  (this key)"
        } else {
            ""
        }
    )
}

fn list_text(listed: &Value, kind: &str) -> String {
    let keys = listed["keys"].as_array().cloned().unwrap_or_default();
    let what = if kind == "api" {
        "API keys"
    } else {
        "sign-in keys"
    };
    if keys.is_empty() {
        return format!(
            "No {what} on {}. Make one with: openagents keys create --name NAME{}",
            listed["account"]["label"]
                .as_str()
                .unwrap_or("your account"),
            if kind == "api" { " --kind api" } else { "" }
        );
    }
    keys.iter()
        .map(|key| key_line(key, kind))
        .collect::<Vec<_>>()
        .join("\n")
}

fn made_text(made: &Value, verb: &str) -> String {
    let key = &made["key"];
    let mut text = format!(
        "{verb} {} ({}). Copy it now; it won't be shown again:\n{}\nUse it: {}",
        quoted(key),
        short(key["id"].as_str().unwrap_or_default()),
        made["token"].as_str().unwrap_or_default(),
        made["use"].as_str().unwrap_or_default(),
    );
    if let Some(saved) = made["saved"].as_str() {
        text.push_str(&format!("\nSaved the new key in {saved}."));
    }
    if let Some(old) = made["revoked"].as_str() {
        text.push_str(&format!("\nRevoked the old key ({}).", short(old)));
    }
    text
}

/// A Unix time as a UTC calendar date, `2026-10-11`.
fn date(seconds: u64) -> String {
    let days = (seconds / DAY) as i64 + 719_468;
    let era = days / 146_097;
    let doe = days - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiries_read_as_days() {
        assert_eq!(expires_days("never"), Ok(None));
        assert_eq!(expires_days("30d"), Ok(Some(30)));
        assert_eq!(expires_days("90"), Ok(Some(90)));
        assert!(expires_days("0d").is_err());
        assert!(expires_days("soon").is_err());
    }

    #[test]
    fn a_listing_line_never_holds_a_secret() {
        let key = json!({"id": "a".repeat(64), "name": "agent", "created_at": 1_791_504_000u64,
            "last_used_at": null, "expires_at": null, "current": true});
        let line = key_line(&key, "signin");
        assert_eq!(
            line,
            "aaaaaaaaaaaa  \"agent\"  made 2026-10-09  last used never  expires never  (this key)"
        );
    }
}
