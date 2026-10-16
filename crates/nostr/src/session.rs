//! NIP-SESS managed-session artifacts. A valid record is evidence of what its
//! signer claims; it never admits a turn, an approval, or an engine binding.
use crate::contracts::{
    self, ContractError, RefusalCode, SCHEMA_MEDIA_TYPE, SchemaClosure, check_artifact_bytes,
    parse_artifact,
};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};

pub const SCHEMA_BYTES: &[u8] = include_bytes!("../../../nips/openagents/schemas/session.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;

/// Every adapter description names each of these exactly once.
pub const FEATURES: [&str; 14] = [
    "create",
    "resume",
    "fork",
    "import",
    "close",
    "configure",
    "submit",
    "queue",
    "reorder",
    "steer",
    "interrupt",
    "history",
    "questions",
    "approvals",
];

pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict_bounded(bytes, 1_048_576)?;
    validate(&body)?;
    Ok(body)
}

/// Validate one closed SESS artifact, including rules the schema cannot say.
pub fn validate(body: &Value) -> Result<()> {
    if contracts::jcs(body)?.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "session body"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc =
        DOC.get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled session schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "session version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| doc["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "session schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "session requires"));
    }
    let fields = doc["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "session properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !fields.contains_key(k)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "session field"));
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

fn schema_ref(value: &Value) -> Result<()> {
    if value.is_null() {
        return Ok(());
    }
    contracts::parse_schema_ref(value).map(|_| ())
}

fn semantic(name: &str, body: &Value) -> Result<()> {
    match name {
        "session-adapter" => {
            schema_ref(&body["protocol"])?;
            let mut seen = BTreeSet::new();
            for feature in body["features"].as_array().into_iter().flatten() {
                let name = feature["feature"].as_str().unwrap_or_default();
                if !seen.insert(name) {
                    return Err(error(RefusalCode::Conflict, "feature named twice"));
                }
                let supported = feature["support"] != "unsupported";
                let pinned = ["operation", "semantics", "evidence"]
                    .iter()
                    .filter(|field| !feature[**field].is_null())
                    .count();
                if supported && pinned != 3 || !supported && pinned != 0 {
                    return Err(error(RefusalCode::Malformed, "feature pins"));
                }
                schema_ref(&feature["semantics"])?;
            }
            if seen.len() != FEATURES.len() {
                return Err(error(RefusalCode::Malformed, "feature matrix"));
            }
        }
        "session-configuration" => {
            schema_ref(&body["settings_schema"])?;
            for bound in body["bounds"].as_array().into_iter().flatten() {
                contracts::parse_bound(bound)?;
            }
        }
        "session-open" | "session-admission" => {
            if (body["source"]["kind"] == "new") != body["source"]["artifact"].is_null() {
                return Err(error(RefusalCode::Malformed, "open source"));
            }
            if name == "session-open" && number(body, "issued_at")? >= number(body, "expires_at")? {
                return Err(error(RefusalCode::Malformed, "open expiry"));
            }
            if name == "session-admission" && (body["generation"] != 0 || body["revision"] != 0) {
                return Err(error(RefusalCode::Malformed, "admission revision"));
            }
        }
        "session-command" => {
            if number(body, "issued_at")? >= number(body, "expires_at")? {
                return Err(error(RefusalCode::Malformed, "command expiry"));
            }
            let doc: Value = serde_json::from_slice(SCHEMA_BYTES).expect("bundled session schema");
            let action = body["action"].as_str().unwrap_or_default();
            let expected = doc["$defs"][format!("payload_{action}")]["required"].clone();
            let keys: BTreeSet<&str> = body["payload"]
                .as_object()
                .map(|o| o.keys().map(String::as_str).collect())
                .unwrap_or_default();
            let wanted: BTreeSet<&str> = expected
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            if keys != wanted {
                return Err(error(RefusalCode::Malformed, "command payload"));
            }
            if action == "promote" {
                hex(body["payload"]["input_digest"]
                    .as_str()
                    .and_then(|s| s.strip_prefix("sha256:"))
                    .unwrap_or_default())?;
            }
            if action == "replace_runtime"
                && ((body["payload"]["source"]["kind"] == "new")
                    != body["payload"]["source"]["artifact"].is_null())
            {
                return Err(error(RefusalCode::Malformed, "replacement source"));
            }
        }
        "turn-intent" => {
            for bound in body["bounds"].as_array().into_iter().flatten() {
                contracts::parse_bound(bound)?;
            }
        }
        "session-control-result" => {
            let status = body["status"].as_str().unwrap_or_default();
            if matches!(status, "accepted" | "duplicate")
                && (body["receipt"].is_null() || !body["reason"].is_null())
                || matches!(status, "conflict" | "refused")
                    && (body["reason"].is_null() || !body["receipt"].is_null())
            {
                return Err(error(RefusalCode::Malformed, "control result"));
            }
        }
        "session-runtime-binding" => {
            if number(&body["scope"], "generation")? == 0 {
                return Err(error(RefusalCode::Malformed, "replacement generation"));
            }
        }
        "session-control-receipt" => {
            let before = number(body, "before_revision")?;
            let after = number(body, "after_revision")?;
            if body["transition"] == "open" && (before, after) != (0, 0)
                || body["transition"] != "open" && before.checked_add(1) != Some(after)
            {
                return Err(error(RefusalCode::Malformed, "receipt revisions"));
            }
        }
        "session-state" => {
            if body["active_turn"].is_null() != body["turn_revision"].is_null() {
                return Err(error(RefusalCode::Malformed, "active turn"));
            }
        }
        "queued-intent" => {
            if (body["state"] == "promoted") == body["turn"].is_null() {
                return Err(error(RefusalCode::Malformed, "promoted turn"));
            }
        }
        "session-child" => {
            let managed = body["relationship"] == "managed";
            if managed && (body["child"].is_null() || body["authorization"].is_null())
                || !managed && body["native_source"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "child relationship"));
            }
        }
        "session-native" => {
            schema_ref(&body["protocol"])?;
            if body["payload"].is_null() && body["capture"] == "complete" {
                return Err(error(RefusalCode::Malformed, "native capture"));
            }
        }
        "session-history" => {
            if body["source"]["kind"] == "managed" && body["cut"].is_null() {
                return Err(error(RefusalCode::Malformed, "managed history cut"));
            }
        }
        "session-question" => schema_ref(&body["answer_schema"])?,
        "session-approval-rule"
            if number(body, "generation")? == 0
                || number(body, "starts_at")? >= number(body, "expires_at")? =>
        {
            return Err(error(
                RefusalCode::Malformed,
                "approval rule generation or expiry",
            ));
        }
        "session-read-result" => {
            if body["query"]["schema"] != "openagents.session-read.v1"
                || body["state"]["schema"] != "openagents.session-state.v1"
                || !body["artifacts"]
                    .as_array()
                    .is_some_and(|refs| refs.contains(&body["state"]))
            {
                return Err(error(RefusalCode::Malformed, "read result pins"));
            }
        }
        "session-read" if body["expires_at"].as_u64() <= body["issued_at"].as_u64() => {
            return Err(error(RefusalCode::Malformed, "read query expiry"));
        }
        "session-import-source" => schema_ref(&body["format"])?,
        "session-steer-receipt"
            if body["consumed"] == true
                && !matches!(
                    body["acknowledgment"].as_str(),
                    Some("engine_replay" | "turn_answer" | "next_turn_start")
                ) =>
        {
            return Err(error(
                RefusalCode::Malformed,
                "acceptance evidence cannot confirm consumption",
            ));
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
                    "command"
                        | "session"
                        | "owner"
                        | "authority"
                        | "lineage"
                        | "intent"
                        | "turn"
                        | "interaction"
                        | "recipient"
                        | "responder"
                        | "requester"
                        | "approver"
                        | "query"
                        | "connection"
                ) && let Some(text) = value.as_str()
                {
                    hex(text)?;
                }
                if key == "intents" {
                    for id in value.as_array().into_iter().flatten() {
                        hex(id.as_str().unwrap_or_default())?;
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
        return Err(error(RefusalCode::IdentityMismatch, "session schema"));
    }
    Ok(body)
}

/// Whether a schema reference names the shared schema media type.
pub fn is_schema(value: &Value) -> bool {
    value["media_type"] == SCHEMA_MEDIA_TYPE
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
    fn artifact(bytes: &[u8], media: &str) -> Value {
        json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":media})
    }
    fn definition(name: &str) -> Value {
        json!({"id":format!("{}:session/{name}","11".repeat(32)),"artifact":artifact(name.as_bytes(),"application/octet-stream")})
    }
    fn adapter() -> Value {
        let features: Vec<Value> = FEATURES
            .iter()
            .map(|f| {
                if *f == "fork" {
                    json!({"feature":f,"support":"unsupported","operation":null,"semantics":null,"evidence":null,"limitations":[]})
                } else {
                    json!({"feature":f,"support":"native","operation":definition(f),
                        "semantics":artifact(f.as_bytes(),SCHEMA_MEDIA_TYPE),"evidence":artifact(b"e","application/json"),"limitations":[]})
                }
            })
            .collect();
        json!({"v":"openagents.session-adapter.v1","requires":[],"adapter":definition("adapter"),
            "engine":artifact(b"engine","application/octet-stream"),"protocol":artifact(b"p",SCHEMA_MEDIA_TYPE),
            "features":features,"compatibility":artifact(b"c","application/json")})
    }
    #[test]
    fn feature_matrices_are_exact_and_unsupported_pins_nothing() {
        validate(&adapter()).unwrap();
        let mut body = adapter();
        body["features"][2]["operation"] = definition("fork");
        assert!(validate(&body).is_err());
        let mut body = adapter();
        body["features"][0]["support"] = json!("emulated");
        body["features"][0]["evidence"] = Value::Null;
        assert!(validate(&body).is_err());
        let mut body = adapter();
        body["features"][1] = body["features"][0].clone();
        assert!(validate(&body).is_err());
        let mut body = adapter();
        body["protocol"]["media_type"] = json!("application/json");
        assert!(validate(&body).is_err());
    }
    #[test]
    fn commands_carry_exactly_their_action_payload() {
        let scope = json!({"authority":"22".repeat(32),"session":"33".repeat(32),"generation":0});
        let mut command = json!({"v":"openagents.session-command.v1","requires":[],"command":"44".repeat(32),
            "scope":scope,"expected_revision":1,"authorization":artifact(b"a","application/json"),
            "issued_at":10,"expires_at":20,"action":"cancel_queued","payload":{"intent":"55".repeat(32)}});
        validate(&command).unwrap();
        command["action"] = json!("archive");
        assert!(validate(&command).is_err());
        command["action"] = json!("reorder");
        command["payload"] = json!({"intents":["55".repeat(32),"55".repeat(32)]});
        assert!(validate(&command).is_err());
        let receipt = json!({"v":"openagents.session-steer-receipt.v1","requires":[],"scope":scope,
            "turn":"66".repeat(32),"correction":{"digest":contracts::digest_bytes(b"c"),"size":1,
            "media_type":"application/json","schema":"openagents.session-steer-correction.v1"},
            "mode":"native","acknowledgment":"input_written","consumed":true,"evidence":[]});
        assert!(validate(&receipt).is_err());
    }
    #[test]
    fn quiescence_keeps_native_evidence_and_child_state_dispositions_distinct() {
        let reference = |schema: &str| {
            let mut r = artifact(schema.as_bytes(), "application/json");
            r["schema"] = json!(schema);
            r
        };
        let mut body = json!({"v":"openagents.session-quiescence.v1","requires":[],
            "scope":{"authority":"11".repeat(32),"session":"22".repeat(32),"generation":0},
            "turn":"33".repeat(32),"attachment":null,"evidence":[reference("openagents.session-native.v1")],
            "children":[reference("openagents.session-state.v1")],"observed_at":10});
        validate(&body).unwrap();
        body["children"][0] = reference("openagents.session-native.v1");
        assert!(validate(&body).is_err());
        body["children"][0] = reference("openagents.session-state.v1");
        body["evidence"] = json!([]);
        assert!(validate(&body).is_err());
    }
    #[test]
    fn private_reads_have_closed_queries_and_current_expiry() {
        let mut query = json!({"v":"openagents.session-read.v1","requires":[],
            "scope":{"authority":"11".repeat(32),"session":"22".repeat(32),"generation":0},
            "query":"33".repeat(32),"issued_at":10,"expires_at":20,"artifacts":[]});
        validate(&query).unwrap();
        query["query"] = json!("z".repeat(64));
        assert!(validate(&query).is_err());
        query["query"] = json!("33".repeat(32));
        query["expires_at"] = json!(10);
        assert!(validate(&query).is_err());
        query["expires_at"] = json!(20);
        query["recursive"] = json!(true);
        assert!(validate(&query).is_err());
    }
    #[test]
    fn promotion_selectors_require_the_exact_closed_head_identity() {
        let mut command = json!({"v":"openagents.session-command.v1","requires":[],"command":"44".repeat(32),
            "scope":{"authority":"22".repeat(32),"session":"33".repeat(32),"generation":0},
            "expected_revision":1,"authorization":artifact(b"a","application/json"),
            "issued_at":10,"expires_at":20,"action":"promote",
            "payload":{"intent":"55".repeat(32),"input_digest":contracts::digest_bytes(b"intent"),"requester":"66".repeat(32)}});
        validate(&command).unwrap();
        for field in ["intent", "input_digest", "requester"] {
            let mut missing = command.clone();
            missing["payload"].as_object_mut().unwrap().remove(field);
            assert!(validate(&missing).is_err());
            let mut malformed = command.clone();
            malformed["payload"][field] =
                json!("z".repeat(if field == "input_digest" { 71 } else { 64 }));
            assert!(validate(&malformed).is_err());
        }
        command["payload"]["skip"] = json!(true);
        assert!(validate(&command).is_err());
    }
    #[test]
    fn runtime_replacement_checks_sources_and_new_generation_evidence() {
        let reference = |schema: &str| {
            let mut r = artifact(schema.as_bytes(), "application/json");
            r["schema"] = json!(schema);
            r
        };
        let payload = json!({"adapter":reference("openagents.session-adapter.v1"),
            "lease":reference("openagents.environment-lease.v1"),
            "materialization":reference("openagents.environment-materialization.v1"),
            "requested":reference("openagents.session-configuration.v1"),
            "source":{"kind":"new","artifact":null},"workspace":null});
        let scope = json!({"authority":"11".repeat(32),"session":"22".repeat(32),"generation":0});
        let mut command = json!({"v":"openagents.session-command.v1","requires":[],"command":"33".repeat(32),
            "scope":scope,"expected_revision":0,"authorization":artifact(b"auth","application/json"),
            "issued_at":10,"expires_at":20,"action":"replace_runtime","payload":payload});
        validate(&command).unwrap();
        command["payload"]["source"]["kind"] = json!("resume");
        assert!(validate(&command).is_err());
        command["payload"]["source"]["artifact"] = reference("openagents.session-history.v1");
        validate(&command).unwrap();
        command["payload"]["source"]["kind"] = json!("fork");
        assert!(validate(&command).is_err());
        let mut record = json!({"v":"openagents.session-runtime-binding.v1","requires":[],"scope":scope,
            "previous":reference("openagents.session-admission.v1"),"request":reference("openagents.session-command.v1"),
            "attachment":reference("openagents.environment-attachment.v1"),"lease":payload["lease"],
            "materialization":payload["materialization"],"adapter":payload["adapter"],"requested":payload["requested"],
            "effective":payload["requested"],"admitted_at":10});
        assert!(validate(&record).is_err());
        record["scope"]["generation"] = json!(1);
        validate(&record).unwrap();
    }
}
