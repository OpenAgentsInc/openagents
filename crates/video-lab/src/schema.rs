//! The three document schemas and a small JSON Schema subset with the
//! Lab's plain reasons (a port of `lab.py validate`).

use serde_json::Value;

use crate::fmt::py;

pub const KINDS: [&str; 3] = ["cut.v1", "shot.v1", "scene.v1"];

pub fn schema(kind: &str) -> Option<Value> {
    let text = match kind {
        "cut.v1" => include_str!("../../../docs/video-lab/schemas/cut.v1.json"),
        "shot.v1" => include_str!("../../../docs/video-lab/schemas/shot.v1.json"),
        "scene.v1" => include_str!("../../../docs/video-lab/schemas/scene.v1.json"),
        _ => return None,
    };
    serde_json::from_str(text).ok()
}

fn place(path: &str) -> &str {
    if path.is_empty() {
        "the document"
    } else {
        path
    }
}

fn is_number(v: &Value) -> bool {
    v.is_number()
}

fn is_int(v: &Value) -> bool {
    v.as_i64().is_some() || v.as_u64().is_some()
}

pub fn validate(value: &Value, sch: &Value, root: &Value, path: &str) -> Vec<String> {
    let resolved;
    let sch = match sch.get("$ref").and_then(Value::as_str) {
        Some(r) => {
            let name = r.rsplit('/').next().unwrap_or_default();
            resolved = root["$defs"][name].clone();
            &resolved
        }
        None => sch,
    };
    let mut out = Vec::new();
    let w = place(path);
    match sch.get("type").and_then(Value::as_str) {
        Some("number") if !is_number(value) => return vec![format!("{w} must be a number")],
        Some("integer") if !is_int(value) => return vec![format!("{w} must be a whole number")],
        Some(t @ ("object" | "array" | "string" | "boolean")) => {
            let ok = match t {
                "object" => value.is_object(),
                "array" => value.is_array(),
                "string" => value.is_string(),
                _ => value.is_boolean(),
            };
            if !ok {
                let article = if t.starts_with(['a', 'e', 'i', 'o', 'u']) {
                    "an"
                } else {
                    "a"
                };
                return vec![format!("{w} must be {article} {t}")];
            }
        }
        _ => {}
    }
    if let Some(choices) = sch.get("enum").and_then(Value::as_array)
        && !choices.contains(value)
    {
        out.push(format!(
            "{w} must be one of {}",
            choices.iter().map(py).collect::<Vec<_>>().join(", ")
        ));
    }
    if let Some(x) = value.as_f64().filter(|_| value.is_number()) {
        if let Some(min) = sch
            .get("minimum")
            .filter(|m| x < m.as_f64().unwrap_or(f64::MIN))
        {
            out.push(format!("{w} must be at least {}", py(min)));
        }
        if let Some(min) = sch
            .get("exclusiveMinimum")
            .filter(|m| x <= m.as_f64().unwrap_or(f64::MIN))
        {
            out.push(format!("{w} must be more than {}", py(min)));
        }
        if let Some(max) = sch
            .get("maximum")
            .filter(|m| x > m.as_f64().unwrap_or(f64::MAX))
        {
            out.push(format!("{w} must be at most {}", py(max)));
        }
    }
    if let Some(s) = value.as_str()
        && (s.chars().count() as u64) < sch.get("minLength").and_then(Value::as_u64).unwrap_or(0)
    {
        out.push(format!("{w} can't be empty"));
    }
    if let Some(items) = value.as_array() {
        let min = sch.get("minItems").and_then(Value::as_u64).unwrap_or(0);
        if (items.len() as u64) < min {
            out.push(format!("{w} needs at least {min} item(s)"));
        }
        if let Some(max) = sch.get("maxItems").and_then(Value::as_u64)
            && items.len() as u64 > max
        {
            out.push(format!("{w} takes at most {max} items"));
        }
        if let Some(item_schema) = sch.get("items") {
            for (i, item) in items.iter().enumerate() {
                let label = item.get("name").and_then(Value::as_str);
                let at =
                    format!("{path}[{i}]") + &label.map(|l| format!(" ({l})")).unwrap_or_default();
                out.extend(validate(item, item_schema, root, &at));
            }
        }
    }
    if let Some(object) = value.as_object() {
        for key in sch
            .get("required")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            if !object.contains_key(key) {
                out.push(format!("{w} needs '{key}'"));
            }
        }
        let empty = serde_json::Map::new();
        let props = sch
            .get("properties")
            .and_then(Value::as_object)
            .unwrap_or(&empty);
        for (key, v) in object {
            if let Some(p) = props.get(key) {
                let at = if path.is_empty() {
                    key.clone()
                } else {
                    format!("{path}.{key}")
                };
                out.extend(validate(v, p, root, &at));
            } else if sch.get("additionalProperties") == Some(&Value::Bool(false)) {
                let prefix: String = key.chars().take(2).collect();
                let near = props
                    .keys()
                    .find(|k| k.starts_with(&prefix) || key.starts_with(k.as_str()));
                let hint = near
                    .map(|n| format!(" (did you mean {n}?)"))
                    .unwrap_or_default();
                out.push(format!("{w} has '{key}', which it doesn't take{hint}"));
            } else if let Some(extra) = sch.get("additionalProperties").filter(|a| a.is_object()) {
                out.extend(validate(v, extra, root, &format!("{path}.{key}")));
            }
        }
        if let Some(alts) = sch.get("anyOf").and_then(Value::as_array) {
            let fits = alts.iter().any(|alt| {
                alt.get("required")
                    .and_then(Value::as_array)
                    .into_iter()
                    .flatten()
                    .filter_map(Value::as_str)
                    .all(|k| object.contains_key(k))
            });
            if !fits {
                let keys: Vec<String> = alts
                    .iter()
                    .map(|alt| {
                        let req: Vec<&str> = alt
                            .get("required")
                            .and_then(Value::as_array)
                            .into_iter()
                            .flatten()
                            .filter_map(Value::as_str)
                            .collect();
                        format!("'{}'", req.join("', '"))
                    })
                    .collect();
                out.push(format!("{w} needs {}", keys.join(" or ")));
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn the_schemas_load() {
        for kind in KINDS {
            assert!(schema(kind).is_some(), "{kind}");
        }
    }

    #[test]
    fn reasons_read_as_the_lab_writes_them() {
        let s = schema("cut.v1").unwrap();
        let mut doc = json!({"shots": [{"name": "s1", "src": "~/a.mp4", "duration": 4}]});
        let found = validate(&doc, &s, &s, "");
        assert!(
            found
                .iter()
                .any(|p| p.starts_with("shots[0] (s1) has 'duration', which it doesn't take")),
            "{found:?}"
        );
        assert!(
            found.iter().any(|p| p == "shots[0] (s1) needs 'dur'"),
            "{found:?}"
        );
        doc["shots"][0] = json!({"name": "s1", "src": "~/a.mp4", "dur": "four"});
        let found = validate(&doc, &s, &s, "");
        assert!(
            found.contains(&"shots[0] (s1).dur must be a number".to_owned()),
            "{found:?}"
        );
    }
}
