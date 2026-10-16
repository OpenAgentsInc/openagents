//! Closed NIP-AUTO records; authentication and scheduling remain host duties.
use crate::contracts::{self, ContractError, RefusalCode, SchemaClosure};
use serde_json::Value;
use std::{collections::BTreeMap, sync::OnceLock};
pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/automation.v1.json");
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
        return Err(error(RefusalCode::LimitExceeded, "automation bytes"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let schema = DOC
        .get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled automation schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "automation version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| schema["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "automation version"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(
            RefusalCode::UnsupportedFeature,
            "automation requires",
        ));
    }
    let fields = schema["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "automation fields"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|key| !fields.contains_key(key)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "automation field"));
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
    for field in [
        "automation",
        "run",
        "task",
        "poll",
        "request",
        "occurrence",
        "command",
    ] {
        if body[field].is_string() {
            crate::coordination::id(&body[field])?;
        }
    }
    for field in ["owner", "controller"] {
        if let Some(key) = body[field].as_str() {
            key.parse::<secp256k1::XOnlyPublicKey>()
                .map_err(|_| error(RefusalCode::Malformed, "automation principal"))?;
        }
    }
    for field in ["per_occurrence", "aggregate", "bounds"] {
        if let Some(bounds) = body.get(field) {
            crate::coordination::bounds_valid(bounds)?;
        }
    }
    match name {
        "automation-plan" => {
            contracts::parse_effects(&body["effects"])?;
            let starts = n(body, "starts_at")?;
            let expires = n(body, "expires_at")?;
            if !(starts < expires && expires < n(body, "retain_until")?) {
                return Err(error(RefusalCode::Conflict, "finite automation lifetime"));
            }
            let trigger = &body["trigger"];
            match trigger["kind"].as_str() {
                Some("once") => {
                    let at = n(trigger, "at")?;
                    if at < starts || at >= expires {
                        return Err(error(RefusalCode::Conflict, "once slot"));
                    }
                }
                Some("interval" | "source") => {
                    let first = n(trigger, "first_at")?;
                    if first < starts || first >= expires {
                        return Err(error(RefusalCode::Conflict, "first slot"));
                    }
                    due_at(
                        trigger,
                        (expires - 1 - first) / n(trigger, "every_seconds")?,
                    )?;
                }
                Some("continuation") => {}
                _ => return Err(error(RefusalCode::UnsupportedFeature, "automation trigger")),
            }
            if trigger["kind"] == "continuation" && body["completion"].is_null() {
                return Err(error(
                    RefusalCode::Conflict,
                    "independent continuation checker",
                ));
            }
            if !body["completion"].is_null() {
                crate::coordination::bounds_valid(&body["completion"]["bounds"])?;
            }
            if trigger["kind"] == "source" {
                crate::coordination::bounds_valid(&trigger["bounds"])?;
            }
        }
        "automation-skipped" => {
            if n(body, "first_slot")? > n(body, "last_slot")?
                || n(body, "first_due_at")? > n(body, "last_due_at")?
            {
                return Err(error(RefusalCode::Conflict, "skipped range"));
            }
        }
        "automation-observation" => {
            let available = body["available"] == true;
            if available
                && (body["revision"].is_null()
                    || body["evidence"].is_null()
                    || !body["reason"].is_null())
                || !available
                    && (!body["revision"].is_null()
                        || !body["evidence"].is_null()
                        || body["reason"].is_null())
            {
                return Err(error(RefusalCode::Conflict, "source availability"));
            }
        }
        "automation-poll" => {
            if body["phase"] == "observe"
                && (body["slot"].is_null() || !body["occurrence"].is_null())
                || body["phase"] == "revalidate"
                    && (!body["slot"].is_null() || body["occurrence"].is_null())
            {
                return Err(error(RefusalCode::Conflict, "poll phase"));
            }
        }
        "automation-occurrence" => {
            if n(body, "due_at")? >= n(body, "deadline")? {
                return Err(error(RefusalCode::Conflict, "occurrence deadline"));
            }
            if (body["key"]["kind"] == "source") != !body["observation"].is_null() {
                return Err(error(
                    RefusalCode::Conflict,
                    "occurrence source observation",
                ));
            }
        }
        "automation-state" => {
            if (n(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Conflict, "state predecessor"));
            }
        }
        "automation-result" => {
            if matches!(body["status"].as_str(), Some("accepted" | "duplicate"))
                && (body["state"].is_null() || !body["reason"].is_null())
                || matches!(body["status"].as_str(), Some("conflict" | "refused"))
                    && body["reason"].is_null()
            {
                return Err(error(RefusalCode::Conflict, "automation result authority"));
            }
        }
        _ => {}
    }
    Ok(())
}
fn n(body: &Value, field: &str) -> Result<u64> {
    body[field]
        .as_u64()
        .ok_or_else(|| error(RefusalCode::Malformed, "automation integer"))
}
pub fn due_at(trigger: &Value, slot: u64) -> Result<u64> {
    n(trigger, "every_seconds")?
        .checked_mul(slot)
        .and_then(|v| v.checked_add(trigger["first_at"].as_u64()?))
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| error(RefusalCode::LimitExceeded, "UTC slot arithmetic"))
}
fn references(body: &Value) -> Result<()> {
    let mut stack = vec![body];
    let mut count = 0;
    while let Some(value) = stack.pop() {
        count += 1;
        if count > 8192 {
            return Err(error(RefusalCode::LimitExceeded, "automation structure"));
        }
        if value.get("digest").is_some() {
            contracts::parse_artifact(value)?;
            continue;
        }
        if value.get("id").is_some() && value.get("artifact").is_some() {
            contracts::parse_definition(value)?;
            continue;
        }
        match value {
            Value::Object(fields) => stack.extend(
                fields
                    .iter()
                    .filter(|(k, _)| k.as_str() != "meta")
                    .map(|(_, v)| v),
            ),
            Value::Array(items) => stack.extend(items),
            _ => {}
        }
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn artifact() -> Value {
        json!({"digest":contracts::digest_bytes(b"test"),"size":4,"media_type":"text/plain"})
    }
    #[test]
    fn unavailable_sources_cannot_invent_revisions() {
        let a = artifact();
        let mut body = json!({"v":"openagents.automation-observation.v1","requires":[],"source":{"id":format!("{}:auto/source","11".repeat(32)),"artifact":a},"resource":"doc","revision":null,"observed_at":100,"available":false,"evidence":null,"reason":"unavailable"});
        validate(&body).unwrap();
        body["revision"] = json!("made-up");
        assert!(validate(&body).is_err());
    }
    #[test]
    fn typed_poll_phases_and_state_links_refuse_ambiguity() {
        let a = artifact();
        let mut body = json!({"v":"openagents.automation-poll.v1","requires":[],"plan":a,"poll":"11".repeat(32),"phase":"observe","slot":0,"occurrence":null,"request":"22".repeat(32),"run":"33".repeat(32),"deadline":120});
        validate(&body).unwrap();
        body["occurrence"] = a;
        assert!(validate(&body).is_err());
    }
    #[test]
    fn utc_slots_check_all_arithmetic() {
        let trigger =
            json!({"kind":"interval","first_at":100,"every_seconds":60,"missed":"latest"});
        assert_eq!(due_at(&trigger, 2).unwrap(), 220);
        assert!(due_at(&trigger, u64::MAX).is_err());
    }
}
