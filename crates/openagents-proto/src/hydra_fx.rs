use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::wire::google::protobuf::{ListValue, Struct, Value as ProtoValue, value::Kind};
use crate::wire::openagents::hydra::v1 as wire;
use crate::wire::openagents::lightning::v1 as wire_lightning;

pub const FX_RFQ_REQUEST_SCHEMA_V1: &str = "openagents.hydra.fx_rfq_request.v1";
pub const FX_RFQ_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.fx_rfq_response.v1";
pub const FX_QUOTE_UPSERT_REQUEST_SCHEMA_V1: &str = "openagents.hydra.fx_quote_upsert_request.v1";
pub const FX_QUOTE_UPSERT_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.fx_quote_upsert_response.v1";
pub const FX_SELECT_REQUEST_SCHEMA_V1: &str = "openagents.hydra.fx_select_request.v1";
pub const FX_SELECT_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.fx_select_response.v1";
pub const FX_SETTLE_REQUEST_SCHEMA_V1: &str = "openagents.hydra.fx_settle_request.v1";
pub const FX_SETTLE_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.fx_settle_response.v1";
pub const FX_SETTLEMENT_RECEIPT_SCHEMA_V1: &str = "openagents.hydra.fx_settlement_receipt.v1";

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HydraFxConversionError {
    #[error("{message}.{field} is required")]
    MissingField {
        message: &'static str,
        field: &'static str,
    },
    #[error("{message}.{field} has invalid enum value: {value}")]
    InvalidEnum {
        message: &'static str,
        field: &'static str,
        value: i32,
    },
    #[error("{message}.{field} must be a JSON object")]
    InvalidObjectField {
        message: &'static str,
        field: &'static str,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FxMoneyV1 {
    pub asset: String,
    pub amount: u64,
    pub unit: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalletExecutionReceiptV1 {
    pub receipt_version: String,
    pub receipt_id: String,
    pub request_id: String,
    pub wallet_id: String,
    pub host: String,
    pub payment_id: String,
    pub invoice_hash: String,
    pub quoted_amount_msats: u64,
    pub settled_amount_msats: u64,
    pub preimage_sha256: String,
    pub paid_at_ms: i64,
    pub rail: String,
    pub asset_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FxQuoteStatusV1 {
    Active,
    Expired,
    Withdrawn,
    Selected,
}

impl From<FxQuoteStatusV1> for wire::FxQuoteStatus {
    fn from(value: FxQuoteStatusV1) -> Self {
        match value {
            FxQuoteStatusV1::Active => wire::FxQuoteStatus::Active,
            FxQuoteStatusV1::Expired => wire::FxQuoteStatus::Expired,
            FxQuoteStatusV1::Withdrawn => wire::FxQuoteStatus::Withdrawn,
            FxQuoteStatusV1::Selected => wire::FxQuoteStatus::Selected,
        }
    }
}

impl TryFrom<i32> for FxQuoteStatusV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let status = wire::FxQuoteStatus::try_from(value).map_err(|_| {
            HydraFxConversionError::InvalidEnum {
                message: "FxQuoteV1",
                field: "status",
                value,
            }
        })?;
        match status {
            wire::FxQuoteStatus::Active => Ok(Self::Active),
            wire::FxQuoteStatus::Expired => Ok(Self::Expired),
            wire::FxQuoteStatus::Withdrawn => Ok(Self::Withdrawn),
            wire::FxQuoteStatus::Selected => Ok(Self::Selected),
            wire::FxQuoteStatus::Unspecified => Err(HydraFxConversionError::InvalidEnum {
                message: "FxQuoteV1",
                field: "status",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FxSettlementStatusV1 {
    Released,
    Withheld,
    Failed,
}

impl From<FxSettlementStatusV1> for wire::FxSettlementStatus {
    fn from(value: FxSettlementStatusV1) -> Self {
        match value {
            FxSettlementStatusV1::Released => wire::FxSettlementStatus::Released,
            FxSettlementStatusV1::Withheld => wire::FxSettlementStatus::Withheld,
            FxSettlementStatusV1::Failed => wire::FxSettlementStatus::Failed,
        }
    }
}

impl TryFrom<i32> for FxSettlementStatusV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: i32) -> Result<Self, Self::Error> {
        let status = wire::FxSettlementStatus::try_from(value).map_err(|_| {
            HydraFxConversionError::InvalidEnum {
                message: "FxSettleResponseV1",
                field: "status",
                value,
            }
        })?;
        match status {
            wire::FxSettlementStatus::Released => Ok(Self::Released),
            wire::FxSettlementStatus::Withheld => Ok(Self::Withheld),
            wire::FxSettlementStatus::Failed => Ok(Self::Failed),
            wire::FxSettlementStatus::Unspecified => Err(HydraFxConversionError::InvalidEnum {
                message: "FxSettleResponseV1",
                field: "status",
                value,
            }),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxRfqRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub requester_id: String,
    pub budget_scope_id: String,
    pub sell: FxMoneyV1,
    pub buy_asset: String,
    pub min_buy_amount: u64,
    pub max_spread_bps: u32,
    pub max_fee_bps: u32,
    pub max_latency_ms: u32,
    pub quote_ttl_seconds: u32,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxRfqRecordV1 {
    pub rfq_id: String,
    pub requester_id: String,
    pub budget_scope_id: String,
    pub sell: FxMoneyV1,
    pub buy_asset: String,
    pub min_buy_amount: u64,
    pub max_spread_bps: u32,
    pub max_fee_bps: u32,
    pub max_latency_ms: u32,
    pub quote_ttl_seconds: u32,
    pub status: String,
    pub created_at_unix: u64,
    pub expires_at_unix: u64,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxRfqResponseV1 {
    pub schema: String,
    pub rfq: FxRfqRecordV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxQuoteV1 {
    pub quote_id: String,
    pub rfq_id: String,
    pub provider_id: String,
    pub sell: FxMoneyV1,
    pub buy: FxMoneyV1,
    pub spread_bps: u32,
    pub fee_bps: u32,
    pub latency_ms: u32,
    pub reliability_bps: u32,
    pub valid_until_unix: u64,
    pub status: FxQuoteStatusV1,
    #[serde(default)]
    pub constraints: Value,
    pub quote_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxQuoteUpsertRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub quote: FxQuoteV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxQuoteUpsertResponseV1 {
    pub schema: String,
    pub quote: FxQuoteV1,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSelectRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub rfq_id: String,
    pub policy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSelectionFactorsV1 {
    pub expected_spread_bps: u32,
    pub expected_fee_bps: u32,
    pub confidence: f64,
    #[serde(default)]
    pub policy_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxDecisionReceiptLinkageV1 {
    pub receipt_schema: String,
    pub receipt_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSelectResponseV1 {
    pub schema: String,
    pub rfq_id: String,
    pub policy: String,
    pub decision_sha256: String,
    pub selected: FxQuoteV1,
    pub candidates: Vec<FxQuoteV1>,
    pub factors: FxSelectionFactorsV1,
    #[serde(default)]
    pub receipt: Option<FxDecisionReceiptLinkageV1>,
    pub decided_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSettleRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub rfq_id: String,
    pub quote_id: String,
    pub reservation_id: String,
    #[serde(default)]
    pub policy_context: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSettlementReceiptV1 {
    pub schema: String,
    pub receipt_id: String,
    pub settlement_id: String,
    pub rfq_id: String,
    pub quote_id: String,
    pub provider_id: String,
    pub sell: FxMoneyV1,
    pub buy: FxMoneyV1,
    pub spread_bps: u32,
    pub fee_bps: u32,
    pub settled_at_unix: u64,
    #[serde(default)]
    pub wallet_receipt: Option<WalletExecutionReceiptV1>,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FxSettleResponseV1 {
    pub schema: String,
    pub settlement_id: String,
    pub status: FxSettlementStatusV1,
    pub receipt: FxSettlementReceiptV1,
}

impl From<FxMoneyV1> for wire::FxMoneyV1 {
    fn from(value: FxMoneyV1) -> Self {
        Self {
            asset: value.asset,
            amount: value.amount,
            unit: value.unit,
        }
    }
}

impl From<wire::FxMoneyV1> for FxMoneyV1 {
    fn from(value: wire::FxMoneyV1) -> Self {
        Self {
            asset: value.asset,
            amount: value.amount,
            unit: value.unit,
        }
    }
}

impl From<WalletExecutionReceiptV1> for wire_lightning::WalletExecutionReceipt {
    fn from(value: WalletExecutionReceiptV1) -> Self {
        Self {
            receipt_version: value.receipt_version,
            receipt_id: value.receipt_id,
            request_id: value.request_id,
            wallet_id: value.wallet_id,
            host: value.host,
            payment_id: value.payment_id,
            invoice_hash: value.invoice_hash,
            quoted_amount_msats: value.quoted_amount_msats,
            settled_amount_msats: value.settled_amount_msats,
            preimage_sha256: value.preimage_sha256,
            paid_at_ms: value.paid_at_ms,
            rail: value.rail,
            asset_id: value.asset_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl From<wire_lightning::WalletExecutionReceipt> for WalletExecutionReceiptV1 {
    fn from(value: wire_lightning::WalletExecutionReceipt) -> Self {
        Self {
            receipt_version: value.receipt_version,
            receipt_id: value.receipt_id,
            request_id: value.request_id,
            wallet_id: value.wallet_id,
            host: value.host,
            payment_id: value.payment_id,
            invoice_hash: value.invoice_hash,
            quoted_amount_msats: value.quoted_amount_msats,
            settled_amount_msats: value.settled_amount_msats,
            preimage_sha256: value.preimage_sha256,
            paid_at_ms: value.paid_at_ms,
            rail: value.rail,
            asset_id: value.asset_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl TryFrom<FxRfqRequestV1> for wire::FxRfqRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxRfqRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            requester_id: value.requester_id,
            budget_scope_id: value.budget_scope_id,
            sell: Some(value.sell.into()),
            buy_asset: value.buy_asset,
            min_buy_amount: value.min_buy_amount,
            max_spread_bps: value.max_spread_bps,
            max_fee_bps: value.max_fee_bps,
            max_latency_ms: value.max_latency_ms,
            quote_ttl_seconds: value.quote_ttl_seconds,
            policy_context: Some(json_to_proto_struct(
                value.policy_context,
                "FxRfqRequestV1",
                "policy_context",
            )?),
        })
    }
}

impl TryFrom<wire::FxRfqRequestV1> for FxRfqRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxRfqRequestV1) -> Result<Self, Self::Error> {
        let sell = value
            .sell
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxRfqRequestV1",
                field: "sell",
            })?
            .into();
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            requester_id: value.requester_id,
            budget_scope_id: value.budget_scope_id,
            sell,
            buy_asset: value.buy_asset,
            min_buy_amount: value.min_buy_amount,
            max_spread_bps: value.max_spread_bps,
            max_fee_bps: value.max_fee_bps,
            max_latency_ms: value.max_latency_ms,
            quote_ttl_seconds: value.quote_ttl_seconds,
            policy_context: proto_struct_to_json(value.policy_context),
        })
    }
}

impl From<FxRfqRecordV1> for wire::FxRfqRecordV1 {
    fn from(value: FxRfqRecordV1) -> Self {
        Self {
            rfq_id: value.rfq_id,
            requester_id: value.requester_id,
            budget_scope_id: value.budget_scope_id,
            sell: Some(value.sell.into()),
            buy_asset: value.buy_asset,
            min_buy_amount: value.min_buy_amount,
            max_spread_bps: value.max_spread_bps,
            max_fee_bps: value.max_fee_bps,
            max_latency_ms: value.max_latency_ms,
            quote_ttl_seconds: value.quote_ttl_seconds,
            status: value.status,
            created_at_unix: value.created_at_unix,
            expires_at_unix: value.expires_at_unix,
            policy_context: Some(json_to_proto_struct_or_empty(value.policy_context)),
        }
    }
}

impl TryFrom<wire::FxRfqRecordV1> for FxRfqRecordV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxRfqRecordV1) -> Result<Self, Self::Error> {
        let sell = value
            .sell
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxRfqRecordV1",
                field: "sell",
            })?
            .into();
        Ok(Self {
            rfq_id: value.rfq_id,
            requester_id: value.requester_id,
            budget_scope_id: value.budget_scope_id,
            sell,
            buy_asset: value.buy_asset,
            min_buy_amount: value.min_buy_amount,
            max_spread_bps: value.max_spread_bps,
            max_fee_bps: value.max_fee_bps,
            max_latency_ms: value.max_latency_ms,
            quote_ttl_seconds: value.quote_ttl_seconds,
            status: value.status,
            created_at_unix: value.created_at_unix,
            expires_at_unix: value.expires_at_unix,
            policy_context: proto_struct_to_json(value.policy_context),
        })
    }
}

impl TryFrom<FxRfqResponseV1> for wire::FxRfqResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxRfqResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            rfq: Some(value.rfq.into()),
        })
    }
}

impl TryFrom<wire::FxRfqResponseV1> for FxRfqResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxRfqResponseV1) -> Result<Self, Self::Error> {
        let rfq = value.rfq.ok_or(HydraFxConversionError::MissingField {
            message: "FxRfqResponseV1",
            field: "rfq",
        })?;
        Ok(Self {
            schema: value.schema,
            rfq: rfq.try_into()?,
        })
    }
}

impl TryFrom<FxQuoteV1> for wire::FxQuoteV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxQuoteV1) -> Result<Self, Self::Error> {
        Ok(Self {
            quote_id: value.quote_id,
            rfq_id: value.rfq_id,
            provider_id: value.provider_id,
            sell: Some(value.sell.into()),
            buy: Some(value.buy.into()),
            spread_bps: value.spread_bps,
            fee_bps: value.fee_bps,
            latency_ms: value.latency_ms,
            reliability_bps: value.reliability_bps,
            valid_until_unix: value.valid_until_unix,
            status: wire::FxQuoteStatus::from(value.status) as i32,
            constraints: Some(json_to_proto_struct(
                value.constraints,
                "FxQuoteV1",
                "constraints",
            )?),
            quote_sha256: value.quote_sha256,
        })
    }
}

impl TryFrom<wire::FxQuoteV1> for FxQuoteV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxQuoteV1) -> Result<Self, Self::Error> {
        let sell = value.sell.ok_or(HydraFxConversionError::MissingField {
            message: "FxQuoteV1",
            field: "sell",
        })?;
        let buy = value.buy.ok_or(HydraFxConversionError::MissingField {
            message: "FxQuoteV1",
            field: "buy",
        })?;
        Ok(Self {
            quote_id: value.quote_id,
            rfq_id: value.rfq_id,
            provider_id: value.provider_id,
            sell: sell.into(),
            buy: buy.into(),
            spread_bps: value.spread_bps,
            fee_bps: value.fee_bps,
            latency_ms: value.latency_ms,
            reliability_bps: value.reliability_bps,
            valid_until_unix: value.valid_until_unix,
            status: FxQuoteStatusV1::try_from(value.status)?,
            constraints: proto_struct_to_json(value.constraints),
            quote_sha256: value.quote_sha256,
        })
    }
}

impl TryFrom<FxQuoteUpsertRequestV1> for wire::FxQuoteUpsertRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxQuoteUpsertRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            quote: Some(value.quote.try_into()?),
        })
    }
}

impl TryFrom<wire::FxQuoteUpsertRequestV1> for FxQuoteUpsertRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxQuoteUpsertRequestV1) -> Result<Self, Self::Error> {
        let quote = value.quote.ok_or(HydraFxConversionError::MissingField {
            message: "FxQuoteUpsertRequestV1",
            field: "quote",
        })?;
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            quote: quote.try_into()?,
        })
    }
}

impl TryFrom<FxQuoteUpsertResponseV1> for wire::FxQuoteUpsertResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxQuoteUpsertResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            quote: Some(value.quote.try_into()?),
        })
    }
}

impl TryFrom<wire::FxQuoteUpsertResponseV1> for FxQuoteUpsertResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxQuoteUpsertResponseV1) -> Result<Self, Self::Error> {
        let quote = value.quote.ok_or(HydraFxConversionError::MissingField {
            message: "FxQuoteUpsertResponseV1",
            field: "quote",
        })?;
        Ok(Self {
            schema: value.schema,
            quote: quote.try_into()?,
        })
    }
}

impl From<FxSelectRequestV1> for wire::FxSelectRequestV1 {
    fn from(value: FxSelectRequestV1) -> Self {
        Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            rfq_id: value.rfq_id,
            policy: value.policy,
        }
    }
}

impl From<wire::FxSelectRequestV1> for FxSelectRequestV1 {
    fn from(value: wire::FxSelectRequestV1) -> Self {
        Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            rfq_id: value.rfq_id,
            policy: value.policy,
        }
    }
}

impl From<FxSelectionFactorsV1> for wire::FxSelectionFactorsV1 {
    fn from(value: FxSelectionFactorsV1) -> Self {
        Self {
            expected_spread_bps: value.expected_spread_bps,
            expected_fee_bps: value.expected_fee_bps,
            confidence: value.confidence,
            policy_notes: value.policy_notes,
        }
    }
}

impl From<wire::FxSelectionFactorsV1> for FxSelectionFactorsV1 {
    fn from(value: wire::FxSelectionFactorsV1) -> Self {
        Self {
            expected_spread_bps: value.expected_spread_bps,
            expected_fee_bps: value.expected_fee_bps,
            confidence: value.confidence,
            policy_notes: value.policy_notes,
        }
    }
}

impl From<FxDecisionReceiptLinkageV1> for wire::FxDecisionReceiptLinkageV1 {
    fn from(value: FxDecisionReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl From<wire::FxDecisionReceiptLinkageV1> for FxDecisionReceiptLinkageV1 {
    fn from(value: wire::FxDecisionReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl TryFrom<FxSelectResponseV1> for wire::FxSelectResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxSelectResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            rfq_id: value.rfq_id,
            policy: value.policy,
            decision_sha256: value.decision_sha256,
            selected: Some(value.selected.try_into()?),
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            factors: Some(value.factors.into()),
            receipt: value.receipt.map(Into::into),
            decided_at_unix: value.decided_at_unix,
        })
    }
}

impl TryFrom<wire::FxSelectResponseV1> for FxSelectResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxSelectResponseV1) -> Result<Self, Self::Error> {
        let selected = value
            .selected
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxSelectResponseV1",
                field: "selected",
            })?
            .try_into()?;
        let factors = value
            .factors
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxSelectResponseV1",
                field: "factors",
            })?
            .into();

        Ok(Self {
            schema: value.schema,
            rfq_id: value.rfq_id,
            policy: value.policy,
            decision_sha256: value.decision_sha256,
            selected,
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            factors,
            receipt: value.receipt.map(Into::into),
            decided_at_unix: value.decided_at_unix,
        })
    }
}

impl TryFrom<FxSettleRequestV1> for wire::FxSettleRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxSettleRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            rfq_id: value.rfq_id,
            quote_id: value.quote_id,
            reservation_id: value.reservation_id,
            policy_context: Some(json_to_proto_struct(
                value.policy_context,
                "FxSettleRequestV1",
                "policy_context",
            )?),
        })
    }
}

impl TryFrom<wire::FxSettleRequestV1> for FxSettleRequestV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxSettleRequestV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            rfq_id: value.rfq_id,
            quote_id: value.quote_id,
            reservation_id: value.reservation_id,
            policy_context: proto_struct_to_json(value.policy_context),
        })
    }
}

impl TryFrom<FxSettlementReceiptV1> for wire::FxSettlementReceiptV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxSettlementReceiptV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            receipt_id: value.receipt_id,
            settlement_id: value.settlement_id,
            rfq_id: value.rfq_id,
            quote_id: value.quote_id,
            provider_id: value.provider_id,
            sell: Some(value.sell.into()),
            buy: Some(value.buy.into()),
            spread_bps: value.spread_bps,
            fee_bps: value.fee_bps,
            settled_at_unix: value.settled_at_unix,
            wallet_receipt: value.wallet_receipt.map(Into::into),
            canonical_json_sha256: value.canonical_json_sha256,
        })
    }
}

impl TryFrom<wire::FxSettlementReceiptV1> for FxSettlementReceiptV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxSettlementReceiptV1) -> Result<Self, Self::Error> {
        let sell = value
            .sell
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxSettlementReceiptV1",
                field: "sell",
            })?
            .into();
        let buy = value.buy.ok_or(HydraFxConversionError::MissingField {
            message: "FxSettlementReceiptV1",
            field: "buy",
        })?;

        Ok(Self {
            schema: value.schema,
            receipt_id: value.receipt_id,
            settlement_id: value.settlement_id,
            rfq_id: value.rfq_id,
            quote_id: value.quote_id,
            provider_id: value.provider_id,
            sell,
            buy: buy.into(),
            spread_bps: value.spread_bps,
            fee_bps: value.fee_bps,
            settled_at_unix: value.settled_at_unix,
            wallet_receipt: value.wallet_receipt.map(Into::into),
            canonical_json_sha256: value.canonical_json_sha256,
        })
    }
}

impl TryFrom<FxSettleResponseV1> for wire::FxSettleResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: FxSettleResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            settlement_id: value.settlement_id,
            status: wire::FxSettlementStatus::from(value.status) as i32,
            receipt: Some(value.receipt.try_into()?),
        })
    }
}

impl TryFrom<wire::FxSettleResponseV1> for FxSettleResponseV1 {
    type Error = HydraFxConversionError;

    fn try_from(value: wire::FxSettleResponseV1) -> Result<Self, Self::Error> {
        let receipt = value
            .receipt
            .ok_or(HydraFxConversionError::MissingField {
                message: "FxSettleResponseV1",
                field: "receipt",
            })?
            .try_into()?;
        Ok(Self {
            schema: value.schema,
            settlement_id: value.settlement_id,
            status: FxSettlementStatusV1::try_from(value.status)?,
            receipt,
        })
    }
}

fn json_to_proto_struct(
    value: Value,
    message: &'static str,
    field: &'static str,
) -> Result<Struct, HydraFxConversionError> {
    let Value::Object(map) = value else {
        return Err(HydraFxConversionError::InvalidObjectField { message, field });
    };
    let fields = map
        .into_iter()
        .map(|(key, value)| (key, json_to_proto_value(value)))
        .collect();
    Ok(Struct { fields })
}

fn json_to_proto_struct_or_empty(value: Value) -> Struct {
    match value {
        Value::Object(map) => Struct {
            fields: map
                .into_iter()
                .map(|(key, value)| (key, json_to_proto_value(value)))
                .collect(),
        },
        _ => Struct {
            fields: BTreeMap::new(),
        },
    }
}

fn proto_struct_to_json(value: Option<Struct>) -> Value {
    let Some(value) = value else {
        return Value::Object(Map::new());
    };
    let map = value
        .fields
        .into_iter()
        .map(|(key, value)| (key, proto_value_to_json(value)))
        .collect();
    Value::Object(map)
}

fn json_to_proto_value(value: Value) -> ProtoValue {
    let kind = match value {
        Value::Null => Kind::NullValue(0),
        Value::Bool(value) => Kind::BoolValue(value),
        Value::Number(value) => Kind::NumberValue(value.as_f64().unwrap_or(0.0)),
        Value::String(value) => Kind::StringValue(value),
        Value::Array(values) => Kind::ListValue(ListValue {
            values: values.into_iter().map(json_to_proto_value).collect(),
        }),
        Value::Object(values) => {
            let fields = values
                .into_iter()
                .map(|(key, value)| (key, json_to_proto_value(value)))
                .collect();
            Kind::StructValue(Struct { fields })
        }
    };
    ProtoValue { kind: Some(kind) }
}

fn proto_value_to_json(value: ProtoValue) -> Value {
    let Some(kind) = value.kind else {
        return Value::Null;
    };

    match kind {
        Kind::NullValue(_) => Value::Null,
        Kind::NumberValue(value) => {
            let number =
                serde_json::Number::from_f64(value).unwrap_or_else(|| serde_json::Number::from(0));
            Value::Number(number)
        }
        Kind::StringValue(value) => Value::String(value),
        Kind::BoolValue(value) => Value::Bool(value),
        Kind::StructValue(value) => {
            let map = value
                .fields
                .into_iter()
                .map(|(key, value)| (key, proto_value_to_json(value)))
                .collect();
            Value::Object(map)
        }
        Kind::ListValue(value) => Value::Array(
            value
                .values
                .into_iter()
                .map(proto_value_to_json)
                .collect::<Vec<_>>(),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_wallet_receipt() -> WalletExecutionReceiptV1 {
        WalletExecutionReceiptV1 {
            receipt_version: "openagents.lightning.wallet_receipt.v1".to_string(),
            receipt_id: "wallet_rcpt_1".to_string(),
            request_id: "wallet_req_1".to_string(),
            wallet_id: "wallet_1".to_string(),
            host: "alice@example.com".to_string(),
            payment_id: "payment_1".to_string(),
            invoice_hash: "hash_1".to_string(),
            quoted_amount_msats: 10_000,
            settled_amount_msats: 10_500,
            preimage_sha256: "preimage_hash".to_string(),
            paid_at_ms: 1_716_000_000_000,
            rail: "lightning".to_string(),
            asset_id: "BTC_LN".to_string(),
            canonical_json_sha256: "sha256:wallet".to_string(),
        }
    }

    #[test]
    fn rfq_request_wire_roundtrip_preserves_fields() {
        let request = FxRfqRequestV1 {
            schema: FX_RFQ_REQUEST_SCHEMA_V1.to_string(),
            idempotency_key: "idem-rfq-1".to_string(),
            requester_id: "autopilot:user-1".to_string(),
            budget_scope_id: "budget_scope_1".to_string(),
            sell: FxMoneyV1 {
                asset: "USD".to_string(),
                amount: 100_000,
                unit: "cents".to_string(),
            },
            buy_asset: "BTC_LN".to_string(),
            min_buy_amount: 2_500_000,
            max_spread_bps: 100,
            max_fee_bps: 50,
            max_latency_ms: 5_000,
            quote_ttl_seconds: 30,
            policy_context: json!({"policy":"balanced_v1","org":"org_1"}),
        };

        let wire_res: Result<wire::FxRfqRequestV1, _> = request.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<FxRfqRequestV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(restored, request);
    }

    #[test]
    fn settle_response_wire_roundtrip_preserves_receipt_and_wallet_receipt() {
        let response = FxSettleResponseV1 {
            schema: FX_SETTLE_RESPONSE_SCHEMA_V1.to_string(),
            settlement_id: "fx_settle_1".to_string(),
            status: FxSettlementStatusV1::Released,
            receipt: FxSettlementReceiptV1 {
                schema: FX_SETTLEMENT_RECEIPT_SCHEMA_V1.to_string(),
                receipt_id: "fx_receipt_1".to_string(),
                settlement_id: "fx_settle_1".to_string(),
                rfq_id: "rfq_1".to_string(),
                quote_id: "quote_1".to_string(),
                provider_id: "provider_1".to_string(),
                sell: FxMoneyV1 {
                    asset: "USD".to_string(),
                    amount: 100_000,
                    unit: "cents".to_string(),
                },
                buy: FxMoneyV1 {
                    asset: "BTC_LN".to_string(),
                    amount: 2_600_000,
                    unit: "msats".to_string(),
                },
                spread_bps: 80,
                fee_bps: 25,
                settled_at_unix: 1_716_000_123,
                wallet_receipt: Some(sample_wallet_receipt()),
                canonical_json_sha256: "sha256:fx_receipt".to_string(),
            },
        };

        let wire_res: Result<wire::FxSettleResponseV1, _> = response.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<FxSettleResponseV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(restored.status, FxSettlementStatusV1::Released);
        assert_eq!(restored.receipt.receipt_id, "fx_receipt_1");
        assert_eq!(
            restored
                .receipt
                .wallet_receipt
                .as_ref()
                .map(|value| value.receipt_id.as_str()),
            Some("wallet_rcpt_1"),
        );
    }
}
