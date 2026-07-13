use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

const TOKEN: &str = "portable-agent-computer-contract-token";

struct Daemon {
    child: Child,
    addr: String,
    state_dir: PathBuf,
}

impl Drop for Daemon {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = std::fs::remove_dir_all(&self.state_dir);
    }
}

fn start_daemon() -> Daemon {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let addr = format!("127.0.0.1:{port}");
    let state_dir = std::env::temp_dir().join(format!(
        "oa-portable-agent-computer-contract-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let auth_root = state_dir.join("auth");
    std::fs::create_dir_all(&auth_root).unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_oa-codex-control"))
        .env("OA_CODEX_CONTROL_TOKEN", TOKEN)
        .env("OA_CODEX_CONTROL_BIND", &addr)
        .env("OA_CODEX_CONTROL_STATE_ROOT", &state_dir)
        .env("OA_CODEX_AUTH_JSON_ROOT", &auth_root)
        .env("OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY", "true")
        .env("OA_CLOUD_VM_PROVISIONER", "fake")
        .spawn()
        .unwrap();
    let daemon = Daemon {
        child,
        addr,
        state_dir,
    };
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if request(&daemon.addr, "GET", "/healthz", None, None)
            .is_ok_and(|(status, _)| status == 200)
        {
            return daemon;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    panic!("daemon failed to start")
}

fn request(
    addr: &str,
    method: &str,
    path: &str,
    body: Option<&[u8]>,
    bearer: Option<&str>,
) -> Result<(u16, Value), String> {
    let mut stream = TcpStream::connect(addr).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    let body = body.unwrap_or_default();
    let mut head = format!(
        "{method} {path} HTTP/1.1\r\nhost: {addr}\r\ncontent-length: {}\r\nconnection: close\r\n",
        body.len()
    );
    if let Some(token) = bearer {
        head.push_str(&format!("authorization: Bearer {token}\r\n"));
    }
    if !body.is_empty() {
        head.push_str("content-type: application/json\r\n");
    }
    head.push_str("\r\n");
    stream
        .write_all(head.as_bytes())
        .map_err(|error| error.to_string())?;
    stream.write_all(body).map_err(|error| error.to_string())?;
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let split = response
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .unwrap();
    let status = String::from_utf8_lossy(&response[..split])
        .lines()
        .next()
        .and_then(|line| line.split_whitespace().nth(1))
        .and_then(|value| value.parse().ok())
        .unwrap();
    let value = serde_json::from_slice(&response[split + 4..]).unwrap_or(Value::Null);
    Ok((status, value))
}

fn post(daemon: &Daemon, value: &Value) -> (u16, Value) {
    let body = serde_json::to_vec(value).unwrap();
    request(
        &daemon.addr,
        "POST",
        "/v1/portable-agent-computers/operations",
        Some(&body),
        Some(TOKEN),
    )
    .unwrap()
}

fn base(action: &str, operation_ref: &str, resource_ref: Option<&str>) -> Value {
    json!({
        "operationRef": operation_ref,
        "action": action,
        "ownerRef": "owner.port03.http",
        "targetRef": "target.port03.http.managed",
        "sessionRef": "session.port03.http",
        "attachmentRef": "attachment.port03.http.managed",
        "generation": 2,
        "resourceRef": resource_ref,
        "payload": {}
    })
}

#[test]
fn retained_http_route_stages_replays_activates_and_reclaims() {
    let daemon = start_daemon();
    let mut stage = base("stage", "operation.port03.http.stage", None);
    stage["payload"] = json!({
        "bundle": {
            "checkpoint": {
                "digest": format!("sha256:{}", "d".repeat(64)),
                "repositoryPostImageDigest": format!("sha256:{}", "a".repeat(64)),
                "diffDigest": format!("sha256:{}", "b".repeat(64)),
                "graphDigest": format!("sha256:{}", "c".repeat(64))
            },
            "executionBinding": { "runRef": "run.port03.http" },
            "graph": {
                "rootAgentRef": "agent.port03.http.root",
                "nodes": [{ "agentRef": "agent.port03.http.root" }]
            },
            "threadCursors": [{
                "threadRef": "thread.port03.http.root",
                "transcriptRef": "transcript.port03.http.root",
                "activityCursor": 1,
                "eventCursor": 1
            }]
        },
        "capabilityLeaseRefs": ["lease.port03.http.provider"]
    });
    let (status, staged) = post(&daemon, &stage);
    assert_eq!(status, 200, "stage response: {staged}");
    assert_eq!(staged["acceptingWork"], false);
    assert_eq!(post(&daemon, &stage).1, staged);
    let resource_ref = staged["resourceRef"].as_str().unwrap();

    let mut activate = base(
        "activate",
        "operation.port03.http.activate",
        Some(resource_ref),
    );
    activate["payload"] = json!({
        "checkpointRef": "checkpoint.port03.http.source",
        "authorityEvidenceRef": "evidence.port03.http.authority",
        "capabilityLeaseRefs": ["lease.port03.http.provider"]
    });
    assert_eq!(post(&daemon, &activate).0, 200);

    let mut quiesce = base(
        "quiesce",
        "operation.port03.http.quiesce",
        Some(resource_ref),
    );
    quiesce["payload"] = stage["payload"]["bundle"]["graph"].clone();
    quiesce["payload"] = json!({ "graph": quiesce["payload"].clone() });
    assert_eq!(post(&daemon, &quiesce).0, 200);

    let mut reclaim = base(
        "reclaim",
        "operation.port03.http.reclaim",
        Some(resource_ref),
    );
    reclaim["payload"] = json!({ "agentRefs": ["agent.port03.http.root"] });
    let (status, reclaimed) = post(&daemon, &reclaim);
    assert_eq!(status, 200);
    assert_eq!(reclaimed["scratch"], "released");
    assert_eq!(post(&daemon, &reclaim).1, reclaimed);

    let journal = std::fs::read_to_string(
        daemon
            .state_dir
            .join("portable-agent-computers/resources")
            .read_dir()
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path(),
    )
    .unwrap();
    assert!(!journal.contains(TOKEN));
    assert!(!journal.contains("/Users/"));
}

#[test]
fn retained_http_route_requires_authentication_and_rejects_conflicting_bytes() {
    let daemon = start_daemon();
    let body = serde_json::to_vec(&base("stage", "operation.port03.http.auth", None)).unwrap();
    assert_eq!(
        request(
            &daemon.addr,
            "POST",
            "/v1/portable-agent-computers/operations",
            Some(&body),
            None,
        )
        .unwrap()
        .0,
        401
    );
}
