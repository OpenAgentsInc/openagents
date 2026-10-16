//! NIP-SOV lifecycle artifacts: activations, checkpoints, transfers, and
//! fencing. A valid record never transfers authority by itself.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, parse_artifact,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/sovereign-lifecycle.v1.json");
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
        "sovereign-checkpoint" => {
            if (number(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Malformed, "checkpoint lineage"));
            }
        }
        "sovereign-activation-request" => {
            if !body["dispatchers"]
                .as_array()
                .is_some_and(|d| d.iter().any(|d| *d == body["controller"]))
            {
                return Err(error(
                    RefusalCode::Malformed,
                    "the controller is one of the dispatchers",
                ));
            }
        }
        "sovereign-transfer" => semantic("sovereign-activation-request", &body["to"])?,
        "sovereign-lifecycle-result" => {
            let status = body["status"].as_str().unwrap_or_default();
            if matches!(status, "refused" | "conflict") == body["reason"].is_null() {
                return Err(error(RefusalCode::Malformed, "result reason"));
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
                    "agent" | "activation" | "controller" | "dispatcher" | "command"
                ) && let Some(text) = value.as_str()
                {
                    hex(text)?;
                }
                if key == "dispatchers" || key == "recipients" {
                    for participant in value.as_array().into_iter().flatten() {
                        hex(participant.as_str().unwrap_or_default())?;
                    }
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
    fn checkpoints_chain_and_requests_name_their_controller() {
        let art = |s: &str| json!({"digest":contracts::digest_bytes(s.as_bytes()),"size":s.len(),"media_type":"application/json","schema":s});
        let mut checkpoint = json!({"v":"openagents.sovereign-checkpoint.v1","requires":[],
            "activation":art("openagents.sovereign-activation.v1"),"revision":1,"previous":null,
            "controller":"11".repeat(32),"generation":1,"record":art("openagents.run-record.v1"),
            "state":art("state"),"obligations":[]});
        assert!(validate(&checkpoint).is_err());
        checkpoint["revision"] = json!(0);
        validate(&checkpoint).unwrap();
    }
}
