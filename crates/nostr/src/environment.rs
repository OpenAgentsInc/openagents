//! NIP-ENV artifacts. A valid record is evidence of what its signer claims;
//! it never grants environment, workspace, credential, or wallet access.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, parse_artifact,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/environment.v1.json");
/// Required CJ feature for an execute that must hold an active attachment.
pub const BINDING_FEATURE: &str = "openagents.environment-binding.v1";
pub type Result<T> = std::result::Result<T, ContractError>;

const REFUSALS: &[&str] = &[
    "malformed",
    "unsupported_version",
    "unsupported_feature",
    "not_admitted",
    "unavailable",
    "content_unavailable",
    "identity_mismatch",
    "incompatible",
    "revoked",
    "stale",
    "cannot_enforce",
    "limit_exceeded",
    "idempotency_conflict",
    "conflict",
];

pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict_bounded(bytes, 1_048_576)?;
    validate(&body)?;
    Ok(body)
}

/// Validate one closed ENV artifact, including the semantic rules the
/// schema cannot express.
pub fn validate(body: &Value) -> Result<()> {
    let bytes = contracts::jcs(body)?;
    if bytes.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "environment body"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc = DOC
        .get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled environment schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "environment version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| doc["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "environment schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(
            RefusalCode::UnsupportedFeature,
            "environment requires",
        ));
    }
    let fields = doc["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "environment properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !fields.contains_key(k)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "environment field"));
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
        "environment-definition" => {
            components(&body["components"])?;
            if !body["bootstrap"].is_null() {
                contracts::parse_effects(&body["bootstrap"]["effects"])?;
                bounds(&body["bootstrap"]["bounds"])?;
            }
        }
        "environment-request" => {
            contracts::parse_effects(&body["effects"])?;
            let names = bounds(&body["bounds"])?;
            if !names.contains("wall_ms") || !names.contains("spend_microunits") {
                return Err(error(
                    RefusalCode::CannotEnforce,
                    "request needs wall and spend ceilings",
                ));
            }
            let create = body["mode"] == "create";
            if create != body["resource"].is_null() || create != (body["cleanup"] == "destroy") {
                return Err(error(RefusalCode::Malformed, "mode/resource/cleanup"));
            }
            if number(body, "not_after")? >= number(body, "retain_until")? {
                return Err(error(RefusalCode::Malformed, "retain_until"));
            }
        }
        "environment-lease" => {
            if number(body, "created_at")? >= number(body, "not_after")?
                || number(body, "not_after")? >= number(body, "retain_until")?
            {
                return Err(error(RefusalCode::Malformed, "lease deadlines"));
            }
        }
        "environment-materialization" => {
            hex(body["generation"].as_str().unwrap_or_default())?;
            components(&body["components"])?;
            let mut principals = BTreeSet::new();
            let mut executors = 0;
            for participant in body["participants"].as_array().into_iter().flatten() {
                if !principals.insert(participant["principal"].as_str()) {
                    return Err(error(RefusalCode::Conflict, "participant principal"));
                }
                if participant["role"] == "executor" {
                    executors += 1;
                }
            }
            if executors != 1 {
                return Err(error(RefusalCode::Malformed, "exactly one executor"));
            }
        }
        "environment-state" => {
            if (number(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Conflict, "state predecessor"));
            }
            let status = body["status"].as_str().unwrap_or_default();
            if status == "released" && body["cleanup"].is_null() {
                return Err(error(RefusalCode::Malformed, "released needs cleanup"));
            }
            if matches!(status, "reserved" | "attached") && body["attachment"].is_null()
                || matches!(status, "ready" | "reserved" | "attached")
                    && body["materialization"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "state references"));
            }
        }
        "environment-control" => {
            if (body["action"] == "renew") == body["requested_until"].is_null() {
                return Err(error(RefusalCode::Malformed, "renewal deadline"));
            }
        }
        "environment-participant-admission" => {
            let evidence = ["enforcement", "root_input_digest", "projection_digest"];
            if body["status"] == "admitted" {
                if evidence.iter().any(|field| body[*field].is_null()) || !body["reason"].is_null()
                {
                    return Err(error(RefusalCode::Malformed, "admitted evidence"));
                }
                for field in ["root_input_digest", "projection_digest"] {
                    digest(&body[field])?;
                }
            } else if body["reason"].is_null() {
                return Err(error(RefusalCode::Malformed, "refusal reason"));
            }
        }
        "environment-cleanup" => {
            if body["status"] == "confirmed"
                && body["action"] == "destroy"
                && body["provider"].is_null()
            {
                return Err(error(
                    RefusalCode::Malformed,
                    "confirmed destroy needs provider confirmation",
                ));
            }
        }
        "environment-result" => {
            let status = body["status"].as_str().unwrap_or_default();
            let accepted = matches!(status, "accepted" | "duplicate");
            if accepted
                && (body["lease"].is_null() || body["state"].is_null() || !body["reason"].is_null())
                || matches!(status, "conflict" | "refused") && body["reason"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "operation result"));
            }
            if let Some(reason) = body["reason"].as_str()
                && !REFUSALS.contains(&reason)
            {
                return Err(error(RefusalCode::UnsupportedFeature, "refusal code"));
            }
        }
        "environment-enforcement" => {
            for bound in body["plan"].as_array().into_iter().flatten() {
                contracts::parse_bound(bound)?;
            }
        }
        "environment-accounting" => {
            for entry in body["entries"].as_array().into_iter().flatten() {
                if (entry["kind"] == "unknown") != entry["amount"].is_null() {
                    return Err(error(RefusalCode::Malformed, "unknown cost amount"));
                }
            }
        }
        "environment-model" if (body["identity"] == "verified") == body["weights"].is_null() => {
            return Err(error(RefusalCode::Malformed, "model identity evidence"));
        }
        _ => {}
    }
    Ok(())
}

fn components(value: &Value) -> Result<()> {
    let mut ids = BTreeSet::new();
    for component in value.as_array().into_iter().flatten() {
        let id = component["id"]
            .as_str()
            .ok_or_else(|| error(RefusalCode::Malformed, "component ID"))?;
        slug(id)?;
        if !ids.insert(id) {
            return Err(error(RefusalCode::Conflict, "component ID"));
        }
    }
    Ok(())
}

fn bounds(value: &Value) -> Result<BTreeSet<&'static str>> {
    let mut names = BTreeSet::new();
    for bound in value.as_array().into_iter().flatten() {
        contracts::parse_bound(bound)?;
        let name = BOUND_NAMES
            .iter()
            .find(|name| bound["bound"] == **name)
            .ok_or_else(|| error(RefusalCode::UnsupportedFeature, "bound"))?;
        if !names.insert(*name) {
            return Err(error(RefusalCode::Conflict, "repeated bound"));
        }
    }
    Ok(names)
}

const BOUND_NAMES: &[&str] = &[
    "wall_ms",
    "memory_bytes",
    "input_bytes",
    "output_bytes",
    "read_bytes",
    "storage_bytes",
    "calls",
    "attempts",
    "concurrency",
    "depth",
    "fuel",
    "spend_microunits",
];

/// One named ceiling from a validated bounds array.
pub fn ceiling(bounds: &Value, name: &str) -> Option<u64> {
    bounds
        .as_array()?
        .iter()
        .find(|bound| bound["bound"] == name)?["ceiling"]
        .as_u64()
}

fn nested(value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            if object.contains_key("digest") && object.contains_key("size") {
                parse_artifact(value)?;
                return Ok(());
            }
            if object.contains_key("id")
                && object.contains_key("artifact")
                && object.len() <= 3
                && !object.contains_key("role")
            {
                contracts::parse_definition(value)?;
                return Ok(());
            }
            for (key, value) in object {
                if key == "meta" {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "request"
                        | "run"
                        | "owner"
                        | "allocator"
                        | "lease"
                        | "attachment"
                        | "command"
                        | "worker"
                        | "controller"
                        | "caller"
                        | "participant"
                        | "principal"
                        | "key"
                ) && let Some(text) = value.as_str()
                {
                    hex(text)?;
                }
                if key == "recipients" {
                    for recipient in value.as_array().into_iter().flatten() {
                        hex(recipient.as_str().unwrap_or_default())?;
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
        return Err(error(RefusalCode::IdentityMismatch, "environment schema"));
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

fn digest(value: &Value) -> Result<()> {
    let text = value
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "digest"))?;
    match text.strip_prefix("sha256:") {
        Some(rest) => hex(rest),
        None => Err(error(RefusalCode::Malformed, "digest")),
    }
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

fn slug(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
        || matches!(value.as_bytes()[0], b'_' | b'-')
    {
        return Err(error(RefusalCode::Malformed, "slug"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn artifact(bytes: &[u8], schema: Option<&str>) -> Value {
        let mut value = json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
        if let Some(schema) = schema {
            value["schema"] = json!(schema);
        }
        value
    }
    fn bound(name: &str, ceiling: u64) -> Value {
        let mut value =
            json!({"bound":name,"ceiling":ceiling,"assurance":"host","mechanism":"env-host"});
        if name == "spend_microunits" {
            value["currency"] = json!("SAT");
        }
        value
    }
    fn request() -> Value {
        json!({"v":"openagents.environment-request.v1","requires":[],"request":"11".repeat(32),"run":"22".repeat(32),
            "owner":"33".repeat(32),"allocator":"44".repeat(32),
            "definition":artifact(b"d",Some("openagents.environment-definition.v1")),
            "frame":artifact(b"f",None),"policy":artifact(b"p",None),
            "reservation":artifact(b"r",Some("openagents.reservation.v1")),
            "mode":"create","resource":null,
            "effects":{"reads":[],"writes":[],"network":[],"process":true,"delegates":false,"spend":false},
            "bounds":[bound("wall_ms",60_000),bound("spend_microunits",1000)],
            "ttl_seconds":60,"idle_seconds":30,"not_after":1000,"retain_until":2000,"cleanup":"destroy"})
    }
    #[test]
    fn requests_are_closed_and_mode_matches_cleanup() {
        validate(&request()).unwrap();
        let mut body = request();
        body["cleanup"] = json!("detach");
        assert!(validate(&body).is_err());
        body = request();
        body["mode"] = json!("adopt");
        assert!(validate(&body).is_err());
        body = request();
        body["bounds"] = json!([bound("wall_ms", 10)]);
        assert_eq!(
            validate(&body).unwrap_err().code,
            RefusalCode::CannotEnforce
        );
        body = request();
        body["placement"] = json!("any");
        assert!(validate(&body).is_err());
        body = request();
        body["requires"] = json!([BINDING_FEATURE]);
        assert!(validate(&body).is_err());
        body = request();
        body["owner"] = json!("ZZ".repeat(32));
        assert!(validate(&body).is_err());
    }
    #[test]
    fn results_and_admissions_keep_refusals_explicit() {
        let result = json!({"v":"openagents.environment-result.v1","requires":[],"input":artifact(b"i",None),
            "status":"refused","lease":null,"state":null,"value":null,"reason":"stale"});
        validate(&result).unwrap();
        let mut accepted = result.clone();
        accepted["status"] = json!("accepted");
        assert!(validate(&accepted).is_err());
        let admission = json!({"v":"openagents.environment-participant-admission.v1","requires":[],
            "input":artifact(b"i",Some("openagents.environment-participant-input.v1")),
            "materialization":artifact(b"m",Some("openagents.environment-materialization.v1")),
            "participant":"55".repeat(32),"status":"admitted","root_input_digest":null,
            "projection_digest":null,"enforcement":null,"reason":null});
        assert!(validate(&admission).is_err());
    }
}
