//! One supported contribution: a frozen checkpoint's protected accuracy improvement.

use gym::sales_evidence::Reference;
use pay_ledger::markets::contribution::Terms;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::PathBuf};

pub const SCHEMA: &str = "openagents.contribution-service.v1";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub authority: String,
    pub protected_root: PathBuf,
    pub worker_root: PathBuf,
    pub state: PathBuf,
    pub ledger: PathBuf,
    pub frozen: Reference,
    pub current: String,
    pub evaluation: Reference,
    pub acceptance: Reference,
}

/// Signed before evaluation by the configured funding authority. No reward,
/// fee, permission, or accuracy threshold has a product default.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Frozen {
    pub schema: String,
    pub terms: Terms,
    pub corpus: Reference,
    pub transfer_corpus: Reference,
    pub recipe: Reference,
    pub baseline: Reference,
    pub attribution: Reference,
    pub plan_policy: String,
    pub rights: Vec<RightPin>,
    pub max_all_in_msat: i64,
    pub platform_fee_msat: i64,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RightPin {
    pub issuer: String,
    pub file: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Attribution {
    pub schema: String,
    pub beneficiary: String,
    pub corpus: String,
    pub transfer_corpus: String,
}
/// A current, signed provenance permission, supplied outside worker records.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rights {
    pub schema: String,
    pub permission: String,
    pub source: String,
    pub group: String,
    pub license: String,
    pub corpus: String,
    pub actions: Vec<String>,
    pub expires_at: i64,
}
/// A current signed authority record. This is an explicit funding and payout
/// grant; it cannot declare evaluator results or grant corpus permissions.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Current {
    pub schema: String,
    pub frozen: String,
    pub enabled: bool,
    pub expires_at: i64,
    pub central_node: String,
    pub destination_kind: String,
    pub destination_value: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Phase {
    pub store: Reference,
    pub commitment: Reference,
}
/// The protected evaluator signs exact native source pins. The service
/// recomputes their result; this record contains no passing verdict.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Evaluation {
    pub schema: String,
    pub frozen: String,
    pub candidate: Reference,
    pub trials: Reference,
    pub artifacts: BTreeMap<String, Reference>,
    pub plan: Reference,
    pub suite: Reference,
    pub transfer_suite: Reference,
    pub development: Phase,
    pub locked: Phase,
    pub locked_ledger: Reference,
    pub transfer: Phase,
    pub deployment: Reference,
    pub finance: Reference,
    pub finance_offer: String,
    pub costs: Vec<CostBinding>,
    pub evaluated_at: i64,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostClass {
    Training,
    Checking,
    Search,
    FailedAttempt,
    License,
    Compute,
}
pub const COST_CLASSES: [CostClass; 6] = [
    CostClass::Training,
    CostClass::Checking,
    CostClass::Search,
    CostClass::FailedAttempt,
    CostClass::License,
    CostClass::Compute,
];
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostBinding {
    pub class: CostClass,
    pub expense: String,
    pub trial: Option<usize>,
}
/// The retained bill line that REV-25's billed expense references.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CostLine {
    pub schema: String,
    pub expense: String,
    pub obligation: String,
    pub class: CostClass,
    pub trial: Option<usize>,
    pub amount_msat: i64,
}
/// Independent consent to the exact recomputed evaluation. A checksum or
/// successful checkpoint seal alone supplies none of this authority.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Consent {
    pub schema: String,
    pub frozen: String,
    pub evaluation: String,
    pub decision: String,
    pub artifact: String,
    pub recipe: String,
    pub improvement: i64,
    pub accepted_at: i64,
}
