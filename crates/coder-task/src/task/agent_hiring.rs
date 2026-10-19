//! Durable hire and retire proposals for the sales floor (REV-64).
//!
//! Paul or the owner records a proposal; only the owner's own key decides it,
//! bound to the proposal's exact digest. The book lives beside crew control
//! and is only read or written while that [`Guard`] lock is held, so a
//! confirmation counts active members, pending budget, and the cap in one
//! step. Confirmation is idempotent: the same decision for the same digest
//! returns the retained outcome and creates nothing twice.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use coder_host::access::crew::{
    FLOOR_USD_MILLIONTHS, HireAction, HireDecision, HireProposal, HireVerdict, JobRole,
    MAX_ACTIVE_HIRES, MAX_HIRE_PROPOSALS,
};
use serde::{Deserialize, Serialize};

use super::agent::Store;
use super::agent_crew_control::Guard;

pub const SCHEMA: &str = "openagents.crew-hiring.v1";
const MAX_BYTES: usize = 192 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Status {
    Pending,
    Confirmed,
    Rejected,
    Expired,
}

/// The caps the host saw when it admitted the proposal.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapSnapshot {
    pub active_hires: usize,
    pub max_active_hires: usize,
    pub committed_usd_millionths: u64,
    pub floor_usd_millionths: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Decided {
    pub verdict: HireVerdict,
    pub sha256: String,
    pub reason: String,
    pub at: u64,
    pub by: String,
    /// What the shared lifecycle did: the new member's public key or the
    /// leads returned to Paul.
    pub outcome: serde_json::Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Entry {
    pub proposal: HireProposal,
    pub sha256: String,
    pub proposed_at: u64,
    pub proposed_by: String,
    pub caps: CapSnapshot,
    pub status: Status,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<Decided>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Book {
    pub schema: String,
    pub entries: BTreeMap<String, Entry>,
}

impl Book {
    fn path(guard: &Guard) -> PathBuf {
        guard.dir().join("hiring.json")
    }
    pub fn load(guard: &Guard) -> Result<Self, String> {
        let path = Self::path(guard);
        if !path.is_file() {
            return Ok(Self {
                schema: SCHEMA.into(),
                entries: BTreeMap::new(),
            });
        }
        let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("The hiring book exceeds 192 KiB.".into());
        }
        let book: Self = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        if book.schema != SCHEMA {
            return Err("The hiring book has an unknown schema.".into());
        }
        Ok(book)
    }
    fn save(&self, guard: &Guard) -> Result<(), String> {
        guard.check()?;
        let bytes = serde_json::to_vec_pretty(self).map_err(|e| e.to_string())?;
        if bytes.len() > MAX_BYTES {
            return Err("The hiring book exceeds 192 KiB.".into());
        }
        super::replace_file(guard.dir(), "hiring.json", &bytes).map_err(|e| e.to_string())
    }
    /// Pending hires that a confirmation would commit alongside the active ones.
    fn pending_budget(&self, now: u64) -> u64 {
        self.entries
            .values()
            .filter(|e| e.status == Status::Pending && e.proposal.expires_at > now)
            .filter(|e| matches!(e.proposal.action, HireAction::Hire { .. }))
            .map(|e| e.proposal.daily_usd_millionths)
            .sum()
    }
}

/// Active sales hires and their committed daily budget, from the agent records.
pub fn active(root: &Path, book: &Book) -> Result<CapSnapshot, String> {
    let mut hires = 0;
    let mut committed = 0_u64;
    for store in Store::all(root) {
        let Some(record) = store.load()? else {
            continue;
        };
        if record.state.is_gone() {
            continue;
        }
        match record.job_role {
            Some(JobRole::SalesLead) | None => {}
            Some(_) => {
                hires += 1;
                let budget = book
                    .entries
                    .values()
                    .filter(|e| e.status == Status::Confirmed)
                    .find_map(|e| match &e.proposal.action {
                        HireAction::Hire { name, .. } if *name == record.name => {
                            Some(e.proposal.daily_usd_millionths)
                        }
                        _ => None,
                    })
                    .unwrap_or(0);
                committed = committed.saturating_add(budget);
            }
        }
    }
    Ok(CapSnapshot {
        active_hires: hires,
        max_active_hires: MAX_ACTIVE_HIRES,
        committed_usd_millionths: committed,
        floor_usd_millionths: FLOOR_USD_MILLIONTHS,
    })
}

fn fits(caps: &CapSnapshot, extra_budget: u64) -> Result<(), String> {
    if caps.active_hires >= caps.max_active_hires {
        return Err(format!(
            "The floor is at its cap of Paul plus {} active hires; retire one or ask the owner for a larger grant.",
            caps.max_active_hires
        ));
    }
    if caps.committed_usd_millionths.saturating_add(extra_budget) > caps.floor_usd_millionths {
        return Err("This hire would take the floor past its USD 5 daily model ceiling.".into());
    }
    Ok(())
}

/// Record a proposal. The host refuses one the caps already rule out.
pub fn propose(
    guard: &Guard,
    root: &Path,
    proposal: &HireProposal,
    by: &str,
    now: u64,
) -> Result<Entry, String> {
    proposal.validate().map_err(|e| e.message)?;
    if proposal.expires_at <= now {
        return Err("The proposal has already expired.".into());
    }
    let mut book = Book::load(guard)?;
    let sha256 = proposal.sha256();
    if let Some(existing) = book.entries.get(&proposal.id) {
        if existing.sha256 == sha256 {
            return Ok(existing.clone());
        }
        return Err("A different proposal already uses this id.".into());
    }
    if book.entries.len() >= MAX_HIRE_PROPOSALS {
        return Err("The hiring book is full; decide or let old proposals expire.".into());
    }
    let caps = active(root, &book)?;
    match &proposal.action {
        HireAction::Hire { name, .. } => {
            if Store::new(root, name)?
                .load()?
                .is_some_and(|r| !r.state.is_gone())
            {
                return Err(format!("{name} is already a member."));
            }
            let mut with_pending = caps.clone();
            with_pending.committed_usd_millionths = with_pending
                .committed_usd_millionths
                .saturating_add(book.pending_budget(now));
            fits(&with_pending, proposal.daily_usd_millionths)?;
        }
        HireAction::Retire { name } => {
            let record = Store::new(root, name)?
                .load()?
                .ok_or_else(|| format!("{name} is not a member."))?;
            if record.job_role.is_none() || record.state.is_gone() {
                return Err(format!("{name} is not an active sales member."));
            }
            if record.job_role == Some(JobRole::SalesLead) {
                return Err("Paul's original binding is retired only by explicit reviewed rebinding, not a hire proposal.".into());
            }
        }
    }
    let entry = Entry {
        proposal: proposal.clone(),
        sha256,
        proposed_at: now,
        proposed_by: by.into(),
        caps,
        status: Status::Pending,
        decision: None,
    };
    book.entries.insert(proposal.id.clone(), entry.clone());
    book.save(guard)?;
    Ok(entry)
}

/// What the owner's decision asks the shared lifecycle to do.
pub enum Act {
    Create { name: String, role: JobRole },
    Retire { name: String },
    Nothing,
}

/// Check a decision against the exact proposal and the caps now, and mark the
/// entry. The caller performs `Act` under the same guard, then calls
/// [`record`]. A replayed decision for a decided entry returns it unchanged.
pub fn decide(
    guard: &Guard,
    root: &Path,
    decision: &HireDecision,
    now: u64,
) -> Result<(Entry, Act), String> {
    decision.validate().map_err(|e| e.message)?;
    let mut book = Book::load(guard)?;
    let entry = book
        .entries
        .get(&decision.proposal)
        .ok_or("There is no proposal with that id.")?
        .clone();
    if entry.sha256 != decision.expected_sha256 {
        return Err("The decision names a different proposal digest; read the current proposal and decide again.".into());
    }
    if let Some(prior) = &entry.decision {
        if prior.verdict == decision.verdict {
            return Ok((entry, Act::Nothing));
        }
        return Err("The proposal already has the opposite owner decision.".into());
    }
    if entry.proposal.expires_at <= now {
        let mut expired = entry;
        expired.status = Status::Expired;
        book.entries.insert(decision.proposal.clone(), expired);
        book.save(guard)?;
        return Err("The proposal expired before a decision; propose again.".into());
    }
    if decision.verdict == HireVerdict::Reject {
        return Ok((entry, Act::Nothing));
    }
    let caps = active(root, &book)?;
    let action = entry.proposal.action.clone();
    match action {
        HireAction::Hire { name, role } => {
            if Store::new(root, &name)?
                .load()?
                .is_some_and(|r| !r.state.is_gone())
            {
                return Err(format!("{name} is already a member."));
            }
            fits(&caps, entry.proposal.daily_usd_millionths)?;
            Ok((entry, Act::Create { name, role }))
        }
        HireAction::Retire { name } => Ok((entry, Act::Retire { name })),
    }
}

pub fn record(
    guard: &Guard,
    decision: &HireDecision,
    by: &str,
    now: u64,
    outcome: serde_json::Value,
) -> Result<Entry, String> {
    let mut book = Book::load(guard)?;
    let entry = book
        .entries
        .get_mut(&decision.proposal)
        .ok_or("There is no proposal with that id.")?;
    if entry.decision.is_none() {
        entry.status = match decision.verdict {
            HireVerdict::Confirm => Status::Confirmed,
            HireVerdict::Reject => Status::Rejected,
        };
        entry.decision = Some(Decided {
            verdict: decision.verdict,
            sha256: decision.expected_sha256.clone(),
            reason: decision.reason.clone(),
            at: now,
            by: by.into(),
            outcome,
        });
    }
    let entry = entry.clone();
    book.save(guard)?;
    Ok(entry)
}

pub fn list(guard: &Guard) -> Result<Book, String> {
    Book::load(guard)
}
