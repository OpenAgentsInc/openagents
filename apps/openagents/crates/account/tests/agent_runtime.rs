//! Runtime choices use the normal account API and its revision checks.
use axum::{Json, Router, extract::State, http::StatusCode, response::IntoResponse, routing::get};
use openagents_account::openagents::{
    Client, Deployment,
    agents::{AgentDefinition, UpdateAgent},
};
use openagents_account::{Controller, agent_runtime, desktop::MemoryStore};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

struct Server {
    agent: Mutex<AgentDefinition>,
    conflict: AtomicBool,
}

#[tokio::test]
async fn a_runtime_choice_preserves_identity_and_rejects_a_concurrent_edit() {
    let id = "b".repeat(64);
    let definition: AgentDefinition = serde_json::from_value(serde_json::json!({
        "id":id,"revision":7,"deleted":false,"spec":{
            "name":"Coordinator","description":"Test coordinator","instructions":"Watch issue 12341 with worker-one.",
            "runtime":"claude-code","model":"opus","access":"host_default",
            "autonomy":{"check_ins":true,"authority":true}
        }
    })).unwrap();
    let original = definition.spec.clone();
    let state = Arc::new(Server {
        agent: Mutex::new(definition),
        conflict: AtomicBool::new(false),
    });
    let app = Router::new()
        .route("/v1/account", get(|| async { Json(serde_json::json!({"account":{"id":"test-runtime","label":"Test"},"workspaces":[]})) }))
        .route("/v1/agent-definitions/{id}", get(|State(s): State<Arc<Server>>| async move {
            Json(s.agent.lock().unwrap().clone())
        }).put(|State(s): State<Arc<Server>>, Json(request): Json<UpdateAgent>| async move {
            let mut saved = s.agent.lock().unwrap();
            if s.conflict.load(Ordering::Relaxed) || request.expected_revision != saved.revision {
                return (StatusCode::CONFLICT, Json(serde_json::json!({"error":"revision_conflict"}))).into_response();
            }
            assert!(request.spec.same_identity(&saved.spec));
            assert_eq!(request.spec.autonomy, saved.spec.autonomy);
            saved.spec = request.spec;
            saved.revision += 1;
            Json(saved.clone()).into_response()
        }))
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let root = tempfile::tempdir().unwrap();
    let controller = Controller::new(
        Client::new(Deployment::new(&format!("{base}/v1"), &base).unwrap()).unwrap(),
        Arc::new(MemoryStore::default()),
        root.path().join("signed-out"),
    );
    controller
        .import_credential(format!("sess_{:064x}", 1))
        .await;
    let lease = controller.lease().unwrap();
    for runtime in ["codex", "claude-code"] {
        let saved = agent_runtime::switch(&lease, &id, runtime).await.unwrap();
        assert!(saved.spec.same_identity(&original));
        assert_eq!(saved.spec.autonomy, original.autonomy);
        assert_eq!(saved.spec.runtime, runtime);
    }
    assert_eq!(state.agent.lock().unwrap().revision, 9);
    state.conflict.store(true, Ordering::Relaxed);
    assert!(agent_runtime::switch(&lease, &id, "codex").await.is_err());
    assert_eq!(state.agent.lock().unwrap().spec.runtime, "claude-code");
    server.abort();
}
