//! Native CLI requests against an isolated SDK fixture, with no saved login.
use axum::{
    Json, Router,
    body::Body,
    extract::State,
    http::HeaderMap,
    response::Response,
    routing::{get, post},
};
use serde_json::{Value, json};
use std::{
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

#[derive(Clone, Default)]
struct Seen(Arc<Mutex<Vec<Value>>>);
fn card(kind: &str, doc: &str) -> Value {
    json!({"schema":"lab.card.v1","kind":kind,"document":doc,"version":2,"title":"Synthetic","summary":"Saved","view":"report","page":format!("/labs/{kind}/{doc}")})
}
fn kinds() -> Value {
    let kind = |kind: &str, prefix: &str, doc_arg: &str, name: &str| {
        let mut properties = json!({doc_arg:{"type":"string"}});
        if kind == "cad" {
            properties["document"] = json!({"type":"object"});
        }
        json!({"kind":kind,"name":kind,"schemas":["synthetic.v1"],"tool_prefix":prefix,"doc_arg":doc_arg,"tools":[{"name":name,"description":"Save a synthetic document.","parameters":{"type":"object","properties":properties}}],"job_types":[],"views":["report"],"limits":{"max_doc_bytes":1024,"max_docs_per_space":10,"max_versions_per_doc":10,"max_args_bytes":1024,"runs_per_hour":10}})
    };
    json!({"data":[kind("cad","cad_","doc","cad_from_json"),kind("video","lab_","document","lab_new")]})
}
fn check(headers: &HeaderMap) {
    assert_eq!(
        headers["authorization"],
        format!("Bearer sess_{}", "a".repeat(64))
    );
    assert_eq!(headers["x-openagents-test"], "1");
}
async fn run(origin: String, args: Vec<&'static str>) -> Value {
    tokio::task::spawn_blocking(move || {
        let home = tempfile::tempdir().unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_openagents"))
            .args(args)
            .env_clear()
            .env("PATH", std::env::var_os("PATH").unwrap_or_default())
            .env("HOME", home.path())
            .current_dir(home.path())
            .env("OPENAGENTS_TOKEN", format!("sess_{}", "a".repeat(64)))
            .env("OPENAGENTS_ORIGIN", origin)
            .env("OPENAGENTS_TEST_RUN", "1")
            .stdin(Stdio::null())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!output.stdout.contains(&0x1b));
        serde_json::from_slice(&output.stdout).unwrap()
    })
    .await
    .unwrap()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cad_video_and_event_following_use_native_account_routes() {
    let seen = Seen::default();
    let router=Router::new()
        .route("/v1/account",get(|headers:HeaderMap|async move {check(&headers);Json(json!({"account":{"id":"acct_fixture","label":"Fixture"},"workspaces":[]}))}))
        .route("/v1/labs/kinds",get(|headers:HeaderMap|async move {check(&headers);Json(kinds())}))
        .route("/v1/labs/documents/bracket",get(||async {Json(json!({"id":"bracket","kind":"cad","schema":"part.v1","title":"Bracket","created_unix_ms":1,"latest":1}))}))
        .route("/v1/labs/documents/film",get(||async {Json(json!({"id":"film","kind":"video","schema":"cut.v1","title":"Film","created_unix_ms":1,"latest":2}))}))
        .route("/v1/labs/documents/film/versions/2/card",get(||async {Json(card("video","film"))}))
        .route("/v1/labs/kinds/cad/tools/cad_from_json",post(|State(seen):State<Seen>,headers:HeaderMap,Json(body):Json<Value>|async move {check(&headers);seen.0.lock().unwrap().push(body);Json(json!({"result":{"card":card("cad","bracket")}}))}))
        .route("/v1/labs/kinds/video/tools/lab_new",post(|State(seen):State<Seen>,headers:HeaderMap,Json(body):Json<Value>|async move {check(&headers);seen.0.lock().unwrap().push(body);Json(json!({"result":{"card":card("video","film")}}))}))
        .route("/v1/labs/documents/film/events",get(|headers:HeaderMap|async move {
            check(&headers);assert_eq!(headers["last-event-id"],"7");
            Response::builder().header("content-type","text/event-stream").body(Body::from("id: 8\nevent: changes\ndata: {\"data\":[{\"cursor\":8,\"document\":\"film\",\"at_unix_ms\":1,\"change\":{\"version\":2}}]}\n\n")).unwrap()
        })).with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let registry = run(origin.clone(), vec!["--json", "lab", "kinds"]).await;
    assert_eq!(registry["kinds"][1]["tools"][0]["name"], "lab_new");
    let cad = run(
        origin.clone(),
        vec![
            "--json",
            "cad",
            "tool",
            "from_json",
            "--json",
            r#"{"doc":"bracket","document":{}}"#,
        ],
    )
    .await;
    assert_eq!(cad["card"]["kind"], "cad");
    let film = run(
        origin.clone(),
        vec![
            "--json",
            "lab",
            "tool",
            "video",
            "lab_new",
            "--json",
            r#"{"document":"film"}"#,
        ],
    )
    .await;
    assert_eq!(film["card"]["document"], "film");
    let event = run(
        origin,
        vec![
            "--json", "lab", "watch", "film", "--since", "7", "--count", "1",
        ],
    )
    .await;
    assert_eq!(event["cursor"], 8);
    let calls = seen.0.lock().unwrap();
    assert_eq!(calls.len(), 2);
    assert_eq!(calls[0]["base"], 1);
    assert_eq!(calls[0]["arguments"]["doc"], "bracket");
    assert_eq!(calls[1]["arguments"]["document"], "film");
    task.abort();
}
