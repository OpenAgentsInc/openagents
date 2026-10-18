//! The `host.upgrade` scope (#12033's machine scopes): only an agent granted
//! it may start an upgrade, the owner takes it away with `openagents lease
//! revoke machine AGENT`, and every request an agent makes, refused or not,
//! is a line in the machine-action log beside the upgrade's own log. An
//! agent proves it is the chat it names (#12067, [`machine::proof`]).

use coder_lease::machine::proof;
pub use coder_lease::machine::proof::{Caller, PROOF_VAR};
use coder_lease::machine::{self, Entry, Grants, Scope};

/// Who asks for an upgrade. `named` is the `--agent` flag. The owner at a
/// terminal is the owner, even naming a chat; an agent must carry the
/// proof of the chat it names, else it's the session its ancestry shows.
/// `owner_request` is the request ID a detached upgrade the owner started
/// carries, proven by the parent ([`detached_proof`]).
///
/// # Errors
/// A chat is named without its proof; the refusal is logged.
pub fn caller(
    named: Option<&str>,
    owner_request: Option<&str>,
    reason: &str,
) -> Result<Caller, String> {
    let (root, _) = machine::here()?;
    if let Some(id) = owner_request {
        let proof = std::env::var(proof::PROOF_VAR).unwrap_or_default();
        if proof::verify(&root, &owner_subject(id), &proof) {
            return Ok(Caller::Owner);
        }
        let why = format!(
            "refused: upgrade {id} says the owner started it, but carries no proof of that ({} is missing or wrong)",
            proof::PROOF_VAR
        );
        return Err(refused(&root, proof::OWNER, reason, why));
    }
    proof::identify_here(&root, named).map_err(|why| {
        let agent = named
            .map(str::to_owned)
            .or_else(|| std::env::var(proof::CHAT_VAR).ok())
            .unwrap_or_default();
        refused(&root, &agent, reason, why)
    })
}

/// The proof a detached upgrade carries in [`proof::PROOF_VAR`]: the
/// agent's own, or for the owner one bound to request `id`.
///
/// # Errors
/// The key can't be read.
pub fn detached_proof(caller: &Caller, id: &str) -> Result<String, String> {
    let (root, _) = machine::here()?;
    match caller {
        Caller::Owner => proof::proof_for(&root, &owner_subject(id)),
        Caller::Chat(agent) | Caller::Session(agent) => proof::proof_for(&root, agent),
    }
}

fn owner_subject(id: &str) -> String {
    format!("owner/host-upgrade/{id}")
}

fn refused(root: &std::path::Path, agent: &str, reason: &str, why: String) -> String {
    let mut entry = Entry::new(&machine::System, agent, Scope::HostUpgrade, reason);
    entry.outcome = machine::Outcome::Refused;
    entry.command = Some("host-upgrade".into());
    entry.detail.clone_from(&why);
    let _ = machine::log(root, &entry);
    why
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
