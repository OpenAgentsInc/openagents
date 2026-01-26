use serde_json::{Value, json};
use std::collections::HashMap;

use crate::contracts::ipc::UiPatch;

#[derive(Debug, Clone)]
pub struct UiTreeState {
    pub root: String,
    pub stack_key: String,
    pub elements: HashMap<String, Value>,
}

impl UiTreeState {
    pub fn new(title: &str, status: &str) -> Self {
        let root = "canvas".to_string();
        let stack_key = "stack".to_string();
        let mut elements = HashMap::new();

        elements.insert(
            root.clone(),
            json!({
                "key": root,
                "type": "canvas",
                "props": {
                    "title": title,
                    "status": status,
                },
                "children": [stack_key],
            }),
        );

        elements.insert(
            stack_key.clone(),
            json!({
                "key": stack_key,
                "type": "stack",
                "props": { "gap": 16 },
                "children": [],
            }),
        );

        Self {
            root,
            stack_key,
            elements,
        }
    }

    pub fn to_value(&self) -> Value {
        json!({
            "root": self.root,
            "elements": self.elements,
        })
    }

    pub fn add_element(&mut self, element: Value) -> Option<UiPatch> {
        let key = element.get("key")?.as_str()?.to_string();
        self.elements.insert(key.clone(), element);
        Some(UiPatch {
            op: "add".to_string(),
            path: format!("/elements/{}", key),
            value: Some(self.elements[&key].clone()),
        })
    }

    pub fn append_child(&mut self, parent: &str, child: String) -> Option<UiPatch> {
        let element = self.elements.get_mut(parent)?;
        let children = element
            .get_mut("children")
            .and_then(|value| value.as_array_mut())?;
        children.push(Value::String(child));
        Some(UiPatch {
            op: "replace".to_string(),
            path: format!("/elements/{}/children", parent),
            value: Some(Value::Array(children.clone())),
        })
    }

    pub fn set_element_prop(
        &mut self,
        key: &str,
        prop_path: &str,
        value: Value,
    ) -> Option<UiPatch> {
        let element = self.elements.get_mut(key)?;
        set_by_path(element, prop_path, value.clone());
        Some(UiPatch {
            op: "replace".to_string(),
            path: format!("/elements/{}{}", key, prop_path),
            value: Some(value),
        })
    }
}

fn set_by_path(target: &mut Value, path: &str, value: Value) {
    let trimmed = path.trim_start_matches('/');
    let segments: Vec<&str> = trimmed.split('/').collect();
    if segments.is_empty() {
        return;
    }

    let mut current = target;
    for (idx, segment) in segments.iter().enumerate() {
        if idx == segments.len() - 1 {
            if let Value::Object(map) = current {
                map.insert(segment.to_string(), value);
            }
            return;
        }

        if let Value::Object(map) = current {
            if !map.contains_key(*segment) {
                map.insert(segment.to_string(), Value::Object(Default::default()));
            }
            current = map.get_mut(*segment).unwrap();
        } else {
            return;
        }
    }
}
