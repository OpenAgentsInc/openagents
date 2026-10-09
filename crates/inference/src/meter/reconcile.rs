//! Daily reconciliation against provider billing
//! (`docs/inference/gateway.md`, section 6, "Cost reconciliation").
//!
//! Each day the meter compares, per account, our computed cost for the
//! previous UTC day with what the provider billed (Google's billing
//! export, the Z.ai console, OpenRouter's and Vercel's usage APIs), and
//! with the cost upstreams reported on the attempts themselves. A gap
//! over [`GAP_PCT`] raises an alert. When the provider also reports a
//! balance, the ledger takes it.

use std::collections::BTreeMap;
use std::sync::Mutex;

use serde::Serialize;

use super::ledger::CreditAccount;

/// The largest gap, in percent, that passes.
pub const GAP_PCT: f64 = 2.0;

/// What a provider billed one account for one UTC day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Billed {
    /// Micros of `currency`.
    pub cost: u64,
    pub currency: String,
    /// The provider's current balance, micros, when it reports one.
    pub balance: Option<u64>,
}

/// A provider's billing, read for reconciliation. Real implementations
/// (billing export, console APIs) come with their adapters.
pub trait ProviderBilling: Send + Sync {
    /// The name of this billing source (`google-billing-export`).
    fn source(&self) -> &str;
    /// Whether this source bills `account`.
    fn covers(&self, account: &CreditAccount) -> bool;
    /// What the provider billed `account` on `day` (days since the Unix
    /// epoch, UTC); `None` when it has no figure yet.
    fn billed(&self, account: &CreditAccount, day: u64) -> Result<Option<Billed>, String>;
}

/// A billing source from a table, for tests and local runs.
#[derive(Debug, Default)]
pub struct FakeBilling {
    pub source: String,
    days: Mutex<BTreeMap<(String, u64), Billed>>,
    failing: Mutex<bool>,
}

impl FakeBilling {
    pub fn new(source: impl Into<String>) -> Self {
        Self {
            source: source.into(),
            ..Self::default()
        }
    }

    /// Set what `account` was billed on `day`.
    pub fn bill(&self, account: &str, day: u64, billed: Billed) {
        if let Ok(mut days) = self.days.lock() {
            days.insert((account.to_owned(), day), billed);
        }
    }

    /// Make every read fail (an unreachable billing API).
    pub fn fail(&self, failing: bool) {
        if let Ok(mut f) = self.failing.lock() {
            *f = failing;
        }
    }
}

impl ProviderBilling for FakeBilling {
    fn source(&self) -> &str {
        &self.source
    }

    fn covers(&self, account: &CreditAccount) -> bool {
        self.days
            .lock()
            .is_ok_and(|days| days.keys().any(|(id, _)| *id == account.id))
    }

    fn billed(&self, account: &CreditAccount, day: u64) -> Result<Option<Billed>, String> {
        if self.failing.lock().is_ok_and(|f| *f) {
            return Err("billing unreachable".into());
        }
        Ok(self
            .days
            .lock()
            .ok()
            .and_then(|days| days.get(&(account.id.clone(), day)).cloned()))
    }
}

/// One comparison.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Reconciled {
    pub account: String,
    pub day: String,
    /// The billing source, or `upstream-reported` for the costs upstreams
    /// sent on the attempts.
    pub source: String,
    pub ours: u64,
    pub theirs: u64,
    pub gap_pct: f64,
    pub ok: bool,
    /// The balance the provider reported, which the ledger now holds.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub balance: Option<u64>,
    /// Why there was no comparison (an unreachable source, another
    /// currency).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trouble: Option<String>,
}

/// The gap between two amounts, in percent of the larger.
pub fn gap_pct(ours: u64, theirs: u64) -> f64 {
    let larger = ours.max(theirs);
    if larger == 0 {
        return 0.0;
    }
    ours.abs_diff(theirs) as f64 * 100.0 / larger as f64
}
