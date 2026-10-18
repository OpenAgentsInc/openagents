//! The `host.upgrade` scope (#12033's machine scopes): only an agent granted
//! it may start an upgrade, the owner takes it away with `openagents lease
//! revoke machine AGENT`, and every request an agent makes, refused or not,
//! is a line in the machine-action log beside the upgrade's own log.

use coder_lease::machine::{self, Entry, Grants, Scope};

/// Who asks for an upgrade: the agent named, else the agent this command
/// runs under (the app chat's ID, else its session), else `None` for the
/// owner at a terminal.
#[must_use]
pub fn requester(named: Option<&str>) -> Option<String> {
    if let Some(agent) = named.map(str::trim).filter(|agent| !agent.is_empty()) {
        return Some(agent.to_owned());
    }
    let env = |name: &str| std::env::var(name).ok().filter(|value| !value.is_empty());
    if let Some(chat) = env("OPENAGENTS_CHAT_ID") {
        return Some(chat.trim().to_owned());
    }
    // An agent runs this if a grant made here would be refused for that.
    coder_lease::grant_refusal_for("", true, &env, &coder_lease::ancestors)
        .map(|_| coder_lease::Session::detect(&env, &coder_lease::ancestors).id)
}

/// Whether `agent` may start an upgrade now. A refusal is logged.
///
/// # Errors
/// The agent holds no live `host.upgrade` grant, or the grants can't be read.
pub fn check(agent: &str, reason: &str) -> Result<(), String> {
    let (root, _) = machine::here()?;
    let grants = Grants::load(&root)?;
    check_in(&root, &grants, agent, reason)
}

fn check_in(
    root: &std::path::Path,
    grants: &Grants,
    agent: &str,
    reason: &str,
) -> Result<(), String> {
    if grants.allows(agent, Scope::HostUpgrade, crate::now_ms()) {
        return Ok(());
    }
    let mut entry = Entry::new(&machine::System, agent, Scope::HostUpgrade, reason);
    entry.outcome = machine::Outcome::Refused;
    entry.command = Some("host-upgrade".into());
    entry.detail = format!(
        "{agent} holds no host.upgrade grant; ask the owner to run `openagents lease grant machine {agent} --scope upgrade`"
    );
    let _ = machine::log(root, &entry);
    Err(entry.detail)
}

/// Log a finished upgrade an agent asked for in the machine-action log.
pub fn record(agent: &str, reason: &str, upgrade: &crate::LogEntry) {
    if let Ok((root, _)) = machine::here() {
        let _ = machine::log(&root, &entry_for(agent, reason, upgrade));
    }
}

fn entry_for(agent: &str, reason: &str, upgrade: &crate::LogEntry) -> Entry {
    let mut entry = Entry::new(&machine::System, agent, Scope::HostUpgrade, reason);
    entry.outcome = match upgrade.outcome {
        crate::Outcome::Installed | crate::Outcome::UpToDate | crate::Outcome::Waiting => {
            machine::Outcome::Done
        }
        crate::Outcome::Refused => machine::Outcome::Refused,
        _ => machine::Outcome::Failed,
    };
    entry.command = Some("host-upgrade".into());
    entry.detail = format!("{}: {}", upgrade.id, upgrade.summary());
    entry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_granted_agent_may_upgrade_and_refusals_are_logged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let err = check_in(root, &Grants::default(), "chat-1", "newest main").unwrap_err();
        assert!(err.contains("holds no host.upgrade grant"), "{err}");
        let log = machine::read_log(root, 10).unwrap();
        assert_eq!(log.len(), 1);
        assert_eq!(
            (log[0].scope, log[0].outcome, log[0].agent.as_str()),
            (Scope::HostUpgrade, machine::Outcome::Refused, "chat-1")
        );

        machine::grant(root, "chat-1", &[Scope::HostUpgrade], None).unwrap();
        let grants = Grants::load(root).unwrap();
        assert!(check_in(root, &grants, "chat-1", "newest main").is_ok());
        assert!(check_in(root, &grants, "chat-2", "newest main").is_err());
        // Another scope isn't this one.
        machine::grant(root, "chat-3", &[Scope::BuildPrune], None).unwrap();
        let grants = Grants::load(root).unwrap();
        assert!(check_in(root, &grants, "chat-3", "newest main").is_err());

        // The owner takes it back.
        machine::revoke(root, "chat-1").unwrap();
        let grants = Grants::load(root).unwrap();
        assert!(check_in(root, &grants, "chat-1", "newest main").is_err());
    }

    #[test]
    fn finished_upgrades_log_their_outcome() {
        let upgrade = crate::LogEntry::new(
            "u1",
            0,
            "chat-1",
            "newest main",
            crate::Outcome::BuildFailed,
            "error[E0425]",
        );
        let entry = entry_for("chat-1", "newest main", &upgrade);
        assert_eq!(entry.outcome, machine::Outcome::Failed);
        assert!(
            entry.detail.starts_with("u1: build failed"),
            "{}",
            entry.detail
        );
    }
}
