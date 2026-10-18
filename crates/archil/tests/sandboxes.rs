mod common;

use std::sync::atomic::Ordering;

use archil::{
    CreateSandboxPortTokenOptions, CreateSandboxRequest, Error, ListSandboxPortTokensOptions,
    ListSandboxesOptions, SandboxEgressPolicy, SandboxEgressRule, SandboxEgressTransform,
    SandboxForkOptions, SandboxMountSpec, SandboxNetwork, SandboxNetworkAction,
    SandboxStartOptions, SandboxStatus, SandboxTimeoutOptions, WaitOptions,
};
use common::*;
use serde_json::{Value, json};
use wiremock::matchers::{body_json, method, path, query_param, query_param_is_missing};
use wiremock::{Mock, MockServer, ResponseTemplate};

const SID: &str = "11111111-1111-1111-1111-111111111111";

fn sb(status: &str) -> Value {
    sandbox_json(SID, status)
}

fn accepted(v: Value) -> ResponseTemplate {
    status(202, v)
}

fn p(suffix: &str) -> String {
    format!("/api/sandboxes/{SID}{suffix}")
}

async fn running(server: &MockServer) -> archil::Sandbox {
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(sb("running")))
        .mount(server)
        .await;
    client(server).sandboxes().get(SID).await.unwrap()
}

#[tokio::test]
async fn create_maps_fields_to_the_wire_and_waits_for_running() {
    let server = MockServer::start().await;
    let (cap, bodies) = Capture::new(accepted(sb("pending")));
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .and(query_param("wait", "true"))
        .respond_with(cap)
        .expect(1)
        .mount(&server)
        .await;
    let get = Seq::new(vec![
        ok(sb("pending")),
        ok(sb("pending")),
        ok(sb("running")),
    ]);
    let polls = get.hits.clone();
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(get)
        .mount(&server)
        .await;

    let req = CreateSandboxRequest::new()
        .name("dev")
        .vcpu_count(4)
        .mem_size_mib(8192)
        .base_image("python:3.12-slim")
        .env("NODE_ENV", "development")
        .max_ttl_seconds(3600)
        .idle_ttl_seconds(60)
        .max_concurrent_execs(2)
        .ports([8080, 3000])
        .network(SandboxNetwork {
            egress: Some(SandboxEgressPolicy {
                default: SandboxNetworkAction::Deny,
                allow: Some(vec![
                    "pypi.org".into(),
                    SandboxEgressRule {
                        target: "api.openai.com".into(),
                        transform: Some(SandboxEgressTransform {
                            headers: Some(
                                [("Authorization".to_string(), "Bearer x".to_string())].into(),
                            ),
                        }),
                        forward_url: None,
                    }
                    .into(),
                ]),
                deny: Some(vec!["10.0.0.0/8".into()]),
                drain_on_pause: Some(vec!["*.example.com".into()]),
            }),
            managed: None,
        })
        .mounts([
            SandboxMountSpec::new("dsk-1").path("/mnt/archil"),
            SandboxMountSpec::new("dsk-2")
                .path("/mnt/models")
                .subdirectory("llama")
                .read_only(true),
            SandboxMountSpec::new("dsk-3")
                .path("/work")
                .conditional(true)
                .queue_ms(250),
        ]);
    let sandbox = client(&server).sandboxes().create(req).await.unwrap();
    assert_eq!(sandbox.status, SandboxStatus::Running);
    assert_eq!(
        polls.load(Ordering::SeqCst),
        3,
        "polled until it left pending"
    );
    assert_eq!(
        bodies.lock().unwrap()[0],
        json!({
            "name": "dev", "vcpu_count": 4, "mem_size_mib": 8192, "base_image": "python:3.12-slim",
            "env": {"NODE_ENV": "development"}, "max_ttl_seconds": 3600, "idle_ttl_seconds": 60,
            "max_concurrent_execs": 2, "ports": [8080, 3000],
            "network": {"egress": {
                "default": "deny",
                "allow": ["pypi.org", {"target": "api.openai.com", "transform": {"headers": {"Authorization": "Bearer x"}}}],
                "deny": ["10.0.0.0/8"], "drain_on_pause": ["*.example.com"]
            }},
            "mounts": [
                {"disk_id": "dsk-1", "path": "/mnt/archil", "read_only": false, "conditional": false},
                {"disk_id": "dsk-2", "path": "/mnt/models", "subdirectory": "llama", "read_only": true, "conditional": false},
                {"disk_id": "dsk-3", "path": "/work", "read_only": false, "conditional": true, "queue_ms": 250}
            ]
        })
    );
}

#[tokio::test]
async fn create_with_defaults_sends_an_empty_body_and_no_wait_skips_polling() {
    let server = MockServer::start().await;
    let (cap, bodies) = Capture::new(accepted(sb("pending")));
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .and(query_param("wait", "false"))
        .respond_with(cap)
        .expect(1)
        .mount(&server)
        .await;
    let sandbox = client(&server)
        .sandboxes()
        .create_with(CreateSandboxRequest::new(), WaitOptions::no_wait())
        .await
        .unwrap();
    assert_eq!(sandbox.status, SandboxStatus::Pending);
    assert_eq!(bodies.lock().unwrap()[0], json!({}));
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|r| r.method.as_str() == "POST"),
        "no polling"
    );
}

#[tokio::test]
async fn create_image_id_conflict_surfaces_the_api_error_code() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/sandboxes"))
        .respond_with(err(409, "image not ready", Some("image_not_ready")))
        .mount(&server)
        .await;
    let e = client(&server)
        .sandboxes()
        .create(CreateSandboxRequest::new().image_id("abc"))
        .await
        .unwrap_err();
    assert!(matches!(e, Error::Api(_)));
    assert_eq!((e.status(), e.code()), (Some(409), Some("image_not_ready")));
}

#[tokio::test]
async fn get_parses_every_field_and_serializes_camel_case() {
    let server = MockServer::start().await;
    let mut body = sb("paused");
    body["image_digest"] = "sha256:abc".into();
    body["platform"] = "arm64".into();
    body["endpoints"] = json!([{"port": 8080, "hostname": "8080-x.example.com"}]);
    body["mounts"] = json!([{"disk_id": "dsk-1", "path": "/mnt/archil", "read_only": true, "conditional": false, "queue_ms": 5}]);
    body["running_at"] = "2026-01-01T00:00:05Z".into();
    body["finished_at"] = "2026-01-01T01:00:00Z".into();
    body["exit_reason"] = "idle".into();
    body["checkpoint"] = "cp-1".into();
    body["idle_ttl_seconds"] = Value::Null;
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(body))
        .mount(&server)
        .await;
    let s = client(&server).sandboxes().get(SID).await.unwrap();
    assert_eq!(
        (s.id.as_str(), s.status, s.vcpu_count, s.mem_size_mib),
        (SID, SandboxStatus::Paused, 1, 2048)
    );
    assert_eq!(s.image_digest, "sha256:abc");
    assert_eq!(s.idle_ttl_seconds, 0, "null idle ttl reads as zero");
    assert_eq!(s.mounts[0].path, "/mnt/archil");
    assert!(s.mounts[0].read_only);
    assert_eq!(s.mounts[0].queue_ms, Some(5));
    assert_eq!(s.endpoints.as_ref().unwrap()[0].port, 8080);
    assert_eq!(s.checkpoint.as_deref(), Some("cp-1"));
    let json = serde_json::to_value(s.to_response()).unwrap();
    assert_eq!(json["id"], SID);
    assert_eq!(
        json["memSizeMiB"],
        Value::Null,
        "field is memSizeMib in Rust's camelCase"
    );
    assert_eq!(json["memSizeMib"], 2048);
    assert_eq!(json["exitReason"], "idle");
    assert_eq!(json["mounts"][0]["diskId"], "dsk-1");
}

#[tokio::test]
async fn list_filters_by_disk_and_tolerates_empty_accounts() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes"))
        .and(query_param("filesystem", "dsk-9"))
        .respond_with(ok(json!({"sandboxes": [sb("running")]})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes"))
        .and(query_param_is_missing("filesystem"))
        .respond_with(ok(json!({})))
        .expect(1)
        .mount(&server)
        .await;
    let c = client(&server);
    let by_disk = c
        .sandboxes()
        .list(ListSandboxesOptions {
            disk: Some("dsk-9".into()),
        })
        .await
        .unwrap();
    assert_eq!(by_disk.len(), 1);
    assert!(
        c.sandboxes()
            .list(ListSandboxesOptions::default())
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn start_waits_and_can_replace_mounts() {
    let server = MockServer::start().await;
    let mut s = {
        Mock::given(method("GET"))
            .and(path(p("")))
            .respond_with(Seq::new(vec![ok(sb("pending")), ok(sb("running"))]))
            .mount(&server)
            .await;
        let mut v = sb("stopped");
        v["name"] = "sb".into();
        Mock::given(method("GET"))
            .and(path("/api/sandboxes/other"))
            .respond_with(ok(v))
            .mount(&server)
            .await;
        let mut s = client(&server).sandboxes().get("other").await.unwrap();
        s.id = SID.to_string();
        s
    };
    let (cap, bodies) = Capture::new(accepted(sb("pending")));
    Mock::given(method("POST"))
        .and(path(p("/start")))
        .and(query_param("wait", "true"))
        .respond_with(cap)
        .expect(1)
        .mount(&server)
        .await;
    s.start_with(SandboxStartOptions {
        wait: true,
        mounts: Some(vec![SandboxMountSpec::new("dsk-5").path("/data")]),
    })
    .await
    .unwrap();
    assert_eq!(s.status, SandboxStatus::Running);
    assert_eq!(
        bodies.lock().unwrap()[0],
        json!({"mounts": [{"disk_id": "dsk-5", "path": "/data", "read_only": false, "conditional": false}]})
    );
}

#[tokio::test]
async fn start_retries_transient_failures_only_without_mounts() {
    let server = MockServer::start().await;
    let plain = Seq::new(vec![err(503, "x", None), accepted(sb("running"))]);
    let plain_hits = plain.hits.clone();
    Mock::given(method("POST"))
        .and(path(p("/start")))
        .and(query_param("wait", "false"))
        .respond_with(plain)
        .mount(&server)
        .await;
    let mut s = running(&server).await;
    s.start_with(SandboxStartOptions {
        wait: false,
        mounts: None,
    })
    .await
    .unwrap();
    assert_eq!(plain_hits.load(Ordering::SeqCst), 2);
    let starts = server.received_requests().await.unwrap();
    assert!(
        starts
            .iter()
            .filter(|r| r.url.path().ends_with("/start"))
            .all(|r| r.body.is_empty()),
        "no body without mounts"
    );

    // Replaying a mount update after a lost success would conflict, so it is not retried.
    let server = MockServer::start().await;
    let with = Seq::new(vec![err(503, "x", None), accepted(sb("running"))]);
    let with_hits = with.hits.clone();
    Mock::given(method("POST"))
        .and(path(p("/start")))
        .respond_with(with)
        .mount(&server)
        .await;
    let mut s = running(&server).await;
    let e = s
        .start_with(SandboxStartOptions {
            wait: false,
            mounts: Some(vec![SandboxMountSpec::new("d").path("/m")]),
        })
        .await
        .unwrap_err();
    assert_eq!(e.status(), Some(503));
    assert_eq!(with_hits.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn stop_waits_while_stopping() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path(p("/stop")))
        .respond_with(accepted(sb("stopping")))
        .expect(1)
        .mount(&server)
        .await;
    let mut s = running(&server).await;
    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/stop")))
        .respond_with(accepted(sb("stopping")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(Seq::new(vec![ok(sb("stopping")), ok(sb("stopped"))]))
        .mount(&server)
        .await;
    s.stop().await.unwrap();
    assert_eq!(s.status, SandboxStatus::Stopped);

    // wait: false returns the intermediate state.
    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/stop")))
        .respond_with(accepted(sb("stopping")))
        .mount(&server)
        .await;
    s.stop_with(WaitOptions::no_wait()).await.unwrap();
    assert_eq!(s.status, SandboxStatus::Stopping);
}

#[tokio::test]
async fn pause_and_resume_wait_and_pause_failure_is_typed() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/pause")))
        .respond_with(accepted(sb("pausing")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(Seq::new(vec![ok(sb("pausing")), ok(sb("paused"))]))
        .mount(&server)
        .await;
    s.pause().await.unwrap();
    assert_eq!(s.status, SandboxStatus::Paused);

    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/resume")))
        .and(query_param("wait", "true"))
        .respond_with(accepted(sb("pending")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(sb("running")))
        .mount(&server)
        .await;
    s.resume().await.unwrap();
    assert_eq!(s.status, SandboxStatus::Running);

    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/pause")))
        .respond_with(accepted(sb("pausing")))
        .mount(&server)
        .await;
    let mut failed = sb("failed");
    failed["exit_reason"] = "snapshot failed".into();
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(failed))
        .mount(&server)
        .await;
    let e = s.pause().await.unwrap_err();
    let Error::SandboxPause(ref pe) = e else {
        panic!("{e:?}")
    };
    assert_eq!(pe.latest.status, SandboxStatus::Failed);
    assert_eq!(
        e.to_string(),
        "Sandbox entered failed before it paused: snapshot failed"
    );
    assert_eq!(
        (e.status(), e.code()),
        (Some(409), Some("SANDBOX_PAUSE_FAILED"))
    );
}

#[tokio::test]
async fn fork_of_a_running_sandbox_pauses_forks_the_checkpoint_and_resumes_the_source() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    server.reset().await;

    let mut pausing = sb("pausing");
    pausing["checkpoint"] = "cp-7".into();
    Mock::given(method("POST"))
        .and(path(p("/pause")))
        .respond_with(accepted(pausing.clone()))
        .expect(1)
        .mount(&server)
        .await;
    let mut paused = sb("paused");
    paused["checkpoint"] = "cp-7".into();
    let (fork_cap, fork_bodies) = Capture::new(accepted(sandbox_json(
        "22222222-2222-2222-2222-222222222222",
        "pending",
    )));
    Mock::given(method("POST"))
        .and(path(p("/fork")))
        .and(query_param("wait", "false"))
        .respond_with(fork_cap)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(p("/resume")))
        .and(query_param("wait", "false"))
        .respond_with(accepted(sb("pending")))
        .expect(1)
        .mount(&server)
        .await;
    // Source: pausing -> paused (for the checkpoint wait), then running after the resume.
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(Seq::new(vec![ok(paused), ok(sb("running"))]))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes/22222222-2222-2222-2222-222222222222"))
        .respond_with(Seq::new(vec![
            ok(sandbox_json(
                "22222222-2222-2222-2222-222222222222",
                "pending",
            )),
            ok(sandbox_json(
                "22222222-2222-2222-2222-222222222222",
                "running",
            )),
        ]))
        .mount(&server)
        .await;

    let fork = s
        .fork_with(SandboxForkOptions {
            wait: true,
            name: Some("agent-task".into()),
        })
        .await
        .unwrap();
    assert_eq!(fork.id, "22222222-2222-2222-2222-222222222222");
    assert_eq!(fork.status, SandboxStatus::Running);
    assert_eq!(
        fork_bodies.lock().unwrap()[0],
        json!({"name": "agent-task", "checkpoint": "cp-7"})
    );

    let order: Vec<String> = server
        .received_requests()
        .await
        .unwrap()
        .iter()
        .filter(|r| r.method.as_str() == "POST")
        .map(|r| r.url.path().rsplit('/').next().unwrap().to_string())
        .collect();
    assert_eq!(
        order,
        ["pause", "fork", "resume"],
        "source resumes as soon as the fork is accepted"
    );
    assert_eq!(
        s.status,
        SandboxStatus::Running,
        "source was waited back to running"
    );
}

#[tokio::test]
async fn fork_of_an_inactive_sandbox_leaves_it_alone() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    server.reset().await;
    let mut stopped = sb("stopped");
    stopped["checkpoint"] = "cp-s".into();
    Mock::given(method("POST"))
        .and(path(p("/pause")))
        .respond_with(ok(stopped))
        .expect(1)
        .mount(&server)
        .await;
    let (cap, bodies) = Capture::new(accepted(sandbox_json(
        "33333333-3333-3333-3333-333333333333",
        "pending",
    )));
    Mock::given(method("POST"))
        .and(path(p("/fork")))
        .and(query_param("wait", "true"))
        .respond_with(cap)
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/api/sandboxes/33333333-3333-3333-3333-333333333333"))
        .respond_with(ok(sandbox_json(
            "33333333-3333-3333-3333-333333333333",
            "running",
        )))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(p("/resume")))
        .respond_with(accepted(sb("pending")))
        .expect(0)
        .mount(&server)
        .await;
    let fork = s.fork().await.unwrap();
    assert_eq!(fork.status, SandboxStatus::Running);
    assert_eq!(bodies.lock().unwrap()[0], json!({"checkpoint": "cp-s"}));
    assert_eq!(s.status, SandboxStatus::Stopped);
}

#[tokio::test]
async fn failed_fork_still_resumes_the_source() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    server.reset().await;
    Mock::given(method("POST"))
        .and(path(p("/pause")))
        .respond_with(accepted(sb("pausing")))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(sb("paused")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(p("/fork")))
        .respond_with(err(409, "sandbox has mounts", Some("conflict")))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(p("/resume")))
        .respond_with(accepted(sb("pending")))
        .expect(1)
        .mount(&server)
        .await;
    let e = s.fork().await.unwrap_err();
    assert_eq!(e.code(), Some("conflict"));
}

#[tokio::test]
async fn ports_expose_list_unexpose() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    Mock::given(method("PUT"))
        .and(path(p("/ports/8080")))
        .respond_with(ok(
            json!({"port": 8080, "hostname": "8080-abc.example.com"}),
        ))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("/ports")))
        .respond_with(ok(
            json!({"ports": [{"port": 8080, "hostname": "8080-abc.example.com"}]}),
        ))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(p("/ports/8080")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    assert_eq!(s.expose_port(8080).await.unwrap(), "8080-abc.example.com");
    let ports = s.list_ports().await.unwrap();
    assert_eq!(
        (ports[0].port, ports[0].hostname.as_str()),
        (8080, "8080-abc.example.com")
    );
    s.unexpose_port(8080).await.unwrap();
}

fn token(id: &str) -> Value {
    json!({"id": id, "port": 8080, "created_at": "2026-01-01T00:00:00Z", "expires_at": "2026-01-01T01:00:00Z"})
}

#[tokio::test]
async fn port_tokens_create_get_list_delete() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    let mut created = token("tok-1");
    created["hostname"] = "8080-abc.example.com".into();
    created["token"] = "raw-secret".into();
    Mock::given(method("POST"))
        .and(path(p("/port-tokens")))
        .and(body_json(json!({"port": 8080, "ttl": "1h"})))
        .respond_with(status(201, created))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(p("/port-tokens")))
        .and(body_json(json!({"port": 9090})))
        .respond_with(status(201, json!({"id": "t2", "port": 9090, "created_at": "2026-01-01T00:00:00Z", "hostname": "h", "token": "s2"})))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("/port-tokens/tok-1")))
        .respond_with(ok(token("tok-1")))
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path(p("/port-tokens/tok-1")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;

    let t = s
        .create_port_token(
            8080,
            CreateSandboxPortTokenOptions {
                ttl: Some("1h".into()),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        (
            t.token.as_str(),
            t.hostname.as_str(),
            t.token_info.id.as_str()
        ),
        ("raw-secret", "8080-abc.example.com", "tok-1")
    );
    assert!(t.token_info.expires_at.is_some());
    assert!(
        !format!("{t:?}").contains("raw-secret"),
        "Debug output must not leak the token"
    );
    let forever = s
        .create_port_token(9090, CreateSandboxPortTokenOptions::default())
        .await
        .unwrap();
    assert!(forever.token_info.expires_at.is_none());
    assert_eq!(s.get_port_token("tok-1").await.unwrap().port, 8080);
    s.delete_port_token("tok-1").await.unwrap();
}

#[tokio::test]
async fn port_token_listing_follows_cursors_and_honors_limit() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    let page = |ids: &[&str], next: Option<&str>| {
        let mut b = json!({"success": true, "data": {"tokens": ids.iter().map(|i| token(i)).collect::<Vec<_>>()}});
        if let Some(n) = next {
            b["nextCursor"] = n.into();
        }
        ResponseTemplate::new(200).set_body_json(b)
    };
    Mock::given(method("GET"))
        .and(path(p("/port-tokens")))
        .and(query_param_is_missing("cursor"))
        .and(query_param("limit", "100"))
        .respond_with(page(&["a", "b"], Some("c1")))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path(p("/port-tokens")))
        .and(query_param("cursor", "c1"))
        .respond_with(page(&["c"], None))
        .expect(1)
        .mount(&server)
        .await;
    let all = s
        .list_port_tokens(ListSandboxPortTokensOptions::default())
        .await
        .unwrap();
    assert_eq!(
        all.iter().map(|t| t.id.as_str()).collect::<Vec<_>>(),
        ["a", "b", "c"]
    );

    let server = MockServer::start().await;
    let s = running(&server).await;
    Mock::given(method("GET"))
        .and(path(p("/port-tokens")))
        .and(query_param("limit", "1"))
        .respond_with(page(&["a"], Some("more")))
        .expect(1)
        .mount(&server)
        .await;
    let capped = s
        .list_port_tokens(ListSandboxPortTokensOptions {
            limit: Some(1),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(capped.len(), 1, "limit caps the total and stops paging");
    Mock::given(method("GET"))
        .and(path(p("/port-tokens")))
        .and(query_param("limit", "10"))
        .respond_with(page(&["a"], Some("more")))
        .mount(&server)
        .await;
    let pg = s
        .list_port_tokens_page(ListSandboxPortTokensOptions {
            limit: Some(10),
            cursor: None,
        })
        .await
        .unwrap();
    assert_eq!(pg.next_cursor.as_deref(), Some("more"));
}

#[tokio::test]
async fn network_get_and_update() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    Mock::given(method("GET"))
        .and(path(p("/network")))
        .respond_with(ok(
            json!({"managed": true, "egress": {"default": "deny", "allow": ["pypi.org"]}}),
        ))
        .mount(&server)
        .await;
    let net = s.get_network().await.unwrap();
    assert_eq!(net.managed, Some(true));
    assert_eq!(
        net.egress.as_ref().unwrap().default,
        SandboxNetworkAction::Deny
    );

    let (cap, bodies) = Capture::new(ok(json!({"egress": {"default": "allow"}})));
    Mock::given(method("PUT"))
        .and(path(p("/network")))
        .respond_with(cap)
        .mount(&server)
        .await;
    let out = s
        .update_network(&SandboxNetwork {
            egress: Some(SandboxEgressPolicy::new(SandboxNetworkAction::Allow)),
            managed: Some(true),
        })
        .await
        .unwrap();
    assert_eq!(out.egress.unwrap().default, SandboxNetworkAction::Allow);
    // `managed` is ignored in requests, so it is never sent.
    assert_eq!(
        bodies.lock().unwrap()[0],
        json!({"egress": {"default": "allow"}})
    );
    s.update_network(&SandboxNetwork::default()).await.unwrap();
    assert_eq!(bodies.lock().unwrap()[1], json!({}));
}

#[tokio::test]
async fn network_update_plan_error_is_surfaced() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    Mock::given(method("PUT"))
        .and(path(p("/network")))
        .respond_with(err(
            403,
            "upgrade required",
            Some("sandbox_network_plan_required"),
        ))
        .mount(&server)
        .await;
    let e = s
        .update_network(&SandboxNetwork::default())
        .await
        .unwrap_err();
    assert_eq!(
        (e.status(), e.code()),
        (Some(403), Some("sandbox_network_plan_required"))
    );
}

#[tokio::test]
async fn set_timeout_sends_only_provided_fields_and_updates_in_place() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    let mut updated = sb("running");
    updated["max_ttl_seconds"] = 7200.into();
    updated["idle_ttl_seconds"] = 90.into();
    let (cap, bodies) = Capture::new(ok(updated));
    Mock::given(method("POST"))
        .and(path(p("/timeout")))
        .respond_with(cap)
        .mount(&server)
        .await;
    s.set_timeout(3600u64).await.unwrap();
    s.set_timeout(SandboxTimeoutOptions {
        timeout_seconds: None,
        idle_ttl_seconds: Some(0),
    })
    .await
    .unwrap();
    s.set_timeout(SandboxTimeoutOptions {
        timeout_seconds: Some(10),
        idle_ttl_seconds: Some(5),
    })
    .await
    .unwrap();
    assert_eq!(
        *bodies.lock().unwrap(),
        [
            json!({"timeout": 3600}),
            json!({"idle_ttl_seconds": 0}),
            json!({"timeout": 10, "idle_ttl_seconds": 5})
        ]
    );
    assert_eq!((s.max_ttl_seconds, s.idle_ttl_seconds), (7200, 90));
}

#[tokio::test]
async fn refresh_updates_in_place_and_delete_accepts_204() {
    let server = MockServer::start().await;
    let mut s = running(&server).await;
    server.reset().await;
    let mut v = sb("exited");
    v["exit_reason"] = "oom".into();
    Mock::given(method("GET"))
        .and(path(p("")))
        .respond_with(ok(v))
        .mount(&server)
        .await;
    s.refresh().await.unwrap();
    assert_eq!(
        (s.status, s.exit_reason.as_deref()),
        (SandboxStatus::Exited, Some("oom"))
    );
    Mock::given(method("DELETE"))
        .and(path(p("")))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    s.delete().await.unwrap();
}

#[tokio::test]
async fn delete_with_children_reports_the_conflict() {
    let server = MockServer::start().await;
    let s = running(&server).await;
    Mock::given(method("DELETE"))
        .and(path(p("")))
        .respond_with(err(409, "has forks", Some("sandbox_has_children")))
        .mount(&server)
        .await;
    let e = s.delete().await.unwrap_err();
    assert_eq!(
        (e.status(), e.code()),
        (Some(409), Some("sandbox_has_children"))
    );
}
