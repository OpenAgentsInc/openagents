use super::*;
use crate::resident::{self, Operation, Outcome, Request, Response, Server};
use std::{
    io::Write,
    os::unix::{fs::PermissionsExt, net::UnixStream},
    time::{Duration, Instant},
};
fn roundtrip(
    server: &mut Server<'_>,
    socket: &std::path::Path,
    request: Request,
    now: u64,
) -> Response {
    let path = socket.to_path_buf();
    let client = std::thread::spawn(move || resident::request(&path, &request));
    let started = Instant::now();
    while !client.is_finished() {
        assert!(started.elapsed() < Duration::from_secs(5));
        server.step(now).unwrap();
        std::thread::sleep(Duration::from_millis(1));
    }
    client.join().unwrap().unwrap()
}
fn inspect(
    server: &mut Server<'_>,
    socket: &std::path::Path,
    who: u8,
    body: &Value,
    now: u64,
) -> crate::Result<Value> {
    let event = workspace_host::host::request(&secret(who), &pk(HOST), body.clone(), now).unwrap();
    match roundtrip(
        server,
        socket,
        Request::new(Operation::Inspect { event }),
        now,
    )
    .outcome
    {
        Outcome::Ok { reply } => resident::read_reply(&reply, &pk(HOST), &secret(who), body),
        Outcome::Error { code } => Err(code),
    }
}
#[test]
fn resident_owns_one_broker_and_serves_only_signed_scoped_reads() {
    let mut fx = Fx::new();
    let c = fx.controller();
    fx.active(&c);
    let device = tracked_input(&c);
    let socket = fx.root.join("live.sock");
    let mut server = Server::bind(&c, socket.clone()).unwrap();
    assert_eq!(
        std::fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(Server::bind(&c, fx.root.join("other.sock")).is_err());
    // An incomplete client must not block another signed request.
    let mut partial = UnixStream::connect(&socket).unwrap();
    partial.write_all(b"{").unwrap();
    let query = json!({"v":"openagents.livehost-operation.v1","requires":[]});
    assert_eq!(
        inspect(&mut server, &socket, STRANGER, &query, fx.now).unwrap_err(),
        "not_admitted"
    );
    let event =
        workspace_host::host::request(&secret(OWNER), &pk(HOST), query.clone(), fx.now).unwrap();
    let response = roundtrip(
        &mut server,
        &socket,
        Request::new(Operation::Inspect { event }),
        fx.now,
    );
    let Outcome::Ok { reply } = response.outcome else {
        panic!("operation reply");
    };
    let operation = resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &query).unwrap();
    assert!(resident::read_reply(&reply, &pk(STRANGER), &secret(OWNER), &query).is_err());
    let mut corrupt = reply.clone();
    corrupt.artifacts.values_mut().next().unwrap().push(0);
    assert!(resident::read_reply(&corrupt, &pk(HOST), &secret(OWNER), &query).is_err());
    let cap = operation["operation"]["artifact"].clone();
    let read = json!({"v":"openagents.livehost-read.v1","requires":[],"artifacts":[cap]});
    assert!(inspect(&mut server, &socket, OWNER, &read, fx.now).is_ok());
    assert_eq!(
        inspect(&mut server, &socket, ALICE, &read, fx.now).unwrap_err(),
        "not_admitted"
    );
    let credential =
        json!({"v":"openagents.livehost-credential.v1","requires":[],"session":fx.session});
    assert_eq!(
        inspect(&mut server, &socket, STRANGER, &credential, fx.now).unwrap_err(),
        "not_admitted"
    );
    let delivered = inspect(&mut server, &socket, ALICE, &credential, fx.now).unwrap();
    let token = delivered["token"].as_str().unwrap();
    c.with_adapters(|a| {
        assert_eq!(
            a.broker
                .redeem(token, &pk(ALICE), "mic", "publish", fx.now)
                .unwrap()
                .session,
            fx.session
        )
    });
    assert_eq!(
        inspect(&mut server, &socket, ALICE, &credential, fx.now).unwrap_err(),
        "unavailable"
    );
    for entry in std::fs::read_dir(fx.root.join("artifacts")).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(!bytes.windows(token.len()).any(|w| w == token.as_bytes()));
    }
    let bytes = std::fs::read(fx.root.join("state.json")).unwrap();
    assert!(!bytes.windows(token.len()).any(|w| w == token.as_bytes()));
    let policy = json!({"v":"openagents.livehost-input-policy.v1","requires":[],"request":fx.next(),"grants":[{
        "plan":fx.plan,"owner":pk(OWNER),"approver":pk(OWNER),"operation":operation["operation"],"expires_at":fx.now+1800}]});
    assert_eq!(
        inspect(&mut server, &socket, OWNER, &policy, fx.now).unwrap_err(),
        "not_admitted"
    );
    let first = inspect(&mut server, &socket, HOST, &policy, fx.now).unwrap();
    assert_eq!(
        inspect(&mut server, &socket, HOST, &policy, fx.now).unwrap(),
        first
    );
    let mut changed = policy;
    changed["grants"] = json!([]);
    assert_eq!(
        inspect(&mut server, &socket, HOST, &changed, fx.now).unwrap_err(),
        "idempotency_conflict"
    );
    let request = json!({"v":"openagents.live-capture.v1","requires":[],"request":fx.next(),"plan":fx.plan,
        "track":"screen","expected_revision":fx.revision(&c),"limits":[bound("storage_bytes",10000)],"purpose":"observation"});
    let response = roundtrip(
        &mut server,
        &socket,
        Request::new(Operation::Submit {
            bundle: fx.bundle(OWNER, &request, vec![]),
        }),
        fx.now,
    );
    let Outcome::Ok { reply } = response.outcome else {
        panic!("capture reply");
    };
    let result = resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &request).unwrap();
    assert_eq!(result["status"], "accepted");
    let read =
        json!({"v":"openagents.livehost-read.v1","requires":[],"artifacts":[result["value"]]});
    assert!(inspect(&mut server, &socket, OWNER, &read, fx.now).is_ok());
    assert!(resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &read).is_err());
    let proposal_ref = input_proposal(&mut fx, &c, &result["value"], b"{\"click\":[10,20]}");
    let proposal_body: Value =
        serde_json::from_slice(&fx.artifacts[proposal_ref["digest"].as_str().unwrap()]).unwrap();
    let response = roundtrip(
        &mut server,
        &socket,
        Request::new(Operation::Submit {
            bundle: fx.bundle(OWNER, &proposal_body, vec![]),
        }),
        fx.now,
    );
    let Outcome::Ok { reply } = response.outcome else {
        panic!("prepare reply");
    };
    let prepared = resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &proposal_body).unwrap();
    assert_eq!(prepared["status"], "accepted");
    assert!(device.borrow().applied.is_empty());
    let approval_request = prepared["value"].clone();
    let read =
        json!({"v":"openagents.livehost-read.v1","requires":[],"artifacts":[approval_request]});
    let event =
        workspace_host::host::request(&secret(OWNER), &pk(HOST), read.clone(), fx.now).unwrap();
    let response = roundtrip(
        &mut server,
        &socket,
        Request::new(Operation::Inspect { event }),
        fx.now,
    );
    let Outcome::Ok { reply } = response.outcome else {
        panic!("approval request bytes");
    };
    resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &read).unwrap();
    let request_body: Value =
        serde_json::from_slice(&reply.artifacts[approval_request["digest"].as_str().unwrap()])
            .unwrap();
    let action = request_body["action"].clone();
    fx.approvals
        .insert(action["digest"].as_str().unwrap().into(), approval_request);
    let admission = admission(&mut fx, &c, &proposal_ref, &action, b"approve");
    for status in ["accepted", "duplicate"] {
        let response = roundtrip(
            &mut server,
            &socket,
            Request::new(Operation::Submit {
                bundle: fx.bundle(OWNER, &admission, vec![]),
            }),
            fx.now,
        );
        let Outcome::Ok { reply } = response.outcome else {
            panic!("input reply");
        };
        let result = resident::read_reply(&reply, &pk(HOST), &secret(OWNER), &admission).unwrap();
        assert_eq!(result["status"], status, "{result}");
        assert_eq!(device.borrow().applied.len(), 1);
    }
    drop(server);
    assert!(!socket.exists());
}
