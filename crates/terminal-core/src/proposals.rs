//! Exact live-shell approvals and result acknowledgment. Recovery never replays input.

use crate::blocks::Block;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Binding {
    pub terminal: String,
    pub generation: String,
    pub cwd: String,
    /// Advisory logical PWD at the prompt; the OS cwd still binds admission.
    #[serde(default)]
    pub shell_directory: Option<String>,
    pub context_digest: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub thread: String,
    pub id: String,
    pub revision: u64,
    pub command: String,
    pub binding: Binding,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    /// The shared effect boundary classed the command read-only. Enter
    /// treats it as ordinary; only this class can auto-run
    /// ([`crate::autorun`]).
    ReadOnly,
    Ordinary,
    Destructive(String),
    Denied(String),
}

/// The mount reads the existing host effect policy and deny list.
pub trait Policy {
    fn effect(&self, command: &str) -> Effect;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Pending,
    Warned { nonce: String },
    Executing { approval: String },
    Uncertain { approval: String },
    Completed { approval: String, block: Block },
    Acknowledged { approval: String, block: Block },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entry {
    pub proposal: Proposal,
    pub phase: Phase,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Book {
    pub entries: BTreeMap<String, Entry>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Approval {
    Warning(String),
    /// Persist the executing state before handing these bytes to the PTY.
    Input {
        identity: String,
        bytes: Vec<u8>,
    },
}

impl Proposal {
    pub fn key(&self) -> String {
        digest(&(&self.thread, &self.id, self.revision))
    }

    fn valid(&self) -> bool {
        [
            &self.thread,
            &self.id,
            &self.binding.terminal,
            &self.binding.generation,
            &self.binding.cwd,
            &self.binding.context_digest,
        ]
        .iter()
        .all(|word| !word.is_empty() && word.len() <= 8192 && !word.chars().any(char::is_control))
            && self.binding.shell_directory.as_ref().is_none_or(|path| {
                !path.is_empty() && path.len() <= 8192 && !path.chars().any(char::is_control)
            })
            && !self.command.is_empty()
            && self.command.len() <= 8192
            && !self.command.chars().any(char::is_control)
    }
}

impl Book {
    /// A repeated proposal is inert; changing any bytes requires a new revision.
    pub fn offer(&mut self, proposal: Proposal) -> Result<String, &'static str> {
        if !proposal.valid() {
            return Err("invalid proposal");
        }
        let key = proposal.key();
        if let Some(entry) = self.entries.get(&key) {
            return if entry.proposal == proposal {
                Ok(key)
            } else {
                Err("proposal revision changed")
            };
        }
        if self.entries.len() >= 256 {
            return Err("proposal capacity reached");
        }
        self.entries.insert(
            key.clone(),
            Entry {
                proposal,
                phase: Phase::Pending,
            },
        );
        Ok(key)
    }

    /// Called only for a person's Enter on the displayed exact proposal.
    pub fn enter(
        &mut self,
        key: &str,
        current: &Binding,
        principal: &str,
        nonce: &str,
        policy: &dyn Policy,
    ) -> Result<Approval, &'static str> {
        if principal.is_empty() || nonce.is_empty() {
            return Err("approval identity is required");
        }
        let entry = self.entries.get_mut(key).ok_or("proposal not found")?;
        if &entry.proposal.binding != current {
            return Err("proposal target or displayed context changed");
        }
        if !matches!(entry.phase, Phase::Pending | Phase::Warned { .. }) {
            return Err("proposal already admitted; reconcile its result");
        }
        match policy.effect(&entry.proposal.command) {
            Effect::Denied(_) => return Err("host policy denied the command"),
            Effect::Destructive(warning) if entry.phase == Phase::Pending => {
                entry.phase = Phase::Warned {
                    nonce: nonce.to_owned(),
                };
                return Ok(Approval::Warning(warning));
            }
            _ => {}
        }
        if let Phase::Warned { nonce: previous } = &entry.phase
            && previous == nonce
        {
            return Err("a second explicit key is required");
        }
        let identity = digest(&(&entry.proposal, principal, nonce));
        entry.phase = Phase::Executing {
            approval: identity.clone(),
        };
        let mut bytes = entry.proposal.command.as_bytes().to_vec();
        bytes.push(b'\r');
        Ok(Approval::Input { identity, bytes })
    }

    /// Admits a proposal without Enter under the auto-run opt-in: only a
    /// still-pending proposal bound to `current`, in a workspace `setting`
    /// admits, whose command the policy classes read-only and is one plain
    /// invocation. Anything else is refused and the proposal stays pending
    /// for Enter, unchanged.
    pub fn auto(
        &mut self,
        key: &str,
        current: &Binding,
        nonce: &str,
        policy: &dyn Policy,
        setting: &crate::autorun::AutoRun,
    ) -> Result<Approval, &'static str> {
        if nonce.is_empty() {
            return Err("approval identity is required");
        }
        let entry = self.entries.get_mut(key).ok_or("proposal not found")?;
        if entry.phase != Phase::Pending {
            return Err("only a pending proposal can auto-run");
        }
        if &entry.proposal.binding != current {
            return Err("proposal target or displayed context changed");
        }
        let binding = &entry.proposal.binding;
        let admission = setting
            .admitting(&binding.cwd)
            .filter(|_| {
                binding
                    .shell_directory
                    .as_deref()
                    .is_none_or(|directory| setting.admitting(directory).is_some())
            })
            .ok_or("auto-run is off for this workspace")?;
        if policy.effect(&entry.proposal.command) != Effect::ReadOnly {
            return Err("only a read-only command can auto-run");
        }
        if !crate::autorun::plain(&entry.proposal.command) {
            return Err("only one plain command can auto-run");
        }
        let identity = digest(&(
            &entry.proposal,
            "auto-run",
            &admission.root,
            &admission.admitted_by,
            setting.revision(),
            nonce,
        ));
        entry.phase = Phase::Executing {
            approval: identity.clone(),
        };
        let mut bytes = entry.proposal.command.as_bytes().to_vec();
        bytes.push(b'\r');
        Ok(Approval::Input { identity, bytes })
    }

    /// Lost input or completion acknowledgment remains uncertain after reopening.
    pub fn recover(&mut self) {
        for entry in self.entries.values_mut() {
            if let Phase::Executing { approval } = &entry.phase {
                entry.phase = Phase::Uncertain {
                    approval: approval.clone(),
                };
            }
        }
    }

    pub fn complete(
        &mut self,
        key: &str,
        approval: &str,
        block: Block,
    ) -> Result<(), &'static str> {
        let entry = self.entries.get_mut(key).ok_or("proposal not found")?;
        if block.command != entry.proposal.command
            || block.cwd.as_deref()
                != Some(
                    entry
                        .proposal
                        .binding
                        .shell_directory
                        .as_deref()
                        .unwrap_or(entry.proposal.binding.cwd.as_str()),
                )
            || block.end.is_none()
        {
            return Err("result does not match the approved command");
        }
        match &entry.phase {
            Phase::Executing { approval: saved } | Phase::Uncertain { approval: saved }
                if saved == approval =>
            {
                entry.phase = Phase::Completed {
                    approval: approval.to_owned(),
                    block,
                };
                Ok(())
            }
            Phase::Completed {
                approval: saved,
                block: previous,
            }
            | Phase::Acknowledged {
                approval: saved,
                block: previous,
            } if saved == approval && previous == &block => Ok(()),
            _ => Err("result identity changed"),
        }
    }

    /// Returns true once. A duplicate acknowledgment never authorizes another command.
    pub fn acknowledge(&mut self, key: &str, approval: &str) -> Result<bool, &'static str> {
        let entry = self.entries.get_mut(key).ok_or("proposal not found")?;
        match &entry.phase {
            Phase::Completed {
                approval: saved,
                block,
            } if saved == approval => {
                entry.phase = Phase::Acknowledged {
                    approval: approval.to_owned(),
                    block: block.clone(),
                };
                Ok(true)
            }
            Phase::Acknowledged {
                approval: saved, ..
            } if saved == approval => Ok(false),
            _ => Err("result is not ready for acknowledgment"),
        }
    }
}

pub fn digest(value: &impl Serialize) -> String {
    let bytes = serde_json::to_vec(value).expect("terminal values serialize");
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    struct ReadOnly;
    impl Policy for ReadOnly {
        fn effect(&self, _: &str) -> Effect {
            Effect::Ordinary
        }
    }
    struct Destructive;
    impl Policy for Destructive {
        fn effect(&self, _: &str) -> Effect {
            Effect::Destructive("deletes files".into())
        }
    }
    struct Denied;
    impl Policy for Denied {
        fn effect(&self, _: &str) -> Effect {
            Effect::Denied("ends the machine".into())
        }
    }

    fn proposal() -> Proposal {
        Proposal {
            thread: "thread".into(),
            id: "offer".into(),
            revision: 1,
            command: "cargo test".into(),
            binding: Binding {
                terminal: "shell".into(),
                generation: "generation-3".into(),
                cwd: "/tmp/repo".into(),
                shell_directory: None,
                context_digest: "context".into(),
            },
        }
    }

    #[test]
    fn every_proposal_waits_for_enter_and_recovery_never_replays() {
        let proposal = proposal();
        let binding = proposal.binding.clone();
        let mut book = Book::default();
        let key = book.offer(proposal.clone()).unwrap();
        assert_eq!(book.entries[&key].phase, Phase::Pending);
        assert_eq!(book.offer(proposal).unwrap(), key);
        let mut stale = binding.clone();
        stale.generation = "new-generation".into();
        assert!(book.enter(&key, &stale, "user", "key", &ReadOnly).is_err());
        assert_eq!(book.entries[&key].phase, Phase::Pending);
        assert!(matches!(
            book.enter(&key, &binding, "user", "key", &ReadOnly)
                .unwrap(),
            Approval::Input { .. }
        ));
        assert!(
            book.enter(&key, &binding, "user", "another", &ReadOnly)
                .is_err()
        );
        book.recover();
        assert!(matches!(book.entries[&key].phase, Phase::Uncertain { .. }));
        assert!(
            book.enter(&key, &binding, "user", "another", &ReadOnly)
                .is_err()
        );
    }

    #[test]
    fn destructive_commands_need_two_keys_and_denied_commands_never_run() {
        let proposal = proposal();
        let binding = proposal.binding.clone();
        let mut book = Book::default();
        let key = book.offer(proposal).unwrap();
        assert!(book.enter(&key, &binding, "user", "one", &Denied).is_err());
        assert!(matches!(
            book.enter(&key, &binding, "user", "one", &Destructive)
                .unwrap(),
            Approval::Warning(_)
        ));
        assert!(matches!(
            book.enter(&key, &binding, "user", "two", &Destructive)
                .unwrap(),
            Approval::Input { .. }
        ));
    }

    #[test]
    fn changed_revision_content_is_refused_and_acknowledgment_is_idempotent() {
        let mut proposal = proposal();
        proposal.binding.shell_directory = Some("/tmp/link-to-repo".into());
        let binding = proposal.binding.clone();
        let mut book = Book::default();
        let key = book.offer(proposal.clone()).unwrap();
        let mut changed = proposal;
        changed.command = "rm files".into();
        assert!(book.offer(changed).is_err());
        let Approval::Input { identity, .. } = book
            .enter(&key, &binding, "user", "one", &ReadOnly)
            .unwrap()
        else {
            panic!("expected input")
        };
        let block = Block {
            id: 1,
            command: "cargo test".into(),
            cwd: Some("/tmp/link-to-repo".into()),
            start: crate::blocks::Position { line: 0, col: 0 },
            end: Some(crate::blocks::Position { line: 1, col: 0 }),
            status: Some(1),
            started_ms: 0,
            elapsed_ms: Some(1),
            output: "failed".into(),
            truncated: false,
            collapsed: false,
            alternate: false,
        };
        assert!(book.complete(&key, "forged", block.clone()).is_err());
        let mut wrong_directory = block.clone();
        wrong_directory.cwd = Some("/tmp/other".into());
        assert!(book.complete(&key, &identity, wrong_directory).is_err());
        book.complete(&key, &identity, block.clone()).unwrap();
        assert_eq!(book.acknowledge(&key, &identity), Ok(true));
        assert_eq!(book.acknowledge(&key, &identity), Ok(false));
        book.complete(&key, &identity, block).unwrap();
        assert!(
            book.enter(&key, &binding, "user", "two", &ReadOnly)
                .is_err()
        );
    }

    struct Classed(Effect);
    impl Policy for Classed {
        fn effect(&self, _: &str) -> Effect {
            self.0.clone()
        }
    }

    #[test]
    fn auto_run_needs_the_opt_in_a_read_only_class_and_a_plain_pending_command() {
        let settings = tempfile::tempdir().unwrap();
        let mut setting = crate::autorun::AutoRun::load(settings.path().join("autorun.json"));
        let read_only = Classed(Effect::ReadOnly);
        let proposal = proposal();
        let binding = proposal.binding.clone();
        let mut book = Book::default();
        let key = book.offer(proposal.clone()).unwrap();
        // Off by default.
        assert!(
            book.auto(&key, &binding, "n1", &read_only, &setting)
                .is_err()
        );
        setting.admit("/tmp", "local-user", 1).unwrap();
        // Not read-only, or not the current binding: it stays pending.
        for effect in [
            Effect::Ordinary,
            Effect::Destructive("x".into()),
            Effect::Denied("x".into()),
        ] {
            assert!(
                book.auto(&key, &binding, "n1", &Classed(effect), &setting)
                    .is_err()
            );
        }
        let mut stale = binding.clone();
        stale.cwd = "/tmp/other".into();
        assert!(book.auto(&key, &stale, "n1", &read_only, &setting).is_err());
        assert_eq!(book.entries[&key].phase, Phase::Pending);
        // A substitution never auto-runs, even classed read-only.
        let mut substituted = proposal.clone();
        substituted.id = "other".into();
        substituted.command = "cat $(which sh)".into();
        let other = book.offer(substituted).unwrap();
        assert!(
            book.auto(&other, &binding, "n1", &read_only, &setting)
                .is_err()
        );
        // The admitted exact revision runs once; nothing replays it.
        let Approval::Input { bytes, .. } = book
            .auto(&key, &binding, "n1", &read_only, &setting)
            .unwrap()
        else {
            panic!("expected input")
        };
        assert_eq!(bytes, b"cargo test\r");
        assert!(
            book.auto(&key, &binding, "n2", &read_only, &setting)
                .is_err()
        );
        book.recover();
        assert!(
            book.auto(&key, &binding, "n3", &read_only, &setting)
                .is_err()
        );
        // Revoking applies to the next proposal.
        let mut next = proposal;
        next.revision = 2;
        let next = book.offer(next).unwrap();
        setting.revoke("/tmp/repo").unwrap();
        assert!(
            book.auto(&next, &binding, "n4", &read_only, &setting)
                .is_err()
        );
    }
}
