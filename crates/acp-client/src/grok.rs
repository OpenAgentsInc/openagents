//! The local Grok Build CLI as an ACP agent: `grok agent stdio`.
//!
//! Grok Build's agent mode is an ACP server on standard input and output.
//! Full access starts it with `--always-approve`. A bounded turn omits that
//! flag, and the host rejects each permission request. `--no-leader` keeps
//! the process off the operator's interactive leader. The design follows
//! Grok Build's published agent-mode contract and is reimplemented here.

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
/// leaves it off so the agent asks, and the host rejects each ask.
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
