//! Check the published schemas against examples and actual handler responses.
use axum::{
    body::{Body, Bytes},
    extract::Request,
    middleware::Next,
    response::Response,
};
use futures_util::StreamExt;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};

fn validator(document: &Value, schema: &Value) -> jsonschema::Validator {
    let mut schema = schema.clone();
    schema["$schema"] = json!("https://json-schema.org/draft/2020-12/schema");
    schema["components"] = document["components"].clone();
    jsonschema::validator_for(&schema).expect("valid published schema")
}

// Validate at EOF without delaying response headers or collecting a request
// body. This also exercises the early413 response while an upload is stalled.
pub(super) async fn check_response(request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let method = request.method().as_str().to_ascii_lowercase();
    let response = next.run(request).await;
    if !path.starts_with("/v1/computers")
        || !response
            .headers()
            .get("content-type")
            .is_some_and(|v| v.as_bytes().starts_with(b"application/json"))
    {
        return response;
    }
    let document = crate::app_openapi::computers_document();
    let paths = document["paths"].as_object().unwrap();
    let item = paths
        .get(&path)
        .or_else(|| {
            paths.iter().find_map(|(template, item)| {
                let wanted: Vec<_> = template.split('/').collect();
                let actual: Vec<_> = path.split('/').collect();
                (wanted.len() == actual.len()
                    && wanted
                        .iter()
                        .zip(actual)
                        .all(|(a, b)| a.starts_with('{') || *a == b))
                .then_some(item)
            })
        })
        .expect("documented route");
    let status = response.status().as_u16().to_string();
    let schema =
        item[&method]["responses"][&status]["content"]["application/json"]["schema"].clone();
    assert!(
        schema.is_object(),
        "Undocumented response {method} {path} {status}"
    );
    let check = validator(&document, &schema);
    let (mut parts, body) = response.into_parts();
    parts.headers.remove("content-length");
    let bytes = Arc::new(Mutex::new(Vec::new()));
    let captured = bytes.clone();
    let stream = body.into_data_stream().map(move |chunk| {
        if let Ok(chunk) = &chunk {
            let mut bytes = captured.lock().unwrap();
            assert!(bytes.len() + chunk.len() <= 1024 * 1024);
            bytes.extend_from_slice(chunk);
        }
        chunk
    });
    let end = futures_util::stream::once(async move {
        let value: Value = serde_json::from_slice(&bytes.lock().unwrap()).unwrap();
        assert!(
            check.is_valid(&value),
            "Response schema drift: {method} {path} {status}"
        );
        Ok::<_, axum::Error>(Bytes::new())
    });
    Response::from_parts(parts, Body::from_stream(stream.chain(end)))
}

#[test]
fn computers_subset_and_examples_match_the_app_contract() {
    let document = crate::app_openapi::computers_document();
    let app = crate::app_openapi::document();
    let mut operations = std::collections::BTreeSet::new();
    for (path, item) in document["paths"].as_object().unwrap() {
        assert_eq!(item, &app["paths"][path]);
        for (_, operation) in item.as_object().unwrap() {
            assert!(operations.insert(operation["operationId"].as_str().unwrap()));
            if let Some(content) = operation["requestBody"]["content"].as_object() {
                for media in content.values() {
                    if let Some(example) = media.get("example") {
                        assert!(validator(&document, &media["schema"]).is_valid(example));
                    }
                }
            }
            for response in operation["responses"].as_object().unwrap().values() {
                if let Some(content) = response["content"].as_object() {
                    for media in content.values() {
                        if let Some(example) = media.get("example") {
                            assert!(validator(&document, &media["schema"]).is_valid(example));
                        }
                    }
                }
            }
        }
    }
    assert_eq!(operations.len(), 18);
    for schema in document["components"]["schemas"]
        .as_object()
        .unwrap()
        .values()
    {
        if let Some(examples) = schema["examples"].as_array() {
            let check = validator(&document, schema);
            for example in examples {
                assert!(check.is_valid(example));
            }
        }
    }
}

#[test]
fn committed_computers_schema_matches_the_handlers() {
    let fixture: Value =
        serde_json::from_str(include_str!("../../../../docs/sdk/openapi.computers.json")).unwrap();
    assert_eq!(
        crate::app_openapi::computers_document(),
        fixture,
        "regenerate with cargo run -p openagents-web --example computers_openapi"
    );
}

#[test]
fn runtime_validation_detects_a_deliberately_changed_contract() {
    let mut document = crate::app_openapi::computers_document();
    let example = document["components"]["schemas"]["AppComputer"]["examples"][0].clone();
    let reference = json!({"$ref":"#/components/schemas/AppComputer"});
    assert!(validator(&document, &reference).is_valid(&example));
    document["components"]["schemas"]["AppComputer"]["required"]
        .as_array_mut()
        .unwrap()
        .push(json!("deliberate_schema_fault"));
    assert!(!validator(&document, &reference).is_valid(&example));
}

#[test]
fn computer_view_preserves_safe_resource_capabilities_and_recorded_policy() {
    let document = crate::app_openapi::computers_document();
    let mut raw = document["components"]["schemas"]["AppComputer"]["examples"][0].clone();
    raw["provider_endpoint"] = json!("https://private.example.test");
    raw["binding_key"] = json!("private-binding-key");
    raw["capabilities"] = json!({
        "driver":"private-driver", "profiles":["core","suspend","private-profile"],
        "sizes":["small","https://private.example.test"],
        "shapes":{"small":{"vcpus":2,"memory_mib":1024,"min_disk_gib":4,"provider_key":"private-shape"},"private-size":{"vcpus":100,"memory_mib":100}},
        "workspace_file_bytes":4194304,"command_sessions":true,
        "environment_artifacts":["private-artifact"],"default_artifact":"private-artifact"
    });
    raw["lifecycle_policy"] = json!({
        "turn_complete":"suspend","idle_ms":100,"observed_extension_ms":200,
        "absolute_ms":5000,"absolute_scope":"boot","uncertain_at_absolute_limit":"delete","stopped_retention_ms":604800000,
        "provider_key":"private-policy"
    });
    let check = validator(
        &document,
        &json!({"$ref":"#/components/schemas/AppComputer"}),
    );
    for declaration in [Value::Null, json!(false), json!(true)] {
        if declaration.is_null() {
            raw["capabilities"]
                .as_object_mut()
                .unwrap()
                .remove("memory_suspend");
        } else {
            raw["capabilities"]["memory_suspend"] = declaration.clone();
        }
        let public = super::view(&raw);
        assert_eq!(
            public["capabilities"]["memory_suspend"],
            declaration.as_bool().unwrap_or(false)
        );
        assert_eq!(
            public["capabilities"]["profiles"],
            json!(["core", "suspend"])
        );
        assert_eq!(public["capabilities"]["sizes"], json!(["small"]));
        assert_eq!(
            public["capabilities"]["shapes"],
            json!({"small":{"vcpus":2,"memory_mib":1024,"min_disk_gib":4}})
        );
        assert_eq!(public["lifecycle_policy"]["absolute_ms"], 5000);
        assert_eq!(
            public["lifecycle_policy"]["stopped_retention_ms"],
            604800000
        );
        assert!(!public.to_string().contains("private"), "{public}");
        assert!(check.is_valid(&public), "{public}");
    }
    raw["lifecycle_policy"]
        .as_object_mut()
        .unwrap()
        .remove("stopped_retention_ms");
    let old_policy = super::view(&raw);
    assert!(old_policy["lifecycle_policy"]["stopped_retention_ms"].is_null());
    assert!(check.is_valid(&old_policy));
    raw.as_object_mut().unwrap().remove("capabilities");
    raw.as_object_mut().unwrap().remove("lifecycle_policy");
    let legacy = super::view(&raw);
    assert!(legacy["capabilities"].is_null());
    assert!(legacy["lifecycle_policy"].is_null());
    assert!(check.is_valid(&legacy));
}
