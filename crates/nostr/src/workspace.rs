//! NIP-WS wire validation. Attribution and actual source authority stay with hosts.
use crate::contracts::{
    self, ContractError, RefusalCode, SchemaClosure, check_artifact_bytes, digest_bytes,
    parse_artifact,
};
use RefusalCode::{Conflict, IdentityMismatch, LimitExceeded, Malformed};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;
pub const SCHEMA_BYTES: &[u8] =
    include_bytes!("../../../nips/openagents/schemas/workspace.v1.json");
pub type Result<T> = std::result::Result<T, ContractError>;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceRef {
    pub authority: String,
    pub workspace: String,
    pub resource: String,
}
impl ResourceRef {
    pub fn validate(&self) -> Result<()> {
        for value in [&self.authority, &self.workspace, &self.resource] {
            hex(value)?;
        }
        Ok(())
    }
    pub fn same_namespace(&self, other: &Self) -> bool {
        self.authority == other.authority && self.workspace == other.workspace
    }
}
pub fn parse(bytes: &[u8]) -> Result<Value> {
    let body = contracts::parse_strict(bytes)?;
    validate(&body)?;
    Ok(body)
}
pub fn validate(body: &Value) -> Result<()> {
    let kind = body["v"]
        .as_str()
        .ok_or_else(|| error(Malformed, "workspace version"))?;
    static DOCUMENT: OnceLock<Value> = OnceLock::new();
    let document = DOCUMENT
        .get_or_init(|| serde_json::from_slice(SCHEMA_BYTES).expect("bundled workspace schema"));
    let name = kind
        .strip_prefix("openagents.")
        .and_then(|s| s.strip_suffix(".v1"))
        .filter(|name| document["$defs"][*name]["properties"]["v"]["const"] == kind)
        .ok_or_else(|| error(RefusalCode::UnsupportedVersion, "workspace schema"))?;
    if body["requires"] != serde_json::json!([]) {
        return Err(error(RefusalCode::UnsupportedFeature, "workspace requires"));
    }
    let properties = document["$defs"][name]["properties"]
        .as_object()
        .ok_or_else(|| error(Malformed, "workspace schema properties"))?;
    if body
        .as_object()
        .is_some_and(|o| o.keys().any(|key| !properties.contains_key(key)))
    {
        return Err(error(RefusalCode::UnsupportedFeature, "workspace field"));
    }
    let bytes = contracts::jcs(body)?;
    if bytes.len() > 1_048_576 {
        return Err(error(LimitExceeded, "workspace body"));
    }
    static CLOSURE: OnceLock<Result<SchemaClosure>> = OnceLock::new();
    let closure = CLOSURE
        .get_or_init(|| {
            contracts::prepare_closure(&BTreeMap::from([(
                digest_bytes(SCHEMA_BYTES),
                SCHEMA_BYTES.to_vec(),
            )]))
        })
        .as_ref()
        .map_err(Clone::clone)?;
    contracts::validate_instance(closure, &digest_bytes(SCHEMA_BYTES), body)?;
    refs_and_bounds(body)?;
    let kind = body["v"].as_str().ok_or_else(|| error(Malformed, "v"))?;
    match kind {
        "openagents.workspace-binding.v1" => {
            let mut roots = BTreeSet::new();
            for r in body["roots"]
                .as_array()
                .ok_or_else(|| error(Malformed, "roots"))?
            {
                let r = resource(r)?;
                if r.workspace != body["workspace"]
                    || r.authority != body["authority"]
                    || !roots.insert(r.resource)
                {
                    return Err(error(IdentityMismatch, "workspace roots"));
                }
            }
            if body["revision"] == 0 && body["generation"] != 0 {
                return Err(error(Malformed, "initial binding generation"));
            }
        }
        "openagents.workspace-resource.v1" => {
            let r = resource(&body["ref"])?;
            if !body["parent"].is_null() {
                let parent = resource(&body["parent"])?;
                if !r.same_namespace(&parent) || r == parent {
                    return Err(error(IdentityMismatch, "resource parent"));
                }
            }
        }
        "openagents.document-snapshot.v1" => {
            if body["editable"] == true
                && (body["coverage"] != "complete"
                    || body["content"].is_null()
                    || !body["derivation"].is_null())
            {
                return Err(error(
                    Malformed,
                    "editable snapshot requires complete exact content",
                ));
            }
        }
        "openagents.resource-change.v1" | "openagents.workspace-restore.v1" => {
            if integer(body, "expires_at")? <= integer(body, "issued_at")? {
                return Err(error(Malformed, "action expiry"));
            }
        }
        "openagents.workspace-action-admission.v1" => {
            let schema = body["proposal"]["schema"].as_str().unwrap_or("");
            if !matches!(
                schema,
                "openagents.resource-change.v1" | "openagents.workspace-restore.v1"
            ) {
                return Err(error(Malformed, "admission proposal"));
            }
        }
        "openagents.resource-change-result.v1" => {
            if body["outcome"] == "completed"
                && (body["dispatched"] != true
                    || body["after"].is_null()
                    || !body["reason"].is_null())
            {
                return Err(error(Malformed, "completed change result"));
            }
            reason(&body["reason"])?;
        }
        "openagents.worktree-binding.v1" => {
            let r = resource(&body["resource"])?;
            let repository = resource(&body["repository"])?;
            if !r.same_namespace(&repository) || r == repository {
                return Err(error(IdentityMismatch, "worktree repository"));
            }
            if matches!(
                body["lifecycle"].as_str(),
                Some("ready" | "cleanup_requested" | "removing" | "removed")
            ) && body["creation"].is_null()
            {
                return Err(error(Malformed, "worktree creation evidence"));
            }
            if (body["lifecycle"] == "removed") == body["removal"].is_null() {
                return Err(error(Malformed, "worktree removal evidence"));
            }
        }
        "openagents.workspace-checkpoint.v1" => {
            if integer(body, "retain_until")? <= integer(body, "created_at")? {
                return Err(error(Malformed, "checkpoint retention"));
            }
        }
        "openagents.projection-definition.v1" => {
            if body["retention"]["max_items"] == 0 || body["retention"]["max_bytes"] == 0 {
                return Err(error(Malformed, "projection retention"));
            }
        }
        "openagents.projection-cut.v1" => {
            unique(
                body["frontiers"]
                    .as_array()
                    .ok_or_else(|| error(Malformed, "frontiers"))?
                    .iter()
                    .map(|f| f["source"]["digest"].as_str().unwrap_or("")),
            )?;
        }
        "openagents.projection-read.v1" => {
            if body["mode"] == "changes" && body["after"].is_null() {
                return Err(error(
                    Malformed,
                    "changes require a completed snapshot cursor",
                ));
            }
        }
        "openagents.projection-page.v1" => {
            if !body["next"].is_null() && !body["resume"].is_null()
                || body["next"].is_null() && body["resume"].is_null()
            {
                return Err(error(Malformed, "snapshot page continuation"));
            }
            unique(
                body["rows"]
                    .as_array()
                    .ok_or_else(|| error(Malformed, "rows"))?
                    .iter()
                    .map(|r| r["key"].as_str().unwrap_or("")),
            )?;
        }
        "openagents.projection-delta.v1" => {
            let changes = body["changes"]
                .as_array()
                .ok_or_else(|| error(Malformed, "changes"))?;
            unique(changes.iter().map(|c| c["key"].as_str().unwrap_or("")))?;
            for change in changes {
                if (change["action"] == "remove") != change["value"].is_null() {
                    return Err(error(Malformed, "delta action/value"));
                }
            }
        }
        "openagents.projection-change-page.v1" => {
            if body["next"].is_null() {
                if body["resume"].is_null() || body["marker"].is_null() {
                    return Err(error(Malformed, "completed catch-up continuation"));
                }
            } else if !body["resume"].is_null() || !body["marker"].is_null() {
                return Err(error(Malformed, "partial catch-up continuation"));
            }
        }
        "openagents.projection-result.v1" => {
            reason(&body["reason"])?;
            if body["status"] == "completed" {
                if body["page"].is_null() || !body["marker"].is_null() || !body["reason"].is_null()
                {
                    return Err(error(Malformed, "completed projection read"));
                }
            } else if !body["page"].is_null() || body["reason"].is_null() {
                return Err(error(Malformed, "refused projection read"));
            }
        }
        "openagents.projection-command-visibility.v1" => {
            if (body["state"] == "visible") == body["cut"].is_null() {
                return Err(error(Malformed, "command visibility cut"));
            }
        }
        _ => {}
    }
    Ok(())
}
fn refs_and_bounds(value: &Value) -> Result<()> {
    match value {
        Value::String(s) if s.len() > 1024 => return Err(error(LimitExceeded, "workspace string")),
        Value::Object(object) => {
            if object.contains_key("digest") {
                parse_artifact(value)?;
                return Ok(());
            }
            if object.contains_key("artifact") && object.contains_key("id") {
                contracts::parse_definition(value)?;
                return Ok(());
            }
            for (key, value) in object {
                if matches!(
                    key.as_str(),
                    "authority"
                        | "owner"
                        | "actor"
                        | "recipient"
                        | "workspace"
                        | "resource"
                        | "epoch"
                        | "projection"
                        | "command"
                        | "request"
                        | "checkpoint"
                ) && let Some(value) = value.as_str()
                {
                    hex(value)?;
                }
                if key != "meta" {
                    refs_and_bounds(value)?;
                }
            }
        }
        Value::Array(values) => {
            for value in values {
                refs_and_bounds(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn reason(value: &Value) -> Result<()> {
    if !value.is_null()
        && !matches!(
            value.as_str(),
            Some(
                "malformed"
                    | "unsupported_version"
                    | "unsupported_feature"
                    | "not_admitted"
                    | "unavailable"
                    | "content_unavailable"
                    | "identity_mismatch"
                    | "incompatible"
                    | "revoked"
                    | "stale"
                    | "cannot_enforce"
                    | "limit_exceeded"
                    | "idempotency_conflict"
                    | "conflict"
            )
        )
    {
        return Err(error(Malformed, "refusal code"));
    }
    Ok(())
}
fn unique<'a>(values: impl Iterator<Item = &'a str>) -> Result<()> {
    let mut seen = BTreeSet::new();
    for key in values {
        if key.is_empty() || !seen.insert(key) {
            return Err(error(Conflict, "duplicate/empty key"));
        }
    }
    Ok(())
}
pub fn resource(value: &Value) -> Result<ResourceRef> {
    let r: ResourceRef = serde_json::from_value(value.clone())
        .map_err(|_| error(Malformed, "resource reference"))?;
    r.validate()?;
    Ok(r)
}
pub fn integer(body: &Value, key: &str) -> Result<u64> {
    body[key]
        .as_u64()
        .filter(|n| *n <= 9_007_199_254_740_991)
        .ok_or_else(|| error(Malformed, key))
}
fn hex(value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || matches!(b, b'a'..=b'f'))
    {
        Err(error(Malformed, "64-hex identity"))
    } else {
        Ok(())
    }
}
pub fn error(code: RefusalCode, detail: &str) -> ContractError {
    ContractError::new(code, detail)
}
/// Validate an exact observation/content relationship. Source currentness and
/// conditional mutation are independent host checks.
pub fn verify_document(snapshot: &Value, observation: &Value, bytes: Option<&[u8]>) -> Result<()> {
    validate(snapshot)?;
    let observed = contracts::parse_observation(observation)?;
    let r = resource(&snapshot["resource"])?;
    if observed.resource_scope != r.workspace || observed.resource_id != r.resource {
        return Err(error(IdentityMismatch, "document observation scope"));
    }
    check_artifact_bytes(
        &parse_artifact(&snapshot["observation"])?,
        &contracts::jcs(observation)?,
    )?;
    if snapshot["derivation"].is_null() {
        if observed.content
            != if snapshot["content"].is_null() {
                None
            } else {
                Some(parse_artifact(&snapshot["content"])?)
            }
        {
            return Err(error(IdentityMismatch, "exact document content"));
        }
    }
    if snapshot["editable"] == true
        && (observed.consistency != contracts::Consistency::Conditional
            || observed.provider_revision.is_none())
    {
        return Err(error(
            RefusalCode::CannotEnforce,
            "editable provider revision",
        ));
    }
    match (bytes, snapshot["content"].is_null()) {
        (Some(bytes), false) => {
            check_artifact_bytes(&parse_artifact(&snapshot["content"])?, bytes)?;
            if snapshot["editable"] == true {
                std::str::from_utf8(bytes).map_err(|_| error(Malformed, "editable UTF-8"))?;
            }
        }
        (None, true) => {}
        _ => return Err(error(RefusalCode::ContentUnavailable, "document bytes")),
    }
    Ok(())
}
/// Ranges bind retained exact snapshot bytes and valid UTF-8 boundaries.
pub fn verify_range(range: &Value, snapshot: &Value, bytes: &[u8]) -> Result<(usize, usize)> {
    let object = range.as_object().ok_or_else(|| error(Malformed, "range"))?;
    if object.len() != 4
        || object
            .keys()
            .any(|k| !matches!(k.as_str(), "snapshot" | "unit" | "start" | "end"))
        || range["unit"] != "utf8_byte"
    {
        return Err(error(Malformed, "range fields/unit"));
    }
    validate(snapshot)?;
    if range["snapshot"]["schema"] != "openagents.document-snapshot.v1" {
        return Err(error(IdentityMismatch, "range snapshot schema"));
    }
    check_artifact_bytes(
        &parse_artifact(&range["snapshot"])?,
        &contracts::jcs(snapshot)?,
    )?;
    check_artifact_bytes(&parse_artifact(&snapshot["content"])?, bytes)?;
    let text = std::str::from_utf8(bytes).map_err(|_| error(Malformed, "range UTF-8"))?;
    let start = usize::try_from(integer(range, "start")?)
        .map_err(|_| error(LimitExceeded, "range start"))?;
    let end =
        usize::try_from(integer(range, "end")?).map_err(|_| error(LimitExceeded, "range end"))?;
    if start > end
        || end > bytes.len()
        || !text.is_char_boundary(start)
        || !text.is_char_boundary(end)
    {
        return Err(error(Malformed, "UTF-8 range bounds"));
    }
    Ok((start, end))
}
/// Exact cuts establish sequence continuity; arrival order cannot resolve forks.
pub fn verify_delta(delta: &Value, from: &Value, to: &Value) -> Result<()> {
    for body in [delta, from, to] {
        validate(body)?;
    }
    for (key, cut) in [("from", from), ("to", to)] {
        check_artifact_bytes(&parse_artifact(&delta[key])?, &contracts::jcs(cut)?)?;
    }
    if delta["projection"] != from["projection"]
        || from["projection"] != to["projection"]
        || from["epoch"] != to["epoch"]
        || integer(to, "sequence")?
            != integer(from, "sequence")?
                .checked_add(1)
                .ok_or_else(|| error(LimitExceeded, "projection counter"))?
    {
        return Err(error(Conflict, "projection continuity"));
    }
    Ok(())
}

/// Verify an admitted descriptor graph as a whole. Equal names never establish
/// identity, and omitted parents cannot produce a complete resource graph.
pub fn verify_resources(binding: &Value, descriptors: &[Value]) -> Result<()> {
    validate(binding)?;
    if binding["v"] != "openagents.workspace-binding.v1" || descriptors.len() > 256 {
        return Err(error(Malformed, "resource graph binding/bound"));
    }
    let binding_bytes = contracts::jcs(binding)?;
    let roots = binding["roots"]
        .as_array()
        .ok_or_else(|| error(Malformed, "roots"))?
        .iter()
        .map(resource)
        .collect::<Result<BTreeSet<_>>>()?;
    let mut graph = BTreeMap::new();
    for descriptor in descriptors {
        validate(descriptor)?;
        if descriptor["v"] != "openagents.workspace-resource.v1" {
            return Err(error(Malformed, "resource descriptor"));
        }
        let r = resource(&descriptor["ref"])?;
        if r.authority != binding["authority"] || r.workspace != binding["workspace"] {
            return Err(error(IdentityMismatch, "resource graph namespace"));
        }
        check_artifact_bytes(&parse_artifact(&descriptor["binding"])?, &binding_bytes)?;
        if roots.contains(&r) && !descriptor["parent"].is_null() {
            return Err(error(Malformed, "root descriptor has a parent"));
        }
        let parent = if descriptor["parent"].is_null() {
            if !roots.contains(&r) {
                return Err(error(IdentityMismatch, "unadmitted root"));
            }
            None
        } else {
            Some(resource(&descriptor["parent"])?)
        };
        if graph.insert(r, parent).is_some() {
            return Err(error(Conflict, "resource identity repeated"));
        }
    }
    for root in &roots {
        if !graph.contains_key(root) {
            return Err(error(
                RefusalCode::ContentUnavailable,
                "root descriptor unavailable",
            ));
        }
    }
    for r in graph.keys() {
        let mut path = BTreeSet::new();
        let mut current = Some(r.clone());
        while let Some(r) = current {
            if !path.insert(r.clone()) {
                return Err(error(Conflict, "resource parent cycle"));
            }
            current = graph
                .get(&r)
                .ok_or_else(|| {
                    error(
                        RefusalCode::ContentUnavailable,
                        "parent descriptor unavailable",
                    )
                })?
                .clone();
        }
    }
    Ok(())
}
