//! Who approved a studio task's step: the approver binding NIP-POL asks
//! for ("Action review and approvals").
//!
//! NIP-HOST says no HOST right approves a POL action, yet a device answers
//! a task's approval with `studio.decision.answer` under `operate`. So the
//! host records each answer to an approval as a decision of its own: the
//! answering device (its key, grant, and revocation epoch) is the approver
//! for that exact action and nothing else, and the record admits one
//! answer, once.
//!
//! - **The exact action** ([`Action`]) binds the task, the revision that
//!   waits, the turn that asked, and that turn's run (its epoch and trace
//!   digest), so an approval never carries over to another step, another
//!   turn, or a rerun of the same turn. Its digest is the approval's
//!   subject.
//! - **The verdict** ([`Verdict`]) reads the answer the decision panel
//!   sends: **Allow once** approves, **Deny** denies, and an answer in the
//!   person's own words is a reply that approves nothing.
//! - **Single use.** The record is bound before the answer is sent to the
//!   task, and consumed when the task's command journal accepts that
//!   answer. Repeating the same answer (the same command ID) retrieves the
//!   same record and never binds again; another answer to an action whose
//!   record is consumed refuses. A record that was bound but never
//!   consumed (its answer was refused or the host stopped) is replaced by
//!   the next answer, because nothing dispatched it while the task still
//!   waits at the same revision.
//!
//! What admitted a record is its [`Basis`]: one person's answer, once. A
//! scoped standing rule (#10549) is another basis beside it, so a step a
//! standing rule admits records the rule, never a person who did not
//! answer.
//!
//! An approval still never widens the task's grant, boundary, routes, or
//! spend ([`super::super::interaction`]): the answer is data for the
//! engine, and the next turn runs under a fresh grant with every usual
//! check. The ledger is a sidecar beside the studio document, bounded to
//! [`MAX_RECORDS`]; the oldest consumed records go first.

use serde::{Deserialize, Serialize};

use super::super::{Task, interaction};
use super::{Error, LOCK_FILE, Studio};

/// The ledger, beside the studio document.
pub const FILE: &str = "approvals.json";
/// The ledger's schema.
pub const SCHEMA: &str = "openagents.coder.studio-approvals.v1";
/// The exact action's schema, part of what its digest covers.
pub const ACTION_SCHEMA: &str = "openagents.coder.studio-approval-action.v1";
/// The most records the ledger keeps.
pub const MAX_RECORDS: usize = 256;
/// The largest ledger the coordinator reads.
const MAX_LEDGER_BYTES: u64 = 512 * 1024;

/// The exact step a waiting task asks to approve.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub v: String,
    pub task: String,
    /// The revision that waits for the answer.
    pub revision: u64,
    /// The turn that asked, from one.
    pub turn: u64,
    /// The owner epoch of the run that asked.
    pub run_epoch: u64,
    /// That run's trace digest, which covers the step it asked about.
    pub trace_digest: String,
}

impl Action {
    /// The step `task` asks to approve, if it waits on an approval.
    #[must_use]
    pub fn of(task: &Task) -> Option<Self> {
        if interaction::pending(task) != Some(interaction::Kind::Approval) {
            return None;
        }
        let run = task.run.as_ref()?;
        let result = run.result.as_ref()?;
        Some(Self {
            v: ACTION_SCHEMA.into(),
            task: task.task_id.clone(),
            revision: task.revision,
            turn: task.follow_ups.len() as u64 + 1,
            run_epoch: run.epoch,
            trace_digest: result.trace_digest.clone(),
        })
    }

    /// The approval subject: the digest of the action's canonical bytes.
    #[must_use]
    pub fn digest(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        nostr::contracts::digest_bytes(&bytes)
    }
}

/// What an answer decides about the action.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// **Allow once**: the step may run, once.
    Approve,
    /// **Deny**: the step may not run.
    Deny,
    /// The person answered in their own words: the engine reads them, and
    /// the record approves nothing.
    Reply,
}

impl Verdict {
    /// The verdict an approval's answer `text` carries. The decision
    /// panel sends `Approved.` for **Allow once** and `Denied.` for
    /// **Deny**; anything else is a reply.
    #[must_use]
    pub fn of(text: &str) -> Self {
        let word = text
            .trim()
            .trim_end_matches(['.', '!'])
            .trim()
            .to_ascii_lowercase();
        match word.as_str() {
            "approved" | "approve" | "allow once" | "allow" => Self::Approve,
            "denied" | "deny" => Self::Deny,
            _ => Self::Reply,
        }
    }
}

/// The device that answered: the approver for one action.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Approver {
    /// The device key.
    pub device: String,
    /// The grant it answered under, when it held one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub grant: Option<String>,
    /// The grant's revocation epoch.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub epoch: Option<u64>,
}

/// What admitted a record.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Basis {
    /// One answer from the approver, under its NIP-HOST command ID.
    Once { command: String },
}

/// One approver binding.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    /// The approval subject: [`Action::digest`].
    pub subject: String,
    pub action: Action,
    pub approver: Approver,
    pub verdict: Verdict,
    pub basis: Basis,
    /// When it was bound, in Unix seconds.
    pub decided_at: u64,
    /// When the task's command journal accepted its answer; a consumed
    /// record admits nothing more.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub consumed_at: Option<u64>,
}

#[derive(Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Ledger {
    schema: String,
    /// Oldest first.
    records: Vec<Record>,
}

impl Studio {
    fn ledger(&self) -> Result<Ledger, Error> {
        let path = self.dir.join(FILE);
        if !super::super::regular_or_absent(&path)? {
            return Ok(Ledger::default());
        }
        if std::fs::metadata(&path)?.len() > MAX_LEDGER_BYTES {
            return Err(Error::Corrupt("the approval ledger is too large"));
        }
        let ledger: Ledger = serde_json::from_slice(&std::fs::read(&path)?)
            .map_err(|_| Error::Corrupt("the approval ledger is malformed"))?;
        if ledger.schema != SCHEMA {
            return Err(Error::Corrupt(
                "the approval ledger's schema is not supported",
            ));
        }
        Ok(ledger)
    }

    fn save_ledger(&self, ledger: &mut Ledger) -> Result<(), Error> {
        super::super::verify_same_file(&self.dir.join(LOCK_FILE), &self.lock)?;
        ledger.schema = SCHEMA.into();
        while ledger.records.len() > MAX_RECORDS {
            let oldest = ledger
                .records
                .iter()
                .position(|record| record.consumed_at.is_some())
                .unwrap_or(0);
            ledger.records.remove(oldest);
        }
        let bytes = serde_json::to_vec_pretty(&*ledger)
            .map_err(|_| Error::Corrupt("the approval ledger could not be encoded"))?;
        super::super::replace_file(&self.dir, FILE, &bytes)?;
        Ok(())
    }

    /// Bind `approver` to `action` with the verdict its answer `text`
    /// carries, under the answer's `command` ID, before the answer is
    /// sent. Returns the record: the existing one when the same command
    /// already bound it.
    ///
    /// # Errors
    /// Another answer already consumed this action's record (`State`), or
    /// the ledger cannot be read or written.
    pub fn bind_approval(
        &mut self,
        action: &Action,
        approver: Approver,
        text: &str,
        command: &str,
        now: u64,
    ) -> Result<Record, Error> {
        let subject = action.digest();
        let basis = Basis::Once {
            command: command.to_owned(),
        };
        let mut ledger = self.ledger()?;
        if let Some(at) = ledger
            .records
            .iter()
            .position(|record| record.subject == subject)
        {
            let held = &ledger.records[at];
            if held.basis == basis {
                return Ok(held.clone());
            }
            if held.consumed_at.is_some() {
                return Err(Error::State(
                    "this approval was already answered; an approval is single-use".into(),
                ));
            }
            // Bound and never dispatched: the task still waits at the
            // same revision, so the newer answer replaces it.
            ledger.records.remove(at);
        }
        let record = Record {
            subject,
            action: action.clone(),
            approver,
            verdict: Verdict::of(text),
            basis,
            decided_at: now,
            consumed_at: None,
        };
        ledger.records.push(record.clone());
        self.save_ledger(&mut ledger)?;
        Ok(record)
    }

    /// Mark the record `command` bound for `subject` consumed: the task's
    /// command journal accepted its answer. Consuming it again changes
    /// nothing.
    ///
    /// # Errors
    /// No record for that subject and command, or the ledger cannot be
    /// read or written.
    pub fn consume_approval(
        &mut self,
        subject: &str,
        command: &str,
        now: u64,
    ) -> Result<Record, Error> {
        let mut ledger = self.ledger()?;
        let basis = Basis::Once {
            command: command.to_owned(),
        };
        let record = ledger
            .records
            .iter_mut()
            .find(|record| record.subject == subject && record.basis == basis)
            .ok_or_else(|| Error::State("no approval is bound for this answer".into()))?;
        if record.consumed_at.is_some() {
            return Ok(record.clone());
        }
        record.consumed_at = Some(now);
        let consumed = record.clone();
        self.save_ledger(&mut ledger)?;
        Ok(consumed)
    }

    /// The approver records for `task`, oldest first.
    ///
    /// # Errors
    /// The ledger cannot be read.
    pub fn approvals(&self, task: &str) -> Result<Vec<Record>, Error> {
        Ok(self
            .ledger()?
            .records
            .into_iter()
            .filter(|record| record.action.task == task)
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn action(revision: u64, trace: char) -> Action {
        Action {
            v: ACTION_SCHEMA.into(),
            task: "a".repeat(64),
            revision,
            turn: 1,
            run_epoch: 1,
            trace_digest: format!("sha256:{}", trace.to_string().repeat(64)),
        }
    }

    fn device(n: char) -> Approver {
        Approver {
            device: n.to_string().repeat(64),
            grant: Some("grant-1".into()),
            epoch: Some(3),
        }
    }

    fn studio() -> (tempfile::TempDir, Studio) {
        let dir = tempfile::tempdir().unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        let studio = Studio::open(&dir.path().join("tasks")).unwrap();
        (dir, studio)
    }

    #[test]
    fn the_panel_answers_read_as_verdicts() {
        assert_eq!(Verdict::of("Approved."), Verdict::Approve);
        assert_eq!(Verdict::of(" Allow once. "), Verdict::Approve);
        assert_eq!(Verdict::of("Denied."), Verdict::Deny);
        assert_eq!(Verdict::of("deny"), Verdict::Deny);
        assert_eq!(Verdict::of("Only if you keep the tests."), Verdict::Reply);
    }

    #[test]
    fn the_action_binds_the_revision_the_turn_and_the_run() {
        let base = action(7, 'b');
        assert_eq!(base.digest(), action(7, 'b').digest());
        assert_ne!(base.digest(), action(8, 'b').digest());
        assert_ne!(base.digest(), action(7, 'c').digest());
        let mut later = base.clone();
        later.turn = 2;
        assert_ne!(base.digest(), later.digest());
    }

    #[test]
    fn an_approval_binds_its_device_once_and_is_consumed_once() {
        let (_dir, mut studio) = studio();
        let step = action(7, 'b');
        let bound = studio
            .bind_approval(&step, device('d'), "Approved.", "c1", 10)
            .unwrap();
        assert_eq!(bound.verdict, Verdict::Approve);
        assert_eq!(bound.approver.device, "d".repeat(64));
        assert_eq!(bound.subject, step.digest());
        assert!(bound.consumed_at.is_none());
        // A retry of the same answer retrieves the same record.
        assert_eq!(
            studio
                .bind_approval(&step, device('d'), "Approved.", "c1", 11)
                .unwrap(),
            bound
        );
        let consumed = studio.consume_approval(&bound.subject, "c1", 12).unwrap();
        assert_eq!(consumed.consumed_at, Some(12));
        // Consuming again changes nothing; a retry still retrieves it.
        assert_eq!(
            studio.consume_approval(&bound.subject, "c1", 13).unwrap(),
            consumed
        );
        assert_eq!(
            studio
                .bind_approval(&step, device('d'), "Approved.", "c1", 14)
                .unwrap(),
            consumed
        );
        // Another answer to the consumed action refuses: single use.
        assert!(matches!(
            studio.bind_approval(&step, device('e'), "Approved.", "c2", 15),
            Err(Error::State(_))
        ));
        // The next step is a new action with its own approver.
        let next = studio
            .bind_approval(&action(9, 'c'), device('e'), "Denied.", "c3", 16)
            .unwrap();
        assert_eq!(next.verdict, Verdict::Deny);
        let records = studio.approvals(&"a".repeat(64)).unwrap();
        assert_eq!(records.len(), 2);
        assert!(studio.approvals(&"b".repeat(64)).unwrap().is_empty());
    }

    #[test]
    fn an_answer_never_dispatched_is_replaced_by_the_next() {
        let (_dir, mut studio) = studio();
        let step = action(7, 'b');
        studio
            .bind_approval(&step, device('d'), "Approved.", "c1", 10)
            .unwrap();
        // The first answer was refused before the journal took it.
        let replaced = studio
            .bind_approval(&step, device('e'), "Denied.", "c2", 11)
            .unwrap();
        assert_eq!(replaced.approver.device, "e".repeat(64));
        assert_eq!(replaced.verdict, Verdict::Deny);
        assert!(studio.consume_approval(&step.digest(), "c1", 12).is_err());
        studio.consume_approval(&step.digest(), "c2", 12).unwrap();
        assert_eq!(studio.approvals(&"a".repeat(64)).unwrap().len(), 1);
    }
}
