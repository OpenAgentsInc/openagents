//! `openagents lease grant machine`, `revoke machine`, `actions`, `prune`,
//! and `stop`: the owner's grants of machine actions to agent IDs, and the
//! logged actions they allow (`coder_lease::machine`, #12033), plus
//! `subgrant` and `subrevoke` for an agent holding the owner's `delegate`
//! (#12163), which reach the chat's own computer through the app.

use std::io::{BufRead as _, IsTerminal as _, Write as _};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use coder_lease::machine::{self, Action, Grants, Outcome, Scope};
use serde_json::{Value, json};

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
                    &format!("`{word}` isn't a machine scope; use build.prune, process.stop, host.upgrade, or delegate"),
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
    let delegating = scopes.contains(&Scope::Delegate);
    let reach = if delegating {
        "and to grant build.prune, process.stop and host.upgrade to other chats on any of your computers"
    } else {
        "on this computer"
    };
    eprint!(
        "Let agent {agent} use {} {reach} {until}? Every use is logged. Type yes to confirm: ",
        names.join(", ")
    );
    let _ = std::io::stderr().flush();
    let mut answer = String::new();
    if std::io::stdin().lock().read_line(&mut answer).is_err()
        || !matches!(answer.trim().to_ascii_lowercase().as_str(), "yes" | "y")
    {
        return output.fail("lease", "not granted");
    }
    let granted = match machine::grant(&root, &agent, &scopes, duration) {
        Ok(granted) => granted,
        Err(message) => return output.fail("lease", &message),
    };
    // `delegate` is checked on the coordinator's own computer: send it
    // there when that's another one, so it runs once on any computer.
    let elsewhere = if delegating {
        Some(send_delegate_home(&agent, duration))
    } else {
        None
    };
    output.emit(
        &json!({ "granted": granted, "chatHost": elsewhere }),
        |_| {
            let mut text = format!("Granted {} to {agent} {until}.", names.join(", "));
            if let Some(note) = &elsewhere {
                text.push(' ');
                text.push_str(note);
            }
            text
        },
    );
    0
}

/// Sends the owner's `delegate` grant to the chat `agent`'s own computer
/// when that's not this one; what happened, in words.
fn send_delegate_home(agent: &str, duration: Option<Duration>) -> String {
    use coder_lease::machine::delegate::GrantRequest;
    let request = GrantRequest {
        agent: agent.to_owned(),
        scopes: vec![Scope::Delegate],
        for_secs: duration.map(|duration| duration.as_secs()),
        issuer: None,
        reason: "the owner granted delegate".to_owned(),
    };
    let result = in_app(|engine| async move {
        let chat = engine
            .resolve_chat(agent)
            .await
            .map_err(|e| format!("{e:#}"))?;
        let local = engine
            .local_device_id()
            .await
            .map_err(|e| format!("{e:#}"))?;
        if chat.device_id == local {
            return Ok("Its chat runs on this computer.".to_owned());
        }
        let mut request = request;
        request.agent = chat.id;
        let params = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        openagents_mcp::grant_machine::send(
            &engine,
            &chat.device_id,
            openagents_rpc::methods::GRANT_MACHINE,
            params,
        )
        .await?;
        Ok(format!(
            "Also granted on its chat's computer ({}).",
            chat.device_id
        ))
    });
    result.unwrap_or_else(|why| {
        format!("It holds it on this computer only; run this on the computer its chat runs on if that's another one ({why}).")
    })
}

/// Runs `work` against this computer's OpenAgents app engine, for at most
/// a minute.
fn in_app<F, Fut>(work: F) -> Result<String, String>
where
    F: FnOnce(Arc<openagents_mcp::OpenAgents>) -> Fut,
    Fut: std::future::Future<Output = Result<String, String>>,
{
    let port = openagents_brand::env_var("OPENAGENTS_IPC_PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .filter(|port| *port != 0)
        .unwrap_or(openagents_brand::DEFAULT_IPC_PORT);
    let engine = Arc::new(openagents_mcp::OpenAgents::new(
        format!("ws://127.0.0.1:{port}"),
        openagents_mcp::Origin::from_env(),
    ));
    crate::runtime().block_on(async {
        tokio::time::timeout(Duration::from_secs(60), work(engine))
            .await
            .unwrap_or_else(|_| Err("the OpenAgents app did not answer within 60 seconds".into()))
    })
}

/// `lease revoke machine AGENT`, with `words` after `revoke`.
pub(crate) fn revoke(output: Output, words: &[String]) -> u8 {
    let args = match crate::argv::parse_command(&words[1..], "lease revoke machine", &[], &[], 1, 1)
    {
        Ok(args) => args,
        Err(message) => return output.usage("lease", &message, USAGE),
    };
    let agent = args.positional()[0].clone();
    let revoked = match machine::here().and_then(|(root, _)| machine::revoke(&root, &agent)) {
        Ok(revoked) => revoked,
        Err(message) => return output.fail("lease", &message),
    };
    // What a delegate holder issued lives on each chat's own computer.
    let others = revoke_everywhere(&agent);
    output.emit(
        &json!({ "revoked": revoked, "otherComputers": others }),
        |_| {
            let mut text = if revoked {
                format!("Revoked {agent}'s machine grant and what it issued here.")
            } else {
                format!("{agent} held no machine grant here.")
            };
            for line in &others {
                text.push_str("\n  ");
                text.push_str(line);
            }
            text
        },
    );
    0
}

/// Ends `agent`'s grants and everything it issued on every other computer
/// the app knows; a line per computer.
fn revoke_everywhere(agent: &str) -> Vec<String> {
    use coder_lease::machine::delegate::RevokeRequest;
    let agent = agent.to_owned();
    let result = in_app(|engine| async move {
        let local = engine
            .local_device_id()
            .await
            .map_err(|e| format!("{e:#}"))?;
        let devices = engine.devices().await.map_err(|e| format!("{e:#}"))?;
        // A chat ID may be a prefix here; its full ID is what grants name.
        let agent = engine
            .resolve_chat(&agent)
            .await
            .map_or(agent, |chat| chat.id);
        let request = RevokeRequest {
            agent,
            issuer: None,
            reason: "the owner revoked it".to_owned(),
        };
        let params = serde_json::to_value(&request).map_err(|e| e.to_string())?;
        let sends = devices
            .iter()
            .filter(|device| device.id != local)
            .map(|device| {
                let (engine, params) = (engine.clone(), params.clone());
                async move {
                    let sent = openagents_mcp::grant_machine::send(
                        &engine,
                        &device.id,
                        openagents_rpc::methods::REVOKE_MACHINE,
                        params,
                    )
                    .await;
                    match sent {
                        Ok(reply) if reply["revoked"] == true => {
                            format!("{}: revoked", device.name)
                        }
                        Ok(_) => format!("{}: nothing to revoke", device.name),
                        Err(why) => format!("{}: not reached: {why}", device.name),
                    }
                }
            });
        Ok(futures_util::future::join_all(sends).await.join("\n"))
    });
    match result {
        Ok(lines) => lines.lines().map(str::to_owned).collect(),
        Err(why) => vec![format!("other computers not reached: {why}")],
    }
}

/// `lease subgrant CHAT --scope SCOPES --reason TEXT [--for DURATION]` and
/// `lease subrevoke CHAT --reason TEXT`, for the agent holding `delegate`
/// this command runs under: the app's `grant_machine` and `revoke_machine`
/// tools, from this chat.
pub(crate) fn subgrant(output: Output, verb: &str, words: &[String]) -> u8 {
    let command = format!("lease {verb}");
    let args =
        match crate::argv::parse_command(words, &command, &["scope", "reason", "for"], &[], 1, 1) {
            Ok(args) => args,
            Err(message) => return output.usage("lease", &message, USAGE),
        };
    let chat = args.positional()[0].clone();
    let reason = args.option("reason").unwrap_or("").to_owned();
    let (tool, call) = if verb == "subgrant" {
        let scopes: Vec<String> = args
            .options("scope")
            .iter()
            .flat_map(|list| list.split(','))
            .map(|word| {
                Scope::parse(word)
                    .map_or_else(|| word.trim().to_owned(), |scope| scope.word().to_owned())
            })
            .collect();
        let minutes = match args.option("for").map(coder_lease::parse_duration) {
            None => None,
            Some(Ok(duration)) => Some(duration.as_secs().div_ceil(60).max(1)),
            Some(Err(message)) => return output.usage("lease", &message, USAGE),
        };
        let mut call = json!({ "chat": chat, "scopes": scopes, "reason": reason });
        if let Some(minutes) = minutes {
            call["for_minutes"] = json!(minutes);
        }
        ("grant_machine", call)
    } else {
        ("revoke_machine", json!({ "chat": chat, "reason": reason }))
    };
    let mut reply = Value::Null;
    let result = in_app(|engine| {
        let reply = &mut reply;
        async move {
            *reply = openagents_mcp::Tools::new(engine).call(tool, call).await?;
            Ok(String::new())
        }
    });
    match result {
        Ok(_) => {
            output.emit(&reply, |reply| {
                let host = reply["deviceId"].as_str().unwrap_or("its computer");
                if verb == "subgrant" {
                    format!("Granted on {host}; it's in the machine-action log there and here.")
                } else if reply["revoked"] == true {
                    format!("Revoked on {host}.")
                } else {
                    format!("You held no grant to that chat on {host}.")
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
                let from = grant.issued_by.as_ref().map_or_else(String::new, |issuer| {
                    format!(
                        "  from {issuer}: {}",
                        grant.reason.as_deref().unwrap_or("no reason")
                    )
                });
                let left = match grant.expires_at_ms {
                    None => "no end".to_owned(),
                    Some(end) if end <= now => "ended".to_owned(),
                    Some(end) => format!("{} left", duration_words((end - now) / 1000)),
                };
                lines.push(format!(
                    "  {}  {}  ({left}){from}",
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
    let reason = args.option("reason").unwrap_or("");
    let entry = match machine::here().and_then(|(root, layout)| {
        let agent = caller(&root, action.scope(), reason)?;
        machine::act(&root, &layout, &machine::System, &agent, &action, reason)
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

/// The agent this command acts for: the app chat that proves it is that
/// chat (#12067), else the session the lease broker finds. The owner at a
/// terminal acts under their own session, never a chat's grant. A chat
/// named without its proof is refused and logged.
fn caller(root: &std::path::Path, scope: Scope, reason: &str) -> Result<String, String> {
    use machine::proof::{self, Caller};
    match proof::identify_here(root, None) {
        Ok(Caller::Chat(id) | Caller::Session(id)) => Ok(id),
        Ok(Caller::Owner) => Ok(coder_lease::Session::detect(
            &|name| std::env::var(name).ok(),
            &coder_lease::ancestors,
        )
        .id),
        Err(why) => {
            let named = std::env::var(proof::CHAT_VAR).unwrap_or_default();
            Err(machine::refuse_unproven(root, &named, scope, reason, why))
        }
    }
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
