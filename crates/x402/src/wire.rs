//! x402 v2 wire shapes for the HTTP transport and their header codecs.

use base64::Engine;
use base64::engine::general_purpose::STANDARD;
use nostr::x402::PaymentRequirements;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

pub const PAYMENT_REQUIRED: &str = "payment-required";
pub const PAYMENT_SIGNATURE: &str = "payment-signature";
pub const PAYMENT_RESPONSE: &str = "payment-response";

#[derive(Debug, thiserror::Error)]
pub enum WireError {
    #[error("header is not base64: {0}")]
    Base64(#[from] base64::DecodeError),
    #[error("header is not the expected JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("x402Version must be 2, got {0}")]
    Version(u64),
    #[error("header exceeds {0} bytes")]
    TooLarge(usize),
}

const MAX_HEADER_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceInfo {
    pub url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mime_type: Option<String>,
    #[serde(flatten)]
    pub rest: Map<String, Value>,
}

/// The `PAYMENT-REQUIRED` body a server sends with a 402.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentRequired {
    pub x402_version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub resource: ResourceInfo,
    pub accepts: Vec<PaymentRequirements>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Map<String, Value>>,
}

/// The `PAYMENT-SIGNATURE` body a buyer sends after paying.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaymentPayload {
    pub x402_version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource: Option<ResourceInfo>,
    pub accepted: PaymentRequirements,
    /// Scheme-specific; for `exact`/`lnbtc` it holds `preimage`.
    pub payload: Map<String, Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extensions: Option<Map<String, Value>>,
}

impl PaymentPayload {
    pub fn preimage(&self) -> Option<&str> {
        self.payload.get("preimage").and_then(Value::as_str)
    }
}

/// The `PAYMENT-RESPONSE` body. `payer` is omitted: Lightning has none.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlementResponse {
    pub success: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_reason: Option<String>,
    pub transaction: String,
    pub network: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<String>,
}

impl SettlementResponse {
    pub fn failed(network: &str, reason: &str) -> Self {
        Self {
            success: false,
            error_reason: Some(reason.to_string()),
            transaction: String::new(),
            network: network.to_string(),
            amount: None,
        }
    }
}

pub fn encode_header<T: Serialize>(value: &T) -> Result<String, WireError> {
    Ok(STANDARD.encode(serde_json::to_vec(value)?))
}

pub fn decode_header<T: for<'de> Deserialize<'de>>(header: &str) -> Result<T, WireError> {
    if header.len() > MAX_HEADER_BYTES {
        return Err(WireError::TooLarge(MAX_HEADER_BYTES));
    }
    let bytes = STANDARD.decode(header.trim())?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn decode_payment_required(header: &str) -> Result<PaymentRequired, WireError> {
    let required: PaymentRequired = decode_header(header)?;
    if required.x402_version != 2 {
        return Err(WireError::Version(required.x402_version));
    }
    Ok(required)
}

pub fn decode_payment_payload(header: &str) -> Result<PaymentPayload, WireError> {
    let payload: PaymentPayload = decode_header(header)?;
    if payload.x402_version != 2 {
        return Err(WireError::Version(payload.x402_version));
    }
    Ok(payload)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn round_trips_payment_required_and_payload() {
        let requirements: PaymentRequirements = serde_json::from_value(json!({
            "scheme":"exact","network":nostr::x402::MAINNET,"amount":"1000","asset":"BTC",
            "payTo":"02".repeat(33),"maxTimeoutSeconds":60,
            "extra":{"paymentFlow":"upfront","requestHash":"00".repeat(32),
                     "requestBindingProfile":"http:1","requestBindingParams":{"headers":[]},
                     "invoice":"lnbc1"}
        }))
        .unwrap();
        let required = PaymentRequired {
            x402_version: 2,
            error: Some("PAYMENT-SIGNATURE header is required".into()),
            resource: ResourceInfo {
                url: "https://example.com/x".into(),
                description: None,
                mime_type: Some("application/json".into()),
                rest: Map::new(),
            },
            accepts: vec![requirements.clone()],
            extensions: None,
        };
        let header = encode_header(&required).unwrap();
        assert!(decode_payment_required(&header).unwrap() == required);

        let mut payload = Map::new();
        payload.insert("preimage".into(), Value::String("00".repeat(32)));
        let signature = PaymentPayload {
            x402_version: 2,
            resource: None,
            accepted: requirements,
            payload,
            extensions: None,
        };
        let header = encode_header(&signature).unwrap();
        let decoded = decode_payment_payload(&header).unwrap();
        assert_eq!(decoded.preimage(), Some("00".repeat(32).as_str()));

        let bad = encode_header(&json!({"x402Version":1,"accepted":{},"payload":{}})).unwrap();
        assert!(decode_payment_payload(&bad).is_err());
    }
}
