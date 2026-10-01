//! Which account each engine is signed in as, as a salted fingerprint, so a
//! usage reading or a refusal hold is never applied to a login it was not
//! about (#10105).
//!
//! The owner logged out of an exhausted Claude account and into one with
//! capacity; the hold and the last reading outlived the login they were
//! about, so Coder kept passing Claude over. Every reading in `usage.json`
//! and every refusal in `capacity.json` now carries the fingerprint of the
//! login it was observed on, and every reader drops a reading or a hold
//! whose fingerprint differs from the login signed in now. A dropped
//! reading leaves its provider due for a probe, so a host with probes on
//! reads the new login's windows on its next look.
//!
//! **What is read.** Only non-secret identity fields, never a token, and
//! nothing is written to a credential store:
//!
//! | Engine | File | Fields |
//! | --- | --- | --- |
//! | Claude Code | `$CLAUDE_CONFIG_DIR/.claude.json`, else `~/.claude.json` | `oauthAccount.accountUuid` (else `emailAddress`) and `organizationUuid` |
//! | Codex | `$CODEX_HOME/auth.json`, else `~/.codex/auth.json` | `tokens.account_id` |
//! | Grok Build | `$GROK_HOME/auth.json`, else `~/.grok/auth.json` | each login's `user_id` (else `email`) |
//!
//! A login those fields do not name (an API key, a Grok login without a
//! user id) has no fingerprint: its holds and readings keep their meaning
//! and end by time, as before. The identity itself never leaves this
//! process: only `sha256(salt ‖ provider ‖ identity)`, cut to 128 bits, is
//! kept, with the salt a random value in the task store (`account.salt`,
//! `0600`), so a fingerprint cannot be matched against a list of emails or
//! across computers.

use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::SystemTime;

use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::capacity::Provider;

/// The salt's file in the task store directory.
pub const SALT_FILE: &str = "account.salt";

/// Reads an environment variable.
pub type Env<'a> = &'a dyn Fn(&str) -> Option<OsString>;

/// The current non-secret identity of a provider's login, or `None`.
pub type Identify = fn(Provider) -> Option<String>;

/// The identity of `provider`'s signed-in login on this computer, read
/// from the process environment's homes. In this crate's own unit tests it
/// reads nothing, so no test ever opens the person's real files; tests pass
/// their own [`Identify`].
#[must_use]
pub fn identify(provider: Provider) -> Option<String> {
    if cfg!(test) {
        return None;
    }
    identify_in(provider, &|name| std::env::var_os(name))
}

/// [`identify`] with `env` for the environment: tests point it at a
/// temporary home.
#[must_use]
pub fn identify_in(provider: Provider, env: Env) -> Option<String> {
    let path = path(provider, env)?;
    let meta = std::fs::metadata(&path).ok().filter(|m| m.is_file())?;
    let stamp = (meta.len(), meta.modified().ok());
    if let Some(found) = cached(&path, stamp) {
        return found;
    }
    let found = std::fs::read(&path)
        .ok()
        .and_then(|bytes| parse(provider, &bytes));
    remember(path, stamp, found.clone());
    found
}

/// The file `provider`'s identity is read from.
fn path(provider: Provider, env: Env) -> Option<PathBuf> {
    let home = || env("HOME").filter(|h| !h.is_empty()).map(PathBuf::from);
    let dir = |name: &str| env(name).filter(|v| !v.is_empty()).map(PathBuf::from);
    match provider {
        Provider::Claude => dir("CLAUDE_CONFIG_DIR")
            .map(|dir| dir.join(".claude.json"))
            .or_else(|| home().map(|home| home.join(".claude.json"))),
        Provider::Codex => dir("CODEX_HOME")
            .or_else(|| home().map(|home| home.join(".codex")))
            .map(|dir| dir.join("auth.json")),
        Provider::Grok => acp_client::grok::auth_path(env),
        Provider::Vertex | Provider::Devin | Provider::OpenCode => None,
    }
}

/// The identity fields of `provider`'s file, typed; every other field,
/// tokens included, is skipped unread into any value.
fn parse(provider: Provider, bytes: &[u8]) -> Option<String> {
    let id = |text: Option<String>| text.filter(|t| !t.is_empty() && t.len() <= 256);
    match provider {
        Provider::Claude => {
            #[derive(Deserialize)]
            struct State {
                #[serde(default, rename = "oauthAccount")]
                account: Option<Account>,
            }
            #[derive(Deserialize)]
            struct Account {
                #[serde(default, rename = "accountUuid")]
                uuid: Option<String>,
                #[serde(default, rename = "emailAddress")]
                email: Option<String>,
                #[serde(default, rename = "organizationUuid")]
                organization: Option<String>,
            }
            let account = serde_json::from_slice::<State>(bytes).ok()?.account?;
            let who = id(account.uuid).or_else(|| id(account.email))?;
            Some(format!(
                "{who}\n{}",
                id(account.organization).unwrap_or_default()
            ))
        }
        Provider::Codex => {
            #[derive(Deserialize)]
            struct Auth {
                #[serde(default)]
                tokens: Option<Tokens>,
            }
            #[derive(Deserialize)]
            struct Tokens {
                #[serde(default)]
                account_id: Option<String>,
            }
            id(serde_json::from_slice::<Auth>(bytes)
                .ok()?
                .tokens?
                .account_id)
        }
        Provider::Grok => {
            #[derive(Deserialize)]
            struct Login {
                #[serde(default)]
                user_id: Option<String>,
                #[serde(default)]
                email: Option<String>,
            }
            let logins: BTreeMap<String, serde_json::Value> = serde_json::from_slice(bytes).ok()?;
            let ids: Vec<String> = logins
                .into_iter()
                .filter_map(|(issuer, login)| {
                    let login: Login = serde_json::from_value(login).ok()?;
                    let who = id(login.user_id).or_else(|| id(login.email))?;
                    Some(format!("{issuer}={who}"))
                })
                .collect();
            (!ids.is_empty()).then(|| ids.join("\n"))
        }
        Provider::Vertex | Provider::Devin | Provider::OpenCode => None,
    }
}

type Stamp = (u64, Option<SystemTime>);

/// Identities read before, by file, while the file's size and time are
/// unchanged: `~/.claude.json` can be large and the books are read often.
static CACHE: Mutex<BTreeMap<PathBuf, (Stamp, Option<String>)>> = Mutex::new(BTreeMap::new());

fn cached(path: &Path, stamp: Stamp) -> Option<Option<String>> {
    let cache = CACHE.lock().ok()?;
    cache
        .get(path)
        .filter(|(seen, _)| *seen == stamp)
        .map(|(_, found)| found.clone())
}

fn remember(path: PathBuf, stamp: Stamp, found: Option<String>) {
    if let Ok(mut cache) = CACHE.lock() {
        cache.insert(path, (stamp, found));
    }
}

/// The fingerprint of `identity` for `provider`, salted with the store
/// `dir`'s salt. `None` when the salt cannot be read or made.
#[must_use]
pub fn fingerprint(dir: &Path, provider: Provider, identity: &str) -> Option<String> {
    let salt = salt(dir)?;
    let mut hash = Sha256::new();
    hash.update(b"openagents.coder.account.v1\0");
    hash.update(salt);
    hash.update(provider.as_str().as_bytes());
    hash.update(b"\0");
    hash.update(identity.as_bytes());
    let digest = hash.finalize();
    Some(digest[..16].iter().map(|b| format!("{b:02x}")).collect())
}

/// The fingerprint of `provider`'s login now, for the store `dir`.
#[must_use]
pub fn current(dir: &Path, provider: Provider, identify: Identify) -> Option<String> {
    identify(provider).and_then(|identity| fingerprint(dir, provider, &identity))
}

/// Whether a record observed on the login `recorded` still applies to the
/// login `current`: only two known, different logins disagree. A record
/// from before fingerprints, or a login with none, keeps its meaning.
#[must_use]
pub fn applies(recorded: Option<&str>, current: Option<&str>) -> bool {
    match (recorded, current) {
        (Some(recorded), Some(current)) => recorded == current,
        _ => true,
    }
}

/// The store's salt: 32 random bytes, made once (`0600`, published with a
/// hard link so two processes never keep different salts).
fn salt(dir: &Path) -> Option<[u8; 32]> {
    static SALTS: Mutex<BTreeMap<PathBuf, [u8; 32]>> = Mutex::new(BTreeMap::new());
    if let Some(salt) = SALTS.lock().ok()?.get(dir) {
        return Some(*salt);
    }
    // A reader never makes the store: with no store there is nothing to
    // compare a fingerprint with.
    if !dir.is_dir() {
        return None;
    }
    let path = dir.join(SALT_FILE);
    let read =
        |path: &Path| -> Option<[u8; 32]> { std::fs::read(path).ok()?.as_slice().try_into().ok() };
    let salt = match read(&path) {
        Some(salt) => salt,
        None => {
            let mut fresh = [0u8; 32];
            getrandom::fill(&mut fresh).ok()?;
            let temporary = dir.join(format!("{SALT_FILE}.{}", std::process::id()));
            {
                use std::io::Write;
                let mut file = crate::capacity::open_private(dir, &temporary).ok()?;
                file.set_len(0).ok()?;
                file.write_all(&fresh).and_then(|()| file.sync_all()).ok()?;
            }
            let linked = std::fs::hard_link(&temporary, &path);
            let _ = std::fs::remove_file(&temporary);
            match linked {
                Ok(()) => fresh,
                // Another process published first: use its salt.
                Err(_) => read(&path)?,
            }
        }
    };
    SALTS.lock().ok()?.insert(dir.to_path_buf(), salt);
    Some(salt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn env(home: &Path) -> impl Fn(&str) -> Option<OsString> + '_ {
        move |name| (name == "HOME").then(|| home.as_os_str().to_owned())
    }

    /// Stand-in credentials only, in a temporary home: the identity comes
    /// from the typed account fields, and no token reaches it.
    #[test]
    fn each_engines_identity_is_read_from_metadata_only() {
        let home = tempfile::tempdir().unwrap();
        let env = env(home.path());
        for provider in Provider::ALL {
            assert_eq!(identify_in(provider, &env), None, "{provider}");
        }
        std::fs::write(
            home.path().join(".claude.json"),
            r#"{"numStartups":3,"oauthAccount":{"accountUuid":"acct-a","emailAddress":"a@example.invalid","organizationUuid":"org-1"},"projects":{}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(home.path().join(".codex")).unwrap();
        std::fs::write(
            home.path().join(".codex/auth.json"),
            r#"{"auth_mode":"chatgpt","tokens":{"access_token":"stand-in-secret","refresh_token":"stand-in-refresh","account_id":"codex-acct-1"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(home.path().join(".grok")).unwrap();
        std::fs::write(
            home.path().join(".grok/auth.json"),
            r#"{"https://auth.x.ai":{"key":"stand-in-key","user_id":"grok-user-7","expires_at":"2026-10-01T14:09:41Z"}}"#,
        )
        .unwrap();
        let claude = identify_in(Provider::Claude, &env).unwrap();
        assert_eq!(claude, "acct-a\norg-1");
        let codex = identify_in(Provider::Codex, &env).unwrap();
        assert_eq!(codex, "codex-acct-1");
        let grok = identify_in(Provider::Grok, &env).unwrap();
        assert!(grok.ends_with("=grok-user-7"));
        for identity in [&claude, &codex, &grok] {
            assert!(!identity.contains("stand-in"));
        }
        // An API-key or id-less login has no identity.
        std::fs::write(
            home.path().join(".grok/auth.json"),
            r#"{"https://auth.x.ai":{"key":"stand-in-key"}}"#,
        )
        .unwrap();
        assert_eq!(identify_in(Provider::Grok, &env), None);
        // `CLAUDE_CONFIG_DIR` relocates Claude Code's config, as it does
        // for Claude Code itself.
        let other = tempfile::tempdir().unwrap();
        std::fs::write(
            other.path().join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"b@example.invalid"}}"#,
        )
        .unwrap();
        let relocated = |name: &str| match name {
            "CLAUDE_CONFIG_DIR" => Some(other.path().as_os_str().to_owned()),
            _ => env(name),
        };
        assert_eq!(
            identify_in(Provider::Claude, &relocated).as_deref(),
            Some("b@example.invalid\n")
        );
    }

    #[test]
    fn a_fingerprint_is_salted_per_store_and_stable_within_one() {
        let one = tempfile::tempdir().unwrap();
        let two = tempfile::tempdir().unwrap();
        let a = fingerprint(one.path(), Provider::Claude, "acct-a").unwrap();
        assert_eq!(a.len(), 32);
        assert!(!a.contains("acct"));
        assert_eq!(
            fingerprint(one.path(), Provider::Claude, "acct-a").unwrap(),
            a
        );
        assert_ne!(
            fingerprint(one.path(), Provider::Claude, "acct-b").unwrap(),
            a
        );
        assert_ne!(
            fingerprint(one.path(), Provider::Codex, "acct-a").unwrap(),
            a
        );
        assert_ne!(
            fingerprint(two.path(), Provider::Claude, "acct-a").unwrap(),
            a
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let meta = std::fs::metadata(one.path().join(SALT_FILE)).unwrap();
            assert_eq!(meta.permissions().mode() & 0o777, 0o600);
        }
        assert!(applies(Some(&a), Some(&a)));
        assert!(!applies(Some(&a), Some("other")));
        assert!(applies(None, Some(&a)));
        assert!(applies(Some(&a), None));
    }
}
