//! Schemas as the reference shows them: a table of fields with their types
//! and descriptions, and an example value built from the schema.

use serde_json::{Map, Value, json};

use super::contract::{deref, ref_name};

/// One row of a field table.
pub(crate) struct Field {
    /// The dotted path, such as `items[].id`.
    pub name: String,
    pub kind: String,
    pub required: bool,
    pub description: String,
}

/// The most rows one table shows.
pub(crate) const MAX_FIELDS: usize = 120;
const MAX_DEPTH: usize = 5;

/// The type of `schema` in a few words: `string (uri)`, `array of AppComputer`,
/// `one of: "a", "b"`.
pub(crate) fn kind(document: &Value, schema: &Value) -> String {
    if let Some(name) = ref_name(schema) {
        let target = deref(document, schema);
        let base = simple(document, target);
        return if base.is_empty() || base == "object" {
            name.to_owned()
        } else {
            format!("{name} ({base})")
        };
    }
    if schema["$ref"].is_string() {
        return "object".to_owned();
    }
    simple(document, schema)
}

fn simple(document: &Value, schema: &Value) -> String {
    if let Some(value) = schema.get("const") {
        return format!("always {value}");
    }
    if let Some(values) = schema["enum"].as_array() {
        let names: Vec<String> = values.iter().take(12).map(Value::to_string).collect();
        let more = if values.len() > 12 { ", ..." } else { "" };
        return format!("one of {}{more}", names.join(", "));
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(options) = schema[key].as_array() {
            let mut names: Vec<String> = options.iter().map(|o| kind(document, o)).collect();
            names.dedup();
            return names.join(" or ");
        }
    }
    if let Some(parts) = schema["allOf"].as_array() {
        let names: Vec<String> = parts.iter().map(|o| kind(document, o)).collect();
        return names.join(" and ");
    }
    let types: Vec<&str> = match &schema["type"] {
        Value::String(t) => vec![t.as_str()],
        Value::Array(list) => list.iter().filter_map(Value::as_str).collect(),
        _ if schema["properties"].is_object() => vec!["object"],
        _ => Vec::new(),
    };
    let mut words: Vec<String> = Vec::new();
    for t in &types {
        let word = match *t {
            "array" => format!("array of {}", kind(document, &schema["items"])),
            "null" => "null".to_owned(),
            other => match schema["format"].as_str() {
                Some(format) => format!("{other} ({format})"),
                None => other.to_owned(),
            },
        };
        words.push(word);
    }
    if words.is_empty() {
        return "any".to_owned();
    }
    words.join(" or ")
}

/// The fields of `schema`, nested ones by dotted path.
pub(crate) fn fields(document: &Value, schema: &Value) -> Vec<Field> {
    let mut out = Vec::new();
    walk(document, schema, "", 0, &mut Vec::new(), &mut out);
    out
}

fn properties<'a>(document: &'a Value, schema: &'a Value) -> Vec<(&'a str, &'a Value, bool)> {
    let schema = deref(document, schema);
    let mut out = Vec::new();
    let required: Vec<&str> = schema["required"]
        .as_array()
        .map(|r| r.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default();
    if let Some(props) = schema["properties"].as_object() {
        for (name, value) in props {
            out.push((name.as_str(), value, required.contains(&name.as_str())));
        }
    }
    if let Some(parts) = schema["allOf"].as_array() {
        for part in parts {
            out.extend(properties(document, part));
        }
    }
    out
}

fn walk(
    document: &Value,
    schema: &Value,
    prefix: &str,
    depth: usize,
    stack: &mut Vec<String>,
    out: &mut Vec<Field>,
) {
    if depth > MAX_DEPTH || out.len() >= MAX_FIELDS {
        return;
    }
    if let Some(name) = ref_name(schema) {
        if stack.iter().any(|s| s == name) {
            return;
        }
        stack.push(name.to_owned());
        walk(document, deref(document, schema), prefix, depth, stack, out);
        stack.pop();
        return;
    }
    if schema["type"] == "array" || schema["items"].is_object() {
        walk(
            document,
            &schema["items"],
            &format!("{prefix}[]"),
            depth,
            stack,
            out,
        );
        return;
    }
    for (name, value, required) in properties(document, schema) {
        if out.len() >= MAX_FIELDS {
            return;
        }
        let path = if prefix.is_empty() {
            name.to_owned()
        } else {
            format!("{prefix}.{name}")
        };
        let target = deref(document, value);
        let mut description = value["description"]
            .as_str()
            .or_else(|| target["description"].as_str())
            .unwrap_or_default()
            .trim()
            .to_owned();
        if let Some(default) = value.get("default").or_else(|| target.get("default")) {
            if !description.is_empty() {
                description.push(' ');
            }
            description.push_str(&format!("Default: `{default}`."));
        }
        out.push(Field {
            name: path.clone(),
            kind: kind(document, value),
            required,
            description,
        });
        walk(document, value, &path, depth + 1, stack, out);
    }
}

/// An example of `schema`: its own example, else its required fields
/// filled with placeholders.
pub(crate) fn example(document: &Value, schema: &Value) -> Value {
    sample(document, schema, 0, &mut Vec::new())
}

fn sample(document: &Value, schema: &Value, depth: usize, stack: &mut Vec<String>) -> Value {
    if depth > 6 {
        return json!({});
    }
    if let Some(name) = ref_name(schema) {
        if stack.iter().any(|s| s == name) {
            return json!({});
        }
        stack.push(name.to_owned());
        let value = sample(document, deref(document, schema), depth, stack);
        stack.pop();
        return value;
    }
    if let Some(value) = schema.get("example") {
        return value.clone();
    }
    if let Some(value) = schema["examples"].as_array().and_then(|e| e.first()) {
        return value.clone();
    }
    for key in ["const", "default"] {
        if let Some(value) = schema.get(key) {
            return value.clone();
        }
    }
    if let Some(value) = schema["enum"].as_array().and_then(|e| e.first()) {
        return value.clone();
    }
    for key in ["oneOf", "anyOf"] {
        if let Some(first) = schema[key].as_array().and_then(|o| {
            o.iter()
                .find(|option| option["type"] != "null")
                .or_else(|| o.first())
        }) {
            return sample(document, first, depth + 1, stack);
        }
    }
    let kind = match &schema["type"] {
        Value::String(t) => t.as_str(),
        Value::Array(list) => list
            .iter()
            .filter_map(Value::as_str)
            .find(|t| *t != "null")
            .unwrap_or("null"),
        _ if schema["properties"].is_object() || schema["allOf"].is_array() => "object",
        _ => "",
    };
    match kind {
        "object" => {
            let mut object = Map::new();
            let all = properties(document, schema);
            let any_required = all.iter().any(|(_, _, required)| *required);
            for (name, value, required) in all {
                if required || !any_required {
                    object.insert(name.to_owned(), sample(document, value, depth + 1, stack));
                }
            }
            Value::Object(object)
        }
        "array" => json!([sample(document, &schema["items"], depth + 1, stack)]),
        "string" => match schema["format"].as_str() {
            Some("uri") => json!("https://example.com"),
            Some("date-time") => json!("2026-10-17T12:00:00Z"),
            Some("uuid") => json!("00000000-0000-4000-8000-000000000000"),
            Some("binary") => json!("<bytes>"),
            _ => json!("string"),
        },
        "integer" => json!(schema["minimum"].as_i64().unwrap_or(0).max(0)),
        "number" => json!(schema["minimum"].as_f64().unwrap_or(0.0).max(0.0)),
        "boolean" => json!(false),
        "null" => Value::Null,
        _ => json!({}),
    }
}
