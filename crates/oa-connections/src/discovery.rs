//! Tools from a Google API Discovery document
//! (`https://www.googleapis.com/discovery/v1/apis/{api}/{version}/rest`).
//!
//! Every method under `resources` (nested resources too) becomes one
//! [`ToolSpec`]: its id is `{prefix}.{resource path}.{method}`
//! (`drive.files.list`), its description is the method's, its input
//! schema has the method's parameters (and `body` when it takes one), its
//! scopes are the method's `scopes`, and its HTTP method sets its default
//! policy. This is how a whole Google API becomes an integration; the
//! curated Drive tools in [`crate::google::drive`] are the subset the
//! product offers first.

use serde_json::{Map, Value, json};

use crate::core::{HttpMethod, ToolSpec};

/// The most tools one document yields; a document past that is cut there.
pub const MAX_TOOLS: usize = 1_000;

/// The tools in `document`, ids starting with `prefix`, in document order.
#[must_use]
pub fn tools(document: &Value, prefix: &str) -> Vec<ToolSpec> {
    let mut out = Vec::new();
    if let Some(resources) = document["resources"].as_object() {
        walk(resources, prefix, &mut out);
    }
    out
}

fn walk(resources: &Map<String, Value>, prefix: &str, out: &mut Vec<ToolSpec>) {
    for (name, resource) in resources {
        let path = format!("{prefix}.{name}");
        if let Some(methods) = resource["methods"].as_object() {
            for (method_name, method) in methods {
                if out.len() >= MAX_TOOLS {
                    return;
                }
                if let Some(tool) = tool(&format!("{path}.{method_name}"), method) {
                    out.push(tool);
                }
            }
        }
        if let Some(nested) = resource["resources"].as_object() {
            walk(nested, &path, out);
        }
    }
}

fn tool(id: &str, method: &Value) -> Option<ToolSpec> {
    let http = HttpMethod::parse(method["httpMethod"].as_str()?)?;
    let mut properties = Map::new();
    let mut required = Vec::new();
    for (name, parameter) in method["parameters"].as_object().into_iter().flatten() {
        let kind = match parameter["type"].as_str() {
            Some("integer") => "integer",
            Some("boolean") => "boolean",
            Some("number") => "number",
            _ => "string",
        };
        let mut property = json!({"type": kind});
        if let Some(description) = parameter["description"].as_str() {
            property["description"] = json!(description);
        }
        if let Some(values) = parameter["enum"].as_array() {
            property["enum"] = Value::Array(values.clone());
        }
        if parameter["repeated"].as_bool() == Some(true) {
            property = json!({"type": "array", "items": property});
        }
        if parameter["required"].as_bool() == Some(true) {
            required.push(json!(name));
        }
        properties.insert(name.clone(), property);
    }
    if method["request"].is_object() {
        properties.insert(
            "body".into(),
            json!({"type": "object", "description": "The request body."}),
        );
        required.push(json!("body"));
    }
    Some(ToolSpec {
        id: id.to_owned(),
        description: method["description"]
            .as_str()
            .unwrap_or_default()
            .chars()
            .take(1_000)
            .collect(),
        method: http,
        input_schema: json!({
            "type": "object",
            "properties": properties,
            "required": required,
        }),
        scopes: method["scopes"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|scope| scope.as_str().map(str::to_owned))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::Policy;

    #[test]
    fn methods_become_tools_with_policy_from_their_http_method() {
        let document = json!({
            "name": "drive",
            "resources": {
                "files": {
                    "methods": {
                        "list": {
                            "httpMethod": "GET",
                            "description": "Lists files.",
                            "parameters": {
                                "q": {"type": "string", "location": "query"},
                                "pageSize": {"type": "integer", "location": "query"}
                            },
                            "scopes": ["https://www.googleapis.com/auth/drive.readonly"]
                        },
                        "delete": {
                            "httpMethod": "DELETE",
                            "parameters": {"fileId": {"type": "string", "required": true}},
                            "scopes": ["https://www.googleapis.com/auth/drive"]
                        }
                    },
                    "resources": {
                        "labels": {"methods": {"modify": {
                            "httpMethod": "POST",
                            "request": {"$ref": "ModifyLabelsRequest"}
                        }}}
                    }
                }
            }
        });
        let tools = tools(&document, "drive");
        let mut ids: Vec<&str> = tools.iter().map(|t| t.id.as_str()).collect();
        ids.sort_unstable();
        assert_eq!(
            ids,
            [
                "drive.files.delete",
                "drive.files.labels.modify",
                "drive.files.list"
            ]
        );
        let find = |id: &str| tools.iter().find(|t| t.id == id).unwrap();
        let list = find("drive.files.list");
        assert_eq!(list.default_policy(), Policy::Allow);
        assert_eq!(
            list.input_schema["properties"]["pageSize"]["type"],
            "integer"
        );
        let delete = find("drive.files.delete");
        assert_eq!(delete.default_policy(), Policy::RequireApproval);
        assert_eq!(delete.input_schema["required"], json!(["fileId"]));
        let modify = find("drive.files.labels.modify");
        assert_eq!(modify.input_schema["required"], json!(["body"]));
    }
}
