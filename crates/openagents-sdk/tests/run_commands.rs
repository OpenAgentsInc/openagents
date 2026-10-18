use axum::{
    Json, Router,
    extract::Path,
    http::HeaderMap,
    routing::{get, post},
};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error,
    runs::{CommandOutcome, CommandReceipt, CommandRefusal, RunCommand, SessionCommand},
};
use serde_json::json;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

#[tokio::test]
async fn command_auth_path_all_actions_retries_refusals_and_reply_binding() {
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = received.clone();
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(
                    json!({"account":{"id":"commands-account","label":"Commands"},"workspaces":[]}),
                )
            }),
        )
        .route(
            "/v1/runs/{id}/commands",
            post(
                move |headers: HeaderMap,
                      Path(run): Path<String>,
                      Json(request): Json<RunCommand>| {
                    let sink = sink.clone();
                    async move {
                        assert_eq!(
                            headers["authorization"],
                            format!("Bearer oa_sk_{}", "a".repeat(64))
                        );
                        sink.lock().unwrap().push(request.clone());
                        Json(CommandReceipt {
                            run: if request.request_id == "wrong" {
                                "other".into()
                            } else {
                                run
                            },
                            request_id: request.request_id.clone(),
                            outcome: if request.request_id == "refused" {
                                CommandOutcome::Refused {
                                    reason: CommandRefusal::NotSteerable,
                                }
                            } else {
                                CommandOutcome::Accepted
                            },
                        })
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        Duration::from_secs(3),
    )
    .unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let runs = client.runs(&session).unwrap();
    let cancel = CancellationToken::new();
    for (i, command) in [
        SessionCommand::Message {
            text: "Hello".into(),
        },
        SessionCommand::Steer {
            text: "Check this".into(),
        },
        SessionCommand::Stop,
        SessionCommand::Restart {
            brief: Some("Continue here".into()),
        },
        SessionCommand::Restart { brief: None },
    ]
    .into_iter()
    .enumerate()
    {
        let id = format!("request:{i}");
        let receipt = runs
            .command("run:1", &id, command.clone(), &cancel)
            .await
            .unwrap();
        assert_eq!(receipt.outcome, CommandOutcome::Accepted);
        assert_eq!(
            runs.command("run:1", &id, command, &cancel).await.unwrap(),
            receipt
        );
    }
    assert_eq!(
        runs.command("terminal", "refused", SessionCommand::Stop, &cancel)
            .await
            .unwrap()
            .outcome,
        CommandOutcome::Refused {
            reason: CommandRefusal::NotSteerable
        }
    );
    assert_eq!(
        runs.command("run", "wrong", SessionCommand::Stop, &cancel)
            .await,
        Err(Error::InvalidResponse)
    );
    let before = received.lock().unwrap().len();
    for (run, id, command) in [
        ("../run", "request", SessionCommand::Stop),
        ("run", "..", SessionCommand::Stop),
        ("run", "id", SessionCommand::Message { text: " ".into() }),
        (
            "run",
            "id",
            SessionCommand::Restart {
                brief: Some("\0".into()),
            },
        ),
    ] {
        assert_eq!(
            runs.command(run, id, command, &cancel).await,
            Err(Error::InvalidReference)
        );
    }
    cancel.cancel();
    assert_eq!(
        runs.command("run", "cancel", SessionCommand::Stop, &cancel)
            .await,
        Err(Error::Cancelled)
    );
    assert_eq!(received.lock().unwrap().len(), before);
    server.abort();
}

#[test]
fn command_wire_rejects_unknown_fields_and_bounds_text_in_bytes() {
    for value in [
        json!({"kind":"stop","text":"extra"}),
        json!({"kind":"shell","text":"no"}),
        json!({"kind":"message"}),
    ] {
        assert!(serde_json::from_value::<SessionCommand>(value).is_err());
    }
    assert!(
        serde_json::from_value::<CommandOutcome>(json!({"status":"accepted","reason":"extra"}))
            .is_err()
    );
    assert!(
        !SessionCommand::Message {
            text: "é".repeat(8193)
        }
        .valid()
    );
    assert!(
        SessionCommand::Message {
            text: "é".repeat(8192)
        }
        .valid()
    );
    for reason in [
        CommandRefusal::NotSteerable,
        CommandRefusal::NoGrant,
        CommandRefusal::RunEnded,
        CommandRefusal::Offline,
        CommandRefusal::Unavailable,
    ] {
        let receipt = CommandReceipt {
            run: "run".into(),
            request_id: "request".into(),
            outcome: CommandOutcome::Refused { reason },
        };
        assert_eq!(
            serde_json::from_value::<CommandReceipt>(serde_json::to_value(&receipt).unwrap())
                .unwrap(),
            receipt
        );
    }
}
