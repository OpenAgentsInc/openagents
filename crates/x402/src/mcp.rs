//! One paid MCP tool over the upstream x402 MCP transport (`mcp:1`).
//!
//! The wire is `specs/transports-v2/mcp.md` at the pinned x402 commit: a
//! `tools/call` without payment is answered with a tool result whose
//! `isError` is true and whose `structuredContent` and `content[0].text`
//! both carry the `PaymentRequired` document; the buyer retries with the
//! `PaymentPayload` under `_meta["x402/payment"]`; the server settles and
//! reports the `SettlementResponse` under `_meta["x402/payment-response"]`.
//! The request is bound from the configured server URI, the tool name, and
//! its `arguments`; no `_meta` name is bound, so the binding is the same
//! before and after the payment is attached.

use std::sync::Arc;

use nostr::x402::{PaymentRequirements, binding_hash, mcp_binding};
use serde_json::{Map, Value, json};

use crate::facilitator::Facilitator;
use crate::replay::ReplayStore;
use crate::server::Receiver;
use crate::wire::{PaymentPayload, PaymentRequired, ResourceInfo, SettlementResponse};

/// The `_meta` name the buyer's payment travels under.
pub const PAYMENT_META: &str = "x402/payment";
/// The `_meta` name the settlement travels back under.
pub const PAYMENT_RESPONSE_META: &str = "x402/payment-response";
/// The `requestBindingParams` of every challenge this module issues.
pub const BOUND_METADATA: [String; 0] = [];

/// The toll on one MCP server's tools.
pub struct PaidTools<S: ReplayStore> {
    /// The absolute URI that names this server in every binding.
    pub server: String,
    pub network: &'static str,
    pub amount_msat: u64,
    pub timeout_secs: u32,
    pub description: String,
    pub receiver: Arc<dyn Receiver>,
    pub facilitator: Facilitator<S>,
}

/// What the toll decided about one `tools/call`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gate {
    /// No payment came; answer with this tool result and do not run.
    Challenge(Value),
    /// The payment did not settle; answer with this tool result and do not run.
    Refused(Value),
    /// Settled; run the tool and put `settlement` under `_meta` of the result.
    Admitted {
        request_hash: String,
        payment_hash: String,
        settlement: Value,
    },
}

impl<S: ReplayStore> PaidTools<S> {
    fn requirements(&self, request_hash: &str, invoice: &str) -> PaymentRequirements {
        let mut extra = Map::new();
        extra.insert("assetTransferMethod".into(), json!("bolt11"));
        extra.insert("paymentFlow".into(), json!("upfront"));
        extra.insert("requestHash".into(), json!(request_hash));
        extra.insert("requestBindingProfile".into(), json!("mcp:1"));
        extra.insert(
            "requestBindingParams".into(),
            json!({"server": self.server, "metadata": BOUND_METADATA}),
        );
        extra.insert("invoice".into(), json!(invoice));
        PaymentRequirements {
            scheme: "exact".into(),
            network: self.network.into(),
            amount: self.amount_msat.to_string(),
            asset: "BTC".into(),
            pay_to: self.receiver.pay_to(),
            max_timeout_seconds: u64::from(self.timeout_secs),
            extra,
        }
    }

    /// The request hash of one `tools/call` `params` document.
    pub fn request_hash(&self, params: &Value) -> Result<String, &'static str> {
        let binding = mcp_binding(&self.server, params, &BOUND_METADATA)
            .map_err(|_| "the tool call cannot be bound")?;
        binding_hash(&binding).map_err(|_| "the tool call cannot be bound")
    }

    /// Decide one `tools/call` at time `now`.
    pub fn gate(&self, params: &Value, now: u64) -> Result<Gate, &'static str> {
        let request_hash = self.request_hash(params)?;
        let payment = params.get("_meta").and_then(|meta| meta.get(PAYMENT_META));
        let Some(payment) = payment else {
            let mut digest = [0u8; 32];
            hex::decode_to_slice(&request_hash, &mut digest).map_err(|_| "binding digest")?;
            let invoice = self
                .receiver
                .invoice(self.amount_msat, digest, self.timeout_secs)
                .map_err(|_| "exact_lnbtc_invoice_issuance_denied")?;
            let tool = params
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let required = PaymentRequired {
                x402_version: 2,
                error: Some(format!("_meta[\"{PAYMENT_META}\"] is required")),
                resource: ResourceInfo {
                    url: self.server.clone(),
                    description: Some(format!("{} ({tool})", self.description)),
                    mime_type: Some("application/json".into()),
                    rest: Map::new(),
                },
                accepts: vec![self.requirements(&request_hash, &invoice)],
                extensions: None,
            };
            let document = serde_json::to_value(&required).map_err(|_| "challenge encoding")?;
            return Ok(Gate::Challenge(payment_required_result(&document)));
        };
        let payload = match serde_json::from_value::<PaymentPayload>(payment.clone()) {
            Ok(payload) if payload.x402_version == 2 => payload,
            _ => return Ok(Gate::Refused(self.refusal("invalid_payment_payload"))),
        };
        let Some(accepted_invoice) = payload
            .accepted
            .extra
            .get("invoice")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
        else {
            return Ok(Gate::Refused(
                self.refusal("invalid_exact_lnbtc_invoice_missing"),
            ));
        };
        let requirements = self.requirements(&request_hash, accepted_invoice);
        match self
            .facilitator
            .settle(&requirements, &payload, &request_hash, now)
        {
            Ok(admitted) => Ok(Gate::Admitted {
                request_hash,
                payment_hash: admitted.proof.payment_hash.clone(),
                settlement: serde_json::to_value(&admitted.response)
                    .map_err(|_| "settlement encoding")?,
            }),
            Err(settlement) => {
                let reason = settlement
                    .error_reason
                    .clone()
                    .unwrap_or_else(|| "settlement_failed".into());
                Ok(Gate::Refused(self.refusal(&reason)))
            }
        }
    }

    fn refusal(&self, reason: &str) -> Value {
        let settlement = SettlementResponse::failed(self.network, reason);
        json!({
            "content": [{ "type": "text", "text": format!("payment refused: {reason}") }],
            "structuredContent": { "error": reason, "x402Version": 2 },
            "isError": true,
            "_meta": { PAYMENT_RESPONSE_META: settlement },
        })
    }
}

/// The payment-required tool result for one `PaymentRequired` document.
pub fn payment_required_result(required: &Value) -> Value {
    json!({
        "content": [{ "type": "text", "text": required.to_string() }],
        "structuredContent": required,
        "isError": true,
    })
}

/// Put `settlement` under `_meta` of a finished tool result.
pub fn with_settlement(mut result: Value, settlement: Value) -> Value {
    if let Some(object) = result.as_object_mut() {
        let meta = object
            .entry("_meta")
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(meta) = meta.as_object_mut() {
            meta.insert(PAYMENT_RESPONSE_META.into(), settlement);
        }
    }
    result
}

/// The `PaymentRequired` a buyer reads out of a payment-required tool result,
/// when that is what the result is. Both copies must agree.
pub fn payment_required_from_result(result: &Value) -> Option<PaymentRequired> {
    if result.get("isError") != Some(&Value::Bool(true)) {
        return None;
    }
    let structured = result.get("structuredContent")?;
    let text = result
        .get("content")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?;
    let from_text: Value = serde_json::from_str(text).ok()?;
    if &from_text != structured {
        return None;
    }
    let required: PaymentRequired = serde_json::from_value(structured.clone()).ok()?;
    (required.x402_version == 2).then_some(required)
}

/// Attach `payload` to `params` under `_meta["x402/payment"]`.
pub fn with_payment(mut params: Value, payload: &PaymentPayload) -> Result<Value, &'static str> {
    let payment = serde_json::to_value(payload).map_err(|_| "payment encoding")?;
    let object = params.as_object_mut().ok_or("params must be an object")?;
    let meta = object
        .entry("_meta")
        .or_insert_with(|| Value::Object(Map::new()));
    meta.as_object_mut()
        .ok_or("_meta must be an object")?
        .insert(PAYMENT_META.into(), payment);
    Ok(params)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::replay::FileReplayStore;
    use nostr::x402::MAINNET;

    // The pinned upstream vector also used by `crates/nostr` and `facilitator`.
    const INVOICE: &str = "lnbc250n1pj48ugqpp54y3u9s8ylemsv8l3ewyzzu0klhujvuvmkl6llchq23vy8rzjsf0qsp5zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zyg3zygshp5p4nz8am4uqj4q8a87z3sk4x6yk4dv2mvel34epw68qqkwy0xcqvqxqzfvcqpjr4rx6ls6j5rpwknuea64evlk7yfx56wmqcer5eerekdsn9tlv6v4ex9mlz5dtm9qapl3svwlqcf7837dmjkru9z9w4h2rvm0md52w2sqxrwu5f";
    const PREIMAGE: &str = "0001020304050607080900010203040506070809000102030405060708090102";
    const PAYEE: &str = "0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
    const NOW: u64 = 1_700_000_000;

    struct Fixed;

    impl Receiver for Fixed {
        fn pay_to(&self) -> String {
            PAYEE.into()
        }
        fn invoice(&self, _: u64, _: [u8; 32], _: u32) -> Result<String, String> {
            Ok(INVOICE.into())
        }
    }

    fn tools(dir: &std::path::Path) -> PaidTools<FileReplayStore> {
        PaidTools {
            server: "mcp://openagents.test/tools".into(),
            network: MAINNET,
            amount_msat: 25_000,
            timeout_secs: 300,
            description: "openagents".into(),
            receiver: Arc::new(Fixed),
            facilitator: Facilitator::with_profiles(
                FileReplayStore::open(dir).unwrap(),
                60,
                crate::facilitator::MCP_ONLY,
            ),
        }
    }

    fn params() -> Value {
        json!({"name": "version", "arguments": {"args": []}})
    }

    #[test]
    fn challenges_and_refuses_over_meta() {
        let dir = std::env::temp_dir().join(format!("oa-x402-mcp-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let tools = tools(&dir);

        let Gate::Challenge(result) = tools.gate(&params(), NOW).unwrap() else {
            panic!("expected a challenge");
        };
        let required = payment_required_from_result(&result).expect("both copies agree");
        let terms = &required.accepts[0];
        assert_eq!(
            terms.extra["requestBindingProfile"],
            json!("mcp:1"),
            "the profile is the MCP one"
        );
        assert_eq!(
            terms.extra["requestBindingParams"],
            json!({"server": "mcp://openagents.test/tools", "metadata": []})
        );
        assert_eq!(
            terms.extra["requestHash"].as_str().unwrap(),
            tools.request_hash(&params()).unwrap()
        );

        let mut payload = Map::new();
        payload.insert("preimage".into(), json!(PREIMAGE));
        let payment = PaymentPayload {
            x402_version: 2,
            resource: Some(required.resource.clone()),
            accepted: terms.clone(),
            payload,
            extensions: None,
        };
        let paid = with_payment(params(), &payment).unwrap();
        assert_eq!(
            tools.request_hash(&paid).unwrap(),
            tools.request_hash(&params()).unwrap(),
            "attaching the payment does not move the binding"
        );
        // The pinned invoice is signed over a different request hash, so the
        // proof reaches the invoice check and is refused there; settlement and
        // replay of a matching proof are the facilitator's tests.
        let Gate::Refused(refused) = tools.gate(&paid, NOW).unwrap() else {
            panic!("expected a refusal");
        };
        assert_eq!(refused["isError"], json!(true));
        assert_eq!(
            refused["structuredContent"]["error"],
            json!("invalid_exact_lnbtc_invoice_request_mismatch")
        );
        assert_eq!(
            refused["_meta"][PAYMENT_RESPONSE_META]["success"],
            json!(false)
        );

        let mut other = params();
        other["arguments"]["args"] = json!(["--help"]);
        let Gate::Refused(refused) = tools
            .gate(&with_payment(other, &payment).unwrap(), NOW)
            .unwrap()
        else {
            panic!("expected a binding refusal");
        };
        assert_eq!(
            refused["structuredContent"]["error"],
            json!("invalid_exact_lnbtc_request_mismatch")
        );

        let mut garbage = params();
        garbage["_meta"] = json!({ PAYMENT_META: {"x402Version": 1} });
        let Gate::Refused(refused) = tools.gate(&garbage, NOW).unwrap() else {
            panic!("expected a payload refusal");
        };
        assert_eq!(
            refused["structuredContent"]["error"],
            json!("invalid_payment_payload")
        );

        let settled = with_settlement(
            json!({"content": [], "isError": false}),
            json!({"success": true}),
        );
        assert_eq!(
            settled["_meta"][PAYMENT_RESPONSE_META]["success"],
            json!(true)
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_plain_error_result_is_not_a_challenge() {
        assert!(
            payment_required_from_result(
                &json!({"isError": true, "content": [{"type":"text","text":"no"}]})
            )
            .is_none()
        );
        let doc = json!({"x402Version": 2, "resource": {"url": "mcp://x"}, "accepts": []});
        let mut result = payment_required_result(&doc);
        assert!(payment_required_from_result(&result).is_some());
        result["content"][0]["text"] = json!("{}");
        assert!(payment_required_from_result(&result).is_none());
    }
}
