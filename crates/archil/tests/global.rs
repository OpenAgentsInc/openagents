//! The module-level convenience functions share one process-wide client, so
//! everything is exercised from a single test to avoid cross-test races.

mod common;

use archil::{
    ClientOptions, CreateApiTokenRequest, CreateDiskRequest, CreateSandboxRequest,
    ListDisksOptions, ListSandboxesOptions, ListTokensOptions, WaitOptions,
};
use common::*;
use serde_json::json;
use wiremock::matchers::{header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn opts(server: &MockServer, key: &str) -> ClientOptions {
    ClientOptions::new()
        .api_key(key)
        .region("aws-us-east-1")
        .base_url(server.uri())
        .s3_base_url(server.uri())
}

#[tokio::test]
async fn module_level_functions_use_the_configured_client_and_can_swap_credentials() {
    let first = MockServer::start().await;
    let second = MockServer::start().await;
    for (server, id) in [(&first, "first"), (&second, "second")] {
        Mock::given(method("GET"))
            .and(path("/api/disks"))
            .and(header("authorization", format!("key-{id}")))
            .respond_with(ok(json!([disk_json(id, id)])))
            .mount(server)
            .await;
    }
    Mock::given(method("GET"))
        .and(path("/api/disks/d1"))
        .respond_with(ok(disk_json("d1", "n")))
        .mount(&first)
        .await;
    Mock::given(method("POST")).and(path("/api/disks")).respond_with(ResponseTemplate::new(201).set_body_json(json!({
        "success": true, "data": {"diskId": "d1", "authorizedUsers": [{"token": "t", "identifier": "i"}]}
    }))).mount(&first).await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes"))
        .respond_with(ok(json!({"sandboxes": []})))
        .mount(&first)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes/s1"))
        .respond_with(ok(sandbox_json("s1", "running")))
        .mount(&first)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .respond_with(ResponseTemplate::new(202).set_body_json(json!({
            "success": true, "data": sandbox_json("s1", "running")
        })))
        .mount(&first)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/tokens"))
        .respond_with(ok(json!({"tokens": []})))
        .mount(&first)
        .await;
    Mock::given(method("POST"))
        .and(path("/api/tokens"))
        .respond_with(ResponseTemplate::new(201).set_body_json(json!({
            "success": true, "data": {"id": "k1", "token": "secret"}
        })))
        .mount(&first)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/api/tokens/k1"))
        .respond_with(ResponseTemplate::new(204))
        .mount(&first)
        .await;
    Mock::given(method("POST")).and(path("/api/exec")).respond_with(ok(json!({
        "exitCode": 0, "stdout": "hi", "stderr": "", "timing": {"totalMs": 1, "queueMs": 0, "executeMs": 1}
    }))).mount(&first).await;

    archil::configure(opts(&first, "first"));
    assert_eq!(
        archil::list_disks(ListDisksOptions::default())
            .await
            .unwrap()[0]
            .id,
        "first"
    );
    assert_eq!(archil::get_disk("d1").await.unwrap().id, "d1");
    assert_eq!(
        archil::create_disk(CreateDiskRequest::new("x"))
            .await
            .unwrap()
            .token
            .as_deref(),
        Some("t")
    );
    assert!(
        archil::list_sandboxes(ListSandboxesOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(archil::get_sandbox("s1").await.unwrap().id, "s1");
    assert_eq!(
        archil::create_sandbox(CreateSandboxRequest::new(), WaitOptions::default())
            .await
            .unwrap()
            .id,
        "s1"
    );
    assert!(
        archil::list_api_keys(ListTokensOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        archil::create_api_key(CreateApiTokenRequest::new("k"))
            .await
            .unwrap()
            .token
            .as_deref(),
        Some("secret")
    );
    archil::delete_api_key("k1").await.unwrap();
    let r = archil::exec(archil::ExecOptions::new("echo hi").disk("d", "d1"))
        .await
        .unwrap();
    assert_eq!(r.stdout, "hi");

    // configure() discards the cached client, so new credentials take effect.
    archil::configure(opts(&second, "second"));
    assert_eq!(
        archil::list_disks(ListDisksOptions::default())
            .await
            .unwrap()[0]
            .id,
        "second"
    );
    assert_eq!(archil::client().unwrap().base_url(), second.uri());

    let disk = archil::get_disk("d1").await;
    assert!(disk.is_err(), "second server has no d1");
}
