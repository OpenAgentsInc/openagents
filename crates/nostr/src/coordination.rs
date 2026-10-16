//! NIP-COORD wire records. Validation grants no resource or budget authority.
use crate::contracts::{self, ContractError, RefusalCode, SchemaClosure};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/coordination.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;
fn error(code: RefusalCode, field: &str) -> ContractError {
    ContractError::new(code, field)
}
pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict(bytes)?;
    validate(&body)?;
    Ok(body)
}
pub fn validate(body: &Value) -> Result<()> {
    if contracts::jcs(body)?.len() > contracts::MAX_BODY_BYTES {
        return Err(error(RefusalCode::LimitExceeded, "coordination body"));
    }
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "coordination version"))?;
    static DOC: OnceLock<Value> = OnceLock::new();
    let schema = DOC
        .get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled coordination schema"));
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| schema["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "coordination schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(
            RefusalCode::UnsupportedFeature,
            "coordination requires",
        ));
    }
    let fields = schema["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "coordination fields"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|key| !fields.contains_key(key)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "coordination field"));
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
    references(body)?;
    for key in ["controller", "worker", "producer", "owner"] {
        if let Some(key) = body[key].as_str() {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| error(RefusalCode::Malformed, "coordination principal"))?;
        }
    }
    if let Some(bounds) = body.get("bounds") {
        bounds_valid(bounds)?;
    }
    match name {
        "coordinator" => {
            id(&body["run"])?;
            text(&body["scope"], 1024)?;
        }
        "task-proposal" => {
            id(&body["task"])?;
            contracts::parse_effects(&body["effects"])?;
            if !body["parent"].is_null() {
                id(&body["parent"])?;
                if body["parent"] == body["task"] {
                    return Err(error(RefusalCode::Conflict, "task parent cycle"));
                }
            }
            for dependency in body["dependencies"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "dependencies"))?
            {
                id(dependency)?;
                if *dependency == body["task"] {
                    return Err(error(RefusalCode::Conflict, "task depends on itself"));
                }
            }
        }
        "claim-request" | "claim" => {
            let mut seen = BTreeSet::new();
            for resource in body["resources"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "claim resources"))?
            {
                text(&resource["id"], 1024)?;
                if !seen.insert(resource["id"].as_str().unwrap_or("")) {
                    return Err(error(RefusalCode::Conflict, "duplicate claimed resource"));
                }
            }
        }
        "coordination-control" => {
            if (body["action"] == "renew") == body["requested_until"].is_null() {
                return Err(error(
                    RefusalCode::Malformed,
                    "coordination renewal deadline",
                ));
            }
            if body["action"] == "renew" && body["claim"].is_null() {
                return Err(error(
                    RefusalCode::Malformed,
                    "renewal needs an exact claim",
                ));
            }
        }
        "coordination-result" => {
            let accepted = matches!(body["status"].as_str(), Some("accepted" | "duplicate"));
            if accepted && !body["reason"].is_null()
                || matches!(body["status"].as_str(), Some("conflict" | "refused"))
                    && body["reason"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "coordination result reason"));
            }
        }
        "background-plan" => {}
        "finding" => {}
        "duplicate-suggestion" => {}
        _ => {
            return Err(error(
                RefusalCode::UnsupportedFeature,
                "coordination semantics",
            ));
        }
    }
    Ok(())
}
pub fn bounds_valid(value: &Value) -> Result<()> {
    let bounds = value
        .as_array()
        .filter(|a| a.len() <= 12)
        .ok_or_else(|| error(RefusalCode::LimitExceeded, "coordination bounds"))?;
    let mut seen = BTreeSet::new();
    for bound in bounds {
        contracts::parse_bound(bound)?;
        if !seen.insert(bound["bound"].as_str().unwrap_or("")) {
            return Err(error(RefusalCode::Conflict, "duplicate bound assignment"));
        }
    }
    Ok(())
}
fn references(value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            if object.contains_key("digest") && object.contains_key("media_type") {
                contracts::parse_artifact(value)?;
                return Ok(());
            }
            if object.len() == 2 && object.contains_key("id") && object.contains_key("artifact") {
                contracts::parse_definition(value)?;
                return Ok(());
            }
            if object.len() == 3
                && object.contains_key("id")
                && object.contains_key("pubkey")
                && object.contains_key("kind")
            {
                id(&value["id"])?;
                value["pubkey"]
                    .as_str()
                    .ok_or_else(|| error(RefusalCode::Malformed, "event pubkey"))?
                    .parse::<secp256k1::XOnlyPublicKey>()
                    .map_err(|_| error(RefusalCode::Malformed, "event pubkey"))?;
                return Ok(());
            }
            for (key, value) in object {
                if key != "meta" {
                    references(value)?;
                }
            }
        }
        Value::Array(values) => {
            let mut seen = BTreeSet::new();
            for value in values {
                if !seen.insert(contracts::jcs(value)?) {
                    return Err(error(
                        RefusalCode::Conflict,
                        "duplicate coordination list entry",
                    ));
                }
                references(value)?;
            }
        }
        Value::String(value) if value.len() > 1024 => {
            return Err(error(RefusalCode::LimitExceeded, "coordination string"));
        }
        _ => {}
    }
    Ok(())
}
pub fn id(value: &Value) -> Result<&str> {
    value
        .as_str()
        .filter(|s| {
            s.len() == 64
                && s.bytes()
                    .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
        })
        .ok_or_else(|| error(RefusalCode::Malformed, "coordination 64-hex ID"))
}
fn text(value: &Value, max: usize) -> Result<()> {
    if value.as_str().is_none_or(|s| s.is_empty() || s.len() > max) {
        return Err(error(RefusalCode::Malformed, "coordination text"));
    }
    Ok(())
}
/// The deterministic equivalence key. Callers still check rights, freshness,
/// retained verification, and trial identity before reusing a result.
pub fn reuse_key(scope: &str, proposal: &Value) -> Result<String> {
    validate(proposal)?;
    if proposal["v"] != "openagents.task-proposal.v1" {
        return Err(error(RefusalCode::Malformed, "proposal required"));
    }
    let mut body = serde_json::Map::from_iter([("scope".into(), Value::String(scope.into()))]);
    for field in [
        "frame",
        "target",
        "lock",
        "input",
        "context",
        "effects",
        "bounds",
        "acceptance",
    ] {
        body.insert(field.into(), proposal[field].clone());
    }
    contracts::digest_value(&Value::Object(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn artifact(bytes: &[u8]) -> Value {
        json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"})
    }
    fn proposal() -> Value {
        let key = crate::domain::RelaySigner::from_secret_hex(&"01".repeat(32))
            .unwrap()
            .pubkey()
            .to_string();
        json!({"v":"openagents.task-proposal.v1","requires":[],"task":"11".repeat(32),"coordinator":artifact(b"coordinator"),"frame":artifact(b"frame"),"parent":null,"target":{"id":format!("{key}:coord/data"),"artifact":artifact(b"operation")},"lock":artifact(b"lock"),"input":artifact(b"input"),"context":artifact(b"context"),"dependencies":[],"effects":{"reads":[],"writes":[],"network":[],"process":false,"delegates":false,"spend":false},"bounds":[{"bound":"calls","ceiling":1,"assurance":"host","mechanism":"native-count"}],"acceptance":artifact(b"policy"),"reuse":"never"})
    }
    #[test]
    fn proposals_are_closed_and_dependencies_cannot_self_link() {
        let mut value = proposal();
        validate(&value).unwrap();
        value["dependencies"] = json!([value["task"]]);
        assert!(validate(&value).is_err());
        value = proposal();
        value["execute"] = json!(true);
        assert!(validate(&value).is_err());
    }
    #[test]
    fn exact_reuse_keys_bind_real_input_and_environment_bytes() {
        let value = proposal();
        let key = reuse_key("scope", &value).unwrap();
        let mut another = value.clone();
        another["task"] = json!("22".repeat(32));
        assert_eq!(reuse_key("scope", &another).unwrap(), key);
        another["input"] = artifact(b"different independent trial");
        assert_ne!(reuse_key("scope", &another).unwrap(), key);
        assert_ne!(reuse_key("another-scope", &value).unwrap(), key);
    }
    #[test]
    fn duplicate_bounds_and_untyped_renewals_refuse() {
        let mut value = proposal();
        value["bounds"] = json!([value["bounds"][0], value["bounds"][0]]);
        assert!(validate(&value).is_err());
        let control = json!({"v":"openagents.coordination-control.v1","requires":[],"coordinator":artifact(b"scope"),"proposal":artifact(b"proposal"),"claim":null,"action":"renew","expected_revision":0,"requested_until":100,"evidence":[]});
        assert!(validate(&control).is_err());
    }
}
