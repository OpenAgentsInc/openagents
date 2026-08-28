//! The `openagents swarm` command surface, and the thin clap types that
//! route to it.
//!
//! The swarm's substance lives in [`crate::swarm`]; this module is the
//! command layer — clap args, human rendering beside the `--json` documents,
//! and refusals that say what to do instead. Kept apart from `swarm.rs` so
//! the library half stays free of the presentation it does not need.

use clap::{Args, Subcommand};

#[derive(Args, Debug)]
pub struct SwarmArgs {
    #[command(subcommand)]
    pub action: SwarmAction,
}

#[derive(Subcommand, Debug)]
pub enum SwarmAction {
    /// List the sessions registered on this machine, live and stale
    List,
    /// Show the parent/child tree of registered sessions
    Tree,
    /// Read a session's inbox (defaults to the most recently active)
    Inbox {
        #[arg(help = "Session id; omit to read the newest session's inbox")]
        session: Option<String>,
        #[arg(long, help = "Also stamp the messages read")]
        drain: bool,
    },
    /// Deliver a message to another session (the sender is `human` unless --from names a session)
    Send {
        #[arg(help = "Destination: a session id, role:children-of:<id>, or all")]
        to: String,
        #[arg(help = "The message body")]
        body: String,
        #[arg(
            long,
            help = "Message kind: question, answer, status, handoff, or broadcast",
            default_value = "status"
        )]
        kind: String,
        #[arg(long, help = "The message id this one answers or continues")]
        thread: Option<String>,
        #[arg(long, help = "The receiving agent may spend a turn answering")]
        reply_expected: bool,
        #[arg(long, help = "Send as this registered session instead of `human`")]
        from: Option<String>,
    },
    /// Deliver one broadcast to every live session, or to one parent's children
    Broadcast {
        #[arg(help = "The message body")]
        body: String,
        #[arg(
            long,
            help = "Limit the fan-out: children-of:<parent-id>. Omit to reach every other live session."
        )]
        role: Option<String>,
        #[arg(long, help = "Send as this registered session instead of `human`")]
        from: Option<String>,
    },
    /// Stop injecting messages from one session; they stay unread
    Mute {
        #[arg(help = "Session id to silence")]
        session: String,
        #[arg(
            long,
            help = "The inbox that holds the mute list; omit for the newest session"
        )]
        inbox: Option<String>,
    },
    /// Resume injecting messages from a previously muted session
    Unmute {
        #[arg(help = "Session id to hear again")]
        session: String,
        #[arg(
            long,
            help = "The inbox that holds the mute list; omit for the newest session"
        )]
        inbox: Option<String>,
    },
    /// Repair a gapped inbox: keep the readable prefix, preserve the tail
    Repair {
        #[arg(help = "Session id; omit to repair the newest session's inbox")]
        session: Option<String>,
        #[arg(
            long,
            help = "Confirm the truncation. Without this the command reports what it would do."
        )]
        yes: bool,
    },
}

/// Run one `swarm` subcommand.
pub async fn run_swarm(action: SwarmAction, json: bool) {
    match action {
        SwarmAction::List => list(json),
        SwarmAction::Tree => tree(json),
        SwarmAction::Inbox { session, drain } => inbox(session.as_deref(), drain, json),
        SwarmAction::Send {
            to,
            body,
            kind,
            thread,
            reply_expected,
            from,
        } => {
            send(
                &to,
                &body,
                &kind,
                thread.as_deref(),
                reply_expected,
                from.as_deref(),
                json,
            );
        }
        SwarmAction::Broadcast { body, role, from } => {
            let to = match role
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
            {
                Some(role) if role.starts_with("role:") => role.to_string(),
                Some(role) => format!("role:{role}"),
                None => "all".to_string(),
            };
            send(&to, &body, "broadcast", None, false, from.as_deref(), json);
        }
        SwarmAction::Mute { session, inbox } => mute(&session, inbox.as_deref(), true, json),
        SwarmAction::Unmute { session, inbox } => mute(&session, inbox.as_deref(), false, json),
        SwarmAction::Repair { session, yes } => repair(session.as_deref(), yes, json),
    }
}

fn list(json: bool) {
    let home = crate::auth::home_directory();
    let registrations = match crate::swarm::list(&home) {
        Ok(list) => list,
        Err(why) => crate::cli::fail(&why),
    };
    let rows: Vec<serde_json::Value> = registrations
        .iter()
        .map(|registration| {
            serde_json::json!({
                "session_id": registration.session_id,
                "state": registration.state().as_str(),
                "role": registration.role,
                "parent": registration.parent,
                "cwd": registration.cwd,
                "lane": registration.lane,
                "model": registration.model,
                "worktree": registration.worktree,
                "pid": registration.pid,
                "started_at_ms": registration.started_at_ms,
                "heartbeat_at_ms": registration.heartbeat_at_ms,
            })
        })
        .collect();
    let document = serde_json::json!({
        "schema": crate::swarm::LISTING_SCHEMA,
        "sessions": rows,
        "live": registrations
            .iter()
            .filter(|registration| registration.state() == crate::swarm::SwarmState::Live)
            .count(),
        "total": registrations.len(),
    });
    let human: Vec<String> = if registrations.is_empty() {
        vec!["No sessions are registered. Run `openagents coder` in a terminal and it registers itself.".to_string()]
    } else {
        registrations
            .iter()
            .map(|registration| {
                format!(
                    "{}  {}  {}  {}  {}",
                    registration.state().as_str(),
                    registration.session_id,
                    registration.role,
                    registration.lane,
                    registration.cwd,
                )
            })
            .collect()
    };
    crate::cli::emit(json, &document, &human);
}

fn tree(json: bool) {
    let home = crate::auth::home_directory();
    let registrations = match crate::swarm::list(&home) {
        Ok(list) => list,
        Err(why) => crate::cli::fail(&why),
    };
    // Children render under their parent, roots in start order. A child
    // whose parent is not registered still shows, at the top level — hiding
    // it would be discovery lying about the machine.
    fn render(
        session: &crate::swarm::Registration,
        depth: usize,
        registrations: &[crate::swarm::Registration],
        lines: &mut Vec<(String, serde_json::Value)>,
    ) {
        let indent = "  ".repeat(depth);
        lines.push((
            format!(
                "{indent}{}  {}  {}  {}",
                crate::swarm::SwarmState::Live.as_str(),
                session.session_id,
                session.lane,
                session.cwd,
            ),
            serde_json::json!({
                "session_id": session.session_id,
                "depth": depth,
                "state": session.state().as_str(),
            }),
        ));
        for child in registrations.iter().filter(|candidate| {
            candidate.role == "child"
                && candidate.parent.as_deref() == Some(session.session_id.as_str())
        }) {
            render(child, depth + 1, registrations, lines);
        }
    }
    let mut lines: Vec<(String, serde_json::Value)> = Vec::new();
    for registration in &registrations {
        if registration.role != "child" {
            render(registration, 0, &registrations, &mut lines);
        }
    }
    let document = serde_json::json!({
        "schema": crate::swarm::LISTING_SCHEMA,
        "tree": lines.iter().map(|(_, value)| value.clone()).collect::<Vec<_>>(),
    });
    let human: Vec<String> = if lines.is_empty() {
        vec!["No sessions are registered.".to_string()]
    } else {
        lines.into_iter().map(|(line, _)| line).collect()
    };
    crate::cli::emit(json, &document, &human);
}

fn inbox(session: Option<&str>, drain: bool, json: bool) {
    let home = crate::auth::home_directory();
    let session_id = match session {
        Some(id) => id.to_string(),
        None => match newest_session_id(&home) {
            Some(id) => id,
            None => crate::cli::fail("No sessions are registered, so there is no inbox to read."),
        },
    };
    let registration = match crate::swarm::load_registration(&home, &session_id) {
        Ok(Some(registration)) => registration,
        Ok(None) => crate::cli::fail(&format!("No session `{session_id}` is registered.")),
        Err(why) => crate::cli::fail(&why),
    };
    let directory = inbox_directory(&registration);
    let messages = match crate::swarm::read_inbox(&directory) {
        Ok(messages) => messages,
        Err(why) => crate::cli::fail(&why),
    };
    if drain {
        let through = messages
            .iter()
            .filter_map(|message| message.sequence)
            .max()
            .unwrap_or_default();
        if let Err(why) = crate::swarm::Mailbox::at(&directory).mark_read_through(through) {
            crate::cli::fail(&why);
        }
    }
    let document = serde_json::json!({
        "schema": "openagents.swarm.inbox.v1",
        "session_id": session_id,
        "drained": drain,
        "messages": messages.iter().map(message_document).collect::<Vec<_>>(),
    });
    let human: Vec<String> = if messages.is_empty() {
        vec![format!("The inbox of {session_id} is empty.")]
    } else {
        messages
            .iter()
            .map(|message| {
                format!(
                    "#{} [{}] from {}  {}{}",
                    message
                        .sequence
                        .map(|sequence| sequence.to_string())
                        .unwrap_or_else(|| "?".to_string()),
                    message.kind,
                    message.from,
                    message.id,
                    match message.read_at_ms {
                        Some(_) => String::new(),
                        None => "  · unread".to_string(),
                    },
                )
            })
            .chain(std::iter::once(String::new()))
            .chain(messages.iter().map(|message| message.body.clone()))
            .collect()
    };
    crate::cli::emit(json, &document, &human);
}

#[allow(clippy::too_many_arguments)]
fn send(
    to: &str,
    body: &str,
    kind: &str,
    thread: Option<&str>,
    reply_expected: bool,
    from: Option<&str>,
    json: bool,
) {
    let home = crate::auth::home_directory();
    // The sender is a registered session (whose own store directory keeps the
    // outbox), or `human` — the operator at a terminal. A human send records
    // its outbox line under the swarm root's `human/` directory, so the
    // outbox always has a home without pretending a human is a session.
    let (from_id, from_directory) = match from {
        Some(id) => {
            let registration = match crate::swarm::load_registration(&home, id) {
                Ok(Some(registration)) => registration,
                Ok(None) => crate::cli::fail(&format!(
                    "No session `{id}` is registered, so it cannot be the sender."
                )),
                Err(why) => crate::cli::fail(&why),
            };
            (id.to_string(), inbox_directory(&registration))
        }
        None => {
            let directory = crate::swarm::swarm_root(&home).join("human");
            let _ = std::fs::create_dir_all(&directory);
            ("human".to_string(), directory)
        }
    };
    let report = match crate::swarm::send(
        &home,
        &from_id,
        &from_directory,
        to,
        kind,
        thread,
        reply_expected,
        body,
        None,
        None,
    ) {
        Ok(report) => report,
        Err(why) => crate::cli::fail(&why),
    };
    let document = serde_json::json!({
        "schema": "openagents.swarm.send_report.v1",
        "from": report.from,
        "to": report.to,
        "kind": report.kind,
        "thread": report.thread,
        "message_id": report.message_id,
        "budget_remaining": report.budget_remaining,
        "reply_depth_remaining": report.reply_depth_remaining,
        "deliveries": report.deliveries,
        "undeliverable": report.undeliverable,
    });
    let mut human = Vec::new();
    for delivery in &report.deliveries {
        human.push(format!(
            "Delivered {} to {} (sequence {}).",
            report.message_id, delivery.to, delivery.sequence
        ));
    }
    for missed in &report.undeliverable {
        human.push(format!("Not delivered to {}: {}.", missed.to, missed.why));
    }
    if report.deliveries.is_empty() {
        human.push("No recipient accepted the message.".to_string());
    }
    crate::cli::emit(json, &document, &human);
}

/// The confirmation-gated repair for a gapped inbox (#280). Without `--yes`
/// the command reports what it would truncate and stops; with it, the tail
/// is preserved at inbox-quarantined.jsonl and the readable prefix stays.
fn repair(session: Option<&str>, yes: bool, json: bool) {
    let home = crate::auth::home_directory();
    let session_id = match session {
        Some(id) => id.to_string(),
        None => match newest_session_id(&home) {
            Some(id) => id,
            None => crate::cli::fail("No sessions are registered, so there is no inbox to repair."),
        },
    };
    let registration = match crate::swarm::load_registration(&home, &session_id) {
        Ok(Some(registration)) => registration,
        Ok(None) => crate::cli::fail(&format!("No session `{session_id}` is registered.")),
        Err(why) => crate::cli::fail(&why),
    };
    let directory = inbox_directory(&registration);
    match crate::swarm::inbox_quarantine(&directory) {
        Ok((_, None)) => crate::cli::fail(
            "this inbox has no gap, so there is nothing to repair: refusing to rewrite healthy mail",
        ),
        Ok((readable, Some(notice))) => {
            if !yes {
                let document = serde_json::json!({
                    "session_id": session_id,
                    "would_truncate_after_sequence": notice.missing_after_sequence,
                    "quarantined_count": notice.quarantined_count,
                    "first_quarantined_id": notice.first_quarantined_id,
                    "first_quarantined_from": notice.first_quarantined_from,
                    "confirmed": false,
                    "hint": "rerun with --yes to perform the repair",
                });
                let human = vec![format!(
                    "The inbox of {session_id} gaps at sequence {}. {} message(s) sit past the gap, the first from {}. Rerun with --yes to keep sequences 1..{} and preserve the tail at inbox-quarantined.jsonl.",
                    notice.missing_after_sequence,
                    notice.quarantined_count,
                    notice.first_quarantined_from,
                    notice.missing_after_sequence
                )];
                crate::cli::emit(json, &document, &human);
                return;
            }
            match crate::swarm::repair_inbox(&directory) {
                Ok(report) => {
                    let document = serde_json::json!({
                        "session_id": session_id,
                        "confirmed": true,
                        "truncated_after_sequence": report.truncated_after_sequence,
                        "quarantined_count": report.quarantined_count,
                        "preserved_at": report.preserved_at,
                        "readable_kept": readable.len(),
                    });
                    let human = vec![format!(
                        "Repaired {session_id}: kept sequences 1..{}, quarantined {} message(s) to {}.",
                        report.truncated_after_sequence,
                        report.quarantined_count,
                        report.preserved_at
                    )];
                    crate::cli::emit(json, &document, &human);
                }
                Err(why) => crate::cli::fail(&why),
            }
        }
        Err(why) => crate::cli::fail(&why),
    }
}

fn mute(session: &str, inbox: Option<&str>, silencing: bool, json: bool) {
    let home = crate::auth::home_directory();
    let owner = match inbox {
        Some(id) => id.to_string(),
        None => match newest_session_id(&home) {
            Some(id) => id,
            None => {
                crate::cli::fail("No sessions are registered, so there is no mute list to write.")
            }
        },
    };
    let registration = match crate::swarm::load_registration(&home, &owner) {
        Ok(Some(registration)) => registration,
        Ok(None) => crate::cli::fail(&format!("No session `{owner}` is registered.")),
        Err(why) => crate::cli::fail(&why),
    };
    let directory = inbox_directory(&registration);
    let mut muted = crate::swarm::load_mute_list(&directory);
    if silencing {
        muted.insert(session.to_string());
    } else {
        muted.remove(session);
    }
    if let Err(why) = crate::swarm::save_mute_list(&directory, &muted) {
        crate::cli::fail(&why);
    }
    let verb = if silencing { "Muted" } else { "Unmuted" };
    let document = serde_json::json!({
        "schema": crate::swarm::MUTE_SCHEMA,
        "owner": owner,
        "session": session,
        "muted": silencing,
        "list": muted.iter().collect::<Vec<_>>(),
    });
    crate::cli::emit(
        json,
        &document,
        &[format!("{verb} {session} on {owner}'s inbox.")],
    );
}

fn inbox_directory(registration: &crate::swarm::Registration) -> std::path::PathBuf {
    crate::swarm::inbox_directory(registration)
}

fn message_document(message: &crate::swarm::SwarmMessage) -> serde_json::Value {
    serde_json::json!({
        "id": message.id,
        "sequence": message.sequence,
        "from": message.from,
        "kind": message.kind,
        "thread": message.thread,
        "reply_expected": message.reply_expected,
        "body": message.body,
        "delivered_at_ms": message.delivered_at_ms,
        "read_at_ms": message.read_at_ms,
    })
}

/// The registration with the newest heartbeat — the inbox a bare
/// `swarm inbox` most plausibly means.
fn newest_session_id(home: &std::path::Path) -> Option<String> {
    crate::swarm::list(home)
        .ok()?
        .into_iter()
        .max_by_key(|registration| registration.heartbeat_at_ms)
        .map(|registration| registration.session_id)
}
