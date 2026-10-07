//! Single-attempt funding requests; collection is established by the gateway's wallet.
use crate::{
    Error, Result,
    account::{Account, Position},
};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum DecisionFundingRequest {
    Quote { id: String, amount_msat: u64 },
    Issue { id: String, approved: String },
    Read { id: String },
    Reconcile { id: String },
}
impl DecisionFundingRequest {
    fn id(&self) -> &str {
        match self {
            Self::Quote { id, .. }
            | Self::Issue { id, .. }
            | Self::Read { id }
            | Self::Reconcile { id } => id,
        }
    }
}
/// Retained quote and observation data, with independently checked customer and digest.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DecisionFunding {
    pub schema: String,
    pub quote_digest: String,
    pub record: Value,
    pub balance: DecisionFundingBalance,
    pub wallet_liquidity: String,
    pub earned_usage: bool,
    pub production_qualification: String,
}
/// Native ledger position, including funding provenance and uncovered holds.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DecisionFundingBalance {
    #[serde(flatten)]
    pub position: Position,
    pub price_versions: Vec<String>,
    pub funding_policy_versions: Vec<String>,
    pub purchased_funding: u64,
    pub promotional_credit: u64,
    pub reversed_credit: u64,
    pub expired_credit: u64,
    pub restricted_credit: u64,
    pub operator_loss: u64,
    pub uncovered_holds: u64,
    /// Verified processor expense in ledger units, separate from customer credit.
    #[serde(default)]
    pub processor_expense_units: i64,
}
impl Account<'_> {
    /// Request, approve, inspect, or reconcile one funding identity without retries.
    /// The issue action creates an invoice; it never pays one from a caller wallet.
    pub async fn decision_funding(
        &self,
        workspace: &str,
        door: &str,
        request: &DecisionFundingRequest,
    ) -> Result<DecisionFunding> {
        for id in [workspace, door, request.id()] {
            if id.is_empty()
                || id.len() > 128
                || !id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.'))
            {
                return Err(Error::Config("Invalid decision funding identity.".into()));
            }
        }
        let raw = self
            .client
            .request_private(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/decision-funding/{door}"),
                Some(
                    serde_json::to_vec(request)
                        .map_err(|_| Error::Config("Invalid funding request.".into()))?,
                ),
            )
            .await?;
        let view: DecisionFunding = serde_json::from_slice(&raw.bytes)
            .map_err(|_| Error::Config("Invalid private funding response.".into()))?;
        let context: receipts::purchase::Context =
            serde_json::from_value(view.record["quote"]["context"].clone())
                .map_err(|_| Error::Config("Missing funding customer context.".into()))?;
        context
            .validate()
            .map_err(|_| Error::Config("Invalid funding customer context.".into()))?;
        if view.schema != "openagents.decision-funding.v1"
            || view.record["quote"]["id"] != request.id()
            || context.workspace != workspace
            || context.door != door
            || context.price.currency != "BTC"
            || view.quote_digest != receipts::execution::digest_request(&view.record["quote"])
            || view.balance.position.currency != "BTC"
            || view.earned_usage
            || view.wallet_liquidity != "unknown"
            || !view.record["observation"]["preimage"].is_null()
            || !matches!(
                view.record["phase"].as_str(),
                Some("quoted" | "issuing" | "unknown" | "invoice" | "funded" | "failed")
            )
        {
            return Err(Error::Config(
                "Funding response changes its original identity or monetary terms.".into(),
            ));
        }
        Ok(view)
    }
}
