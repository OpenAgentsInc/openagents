use super::*;
use axum::{Json, response::IntoResponse, routing::get};
use openagents_sdk::{
    CancellationToken, Client, Deployment,
    runs::{RunHeartbeat, SessionCommand},
    sessions::SessionStatus,
};
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
};
#[path = "../../../coder-control/src/tests/relay.rs"]
mod relay;

fn record() -> Record {
    Record {
        request: RunCommand {
            request_id: "request".into(),
            command: SessionCommand::Stop,
        },
        computer: "host".into(),
        binding: None,
        pending: None,
        receipt: Some(receipt("run", "request", CommandRefusal::NotSteerable)),
    }
}
#[tokio::test]
async fn retry_records_survive_restart_conflict_and_separate_accounts() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().join("store"));
    let owner = crate::chat_store::account_owner("alice");
    let other = crate::chat_store::account_owner("bob");
    let proposed = record();
    let key = request_key("run", "request");
    let (a, b) = tokio::join!(
        reserve(&store, &owner, &key, &proposed),
        reserve(&store, &owner, &key, &proposed)
    );
    assert!(a.unwrap().is_ok() && b.unwrap().is_ok());
    let reopened = Store::local(root.path().join("store"));
    assert_eq!(load(&reopened, &owner).await.unwrap().0.records.len(), 1);
    assert!(load(&reopened, &other).await.unwrap().0.records.is_empty());
    let mut changed = proposed.clone();
    changed.request.command = SessionCommand::Message {
        text: "Different".into(),
    };
    assert!(
        reserve(&reopened, &owner, &key, &changed)
            .await
            .unwrap()
            .is_err()
    );
    assert_ne!(request_key("other", "request"), key);
}

#[derive(Default)]
struct Commands(Mutex<Vec<serde_json::Value>>);
impl coder_access::app_rpc::Handler for Commands {
    fn call(
        &self,
        _: &str,
        _: &str,
        grant: Option<(&str, u64)>,
        request: &Request,
    ) -> Result<coder_access::app_rpc::Answer, coder_access::Code> {
        assert!(grant.is_some());
        assert_eq!(request.method, Method::RunCommand);
        assert_eq!(request.params["account"], "command-alice");
        self.0.lock().unwrap().push(request.params.clone());
        Ok(coder_access::app_rpc::Answer {
            method: request.method,
            value: json!(CommandReceipt {
                run: request.params["run"].as_str().unwrap().into(),
                request_id: request.params["request_id"].as_str().unwrap().into(),
                outcome: CommandOutcome::Accepted
            }),
        })
    }
}
fn protected(path: &std::path::Path, bytes: impl AsRef<[u8]>) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (base, task)
}
async fn auth(headers: HeaderMap) -> Response {
    let token = headers
        .get("authorization")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    let id = if token == format!("Bearer sess_{}", "a".repeat(64)) {
        "command-alice"
    } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
        "command-bob"
    } else {
        return StatusCode::UNAUTHORIZED.into_response();
    };
    Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]})).into_response()
}
async fn session(headers: HeaderMap) -> Response {
    let response = auth(headers).await;
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), 4096)
        .await
        .unwrap();
    if status != StatusCode::OK {
        return status.into_response();
    };
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    Json(json!({"session":{"id":"command-fixture","account":value["account"]["id"],"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn sdk_route_native_channel_all_commands_retries_terminal_and_missing_grant() {
    use coder_access::{RelayPolicy, Rights, protocol::HostInvitation};
    use coder_host::serve::keys::{FileKeySource, Keys};
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    let (relay, relay_task, _) = relay::start().await;
    let mut config = coder_host::Config::new(dir.join("native-access"), vec![relay.clone()], 1);
    config.policy = RelayPolicy::LoopbackTest;
    config.telemetry = false;
    config.keys = Some(Keys(Arc::new(FileKeySource::new(dir.join("native-keys")))));
    let host = coder_host::start(config, Arc::new(coder_host::NoTasks))
        .await
        .unwrap();
    let commands = Arc::new(Commands::default());
    host.set_app_rpc(Some(commands.clone()));
    let secret = secp256k1::SecretKey::new(&mut secp256k1::rand::rng());
    let now = crate::cloud::session::now();
    let invite = host
        .authority()
        .local(|h, t| {
            h.invite(&relay, Rights::parse_list("observe,operate")?, t, t + 3600)
                .map(|i| i.code)
        })
        .unwrap();
    let invitation = HostInvitation::parse(&invite, now, RelayPolicy::LoopbackTest).unwrap();
    let pending =
        coder_access::client::prepare_redeem(&invitation, &secret, now, RelayPolicy::LoopbackTest)
            .unwrap();
    let reply = host
        .authority()
        .local(|h, t| h.handle_redemption(&pending.event, || Ok(t)))
        .unwrap();
    let access = coder_access::client::finish_redeem(
        &invitation,
        &pending,
        &reply,
        &secret,
        now,
        RelayPolicy::LoopbackTest,
    )
    .unwrap();
    let access_path = dir.join("device-access.json");
    let secret_path = dir.join("device-secret");
    protected(&access_path, serde_json::to_vec(&access).unwrap());
    protected(&secret_path, secret.secret_bytes());
    let effects = dir.join("effects");
    std::fs::create_dir(&effects).unwrap();
    std::fs::set_permissions(&effects, std::fs::Permissions::from_mode(0o700)).unwrap();
    let binding_path = dir.join("bindings.json");
    protected(&binding_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.host-bindings.v1","bindings":[{"id":"scratch-host","account":"command-alice","workspace":"workspace","members_epoch":1,"host_workspace":"checkout","host_generation":1,"route":format!("tcp://{}",host.local_addr()),"access_file":access_path,"device_secret":secret_path}],"controls":{"directory":effects,"bindings":["scratch-host"]}})).unwrap());
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", get(auth))
            .route("/v1/session", get(session)),
    )
    .await;
    let csrf = dir.join("csrf");
    protected(&csrf, [42; 32]);
    let cloud_path = dir.join("cloud.json");
    protected(&cloud_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":csrf})).unwrap());
    let mut config = crate::Config::development(dir.join("tasks"));
    config.chat_store = Arc::new(Store::local(dir.join("chats")));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&cloud_path).unwrap(),
    ));
    config.cloud_hosts = Some(Arc::new(
        crate::cloud::hosts::Hosts::load(&binding_path).unwrap(),
    ));
    let app = App(Arc::new(crate::Inner { config }));
    let (base, api_task) = serve(
        routes()
            .merge(crate::runs::routes())
            .with_state(app.clone())
            .route("/v1/account", get(auth)),
    )
    .await;
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        Duration::from_secs(10),
    )
    .unwrap();
    let alice = client
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    let bob = client
        .authenticate(&format!("sess_{}", "b".repeat(64)))
        .await
        .unwrap();
    let runs = client.runs(&alice).unwrap();
    let cancel = CancellationToken::new();
    let mut beat = RunHeartbeat {
        run: "app-run".into(),
        kind: RunKind::AppChat,
        harness: "any".into(),
        harness_session: Some("native-session".into()),
        computer: "scratch-host".into(),
        issues: vec![],
        status: SessionStatus::Working,
        last_output_at: 0,
        last_progress_at: None,
        note: None,
        agent: None,
    };
    runs.heartbeat(&beat, &cancel).await.unwrap();
    let started = std::time::Instant::now();
    for (i, command) in [
        SessionCommand::Message {
            text: "Hello".into(),
        },
        SessionCommand::Steer {
            text: "Review".into(),
        },
        SessionCommand::Stop,
        SessionCommand::Restart {
            brief: Some("Continue".into()),
        },
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("request-{i}");
        let result = runs
            .command(&beat.run, &id, command.clone(), &cancel)
            .await
            .unwrap();
        assert_eq!(result.outcome, CommandOutcome::Accepted);
        if i == 0 {
            assert!(started.elapsed() < Duration::from_secs(10));
        }
        assert_eq!(
            runs.command(&beat.run, &id, command, &cancel)
                .await
                .unwrap(),
            result
        );
    }
    assert_eq!(commands.0.lock().unwrap().len(), 4);
    assert!(matches!(
        runs.command(&beat.run, "request-0", SessionCommand::Stop, &cancel)
            .await,
        Err(openagents_sdk::Error::Refused {
            status: 409,
            code: Some("command_conflict")
        })
    ));
    assert_eq!(
        client
            .runs(&bob)
            .unwrap()
            .command(&beat.run, "other", SessionCommand::Stop, &cancel)
            .await,
        Err(openagents_sdk::Error::Refused {
            status: 404,
            code: Some("not_found")
        })
    );
    beat.run = "terminal-run".into();
    beat.kind = RunKind::Terminal;
    runs.heartbeat(&beat, &cancel).await.unwrap();
    assert_eq!(
        runs.command(&beat.run, "stop", SessionCommand::Stop, &cancel)
            .await
            .unwrap()
            .outcome,
        CommandOutcome::Refused {
            reason: CommandRefusal::NotSteerable
        }
    );
    beat.run = "ungranted-run".into();
    beat.kind = RunKind::AppChat;
    beat.computer = "no-binding".into();
    runs.heartbeat(&beat, &cancel).await.unwrap();
    assert_eq!(
        runs.command(&beat.run, "stop", SessionCommand::Stop, &cancel)
            .await
            .unwrap()
            .outcome,
        CommandOutcome::Refused {
            reason: CommandRefusal::NoGrant
        }
    );
    host.authority()
        .local(|h, t| h.revoke(&access.grant.device, t))
        .unwrap();
    assert_eq!(
        runs.command("app-run", "revoked", SessionCommand::Stop, &cancel)
            .await
            .unwrap()
            .outcome,
        CommandOutcome::Refused {
            reason: CommandRefusal::NoGrant
        }
    );
    assert_eq!(commands.0.lock().unwrap().len(), 4);
    runs.end("app-run", &cancel).await.unwrap();
    assert_eq!(
        runs.command("app-run", "ended", SessionCommand::Stop, &cancel)
            .await
            .unwrap()
            .outcome,
        CommandOutcome::Refused {
            reason: CommandRefusal::RunEnded
        }
    );
    api_task.abort();
    auth_task.abort();
    host.shutdown().await;
    relay_task.abort();
}
#[test]
fn openapi_declares_command_body_receipt_and_security() {
    let doc = crate::app_openapi::document();
    let op = &doc["paths"]["/v1/runs/{id}/commands"]["post"];
    assert_eq!(op["operationId"], "commandRun");
    assert_eq!(op["security"][0], json!({"appBearer":[]}));
    assert_eq!(
        doc["components"]["schemas"]["SessionCommand"]["oneOf"]
            .as_array()
            .unwrap()
            .len(),
        4
    );
}
