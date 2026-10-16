//! NIP-ATT release assurance beyond the head's own admission:
//!
//! - [`rekor`]: the release's Rekor entry, included under a signed
//!   checkpoint from a pinned log key, consistent with what was seen before.
//! - [`rebuild`]: independent builders' signed rebuild statements (`3203`).
//! - [`approvals`]: the personal approval set, with notice, emergency and
//!   revocation behavior.
//! - [`receipt`]: public sealed-job `3201` receipts over ciphertext.
//!
//! [`check_release`] composes them. Any missing, stale, forked or
//! mismatched evidence refuses.
#[cfg(test)]
extern crate self as att_release;

pub mod approvals;
pub mod rebuild;
pub mod receipt;
pub mod rekor;

#[cfg(test)]
mod tests;

use nostr::att::{self, Release};
use nostr::domain::Event;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub type Result<T> = std::result::Result<T, String>;

/// The digest a release's transparency entry logs: SHA-256 of the JCS
/// release body with `transparency` empty (the entry index cannot be in
/// the bytes it logs).
///
/// # Errors
///
/// When the release does not encode.
pub fn logged_digest(release: &Release) -> Result<String> {
    let mut body = release.clone();
    body.transparency = Vec::new();
    let value = serde_json::to_value(&body).map_err(|e| e.to_string())?;
    let bytes = nostr::contracts::jcs(&value).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(&bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect())
}

/// What the client requires of a release.
pub struct Policy<'a> {
    pub publisher: &'a str,
    pub workload: &'a str,
    /// The pinned log key; `None` means transparency is not required.
    pub log: Option<&'a rekor::LogKey>,
    pub min_rebuilds: usize,
    pub seen_generation: Option<u64>,
}

/// The evidence fetched for one release.
pub struct Evidence<'a> {
    pub release: &'a Event,
    pub head: &'a Event,
    pub entry: Option<&'a rekor::Entry>,
    pub consistency: Option<&'a [rekor::Hash]>,
    pub rebuilds: &'a [Event],
    pub approvals: Option<&'a approvals::ApprovalSet>,
}

/// Check a release end to end. Returns the person's decision when an
/// approval set is in use, else `Admitted`.
///
/// # Errors
///
/// On any refusal: signatures, head admission or rollback, transparency,
/// rebuilds.
pub fn check_release(
    evidence: &Evidence<'_>,
    policy: &Policy<'_>,
    witness: &mut rekor::Witness,
    now: u64,
) -> Result<approvals::Decision> {
    let release = att::parse_release(evidence.release)?;
    let head = att::parse_head(evidence.head)?;
    if evidence.release.pubkey != policy.publisher || evidence.head.pubkey != policy.publisher {
        return Err("the release or head is signed by another publisher".into());
    }
    if release.workload != policy.workload || head.workload != policy.workload {
        return Err("the records are for another workload".into());
    }
    att::check_generation(policy.seen_generation, &head)?;
    head.admits(&evidence.release.id, &release, now)?;
    if let Some(key) = policy.log {
        let entry = evidence
            .entry
            .ok_or("the release has no transparency entry")?;
        transparency(
            &release,
            &head,
            &evidence.release.id,
            entry,
            key,
            witness,
            evidence.consistency,
        )?;
    }
    rebuild::check(&release, evidence.rebuilds, policy.min_rebuilds)?;
    match evidence.approvals {
        Some(set) => set.decide(&head, &evidence.release.id, &release, now),
        None => Ok(approvals::Decision::Admitted),
    }
}

fn transparency(
    release: &Release,
    head: &att::Head,
    release_id: &str,
    entry: &rekor::Entry,
    key: &rekor::LogKey,
    witness: &mut rekor::Witness,
    consistency: Option<&[rekor::Hash]>,
) -> Result<()> {
    if !release
        .transparency
        .iter()
        .any(|t| t.log == "rekor" && t.index == entry.log_index)
    {
        return Err("the release does not list this transparency entry".into());
    }
    let included = rekor::verify_entry(entry, key, witness, consistency)?;
    let body: &Value = &included.body;
    let hash = &body["spec"]["data"]["hash"];
    if body["kind"] != "hashedrekord"
        || hash["algorithm"] != "sha256"
        || hash["value"] != logged_digest(release)?.as_str()
    {
        return Err("the transparency entry logs another release".into());
    }
    let effective_at = head
        .admitted
        .iter()
        .find(|a| a.release == release_id)
        .map(|a| a.effective_at)
        .ok_or("the head does not list the release")?;
    if included.integrated_time > effective_at {
        return Err("the release was logged only after it took effect".into());
    }
    Ok(())
}
