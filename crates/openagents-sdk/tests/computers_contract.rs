use openagents_sdk::generated::computers::*;
use serde_json::json;
use sha2::{Digest, Sha256};

fn fixture() -> serde_json::Value {
    serde_json::from_slice(include_bytes!("../../../docs/sdk/openapi.computers.json")).unwrap()
}

#[test]
fn generated_bindings_pin_the_exact_exported_schema_and_operations() {
    let digest = format!(
        "{:x}",
        Sha256::digest(include_bytes!("../../../docs/sdk/openapi.computers.json"))
    );
    assert_eq!(SCHEMA_SHA256, digest);
    let spec = fixture();
    for operation in OPERATIONS {
        let op = &spec["paths"][operation.path][operation.method.to_ascii_lowercase()];
        assert_eq!(op["operationId"], operation.id);
        let success: Vec<_> = op["responses"]
            .as_object()
            .unwrap()
            .keys()
            .filter(|s| s.starts_with('2'))
            .map(|s| s.parse::<u16>().unwrap())
            .collect();
        assert_eq!(operation.success, success);
    }
    assert_eq!(OPERATIONS.len(), 21);
}

#[test]
fn published_success_examples_decode_into_generated_wire_values() {
    let spec = fixture();
    let sample = |name: &str| spec["components"]["schemas"][name]["examples"][0].clone();
    let _: AppComputer = serde_json::from_value(sample("AppComputer")).unwrap();
    let _: AppComputerList = serde_json::from_value(sample("AppComputerList")).unwrap();
    let _: AppComputerCapabilities =
        serde_json::from_value(sample("AppComputerCapabilities")).unwrap();
    let _: AppCommand = serde_json::from_value(sample("AppCommand")).unwrap();
    let _: AppCommandOutput = serde_json::from_value(sample("AppCommandOutput")).unwrap();
    let _: AppComputerFileInfo = serde_json::from_value(sample("AppComputerFileInfo")).unwrap();
}

#[test]
fn future_states_and_fields_survive_decoding_without_becoming_success() {
    let mut value = fixture()["components"]["schemas"]["AppCommand"]["examples"][0].clone();
    value["state"] = json!("future_state");
    value["future_detail"] = json!({"version":2});
    let command: AppCommand = serde_json::from_value(value).unwrap();
    assert_eq!(command.state, "future_state");
    assert_eq!(command.extra["future_detail"]["version"], 2);
    assert!(!format!("{command:?}").contains("future_state"));
    let old: AppComputerCapabilities = serde_json::from_value(json!({
        "schema":"openagents.computers.capabilities.v1","profiles":["core"],
        "sizes":["small"],"limits":{"max_computers":20}
    }))
    .unwrap();
    assert!(old.files.is_none());
}

#[test]
fn computer_resources_decode_optional_capabilities_without_claiming_legacy_support() {
    let mut value = fixture()["components"]["schemas"]["AppComputer"]["examples"][0].clone();
    let current: AppComputer = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(current.capabilities.as_ref().unwrap().memory_suspend, true);
    assert_eq!(current.lifecycle_policy.as_ref().unwrap().absolute_ms, 5000);
    assert_eq!(
        current
            .lifecycle_policy
            .as_ref()
            .unwrap()
            .stopped_retention_ms,
        Some(604800000)
    );
    value["capabilities"]["memory_suspend"] = json!(false);
    let disk_only: AppComputer = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        disk_only.capabilities.as_ref().unwrap().memory_suspend,
        false
    );
    for field in ["capabilities", "lifecycle_policy"] {
        value.as_object_mut().unwrap().remove(field);
    }
    let legacy: AppComputer = serde_json::from_value(value).unwrap();
    assert!(legacy.capabilities.is_none());
    assert!(legacy.lifecycle_policy.is_none());
}

#[test]
fn request_builders_keep_keys_and_raw_path_components_separate() {
    let params: CreateComputerParams = serde_json::from_value(json!({
        "Idempotency-Key":"stable-request", "body":{"size":"small"}
    }))
    .unwrap();
    let request = params.into_request().unwrap();
    assert_eq!(request.operation.id, "createComputer");
    assert_eq!(request.path_segments, ["computers"]);
    assert_eq!(
        request.headers,
        [("Idempotency-Key", "stable-request".to_owned())]
    );
    match request.body {
        RequestBody::Json(body) => assert_eq!(body, json!({"size":"small"})),
        _ => panic!("JSON request"),
    }
    let params: DeleteComputerParams =
        serde_json::from_value(json!({"id":"name /?","kind":"cloud"})).unwrap();
    let request = params.into_request().unwrap();
    assert_eq!(request.path_segments, ["computers", "name /?"]);
    assert_eq!(request.query, [("kind", "cloud".to_owned())]);
    assert!(request.operation.empty_response);
    assert!(request.operation.success.contains(&202) && request.operation.success.contains(&204));
}

#[test]
fn binary_bodies_remain_bytes_and_debug_hides_request_contents() {
    let params: WriteComputerFileParams = serde_json::from_value(json!({
        "id":"cmp_0123456789abcdef0123","path":"space 'té.bin",
        "Content-Length":4,"X-Content-SHA256":"a".repeat(64),
        "body":[0,255,0,1]
    }))
    .unwrap();
    let request = params.into_request().unwrap();
    assert_eq!(request.query, [("path", "space 'té.bin".to_owned())]);
    assert!(
        request
            .headers
            .iter()
            .any(|(key, value)| *key == "Content-Length" && value == "4")
    );
    assert!(!format!("{request:?}").contains("té.bin"));
    match request.body {
        RequestBody::Binary(bytes) => assert_eq!(bytes, [0, 255, 0, 1]),
        _ => panic!("binary request"),
    }
    let params: StartAppLoginParams =
        serde_json::from_value(json!({"body":{"computer":"Example"}})).unwrap();
    assert_eq!(
        params.into_request().unwrap().operation.authority,
        Authority::Public
    );
}
