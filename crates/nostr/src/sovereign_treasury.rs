//! NIP-SOV treasury artifacts. A valid record is evidence of what its signer
//! claims; it is never a spendable balance or a payment authorization.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, parse_artifact,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/sovereign-treasury.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;

pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict_bounded(bytes, 1_048_576)?;
    validate(&body)?;
    Ok(body)
}

/// Validate one closed LIVE artifact, including rules the schema cannot say.
pub fn validate(body: &Value) -> Result<()> {
    if contracts::jcs(body)?.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "sovereign body"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc =
        DOC.get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled sovereign schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "sovereign version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| doc["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "sovereign schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "sovereign requires"));
    }
    let fields = doc["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "sovereign properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !fields.contains_key(k)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "sovereign field"));
    }
    static CLOSURE: OnceLock<Result<SchemaClosure>> = OnceLock::new();
    let closure = CLOSURE
        .get_or_init(|| {
            contracts::prepare_closure(&BTreeMap::from([(
                contracts::digest_bytes(SCHEMA_BYTES),
                SCHEMA_BYTES.to_vec(),
            )]))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    contracts::validate_instance(closure, &contracts::digest_bytes(SCHEMA_BYTES), body)?;
    nested(body)?;
    semantic(name, body)
}

fn semantic(name: &str, body: &Value) -> Result<()> {
    match name {
        "sovereign-treasury-policy" => {
            if (number(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Malformed, "policy lineage"));
            }
            if number(body, "starts_at")? >= number(body, "expires_at")? {
                return Err(error(RefusalCode::Malformed, "policy validity"));
            }
            if number(body, "per_operation_msat")? > number(body, "allowance_msat")? {
                return Err(error(
                    RefusalCode::Malformed,
                    "per-operation ceiling exceeds the allowance",
                ));
            }
            if (body["income"]["mode"] == "none") != (body["income"]["max_msat"] == 0) {
                return Err(error(RefusalCode::Malformed, "income replenishment bound"));
            }
        }
        "sovereign-purchase-receipt" => {
            let status = body["status"].as_str().unwrap_or_default();
            if (status == "refused") == body["reason"].is_null() {
                return Err(error(RefusalCode::Malformed, "receipt reason"));
            }
            if status == "completed" && (body["payment"].is_null() || body["credit"].is_null()) {
                return Err(error(RefusalCode::Malformed, "completed purchase evidence"));
            }
        }
        _ => {}
    }
    Ok(())
}

fn nested(value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            if object.contains_key("digest") && object.contains_key("size") {
                parse_artifact(value)?;
                return Ok(());
            }
            if object.contains_key("id") && object.contains_key("artifact") && object.len() <= 3 {
                contracts::parse_definition(value)?;
                return Ok(());
            }
            for (key, value) in object {
                if key == "meta" {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "agent"
                        | "authority"
                        | "beneficiary"
                        | "authorizer"
                        | "reconciliation"
                        | "purchase"
                        | "requester"
                        | "principal"
                ) && let Some(text) = value.as_str()
                {
                    hex(text)?;
                }
                nested(value)?;
            }
        }
        Value::Array(values) => {
            let mut seen = BTreeSet::new();
            for value in values {
                if !seen.insert(contracts::jcs(value)?) {
                    return Err(error(RefusalCode::Conflict, "duplicate list entry"));
                }
                nested(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Check exact bytes against a reference and validate them as `schema`.
pub fn verify(reference: &Value, bytes: &[u8], schema: &str) -> Result<Value> {
    check_artifact_bytes(&parse_artifact(reference)?, bytes)?;
    let body = parse(bytes)?;
    if body["v"] != schema || reference.get("schema").is_some_and(|s| *s != body["v"]) {
        return Err(error(RefusalCode::IdentityMismatch, "sovereign schema"));
    }
    Ok(body)
}

pub fn number(body: &Value, key: &str) -> Result<u64> {
    body[key]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| error(RefusalCode::Malformed, key))
}

pub fn error(code: RefusalCode, detail: &str) -> ContractError {
    ContractError::new(code, detail)
}

fn hex(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        return Err(error(RefusalCode::Malformed, "64-hex identity"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn income_replenishment_is_explicit_and_bounded() {
        let art = |s: &str| json!({"digest":contracts::digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json"});
        let mut policy = json!({"v":"openagents.sovereign-treasury-policy.v1","requires":[],"agent":"11".repeat(32),
            "authority":"22".repeat(32),"revision":0,"previous":null,"account":{"id":"acct","control":art("c")},
            "beneficiary":"33".repeat(32),"wallet":{"adapter":{"id":format!("{}:wallet/fake","22".repeat(32)),"artifact":art("w")},
            "account":"agent-wallet","fee_limit":true,"lookup":true},"authorizer":"44".repeat(32),
            "destinations":[{"provider":"node","resource":"https://example.test/topup"}],"profile":"x402-http-lightning",
            "network":"regtest","unit":"msat","per_operation_msat":10,"fee_ceiling_msat":1,"allowance_msat":100,
            "period_seconds":60,"income":{"mode":"none","max_msat":0},"spenders":[{"principal":"55".repeat(32),"cap_msat":50}],
            "reconciliation":"66".repeat(32),"starts_at":1,"expires_at":2});
        validate(&policy).unwrap();
        policy["income"]["max_msat"] = json!(5);
        assert!(validate(&policy).is_err());
        policy["income"] = json!({"mode":"replenish","max_msat":5});
        validate(&policy).unwrap();
        policy["per_operation_msat"] = json!(1000);
        assert!(validate(&policy).is_err());
    }
}
