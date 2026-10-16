//! The public System One slice, generated from the decision API's own contract.

use serde_json::Value;

const CONTRACT: &str = include_str!("../../../docs/sdk/openapi.systemone.json");

pub(crate) fn extend(document: &mut Value) -> Result<(), &'static str> {
    let slice: Value = serde_json::from_str(CONTRACT).map_err(|_| "Invalid System One slice")?;
    let method = axum::http::Method::POST;
    if !crate::api_alias::forwards(&method, "/v1/systemone", false) {
        return Err("System One is not public on this front");
    }
    for (path, operations) in slice["paths"]
        .as_object()
        .ok_or("Missing System One path")?
    {
        let target = document["paths"]
            .as_object_mut()
            .ok_or("Missing public paths")?
            .entry(path.clone())
            .or_insert_with(|| serde_json::json!({}));
        for (method, value) in operations
            .as_object()
            .ok_or("Invalid System One operation")?
        {
            let target = target.as_object_mut().ok_or("Invalid public path")?;
            if target.get(method).is_some_and(|previous| previous != value) {
                return Err("Conflicting System One operation");
            }
            target.insert(method.clone(), value.clone());
        }
    }
    for (section, entries) in slice["components"]
        .as_object()
        .ok_or("Missing System One components")?
    {
        if document["components"][section].is_null() {
            document["components"][section] = serde_json::json!({});
        }
        let target = document["components"][section]
            .as_object_mut()
            .ok_or("Invalid public components")?;
        for (name, value) in entries.as_object().ok_or("Invalid System One components")? {
            if target.get(name).is_some_and(|previous| previous != value) {
                return Err("Conflicting System One component");
            }
            target.insert(name.clone(), value.clone());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    #[test]
    fn only_systemone_is_published_with_its_complete_component_closure() {
        let mut document = json!({"paths":{},"components":{}});
        super::extend(&mut document).unwrap();
        let source: Value = serde_json::from_str(super::CONTRACT).unwrap();
        assert_eq!(document["paths"], source["paths"]);
        assert_eq!(document["paths"].as_object().unwrap().len(), 1);
        let operation = &document["paths"]["/v1/systemone"]["post"];
        assert_eq!(operation["operationId"], "answerSystemOne");
        assert_eq!(operation["security"], json!([{"SystemOneBearer":[]},{}]));
        assert_eq!(operation["parameters"][0]["name"], "X-Workspace-Id");
        assert!(operation["responses"]["429"]["headers"]["Retry-After"].is_object());
        assert!(
            document["components"]["schemas"]
                .get("SystemOneJobStatus")
                .is_none()
        );
        assert!(document["components"]["schemas"]["SystemOneTypedError"]["properties"]["error"]["properties"]["code"].get("enum").is_none());
        fn refs(value: &Value, document: &Value) {
            match value {
                Value::Object(map) => {
                    for (key, value) in map {
                        if key == "$ref" {
                            let reference = value.as_str().unwrap();
                            assert!(reference.starts_with("#/components/"));
                            assert!(document.pointer(&reference[1..]).is_some(), "{reference}");
                        } else {
                            refs(value, document);
                        }
                    }
                }
                Value::Array(list) => {
                    for value in list {
                        refs(value, document);
                    }
                }
                _ => {}
            }
        }
        refs(&document, &document);
    }

    #[test]
    fn conflicting_operation_or_component_is_not_replaced() {
        for mut document in [
            json!({"paths":{"/v1/systemone":{"post":{"operationId":"different"}}},"components":{}}),
            json!({"paths":{},"components":{"schemas":{"SystemOneCall":{"type":"number"}}}}),
        ] {
            assert!(super::extend(&mut document).is_err());
        }
    }
}
