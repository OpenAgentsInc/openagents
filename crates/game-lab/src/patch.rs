//! JSON Patch (RFC 6902): `add`, `remove`, `replace`, `test`, `move` and
//! `copy`, on a copy of the document. A patch that doesn't apply says why
//! in plain words.

use serde_json::Value;

/// A patch from a tool argument: a JSON array, or a string holding one
/// (the chat's model often sends it as text).
///
/// # Errors
/// The argument isn't a JSON Patch.
pub fn parse(argument: &Value) -> Result<Vec<Value>, String> {
    let value = match argument {
        Value::String(text) => {
            serde_json::from_str(text).map_err(|e| format!("The patch isn't valid JSON ({e})."))?
        }
        other => other.clone(),
    };
    match value {
        Value::Array(ops) if !ops.is_empty() => Ok(ops),
        Value::Object(_) => Ok(vec![value]),
        _ => Err("The patch must be a list of operations, like [{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":4.48}].".into()),
    }
}

fn pointer(path: &str) -> Result<Vec<String>, String> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let Some(rest) = path.strip_prefix('/') else {
        return Err(format!("The path {path} must start with /."));
    };
    Ok(rest
        .split('/')
        .map(|part| part.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn parent<'a>(doc: &'a mut Value, parts: &[String], path: &str) -> Result<&'a mut Value, String> {
    let mut at = doc;
    for part in parts {
        at = match at {
            Value::Object(map) => map.get_mut(part),
            Value::Array(list) => part.parse::<usize>().ok().and_then(|i| list.get_mut(i)),
            _ => None,
        }
        .ok_or_else(|| format!("The document has nothing at {path}."))?;
    }
    Ok(at)
}

fn add(doc: &mut Value, path: &str, value: Value) -> Result<(), String> {
    let parts = pointer(path)?;
    let Some((last, head)) = parts.split_last() else {
        *doc = value;
        return Ok(());
    };
    match parent(doc, head, path)? {
        Value::Object(map) => {
            map.insert(last.clone(), value);
            Ok(())
        }
        Value::Array(list) => {
            let i = if last == "-" {
                list.len()
            } else {
                last.parse::<usize>()
                    .ok()
                    .filter(|i| *i <= list.len())
                    .ok_or_else(|| format!("{path} isn't a place in the list."))?
            };
            list.insert(i, value);
            Ok(())
        }
        _ => Err(format!("{path} is inside a value, not a section.")),
    }
}

fn remove(doc: &mut Value, path: &str) -> Result<Value, String> {
    let parts = pointer(path)?;
    let Some((last, head)) = parts.split_last() else {
        return Err("The whole document can't be removed.".into());
    };
    match parent(doc, head, path)? {
        Value::Object(map) => map
            .remove(last)
            .ok_or_else(|| format!("The document has nothing at {path}.")),
        Value::Array(list) => last
            .parse::<usize>()
            .ok()
            .filter(|i| *i < list.len())
            .map(|i| list.remove(i))
            .ok_or_else(|| format!("The document has nothing at {path}.")),
        _ => Err(format!("The document has nothing at {path}.")),
    }
}

fn get(doc: &Value, path: &str) -> Result<Value, String> {
    let parts = pointer(path)?;
    let mut at = doc;
    for part in &parts {
        at = match at {
            Value::Object(map) => map.get(part),
            Value::Array(list) => part.parse::<usize>().ok().and_then(|i| list.get(i)),
            _ => None,
        }
        .ok_or_else(|| format!("The document has nothing at {path}."))?;
    }
    Ok(at.clone())
}

/// Applies `ops` to a copy of `doc`.
///
/// # Errors
/// An operation that doesn't apply, in plain words.
pub fn apply(doc: &Value, ops: &[Value]) -> Result<Value, String> {
    let mut out = doc.clone();
    for op in ops {
        let path = op["path"]
            .as_str()
            .ok_or("Each operation needs a path, like /man/man_chase.")?;
        let value = || {
            op.get("value")
                .cloned()
                .ok_or_else(|| format!("The operation on {path} needs a value."))
        };
        match op["op"].as_str() {
            Some("add") => add(&mut out, path, value()?)?,
            Some("remove") => {
                remove(&mut out, path)?;
            }
            Some("replace") => {
                get(&out, path)?;
                remove(&mut out, path)?;
                add(&mut out, path, value()?)?;
            }
            Some("test") => {
                if get(&out, path)? != value()? {
                    return Err(format!("{path} isn't what the patch expected."));
                }
            }
            Some(kind @ ("move" | "copy")) => {
                let from = op["from"]
                    .as_str()
                    .ok_or_else(|| format!("The {kind} to {path} needs a from."))?;
                let moved = if kind == "move" {
                    remove(&mut out, from)?
                } else {
                    get(&out, from)?
                };
                add(&mut out, path, moved)?;
            }
            _ => {
                return Err(
                    "Each operation's op must be add, remove, replace, test, move or copy.".into(),
                );
            }
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replace_add_and_test_apply_in_order() {
        let doc = json!({"man": {"man_chase": 5.6}, "layout": {"seed": 7}});
        let ops = parse(&json!(
            "[{\"op\":\"test\",\"path\":\"/man/man_chase\",\"value\":5.6},{\"op\":\"replace\",\"path\":\"/man/man_chase\",\"value\":4.48},{\"op\":\"add\",\"path\":\"/layout/extra_closets\",\"value\":2}]"
        ))
        .unwrap();
        let out = apply(&doc, &ops).unwrap();
        assert_eq!(out["man"]["man_chase"], 4.48);
        assert_eq!(out["layout"]["extra_closets"], 2);
        assert_eq!(doc["man"]["man_chase"], 5.6, "the original is untouched");
    }

    #[test]
    fn a_patch_that_doesnt_apply_says_why() {
        let doc = json!({"man": {"man_chase": 5.6}});
        let missing = apply(
            &doc,
            &[json!({"op": "replace", "path": "/man/speed", "value": 1})],
        );
        assert!(missing.unwrap_err().contains("nothing at /man/speed"));
        assert!(parse(&json!("not json")).is_err());
        assert!(parse(&json!(3)).is_err());
    }
}
