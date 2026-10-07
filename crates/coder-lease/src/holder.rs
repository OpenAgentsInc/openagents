//! Who holds a lease: the agent session, its kind, and the process.

use serde::{Deserialize, Serialize};

/// Names the session a process belongs to. Coder sets it for its
/// delegates, and a wrapped command inherits it.
pub const SESSION_VAR: &str = "OPENAGENTS_SESSION";

/// The variables an agent's own process sets, which mark an agent
/// environment: a screen grant is refused under any of them.
pub const AGENT_VARS: [&str; 10] = [
    "AI_AGENT",
    "CLAUDECODE",
    "CLAUDE_CODE_SESSION_ID",
    "CODEX_SANDBOX",
    "CODEX_THREAD_ID",
    "CODEX_SESSION_ID",
    "CURSOR_AGENT",
    "GEMINI_CLI",
    "OPENCODE",
    crate::LEASE_ID_VAR,
];

/// The process names of agents, matched against ancestor processes.
pub const AGENT_PROCESSES: [&str; 12] = [
    "claude",
    "codex",
    "coder",
    "microcoder",
    "devin",
    "opencode",
    "grok",
    "gemini",
    "cursor-agent",
    "aider",
    "goose",
    "amp",
];

/// The holder a lease records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Holder {
    /// The agent session: `OPENAGENTS_SESSION`, else an agent's own
    /// session variable, else the nearest agent ancestor process.
    pub session: String,
    /// The agent kind, such as `claude-code` or `codex`, or `none` when no
    /// agent was found.
    pub agent: String,
    /// The process that holds the lease.
    pub pid: u32,
    /// The command's name: its first word's file name, never its arguments.
    pub command: String,
}

impl Holder {
    /// This process as the holder of a lease for `command`, with the
    /// session found in this environment and its ancestors.
    #[must_use]
    pub fn detect(command: &str) -> Holder {
        let session = Session::detect(&|name| std::env::var(name).ok(), &ancestors);
        Holder {
            session: session.id,
            agent: session.agent,
            pid: std::process::id(),
            command: command_name(command),
        }
    }
}

/// A command's name for the record: the file name of its first word.
/// Arguments can carry secrets, so they are never recorded.
#[must_use]
pub fn command_name(command: &str) -> String {
    let first = command.split_whitespace().next().unwrap_or("");
    let name = std::path::Path::new(first)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.chars().take(64).collect()
}

/// An agent session and its kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Session {
    /// The session's identity.
    pub id: String,
    /// The agent kind, or `none`.
    pub agent: String,
}

impl Session {
    /// The session `env` names, else the nearest agent among `ancestors`
    /// (process identifiers and names, nearest first), else this process's
    /// parent. `ancestors` is called only when no variable names one.
    pub fn detect(
        env: &dyn Fn(&str) -> Option<String>,
        ancestors: &dyn Fn() -> Vec<(u32, String)>,
    ) -> Session {
        let set = |name: &str| env(name).filter(|value| !value.is_empty());
        let from_env = if set("CLAUDECODE").is_some() || set("CLAUDE_CODE_SESSION_ID").is_some() {
            Some("claude-code")
        } else if set("CODEX_THREAD_ID").is_some()
            || set("CODEX_SESSION_ID").is_some()
            || set("CODEX_SANDBOX").is_some()
        {
            Some("codex")
        } else if set("GEMINI_CLI").is_some() {
            Some("gemini")
        } else if set("CURSOR_AGENT").is_some() {
            Some("cursor")
        } else if set("OPENCODE").is_some() {
            Some("opencode")
        } else {
            None
        };
        let own = set("CLAUDE_CODE_SESSION_ID")
            .map(|id| format!("claude-code:{id}"))
            .or_else(|| {
                set("CODEX_THREAD_ID")
                    .or_else(|| set("CODEX_SESSION_ID"))
                    .map(|id| format!("codex:{id}"))
            });
        let mut agent = from_env.map(str::to_owned);
        let id = match (set(SESSION_VAR), own) {
            (Some(id), _) | (None, Some(id)) => id,
            (None, None) => {
                let found = agent_ancestor(&ancestors());
                if let Some((pid, name)) = &found {
                    agent.get_or_insert_with(|| name.clone());
                    format!("{name}:{pid}")
                } else {
                    format!("process:{}", parent_pid())
                }
            }
        };
        if agent.is_none() && set(SESSION_VAR).is_some() {
            agent = agent_ancestor(&ancestors()).map(|(_, name)| name);
        }
        Session {
            id,
            agent: agent.unwrap_or_else(|| "none".to_owned()),
        }
    }
}

/// The nearest ancestor whose process name is an agent's.
#[must_use]
pub fn agent_ancestor(ancestors: &[(u32, String)]) -> Option<(u32, String)> {
    ancestors.iter().find_map(|(pid, name)| {
        let base = name
            .rsplit('/')
            .next()
            .unwrap_or(name)
            .trim_start_matches('-');
        AGENT_PROCESSES
            .contains(&base)
            .then(|| (*pid, base.to_owned()))
    })
}

/// Why this environment may not grant the screen, or `None` when it may:
/// standard input must be a terminal, and no agent variable or agent
/// ancestor may be present.
pub fn grant_refusal(
    stdin_terminal: bool,
    env: &dyn Fn(&str) -> Option<String>,
    ancestors: &dyn Fn() -> Vec<(u32, String)>,
) -> Option<String> {
    if !stdin_terminal {
        return Some(
            "a screen grant needs the owner at an interactive terminal; standard input is not a terminal"
                .to_owned(),
        );
    }
    if let Some(name) = AGENT_VARS
        .iter()
        .find(|name| env(name).is_some_and(|value| !value.is_empty()))
    {
        return Some(format!(
            "a screen grant needs the owner, and {name} marks an agent environment"
        ));
    }
    if let Some((pid, name)) = agent_ancestor(&ancestors()) {
        return Some(format!(
            "a screen grant needs the owner, and this command runs under {name} (pid {pid})"
        ));
    }
    None
}

fn parent_pid() -> u32 {
    #[cfg(unix)]
    {
        // SAFETY: `getppid` has no preconditions.
        u32::try_from(unsafe { libc::getppid() }).unwrap_or(0)
    }
    #[cfg(not(unix))]
    {
        0
    }
}

/// This process's ancestors, nearest first, as process identifiers and
/// names, read through `ps`. Empty when `ps` is unavailable.
#[must_use]
pub fn ancestors() -> Vec<(u32, String)> {
    let mut found = Vec::new();
    #[cfg(unix)]
    {
        let mut pid = parent_pid();
        while pid > 1 && found.len() < 32 {
            let Ok(output) = std::process::Command::new("ps")
                .args(["-o", "ppid=", "-o", "comm=", "-p", &pid.to_string()])
                .stdin(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .output()
            else {
                break;
            };
            let text = String::from_utf8_lossy(&output.stdout);
            let line = text.trim();
            let Some((parent, name)) = line.split_once(char::is_whitespace) else {
                break;
            };
            found.push((pid, name.trim().to_owned()));
            match parent.trim().parse::<u32>() {
                Ok(parent) if parent != pid => pid = parent,
                _ => break,
            }
        }
    }
    found
}

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
    fn the_session_variable_comes_first_then_the_agents_own() {
        let none = || Vec::new();
        let session = Session::detect(
            &env(&[(SESSION_VAR, "studio-seat-3"), ("CLAUDECODE", "1")]),
            &none,
        );
        assert_eq!(session.id, "studio-seat-3");
        assert_eq!(session.agent, "claude-code");
        let session = Session::detect(&env(&[("CLAUDE_CODE_SESSION_ID", "abc")]), &none);
        assert_eq!(session.id, "claude-code:abc");
        let session = Session::detect(&env(&[("CODEX_THREAD_ID", "t1")]), &none);
        assert_eq!(session.id, "codex:t1");
        assert_eq!(session.agent, "codex");
    }

    #[test]
    fn otherwise_the_nearest_agent_ancestor_names_the_session() {
        let tree = || {
            vec![
                (40, "zsh".to_owned()),
                (30, "/opt/bin/codex".to_owned()),
                (20, "claude".to_owned()),
            ]
        };
        let session = Session::detect(&env(&[]), &tree);
        assert_eq!(session.id, "codex:30");
        assert_eq!(session.agent, "codex");
        let session = Session::detect(&env(&[]), &|| vec![(9, "-zsh".to_owned())]);
        assert!(session.id.starts_with("process:"));
        assert_eq!(session.agent, "none");
    }

    #[test]
    fn command_names_drop_arguments_and_directories() {
        assert_eq!(command_name("/usr/bin/cargo test --token s3cret"), "cargo");
        assert_eq!(command_name("scripts/grid-soak.sh"), "grid-soak.sh");
        assert_eq!(command_name(""), "");
    }

    #[test]
    fn a_grant_needs_a_terminal_and_no_agent() {
        let none = || Vec::new();
        assert!(grant_refusal(false, &env(&[]), &none).is_some());
        assert!(grant_refusal(true, &env(&[("CLAUDECODE", "1")]), &none).is_some());
        assert!(grant_refusal(true, &env(&[("AI_AGENT", "x")]), &none).is_some());
        let tree = || vec![(5, "claude".to_owned())];
        assert!(grant_refusal(true, &env(&[]), &tree).is_some());
        assert!(grant_refusal(true, &env(&[]), &none).is_none());
    }
}
