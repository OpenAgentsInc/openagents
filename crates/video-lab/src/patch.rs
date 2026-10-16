//! JSON Patch for Video Lab documents (a port of `lab.py apply_patch`):
//! RFC 6902 operations, `@name` picking the list item with that name, and
//! refusals worded exactly as the Lab has always worded them.

use lab_kit::Refusal;
use serde_json::{Value, json};

fn refuse(message: impl Into<String>) -> Refusal {
    Refusal::new("patch", message)
}

/// Python's `json.dumps` (spaces after `,` and `:`), for messages.
pub fn dumps(v: &Value) -> String {
    match v {
        Value::Object(map) => format!(
            "{{{}}}",
            map.iter()
                .map(|(k, v)| format!("{}: {}", Value::String(k.clone()), dumps(v)))
                .collect::<Vec<_>>()
                .join(", ")
        ),
        Value::Array(items) => format!(
            "[{}]",
            items.iter().map(dumps).collect::<Vec<_>>().join(", ")
        ),
        Value::String(s) => {
            // json.dumps escapes non-ASCII by default.
            let mut out = String::from("\"");
            for c in s.chars() {
                match c {
                    '"' => out.push_str("\\\""),
                    '\\' => out.push_str("\\\\"),
                    '\n' => out.push_str("\\n"),
                    '\r' => out.push_str("\\r"),
                    '\t' => out.push_str("\\t"),
                    c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                        let mut buf = [0u16; 2];
                        for unit in c.encode_utf16(&mut buf) {
                            out.push_str(&format!("\\u{unit:04x}"));
                        }
                    }
                    c => out.push(c),
                }
            }
            out.push('"');
            out
        }
        Value::Number(n) => match n.as_f64() {
            Some(f) if n.is_f64() => {
                let s = format!("{f:?}");
                s
            }
            _ => n.to_string(),
        },
        other => other.to_string(),
    }
}

fn is_digit(p: &str) -> bool {
    !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit())
}

/// A JSON Pointer as parts, with `@name` resolved to an index.
pub fn pointer(doc: &Value, path: &str) -> Result<Vec<String>, Refusal> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(refuse(format!(
            "'{path}' isn't a JSON Pointer; it starts with /, like /shots/@s4-delegation/dur"
        )));
    };
    let mut out = Vec::new();
    let mut cur = Some(doc);
    for raw in rest.split('/') {
        let mut p = raw.replace("~1", "/").replace("~0", "~");
        if let (Some(Value::Array(items)), Some(name)) = (cur, p.strip_prefix('@')) {
            match items
                .iter()
                .position(|x| x.get("name").and_then(Value::as_str) == Some(name))
            {
                Some(i) => p = i.to_string(),
                None => {
                    let names: Vec<String> = items
                        .iter()
                        .filter(|x| x.is_object())
                        .map(|x| match x.get("name") {
                            Some(Value::String(s)) => s.clone(),
                            Some(other) => crate::fmt::py(other),
                            None => "None".into(),
                        })
                        .collect();
                    return Err(refuse(format!(
                        "there's no shot called {name}; the shots are {}",
                        names.join(", ")
                    )));
                }
            }
        }
        cur = match cur {
            Some(Value::Array(items)) => {
                if is_digit(&p) {
                    p.parse::<usize>().ok().and_then(|i| items.get(i))
                } else {
                    None
                }
            }
            Some(Value::Object(map)) => map.get(&p),
            _ => None,
        };
        out.push(p);
    }
    Ok(out)
}

fn joined(parts: &[String]) -> String {
    format!("/{}", parts.join("/"))
}

fn walk<'a>(data: &'a mut Value, parts: &[String]) -> Result<&'a mut Value, Refusal> {
    let all = joined(parts);
    let mut cur = data;
    for p in &parts[..parts.len().saturating_sub(1)] {
        cur = match cur {
            Value::Array(items) => {
                let len = items.len();
                match p.parse::<usize>().ok().filter(|i| is_digit(p) && *i < len) {
                    Some(i) => &mut items[i],
                    None => return Err(refuse(format!("there's no item {p} at {all}"))),
                }
            }
            Value::Object(map) => match map.get_mut(p) {
                Some(v) => v,
                None => return Err(refuse(format!("{all}: '{p}' isn't there"))),
            },
            _ => {
                return Err(refuse(format!(
                    "{all} goes inside a value that has no fields"
                )));
            }
        };
    }
    Ok(cur)
}

fn get(data: &mut Value, parts: &[String]) -> Result<Value, Refusal> {
    if parts.is_empty() {
        return Ok(data.clone());
    }
    let all = joined(parts);
    let last = parts.last().unwrap();
    let parent = walk(data, parts)?;
    match parent {
        Value::Array(items) => match last
            .parse::<usize>()
            .ok()
            .filter(|i| is_digit(last) && *i < items.len())
        {
            Some(i) => Ok(items[i].clone()),
            None => Err(refuse(format!("there's no item {last} at {all}"))),
        },
        Value::Object(map) => map
            .get(last)
            .cloned()
            .ok_or_else(|| refuse(format!("{all} isn't set"))),
        // Python's `last not in parent` on a scalar raises; the Lab reports it the same way.
        _ => Err(refuse(format!("{all} isn't set"))),
    }
}

/// Parse what the chat passed: a JSON string, one operation, or a list.
pub fn operations(ops: &Value) -> Result<Vec<Value>, Refusal> {
    let parsed;
    let ops = match ops {
        Value::String(text) => {
            parsed = serde_json::from_str::<Value>(text)
                .map_err(|e| refuse(format!("the patch isn't JSON ({e})")))?;
            &parsed
        }
        other => other,
    };
    let list = match ops {
        Value::Object(_) => vec![ops.clone()],
        Value::Array(items) if !items.is_empty() => items.clone(),
        _ => {
            return Err(refuse(
                "the patch is a list of operations, like [{\"op\": \"replace\", \"path\": \"/shots/@s4-delegation/dur\", \"value\": 12.5}]",
            ));
        }
    };
    Ok(list)
}

pub fn apply_patch(data: &Value, ops: &Value) -> Result<Value, Refusal> {
    let ops = operations(ops)?;
    let mut out = data.clone();
    for op in ops {
        let name = op.get("op").and_then(Value::as_str).unwrap_or_default();
        if !op.is_object() || !["add", "remove", "replace", "move", "copy", "test"].contains(&name)
        {
            let text: String = dumps(&op).chars().take(120).collect();
            return Err(refuse(format!(
                "{text} isn't a patch operation (add, remove, replace, move, copy or test)"
            )));
        }
        let path = op
            .get("path")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        let mut parts = pointer(&out, &path)?;
        if ["add", "replace", "test"].contains(&name) && op.get("value").is_none() {
            let shown = op
                .get("path")
                .map(crate::fmt::py)
                .unwrap_or_else(|| "None".into());
            return Err(refuse(format!("{name} {shown} needs a value")));
        }
        if name == "test" {
            let found = get(&mut out, &parts)?;
            if found != op["value"] {
                return Err(refuse(format!(
                    "{path} is {}, not {}",
                    dumps(&found),
                    dumps(&op["value"])
                )));
            }
            continue;
        }
        let mut kind = name.to_owned();
        let mut value = op.get("value").cloned().unwrap_or(Value::Null);
        if name == "move" || name == "copy" {
            let from = op
                .get("from")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let from_parts = pointer(&out, &from)?;
            value = get(&mut out, &from_parts)?;
            if name == "move" {
                let src = pointer(&out, &from)?;
                let last = src.last().cloned().unwrap_or_default();
                let parent = walk(&mut out, &src)?;
                match parent {
                    Value::Array(items) => {
                        if let Ok(i) = last.parse::<usize>() {
                            if i < items.len() {
                                items.remove(i);
                            }
                        }
                    }
                    Value::Object(map) => {
                        map.shift_remove(&last);
                    }
                    _ => {}
                }
                parts = pointer(&out, &path)?;
            }
            kind = "add".into();
        }
        if parts.is_empty() {
            return Err(refuse("a patch can't replace the whole document"));
        }
        let last = parts.last().cloned().unwrap();
        let parent = walk(&mut out, &parts)?;
        match parent {
            Value::Array(items) => {
                if kind == "add" {
                    let idx = if last == "-" {
                        Some(items.len())
                    } else if is_digit(&last) {
                        last.parse().ok()
                    } else {
                        None
                    };
                    match idx.filter(|i| *i <= items.len()) {
                        Some(i) => items.insert(i, value),
                        None => return Err(refuse(format!("can't add at {path}"))),
                    }
                } else {
                    let i = last
                        .parse::<usize>()
                        .ok()
                        .filter(|i| is_digit(&last) && *i < items.len());
                    let Some(i) = i else {
                        return Err(refuse(format!("there's no item {last} at {path}")));
                    };
                    if kind == "remove" {
                        items.remove(i);
                    } else {
                        items[i] = value;
                    }
                }
            }
            Value::Object(map) => {
                if kind == "add" {
                    map.insert(last, value);
                } else if !map.contains_key(&last) {
                    let verb = if kind == "remove" {
                        "removed"
                    } else {
                        "replaced"
                    };
                    let hint = if kind == "replace" { "; use add" } else { "" };
                    return Err(refuse(format!(
                        "{path} isn't set, so it can't be {verb}{hint}"
                    )));
                } else if kind == "remove" {
                    map.shift_remove(&last);
                } else {
                    map.insert(last, value);
                }
            }
            _ => {
                return Err(refuse(format!(
                    "{path} goes inside a value that has no fields"
                )));
            }
        }
    }
    let _ = json!(null);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn cut() -> Value {
        json!({"shots": [{"name": "s1", "dur": 4}, {"name": "s4-delegation", "dur": 10}], "captions": []})
    }

    #[test]
    fn named_shots_and_plain_words() {
        let out = apply_patch(
            &cut(),
            &json!("[{\"op\":\"replace\",\"path\":\"/shots/@s4-delegation/dur\",\"value\":12.5}]"),
        )
        .unwrap();
        assert_eq!(out["shots"][1]["dur"], 12.5);
        let cases = [
            (
                json!({"op": "replace", "path": "/shots/@nope/dur", "value": 1}),
                "there's no shot called nope; the shots are s1, s4-delegation",
            ),
            (
                json!({"op": "replace", "path": "shots/0/dur", "value": 1}),
                "'shots/0/dur' isn't a JSON Pointer; it starts with /, like /shots/@s4-delegation/dur",
            ),
            (
                json!({"op": "replace", "path": "/shots/0/play", "value": 1}),
                "/shots/0/play isn't set, so it can't be replaced; use add",
            ),
            (
                json!({"op": "remove", "path": "/shots/0/play"}),
                "/shots/0/play isn't set, so it can't be removed",
            ),
            (
                json!({"op": "replace", "path": "/shots/0/dur"}),
                "replace /shots/0/dur needs a value",
            ),
            (
                json!({"op": "test", "path": "/shots/0/dur", "value": 5}),
                "/shots/0/dur is 4, not 5",
            ),
            (
                json!({"op": "add", "path": "/shots/9", "value": {}}),
                "can't add at /shots/9",
            ),
            (
                json!({"op": "remove", "path": "/shots/9/dur"}),
                "there's no item 9 at /shots/9/dur",
            ),
            (
                json!({"op": "replace", "path": "", "value": {}}),
                "a patch can't replace the whole document",
            ),
            (
                json!({"op": "frob", "path": "/x"}),
                "{\"op\": \"frob\", \"path\": \"/x\"} isn't a patch operation (add, remove, replace, move, copy or test)",
            ),
        ];
        for (op, want) in cases {
            assert_eq!(apply_patch(&cut(), &op).unwrap_err().message, want);
        }
        assert_eq!(
            apply_patch(&cut(), &json!([])).unwrap_err().message,
            "the patch is a list of operations, like [{\"op\": \"replace\", \"path\": \"/shots/@s4-delegation/dur\", \"value\": 12.5}]"
        );
        assert!(
            apply_patch(&cut(), &json!("not json"))
                .unwrap_err()
                .message
                .starts_with("the patch isn't JSON (")
        );
    }

    #[test]
    fn move_and_copy_by_name() {
        let out = apply_patch(
            &cut(),
            &json!([{"op": "move", "from": "/shots/@s4-delegation", "path": "/shots/0"}]),
        )
        .unwrap();
        assert_eq!(out["shots"][0]["name"], "s4-delegation");
        let out = apply_patch(
            &cut(),
            &json!([{"op": "copy", "from": "/shots/@s1", "path": "/shots/-"}]),
        )
        .unwrap();
        assert_eq!(out["shots"].as_array().unwrap().len(), 3);
        // Added keys go last, as in the Lab's documents.
        let out = apply_patch(
            &json!({"b": 1, "a": 2}),
            &json!({"op": "add", "path": "/c", "value": 3}),
        )
        .unwrap();
        assert_eq!(
            out.as_object().unwrap().keys().collect::<Vec<_>>(),
            ["b", "a", "c"]
        );
    }
}
