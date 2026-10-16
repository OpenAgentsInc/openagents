//! The personal approval set: releases a person has approved for the
//! workloads they rely on. With it on, a release is admitted only when the
//! head admits it (including its notice delay or emergency minimum) **and**
//! the person approved it and has not revoked that approval. Stored on the
//! device or as a self-addressed `3188` private artifact.
use crate::Result;
use nostr::att::{Head, Release};
use serde::{Deserialize, Serialize};

pub const APPROVALS_V: &str = "openagents.att-approvals.v1";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Approval {
    pub release: String,
    pub workload: String,
    pub approved_at: u64,
    pub revoked_at: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ApprovalSet {
    pub v: String,
    pub requires: Vec<String>,
    /// Grows with every change; a lower generation is a rollback.
    pub generation: u64,
    pub approvals: Vec<Approval>,
}

impl Default for ApprovalSet {
    fn default() -> Self {
        Self {
            v: APPROVALS_V.into(),
            requires: Vec::new(),
            generation: 0,
            approvals: Vec::new(),
        }
    }
}

/// Why a release is not admitted for this person.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Decision {
    Admitted,
    /// The head admits it, but the person has not approved it yet. Clients
    /// show the release's `changes` and ask, more urgently for an emergency.
    AwaitingApproval {
        emergency: bool,
    },
    Revoked,
}

impl ApprovalSet {
    /// Check a loaded set.
    ///
    /// # Errors
    ///
    /// On an unknown version, or a generation lower than one already seen.
    pub fn check(&self, seen_generation: Option<u64>) -> Result<()> {
        if self.v != APPROVALS_V || !self.requires.is_empty() {
            return Err("an unknown approval set version or feature".into());
        }
        if seen_generation.is_some_and(|g| self.generation < g) {
            return Err("the approval set is older than one already seen".into());
        }
        Ok(())
    }

    /// Approve a release. Re-approving a revoked release is a new approval.
    pub fn approve(&mut self, release: &str, workload: &str, now: u64) {
        self.approvals.retain(|a| a.release != release);
        self.approvals.push(Approval {
            release: release.into(),
            workload: workload.into(),
            approved_at: now,
            revoked_at: None,
        });
        self.generation += 1;
    }

    /// Revoke an approval, effective immediately.
    ///
    /// # Errors
    ///
    /// When the release was never approved.
    pub fn revoke(&mut self, release: &str, now: u64) -> Result<()> {
        let approval = self
            .approvals
            .iter_mut()
            .find(|a| a.release == release)
            .ok_or("that release was never approved")?;
        approval.revoked_at = Some(now);
        self.generation += 1;
        Ok(())
    }

    /// Whether the release is admitted for this person at `now`.
    ///
    /// # Errors
    ///
    /// When the head itself does not admit the release (the approval set
    /// never shortens the notice delay or overrides a retirement).
    pub fn decide(
        &self,
        head: &Head,
        release_id: &str,
        release: &Release,
        now: u64,
    ) -> Result<Decision> {
        head.admits(release_id, release, now)?;
        let emergency = head
            .emergency
            .as_ref()
            .is_some_and(|e| e.release == release_id);
        match self
            .approvals
            .iter()
            .find(|a| a.release == release_id && a.workload == release.workload)
        {
            None => Ok(Decision::AwaitingApproval { emergency }),
            Some(a) if a.revoked_at.is_some_and(|r| r <= now) => Ok(Decision::Revoked),
            Some(a) if a.approved_at > now => Ok(Decision::AwaitingApproval { emergency }),
            Some(_) => Ok(Decision::Admitted),
        }
    }

    /// The set as a self-addressed `3188` private artifact.
    ///
    /// # Errors
    ///
    /// When the set does not encode or seal.
    pub fn seal(
        &self,
        secret: &secp256k1::SecretKey,
        mailbox: &str,
        now: u64,
        nonce: [u8; 32],
    ) -> Result<nostr::domain::Event> {
        let inline = serde_json::to_value(self).map_err(|e| e.to_string())?;
        let bytes = nostr::contracts::jcs(&inline).map_err(|e| e.to_string())?;
        let body = serde_json::json!({
            "v": "openagents.artifact-envelope.v1", "requires": [],
            "artifact": {"digest": nostr::contracts::digest_bytes(&bytes), "size": bytes.len(),
                "media_type": "application/json", "schema": APPROVALS_V},
            "inline": inline, "issued_at": now, "retain_until": now + 10 * 365 * 86_400,
        });
        let me = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), secret)
            .x_only_public_key()
            .0;
        nostr::private_artifact::seal(&body, secret, &me, mailbox, now, nonce)
            .map_err(|e| e.to_string())
    }

    /// Open a self-addressed set and check it against the last generation
    /// seen.
    ///
    /// # Errors
    ///
    /// When it is not the person's own set, does not open, or rolls back.
    pub fn open(
        event: &nostr::domain::Event,
        secret: &secp256k1::SecretKey,
        seen_generation: Option<u64>,
    ) -> Result<Self> {
        let opened = nostr::private_artifact::open(event, secret).map_err(|e| e.to_string())?;
        if opened.signer() != opened.recipient() {
            return Err("an approval set must be addressed to its own author".into());
        }
        if opened.artifact().schema.as_deref() != Some(APPROVALS_V) {
            return Err("the artifact is not an approval set".into());
        }
        let bytes = opened
            .inline_bytes()
            .ok_or("the approval set is not inline")?;
        let set: Self =
            serde_json::from_slice(bytes).map_err(|_| "the approval set does not parse")?;
        set.check(seen_generation)?;
        Ok(set)
    }
}
