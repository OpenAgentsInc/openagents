//! `openagents lease grant machine`, `revoke machine`, `actions`, `prune`,
//! and `stop`: the owner's grants of machine actions to agent IDs, and the
//! logged actions they allow (`coder_lease::machine`, #12033).

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;

use coder_lease::machine::{self, Action, Grants, Outcome, Scope};
use serde_json::json;

use crate::Output;
use crate::lease::USAGE;

/// `lease grant machine AGENT --scope SCOPES [--for DURATION]`, with
/// `words` after `grant`.
pub(crate) fn grant(output: Output, words: &[String]) -> u8 {
    let args = match crate::argv::parse_command(
        &words[1..],
        "lease grant machine",
        &["scope", "for"],
        &[],
        1,
        1,
    ) {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let agent = args.positional()[0].clone();
    let mut scopes = Vec::new();
    for word in args
        .options("scope")
        .iter()
        .flat_map(|list| list.split(','))
    {
        match Scope::parse(word) {
            Some(scope) => scopes.push(scope),
            None => {
                return output.usage(
                    "lease",
                    &format!("`{word}` isn't a machine scope; use build.prune, process.stop, or host.upgrade"),
                    USAGE,
                );
            }
        }
    }
    if scopes.is_empty() {
        return output.usage("lease", "a machine grant needs --scope", USAGE);
    }
    let duration = match args.option("for").map(coder_lease::parse_duration) {
        None => None,
        Some(Ok(duration)) => Some(duration),
        Some(Err(message)) => return output.usage("lease", &message, USAGE),
    };
    if let Some(reason) = coder_lease::grant_refusal_for(
        "a machine grant",
        std::io::stdin().is_terminal(),
        &|name| std::env::var(name).ok(),
        &coder_lease::ancestors,
    ) {
        return output.fail("lease", &reason);
    }
    let (root, _) = match machine::here() {
        Ok(here) => here,
        Err(message) => return output.fail("lease", &message),
    };
    let names: Vec<&str> = scopes.iter().map(|scope| scope.word()).collect();
    let until = duration.map_or_else(
        || "until you revoke it".to_owned(),
        |duration| format!("for {}", duration_words(duration.as_secs())),
    );
    eprint!(
        "Let agent {agent} use {} on this computer {until}? Every use is logged. Type yes to confirm: ",
        names.join(", ")
    );
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err()
        || !matches!(answer.trim().to_ascii_lowercase().as_str(), "yes" | "y")
    {
        return output.fail("lease", "not granted");
    }
    match machine::grant(&root, &agent, &scopes, duration) {
        Ok(granted) => {
            output.emit(&json!({ "granted": granted }), |_| {
                format!("Granted {} to {agent} {until}.", names.join(", "))
            });
            0
        }
        Err(message) => output.fail("lease", &message),
    }
}

/// `lease revoke machine AGENT`, with `words` after `revoke`.
pub(crate) fn revoke(output: Output, words: &[String]) -> u8 {
    let args = match crate::argv::parse_command(&words[1..], "lease revoke machine", &[], &[], 1, 1)
    {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let agent = &args.positional()[0];
    let result = machine::here().and_then(|(root, _)| machine::revoke(&root, agent));
    match result {
        Ok(revoked) => {
            output.emit(&json!({ "revoked": revoked }), |_| {
                if revoked {
                    format!("Revoked {agent}'s machine grant.")
                } else {
                    format!("{agent} held no machine grant.")
                }
            });
            0
        }
        Err(message) => output.fail("lease", &message),
    }
}

/// `lease actions [--limit N]`: the grants and the recent log.
pub(crate) fn actions(output: Output, words: &[String]) -> u8 {
    let args = match crate::argv::parse_command(words, "lease actions", &["limit"], &[], 0, 0) {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let limit = match args.number("limit", 20usize) {
        Ok(limit) => limit,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let (root, _) = match machine::here() {
        Ok(here) => here,
        Err(message) => return output.fail("lease", &message),
    };
    let grants = match Grants::load(&root) {
        Ok(grants) => grants,
        Err(message) => return output.fail("lease", &message),
    };
    let entries = match machine::read_log(&root, limit) {
        Ok(entries) => entries,
        Err(error) => return output.fail("lease", &error.to_string()),
    };
    let now = coder_lease::now_ms();
    output.emit(
        &json!({ "grants": grants.grants, "entries": entries }),
        |_| {
            let mut lines = vec!["Machine grants:".to_owned()];
            if grants.grants.is_empty() {
                lines.push("  none".to_owned());
            }
            for grant in &grants.grants {
                let scopes: Vec<&str> = grant.scopes.iter().map(|scope| scope.word()).collect();
                let left = match grant.expires_at_ms {
                    None => "no end".to_owned(),
                    Some(end) if end <= now => "ended".to_owned(),
                    Some(end) => format!("{} left", duration_words((end - now) / 1000)),
                };
                lines.push(format!(
                    "  {}  {}  ({left})",
                    grant.agent,
                    scopes.join(", ")
                ));
            }
            lines.push("Recent actions:".to_owned());
            if entries.is_empty() {
                lines.push("  none".to_owned());
            }
            lines.extend(entries.iter().map(|entry| format!("  {}", entry.line())));
            lines.join("\n")
        },
    );
    0
}

/// `lease prune PATH --reason TEXT` and `lease stop PID --reason TEXT`,
/// with `words` after the verb, for the agent this command runs under.
pub(crate) fn act(output: Output, verb: &str, words: &[String]) -> u8 {
    let command = format!("lease {verb}");
    let args = match crate::argv::parse_command(words, &command, &["reason"], &[], 1, 1) {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let target = &args.positional()[0];
    let action = if verb == "stop" {
        match target.parse::<u32>() {
            Ok(pid) => Action::Stop(pid),
            Err(_) => return output.usage("lease", &format!("`{target}` isn't a pid"), USAGE),
        }
    } else {
        let path = PathBuf::from(target);
        let path = if path.is_absolute() {
            path
        } else {
            std::env::current_dir().map_or(path.clone(), |cwd| cwd.join(path))
        };
        Action::Prune(path)
    };
    let agent = agent_id();
    let entry = match machine::here().and_then(|(root, layout)| {
        machine::act(
            &root,
            &layout,
            &machine::System,
            &agent,
            &action,
            args.option("reason").unwrap_or(""),
        )
    }) {
        Ok(entry) => entry,
        Err(message) => return output.fail("lease", &message),
    };
    let line = entry.line();
    if entry.outcome == Outcome::Done {
        output.emit(&json!({ "entry": entry }), |_| line);
        0
    } else {
        output.fail("lease", &line)
    }
}

/// The agent this command runs for: the app chat's ID when the engine
/// set it, else the session the lease broker finds.
fn agent_id() -> String {
    std::env::var("OPENAGENTS_CHAT_ID")
        .ok()
        .map(|id| id.trim().to_owned())
        .filter(|id| !id.is_empty())
        .unwrap_or_else(|| {
            coder_lease::Session::detect(&|name| std::env::var(name).ok(), &coder_lease::ancestors)
                .id
        })
}

fn duration_words(seconds: u64) -> String {
    let minutes = seconds / 60;
    if minutes >= 48 * 60 {
        format!("{}d", minutes / (24 * 60))
    } else if minutes >= 120 {
        format!("{}h", minutes / 60)
    } else {
        format!("{}m", minutes.max(1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grant_lengths_read_in_minutes_hours_and_days() {
        assert_eq!(duration_words(30), "1m");
        assert_eq!(duration_words(90 * 60), "90m");
        assert_eq!(duration_words(8 * 3600), "8h");
        assert_eq!(duration_words(3 * 86_400), "3d");
    }
}
