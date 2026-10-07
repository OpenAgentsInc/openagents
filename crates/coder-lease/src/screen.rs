//! The screen check: an agent opens a window on the real screen, or
//! captures it, only while it holds a `screen` lease, which the broker
//! admits only under the owner's grant ([`crate::Grant`]).
//!
//! [`screen_refusal`] is the check a windowed program runs before it opens
//! a window. A person running the program by hand is unaffected; an agent
//! environment without the lease is refused with a sentence that points to
//! offscreen capture. [`SCREENCAPTURE_SHIM`] is the same rule for macOS
//! `screencapture`, which [`crate::shim`] writes beside the `cargo` shim.

use crate::{AGENT_VARS, LEASE_ID_VAR, LEASES_VAR, SESSION_VAR};

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
/// when it may: outside an agent environment, or under a `screen` lease.
#[must_use]
pub fn screen_refusal(program: &str, env: &dyn Fn(&str) -> Option<String>) -> Option<String> {
    if holds_screen(env) {
        return None;
    }
    let marker = agent_marker(env)?;
    Some(format!(
        "{program} doesn't open a window here: {marker} marks an agent environment, and an agent \
         uses the real screen only under a screen lease, which needs the owner's grant. Render \
         offscreen instead, such as `verse --capture FILE.png`, or run `openagents lease screen \
         -- {program} ...` once the owner has run `openagents lease grant screen`"
    ))
}

/// [`screen_refusal`] over this process's environment.
#[must_use]
pub fn screen_refusal_here(program: &str) -> Option<String> {
    screen_refusal(program, &|name| std::env::var(name).ok())
}

/// The `screencapture` shim's text: without a `screen` lease it refuses,
/// and under one it runs the real `screencapture`.
pub const SCREENCAPTURE_SHIM: &str = r#"#!/bin/sh
# OpenAgents lease shim for screencapture, written by the coder-lease crate.
# It runs the real screencapture only under a screen lease.
# docs/coder/runtime/leases.md explains it. Don't edit: it's rewritten.
set -f
case ",${OPENAGENTS_LEASES:-}," in
  *,screen,*) ;;
  *)
    echo "screencapture: an agent captures the real screen only under a screen lease, which needs the owner's grant. Render offscreen instead, such as \`verse --capture FILE.png\`, or run \`openagents lease screen -- screencapture ...\` once the owner has run \`openagents lease grant screen\`" >&2
    exit 1
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
}
