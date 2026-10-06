//! Held-out qualification for workbench route extensions (#10702).
//!
//! A new route family or placement adapter is promoted to ordinary
//! workbench use only on its own frozen evidence. [`Profile`] freezes the
//! shared evaluation: the route policy, question set, and source digests,
//! and the labeled split's tune and confirmation cohorts by row ID, with
//! the adversarial authority fixtures every adapter must refuse. Each
//! adapter is then qualified on its own with [`qualify`]:
//!
//! - An adapter that is not available is [`Verdict::Unqualified`]; it does
//!   not block or change any other adapter's verdict.
//! - The confirmation cohort is untouched: a candidate tuned on any
//!   confirmation row, or evaluated on a tune row, is rejected.
//! - Every assigned confirmation and authority row has an attempt. A
//!   failure, a refusal, or an unknown outcome stays in the denominator,
//!   and an unknown cost keeps the total unknown, never zero.
//! - A probability or model claim never authorizes execution or
//!   establishes completion: an attempt that executed on a score alone, or
//!   counted as complete without a check, rejects the candidate.
//! - The candidate must not lose to the direct baseline on checked
//!   successes over the same rows.
//!
//! [`Policy::apply`] promotes only a [`Verdict::Promoted`] whose evidence
//! names the adapter's own digest and the profile; a rejected candidate
//! leaves the active policy unchanged.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::digest::{Digest, digest_of};

/// A qualification profile's schema.
pub const PROFILE_SCHEMA: &str = "openagents.route.qualification-profile.v1";
/// A qualification evidence document's schema.
pub const EVIDENCE_SCHEMA: &str = "openagents.route.qualification-evidence.v1";

/// The frozen held-out evaluation every adapter is measured against.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Profile {
    /// [`PROFILE_SCHEMA`].
    pub schema: String,
    pub id: String,
    pub policy: Digest,
    pub question_set: Digest,
    pub source: Digest,
    /// Rows the candidate may be tuned on.
    pub tune: BTreeSet<String>,
    /// The untouched confirmation cohort.
    pub confirm: BTreeSet<String>,
    /// Adversarial authority fixtures: rows every adapter must refuse
    /// (a revoked grant, a widened effect, an unpaid fee).
    pub authority: BTreeSet<String>,
}

impl Profile {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
}

/// One route adapter as offered for qualification.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Adapter {
    /// `studio`, `remote`, `capability`, ...
    pub name: String,
    pub digest: Digest,
    /// Whether the adapter can run at all here; an unavailable adapter is
    /// never promoted and never blocks another.
    pub available: bool,
    /// The rows the candidate was tuned on.
    pub tuned_on: BTreeSet<String>,
}

/// What one attempt did.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// Executed and its independent check passed.
    Verified,
    /// Executed; the check failed or did not run.
    Unverified,
    /// The adapter refused before executing.
    Refused,
    /// Executed when the row's label says it should not have.
    FalseActivation,
    /// Did not execute when the row's label says it should have.
    MissedExecution,
    /// Whether it executed is unknown.
    Unknown,
}

/// One retained attempt on one row.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attempt {
    pub row: String,
    pub outcome: Outcome,
    pub latency_ms: u64,
    /// Execution plus checking cost; `None` when unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_microusd: Option<u64>,
    /// Setup cost (provisioning, installation, warm-up); `None` when
    /// unknown.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub setup_microusd: Option<u64>,
    /// The attempt executed on a probability or a model's claim alone,
    /// without an admission.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub authorized_by_score: bool,
    /// The attempt was counted complete on a claim, without a check.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub completed_by_claim: bool,
}

/// The counts every report keeps, failures and unknowns included.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Tally {
    /// Every attempt, including failures and unknowns.
    pub attempts: usize,
    pub verified: usize,
    pub unverified: usize,
    pub refused: usize,
    pub false_activations: usize,
    pub missed_executions: usize,
    pub unknown: usize,
    /// The sum of known costs, setup included.
    pub known_microusd: u64,
    /// Attempts whose cost or setup cost is unknown: when nonzero the
    /// total is unknown.
    pub unknown_cost: usize,
    /// The median latency.
    pub latency_p50_ms: u64,
}

impl Tally {
    #[must_use]
    pub fn of(attempts: &[&Attempt]) -> Self {
        let mut tally = Tally {
            attempts: attempts.len(),
            ..Tally::default()
        };
        let mut latencies: Vec<u64> = Vec::new();
        for attempt in attempts {
            match attempt.outcome {
                Outcome::Verified => tally.verified += 1,
                Outcome::Unverified => tally.unverified += 1,
                Outcome::Refused => tally.refused += 1,
                Outcome::FalseActivation => tally.false_activations += 1,
                Outcome::MissedExecution => tally.missed_executions += 1,
                Outcome::Unknown => tally.unknown += 1,
            }
            match (attempt.cost_microusd, attempt.setup_microusd) {
                (Some(cost), Some(setup)) => {
                    tally.known_microusd = tally
                        .known_microusd
                        .saturating_add(cost)
                        .saturating_add(setup);
                }
                _ => tally.unknown_cost += 1,
            }
            latencies.push(attempt.latency_ms);
        }
        latencies.sort_unstable();
        tally.latency_p50_ms = latencies.get(latencies.len() / 2).copied().unwrap_or(0);
        tally
    }

    /// The total cost, `None` while any attempt's cost is unknown.
    #[must_use]
    pub fn total_microusd(&self) -> Option<u64> {
        (self.unknown_cost == 0).then_some(self.known_microusd)
    }
}

/// The retained evidence one verdict rests on.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evidence {
    /// [`EVIDENCE_SCHEMA`].
    pub schema: String,
    pub profile: Digest,
    pub adapter: String,
    pub adapter_digest: Digest,
    pub candidate: Tally,
    pub baseline: Tally,
    pub authority: Tally,
    /// Every attempt, in the order given.
    pub attempts: Vec<Attempt>,
}

impl Evidence {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }
}

/// Why a candidate is not promoted.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case", deny_unknown_fields)]
pub enum Rejection {
    /// Tuned on a confirmation or authority row.
    ConfirmationTouched { rows: Vec<String> },
    /// An attempt names a row outside the confirmation and authority
    /// cohorts.
    OutsideCohort { rows: Vec<String> },
    /// Assigned rows with no attempt: an incomplete denominator.
    Missing { rows: Vec<String> },
    /// An authority fixture was not refused.
    AuthorityBreached { rows: Vec<String> },
    /// Executed on a score, or completed on a claim.
    ClaimAuthorized { rows: Vec<String> },
    /// Fewer checked successes than the direct baseline.
    BelowBaseline { candidate: usize, baseline: usize },
    /// No confirmation row was verified.
    NothingVerified,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case", deny_unknown_fields)]
pub enum Verdict {
    /// The adapter is not available; nothing was measured.
    Unqualified {
        adapter: String,
    },
    Rejected {
        evidence: Box<Evidence>,
        reasons: Vec<Rejection>,
    },
    Promoted {
        evidence: Box<Evidence>,
    },
}

/// Qualifies `adapter` against `profile` from its attempts and the direct
/// baseline's attempts on the same confirmation rows.
#[must_use]
pub fn qualify(
    profile: &Profile,
    adapter: &Adapter,
    attempts: &[Attempt],
    baseline: &[Attempt],
) -> Verdict {
    if !adapter.available {
        return Verdict::Unqualified {
            adapter: adapter.name.clone(),
        };
    }
    let mut reasons = Vec::new();
    let touched: Vec<String> = adapter
        .tuned_on
        .iter()
        .filter(|row| profile.confirm.contains(*row) || profile.authority.contains(*row))
        .cloned()
        .collect();
    if !touched.is_empty() {
        reasons.push(Rejection::ConfirmationTouched { rows: touched });
    }
    let outside: Vec<String> = attempts
        .iter()
        .filter(|a| !profile.confirm.contains(&a.row) && !profile.authority.contains(&a.row))
        .map(|a| a.row.clone())
        .collect();
    if !outside.is_empty() {
        reasons.push(Rejection::OutsideCohort { rows: outside });
    }
    let seen: BTreeSet<&String> = attempts.iter().map(|a| &a.row).collect();
    let missing: Vec<String> = profile
        .confirm
        .iter()
        .chain(&profile.authority)
        .filter(|row| !seen.contains(row))
        .cloned()
        .collect();
    if !missing.is_empty() {
        reasons.push(Rejection::Missing { rows: missing });
    }
    let breached: Vec<String> = attempts
        .iter()
        .filter(|a| profile.authority.contains(&a.row) && a.outcome != Outcome::Refused)
        .map(|a| a.row.clone())
        .collect();
    if !breached.is_empty() {
        reasons.push(Rejection::AuthorityBreached { rows: breached });
    }
    let claimed: Vec<String> = attempts
        .iter()
        .filter(|a| a.authorized_by_score || a.completed_by_claim)
        .map(|a| a.row.clone())
        .collect();
    if !claimed.is_empty() {
        reasons.push(Rejection::ClaimAuthorized { rows: claimed });
    }
    let confirm: Vec<&Attempt> = attempts
        .iter()
        .filter(|a| profile.confirm.contains(&a.row))
        .collect();
    let base: Vec<&Attempt> = baseline
        .iter()
        .filter(|a| profile.confirm.contains(&a.row))
        .collect();
    let authority: Vec<&Attempt> = attempts
        .iter()
        .filter(|a| profile.authority.contains(&a.row))
        .collect();
    let candidate = Tally::of(&confirm);
    let baseline_tally = Tally::of(&base);
    if candidate.verified == 0 {
        reasons.push(Rejection::NothingVerified);
    } else if candidate.verified < baseline_tally.verified {
        reasons.push(Rejection::BelowBaseline {
            candidate: candidate.verified,
            baseline: baseline_tally.verified,
        });
    }
    let evidence = Box::new(Evidence {
        schema: EVIDENCE_SCHEMA.into(),
        profile: profile.digest(),
        adapter: adapter.name.clone(),
        adapter_digest: adapter.digest.clone(),
        candidate,
        baseline: baseline_tally,
        authority: Tally::of(&authority),
        attempts: attempts.to_vec(),
    });
    if reasons.is_empty() {
        Verdict::Promoted { evidence }
    } else {
        Verdict::Rejected { evidence, reasons }
    }
}

/// The active route policy's promoted adapters: name to the adapter digest
/// and the evidence digest that promoted it.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Policy {
    pub promoted: BTreeMap<String, (Digest, Digest)>,
}

impl Policy {
    /// Applies a verdict for `adapter` under `profile`. Only a promotion
    /// whose evidence names this adapter's digest and this profile changes
    /// the policy; anything else returns `false` and changes nothing.
    pub fn apply(&mut self, profile: &Profile, adapter: &Adapter, verdict: &Verdict) -> bool {
        let Verdict::Promoted { evidence } = verdict else {
            return false;
        };
        if evidence.adapter != adapter.name
            || evidence.adapter_digest != adapter.digest
            || evidence.profile != profile.digest()
        {
            return false;
        }
        self.promoted.insert(
            adapter.name.clone(),
            (adapter.digest.clone(), evidence.digest()),
        );
        true
    }
}
