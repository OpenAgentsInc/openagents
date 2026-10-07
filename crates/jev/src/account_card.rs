//! Private native prepaid actions. Creation and reconciliation never retry automatically.
use super::Account;
use crate::{Error, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};

#[derive(Clone, Serialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum CardFundingAction {
    Quote { id: String, gross_units: u64 },
    Checkout { id: String, approved: String },
    Read { id: String },
    Reconcile { id: String },
}
impl CardFundingAction {
    fn id(&self) -> &str {
        match self {
            Self::Quote { id, .. }
            | Self::Checkout { id, .. }
            | Self::Read { id }
            | Self::Reconcile { id } => id,
        }
    }
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingQuote {
    pub id: String,
    pub origin: String,
    pub policy: String,
    pub conversion: String,
    pub gross_units: u64,
    pub maximum_fee_units: u64,
    pub expires_at: u64,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingBinding {
    pub context: receipts::purchase::Context,
    pub quote: CardFundingQuote,
    pub quoted_at: u64,
    pub policy_digest: String,
    pub deployment: String,
    pub customer_reference: String,
    pub merchant: String,
    pub live: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_version: Option<String>,
    pub return_origin: String,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingCreate {
    pub idempotency: String,
    pub started_at: u64,
    pub native: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CardFundingFinality {
    Pending,
    Confirmed,
    Final,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingPayment {
    pub id: String,
    pub origin: String,
    pub payment: String,
    pub policy: String,
    pub conversion: String,
    pub gross_units: u64,
    pub fee_units: u64,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingSnapshot {
    pub quote: String,
    pub funding: CardFundingPayment,
    pub paid_at: u64,
    pub finality: CardFundingFinality,
    pub evidence: String,
    pub revision: u64,
    pub refunded_source_units: u64,
    pub disputed_source_units: u64,
    pub reconciliation_pending: bool,
    #[serde(default)]
    pub processor_expense_units: i64,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingObservation {
    pub snapshot: CardFundingSnapshot,
    pub adjustment_fee_units: i64,
    pub excess_removed_units: u64,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingRecord {
    pub unpaid_status: Option<String>,
    pub binding: CardFundingBinding,
    pub customer: Option<CardFundingCreate>,
    pub checkout: Option<CardFundingCreate>,
    pub hosted_url: Option<String>,
    pub applied: Option<CardFundingObservation>,
    pub applying: Option<CardFundingObservation>,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingBalance {
    pub currency: String,
    pub credited: u64,
    pub reserved: u64,
    pub settled: u64,
    pub refunded: u64,
    pub available: u64,
    pub spend_remaining: u64,
    pub restricted_credit: u64,
    pub operator_loss: u64,
    pub uncovered_holds: u64,
    pub reversed_credit: u64,
    pub purchased_funding: u64,
    pub promotional_credit: u64,
    pub processor_expense_units: i64,
}
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingHold {
    pub attempt: String,
    pub reserved: u64,
    pub phase: String,
}
/// Retained checkout references belong in private client storage. Debug omits the payment URL.
#[derive(Clone, Deserialize, Serialize)]
pub struct CardFundingView {
    pub schema: String,
    pub approval_digest: String,
    pub record: CardFundingRecord,
    pub balance: CardFundingBalance,
    pub processor_liquidity: String,
    /// Missing on older servers means detail unavailable, rather than no holds.
    pub outstanding: Option<Vec<CardFundingHold>>,
    pub outstanding_count: Option<u64>,
    pub production_qualification: String,
}
impl std::fmt::Debug for CardFundingView {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("CardFundingView")
            .field("schema", &self.schema)
            .field("currency", &self.balance.currency)
            .finish_non_exhaustive()
    }
}
fn identifier(v: &str) -> bool {
    !v.is_empty()
        && v.len() <= 128
        && v.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
        && !matches!(v, "." | "..")
}
impl Account<'_> {
    /// Call the native card lane for one retained private purchase reference.
    /// Read requests move no money; return URLs have no success or funding authority.
    pub async fn card_funding(
        &self,
        workspace: &str,
        door: &str,
        action: &CardFundingAction,
    ) -> Result<CardFundingView> {
        if !identifier(workspace)
            || !identifier(door)
            || !(16..=128).contains(&action.id().len())
            || !action
                .id()
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
        {
            return Err(Error::Config(
                "Invalid native checkout route or reference.".into(),
            ));
        }
        let body = serde_json::to_vec(action)
            .map_err(|_| Error::Config("Invalid native checkout request.".into()))?;
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            "x-workspace-id",
            workspace
                .parse()
                .map_err(|_| Error::Config("Invalid native checkout workspace.".into()))?,
        );
        let raw = self
            .client
            .request_private_headers(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/card-funding/{door}"),
                Some(body),
                &headers,
            )
            .await?;
        let invalid = || Error::ResponseValidation {
            status: raw.status,
            field_path: "native checkout".into(),
            body: None,
            request_id: None,
        };
        if raw.bytes.len() > 32 * 1024 {
            return Err(invalid());
        }
        let view: CardFundingView = serde_json::from_slice(&raw.bytes).map_err(|_| invalid())?;
        if view.schema != "openagents.card-funding.v1"
            || view.record.binding.context.workspace != workspace
            || view.record.binding.context.payer_workspace != workspace
            || view.record.binding.context.door != door
            || view.record.binding.quote.id != action.id()
            || view.balance.currency != "USD"
            || view.processor_liquidity != "unknown"
        {
            return Err(invalid());
        }
        let binding = serde_json::to_value(&view.record.binding).map_err(|_| invalid())?;
        if receipts::execution::digest_request(&binding) != view.approval_digest {
            return Err(invalid());
        }
        if let CardFundingAction::Quote { gross_units, .. } = action {
            if view.record.binding.quote.gross_units != *gross_units {
                return Err(invalid());
            }
        }
        if let CardFundingAction::Checkout { approved, .. } = action {
            if &view.approval_digest != approved {
                return Err(invalid());
            }
        }
        Ok(view)
    }
}
