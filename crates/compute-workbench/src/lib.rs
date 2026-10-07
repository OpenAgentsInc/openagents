//! Read-only shared compute projections. Amounts remain exact millisatoshis;
//! one displayed credit is one sat. The optional client calls the retail
//! service through explicit offer controls; it owns no money ledger.
use serde::{Deserialize, Serialize};

#[cfg(feature = "host")]
pub mod host;

#[cfg(all(feature = "client", unix))]
pub mod retail;

pub const SCHEMA: &str = "openagents.compute-workbench.v1";
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Balance {
    pub credited_msat: i64,
    pub available_msat: i64,
    pub held_msat: i64,
    pub settled_msat: i64,
    pub released_msat: i64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaymentState {
    Pending,
    Paid,
    Expired,
    Unknown,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TopUp {
    pub purchase: String,
    pub payment_hash: String,
    pub amount_msat: i64,
    pub expires_at: i64,
    pub state: PaymentState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Quote {
    pub offer: String,
    pub digest: String,
    pub book: String,
    pub expires_at: u64,
    pub maximum_msat: i64,
    /// Informational only. Confirmation must use the owner's separate offer control.
    pub confirmable: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub account: String,
    pub settlement_resource: String,
    pub execution: String,
    pub request: String,
    pub offer: String,
    pub quote: String,
    pub source_repository: String,
    pub source_commit: String,
    pub computer: String,
    pub executor: Option<String>,
    pub model_payer: String,
    pub verification: String,
    pub quoted_msat: i64,
    pub reserved_msat: i64,
    pub settled_msat: Option<i64>,
    pub released_msat: Option<i64>,
    pub usage_seconds: Option<u64>,
    pub cost_unknown: bool,
    pub cancellation_requested: bool,
    pub cancellation_acknowledged: bool,
    pub check_failed: Option<bool>,
    pub settlement_source: Option<String>,
    pub usage_digest: Option<String>,
    pub settled_at: Option<i64>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Account {
    pub schema: String,
    pub account: String,
    pub observed_at: i64,
    pub balance: Balance,
    pub topups: Vec<TopUp>,
    pub quotes: Vec<Quote>,
    pub receipts: Vec<Receipt>,
}

/// Exact credit text, without floating-point rounding.
pub fn credits(msat: i64) -> String {
    let sign = if msat < 0 { "-" } else { "" };
    let magnitude = msat.unsigned_abs();
    format!(
        "{sign}{}.{:03} credits ({} msat)",
        magnitude / 1000,
        magnitude % 1000,
        msat
    )
}
impl Account {
    pub fn lines(&self) -> String {
        let mut lines = vec![
            format!("Account {}", self.account),
            format!("Available {}", credits(self.balance.available_msat)),
            format!("Held {}", credits(self.balance.held_msat)),
            format!("Settled {}", credits(self.balance.settled_msat)),
            format!(
                "Released {} (already available; not a refund)",
                credits(self.balance.released_msat)
            ),
        ];
        for purchase in self.topups.iter().take(8) {
            lines.push(format!(
                "Top-up {}: {:?}, {}",
                purchase.purchase,
                purchase.state,
                credits(purchase.amount_msat)
            ));
        }
        for quote in self.quotes.iter().take(4) {
            lines.push(format!(
                "Offer {}: {} maximum; {}",
                quote.offer,
                credits(quote.maximum_msat),
                if quote.confirmable {
                    "separate offer control required"
                } else {
                    "expired or changed; cannot confirm"
                }
            ));
        }
        for receipt in self.receipts.iter().take(4) {
            lines.push(format!(
                "Run {} / offer {}: {}",
                receipt.execution,
                receipt.offer,
                if receipt.cost_unknown {
                    "unknown cost; funds remain held"
                } else {
                    "known charge"
                }
            ));
        }
        lines.join("\n")
    }
}
impl Receipt {
    pub fn lines(&self) -> String {
        format!(
            "Payer account {}\nSettlement resource {}\nRun {}\nRequest {}\nOffer {}\nQuote {}\nSource {} @ {}\nComputer {}\nExecutor {}\nModel payer {}\nVerification {}\nQuoted {}\nReserved {}\nSettled {}\nReleased {}\nUsage {}\nCancellation {}\nChecks {}\nSettlement {}",
            self.account,
            self.settlement_resource,
            self.execution,
            self.request,
            self.offer,
            self.quote,
            self.source_repository,
            self.source_commit,
            self.computer,
            self.executor.as_deref().unwrap_or("unknown"),
            self.model_payer,
            self.verification,
            credits(self.quoted_msat),
            credits(self.reserved_msat),
            self.settled_msat
                .map(credits)
                .unwrap_or_else(|| "unknown cost; funds remain held".into()),
            self.released_msat
                .map(credits)
                .unwrap_or_else(|| "unknown".into()),
            self.usage_seconds
                .map(|s| format!("{s} seconds"))
                .unwrap_or_else(|| "unknown".into()),
            if self.cancellation_acknowledged {
                "acknowledged"
            } else if self.cancellation_requested {
                "requested; effects not acknowledged"
            } else {
                "not requested"
            },
            match self.check_failed {
                Some(true) => "failed check",
                Some(false) => "passed retained declared checks",
                None => "unverified",
            },
            self.settlement_source.as_deref().unwrap_or("not settled")
        )
    }
}

/// Public statistics never include account IDs, invoice hashes, source, or task records.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicAggregate {
    pub completed_runs: u64,
    pub settled_msat: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn credit_text_preserves_subsat_and_extreme_amounts() {
        assert_eq!(credits(1234), "1.234 credits (1234 msat)");
        assert!(credits(i64::MIN).starts_with("-9223372036854775.808"));
    }
    #[test]
    fn public_shape_contains_no_private_resource() {
        let value = serde_json::to_value(PublicAggregate {
            completed_runs: 1,
            settled_msat: 1234,
        })
        .unwrap();
        assert_eq!(value.as_object().unwrap().len(), 2);
    }
}
