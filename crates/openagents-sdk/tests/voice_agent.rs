use axum::{Json, Router, routing::get};
use openagents_sdk::{
    CancellationToken, Client, Deployment, Error,
    agents::voice::{SetVoiceAgent, VoiceAgent, VoiceTarget},
};
use serde_json::json;
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn two_devices_read_the_same_choice_and_prepared_requests_stay_account_bound() {
    let choice = Arc::new(Mutex::new(VoiceAgent::default()));
    let read = choice.clone();
    let write = choice.clone();
    let router = Router::new()
        .route(
            "/v1/account",
            get(|headers: axum::http::HeaderMap| async move {
                let id = if headers["authorization"]
                    .to_str()
                    .unwrap()
                    .ends_with(&"b".repeat(64))
                {
                    "other"
                } else {
                    "voice-fixture"
                };
                Json(json!({"account":{"id":id,"label":"Fixture"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/voice-agent",
            get(move || {
                let read = read.clone();
                async move { Json(read.lock().unwrap().clone()) }
            })
            .put(move |Json(request): Json<SetVoiceAgent>| {
                let write = write.clone();
                async move {
                    let mut value = write.lock().unwrap();
                    assert_eq!(request.expected_revision, value.revision);
                    *value = VoiceAgent {
                        revision: value.revision + 1,
                        target: request.target,
                    };
                    Json(value.clone())
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let deployment = Deployment::new(&format!("{origin}/v1"), &origin).unwrap();
    let desktop = Client::new(deployment.clone()).unwrap();
    let phone = Client::new(deployment).unwrap();
    let session = desktop
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let phone_session = phone
        .authenticate(&format!("oa_sk_{}", "a".repeat(64)))
        .await
        .unwrap();
    let other_session = phone
        .authenticate(&format!("oa_sk_{}", "b".repeat(64)))
        .await
        .unwrap();
    let agents = desktop.agents(&session).unwrap();
    let cancel = CancellationToken::new();
    let initial = agents.voice_agent(&cancel).await.unwrap();
    let target = VoiceTarget {
        agent_id: format!("{:064x}", 123),
        chat_id: "fork-chat".into(),
        host_id: "fixture-host".into(),
    };
    let request = agents
        .voice_agent_request(initial.revision, Some(target.clone()))
        .unwrap();
    let saved = agents.set_voice_agent(&request, &cancel).await.unwrap();
    assert_eq!(saved.target, Some(target));
    assert_eq!(
        phone
            .agents(&phone_session)
            .unwrap()
            .voice_agent(&cancel)
            .await
            .unwrap(),
        saved
    );
    assert!(matches!(
        phone
            .agents(&other_session)
            .unwrap()
            .set_voice_agent(&request, &cancel)
            .await,
        Err(Error::WrongAccount)
    ));
    let invalid = VoiceTarget {
        agent_id: "display-name".into(),
        chat_id: "chat".into(),
        host_id: "host".into(),
    };
    assert!(matches!(
        agents.voice_agent_request(1, Some(invalid)),
        Err(Error::InvalidReference)
    ));
    server.abort();
}
