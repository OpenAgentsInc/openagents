use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DynamicValue<T> {
    Literal(T),
    Path { path: String },
}

pub type DynamicString = DynamicValue<String>;
pub type DynamicNumber = DynamicValue<f64>;
pub type DynamicBoolean = DynamicValue<bool>;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum LogicExpression {
    And { and: Vec<LogicExpression> },
    Or { or: Vec<LogicExpression> },
    Not { not: Box<LogicExpression> },
    Path { path: String },
    Eq { eq: [DynamicValue<Value>; 2] },
    Neq { neq: [DynamicValue<Value>; 2] },
    Gt { gt: [DynamicNumber; 2] },
    Gte { gte: [DynamicNumber; 2] },
    Lt { lt: [DynamicNumber; 2] },
    Lte { lte: [DynamicNumber; 2] },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VisibilityCondition {
    Boolean(bool),
    Path { path: String },
    Auth { auth: AuthVisibility },
    Logic(LogicExpression),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AuthVisibility {
    SignedIn,
    SignedOut,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiElement {
    pub key: String,
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(default)]
    pub props: Value,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_key: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible: Option<VisibilityCondition>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiTree {
    pub root: String,
    pub elements: HashMap<String, UiElement>,
}

impl UiTree {
    pub fn empty() -> Self {
        Self {
            root: String::new(),
            elements: HashMap::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PatchOp {
    Add,
    Remove,
    Replace,
    Set,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct UiPatch {
    pub op: PatchOp,
    pub path: String,
    pub value: Option<Value>,
}

pub fn parse_patch_line(line: &str) -> Option<UiPatch> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with("//") {
        return None;
    }
    serde_json::from_str(trimmed).ok()
}

pub fn apply_patch(tree: &UiTree, patch: &UiPatch) -> UiTree {
    let mut next = UiTree {
        root: tree.root.clone(),
        elements: tree.elements.clone(),
    };

    match patch.op {
        PatchOp::Add | PatchOp::Replace | PatchOp::Set => {
            apply_value_patch(&mut next, patch);
        }
        PatchOp::Remove => {
            if let Some(key) = element_key_from_path(&patch.path) {
                next.elements.remove(key);
            }
        }
    }

    next
}

fn apply_value_patch(tree: &mut UiTree, patch: &UiPatch) {
    if patch.path == "/root" {
        if let Some(Value::String(root)) = patch.value.clone() {
            tree.root = root;
        }
        return;
    }

    let Some((element_key, prop_path)) = element_path_parts(&patch.path) else {
        return;
    };

    if prop_path.is_empty() {
        let Some(value) = patch.value.clone() else {
            return;
        };
        if let Ok(element) = serde_json::from_value::<UiElement>(value) {
            tree.elements.insert(element_key.to_string(), element);
        }
        return;
    }

    let Some(existing) = tree.elements.get(element_key).cloned() else {
        return;
    };

    let mut element_value = match serde_json::to_value(existing) {
        Ok(value) => value,
        Err(_) => return,
    };

    let Some(value) = patch.value.clone() else {
        return;
    };

    let patch_path = format!("/{}", prop_path.join("/"));
    set_by_path(&mut element_value, &patch_path, value);

    if let Ok(updated) = serde_json::from_value::<UiElement>(element_value) {
        tree.elements.insert(element_key.to_string(), updated);
    }
}

fn element_key_from_path(path: &str) -> Option<&str> {
    element_path_parts(path).map(|(key, _)| key)
}

fn element_path_parts(path: &str) -> Option<(&str, Vec<&str>)> {
    let rest = path.strip_prefix("/elements/")?;
    let mut parts = rest.split('/');
    let key = parts.next()?;
    if key.is_empty() {
        return None;
    }
    let prop_path = parts.filter(|segment| !segment.is_empty()).collect::<Vec<_>>();
    Some((key, prop_path))
}

fn set_by_path(target: &mut Value, path: &str, value: Value) {
    let segments = path
        .trim_start_matches('/')
        .split('/')
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();

    if segments.is_empty() {
        return;
    }

    let mut current = target;
    for segment in &segments[..segments.len().saturating_sub(1)] {
        match current {
            Value::Object(map) => {
                if !map.contains_key(*segment) || !map[*segment].is_object() {
                    map.insert((*segment).to_string(), Value::Object(serde_json::Map::new()));
                }
                let Some(next) = map.get_mut(*segment) else {
                    return;
                };
                current = next;
            }
            _ => {
                *current = Value::Object(serde_json::Map::new());
                if let Value::Object(map) = current {
                    map.insert((*segment).to_string(), Value::Object(serde_json::Map::new()));
                    let Some(next) = map.get_mut(*segment) else {
                        return;
                    };
                    current = next;
                } else {
                    return;
                }
            }
        }
    }

    let last = segments[segments.len() - 1];
    match current {
        Value::Object(map) => {
            map.insert(last.to_string(), value);
        }
        _ => {
            let mut map = serde_json::Map::new();
            map.insert(last.to_string(), value);
            *current = Value::Object(map);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn apply_patch_sets_root() {
        let tree = UiTree::empty();
        let patch = UiPatch {
            op: PatchOp::Set,
            path: "/root".to_string(),
            value: Some(Value::String("root".to_string())),
        };

        let next = apply_patch(&tree, &patch);
        assert_eq!(next.root, "root");
    }

    #[test]
    fn apply_patch_adds_element() {
        let tree = UiTree::empty();
        let patch = UiPatch {
            op: PatchOp::Add,
            path: "/elements/canvas".to_string(),
            value: Some(json!({
                "key": "canvas",
                "type": "canvas",
                "props": {"title": "Autopilot"},
                "children": []
            })),
        };

        let next = apply_patch(&tree, &patch);
        assert!(next.elements.contains_key("canvas"));
    }

    #[test]
    fn apply_patch_updates_props() {
        let mut tree = UiTree::empty();
        tree.elements.insert(
            "canvas".to_string(),
            UiElement {
                key: "canvas".to_string(),
                element_type: "canvas".to_string(),
                props: json!({"title": "Old"}),
                children: vec![],
                parent_key: None,
                visible: None,
            },
        );

        let patch = UiPatch {
            op: PatchOp::Replace,
            path: "/elements/canvas/props/title".to_string(),
            value: Some(Value::String("New".to_string())),
        };

        let next = apply_patch(&tree, &patch);
        let updated_title = next
            .elements
            .get("canvas")
            .and_then(|element| element.props.get("title"));
        assert_eq!(updated_title, Some(&Value::String("New".to_string())));
    }

    #[test]
    fn apply_patch_removes_element() {
        let mut tree = UiTree::empty();
        tree.elements.insert(
            "canvas".to_string(),
            UiElement {
                key: "canvas".to_string(),
                element_type: "canvas".to_string(),
                props: Value::Null,
                children: vec![],
                parent_key: None,
                visible: None,
            },
        );

        let patch = UiPatch {
            op: PatchOp::Remove,
            path: "/elements/canvas".to_string(),
            value: None,
        };

        let next = apply_patch(&tree, &patch);
        assert!(!next.elements.contains_key("canvas"));
    }
}
