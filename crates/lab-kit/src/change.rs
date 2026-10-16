//! Changes: a JSON Patch (RFC 6902) or a kind's typed intent.
//!
//! Every edit, from a chat tool, a timeline drag or a phone button, is one
//! [`Change`] given to the kind's `apply`, so it is checked once, in one place.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::Refusal;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Change {
    Patch {
        ops: Vec<PatchOp>,
    },
    /// A kind-defined intent, validated by that kind (`{"intent": "move", ...}`).
    Intent {
        intent: Value,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case", deny_unknown_fields)]
pub enum PatchOp {
    Add { path: String, value: Value },
    Remove { path: String },
    Replace { path: String, value: Value },
    Move { from: String, path: String },
    Copy { from: String, path: String },
    Test { path: String, value: Value },
}

/// How a kind names places that plain JSON Pointers can't name stably, such
/// as Video's `/shots/@s4-delegation/dur`. It turns one path segment into
/// an array index (or `None` when nothing matches).
pub trait Addressing {
    fn resolve(&self, parent: &Value, segment: &str) -> Option<String>;
}

/// Plain RFC 6901 pointers only.
pub struct Plain;

impl Addressing for Plain {
    fn resolve(&self, _parent: &Value, _segment: &str) -> Option<String> {
        None
    }
}

/// Apply a patch atomically: on any failure the document is unchanged and
/// the refusal names the operation.
pub fn apply_patch(
    doc: &Value,
    ops: &[PatchOp],
    addressing: &dyn Addressing,
) -> Result<Value, Refusal> {
    let mut out = doc.clone();
    for (i, op) in ops.iter().enumerate() {
        apply_one(&mut out, op, addressing).map_err(|why| {
            Refusal::new(
                "patch",
                format!(
                    "Change {} of {} couldn't be applied: {why}.",
                    i + 1,
                    ops.len()
                ),
            )
        })?;
    }
    Ok(out)
}

fn apply_one(doc: &mut Value, op: &PatchOp, addressing: &dyn Addressing) -> Result<(), String> {
    match op {
        PatchOp::Add { path, value } => add(doc, path, value.clone(), addressing),
        PatchOp::Remove { path } => remove(doc, path, addressing).map(|_| ()),
        PatchOp::Replace { path, value } => {
            let target = pointer_mut(doc, path, addressing)?;
            *target = value.clone();
            Ok(())
        }
        PatchOp::Move { from, path } => {
            if path.starts_with(&format!("{from}/")) {
                return Err(format!("{from} can't move inside itself"));
            }
            let value = remove(doc, from, addressing)?;
            add(doc, path, value, addressing)
        }
        PatchOp::Copy { from, path } => {
            let value = pointer_mut(doc, from, addressing)?.clone();
            add(doc, path, value, addressing)
        }
        PatchOp::Test { path, value } => {
            if pointer_mut(doc, path, addressing)? == value {
                Ok(())
            } else {
                Err(format!("{path} doesn't hold the expected value"))
            }
        }
    }
}

fn segments(path: &str) -> Result<Vec<String>, String> {
    if path.is_empty() {
        return Ok(Vec::new());
    }
    let rest = path
        .strip_prefix('/')
        .ok_or_else(|| format!("{path} isn't a path (start it with /)"))?;
    Ok(rest
        .split('/')
        .map(|s| s.replace("~1", "/").replace("~0", "~"))
        .collect())
}

fn index(
    parent: &Value,
    segment: &str,
    addressing: &dyn Addressing,
    allow_end: bool,
) -> Result<String, String> {
    if let Some(resolved) = addressing.resolve(parent, segment) {
        return Ok(resolved);
    }
    if segment.starts_with('@') {
        return Err(format!("nothing is named {}", &segment[1..]));
    }
    if allow_end && segment == "-" {
        return Ok(segment.to_owned());
    }
    Ok(segment.to_owned())
}

fn pointer_mut<'a>(
    doc: &'a mut Value,
    path: &str,
    addressing: &dyn Addressing,
) -> Result<&'a mut Value, String> {
    let mut at = doc;
    for segment in segments(path)? {
        let key = index(at, &segment, addressing, false)?;
        at = match at {
            Value::Object(map) => map
                .get_mut(&key)
                .ok_or_else(|| format!("{path} doesn't exist"))?,
            Value::Array(items) => {
                let i: usize = key.parse().map_err(|_| format!("{path} doesn't exist"))?;
                items
                    .get_mut(i)
                    .ok_or_else(|| format!("{path} doesn't exist"))?
            }
            _ => return Err(format!("{path} doesn't exist")),
        };
    }
    Ok(at)
}

fn split_last(path: &str) -> Result<(String, String), String> {
    let mut parts = segments(path)?;
    let last = parts
        .pop()
        .ok_or("the whole document can't be added or removed")?;
    let parent = parts
        .iter()
        .map(|s| format!("/{}", s.replace('~', "~0").replace('/', "~1")))
        .collect();
    Ok((parent, last))
}

fn add(
    doc: &mut Value,
    path: &str,
    value: Value,
    addressing: &dyn Addressing,
) -> Result<(), String> {
    if path.is_empty() {
        *doc = value;
        return Ok(());
    }
    let (parent_path, last) = split_last(path)?;
    let parent = pointer_mut(doc, &parent_path, addressing)?;
    let key = index(parent, &last, addressing, true)?;
    match parent {
        Value::Object(map) => {
            map.insert(key, value);
            Ok(())
        }
        Value::Array(items) => {
            let i = if key == "-" {
                items.len()
            } else {
                key.parse()
                    .map_err(|_| format!("{path} isn't a place in a list"))?
            };
            if i > items.len() {
                return Err(format!("{path} is past the end of the list"));
            }
            items.insert(i, value);
            Ok(())
        }
        _ => Err(format!("{path} isn't inside an object or list")),
    }
}

fn remove(doc: &mut Value, path: &str, addressing: &dyn Addressing) -> Result<Value, String> {
    let (parent_path, last) = split_last(path)?;
    let parent = pointer_mut(doc, &parent_path, addressing)?;
    let key = index(parent, &last, addressing, false)?;
    match parent {
        Value::Object(map) => map
            .remove(&key)
            .ok_or_else(|| format!("{path} doesn't exist")),
        Value::Array(items) => {
            let i: usize = key.parse().map_err(|_| format!("{path} doesn't exist"))?;
            (i < items.len())
                .then(|| items.remove(i))
                .ok_or_else(|| format!("{path} doesn't exist"))
        }
        _ => Err(format!("{path} doesn't exist")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Video's naming: `@name` is the item of a list whose `name` matches.
    struct ByName;
    impl Addressing for ByName {
        fn resolve(&self, parent: &Value, segment: &str) -> Option<String> {
            let name = segment.strip_prefix('@')?;
            parent
                .as_array()?
                .iter()
                .position(|i| i["name"] == name)
                .map(|i| i.to_string())
        }
    }

    fn ops(v: Value) -> Vec<PatchOp> {
        serde_json::from_value(v).unwrap()
    }

    #[test]
    fn every_operation_works() {
        let doc = json!({"a": 1, "list": [1, 2, 3], "o": {"k": "v"}});
        let out = apply_patch(
            &doc,
            &ops(json!([
                {"op": "add", "path": "/b", "value": 2},
                {"op": "add", "path": "/list/1", "value": 9},
                {"op": "add", "path": "/list/-", "value": 4},
                {"op": "remove", "path": "/a"},
                {"op": "replace", "path": "/o/k", "value": "w"},
                {"op": "copy", "from": "/o", "path": "/p"},
                {"op": "move", "from": "/b", "path": "/c"},
                {"op": "test", "path": "/c", "value": 2}
            ])),
            &Plain,
        )
        .unwrap();
        assert_eq!(
            out,
            json!({"list": [1, 9, 2, 3, 4], "o": {"k": "w"}, "p": {"k": "w"}, "c": 2})
        );
    }

    #[test]
    fn a_failed_patch_changes_nothing_and_says_which_change() {
        let doc = json!({"a": 1});
        let err = apply_patch(
            &doc,
            &ops(json!([
                {"op": "replace", "path": "/a", "value": 2},
                {"op": "remove", "path": "/missing"}
            ])),
            &Plain,
        )
        .unwrap_err();
        assert_eq!(err.code, "patch");
        assert_eq!(
            err.message,
            "Change 2 of 2 couldn't be applied: /missing doesn't exist."
        );
        assert!(
            apply_patch(
                &doc,
                &ops(json!([{"op": "test", "path": "/a", "value": 5}])),
                &Plain
            )
            .is_err()
        );
        assert!(
            apply_patch(
                &doc,
                &ops(json!([{"op": "move", "from": "/a", "path": "/a/b"}])),
                &Plain
            )
            .is_err()
        );
        assert!(
            apply_patch(
                &doc,
                &ops(json!([{"op": "replace", "path": "a", "value": 1}])),
                &Plain
            )
            .is_err()
        );
    }

    #[test]
    fn a_kind_can_name_list_items() {
        let doc =
            json!({"shots": [{"name": "s1", "dur": 4}, {"name": "s4-delegation", "dur": 10}]});
        let out = apply_patch(
            &doc,
            &ops(json!([{"op": "replace", "path": "/shots/@s4-delegation/dur", "value": 12.5}])),
            &ByName,
        )
        .unwrap();
        assert_eq!(out["shots"][1]["dur"], 12.5);
        let err = apply_patch(
            &doc,
            &ops(json!([{"op": "replace", "path": "/shots/@nope/dur", "value": 1}])),
            &ByName,
        )
        .unwrap_err();
        assert!(
            err.message.contains("nothing is named nope"),
            "{}",
            err.message
        );
        // Without the kind's naming, @ never silently means a key.
        assert!(
            apply_patch(
                &doc,
                &ops(json!([{"op": "replace", "path": "/shots/@s1/dur", "value": 1}])),
                &Plain
            )
            .is_err()
        );
    }

    #[test]
    fn escaped_keys_follow_rfc_6901() {
        let doc = json!({"a/b": {"m~n": 1}});
        let out = apply_patch(
            &doc,
            &ops(json!([{"op": "replace", "path": "/a~1b/m~0n", "value": 2}])),
            &Plain,
        )
        .unwrap();
        assert_eq!(out, json!({"a/b": {"m~n": 2}}));
    }
}
