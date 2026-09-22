//! A bounded JSON Schema 2020-12 evaluator.
//!
//! `$ref` resolves only inside the pinned closure. An `http` or `https`
//! reference is refused rather than fetched. A schema that requires a
//! vocabulary or keyword this evaluator does not implement is refused
//! before it is applied.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value};

use super::error::{ContractError, RefusalCode};
use super::json::{jcs, parse_strict};

const DRAFT: &str = "https://json-schema.org/draft/2020-12/schema";
const CORE: &str = "https://json-schema.org/draft/2020-12/vocab/core";
const APPLICATOR: &str = "https://json-schema.org/draft/2020-12/vocab/applicator";
const VALIDATION: &str = "https://json-schema.org/draft/2020-12/vocab/validation";
const META: &str = "https://json-schema.org/draft/2020-12/vocab/meta-data";

const KEYWORDS: &[&str] = &[
    "$schema",
    "$id",
    "$comment",
    "$ref",
    "$defs",
    "$vocabulary",
    "title",
    "description",
    "type",
    "enum",
    "const",
    "properties",
    "required",
    "additionalProperties",
    "prefixItems",
    "items",
    "minItems",
    "maxItems",
    "uniqueItems",
    "minLength",
    "maxLength",
    "minimum",
    "maximum",
    "exclusiveMinimum",
    "exclusiveMaximum",
    "minProperties",
    "maxProperties",
    "allOf",
    "anyOf",
    "oneOf",
    "not",
];

/// Schemas keyed by `sha256:` digest.
pub type SchemaClosure = BTreeMap<String, Value>;

/// Parse schema documents and refuse ones this evaluator cannot apply.
///
/// # Errors
///
/// Returns [`RefusalCode::UnsupportedFeature`] for an unknown keyword or a
/// required vocabulary, and [`RefusalCode::Malformed`] for a schema that is
/// not an object or boolean.
pub fn prepare_closure(
    documents: &BTreeMap<String, Vec<u8>>,
) -> Result<SchemaClosure, ContractError> {
    let mut closure = SchemaClosure::new();
    for (digest, bytes) in documents {
        let value = parse_strict(bytes)?;
        check_supported(&value)?;
        closure.insert(digest.clone(), value);
    }
    Ok(closure)
}

/// Validate `instance` against the schema document `digest` in `closure`.
///
/// # Errors
///
/// Returns [`RefusalCode::ContentUnavailable`] when the digest is not pinned,
/// [`RefusalCode::UnsupportedFeature`] for a remote reference, and
/// [`RefusalCode::Malformed`] when the instance does not match.
pub fn validate_instance(
    closure: &SchemaClosure,
    digest: &str,
    instance: &Value,
) -> Result<(), ContractError> {
    let schema = closure
        .get(digest)
        .ok_or_else(|| ContractError::new(RefusalCode::ContentUnavailable, "schema document"))?;
    let mut stack = Vec::new();
    apply(closure, digest, schema, instance, &mut stack)
}

fn check_supported(schema: &Value) -> Result<(), ContractError> {
    match schema {
        Value::Bool(_) => Ok(()),
        Value::Object(object) => {
            check_vocabulary(object)?;
            if let Some(dialect) = object.get("$schema") {
                let dialect = dialect
                    .as_str()
                    .ok_or_else(|| malformed("schema dialect"))?;
                if dialect != DRAFT {
                    return Err(ContractError::new(
                        RefusalCode::UnsupportedVersion,
                        "schema dialect",
                    ));
                }
            }
            for (key, value) in object {
                if !KEYWORDS.contains(&key.as_str()) {
                    return Err(ContractError::new(
                        RefusalCode::UnsupportedFeature,
                        format!("schema keyword {key}"),
                    ));
                }
                match key.as_str() {
                    "properties" | "$defs" => {
                        let map = value.as_object().ok_or_else(|| malformed(key))?;
                        for nested in map.values() {
                            check_supported(nested)?;
                        }
                    }
                    "items" | "additionalProperties" | "not" => check_supported(value)?,
                    "prefixItems" | "allOf" | "anyOf" | "oneOf" => {
                        let list = value.as_array().ok_or_else(|| malformed(key))?;
                        for nested in list {
                            check_supported(nested)?;
                        }
                    }
                    _ => {}
                }
            }
            Ok(())
        }
        _ => Err(malformed("schema document")),
    }
}

fn check_vocabulary(object: &Map<String, Value>) -> Result<(), ContractError> {
    let Some(vocabulary) = object.get("$vocabulary") else {
        return Ok(());
    };
    let vocabulary = vocabulary
        .as_object()
        .ok_or_else(|| malformed("$vocabulary"))?;
    for (name, required) in vocabulary {
        let required = required.as_bool().ok_or_else(|| malformed("$vocabulary"))?;
        if required && !matches!(name.as_str(), CORE | APPLICATOR | VALIDATION | META) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "schema vocabulary",
            ));
        }
    }
    Ok(())
}

fn apply(
    closure: &SchemaClosure,
    document: &str,
    schema: &Value,
    instance: &Value,
    stack: &mut Vec<(String, String)>,
) -> Result<(), ContractError> {
    let Value::Object(object) = schema else {
        return if schema.as_bool() == Some(true) {
            Ok(())
        } else if schema.as_bool() == Some(false) {
            Err(malformed("schema false"))
        } else {
            Err(malformed("schema"))
        };
    };
    if let Some(reference) = object.get("$ref") {
        let reference = reference.as_str().ok_or_else(|| malformed("$ref"))?;
        let (next_document, pointer) = resolve_ref(document, reference)?;
        let frame = (next_document.clone(), pointer.clone());
        if stack.contains(&frame) || stack.len() > 32 {
            return Err(ContractError::new(
                RefusalCode::LimitExceeded,
                "schema reference",
            ));
        }
        let target_document = closure.get(&next_document).ok_or_else(|| {
            ContractError::new(RefusalCode::ContentUnavailable, "schema reference")
        })?;
        let target = pointer_lookup(target_document, &pointer)?;
        stack.push(frame);
        apply(closure, &next_document, target, instance, stack)?;
        stack.pop();
    }
    if let Some(kind) = object.get("type") {
        check_type(kind, instance)?;
    }
    if let Some(expected) = object.get("const")
        && expected != instance
    {
        return Err(malformed("const"));
    }
    if let Some(values) = object.get("enum") {
        let values = values.as_array().ok_or_else(|| malformed("enum"))?;
        if !values.contains(instance) {
            return Err(malformed("enum"));
        }
    }
    if let Some(required) = object.get("required") {
        let required = required.as_array().ok_or_else(|| malformed("required"))?;
        let map = instance.as_object().ok_or_else(|| malformed("required"))?;
        for key in required {
            let key = key.as_str().ok_or_else(|| malformed("required"))?;
            if !map.contains_key(key) {
                return Err(malformed(format!("required {key}")));
            }
        }
    }
    if let Some(properties) = object.get("properties") {
        let properties = properties
            .as_object()
            .ok_or_else(|| malformed("properties"))?;
        if let Some(map) = instance.as_object() {
            for (key, property) in properties {
                if let Some(value) = map.get(key) {
                    apply(closure, document, property, value, stack)?;
                }
            }
            if let Some(additional) = object.get("additionalProperties") {
                for (key, value) in map {
                    if !properties.contains_key(key) {
                        apply(closure, document, additional, value, stack)?;
                    }
                }
            }
        }
    } else if let Some(additional) = object.get("additionalProperties")
        && let Some(map) = instance.as_object()
    {
        for value in map.values() {
            apply(closure, document, additional, value, stack)?;
        }
    }
    if object.get("additionalProperties") == Some(&Value::Bool(false))
        && let Some(map) = instance.as_object()
    {
        let known: BTreeSet<&String> = object
            .get("properties")
            .and_then(Value::as_object)
            .map(|properties| properties.keys().collect())
            .unwrap_or_default();
        if map.keys().any(|key| !known.contains(key)) {
            return Err(malformed("additional property"));
        }
    }
    if let Some(items) = object.get("prefixItems")
        && let Some(array) = instance.as_array()
    {
        let items = items.as_array().ok_or_else(|| malformed("prefixItems"))?;
        for (schema, value) in items.iter().zip(array.iter()) {
            apply(closure, document, schema, value, stack)?;
        }
    }
    if let Some(items) = object.get("items")
        && let Some(array) = instance.as_array()
    {
        let start = object
            .get("prefixItems")
            .and_then(Value::as_array)
            .map_or(0, Vec::len);
        for value in array.iter().skip(start) {
            apply(closure, document, items, value, stack)?;
        }
    }
    if let Some(minimum) = object.get("minItems") {
        let length = instance.as_array().map_or(0, Vec::len);
        if length < number_usize(minimum, "minItems")? {
            return Err(malformed("minItems"));
        }
    }
    if let Some(maximum) = object.get("maxItems") {
        let length = instance.as_array().map_or(0, Vec::len);
        if length > number_usize(maximum, "maxItems")? {
            return Err(malformed("maxItems"));
        }
    }
    if object.get("uniqueItems") == Some(&Value::Bool(true))
        && let Some(array) = instance.as_array()
    {
        let mut seen = BTreeSet::new();
        for item in array {
            if !seen.insert(jcs(item)?) {
                return Err(malformed("uniqueItems"));
            }
        }
    }
    if let Some(minimum) = object.get("minLength") {
        let length = instance.as_str().map_or(0, |text| text.chars().count());
        if length < number_usize(minimum, "minLength")? {
            return Err(malformed("minLength"));
        }
    }
    if let Some(maximum) = object.get("maxLength") {
        let length = instance.as_str().map_or(0, |text| text.chars().count());
        if length > number_usize(maximum, "maxLength")? {
            return Err(malformed("maxLength"));
        }
    }
    if let Some(minimum) = object.get("minProperties") {
        let length = instance.as_object().map_or(0, Map::len);
        if length < number_usize(minimum, "minProperties")? {
            return Err(malformed("minProperties"));
        }
    }
    if let Some(maximum) = object.get("maxProperties") {
        let length = instance.as_object().map_or(0, Map::len);
        if length > number_usize(maximum, "maxProperties")? {
            return Err(malformed("maxProperties"));
        }
    }
    check_numeric(object, instance)?;
    if let Some(list) = object.get("allOf") {
        for schema in list.as_array().ok_or_else(|| malformed("allOf"))? {
            apply(closure, document, schema, instance, stack)?;
        }
    }
    if let Some(list) = object.get("anyOf") {
        let list = list.as_array().ok_or_else(|| malformed("anyOf"))?;
        if !list
            .iter()
            .any(|schema| apply(closure, document, schema, instance, stack).is_ok())
        {
            return Err(malformed("anyOf"));
        }
    }
    if let Some(list) = object.get("oneOf") {
        let list = list.as_array().ok_or_else(|| malformed("oneOf"))?;
        let matches = list
            .iter()
            .filter(|schema| apply(closure, document, schema, instance, stack).is_ok())
            .count();
        if matches != 1 {
            return Err(malformed("oneOf"));
        }
    }
    if let Some(schema) = object.get("not")
        && apply(closure, document, schema, instance, stack).is_ok()
    {
        return Err(malformed("not"));
    }
    Ok(())
}

fn check_type(kind: &Value, instance: &Value) -> Result<(), ContractError> {
    let kinds: Vec<&str> = if let Some(one) = kind.as_str() {
        vec![one]
    } else if let Some(list) = kind.as_array() {
        list.iter()
            .map(|item| item.as_str().ok_or_else(|| malformed("type")))
            .collect::<Result<Vec<_>, _>>()?
    } else {
        return Err(malformed("type"));
    };
    for kind in &kinds {
        if !matches!(
            *kind,
            "null" | "boolean" | "object" | "array" | "string" | "number" | "integer"
        ) {
            return Err(ContractError::new(
                RefusalCode::UnsupportedFeature,
                "schema type",
            ));
        }
    }
    let matched = kinds.iter().any(|kind| match *kind {
        "null" => instance.is_null(),
        "boolean" => instance.is_boolean(),
        "object" => instance.is_object(),
        "array" => instance.is_array(),
        "string" => instance.is_string(),
        "number" => instance.is_number(),
        "integer" => is_integer(instance),
        _ => false,
    });
    if !matched {
        return Err(malformed("type"));
    }
    Ok(())
}

fn is_integer(value: &Value) -> bool {
    value.as_u64().is_some() || value.as_i64().is_some()
}

fn check_numeric(object: &Map<String, Value>, instance: &Value) -> Result<(), ContractError> {
    let Some(number) = instance.as_f64() else {
        return Ok(());
    };
    for key in ["minimum", "maximum", "exclusiveMinimum", "exclusiveMaximum"] {
        let Some(bound) = object.get(key) else {
            continue;
        };
        let bound = bound.as_f64().ok_or_else(|| malformed(key))?;
        let ok = match key {
            "minimum" => number >= bound,
            "maximum" => number <= bound,
            "exclusiveMinimum" => number > bound,
            "exclusiveMaximum" => number < bound,
            _ => true,
        };
        if !ok {
            return Err(malformed(key));
        }
    }
    Ok(())
}

fn number_usize(value: &Value, path: &str) -> Result<usize, ContractError> {
    let number = value.as_u64().ok_or_else(|| malformed(path))?;
    usize::try_from(number).map_err(|_| malformed(path))
}

fn resolve_ref(document: &str, reference: &str) -> Result<(String, String), ContractError> {
    if reference.starts_with("http://") || reference.starts_with("https://") {
        return Err(ContractError::new(
            RefusalCode::UnsupportedFeature,
            "remote schema reference",
        ));
    }
    if reference == "#" || reference.is_empty() {
        return Ok((document.to_owned(), String::new()));
    }
    if let Some(pointer) = reference.strip_prefix("#") {
        return Ok((document.to_owned(), pointer.to_owned()));
    }
    if let Some((digest, pointer)) = reference.split_once('#') {
        if !digest.starts_with("sha256:") {
            return Err(malformed("schema reference"));
        }
        return Ok((digest.to_owned(), pointer.to_owned()));
    }
    if reference.starts_with("sha256:") {
        return Ok((reference.to_owned(), String::new()));
    }
    Err(ContractError::new(
        RefusalCode::UnsupportedFeature,
        "schema reference",
    ))
}

fn pointer_lookup<'a>(document: &'a Value, pointer: &str) -> Result<&'a Value, ContractError> {
    if pointer.is_empty() || pointer == "/" {
        return Ok(document);
    }
    let mut current = document;
    for raw in pointer.trim_start_matches('/').split('/') {
        let token = raw.replace("~1", "/").replace("~0", "~");
        current = match current {
            Value::Object(map) => map.get(&token).ok_or_else(|| {
                ContractError::new(RefusalCode::ContentUnavailable, "schema pointer")
            })?,
            Value::Array(items) => {
                let index: usize = token.parse().map_err(|_| malformed("schema pointer"))?;
                items.get(index).ok_or_else(|| {
                    ContractError::new(RefusalCode::ContentUnavailable, "schema pointer")
                })?
            }
            _ => {
                return Err(ContractError::new(
                    RefusalCode::ContentUnavailable,
                    "schema pointer",
                ));
            }
        };
    }
    Ok(current)
}

fn malformed(detail: impl Into<String>) -> ContractError {
    ContractError::new(RefusalCode::Malformed, detail)
}
