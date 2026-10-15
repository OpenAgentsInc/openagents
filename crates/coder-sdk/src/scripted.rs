//! A scripted backend: replays a recorded session through the registered
//! hooks, so hooks can be tested with no login and no model.
//!
//! Every event is native on it: the replay calls the hooks before it
//! applies each recorded step.

pub mod conversation;

use coder_delegate::session::HookBackend;
use serde::{Deserialize, Serialize};

use super::compose::{Applied, Refused, Runner, check_backend};
use super::events::{Call, Event};
use super::outcome::{Permission, Stop};

/// The schema of a recorded session.
pub const RECORDED_SESSION_SCHEMA: &str = "openagents.coder-sdk.recorded-session.v1";

/// A recorded session: its events in order, with their payloads.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Recorded {
    pub schema: String,
    #[serde(default)]
    pub session_id: Option<String>,
    pub events: Vec<Call>,
}

/// What a replay did.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Replay {
    pub session_id: Option<String>,
    /// Each replayed event, with the hooks' outcome as applied.
    pub applied: Vec<Applied>,
    /// Recorded events not replayed: a denied tool's results, and
    /// everything after a hook ended the session.
    pub skipped: usize,
    /// Whether a hook ended the session before the recording did.
    pub ended_by_hook: bool,
}

/// A recorded session, ready to replay.
#[derive(Clone, Debug, PartialEq)]
pub struct ScriptedSession {
    pub recorded: Recorded,
}

impl ScriptedSession {
    /// Reads a recorded session.
    ///
    /// # Errors
    ///
    /// When the text isn't a recorded session.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let recorded: Recorded = serde_json::from_str(text)
            .map_err(|error| format!("This isn't a recorded session: {error}"))?;
        if recorded.schema != RECORDED_SESSION_SCHEMA {
            return Err(format!(
                "This recorded session has the unknown format {}.",
                recorded.schema
            ));
        }
        Ok(ScriptedSession { recorded })
    }

    /// Replays the session through `runner`'s hooks. A denied tool's
    /// `PostToolUse` is skipped, since the tool never ran; a hook that ends
    /// the session ends the replay.
    ///
    /// # Errors
    ///
    /// The hooks this backend can't honor, before anything replays.
    pub fn replay(&self, runner: &mut Runner) -> Result<Replay, Vec<Refused>> {
        check_backend(&runner.registry, HookBackend::Scripted)?;
        let mut applied = Vec::new();
        let mut skipped = 0;
        let mut ended_by_hook = false;
        let mut denied: Option<String> = None;
        let events = &self.recorded.events;
        for (index, call) in events.iter().enumerate() {
            if call.event == Event::PostToolUse
                && let Some(tool) = denied.take()
                && call.tool.as_deref() == Some(tool.as_str())
            {
                skipped += 1;
                continue;
            }
            let outcome = runner.fire(call);
            if call.event == Event::PreToolUse {
                // A replay has no one to answer an ask, so the tool doesn't run.
                denied = matches!(
                    outcome.permission,
                    Permission::Deny { .. } | Permission::Ask { .. }
                )
                .then(|| call.tool.clone())
                .flatten();
            }
            let end = outcome.stop == Stop::End && call.event != Event::SessionEnd;
            applied.push(outcome);
            if end {
                ended_by_hook = true;
                skipped += events.len() - index - 1;
                break;
            }
        }
        Ok(Replay {
            session_id: self.recorded.session_id.clone(),
            applied,
            skipped,
            ended_by_hook,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::super::compose::Budget;
    use super::super::outcome::{Context, HookOutcome};
    use super::super::registry::{Hook, Registry, Source};
    use super::*;

    fn recording() -> String {
        json!({
            "schema": RECORDED_SESSION_SCHEMA,
            "session_id": "s-1",
            "events": [
                {"event": "SessionStart", "input": {"cwd": "/work"}},
                {"event": "PreToolUse", "tool": "Bash", "input": {"command": "cargo test"}},
                {"event": "PostToolUse", "tool": "Bash", "input": {"output": "ok"}},
                {"event": "PreToolUse", "tool": "Bash", "input": {"command": "rm -rf /"}},
                {"event": "PostToolUse", "tool": "Bash", "input": {"output": "gone"}},
                {"event": "Stop", "input": null},
                {"event": "SessionEnd", "input": null},
            ],
        })
        .to_string()
    }

    fn hooks() -> Registry {
        let mut registry = Registry::new();
        registry
            .register(
                Hook::floor(
                    "floors",
                    Event::PreToolUse,
                    Permission::ask("confirm the command"),
                    |call: &Call| {
                        let command = call.input["command"].as_str().unwrap_or_default();
                        Ok(if command.starts_with("rm -rf /") {
                            HookOutcome::deny("deletes the whole disk")
                        } else {
                            HookOutcome::none()
                        })
                    },
                )
                .matching("Bash"),
            )
            .unwrap();
        registry
            .register(Hook::new(
                "trust",
                Event::PreToolUse,
                Source::Command,
                |_| Ok(HookOutcome::allow()),
            ))
            .unwrap();
        registry
            .register(Hook::new(
                "squeeze",
                Event::PostToolUse,
                Source::BuiltIn,
                |call: &Call| {
                    let output = call.input["output"].as_str().unwrap_or_default();
                    Ok(HookOutcome::context(Context::new(
                        "squeeze",
                        &format!("the command printed {output}"),
                        1,
                    )))
                },
            ))
            .unwrap();
        registry
            .register(Hook::new("done", Event::Stop, Source::Decision, |_| {
                Ok(HookOutcome::stopping(Stop::End))
            }))
            .unwrap();
        registry
    }

    #[test]
    fn a_recorded_session_runs_through_the_hooks_and_shows_each_outcome() {
        let session = ScriptedSession::from_json(&recording()).unwrap();
        let mut runner = Runner::new(hooks(), Budget::default());
        let replay = session.replay(&mut runner).unwrap();
        assert_eq!(replay.session_id.as_deref(), Some("s-1"));
        let seen: Vec<Event> = replay.applied.iter().map(|a| a.event).collect();
        assert_eq!(
            seen,
            [
                Event::SessionStart,
                Event::PreToolUse,
                Event::PostToolUse,
                Event::PreToolUse,
                Event::Stop,
            ]
        );
        // The first command is allowed; the floor denies the second, over
        // the allow.
        assert_eq!(replay.applied[1].permission, Permission::Allow);
        assert_eq!(
            replay.applied[3].permission,
            Permission::deny("deletes the whole disk")
        );
        assert_eq!(replay.applied[3].decided_by.as_deref(), Some("floors"));
        // The tool that ran adds context; the denied one's output is skipped.
        assert_eq!(
            replay.applied[2].context,
            [Context::new("squeeze", "the command printed ok", 1)]
        );
        // The stop hook ends the session, so SessionEnd is not replayed.
        assert_eq!(replay.applied[4].stop, Stop::End);
        assert!(replay.ended_by_hook);
        assert_eq!(replay.skipped, 2);
    }

    #[test]
    fn an_unknown_recording_is_refused() {
        assert!(ScriptedSession::from_json("{}").is_err());
        let other = json!({"schema": "other", "events": []}).to_string();
        assert!(ScriptedSession::from_json(&other).is_err());
    }
}
