use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use thiserror::Error;

use crate::wire::google::protobuf::{value::Kind, ListValue, Struct, Value as ProtoValue};
use crate::wire::openagents::hydra::v1 as wire;

pub const ROUTING_SCORE_REQUEST_SCHEMA_V1: &str = "openagents.hydra.routing_score_request.v1";
pub const ROUTING_SCORE_RESPONSE_SCHEMA_V1: &str = "openagents.hydra.routing_score_response.v1";

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum HydraRoutingConversionError {
    #[error("{message}.{field} is required")]
    MissingField {
        message: &'static str,
        field: &'static str,
    },
    #[error("{message}.{field} must be a JSON object")]
    InvalidObjectField {
        message: &'static str,
        field: &'static str,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingCandidateQuoteV1 {
    pub marketplace_id: String,
    pub provider_id: String,
    #[serde(default)]
    pub provider_worker_id: Option<String>,
    pub total_price_msats: u64,
    #[serde(default)]
    pub latency_ms: Option<u64>,
    pub reliability_bps: u32,
    #[serde(default)]
    pub constraints: Value,
    #[serde(default)]
    pub quote_id: Option<String>,
    #[serde(default)]
    pub quote_sha256: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingScoreRequestV1 {
    pub schema: String,
    pub idempotency_key: String,
    pub run_id: String,
    pub marketplace_id: String,
    pub capability: String,
    pub policy: String,
    #[serde(default)]
    pub objective_hash: Option<String>,
    pub decided_at_unix: u64,
    pub candidates: Vec<RoutingCandidateQuoteV1>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingDecisionFactorsV1 {
    pub expected_fee_msats: u64,
    pub confidence: f64,
    pub liquidity_score: f64,
    #[serde(default)]
    pub policy_notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingDecisionReceiptLinkageV1 {
    pub receipt_schema: String,
    pub receipt_id: String,
    pub canonical_json_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RoutingScoreResponseV1 {
    pub schema: String,
    pub decision_sha256: String,
    pub policy: String,
    pub run_id: String,
    pub marketplace_id: String,
    pub capability: String,
    #[serde(default)]
    pub objective_hash: Option<String>,
    pub selected: RoutingCandidateQuoteV1,
    pub candidates: Vec<RoutingCandidateQuoteV1>,
    pub factors: RoutingDecisionFactorsV1,
    #[serde(default)]
    pub receipt: Option<RoutingDecisionReceiptLinkageV1>,
    #[serde(default)]
    pub nostr_event: Value,
    pub decided_at_unix: u64,
}

impl TryFrom<RoutingCandidateQuoteV1> for wire::RoutingCandidateQuoteV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: RoutingCandidateQuoteV1) -> Result<Self, Self::Error> {
        Ok(Self {
            marketplace_id: value.marketplace_id,
            provider_id: value.provider_id,
            provider_worker_id: value.provider_worker_id,
            total_price_msats: value.total_price_msats,
            latency_ms: value.latency_ms,
            reliability_bps: value.reliability_bps,
            constraints: Some(json_to_proto_struct(
                value.constraints,
                "RoutingCandidateQuoteV1",
                "constraints",
            )?),
            quote_id: value.quote_id,
            quote_sha256: value.quote_sha256,
        })
    }
}

impl TryFrom<wire::RoutingCandidateQuoteV1> for RoutingCandidateQuoteV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: wire::RoutingCandidateQuoteV1) -> Result<Self, Self::Error> {
        Ok(Self {
            marketplace_id: value.marketplace_id,
            provider_id: value.provider_id,
            provider_worker_id: value.provider_worker_id,
            total_price_msats: value.total_price_msats,
            latency_ms: value.latency_ms,
            reliability_bps: value.reliability_bps,
            constraints: proto_struct_to_json(value.constraints),
            quote_id: value.quote_id,
            quote_sha256: value.quote_sha256,
        })
    }
}

impl TryFrom<RoutingScoreRequestV1> for wire::RoutingScoreRequestV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: RoutingScoreRequestV1) -> Result<Self, Self::Error> {
        let mut candidates = Vec::with_capacity(value.candidates.len());
        for candidate in value.candidates {
            candidates.push(candidate.try_into()?);
        }
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            marketplace_id: value.marketplace_id,
            capability: value.capability,
            policy: value.policy,
            objective_hash: value.objective_hash,
            decided_at_unix: value.decided_at_unix,
            candidates,
        })
    }
}

impl TryFrom<wire::RoutingScoreRequestV1> for RoutingScoreRequestV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: wire::RoutingScoreRequestV1) -> Result<Self, Self::Error> {
        let mut candidates = Vec::with_capacity(value.candidates.len());
        for candidate in value.candidates {
            candidates.push(candidate.try_into()?);
        }
        Ok(Self {
            schema: value.schema,
            idempotency_key: value.idempotency_key,
            run_id: value.run_id,
            marketplace_id: value.marketplace_id,
            capability: value.capability,
            policy: value.policy,
            objective_hash: value.objective_hash,
            decided_at_unix: value.decided_at_unix,
            candidates,
        })
    }
}

impl From<RoutingDecisionFactorsV1> for wire::RoutingDecisionFactorsV1 {
    fn from(value: RoutingDecisionFactorsV1) -> Self {
        Self {
            expected_fee_msats: value.expected_fee_msats,
            confidence: value.confidence,
            liquidity_score: value.liquidity_score,
            policy_notes: value.policy_notes,
        }
    }
}

impl From<wire::RoutingDecisionFactorsV1> for RoutingDecisionFactorsV1 {
    fn from(value: wire::RoutingDecisionFactorsV1) -> Self {
        Self {
            expected_fee_msats: value.expected_fee_msats,
            confidence: value.confidence,
            liquidity_score: value.liquidity_score,
            policy_notes: value.policy_notes,
        }
    }
}

impl From<RoutingDecisionReceiptLinkageV1> for wire::RoutingDecisionReceiptLinkageV1 {
    fn from(value: RoutingDecisionReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl From<wire::RoutingDecisionReceiptLinkageV1> for RoutingDecisionReceiptLinkageV1 {
    fn from(value: wire::RoutingDecisionReceiptLinkageV1) -> Self {
        Self {
            receipt_schema: value.receipt_schema,
            receipt_id: value.receipt_id,
            canonical_json_sha256: value.canonical_json_sha256,
        }
    }
}

impl TryFrom<RoutingScoreResponseV1> for wire::RoutingScoreResponseV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: RoutingScoreResponseV1) -> Result<Self, Self::Error> {
        Ok(Self {
            schema: value.schema,
            decision_sha256: value.decision_sha256,
            policy: value.policy,
            run_id: value.run_id,
            marketplace_id: value.marketplace_id,
            capability: value.capability,
            objective_hash: value.objective_hash,
            selected: Some(value.selected.try_into()?),
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            factors: Some(value.factors.into()),
            receipt: value.receipt.map(Into::into),
            nostr_event: Some(json_to_proto_struct(
                value.nostr_event,
                "RoutingScoreResponseV1",
                "nostr_event",
            )?),
            decided_at_unix: value.decided_at_unix,
        })
    }
}

impl TryFrom<wire::RoutingScoreResponseV1> for RoutingScoreResponseV1 {
    type Error = HydraRoutingConversionError;

    fn try_from(value: wire::RoutingScoreResponseV1) -> Result<Self, Self::Error> {
        let selected = value
            .selected
            .ok_or(HydraRoutingConversionError::MissingField {
                message: "RoutingScoreResponseV1",
                field: "selected",
            })?
            .try_into()?;
        let factors = value
            .factors
            .ok_or(HydraRoutingConversionError::MissingField {
                message: "RoutingScoreResponseV1",
                field: "factors",
            })?
            .into();

        Ok(Self {
            schema: value.schema,
            decision_sha256: value.decision_sha256,
            policy: value.policy,
            run_id: value.run_id,
            marketplace_id: value.marketplace_id,
            capability: value.capability,
            objective_hash: value.objective_hash,
            selected,
            candidates: value
                .candidates
                .into_iter()
                .map(TryInto::try_into)
                .collect::<Result<Vec<_>, _>>()?,
            factors,
            receipt: value.receipt.map(Into::into),
            nostr_event: proto_struct_to_json(value.nostr_event),
            decided_at_unix: value.decided_at_unix,
        })
    }
}

fn json_to_proto_struct(
    value: Value,
    message: &'static str,
    field: &'static str,
) -> Result<Struct, HydraRoutingConversionError> {
    let Value::Object(map) = value else {
        return Err(HydraRoutingConversionError::InvalidObjectField { message, field });
    };

    let fields = map
        .into_iter()
        .map(|(key, value)| (key, json_to_proto_value(value)))
        .collect();
    Ok(Struct { fields })
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

    #[test]
    fn routing_score_request_wire_roundtrip_preserves_fields() {
        let request = RoutingScoreRequestV1 {
            schema: ROUTING_SCORE_REQUEST_SCHEMA_V1.to_string(),
            idempotency_key: "idem-1".to_string(),
            run_id: "run_123".to_string(),
            marketplace_id: "openagents".to_string(),
            capability: "oa.sandbox_run.v1".to_string(),
            policy: "balanced_v1".to_string(),
            objective_hash: Some("sha256:abc".to_string()),
            decided_at_unix: 1_716_000_000,
            candidates: vec![RoutingCandidateQuoteV1 {
                marketplace_id: "openagents".to_string(),
                provider_id: "provider_1".to_string(),
                provider_worker_id: Some("worker_1".to_string()),
                total_price_msats: 10_000,
                latency_ms: Some(200),
                reliability_bps: 9_800,
                constraints: json!({"region":"us-central1"}),
                quote_id: Some("quote_1".to_string()),
                quote_sha256: Some("sha256:def".to_string()),
            }],
        };

        let wire_res: Result<wire::RoutingScoreRequestV1, _> = request.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<RoutingScoreRequestV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(restored, request);
    }

    #[test]
    fn routing_score_response_wire_roundtrip_preserves_receipt_and_nostr() {
        let response = RoutingScoreResponseV1 {
            schema: ROUTING_SCORE_RESPONSE_SCHEMA_V1.to_string(),
            decision_sha256: "sha256:decision".to_string(),
            policy: "balanced_v1".to_string(),
            run_id: "run_123".to_string(),
            marketplace_id: "openagents".to_string(),
            capability: "oa.sandbox_run.v1".to_string(),
            objective_hash: Some("sha256:obj".to_string()),
            selected: RoutingCandidateQuoteV1 {
                marketplace_id: "openagents".to_string(),
                provider_id: "provider_1".to_string(),
                provider_worker_id: None,
                total_price_msats: 9_000,
                latency_ms: Some(150),
                reliability_bps: 9_900,
                constraints: json!({"zone":"a"}),
                quote_id: Some("quote_1".to_string()),
                quote_sha256: Some("sha256:quote".to_string()),
            },
            candidates: vec![],
            factors: RoutingDecisionFactorsV1 {
                expected_fee_msats: 500,
                confidence: 0.97,
                liquidity_score: 0.88,
                policy_notes: vec!["fee_cap_ok".to_string(), "peer_health_ok".to_string()],
            },
            receipt: Some(RoutingDecisionReceiptLinkageV1 {
                receipt_schema: "openagents.hydra.routing_decision_receipt.v1".to_string(),
                receipt_id: "rcpt_123".to_string(),
                canonical_json_sha256: "sha256:receipt".to_string(),
            }),
            nostr_event: json!({"kind": 30402, "id": "ev_123"}),
            decided_at_unix: 1_716_000_123,
        };

        let wire_res: Result<wire::RoutingScoreResponseV1, _> = response.clone().try_into();
        assert!(wire_res.is_ok());
        let wire = match wire_res {
            Ok(value) => value,
            Err(_) => return,
        };
        let restored_res: Result<RoutingScoreResponseV1, _> = wire.try_into();
        assert!(restored_res.is_ok());
        let restored = match restored_res {
            Ok(value) => value,
            Err(_) => return,
        };
        assert_eq!(restored.decision_sha256, response.decision_sha256);
        assert_eq!(restored.receipt, response.receipt);
        assert_eq!(
            restored.nostr_event.get("kind").and_then(Value::as_f64),
            Some(30402.0),
        );
    }
}
