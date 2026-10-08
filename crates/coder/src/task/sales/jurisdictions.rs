//! Reviewed recipient scopes beyond the US baseline (REV-76). Default
//! international contact is disabled: a lead outside `US` needs positive
//! evidence that names an enabled scope whose jurisdiction, recipient
//! category, channel, permission basis, and rule version match exactly.
//! The first reviewed scope is Canada business email under CASL.
use super::*;
use std::collections::BTreeSet;

pub const SCOPE_SCHEMA: &str = "openagents.sales-jurisdiction-scope.v1";
const MAX_SCOPES: usize = 16;
/// The longest a jurisdiction review stays current without renewal.
const MAX_REVIEW_SECS: u64 = 366 * 86_400;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Business,
    Individual,
}

/// CASL consent bases; `ExpressConsent` and `ExistingBusinessRelationship`
/// are the only ones a reviewed Canadian business scope admits.
#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    ExpressConsent,
    ExistingBusinessRelationship,
    ConspicuousPublication,
}

/// The exact requirements the reviewer versioned; dispatch rechecks them.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Requirements {
    pub identification: bool,
    pub postal_address: bool,
    /// Days the law allows to honor an unsubscribe; this floor suppresses at once.
    pub opt_out_days: u32,
    /// Days an unsubscribe mechanism must keep working after a message.
    pub opt_out_available_days: u32,
    pub ai_disclosure: bool,
    /// How long a permission basis may be relied on from its recorded date.
    pub permission_max_secs: u64,
    pub retention_max_secs: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub jurisdiction: String,
    pub category: Category,
    pub channel: String,
    pub bases: Vec<Basis>,
    pub cohort_reference: String,
    pub reviewer: String,
    pub review_reference: String,
    pub reviewed_at: u64,
    pub expires_at: u64,
    pub requirements: Requirements,
}
impl Scope {
    pub fn check(&self, now: u64) -> Result<()> {
        id(&self.id)?;
        for s in [
            &self.cohort_reference,
            &self.reviewer,
            &self.review_reference,
        ] {
            text(s, 256)?;
        }
        if self.schema != SCOPE_SCHEMA
            || self.version == 0
            || self.jurisdiction == "US"
            || self.jurisdiction.len() != 2
            || !self.jurisdiction.bytes().all(|b| b.is_ascii_uppercase())
            || self.channel != "email"
            || self.bases.is_empty()
            || self.bases.iter().collect::<BTreeSet<_>>().len() != self.bases.len()
            || self.reviewed_at > now
            || self.expires_at <= now
            || self.expires_at > self.reviewed_at.saturating_add(MAX_REVIEW_SECS)
            || !self.requirements.identification
            || !self.requirements.postal_address
            || !self.requirements.ai_disclosure
            || self.requirements.opt_out_days == 0
            || self.requirements.opt_out_available_days < 30
            || self.requirements.permission_max_secs == 0
            || !(1..=RETENTION_MAX).contains(&self.requirements.retention_max_secs)
        {
            return Err(
                "jurisdiction scope needs a reviewed non-US email scope with exact requirements"
                    .into(),
            );
        }
        if self.jurisdiction == "CA"
            && (self.category != Category::Business
                || self.bases.contains(&Basis::ConspicuousPublication))
        {
            return Err(
                "the reviewed Canadian scope admits business recipients on express consent or an existing business relationship only"
                    .into(),
            );
        }
        Ok(())
    }
    pub fn sha256(&self) -> Result<String> {
        Ok(digest(
            &serde_json::to_vec(self).map_err(|_| "jurisdiction scope serialization failed")?,
        ))
    }
}

/// A lead's positive evidence; absent for the US baseline.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    pub scope: String,
    pub scope_version: u64,
    pub category: Category,
    pub basis: Basis,
    /// Reference to the consent or relationship record; stored as given, read as a digest.
    pub basis_reference: String,
    pub basis_recorded_at: u64,
    pub evidence_expires_at: u64,
    pub reviewer: String,
}
impl Evidence {
    pub(super) fn check(&self) -> Result<()> {
        id(&self.scope)?;
        text(&self.basis_reference, 256)?;
        text(&self.reviewer, 256)?;
        if self.scope_version == 0
            || self.basis_reference.trim().is_empty()
            || self.evidence_expires_at <= self.basis_recorded_at
        {
            return Err(
                "jurisdiction evidence needs a scope version, basis record, and expiry".into(),
            );
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct Revision {
    scope: Scope,
    sha256: String,
    recorded_by: String,
    at: u64,
    revoked_at: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Book {
    #[serde(default)]
    scopes: BTreeMap<String, Revision>,
}
impl Book {
    pub(super) fn is_empty(&self) -> bool {
        self.scopes.is_empty()
    }
    pub(super) fn check(&self) -> Result<()> {
        if self.scopes.len() > MAX_SCOPES {
            return Err("jurisdiction scopes exceed their bound".into());
        }
        Ok(())
    }
    pub(super) fn record(&mut self, scope: Scope, by: &str, now: u64) -> Result<u64> {
        scope.check(now)?;
        let expected = self
            .scopes
            .get(&scope.id)
            .map_or(1, |r| r.scope.version.saturating_add(1));
        if scope.version != expected {
            return Err("jurisdiction scope is not the next version".into());
        }
        let sha256 = scope.sha256()?;
        let version = scope.version;
        self.scopes.insert(
            scope.id.clone(),
            Revision {
                scope,
                sha256,
                recorded_by: by.into(),
                at: now,
                revoked_at: None,
            },
        );
        self.check()?;
        Ok(version)
    }
    pub(super) fn revoke(&mut self, id: &str, now: u64) -> Result<()> {
        let r = self
            .scopes
            .get_mut(id)
            .ok_or("jurisdiction scope is unavailable")?;
        if r.revoked_at.is_none() {
            r.revoked_at = Some(now);
        }
        Ok(())
    }
    pub(super) fn current(&self, id: &str, now: u64) -> Option<&Scope> {
        self.scopes
            .get(id)
            .filter(|r| r.revoked_at.is_none() && r.scope.expires_at > now)
            .map(|r| &r.scope)
    }
    /// Admits a lead's jurisdiction for a channel: the US baseline without
    /// evidence, or exact evidence on a current scope. Nothing is inferred
    /// from a name or domain suffix.
    pub(super) fn admit(&self, details: &Details, channel: &str, now: u64) -> Result<()> {
        match (&details.jurisdiction[..], &details.scope) {
            ("US", None) => Ok(()),
            ("US", Some(_)) => Err("US leads carry no jurisdiction scope evidence".into()),
            (_, None) => Err("sales jurisdiction is unknown or outside policy".into()),
            (jurisdiction, Some(evidence)) => {
                let scope = self
                    .current(&evidence.scope, now)
                    .ok_or("jurisdiction scope is disabled, expired, or revoked")?;
                let permission = &details.permission;
                if scope.jurisdiction != jurisdiction
                    || scope.version != evidence.scope_version
                    || scope.category != evidence.category
                    || scope.channel != channel
                    || !scope.bases.contains(&evidence.basis)
                    || !permission.channels.iter().any(|c| c == channel)
                    || evidence.evidence_expires_at <= now
                    || evidence.basis_recorded_at > now
                    || evidence
                        .basis_recorded_at
                        .saturating_add(scope.requirements.permission_max_secs)
                        <= now
                    || details.data.retain_until
                        > now.saturating_add(scope.requirements.retention_max_secs)
                {
                    return Err(
                        "jurisdiction evidence does not match the reviewed scope, channel, or rule version"
                            .into(),
                    );
                }
                Ok(())
            }
        }
    }
    /// Owner-readable summary without lead data.
    pub(super) fn view(&self, now: u64) -> serde_json::Value {
        serde_json::json!({
            "international_contact": if self.scopes.values().any(|r| r.revoked_at.is_none() && r.scope.expires_at > now) { "scoped" } else { "disabled" },
            "scopes": self.scopes.values().map(|r| serde_json::json!({
                "id": r.scope.id, "version": r.scope.version, "jurisdiction": r.scope.jurisdiction,
                "category": r.scope.category, "channel": r.scope.channel, "bases": r.scope.bases,
                "reviewer": r.scope.reviewer, "expires_at": r.scope.expires_at,
                "revoked_at": r.revoked_at, "sha256": r.sha256,
            })).collect::<Vec<_>>(),
        })
    }
}
