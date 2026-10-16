//! Pure NIP-POL action and approval checks. Signer authority, freshness,
//! consumption, and actual dispatch remain the host's responsibility.
use crate::contracts::{self, ContractError, RefusalCode};
use serde_json::Value;
use std::collections::BTreeSet;

pub const ACTION_SCHEMA: &str = "openagents.action.v1";
pub const REQUEST_SCHEMA: &str = "openagents.approval-request.v1";
pub const DECISION_SCHEMA: &str = "openagents.approval-decision.v1";
pub type Result<T> = std::result::Result<T, ContractError>;

fn error(code: RefusalCode, detail: &str) -> ContractError {
    ContractError::new(code, detail)
}

pub fn parse(bytes: &[u8]) -> Result<Value> {
    let value = contracts::parse_strict_bounded(bytes, 1_048_576)?;
    validate(&value)?;
    Ok(value)
}

pub fn validate(body: &Value) -> Result<()> {
    if contracts::jcs(body)?.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "POL body"));
    }
    match body["v"].as_str() {
        Some(REQUEST_SCHEMA | DECISION_SCHEMA) => return crate::sovereign::validate(body),
        Some(ACTION_SCHEMA) => {}
        _ => return Err(error(RefusalCode::UnsupportedVersion, "POL schema")),
    }
    let required = [
        "v",
        "requires",
        "task_frame",
        "snapshot",
        "operation",
        "binding",
        "input",
        "context",
        "effects",
        "bounds",
        "recipient",
        "preconditions",
        "nonce",
    ];
    let fields = body
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "POL action"))?;
    if required.iter().any(|key| !fields.contains_key(*key)) {
        return Err(error(RefusalCode::Malformed, "POL action field"));
    }
    if fields
        .keys()
        .any(|key| !required.contains(&key.as_str()) && key != "meta")
    {
        return Err(error(RefusalCode::UnsupportedFeature, "POL action field"));
    }
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "POL requires"));
    }
    if fields.get("meta").is_some_and(|meta| !meta.is_object()) {
        return Err(error(RefusalCode::Malformed, "POL meta"));
    }
    for field in [
        "task_frame",
        "snapshot",
        "binding",
        "input",
        "context",
        "preconditions",
    ] {
        contracts::parse_artifact(&body[field])?;
    }
    contracts::parse_definition(&body["operation"])?;
    if !body["recipient"].is_null() {
        contracts::parse_artifact(&body["recipient"])?;
    }
    contracts::parse_effects(&body["effects"])?;
    let bounds = body["bounds"]
        .as_array()
        .ok_or_else(|| error(RefusalCode::Malformed, "POL bounds"))?;
    if bounds.len() > 64 {
        return Err(error(RefusalCode::LimitExceeded, "POL bounds"));
    }
    let mut seen = BTreeSet::new();
    for bound in bounds {
        contracts::parse_bound(bound)?;
        if !seen.insert(contracts::jcs(bound)?) {
            return Err(error(RefusalCode::Conflict, "duplicate POL bound"));
        }
    }
    let nonce = body["nonce"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "POL nonce"))?;
    if nonce.len() != 64
        || !nonce
            .bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
    {
        return Err(error(RefusalCode::Malformed, "POL nonce"));
    }
    Ok(())
}

pub fn verify(reference: &Value, bytes: &[u8], schema: &str) -> Result<Value> {
    let artifact = contracts::parse_artifact(reference)?;
    if artifact.schema.as_deref() != Some(schema) {
        return Err(error(RefusalCode::IdentityMismatch, "POL artifact schema"));
    }
    contracts::check_artifact_bytes(&artifact, bytes)?;
    let body = parse(bytes)?;
    if body["v"] != schema {
        return Err(error(RefusalCode::IdentityMismatch, "POL body schema"));
    }
    Ok(body)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn reference(bytes: &[u8], schema: Option<&str>) -> Value {
        let mut value = json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
        if let Some(schema) = schema {
            value["schema"] = json!(schema);
        }
        value
    }
    fn action() -> Value {
        let a = reference(b"input", None);
        json!({"v":ACTION_SCHEMA,"requires":[],"task_frame":a,"snapshot":a,
            "operation":{"id":format!("{}:session/hash","11".repeat(32)),"artifact":a},
            "binding":a,"input":a,"context":a,"effects":{"reads":["artifact"],"writes":[],"network":[],
            "process":false,"delegates":false,"spend":false},"bounds":[],"recipient":null,"preconditions":a,"nonce":"22".repeat(32)})
    }
    #[test]
    fn exact_action_bytes_and_all_required_pins_are_checked() {
        let body = action();
        validate(&body).unwrap();
        for field in [
            "task_frame",
            "snapshot",
            "operation",
            "binding",
            "input",
            "context",
            "effects",
            "bounds",
            "recipient",
            "preconditions",
            "nonce",
        ] {
            let mut missing = body.clone();
            missing.as_object_mut().unwrap().remove(field);
            assert!(validate(&missing).is_err());
        }
        let bytes = contracts::jcs(&body).unwrap();
        let r = reference(&bytes, Some(ACTION_SCHEMA));
        verify(&r, &bytes, ACTION_SCHEMA).unwrap();
        let mut changed = body.clone();
        changed["nonce"] = json!("33".repeat(32));
        assert!(verify(&r, &contracts::jcs(&changed).unwrap(), ACTION_SCHEMA).is_err());
        let mut unknown = body.clone();
        unknown["allow"] = json!(true);
        assert!(validate(&unknown).is_err());
        let mut feature = body.clone();
        feature["requires"] = json!(["unknown"]);
        assert!(validate(&feature).is_err());
    }
    #[test]
    fn approval_records_use_the_shared_pinned_format() {
        let action = reference(&contracts::jcs(&action()).unwrap(), Some(ACTION_SCHEMA));
        let request = json!({"v":REQUEST_SCHEMA,"requires":[],"request":"44".repeat(32),"action":action,
            "requester":"55".repeat(32),"approver":"66".repeat(32),"expires_at":100,"reviews":[]});
        validate(&request).unwrap();
        let mut decision = json!({"v":DECISION_SCHEMA,"requires":[],"request":reference(&contracts::jcs(&request).unwrap(),Some(REQUEST_SCHEMA)),
            "action":action,"approver":"66".repeat(32),"decision":"approve","expires_at":100,"reason":""});
        validate(&decision).unwrap();
        decision["decision"] = json!("allow_always");
        assert!(validate(&decision).is_err());
    }
}
