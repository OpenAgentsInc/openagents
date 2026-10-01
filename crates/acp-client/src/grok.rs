//! The local Grok Build CLI as an ACP agent: `grok agent stdio`.
//!
//! Grok Build's agent mode is an ACP server on standard input and output.
//! Full access starts it with `--always-approve`. A bounded turn omits that
//! flag and runs the whole process inside the host's own operating-system
//! boundary, with a private Grok home that holds a copy of the login
//! ([`login_seconds_left`] says whether a copy is safe to use); the host
//! then answers its permission requests. `--no-leader` keeps the process
//! off the operator's interactive leader. The design follows Grok Build's
//! published agent-mode contract and is reimplemented here.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::process::{first_executable, on_path};

/// The variable that names the binary.
pub const BIN_VAR: &str = "GROK_BIN";
/// The route model that keeps Grok Build's own default model.
pub const DEFAULT_MODEL: &str = "default";
/// Grok Build's API login, used when no auth file is present.
pub const API_KEY_VAR: &str = "XAI_API_KEY";
/// The variable that relocates Grok Build's home, including `auth.json`.
pub const HOME_VAR: &str = "GROK_HOME";

/// Where Grok Build keeps its CLI login: `$GROK_HOME/auth.json`, or
/// `~/.grok/auth.json`.
#[must_use]
pub fn auth_path(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(home) = variable(HOME_VAR).filter(|value| !value.is_empty()) {
        return Some(PathBuf::from(home).join("auth.json"));
    }
    variable("HOME").map(|home| Path::new(&home).join(".grok/auth.json"))
}

/// The Grok Build binary: `GROK_BIN`, else `grok` on `PATH`, else
/// `~/.local/bin/grok`, else `~/.grok/bin/grok`. `variable` reads the
/// environment.
#[must_use]
pub fn binary(variable: &dyn Fn(&str) -> Option<OsString>) -> Option<PathBuf> {
    if let Some(named) = variable(BIN_VAR).filter(|value| !value.is_empty()) {
        return first_executable([PathBuf::from(named)]);
    }
    let home = variable("HOME").map(PathBuf::from);
    first_executable(
        on_path("grok", variable("PATH").as_deref())
            .into_iter()
            .chain(home.as_ref().map(|home| home.join(".local/bin/grok")))
            .chain(home.as_ref().map(|home| home.join(".grok/bin/grok"))),
    )
}

/// Whether Grok Build has a stored login or [`API_KEY_VAR`]. Only the auth
/// file's size is read. The variable's value is never logged.
#[must_use]
pub fn signed_in(variable: &dyn Fn(&str) -> Option<OsString>) -> bool {
    let file = auth_path(variable)
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|meta| meta.is_file() && meta.len() > 0);
    let key = variable(API_KEY_VAR).is_some_and(|value| !value.is_empty());
    file || key
}

/// Whether `model` is [`DEFAULT_MODEL`] or a Grok Build model id: 1 to 128
/// characters, starting with an ASCII letter or digit, and containing only
/// ASCII letters, digits, `.`, `_`, and `-`.
///
/// # Errors
/// `model` is empty, too long, or contains another character.
pub fn parse_model(model: &str) -> Result<(), String> {
    if model == DEFAULT_MODEL {
        return Ok(());
    }
    let ok = !model.is_empty()
        && model.len() <= 128
        && model.as_bytes()[0].is_ascii_alphanumeric()
        && model
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
    if ok {
        Ok(())
    } else {
        Err(format!(
            "`{model}` is not a Grok Build model id (letters, digits, `.`, `_`, and `-`)"
        ))
    }
}

/// The arguments that start Grok Build as an ACP agent with `model`.
///
/// `approve` adds `--always-approve`, which is full access. A bounded turn
/// leaves it off so the agent asks, and the host answers each ask.
#[must_use]
pub fn arguments(model: &str, approve: bool) -> Vec<String> {
    let mut arguments = vec!["agent".to_string()];
    if approve {
        arguments.push("--always-approve".into());
    }
    if model != DEFAULT_MODEL {
        arguments.push("--model".into());
        arguments.push(model.into());
    }
    arguments.push("--no-leader".into());
    arguments.push("stdio".into());
    arguments
}

/// Whether the session's reported model is the one the route admitted.
/// [`DEFAULT_MODEL`] admits whichever model Grok Build reports.
#[must_use]
pub fn admits(route_model: &str, reported: Option<&str>) -> bool {
    route_model == DEFAULT_MODEL || reported == Some(route_model)
}

/// The least a login copied into a bounded turn's private Grok home must
/// have left beyond the time the turn is expected to take. Grok Build refreshes its sign-in
/// only near its expiry, and a refresh inside the copy could retire the
/// refresh token the person's own login still holds.
pub const LOGIN_MARGIN_SECONDS: i64 = 600;

/// The seconds until the earliest `expires_at` in a Grok Build `auth.json`
/// (`bytes`), at the Unix time `now`; `None` when the file names no
/// expiry this can read. Only the `expires_at` values are read.
#[must_use]
pub fn login_seconds_left(bytes: &[u8], now: i64) -> Option<i64> {
    let logins: serde_json::Map<String, serde_json::Value> = serde_json::from_slice(bytes).ok()?;
    logins
        .values()
        .filter_map(|login| login.get("expires_at")?.as_str())
        .filter_map(unix_seconds)
        .map(|at| at - now)
        .min()
}

/// An RFC 3339 time, such as `2026-10-01T14:09:41.808793Z` or
/// `2026-10-01T14:09:41+02:00`, as Unix seconds.
fn unix_seconds(text: &str) -> Option<i64> {
    let bytes = text.as_bytes();
    if bytes.len() < 20 || bytes[4] != b'-' || bytes[7] != b'-' || bytes[13] != b':' {
        return None;
    }
    if !matches!(bytes[10], b'T' | b't' | b' ') || bytes[16] != b':' {
        return None;
    }
    let number = |range: std::ops::Range<usize>| -> Option<i64> {
        let part = text.get(range)?;
        part.bytes()
            .all(|byte| byte.is_ascii_digit())
            .then(|| part.parse().ok())?
    };
    let (year, month, day) = (number(0..4)?, number(5..7)?, number(8..10)?);
    let (hour, minute, second) = (number(11..13)?, number(14..16)?, number(17..19)?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) || hour > 23 || minute > 59 {
        return None;
    }
    let mut rest = &text[19..];
    if let Some(fraction) = rest.strip_prefix('.') {
        let digits = fraction.bytes().take_while(u8::is_ascii_digit).count();
        rest = &fraction[digits..];
    }
    let offset = match rest {
        "Z" | "z" => 0,
        _ => {
            let sign = match rest.as_bytes().first()? {
                b'+' => 1,
                b'-' => -1,
                _ => return None,
            };
            if rest.len() != 6 || rest.as_bytes()[3] != b':' {
                return None;
            }
            let hours: i64 = rest[1..3].parse().ok()?;
            let minutes: i64 = rest[4..6].parse().ok()?;
            sign * (hours * 3600 + minutes * 60)
        }
    };
    // Days from the civil date (Howard Hinnant's algorithm).
    let shifted = if month <= 2 { year - 1 } else { year };
    let era = shifted.div_euclid(400);
    let year_of_era = shifted - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days = era * 146_097 + day_of_era - 719_468;
    Some(days * 86_400 + hour * 3600 + minute * 60 + second - offset)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(unix)]
    fn the_binary_is_found_by_variable_path_or_install_location() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join(".grok/bin");
        std::fs::create_dir_all(&bin).unwrap();
        let grok = bin.join("grok");
        std::fs::write(&grok, "#!/bin/sh\n").unwrap();
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&grok, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert_eq!(binary(&env), Some(grok.clone()));
        let named = grok.as_os_str().to_owned();
        let env = |name: &str| (name == BIN_VAR).then(|| named.clone());
        assert_eq!(binary(&env), Some(grok));
        let missing = |name: &str| (name == BIN_VAR).then(|| OsString::from("/nonexistent/grok"));
        assert_eq!(binary(&missing), None);
    }

    #[test]
    fn the_login_is_checked_without_reading_it() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == "HOME").then(|| home.clone());
        assert!(!signed_in(&env));
        let path = auth_path(&env).unwrap();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, "").unwrap();
        assert!(!signed_in(&env));
        std::fs::write(&path, "x").unwrap();
        assert!(signed_in(&env));

        let empty = dir.path().join("empty-home");
        std::fs::create_dir_all(&empty).unwrap();
        let empty_home = empty.as_os_str().to_owned();
        let key = OsString::from("present");
        let env = |name: &str| match name {
            "HOME" => Some(empty_home.clone()),
            API_KEY_VAR => Some(key.clone()),
            _ => None,
        };
        assert!(signed_in(&env));
        assert!(!empty.join(".grok").exists());
    }

    #[test]
    fn the_login_expiry_is_read_and_nothing_else() {
        // 2026-10-01T14:09:41Z is 1 790 863 781.
        let at = 1_790_863_781;
        let bytes = br#"{"https://auth.x.ai":{"key":"unread","refresh_token":"unread",
            "expires_at":"2026-10-01T14:09:41.808793Z"},
            "other":{"expires_at":"2026-10-01T16:09:41+02:00"}}"#;
        assert_eq!(login_seconds_left(bytes, at - 100), Some(100));
        assert_eq!(
            login_seconds_left(br#"{"a":{"expires_at":"2026-10-01T15:09:41Z"}}"#, at),
            Some(3600)
        );
        assert_eq!(login_seconds_left(br#"{"a":{"key":"k"}}"#, at), None);
        assert_eq!(login_seconds_left(b"not json", at), None);
        assert_eq!(
            login_seconds_left(br#"{"a":{"expires_at":"tomorrow"}}"#, at),
            None
        );
        assert_eq!(unix_seconds("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(unix_seconds("2000-03-01T00:00:00Z"), Some(951_868_800));
    }

    #[test]
    fn grok_home_relocates_the_auth_file() {
        let dir = tempfile::tempdir().unwrap();
        let relocated = dir.path().as_os_str().to_owned();
        let env = |name: &str| (name == HOME_VAR).then(|| relocated.clone());
        assert_eq!(auth_path(&env), Some(dir.path().join("auth.json")));
    }

    #[test]
    fn the_model_and_the_arguments_are_explicit() {
        assert_eq!(
            arguments(DEFAULT_MODEL, true),
            vec!["agent", "--always-approve", "--no-leader", "stdio"]
        );
        assert_eq!(
            arguments("grok-4.6", true),
            vec![
                "agent",
                "--always-approve",
                "--model",
                "grok-4.6",
                "--no-leader",
                "stdio"
            ]
        );
        assert_eq!(
            arguments(DEFAULT_MODEL, false),
            vec!["agent", "--no-leader", "stdio"]
        );
        assert!(parse_model(DEFAULT_MODEL).is_ok());
        assert!(parse_model("grok-4.6").is_ok());
        assert!(parse_model("grok/4").is_err());
        assert!(parse_model("").is_err());
        assert!(admits(DEFAULT_MODEL, Some("grok-4.6")));
        assert!(admits(DEFAULT_MODEL, None));
        assert!(admits("grok-4.6", Some("grok-4.6")));
        assert!(!admits("grok-4.6", Some("grok-4.5")));
        assert!(!admits("grok-4.6", None));
    }
}
