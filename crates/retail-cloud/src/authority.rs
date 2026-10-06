//! The four retail authorities (#10708): observation, execution,
//! disclosure, and spending, each admitted and checked on its own.
//!
//! A [`RetailAdmission`] binds everything a funded retail task may do: the
//! account, the source, the computer class and its retail grant generation,
//! the effects, the material recipients, the model payer, the price book,
//! and the maximum charge. It is an additive document beside the frozen
//! admission snapshot, bound by digest (`docs/cloud/retail-contract.md`,
//! "New vocabulary arrives versioned").
//!
//! [`check`] runs before every side effect: reservation, provisioning,
//! dispatch, reads, control, and material upload. Each step needs a fixed set
//! of authorities ([`Step::needs`]), read from the current state
//! ([`Current`]) rather than from the admission. Pairing, world membership,
//! a balance, and a paid invoice are present in [`Current`] only so a test
//! can show they supply nothing.

use route_contract::snapshot::{ContentClass, Effects, Payer, Recipient};
use route_contract::{Digest, digest_of};
use serde::{Deserialize, Serialize};

/// The retail admission's schema.
pub const ADMISSION_SCHEMA: &str = "openagents.cloud.retail-admission.v1";
/// The retail contract this crate implements.
pub const CONTRACT: &str = "openagents.cloud.retail.v1";

/// A public GitHub repository at an exact commit.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    /// `https://github.com/<owner>/<repo>`.
    pub repository: String,
    /// 40 lowercase hex digits.
    pub commit: String,
}

impl Source {
    /// Whether the source is one the v1 task class accepts.
    #[must_use]
    pub fn supported(&self) -> bool {
        let Some(path) = self.repository.strip_prefix("https://github.com/") else {
            return false;
        };
        let parts: Vec<&str> = path.split('/').collect();
        parts.len() == 2
            && parts.iter().all(|p| {
                !p.is_empty()
                    && p.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
            })
            && self.commit.len() == 40
            && self
                .commit
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }
}

/// Everything a funded retail task may do, bound by digest.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetailAdmission {
    pub schema: String,
    pub contract: String,
    pub account: String,
    /// The funded execution identity.
    pub execution: String,
    pub source: Source,
    /// The digest of the task text and the declared checks.
    pub request: String,
    /// `retail-boat-large-v1`.
    pub computer_class: String,
    /// `retail-repo-change-v1`.
    pub task_class: String,
    /// The retail grant generation the execution right was minted at.
    pub grant_generation: u64,
    pub effects: Effects,
    /// Who receives material: the sandbox and the customer's model provider.
    pub recipients: Vec<Recipient>,
    pub disclosed: Vec<ContentClass>,
    pub model_payer: Payer,
    pub price_book: String,
    pub price_book_digest: Digest,
    pub max_charge_sats: u64,
}

impl RetailAdmission {
    #[must_use]
    pub fn digest(&self) -> Digest {
        digest_of(self)
    }

    /// What differs between this admission and `proposed`. Anything listed
    /// needs a new offer and a new admission.
    #[must_use]
    pub fn changes(&self, proposed: &RetailAdmission) -> Vec<Change> {
        let mut out = Vec::new();
        if self.account != proposed.account || self.execution != proposed.execution {
            out.push(Change::Identity);
        }
        if self.source != proposed.source || self.request != proposed.request {
            out.push(Change::Source);
        }
        if self.computer_class != proposed.computer_class
            || self.task_class != proposed.task_class
            || self.grant_generation != proposed.grant_generation
        {
            out.push(Change::Computer);
        }
        if self.effects != proposed.effects {
            out.push(Change::Effects);
        }
        if self.recipients != proposed.recipients || self.disclosed != proposed.disclosed {
            out.push(Change::Recipients);
        }
        if self.model_payer != proposed.model_payer {
            out.push(Change::Payer);
        }
        if self.price_book != proposed.price_book
            || self.price_book_digest != proposed.price_book_digest
            || self.max_charge_sats != proposed.max_charge_sats
        {
            out.push(Change::Quote);
        }
        out
    }
}

/// One way a proposal differs from what was admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    Identity,
    Source,
    Computer,
    Effects,
    Recipients,
    Payer,
    Quote,
}

/// The four authorities.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Authority {
    Observe,
    Execute,
    Disclose,
    Spend,
}

/// A step that has a side effect or reveals a record.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Step {
    /// Hold funds from the balance.
    Reserve,
    /// Ask the provider for a computer.
    Provision,
    /// Start the executor.
    Dispatch,
    /// Read the task's progress, artifacts, or receipt.
    Read,
    /// Cancel or steer the task.
    Control,
    /// Deliver the source and the customer's provider key to the computer.
    UploadMaterial,
}

impl Step {
    /// The authorities this step needs, all of them.
    #[must_use]
    pub fn needs(self) -> &'static [Authority] {
        match self {
            Self::Reserve => &[Authority::Spend],
            Self::Provision => &[Authority::Spend, Authority::Execute],
            Self::Dispatch => &[Authority::Spend, Authority::Execute, Authority::Disclose],
            Self::Read => &[Authority::Observe],
            Self::Control => &[Authority::Observe, Authority::Execute],
            Self::UploadMaterial => &[Authority::Execute, Authority::Disclose],
        }
    }
}

/// Where a grant came from. Only a retail grant admits retail execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantSource {
    /// Minted for one funded execution and one sandbox.
    Retail,
    /// An operator placement (`boat:<sandbox>`, `gce:<pool>`).
    Operator,
    /// A pool grant.
    Pool,
    /// The owner's local auto-start policy.
    Autostart,
    /// A paired device.
    Pairing,
}

/// The execution grant as the host holds it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecuteGrant {
    pub source: GrantSource,
    pub execution: String,
    pub generation: u64,
    pub revoked: bool,
}

/// The observation grant as the host holds it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObserveGrant {
    pub account: String,
    pub execution: String,
    pub revoked: bool,
}

/// The disclosure consent the customer gave when confirming the offer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisclosureConsent {
    /// The admission digest the consent was given for.
    pub admission: Digest,
    pub withdrawn: bool,
}

/// The spending right as the ledger resolves it now.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpendRight {
    pub account: String,
}

/// Current rights, read fresh before every step.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Current {
    pub observe: Option<ObserveGrant>,
    pub execute: Option<ExecuteGrant>,
    pub disclose: Option<DisclosureConsent>,
    pub spend: Option<SpendRight>,
    /// Supplies nothing; present to prove it.
    pub paired: bool,
    /// Supplies nothing; present to prove it.
    pub world_member: bool,
    /// Supplies nothing; present to prove it.
    pub balance_msat: i64,
    /// Supplies nothing; present to prove it.
    pub invoice_paid: bool,
}

/// Why a step was refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Denial {
    pub step: Step,
    pub authority: Authority,
    pub reason: DenialReason,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DenialReason {
    /// No such right is held.
    Missing,
    /// The right was revoked or withdrawn.
    Revoked,
    /// The right is for another account, execution, generation, or
    /// admission.
    Mismatch,
    /// The grant is not a retail grant.
    NotRetail,
}

/// Check every authority `step` needs against `current`.
///
/// # Errors
///
/// The first [`Denial`], in the order of [`Step::needs`].
pub fn check(step: Step, admission: &RetailAdmission, current: &Current) -> Result<(), Denial> {
    for &authority in step.needs() {
        let deny = |reason| Denial {
            step,
            authority,
            reason,
        };
        match authority {
            Authority::Observe => match &current.observe {
                None => return Err(deny(DenialReason::Missing)),
                Some(grant) if grant.revoked => return Err(deny(DenialReason::Revoked)),
                Some(grant)
                    if grant.account != admission.account
                        || grant.execution != admission.execution =>
                {
                    return Err(deny(DenialReason::Mismatch));
                }
                Some(_) => {}
            },
            Authority::Execute => match &current.execute {
                None => return Err(deny(DenialReason::Missing)),
                Some(grant) if grant.source != GrantSource::Retail => {
                    return Err(deny(DenialReason::NotRetail));
                }
                Some(grant) if grant.revoked => return Err(deny(DenialReason::Revoked)),
                Some(grant)
                    if grant.execution != admission.execution
                        || grant.generation != admission.grant_generation =>
                {
                    return Err(deny(DenialReason::Mismatch));
                }
                Some(_) => {}
            },
            Authority::Disclose => match &current.disclose {
                None => return Err(deny(DenialReason::Missing)),
                Some(consent) if consent.withdrawn => return Err(deny(DenialReason::Revoked)),
                Some(consent) if consent.admission != admission.digest() => {
                    return Err(deny(DenialReason::Mismatch));
                }
                Some(_) => {}
            },
            Authority::Spend => match &current.spend {
                None => return Err(deny(DenialReason::Missing)),
                Some(right) if right.account != admission.account => {
                    return Err(deny(DenialReason::Mismatch));
                }
                Some(_) => {}
            },
        }
    }
    Ok(())
}

/// The spending right for `principal`, read from the ledger now: `None` when
/// the principal is unknown, revoked, rotated, or lacks the spend right.
#[must_use]
pub fn spend_right(
    ledger: &pay_ledger::Ledger,
    principal: &str,
    credential: &str,
) -> Option<SpendRight> {
    ledger
        .resolve_principal(principal, credential, pay_ledger::compute::Need::Spend)
        .ok()
        .map(|p| SpendRight { account: p.account })
}

/// What a revocation means for work already admitted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AfterRevocation {
    /// Nothing ran: the hold is released with no charge.
    ReleaseHold,
    /// A computer exists but no executor started: tear it down, no charge.
    TeardownWithoutCharge,
    /// The executor started: request a stop, wait for its acknowledgment,
    /// tear down, and settle the measured seconds.
    StopAndSettle,
    /// The stage is not known: reconcile before reporting anything.
    Reconcile,
}

/// How far an admitted task got, as the revocation path sees it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Stage {
    Reserved,
    Provisioned,
    ExecutorStarted,
    Unknown,
}

/// The honest consequence of revoking a right at `stage`. Revocation stops
/// new work; it never erases a cleanup obligation or a measured cost.
#[must_use]
pub fn after_revocation(stage: Stage) -> AfterRevocation {
    match stage {
        Stage::Reserved => AfterRevocation::ReleaseHold,
        Stage::Provisioned => AfterRevocation::TeardownWithoutCharge,
        Stage::ExecutorStarted => AfterRevocation::StopAndSettle,
        Stage::Unknown => AfterRevocation::Reconcile,
    }
}
