//! The live bindings a funded qualification runs on (#10748).
//!
//! A bindings file (`openagents.cloud.retail-bindings.v1`) names the
//! resident receiver wallet's home (its `control.sock`), the Boat API base
//! and organization of a separate retail Boat account, the daily template,
//! and a fresh state directory for the run's ledger, journal, and Boat
//! index. Secrets never live in the file: the retail Boat key and the test
//! customer's model key come from their own environment variables, and the
//! operator's `BOAT_API_KEY` and Secret Manager entry are never read. A key
//! equal to the operator's is refused, so the operator allowance that
//! `chat work --on boat` uses cannot pay for a retail run.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "openagents.cloud.retail-bindings.v1";
/// The retail Boat account's API key.
pub const BOAT_KEY_ENV: &str = "OPENAGENTS_RETAIL_BOAT_API_KEY";
/// The test customer's own model provider key, delivered to the sandbox.
pub const MODEL_KEY_ENV: &str = "OPENAGENTS_RETAIL_CUSTOMER_MODEL_KEY";
/// The operator's Boat key, which a retail run must not use.
pub const OPERATOR_BOAT_KEY_ENV: &str = "BOAT_API_KEY";
/// The live Boat API.
pub const BOAT_API: &str = "https://boat.dev/api/v1";

/// Where the live adapters reach.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Bindings {
    pub schema: String,
    /// True only for the simulated backends; a funded run refuses it.
    pub simulation: bool,
    /// The resident receiver wallet's home, holding `control.sock`.
    pub wallet_home: PathBuf,
    pub boat_api_base: String,
    /// The retail Boat organization, never the operator's.
    pub boat_org: Option<String>,
    /// `oa-coder-main-YYYYMMDD`, the newest ready daily template.
    pub template: String,
    /// A new, empty directory for this run's ledger, journal, and index.
    pub state_dir: PathBuf,
    /// The model provider the test customer's key is for (`openai`).
    pub model_provider: String,
    /// How long to wait for the top-up invoice to be paid.
    pub payment_wait_seconds: u64,
    /// The pause between observations.
    pub poll_millis: u64,
}

/// Why bindings are refused.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "reason", rename_all = "snake_case")]
pub enum BindingRefusal {
    Schema,
    /// A funded run named simulated bindings, or a simulated run live ones.
    Simulation,
    /// A funded run must reach the live Boat API over HTTPS.
    BoatBase,
    /// The retail Boat key is not set.
    BoatKeyMissing,
    /// The retail Boat key is the operator's.
    OperatorCredential,
    /// The test customer's model key is not set.
    ModelKeyMissing,
    /// The template is not a daily `oa-coder-main-YYYYMMDD` name.
    Template,
    /// The state directory already holds files from another run.
    StateDirInUse,
    /// No resident wallet answers at the wallet home.
    WalletUnreachable,
}

/// The secrets a run uses, resolved from the environment.
pub struct Secrets {
    pub boat_key: boat::ApiKey,
    pub model_key: String,
}

impl std::fmt::Debug for Secrets {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secrets(redacted)")
    }
}

impl Bindings {
    /// Check the bindings for a funded (`funded`) or simulated run and
    /// resolve their secrets through `env`.
    ///
    /// # Errors
    ///
    /// The first [`BindingRefusal`].
    pub fn check(
        &self,
        funded: bool,
        env: &dyn Fn(&str) -> Option<String>,
    ) -> Result<Secrets, BindingRefusal> {
        if self.schema != SCHEMA {
            return Err(BindingRefusal::Schema);
        }
        if self.simulation == funded {
            return Err(BindingRefusal::Simulation);
        }
        if funded && self.boat_api_base.trim_end_matches('/') != BOAT_API {
            return Err(BindingRefusal::BoatBase);
        }
        let date = self.template.strip_prefix("oa-coder-main-").unwrap_or("");
        if date.len() != 8 || !date.bytes().all(|b| b.is_ascii_digit()) {
            return Err(BindingRefusal::Template);
        }
        let key = env(BOAT_KEY_ENV)
            .filter(|k| !k.trim().is_empty())
            .ok_or(BindingRefusal::BoatKeyMissing)?;
        if env(OPERATOR_BOAT_KEY_ENV).is_some_and(|operator| operator.trim() == key.trim()) {
            return Err(BindingRefusal::OperatorCredential);
        }
        let boat_key = boat::ApiKey::new(key).map_err(|_| BindingRefusal::BoatKeyMissing)?;
        let model_key = env(MODEL_KEY_ENV)
            .filter(|k| !k.trim().is_empty())
            .ok_or(BindingRefusal::ModelKeyMissing)?;
        if std::fs::read_dir(&self.state_dir).is_ok_and(|mut entries| entries.next().is_some()) {
            return Err(BindingRefusal::StateDirInUse);
        }
        Ok(Secrets {
            boat_key,
            model_key: model_key.trim().to_owned(),
        })
    }
}
