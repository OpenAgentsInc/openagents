//! Revised NIP-PRG v1 program definitions.
//!
//! A definition names pinned steps. It grants nothing, carries no shell
//! command, and selecting it is not admission. A missing pin is
//! `content_unavailable`. An unknown effect is not safe to retry.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use crate::contracts::{
    ContractError, RefusalCode, parse_artifact, parse_schema_ref, parse_strict, prepare_closure,
    validate_instance,
};
use crate::domain::Tag;

/// Addressable program discovery.
pub const DISCOVERY_KIND: u16 = 30_182;
/// Program discovery marker.
pub const PROGRAM_MARKER: &str = "oa:program:v1";
/// Default ceiling on nested child programs.
pub const DEFAULT_MAX_DEPTH: usize = 8;

const KINDS: &[&str] = &[
    "query", "check", "decide", "delegate", "program", "module", "invoke",
];

/// One validated program definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Definition {
    /// Qualified id.
    pub id: String,
    /// Component slug.
    pub component: String,
    /// Inert summary.
    pub summary: String,
    /// Steps in source order.
    pub steps: Vec<Step>,
}

/// One step in a definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    /// Unique slug.
    pub name: String,
    /// One of the seven kinds.
    pub kind: String,
    /// Preceding step names. Each must appear earlier in the array.
    pub after: Vec<String>,
    /// `stop` or `continue`.
    pub on_error: String,
    /// Qualified target id, when the step names one.
    pub target: Option<String>,
}

/// What a caller asked to run. `None` is an ordinary turn, not a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Selection {
    /// A program component slug.
    Program(String),
    /// No program.
    None,
}

/// Parse a program definition.
///
/// # Errors
///
/// Returns a typed refusal for an unknown field or kind, a cycle, a
/// forward dependency, a malformed binding, or a command hidden on a query.
pub fn parse_definition(value: &Value) -> Result<Definition, ContractError> {
    let object = as_map(value, "definition")?;
    reject(
        object,
        &[
            "v", "requires", "id", "summary", "input", "output", "steps", "result", "bounds",
            "meta",
        ],
        "definition",
    )?;
    version(object, "definition")?;
    let id = qualified(require(object, "id", "definition")?)?;
    let component = id.rsplit('/').next().unwrap_or("").to_string();
    let summary = text(require(object, "summary", "definition")?, "summary")?.to_string();
    schema(require(object, "input", "definition")?)?;
    schema(require(object, "output", "definition")?)?;
    bound_object(require(object, "bounds", "definition")?)?;
    if let Some(meta) = object.get("meta") {
        meta.as_object().ok_or_else(|| malformed("meta"))?;
    }
    let steps_value = require(object, "steps", "definition")?
        .as_array()
        .ok_or_else(|| malformed("steps"))?;
    if steps_value.is_empty() || steps_value.len() > 256 {
        return Err(malformed("steps"));
    }
    let mut steps = Vec::with_capacity(steps_value.len());
    let mut seen = BTreeSet::new();
    for step in steps_value {
        steps.push(parse_step(step, &steps, &mut seen)?);
    }
    parse_binding(require(object, "result", "definition")?, &steps, false)?;
    Ok(Definition {
        id,
        component,
        summary,
        steps,
    })
}

/// Selection does not admit the program. `None` leaves the turn unchanged.
#[must_use]
pub fn admits(selection: &Selection, host_allows: bool) -> bool {
    match selection {
        Selection::None => false,
        Selection::Program(_) => host_allows,
    }
}

/// Discovery tags. One `oa:step:` tag per distinct kind, agreeing with the body.
#[must_use]
pub fn discovery_tags(definition: &Definition) -> Vec<Tag> {
    let mut tags = vec![Tag::new(vec!["t".into(), PROGRAM_MARKER.into()])];
    let mut kinds = BTreeSet::new();
    for step in &definition.steps {
        if kinds.insert(step.kind.clone()) {
            tags.push(Tag::new(vec!["t".into(), format!("oa:step:{}", step.kind)]));
        }
    }
    tags
}

/// Refuse tags that name a step kind the definition does not contain.
///
/// # Errors
///
/// Returns [`RefusalCode::IdentityMismatch`] when a step tag disagrees.
pub fn check_discovery_tags(tags: &[Tag], definition: &Definition) -> Result<(), ContractError> {
    let mut marker = 0;
    let mut seen = BTreeSet::new();
    for tag in tags {
        if tag.name() != Some("t") {
            continue;
        }
        let value = tag.value().unwrap_or("");
        if value == PROGRAM_MARKER {
            marker += 1;
        } else if let Some(kind) = value.strip_prefix("oa:step:") {
            if !definition.steps.iter().any(|step| step.kind == kind) {
                return Err(ContractError::new(
                    RefusalCode::IdentityMismatch,
                    "discovery step tag",
                ));
            }
            if !seen.insert(kind.to_string()) {
                return Err(malformed("duplicate step tag"));
            }
        }
    }
    let distinct = definition
        .steps
        .iter()
        .map(|step| step.kind.as_str())
        .collect::<BTreeSet<_>>();
    if marker != 1 || seen.len() != distinct.len() {
        return Err(malformed("discovery tags"));
    }
    Ok(())
}

/// Pin every definition the root names. A digest that is absent is not
/// replaced by another entry in `store`. A shared child is pinned once.
/// A node already on the path is a cycle.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] when a pinned id has no
/// bytes, [`RefusalCode::Incompatible`] for a cycle, and
/// [`RefusalCode::LimitExceeded`] when a chain exceeds [`DEFAULT_MAX_DEPTH`].
pub fn pin_closure(
    root: &str,
    edges: &BTreeMap<String, Vec<String>>,
    store: &BTreeMap<String, Vec<u8>>,
) -> Result<BTreeMap<String, Vec<u8>>, ContractError> {
    let mut retained = BTreeMap::new();
    let mut path = Vec::new();
    retain(root, edges, store, &mut retained, &mut path)?;
    Ok(retained)
}

fn retain(
    id: &str,
    edges: &BTreeMap<String, Vec<String>>,
    store: &BTreeMap<String, Vec<u8>>,
    retained: &mut BTreeMap<String, Vec<u8>>,
    path: &mut Vec<String>,
) -> Result<(), ContractError> {
    if path.iter().any(|seen| seen == id) {
        return Err(ContractError::new(
            RefusalCode::Incompatible,
            "program cycle",
        ));
    }
    if retained.contains_key(id) {
        return Ok(());
    }
    let Some(bytes) = store.get(id) else {
        return Err(ContractError::new(RefusalCode::ContentUnavailable, id));
    };
    if path.len() >= DEFAULT_MAX_DEPTH {
        return Err(ContractError::new(
            RefusalCode::LimitExceeded,
            "program depth",
        ));
    }
    path.push(id.to_string());
    retained.insert(id.to_string(), bytes.clone());
    for child in edges.get(id).into_iter().flatten() {
        retain(child, edges, store, retained, path)?;
    }
    path.pop();
    Ok(())
}

/// Bytes for `pinned`. A newer `head` is not a substitute.
///
/// # Errors
///
/// Returns [`RefusalCode::Stale`] when `pinned` is absent and `head` names
/// different bytes. Returns [`RefusalCode::ContentUnavailable`] when neither
/// id is in `store`.
pub fn locked_definition<'a>(
    pinned: &str,
    head: &str,
    store: &'a BTreeMap<String, Vec<u8>>,
) -> Result<&'a [u8], ContractError> {
    if let Some(bytes) = store.get(pinned) {
        return Ok(bytes.as_slice());
    }
    if pinned != head && store.contains_key(head) {
        return Err(ContractError::new(RefusalCode::Stale, "program head"));
    }
    Err(ContractError::new(RefusalCode::ContentUnavailable, pinned))
}

/// An unknown effect blocks retry even when `retry_on` names the cause.
///
/// # Errors
///
/// Returns [`RefusalCode::CannotEnforce`] when the effect is unknown.
pub fn allow_retry(unknown_effect: bool) -> Result<(), ContractError> {
    if unknown_effect {
        return Err(ContractError::new(
            RefusalCode::CannotEnforce,
            "unknown effect is not retried",
        ));
    }
    Ok(())
}

/// A child reservation wider than the parent has left is a grant widening.
///
/// # Errors
///
/// Returns [`RefusalCode::Conflict`] when `child_requested` exceeds
/// `parent_remaining`.
pub fn reserve_within(parent_remaining: u64, child_requested: u64) -> Result<(), ContractError> {
    if child_requested > parent_remaining {
        return Err(ContractError::new(RefusalCode::Conflict, "grant widening"));
    }
    Ok(())
}

/// Iteration indexes in source order. Overflow refuses before the first item.
///
/// # Errors
///
/// Returns [`RefusalCode::LimitExceeded`] when `item_count` exceeds `max_items`.
pub fn each_indexes(item_count: usize, max_items: u64) -> Result<Vec<usize>, ContractError> {
    if item_count as u64 > max_items {
        return Err(ContractError::new(RefusalCode::LimitExceeded, "each"));
    }
    Ok((0..item_count).collect())
}

/// `stop` blocks descendants after a failed dependency. `continue` does not
/// relabel that failure as success; it only leaves the outcome visible.
///
/// # Errors
///
/// Returns [`RefusalCode::UnsupportedFeature`] for any other `on_error`.
pub fn blocks_descendants(on_error: &str, dependency_failed: bool) -> Result<bool, ContractError> {
    match on_error {
        "stop" => Ok(dependency_failed),
        "continue" => Ok(false),
        _ => Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "on_error",
        )),
    }
}

/// One step result. Outcome, verification, and integration stay separate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Envelope {
    /// Common outcome, or `skipped`.
    pub outcome: String,
    /// Verification state.
    pub verification: String,
    /// Integration state.
    pub integration: String,
}

/// Parse a step envelope.
///
/// # Errors
///
/// Returns a typed refusal when a field is missing or names an unknown state.
pub fn parse_envelope(value: &Value) -> Result<Envelope, ContractError> {
    let object = as_map(value, "envelope")?;
    reject(
        object,
        &["outcome", "value", "receipt", "verification", "integration"],
        "envelope",
    )?;
    let outcome = text(require(object, "outcome", "envelope")?, "outcome")?;
    if !matches!(
        outcome,
        "completed" | "refused" | "failed" | "cancelled" | "unknown" | "skipped"
    ) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "envelope.outcome",
        ));
    }
    if outcome != "completed" && object.get("value") != Some(&Value::Null) {
        return Err(malformed("envelope.value"));
    }
    if !object.contains_key("value") {
        return Err(malformed("envelope.value"));
    }
    match object.get("receipt") {
        Some(Value::Null) => {}
        Some(receipt) => {
            parse_artifact(receipt)?;
        }
        None => return Err(malformed("envelope.receipt")),
    }
    let verification = text(require(object, "verification", "envelope")?, "verification")?;
    if !matches!(
        verification,
        "passed" | "failed" | "unverifiable" | "not_run"
    ) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "envelope.verification",
        ));
    }
    let integration = text(require(object, "integration", "envelope")?, "integration")?;
    if !matches!(
        integration,
        "accepted" | "rejected" | "pending" | "not_requested"
    ) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "envelope.integration",
        ));
    }
    Ok(Envelope {
        outcome: outcome.to_string(),
        verification: verification.to_string(),
        integration: integration.to_string(),
    })
}

/// A completed value is not acceptance. Acceptance needs verification and
/// integration as well.
#[must_use]
pub fn accepted(envelope: &Envelope) -> bool {
    envelope.outcome == "completed"
        && envelope.verification == "passed"
        && envelope.integration == "accepted"
}

/// Validate `instance` against the pinned schema document `digest`.
///
/// # Errors
///
/// Returns the schema evaluator's refusal: a missing document, an
/// unsupported keyword, or an instance the schema does not accept.
pub fn validate_result(
    documents: &BTreeMap<String, Vec<u8>>,
    digest: &str,
    instance: &Value,
) -> Result<(), ContractError> {
    let closure = prepare_closure(documents)?;
    validate_instance(&closure, digest, instance)
}

fn parse_step(
    value: &Value,
    earlier: &[Step],
    seen: &mut BTreeSet<String>,
) -> Result<Step, ContractError> {
    let object = as_map(value, "step")?;
    reject(
        object,
        &[
            "name", "kind", "target", "after", "input", "output", "bounds", "on_error", "when",
            "retry", "each",
        ],
        "step",
    )?;
    let name = slug(require(object, "name", "step")?)?;
    if !seen.insert(name.clone()) {
        return Err(ContractError::new(RefusalCode::Conflict, "duplicate step"));
    }
    let kind = text(require(object, "kind", "step")?, "kind")?;
    if !KINDS.contains(&kind) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            format!("step kind {kind}"),
        ));
    }
    if kind == "query" {
        for hidden in ["command", "argv", "path", "shell"] {
            if object.contains_key(hidden) {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "query command",
                ));
            }
        }
    }
    if kind == "invoke"
        && let Some(id) = object
            .get("target")
            .and_then(|target| target.get("id"))
            .and_then(Value::as_str)
        && (id.contains(' ') || id.contains('\\'))
    {
        return Err(malformed("invoke target"));
    }
    let after = string_list(require(object, "after", "step")?, "after")?;
    let mut after_seen = BTreeSet::new();
    for dependency in &after {
        if !after_seen.insert(dependency.clone()) {
            return Err(malformed("after"));
        }
        if !earlier.iter().any(|step| step.name == *dependency) {
            return Err(ContractError::new(
                RefusalCode::Incompatible,
                format!("step {name} depends on {dependency}"),
            ));
        }
    }
    let on_error = text(require(object, "on_error", "step")?, "on_error")?;
    if on_error != "stop" && on_error != "continue" {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "on_error",
        ));
    }
    let target = match object.get("target") {
        None => None,
        Some(value) => Some(definition_ref(value)?),
    };
    let predecessors = dependency_closure(&after, earlier);
    let has_each = object.contains_key("each");
    parse_binding(require(object, "input", "step")?, &predecessors, has_each)?;
    schema(require(object, "output", "step")?)?;
    bound_object(require(object, "bounds", "step")?)?;
    if let Some(when) = object.get("when") {
        parse_when(when, &predecessors, has_each)?;
    }
    if let Some(retry) = object.get("retry") {
        parse_retry(retry)?;
    }
    if let Some(each) = object.get("each") {
        parse_each(each, &predecessors)?;
    }
    Ok(Step {
        name,
        kind: kind.to_string(),
        after,
        on_error: on_error.to_string(),
        target,
    })
}

fn parse_when(value: &Value, earlier: &[Step], allow_item: bool) -> Result<(), ContractError> {
    let object = as_map(value, "when")?;
    reject(object, &["source", "equals", "present"], "when")?;
    parse_binding(require(object, "source", "when")?, earlier, allow_item)?;
    match (object.get("equals"), object.get("present")) {
        (Some(_), None) => Ok(()),
        (None, Some(present)) if present == &Value::Bool(true) => Ok(()),
        _ => Err(malformed("when")),
    }
}

fn parse_retry(value: &Value) -> Result<(), ContractError> {
    let object = as_map(value, "retry")?;
    reject(object, &["max_attempts", "retry_on"], "retry")?;
    let attempts = require(object, "max_attempts", "retry")?
        .as_u64()
        .ok_or_else(|| malformed("max_attempts"))?;
    if attempts == 0 {
        return Err(malformed("max_attempts"));
    }
    let _causes = string_list(require(object, "retry_on", "retry")?, "retry_on")?;
    Ok(())
}

fn parse_each(value: &Value, earlier: &[Step]) -> Result<(), ContractError> {
    let object = as_map(value, "each")?;
    reject(object, &["items", "max_items"], "each")?;
    parse_binding(require(object, "items", "each")?, earlier, false)?;
    let max_items = require(object, "max_items", "each")?
        .as_u64()
        .ok_or_else(|| malformed("max_items"))?;
    if max_items == 0 {
        return Err(malformed("max_items"));
    }
    Ok(())
}

fn parse_binding(value: &Value, earlier: &[Step], allow_item: bool) -> Result<(), ContractError> {
    let object = as_map(value, "binding")?;
    let discriminant = ["literal", "from", "object", "array"]
        .into_iter()
        .find(|key| object.contains_key(*key));
    let Some(discriminant) = discriminant else {
        return Err(malformed("binding"));
    };
    if ["literal", "from", "object", "array"]
        .into_iter()
        .filter(|key| object.contains_key(*key))
        .count()
        != 1
    {
        return Err(malformed("binding"));
    }
    match discriminant {
        "literal" => Ok(()),
        "from" => {
            let from = text(require(object, "from", "binding")?, "from")?;
            let pointer = object.get("pointer").and_then(Value::as_str).unwrap_or("");
            if pointer.contains('*') || pointer.contains('$') {
                return Err(ContractError::new(
                    RefusalCode::UnsupportedFeature,
                    "binding pointer",
                ));
            }
            if from == "input" || (allow_item && from == "item") {
                return Ok(());
            }
            let Some(name) = from.strip_prefix("step:") else {
                return Err(malformed("binding from"));
            };
            if !earlier.iter().any(|step| step.name == name) {
                return Err(ContractError::new(
                    RefusalCode::Incompatible,
                    "binding step",
                ));
            }
            Ok(())
        }
        "object" => {
            let fields = as_map(require(object, "object", "binding")?, "object")?;
            for field in fields.values() {
                parse_binding(field, earlier, allow_item)?;
            }
            Ok(())
        }
        "array" => {
            let items = require(object, "array", "binding")?
                .as_array()
                .ok_or_else(|| malformed("array"))?;
            for item in items {
                parse_binding(item, earlier, allow_item)?;
            }
            Ok(())
        }
        _ => Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "binding",
        )),
    }
}

fn definition_ref(value: &Value) -> Result<String, ContractError> {
    let object = as_map(value, "target")?;
    reject(object, &["id", "artifact", "event"], "target")?;
    let id = qualified(require(object, "id", "target")?)?;
    parse_artifact(require(object, "artifact", "target")?)?;
    Ok(id)
}

fn schema(value: &Value) -> Result<(), ContractError> {
    parse_schema_ref(value)?;
    Ok(())
}

/// Steps named by `after`, plus the steps those depend on.
fn dependency_closure(after: &[String], earlier: &[Step]) -> Vec<Step> {
    let mut names = BTreeSet::new();
    let mut pending = after.to_vec();
    while let Some(name) = pending.pop() {
        if !names.insert(name.clone()) {
            continue;
        }
        if let Some(step) = earlier.iter().find(|step| step.name == name) {
            pending.extend(step.after.iter().cloned());
        }
    }
    earlier
        .iter()
        .filter(|step| names.contains(&step.name))
        .cloned()
        .collect()
}

fn bound_object(value: &Value) -> Result<(), ContractError> {
    let object = as_map(value, "bounds")?;
    for name in object.keys() {
        if !matches!(
            name.as_str(),
            "wall_ms"
                | "memory_bytes"
                | "input_bytes"
                | "output_bytes"
                | "read_bytes"
                | "storage_bytes"
                | "calls"
                | "attempts"
                | "concurrency"
                | "depth"
                | "fuel"
                | "spend_microunits"
        ) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                format!("bound {name}"),
            ));
        }
    }
    Ok(())
}

fn version(object: &Map<String, Value>, path: &str) -> Result<(), ContractError> {
    if object.get("v").and_then(Value::as_u64) != Some(1) {
        return Err(ContractError::new(
            RefusalCode::UnsupportedVersion,
            format!("{path}.v"),
        ));
    }
    let requires = require(object, "requires", path)?
        .as_array()
        .ok_or_else(|| malformed("requires"))?;
    if !requires.is_empty() {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "requires",
        ));
    }
    Ok(())
}

fn as_map<'a>(value: &'a Value, path: &str) -> Result<&'a Map<String, Value>, ContractError> {
    value.as_object().ok_or_else(|| malformed(path))
}

fn require<'a>(
    object: &'a Map<String, Value>,
    key: &str,
    path: &str,
) -> Result<&'a Value, ContractError> {
    object
        .get(key)
        .ok_or_else(|| malformed(format!("{path}.{key}")))
}

fn reject(object: &Map<String, Value>, allowed: &[&str], path: &str) -> Result<(), ContractError> {
    for key in object.keys() {
        if !allowed.contains(&key.as_str()) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                format!("{path}.{key}"),
            ));
        }
    }
    Ok(())
}

fn text<'a>(value: &'a Value, path: &str) -> Result<&'a str, ContractError> {
    value.as_str().ok_or_else(|| malformed(path))
}

fn string_list(value: &Value, path: &str) -> Result<Vec<String>, ContractError> {
    value
        .as_array()
        .ok_or_else(|| malformed(path))?
        .iter()
        .map(|item| text(item, path).map(str::to_string))
        .collect()
}

fn qualified(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "id")?;
    let Some((publisher, rest)) = text.split_once(':') else {
        return Err(malformed("id"));
    };
    let Some((package, component)) = rest.split_once('/') else {
        return Err(malformed("id"));
    };
    if publisher.len() != 64
        || !publisher
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        || !is_slug(package)
        || !is_slug(component)
    {
        return Err(malformed("id"));
    }
    Ok(text.to_string())
}

fn slug(value: &Value) -> Result<String, ContractError> {
    let text = text(value, "name")?;
    if !is_slug(text) {
        return Err(malformed("name"));
    }
    Ok(text.to_string())
}

fn is_slug(value: &str) -> bool {
    let mut chars = value.chars();
    let Some(first) = chars.next() else {
        return false;
    };
    value.len() <= 64
        && (first.is_ascii_lowercase() || first.is_ascii_digit())
        && chars.all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_' || ch == '-')
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}

/// Parse bytes with the shared strict JSON parser.
///
/// # Errors
///
/// Returns the parser's refusal.
pub fn parse_json(bytes: &[u8]) -> Result<Value, ContractError> {
    parse_strict(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const PUB: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const SCHEMA: &str = "sha256:a2c799262a3ce3c19ef5cdd983bf3d12b43ab3c426227091b909dcb7054738c0";

    fn schema() -> Value {
        json!({"digest": SCHEMA, "size": 17, "media_type": "application/schema+json"})
    }

    fn target(component: &str) -> Value {
        json!({"id": format!("{PUB}:openagents/{component}"), "artifact": {"digest": SCHEMA, "size": 17, "media_type": "application/octet-stream"}})
    }

    fn step(name: &str, kind: &str, after: Value) -> Value {
        json!({
            "name": name,
            "kind": kind,
            "target": target(name),
            "after": after,
            "input": {"from": "input", "pointer": ""},
            "output": schema(),
            "bounds": {},
            "on_error": "stop"
        })
    }

    fn program(steps: Value) -> Value {
        json!({
            "v": 1,
            "requires": [],
            "id": format!("{PUB}:openagents/demo"),
            "summary": "a typed workflow",
            "input": schema(),
            "output": schema(),
            "steps": steps,
            "result": {"from": "step:one", "pointer": "/value"},
            "bounds": {}
        })
    }

    #[test]
    fn seven_kinds_parse_and_a_forward_dependency_is_refused() {
        let body = program(json!([
            step("one", "query", json!([])),
            step("two", "check", json!(["one"])),
            step("three", "decide", json!(["two"])),
            step("four", "delegate", json!(["three"])),
            step("five", "program", json!(["four"])),
            step("six", "module", json!(["five"])),
            step("seven", "invoke", json!(["six"]))
        ]));
        let parsed = parse_definition(&body).unwrap();
        assert_eq!(parsed.steps.len(), 7);
        let tags = discovery_tags(&parsed);
        check_discovery_tags(&tags, &parsed).unwrap();

        let forward = program(json!([step("one", "query", json!(["later"]))]));
        assert_eq!(
            parse_definition(&forward).unwrap_err().code,
            RefusalCode::Incompatible
        );
        let mut command = step("one", "query", json!([]));
        command["command"] = json!("rm -rf /");
        assert_eq!(
            parse_definition(&program(json!([command])))
                .unwrap_err()
                .code,
            RefusalCode::UnsupportedFeature
        );
        let teleport = program(json!([step("one", "teleport", json!([]))]));
        let error = parse_definition(&teleport).unwrap_err();
        assert!(error.to_string().contains("teleport"), "{error}");
    }

    #[test]
    fn none_is_not_admission_and_an_unknown_effect_is_not_retried() {
        assert!(!admits(&Selection::None, true));
        assert!(!admits(&Selection::Program("demo".into()), false));
        assert!(admits(&Selection::Program("demo".into()), true));
        assert_eq!(
            allow_retry(true).unwrap_err().code,
            RefusalCode::CannotEnforce
        );
        assert!(allow_retry(false).is_ok());
    }

    #[test]
    fn a_missing_pin_is_not_replaced_by_a_newer_head() {
        let mut store = BTreeMap::new();
        store.insert(format!("{PUB}:openagents/root"), b"root".to_vec());
        store.insert(format!("{PUB}:openagents/latest"), b"latest".to_vec());
        let mut edges = BTreeMap::new();
        edges.insert(
            format!("{PUB}:openagents/root"),
            vec![format!("{PUB}:openagents/child")],
        );
        assert_eq!(
            pin_closure(&format!("{PUB}:openagents/root"), &edges, &store)
                .unwrap_err()
                .code,
            RefusalCode::ContentUnavailable
        );
        assert_eq!(
            locked_definition(
                &format!("{PUB}:openagents/child"),
                &format!("{PUB}:openagents/latest"),
                &store
            )
            .unwrap_err()
            .code,
            RefusalCode::Stale
        );
        store.insert(format!("{PUB}:openagents/child"), b"child".to_vec());
        assert_eq!(
            locked_definition(
                &format!("{PUB}:openagents/child"),
                &format!("{PUB}:openagents/latest"),
                &store
            )
            .unwrap(),
            b"child".as_slice()
        );
    }

    #[test]
    fn schemas_conditions_conflicts_and_grants_refuse_as_specified() {
        let mut missing = program(json!([step("one", "query", json!([]))]));
        missing.as_object_mut().unwrap().remove("input");
        assert_eq!(
            parse_definition(&missing).unwrap_err().code,
            RefusalCode::Malformed
        );

        let mut incompatible = step("one", "query", json!([]));
        incompatible["output"]["media_type"] = json!("text/plain");
        assert_eq!(
            parse_definition(&program(json!([incompatible])))
                .unwrap_err()
                .code,
            RefusalCode::Incompatible
        );

        let mut when = step("one", "query", json!([]));
        when["when"] = json!({"source": {"from": "input", "pointer": ""}, "present": false});
        assert_eq!(
            parse_definition(&program(json!([when]))).unwrap_err().code,
            RefusalCode::Malformed
        );

        let mut item = step("one", "query", json!([]));
        item["input"] = json!({"from": "item", "pointer": ""});
        assert_eq!(
            parse_definition(&program(json!([item]))).unwrap_err().code,
            RefusalCode::Malformed
        );

        let duplicated = program(json!([
            step("one", "query", json!([])),
            step("one", "check", json!(["one"]))
        ]));
        assert_eq!(
            parse_definition(&duplicated).unwrap_err().code,
            RefusalCode::Conflict
        );

        assert_eq!(
            reserve_within(2, 3).unwrap_err().code,
            RefusalCode::Conflict
        );
        assert!(reserve_within(3, 3).is_ok());
        assert_eq!(each_indexes(2, 4).unwrap(), vec![0, 1]);
        assert_eq!(
            each_indexes(4, 2).unwrap_err().code,
            RefusalCode::LimitExceeded
        );
        assert!(blocks_descendants("stop", true).unwrap());
        assert!(!blocks_descendants("continue", true).unwrap());

        let envelope = parse_envelope(&json!({
            "outcome": "completed",
            "value": {"ok": true},
            "receipt": null,
            "verification": "not_run",
            "integration": "pending"
        }))
        .unwrap();
        assert!(!accepted(&envelope));

        let mut documents = BTreeMap::new();
        documents.insert(SCHEMA.to_string(), br#"{"type":"string"}"#.to_vec());
        assert!(validate_result(&documents, SCHEMA, &json!("ok")).is_ok());
        assert_eq!(
            validate_result(&documents, SCHEMA, &json!(1))
                .unwrap_err()
                .code,
            RefusalCode::Malformed
        );
        assert_eq!(
            validate_result(
                &documents,
                "sha256:0000000000000000000000000000000000000000000000000000000000000000",
                &json!("ok")
            )
            .unwrap_err()
            .code,
            RefusalCode::ContentUnavailable
        );
    }

    #[test]
    fn a_cycle_refuses_and_a_shared_child_is_pinned_once() {
        let root = format!("{PUB}:openagents/root");
        let left = format!("{PUB}:openagents/left");
        let right = format!("{PUB}:openagents/right");
        let shared = format!("{PUB}:openagents/shared");
        let mut store = BTreeMap::new();
        for id in [&root, &left, &right, &shared] {
            store.insert(id.clone(), id.as_bytes().to_vec());
        }
        let mut edges = BTreeMap::new();
        edges.insert(root.clone(), vec![left.clone(), right.clone()]);
        edges.insert(left.clone(), vec![shared.clone()]);
        edges.insert(right.clone(), vec![shared.clone()]);
        let pinned = pin_closure(&root, &edges, &store).unwrap();
        assert_eq!(pinned.len(), 4);
        assert_eq!(pinned.get(&shared).unwrap(), shared.as_bytes());

        edges.insert(shared.clone(), vec![root.clone()]);
        assert_eq!(
            pin_closure(&root, &edges, &store).unwrap_err().code,
            RefusalCode::Incompatible
        );

        let mut chain_store = BTreeMap::new();
        let mut chain_edges = BTreeMap::new();
        let mut previous = String::new();
        for index in 0..=DEFAULT_MAX_DEPTH {
            let id = format!("{PUB}:openagents/n{index}");
            chain_store.insert(id.clone(), b"n".to_vec());
            if !previous.is_empty() {
                chain_edges.insert(previous, vec![id.clone()]);
            }
            previous = id;
        }
        assert_eq!(
            pin_closure(&format!("{PUB}:openagents/n0"), &chain_edges, &chain_store)
                .unwrap_err()
                .code,
            RefusalCode::LimitExceeded
        );
    }
}
