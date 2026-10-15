//! NIP-WORK artifacts. A valid record still grants no mutation or execution.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, parse_artifact,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::OnceLock,
};
pub const SCHEMA_BYTES: &[u8] = include_bytes!("../../../nips/openagents/schemas/work.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkRef {
    pub authority: String,
    pub scope: String,
    pub item: String,
}
impl WorkRef {
    pub fn check(&self) -> Result<()> {
        for value in [&self.authority, &self.scope, &self.item] {
            hex(value)?;
        }
        self.authority
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| error(RefusalCode::Malformed, "work authority pubkey"))?;
        Ok(())
    }
    pub fn key(&self) -> String {
        format!("{}:{}:{}", self.authority, self.scope, self.item)
    }
}
pub fn work_ref(value: &Value) -> Result<WorkRef> {
    let value: WorkRef = serde_json::from_value(value.clone())
        .map_err(|_| error(RefusalCode::Malformed, "work reference"))?;
    value.check()?;
    Ok(value)
}
pub fn revision_ref(value: &Value) -> Result<WorkRef> {
    let object = value
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "work revision reference"))?;
    if object.len() != 3
        || !object.contains_key("work")
        || !object.contains_key("revision")
        || !object.contains_key("artifact")
        || number(value, "revision")? == 0
    {
        return Err(error(RefusalCode::Malformed, "revision reference fields"));
    }
    let artifact = parse_artifact(&value["artifact"])?;
    if artifact.schema.as_deref() != Some("openagents.work-revision.v1")
        || artifact.media_type != "application/json"
    {
        return Err(error(
            RefusalCode::IdentityMismatch,
            "revision reference schema",
        ));
    }
    work_ref(&value["work"])
}
pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict(bytes)?;
    validate(&body)?;
    Ok(body)
}
pub fn validate(body: &Value) -> Result<()> {
    let bytes = contracts::jcs(body)?;
    if bytes.len() > 1_048_576 {
        return Err(error(RefusalCode::LimitExceeded, "work body"));
    }
    static DOC: OnceLock<Value> = OnceLock::new();
    let doc =
        DOC.get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled work schema"));
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "work version"))?;
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| doc["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "work schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "work requires"));
    }
    let fields = doc["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(RefusalCode::Malformed, "work properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|k| !fields.contains_key(k)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "work field"));
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
    match name {
        "work-scope" => {
            hex(body["scope"]
                .as_str()
                .ok_or_else(|| error(RefusalCode::Malformed, "scope ID"))?)?;
            if body["issuer"] != body["owner"] {
                return Err(error(RefusalCode::IdentityMismatch, "scope owner"));
            }
        }
        "work-item" => {
            bounded_text(&body["title"], 512, true)?;
            if (body["kind"] == "work") == (body["frame"].is_null() || body["acceptance"].is_null())
            {
                return Err(error(RefusalCode::Malformed, "work frame/acceptance"));
            }
            if body["kind"] != "work" && (!body["frame"].is_null() || !body["acceptance"].is_null())
            {
                return Err(error(RefusalCode::Malformed, "planning frame/acceptance"));
            }
            let timebox = &body["timebox"];
            if body["kind"] == "cycle" {
                if timebox.is_null() || number(timebox, "starts_at")? >= number(timebox, "ends_at")?
                {
                    return Err(error(RefusalCode::Malformed, "cycle timebox"));
                }
            } else if !timebox.is_null() {
                return Err(error(RefusalCode::Malformed, "non-cycle timebox"));
            }
            for label in body["labels"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "labels"))?
            {
                slug(
                    label
                        .as_str()
                        .ok_or_else(|| error(RefusalCode::Malformed, "label"))?,
                )?;
            }
            if body["relations"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "relations"))?
                .iter()
                .filter(|r| r["type"] == "parent")
                .count()
                > 1
            {
                return Err(error(RefusalCode::Conflict, "multiple parents"));
            }
            if matches!(
                body["state"].as_str(),
                Some("resolved" | "cancelled" | "superseded" | "archived")
            ) && body["disposition"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "terminal disposition"));
            }
        }
        "work-command" => {
            let work = work_ref(&body["work"])?;
            if (body["action"] == "create") != body["expected"].is_null() {
                return Err(error(RefusalCode::Malformed, "create/replace precondition"));
            }
            if !body["expected"].is_null() && revision_ref(&body["expected"])? != work {
                return Err(error(RefusalCode::IdentityMismatch, "expected work"));
            }
        }
        "work-revision" => {
            let work = work_ref(&body["work"])?;
            let revision = number(body, "revision")?;
            if body["issuer"] != work.authority || ((revision == 1) != body["previous"].is_null()) {
                return Err(error(
                    RefusalCode::IdentityMismatch,
                    "revision authority/previous",
                ));
            }
            if !body["previous"].is_null()
                && (revision_ref(&body["previous"])? != work
                    || number(&body["previous"], "revision")?.checked_add(1) != Some(revision))
            {
                return Err(error(RefusalCode::Conflict, "revision predecessor"));
            }
        }
        "work-operation-result" => {
            if let Some(reason) = body["reason"].as_str() {
                if ![
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
                ]
                .contains(&reason)
                {
                    return Err(error(RefusalCode::UnsupportedFeature, "work refusal code"));
                }
            }
            let accepted = matches!(body["status"].as_str(), Some("accepted" | "duplicate"));
            if accepted && (body["revision"].is_null() || !body["reason"].is_null())
                || matches!(body["status"].as_str(), Some("conflict" | "refused"))
                    && body["reason"].is_null()
                || body["status"] == "unknown" && !body["revision"].is_null()
            {
                return Err(error(RefusalCode::Malformed, "operation result"));
            }
        }
        "work-delegation" => {
            if revision_ref(&body["basis"])? != work_ref(&body["work"])? {
                return Err(error(RefusalCode::IdentityMismatch, "delegation basis"));
            }
            if !body["order"].is_null() {
                crate::market_contracts::parse_order_ref(&body["order"])?;
            }
            for execution in body["executions"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "executions"))?
            {
                if ["proposal", "request", "run", "session"]
                    .iter()
                    .all(|field| execution[*field].is_null())
                {
                    return Err(error(RefusalCode::Malformed, "empty execution link"));
                }
            }
        }
        "work-evidence" | "work-disposition" | "work-attention" => {
            if revision_ref(&body["basis"])? != work_ref(&body["work"])? {
                return Err(error(RefusalCode::IdentityMismatch, "record basis"));
            }
            if name == "work-disposition" {
                if (body["outcome"] == "superseded") == body["successor"].is_null() {
                    return Err(error(RefusalCode::Malformed, "disposition successor"));
                }
                if body["outcome"] == "accepted"
                    && body["evidence"].as_array().is_none_or(Vec::is_empty)
                {
                    return Err(error(RefusalCode::Malformed, "acceptance evidence"));
                }
                if parse_artifact(&body["reason"])?.size == 0 {
                    return Err(error(RefusalCode::Malformed, "empty disposition reason"));
                }
            }
        }
        "work-source" => {
            bounded_text(&body["system"], 512, true)?;
            bounded_text(&body["subject"], 512, true)?;
        }
        "work-view-row" => {
            if revision_ref(&body["revision"])? != work_ref(&body["work"])? {
                return Err(error(RefusalCode::IdentityMismatch, "view row work"));
            }
        }
        "task-frame" => {
            if (number(body, "revision")? == 0) != body["previous"].is_null() {
                return Err(error(RefusalCode::Conflict, "frame predecessor"));
            }
            let mut names = BTreeSet::new();
            for variable in body["variables"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "variables"))?
            {
                let name = variable["name"]
                    .as_str()
                    .ok_or_else(|| error(RefusalCode::Malformed, "variable name"))?;
                slug(name)?;
                if !names.insert(name) {
                    return Err(error(RefusalCode::Conflict, "variable name"));
                }
            }
        }
        "snapshot" => {
            let mut ids = BTreeSet::new();
            for source in body["sources"]
                .as_array()
                .ok_or_else(|| error(RefusalCode::Malformed, "snapshot sources"))?
            {
                let id = source["id"]
                    .as_str()
                    .ok_or_else(|| error(RefusalCode::Malformed, "source ID"))?;
                if id.is_empty() || !ids.insert(id) {
                    return Err(error(RefusalCode::Conflict, "source identity"));
                }
                if source["kind"] == "external" {
                    if source.get("adapter").is_none()
                        || source["version"]["schema"] != "openagents.observation.v1"
                    {
                        return Err(error(
                            RefusalCode::IdentityMismatch,
                            "external observation adapter/version",
                        ));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}
fn nested(value: &Value) -> Result<()> {
    match value {
        Value::Object(object) => {
            if object.contains_key("digest") {
                parse_artifact(value)?;
                return Ok(());
            }
            if object.contains_key("id") && object.contains_key("artifact") {
                contracts::parse_definition(value)?;
                return Ok(());
            }
            if object.len() == 3
                && object.contains_key("authority")
                && object.contains_key("scope")
                && object.contains_key("item")
            {
                work_ref(value)?;
                return Ok(());
            }
            for (key, value) in object {
                if key == "meta" {
                    continue;
                }
                if matches!(
                    key.as_str(),
                    "issuer"
                        | "owner"
                        | "authority"
                        | "delegate"
                        | "controller"
                        | "accountable"
                        | "recipient"
                        | "command_id"
                        | "task"
                ) && let Some(value) = value.as_str()
                {
                    hex(value)?;
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
        Value::String(text) if text.len() > 1024 => {
            return Err(error(RefusalCode::LimitExceeded, "work string"));
        }
        _ => {}
    }
    Ok(())
}
pub fn verify_source(source: &Value, observation: &Value) -> Result<()> {
    validate(source)?;
    let parsed = contracts::parse_observation(observation)?;
    check_artifact_bytes(
        &parse_artifact(&source["assurance"])?,
        &contracts::jcs(observation)?,
    )?;
    if source["system"] != parsed.resource_scope
        || source["subject"] != parsed.resource_id
        || source["adapter"] != observation["adapter"]
        || number(source, "observed_at")? != parsed.captured_at
        || source["version"] != observation["provider_revision"]
        || source["content"] != observation["content"]
    {
        return Err(error(
            RefusalCode::IdentityMismatch,
            "work source assurance",
        ));
    }
    Ok(())
}
pub fn verify_revision(reference: &Value, revision: &Value) -> Result<()> {
    let work = revision_ref(reference)?;
    validate(revision)?;
    check_artifact_bytes(
        &parse_artifact(&reference["artifact"])?,
        &contracts::jcs(revision)?,
    )?;
    if revision["v"] != "openagents.work-revision.v1"
        || work_ref(&revision["work"])? != work
        || reference["revision"] != revision["revision"]
    {
        return Err(error(RefusalCode::IdentityMismatch, "exact work revision"));
    }
    Ok(())
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
fn bounded_text(value: &Value, max: usize, nonempty: bool) -> Result<()> {
    let text = value
        .as_str()
        .ok_or_else(|| error(RefusalCode::Malformed, "text"))?;
    if text.len() > max || nonempty && text.is_empty() {
        return Err(error(RefusalCode::LimitExceeded, "text bound"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn artifact(bytes: &[u8]) -> Value {
        json!({"digest":contracts::digest_bytes(bytes),"size":bytes.len(),"media_type":"text/plain"})
    }
    fn scope() -> Value {
        json!({"v":"openagents.work-scope.v1","requires":[],"issuer":"11".repeat(32),"owner":"11".repeat(32),"authority":"22".repeat(32),"scope":"33".repeat(32),"policy":artifact(b"policy"),"resources":null,"created_at":100})
    }
    #[test]
    fn closed_scopes_require_hex_identity_and_supported_features() {
        let mut body = scope();
        validate(&body).unwrap();
        body["scope"] = json!("z".repeat(64));
        assert!(validate(&body).is_err());
        body = scope();
        body["execute"] = json!(true);
        assert!(validate(&body).is_err());
        body = scope();
        body["requires"] = json!(["unknown-feature"]);
        assert!(validate(&body).is_err());
    }
    #[test]
    fn source_assurance_cannot_strengthen_or_relabel_an_observation() {
        let definition =
            json!({"id":format!("{}:work/source","11".repeat(32)),"artifact":artifact(b"adapter")});
        let observation = json!({"v":"openagents.observation.v1","requires":[],"resource":{"scope":"github:example/repo","id":"I_node"},"adapter":definition,"captured_at":100,"provider_revision":"version","content":artifact(b"excerpt"),"consistency":"observational","valid_until":null});
        let bytes = contracts::jcs(&observation).unwrap();
        let mut reference = artifact(&bytes);
        reference["media_type"] = json!("application/json");
        reference["schema"] = json!("openagents.observation.v1");
        let mut source = json!({"v":"openagents.work-source.v1","requires":[],"issuer":"11".repeat(32),"system":"github:example/repo","subject":"I_node","adapter":definition,"observed_at":100,"version":"version","content":artifact(b"excerpt"),"assurance":reference});
        verify_source(&source, &observation).unwrap();
        source["subject"] = json!("I_different");
        assert!(verify_source(&source, &observation).is_err());
    }
    #[test]
    fn malformed_duplicate_json_and_invalid_revision_references_refuse() {
        assert!(
            parse(br#"{"v":"openagents.work-scope.v1","v":"openagents.work-item.v1"}"#).is_err()
        );
        let work =
            json!({"authority":"11".repeat(32),"scope":"22".repeat(32),"item":"33".repeat(32)});
        let mut reference = json!({"work":work,"revision":0,"artifact":artifact(b"revision")});
        assert!(revision_ref(&reference).is_err());
        reference["revision"] = json!(1);
        assert!(revision_ref(&reference).is_err());
    }
}
