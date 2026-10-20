//! The screen check: an unattended agent opens a window on the real
//! screen, or captures it, only while it holds a `screen` lease, which the
//! broker admits only under the owner's grant ([`crate::Grant`]). When the
//! owner asked for the window, the agent says so with [`OWNER_ASKED_VAR`]
//! (or a program's `--owner-asked` flag) and runs it directly, as AGENTS.md
//! requires; [`screen_check`] records that launch in [`SCREEN_LOG_FILE`].
//!
//! [`screen_refusal`] is the check a windowed program runs before it opens
//! a window. A person running the program by hand is unaffected; an agent
//! environment without the lease or the owner's ask is refused with a
//! sentence that points to offscreen capture. [`SCREENCAPTURE_SHIM`] is the
//! same rule for macOS `screencapture`, which [`crate::shim`] writes beside
//! the `cargo` shim.

use std::path::Path;

use crate::{AGENT_VARS, LEASE_ID_VAR, LEASES_VAR, SESSION_VAR};

/// Set to `1` by an agent when the owner asked to see a window or run a
/// desktop test (issue #12238). It lets the agent open the window without a
/// `screen` lease; the launch is logged in [`SCREEN_LOG_FILE`].
pub const OWNER_ASKED_VAR: &str = "OPENAGENTS_OWNER_ASKED";

/// The log, in the lease root, of windows an agent opened because the
/// owner asked.
pub const SCREEN_LOG_FILE: &str = "screen-launches.jsonl";

/// The schema of a [`SCREEN_LOG_FILE`] line.
pub const SCREEN_LOG_SCHEMA: &str = "openagents.screen-launch.v1";

/// Whether `OPENAGENTS_OWNER_ASKED` says the owner asked for the window.
#[must_use]
pub fn owner_asked(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env(OWNER_ASKED_VAR).is_some_and(|value| {
        matches!(
            value.trim().to_ascii_lowercase().as_str(),
            "1" | "true" | "yes"
        )
    })
}

/// The variable that marks this environment as an agent's, if any:
/// `OPENAGENTS_SESSION`, which Coder sets for its delegates and a lease
/// passes to its command, or an agent's own variable such as `CLAUDECODE`.
/// A person can run a command under a lease too, so
/// `OPENAGENTS_LEASE_ID` doesn't count, and neither does a session named
/// `process:PID`, which is what a lease records when no agent holds it.
#[must_use]
pub fn agent_marker(env: &dyn Fn(&str) -> Option<String>) -> Option<&'static str> {
    if env(SESSION_VAR)
        .is_some_and(|session| !session.is_empty() && !session.starts_with("process:"))
    {
        return Some(SESSION_VAR);
    }
    AGENT_VARS
        .into_iter()
        .filter(|name| *name != LEASE_ID_VAR)
        .find(|name| env(name).is_some_and(|value| !value.is_empty()))
}

/// Whether `OPENAGENTS_LEASES` names the `screen` lease.
#[must_use]
pub fn holds_screen(env: &dyn Fn(&str) -> Option<String>) -> bool {
    env(LEASES_VAR).is_some_and(|leases| leases.split(',').any(|lease| lease.trim() == "screen"))
}

/// Why `program` may not open a window on the real screen here, or `None`
/// when it may: outside an agent environment, under a `screen` lease, or
/// when [`OWNER_ASKED_VAR`] says the owner asked for it.
#[must_use]
pub fn screen_refusal(program: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if holds_screen(env) || owner_asked(env) {
        return None;
    }
    let marker = agent_marker(env)?;
    Some(format!(
        "{program} doesn't open a window here: {marker} marks an agent environment, and an \
         unattended agent renders offscreen, such as `verse --capture FILE.png`. When the owner \
         asked to see the window, run it directly with {OWNER_ASKED_VAR}=1 (or `--owner-asked`); \
         otherwise run `openagents lease screen -- {program} ...` once the owner has run \
         `openagents lease grant screen`"
    ))
}

/// [`screen_refusal`] over this process's environment.
#[must_use]
pub fn screen_refusal_here(program: &str) -> Option<String> {
    screen_refusal(program, &|name| std::env::var(name).ok())
}

/// The check a windowed program runs before it opens a window: `Err` with
/// [`screen_refusal`]'s sentence when it may not. `flag` is the program's
/// own `--owner-asked`, which counts as [`OWNER_ASKED_VAR`]. When an agent
/// opens the window because the owner asked, and holds no `screen` lease,
/// the launch is appended to [`SCREEN_LOG_FILE`] under `root`; a log that
/// can't be written doesn't stop the window.
///
/// # Errors
/// The refusal sentence.
pub fn screen_check(
    program: &str,
    flag: bool,
    env: &dyn Fn(&str) -> Option<String>,
    root: Option<&Path>,
    session: &dyn Fn() -> crate::Session,
) -> Result<(), String> {
    let asked = flag || owner_asked(env);
    let with_ask = |name: &str| {
        if asked && name == OWNER_ASKED_VAR {
            Some("1".to_owned())
        } else {
            env(name)
        }
    };
    if let Some(refusal) = screen_refusal(program, &with_ask) {
        return Err(refusal);
    }
    if asked
        && !holds_screen(env)
        && let Some(marker) = agent_marker(env)
        && let Some(root) = root
    {
        let session = session();
        let _ = log_launch(root, program, marker, &session, crate::now_ms());
    }
    Ok(())
}

/// [`screen_check`] over this process's environment and lease root.
///
/// # Errors
/// The refusal sentence.
pub fn screen_check_here(program: &str, flag: bool) -> Result<(), String> {
    let env = |name: &str| std::env::var(name).ok();
    let root = crate::root_from_env().ok();
    screen_check(program, flag, &env, root.as_deref(), &|| {
        crate::Session::detect(&env, &crate::ancestors)
    })
}

/// Appends one owner-asked launch to [`SCREEN_LOG_FILE`] under `root`.
///
/// # Errors
/// The log can't be written.
pub fn log_launch(
    root: &Path,
    program: &str,
    marker: &str,
    session: &crate::Session,
    at_ms: u64,
) -> std::io::Result<()> {
    use std::io::Write as _;
    std::fs::create_dir_all(root)?;
    let line = serde_json::json!({
        "schema": SCREEN_LOG_SCHEMA,
        "at_ms": at_ms,
        "program": program,
        "reason": "owner-asked",
        "marker": marker,
        "session": session.id,
        "agent": session.agent,
    });
    let mut bytes = serde_json::to_vec(&line).map_err(std::io::Error::other)?;
    bytes.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join(SCREEN_LOG_FILE))?
        .write_all(&bytes)
}

/// The `screencapture` shim's text: without a `screen` lease it refuses,
/// and under one it runs the real `screencapture`.
pub const SCREENCAPTURE_SHIM: &str = r#"#!/bin/sh
# OpenAgents lease shim for screencapture, written by the coder-lease crate.
# It runs the real screencapture under a screen lease, or when
# OPENAGENTS_OWNER_ASKED=1 says the owner asked for it.
# docs/coder/runtime/leases.md explains it. Don't edit: it's rewritten.
set -f
case "${OPENAGENTS_OWNER_ASKED:-}" in
  1|true|TRUE|yes|YES) owner_asked=1 ;;
  *) owner_asked= ;;
esac
case ",${OPENAGENTS_LEASES:-}," in
  *,screen,*) ;;
  *)
    if [ -z "$owner_asked" ]; then
      echo "screencapture: an unattended agent renders offscreen, such as \`verse --capture FILE.png\`. When the owner asked for the capture, run it with OPENAGENTS_OWNER_ASKED=1; otherwise run \`openagents lease screen -- screencapture ...\` once the owner has run \`openagents lease grant screen\`" >&2
      exit 1
    fi
    ;;
esac
shims=$(CDPATH= cd -- "$(dirname -- "$0")" 2>/dev/null && pwd -P)
real=
saved_ifs=$IFS
IFS=:
for dir in $PATH; do
  [ -n "$dir" ] || dir=.
  here=$(CDPATH= cd -- "$dir" 2>/dev/null && pwd -P) || continue
  [ "$here" = "$shims" ] && continue
  if [ -f "$dir/screencapture" ] && [ -x "$dir/screencapture" ]; then
    real=$dir/screencapture
    break
  fi
done
IFS=$saved_ifs
if [ -z "$real" ]; then
  echo "screencapture: no screencapture on PATH besides the lease shim in $shims" >&2
  exit 127
fi
exec "$real" "$@"
"#;

#[cfg(test)]
mod tests {
    use super::*;

    fn env(pairs: &'static [(&'static str, &'static str)]) -> impl Fn(&str) -> Option<String> {
        move |name| {
            pairs
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    #[test]
    fn an_agent_without_the_screen_lease_gets_no_window() {
        for agent in [
            &[(SESSION_VAR, "seat-1")][..],
            &[("CLAUDECODE", "1")],
            &[("CODEX_THREAD_ID", "t")],
            &[("CLAUDECODE", "1"), (LEASES_VAR, "build,quiet")],
        ] {
            let refusal = screen_refusal("verse", &|name| {
                agent
                    .iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| (*value).to_owned())
            })
            .expect("refused");
            assert!(refusal.contains("verse --capture FILE.png"), "{refusal}");
            assert!(refusal.contains("OPENAGENTS_OWNER_ASKED=1"), "{refusal}");
            assert!(
                refusal.contains("openagents lease screen -- verse"),
                "{refusal}"
            );
        }
    }

    #[test]
    fn the_screen_lease_or_a_person_opens_a_window() {
        assert_eq!(
            screen_refusal(
                "verse",
                &env(&[("CLAUDECODE", "1"), (LEASES_VAR, "build,screen")])
            ),
            None
        );
        assert_eq!(screen_refusal("verse", &env(&[])), None);
        assert_eq!(screen_refusal("verse", &env(&[("CLAUDECODE", "")])), None);
        // A person's own leased command is not an agent environment.
        assert_eq!(
            screen_refusal(
                "verse",
                &env(&[
                    (LEASE_ID_VAR, "l1"),
                    (LEASES_VAR, "quiet"),
                    (SESSION_VAR, "process:42")
                ])
            ),
            None
        );
    }

    #[test]
    fn the_owners_ask_opens_a_window_and_is_logged() {
        let dir = tempfile::tempdir().expect("temp");
        let session = || crate::Session {
            id: "claude-code:abc".to_owned(),
            agent: "claude-code".to_owned(),
        };
        let agent = env(&[("CLAUDE_CODE_SESSION_ID", "abc")]);
        // Unattended: refused, nothing logged.
        assert!(screen_check("verse", false, &agent, Some(dir.path()), &session).is_err());
        assert!(!dir.path().join(SCREEN_LOG_FILE).exists());
        // The flag, then the variable: both open it and log a line each.
        screen_check("verse", true, &agent, Some(dir.path()), &session).expect("flag");
        let asked = env(&[("CLAUDE_CODE_SESSION_ID", "abc"), (OWNER_ASKED_VAR, "1")]);
        screen_check("verse", false, &asked, Some(dir.path()), &session).expect("var");
        assert_eq!(screen_refusal("verse", &asked), None);
        let log = std::fs::read_to_string(dir.path().join(SCREEN_LOG_FILE)).expect("log");
        let lines: Vec<serde_json::Value> = log
            .lines()
            .map(|line| serde_json::from_str(line).expect("json"))
            .collect();
        assert_eq!(lines.len(), 2);
        assert_eq!(lines[0]["schema"], SCREEN_LOG_SCHEMA);
        assert_eq!(lines[0]["program"], "verse");
        assert_eq!(lines[0]["session"], "claude-code:abc");
        assert_eq!(lines[0]["marker"], "CLAUDE_CODE_SESSION_ID");
        // A false value is no ask.
        let no = env(&[("CLAUDECODE", "1"), (OWNER_ASKED_VAR, "0")]);
        assert!(screen_refusal("verse", &no).is_some());
        // A person needs no ask and leaves no line.
        let person = tempfile::tempdir().expect("temp");
        screen_check("verse", true, &env(&[]), Some(person.path()), &session).expect("person");
        assert!(!person.path().join(SCREEN_LOG_FILE).exists());
    }

    #[test]
    fn the_screencapture_shim_honors_the_owners_ask() {
        assert!(SCREENCAPTURE_SHIM.contains("OPENAGENTS_OWNER_ASKED"));
    }
}
