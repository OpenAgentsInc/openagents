//! The local control protocol: wire shape, a round trip over a byte stream,
//! and refusals.

use std::path::Path;

use openagents_connect::Code;
use openagents_connect::control::{
    self, Autostart, Device, EngineReport, MAX_MESSAGE_BYTES, Op, Project, Reply, Request,
    Response, Status, VERSION, socket_path_for,
};
use openagents_connect::wire::{read_message, write_message};
use serde_json::json;
use tokio::io::AsyncWriteExt;

fn every_op() -> Vec<Op> {
    vec![
        Op::TaskActivity {
            task: "11".repeat(32),
        },
        Op::Status {},
        Op::InviteCreate {},
        Op::InviteCancel {
            invitation: "11".repeat(32),
        },
        Op::InviteCancelAll {},
        Op::DeviceList {},
        Op::DeviceRevoke {
            device: "22".repeat(32),
        },
        Op::AutostartGet {},
        Op::AutostartSet {
            policy: Autostart {
                enabled: true,
                projects: vec!["openagents".into()],
                max_running: 1,
            },
        },
        Op::ProjectList {},
        Op::ProjectAdd {
            path: "/Users/kai/work/openagents".into(),
        },
        Op::ProjectRemove {
            label: "openagents".into(),
        },
        Op::OwnerImport {
            secret: "33".repeat(32),
        },
        Op::EngineStatus {},
        Op::EngineRefresh {
            providers: vec!["claude".into()],
        },
        Op::ChatMigrate {
            home: "/Users/kai/.openagents/chat".into(),
        },
    ]
}

#[test]
fn local_activity_uses_the_phone_summary_contract_and_rejects_extra_content() {
    use nostr::activity_summary::{self, Attention, Phase, SubjectKind, SummaryDraft};
    let host = "a".repeat(64);
    let task = "b".repeat(64);
    let summary = activity_summary::encode(&SummaryDraft {
        host: &host,
        subject_kind: SubjectKind::Task,
        subject: &task,
        sequence: 4,
        phase: Phase::Waiting,
        headline: "Coder asked a question",
        attention: Attention::Input,
        updated_at: 1_790_000_000,
    })
    .unwrap();
    let reply = Reply::TaskActivity { summary };
    let wire = serde_json::to_value(&reply).unwrap();
    assert_eq!(
        serde_json::from_value::<Reply>(wire.clone()).unwrap(),
        reply
    );
    let mut extra = wire;
    extra["summary"]["question"] = json!("private question text");
    assert!(serde_json::from_value::<Reply>(extra).is_err());
}

#[test]
fn requests_have_a_stable_wire_shape() {
    let request = Request::new(3, Op::InviteCreate {});
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({"v": VERSION, "id": 3, "op": {"kind": "invite_create"}})
    );
    let request = Request::new(4, Op::Status {});
    assert_eq!(
        serde_json::to_value(&request).unwrap(),
        json!({"v": VERSION, "id": 4, "op": {"kind": "status"}})
    );
    for op in every_op() {
        let request = Request::new(9, op);
        let text = serde_json::to_string(&request).unwrap();
        assert_eq!(serde_json::from_str::<Request>(&text).unwrap(), request);
    }
}

#[test]
fn unknown_operations_and_fields_do_not_parse() {
    for text in [
        json!({"v": VERSION, "id": 1, "op": {"kind": "shell"}}),
        json!({"v": VERSION, "id": 1, "op": {"kind": "status", "extra": 1}}),
        // Nothing on the wire narrows or widens a code's rights.
        json!({"v": VERSION, "id": 1, "op": {"kind": "invite_create", "terminal": false}}),
        json!({"v": VERSION, "id": 1, "op": {"kind": "invite_create", "rights": ["observe"]}}),
        json!({"v": VERSION, "id": 1, "op": {"kind": "nearby_decide", "id": 7, "connect": true, "terminal": false}}),
        json!({"v": VERSION, "id": 1, "op": {"kind": "status"}, "grant": "x"}),
        json!({"v": VERSION, "id": 1, "op": {"kind": "engine_status", "access_token": "x"}}),
    ] {
        assert!(
            serde_json::from_value::<Request>(text.clone()).is_err(),
            "{text}"
        );
    }
}

#[test]
fn an_engine_report_cannot_carry_a_credential() {
    let mut value = serde_json::to_value(EngineReport {
        enabled: false,
        adapter: String::new(),
        model: String::new(),
        routes: vec![],
        accounts: vec![],
        usage_probe: None,
        refresh_due: false,
    })
    .unwrap();
    value["access_token"] = json!("secret");
    assert!(serde_json::from_value::<EngineReport>(value).is_err());
}

#[test]
fn replies_round_trip() {
    let replies = vec![
        Reply::Status(Status {
            host: "aa".repeat(32),
            endpoint: "bb".repeat(32),
            label: "Studio Mac".into(),
            online: true,
            relay: Some("https://iroh.openagents.com/".into()),
            devices: 1,
            outstanding_invitations: 2,
            version: "0.1.0".into(),
        }),
        Reply::Invite {
            invitation: "11".repeat(32),
            code: "openagents-connect:AQ".into(),
            expires_at: 1_800_000_300,
            rights: vec!["observe".into(), "operate".into()],
        },
        Reply::Cancelled { count: 2 },
        Reply::Devices {
            devices: vec![Device {
                device: "22".repeat(32),
                label: "Kai's iPhone".into(),
                rights: vec!["observe".into(), "operate".into(), "terminal".into()],
                grant: "33".repeat(32),
                epoch: 0,
                enrolled_at: 1_800_000_010,
                last_seen: None,
                revoked: false,
            }],
        },
        Reply::Revoked {
            device: "22".repeat(32),
            epoch: 1,
        },
        Reply::Autostart {
            policy: Autostart {
                enabled: false,
                projects: vec![],
                max_running: 1,
            },
        },
        Reply::EngineStatus {
            report: EngineReport {
                enabled: false,
                adapter: String::new(),
                model: String::new(),
                routes: vec![],
                accounts: vec![],
                usage_probe: None,
                refresh_due: false,
            },
        },
        Reply::Projects {
            projects: vec![Project {
                label: "openagents".into(),
                path: "/Users/kai/.openagents/host/projects/openagents-1a2b3c4d".into(),
                folder: Some("/Users/kai/work/openagents".into()),
            }],
        },
        Reply::Owner {
            owner: "44".repeat(32),
        },
        Reply::ChatMigrated {
            moved: 3,
            present: 1,
        },
        Reply::Refused {
            code: "forbidden".into(),
            message: "that project is not a Git checkout".into(),
        },
    ];
    for reply in replies {
        let response = Response::new(5, reply);
        let text = serde_json::to_string(&response).unwrap();
        assert_eq!(serde_json::from_str::<Response>(&text).unwrap(), response);
    }
    assert_eq!(
        serde_json::to_value(Response::new(1, Reply::Cancelled { count: 0 })).unwrap(),
        json!({"v": VERSION, "id": 1, "result": {"kind": "cancelled", "count": 0}})
    );
}

#[tokio::test]
async fn call_and_serve_over_a_stream() {
    let (mut client, mut host) = tokio::io::duplex(4096);
    let server = tokio::spawn(async move {
        let mut served = Vec::new();
        while let Some(request) = control::next_request(&mut host).await.unwrap() {
            let reply = match &request.op {
                Op::InviteCancelAll {} => Reply::Cancelled { count: 3 },
                Op::DeviceRevoke { device } => Reply::Revoked {
                    device: device.clone(),
                    epoch: 1,
                },
                _ => Reply::Refused {
                    code: "unsupported_feature".into(),
                    message: "not in this test".into(),
                },
            };
            served.push(request.id);
            control::respond(&mut host, &Response::new(request.id, reply))
                .await
                .unwrap();
        }
        served
    });
    let reply = control::call(&mut client, &Request::new(1, Op::InviteCancelAll {}))
        .await
        .unwrap();
    assert_eq!(reply, Reply::Cancelled { count: 3 });
    let reply = control::call(
        &mut client,
        &Request::new(
            2,
            Op::DeviceRevoke {
                device: "22".repeat(32),
            },
        ),
    )
    .await
    .unwrap();
    assert_eq!(
        reply,
        Reply::Revoked {
            device: "22".repeat(32),
            epoch: 1
        }
    );
    drop(client);
    assert_eq!(server.await.unwrap(), vec![1, 2]);
}

#[tokio::test]
async fn a_response_for_another_request_or_version_is_refused() {
    let (mut client, mut host) = tokio::io::duplex(4096);
    tokio::spawn(async move {
        let request = control::next_request(&mut host).await.unwrap().unwrap();
        control::respond(
            &mut host,
            &Response::new(request.id + 1, Reply::Cancelled { count: 0 }),
        )
        .await
        .unwrap();
        let request = control::next_request(&mut host).await.unwrap().unwrap();
        let mut response = Response::new(request.id, Reply::Cancelled { count: 0 });
        response.v = "openagents.control.v2".into();
        control::respond(&mut host, &response).await.unwrap();
    });
    let error = control::call(&mut client, &Request::new(1, Op::Status {}))
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::Malformed);
    let error = control::call(&mut client, &Request::new(2, Op::Status {}))
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::UnsupportedVersion);
    // The host closed: a call now reports it.
    let error = control::call(&mut client, &Request::new(3, Op::Status {}))
        .await
        .unwrap_err();
    assert_eq!(error.code, Code::Unavailable);
}

#[tokio::test]
async fn oversized_and_malformed_messages_are_refused() {
    let (mut client, mut host) = tokio::io::duplex(1024);
    client
        .write_all(&((MAX_MESSAGE_BYTES as u32) + 1).to_be_bytes())
        .await
        .unwrap();
    assert_eq!(
        control::next_request(&mut host).await.unwrap_err().code,
        Code::Bounds
    );

    let (mut client, mut host) = tokio::io::duplex(1024);
    client.write_all(&5u32.to_be_bytes()).await.unwrap();
    client.write_all(b"nope!").await.unwrap();
    assert_eq!(
        control::next_request(&mut host).await.unwrap_err().code,
        Code::Malformed
    );

    let (mut client, mut host) = tokio::io::duplex(1024);
    write_message(&mut client, &Request::new(1, Op::Status {}), 8)
        .await
        .unwrap_err();
    let mut request = Request::new(1, Op::Status {});
    request.v = "openagents.control.v0".into();
    write_message(&mut client, &request, MAX_MESSAGE_BYTES)
        .await
        .unwrap();
    assert_eq!(
        control::next_request(&mut host).await.unwrap_err().code,
        Code::UnsupportedVersion
    );

    // A stream that ends inside a message.
    let (mut client, mut host) = tokio::io::duplex(1024);
    client.write_all(&10u32.to_be_bytes()).await.unwrap();
    client.write_all(b"{}").await.unwrap();
    drop(client);
    assert_eq!(
        read_message::<_, Request>(&mut host, MAX_MESSAGE_BYTES)
            .await
            .unwrap_err()
            .code,
        Code::Unavailable
    );
}

#[test]
fn socket_paths_follow_the_platform() {
    let home = Path::new("/Users/kai");
    let runtime = Path::new("/run/user/1000");
    assert_eq!(
        socket_path_for("macos", Some(home), None).unwrap(),
        Path::new("/Users/kai/Library/Application Support/OpenAgents/control.sock")
    );
    assert_eq!(
        socket_path_for("linux", Some(home), Some(runtime)).unwrap(),
        Path::new("/run/user/1000/openagents/control.sock")
    );
    assert_eq!(socket_path_for("linux", Some(home), None), None);
    assert_eq!(socket_path_for("windows", Some(home), Some(runtime)), None);
}
