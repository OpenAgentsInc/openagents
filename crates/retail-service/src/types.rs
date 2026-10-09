//! Bounded customer requests and explicit operator admission.

use retail_cloud::contract::TaskRequest;
use retail_qualify::qualify::QualificationReceipt;
use route_contract::Digest;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

pub const SCHEMA: &str = "openagents.cloud.retail-customer.v1";
pub const BODY_MAX: usize = 32 * 1024;
pub const STEP_MAX: usize = 16;
pub const RECORD_MAX: usize = 4096;
pub const KEY_MAX: usize = 8192;

/// An operator grants these rights independently of the compute balance.
/// A confirmation narrows execution and disclosure to one exact admission.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetailGrant {
    pub principal: String,
    pub account: String,
    pub generation: i64,
    pub observe: bool,
    pub execute: bool,
    pub disclose: bool,
}

/// No path or credential falls back to the operator's home or environment.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub schema: String,
    pub state: PathBuf,
    pub ledger: PathBuf,
    pub template: String,
    pub grants: Vec<RetailGrant>,
    pub contract_confirmed: bool,
    pub qualification: Option<QualificationReceipt>,
    pub supported_plan: String,
    pub plan_starts_left: Option<u32>,
    /// Saved customer environments (ENV-10); absent keeps them closed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environments: Option<EnvironmentLaunch>,
}

/// The owner's published environment plan and the gate that opens it
/// (`docs/cloud/retail-environment-contract.md`). The gate stays shut
/// unless it names this plan's digest, the reviewed contract, and a
/// funded qualification.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentLaunch {
    pub plan: retail_cloud::environment::EnvironmentPlan,
    pub gate: retail_cloud::environment::Gate,
}

/// Customer credentials are transient request material. Neither debug nor
/// serialization can disclose this value.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Credential {
    pub provider: String,
    pub key: String,
    /// Explicit consent to private service custody until delivery/cleanup.
    pub service_custody: bool,
}
impl Drop for Credential {
    fn drop(&mut self) {
        let mut bytes = std::mem::take(&mut self.key).into_bytes();
        bytes.fill(0);
    }
}

#[derive(Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum Request {
    Account {},
    Capacity {},
    TopUp {
        idempotency: String,
        amount_sats: u64,
    },
    TopUpStatus {
        purchase: String,
    },
    Offer {
        idempotency: String,
        task: TaskRequest,
    },
    Confirm {
        offer: String,
        digest: Digest,
        admission: Digest,
        custody: Digest,
        credential: Credential,
    },
    Executions {
        after: Option<String>,
    },
    Execution {
        execution: String,
    },
    Progress {
        execution: String,
        after: u64,
    },
    Cancel {
        execution: String,
    },
    Artifact {
        execution: String,
        name: String,
    },
    Receipt {
        execution: String,
    },
    EnvironmentOffer {
        idempotency: String,
        request: retail_cloud::environment::EnvironmentRequest,
    },
    EnvironmentConfirm {
        purchase: String,
        digest: Digest,
    },
    Environment {
        purchase: String,
    },
    /// Delete a saved version to free its storage.
    EnvironmentDelete {
        purchase: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct Confirmation {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commercial: Option<receipts::purchase::CommercialRef>,
    pub principal: String,
    pub generation: i64,
    pub admission: Digest,
    pub key_digest: String,
    pub custody: Digest,
    pub at: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkerReport {
    pub visited: usize,
    pub pending: usize,
    /// Opaque execution identities; provider errors never expose credentials.
    pub failed: Vec<String>,
}
