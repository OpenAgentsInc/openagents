//! NIP-LIVE artifacts. A valid record is evidence of what its signer claims;
//! room membership, a transcript, or a recognized voice grants nothing.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, parse_artifact,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const SCHEMA_BYTES: &[u8] = include_bytes!("../../../nips/openagents/schemas/live.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;

pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict_bounded(bytes, 1_048_576)?;
    validate(&body)?;
    Ok(body)
}

/// Validate one closed LIVE artifact, including rules the schema cannot say.
pub fn validate(body: &Value) -> Result<()> {
    if contracts::jcs(body)?.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "live body"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc =
        DOC.get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled live schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "live version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| doc["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "live schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "live requires"));
    }
    let fields = doc["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "live properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !fields.contains_key(k)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "live field"));
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

fn interval(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    if number(value, "start_ms")? > number(value, "end_ms")? {
        return Err(error(RefusalCode::Malformed, "interval order"));
    }
    Ok(())
}

fn semantic(name: &str, body: &Value) -> Result<()> {
    match name {
        "live-plan" => {
            if body["previous"].is_null() != (body["expected_revision"] == 0) {
                return Err(error(RefusalCode::Malformed, "plan predecessor"));
            }
            let mut ids = BTreeSet::new();
            for track in body["tracks"].as_array().into_iter().flatten() {
                let id = track["id"].as_str().unwrap_or_default();
                slug(id)?;
                if !ids.insert(id) {
                    return Err(error(RefusalCode::Conflict, "track ID"));
                }
                if (track["retention"]["mode"] == "none") != track["retention"]["until"].is_null() {
                    return Err(error(RefusalCode::Malformed, "track retention"));
                }
            }
            let mut names = BTreeSet::new();
            for bound in body["bounds"].as_array().into_iter().flatten() {
                contracts::parse_bound(bound)?;
                names.insert(bound["bound"].as_str().unwrap_or_default());
            }
            for required in ["wall_ms", "storage_bytes", "concurrency"] {
                if !names.contains(required) {
                    return Err(error(RefusalCode::CannotEnforce, "finite plan bounds"));
                }
            }
        }
        "live-participant" => {
            if body["action"] == "revoke"
                && (body["tracks"] != serde_json::json!([])
                    || body["rights"] != serde_json::json!([]))
            {
                return Err(error(RefusalCode::Malformed, "revoke carries no rights"));
            }
        }
        "live-capture" => {
            for bound in body["limits"].as_array().into_iter().flatten() {
                contracts::parse_bound(bound)?;
            }
        }
        "live-observation" => {
            if body["content"].is_null() && !body["evidence"].is_null() {
                return Err(error(
                    RefusalCode::Malformed,
                    "unavailable bytes cannot be cited as evidence",
                ));
            }
            interval(&body["interval"])?;
        }
        "live-transcript" => {
            if (number(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Malformed, "transcript predecessor"));
            }
            let mut ids = BTreeSet::new();
            for segment in body["segments"].as_array().into_iter().flatten() {
                let id = segment["id"].as_str().unwrap_or_default();
                slug(id)?;
                if !ids.insert(id) {
                    return Err(error(RefusalCode::Conflict, "segment ID"));
                }
                interval(&segment["interval"])?;
            }
        }
        "live-result" => {
            let status = body["status"].as_str().unwrap_or_default();
            if matches!(status, "refused" | "conflict") == body["reason"].is_null()
                && status != "unknown"
            {
                return Err(error(RefusalCode::Malformed, "result reason"));
            }
        }
        "live-input-outcome" if body["dispatched"] == false && body["outcome"] != "refused" => {
            return Err(error(RefusalCode::Malformed, "undispatched input outcome"));
        }
        "live-playback-interruption"
            if number(body, "confirmed_ms")? > number(body, "uncertain_until_ms")? =>
        {
            return Err(error(RefusalCode::Malformed, "playback positions"));
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
                    "session"
                        | "task"
                        | "owner"
                        | "controller"
                        | "request"
                        | "participant"
                        | "nonce"
                        | "capture"
                ) && let Some(text) = value.as_str()
                {
                    hex(text)?;
                }
                if key == "participants" {
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
        return Err(error(RefusalCode::IdentityMismatch, "live schema"));
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
        let mut v = json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"application/json"});
        if let Some(schema) = schema {
            v["schema"] = json!(schema);
        }
        v
    }
    #[test]
    fn revokes_carry_no_rights_and_transcripts_chain() {
        let plan = artifact(b"plan", Some("openagents.live-plan.v1"));
        let mut op = json!({"v":"openagents.live-participant.v1","requires":[],"request":"11".repeat(32),
            "plan":plan,"participant":"22".repeat(32),"action":"revoke","expected_revision":1,
            "tracks":[],"rights":[],"consent":[],"expires_at":10});
        validate(&op).unwrap();
        op["rights"] = json!(["publish"]);
        assert!(validate(&op).is_err());
        let observation = artifact(b"o", Some("openagents.live-observation.v1"));
        let transform =
            json!({"id":format!("{}:live/asr","33".repeat(32)),"artifact":artifact(b"asr",None)});
        let mut transcript = json!({"v":"openagents.live-transcript.v1","requires":[],"observation":observation,
            "transform":transform,"revision":1,"previous":null,"segments":[],"coverage":"partial"});
        assert!(validate(&transcript).is_err());
        transcript["revision"] = json!(0);
        validate(&transcript).unwrap();
        transcript["segments"] = json!([{"id":"s1","interval":{"clock":"c","start_ms":5,"end_ms":1},
            "text":artifact(b"t",None),"speaker":null,"final":true}]);
        assert!(validate(&transcript).is_err());
    }
}
