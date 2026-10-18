use super::*;
use axum::{
    Json,
    extract::State,
    http::{Method, Uri, header},
    response::IntoResponse,
    routing::post,
};
use openagents_sdk::{CancellationToken, Client, Deployment, Error as SdkError};
use serde_json::Value;
use std::{
    os::unix::fs::PermissionsExt,
    sync::{Arc, Mutex},
};

#[derive(Default)]
struct Fake {
    comments: BTreeMap<u64, Vec<Value>>,
    closed: BTreeSet<u64>,
    assignments: Vec<Value>,
    moves: Vec<Value>,
    fail_comment: bool,
    pause_scan: Option<Arc<Gate>>,
}
#[derive(Default)]
struct Gate {
    entered: tokio::sync::Notify,
    resume: tokio::sync::Notify,
}
fn account(headers: &HeaderMap) -> Option<&'static str> {
    let token = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    if token == format!("Bearer sess_{}", "a".repeat(64)) {
        Some("claims-alice")
    } else if token == format!("Bearer sess_{}", "b".repeat(64)) {
        Some("claims-bob")
    } else {
        None
    }
}
async fn auth(headers: HeaderMap) -> Response {
    match account(&headers) {
        Some(id) => Json(json!({"account":{"id":id,"label":id,"admin":false},"workspaces":[]}))
            .into_response(),
        None => (
            StatusCode::UNAUTHORIZED,
            Json(json!({"error":{"code":"signed_out"}})),
        )
            .into_response(),
    }
}
async fn auth_session(headers: HeaderMap) -> Response {
    match account(&headers) {
        Some(id) => Json(json!({"session":{"id":id,"account":id,"kind":"user","state":"active","expires_at":crate::cloud::session::now()+3600}})).into_response(),
        None => auth(headers).await,
    }
}
async fn token(headers: HeaderMap) -> Response {
    match account(&headers) {
        Some(id) => Json(json!({"token":format!("fixture-{id}")})).into_response(),
        None => auth(headers).await,
    }
}
async fn github(
    State(fake): State<Arc<Mutex<Fake>>>,
    method: Method,
    uri: Uri,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let auth = headers
        .get(header::AUTHORIZATION)
        .unwrap()
        .to_str()
        .unwrap();
    assert!(auth == "Bearer fixture-claims-alice" || auth == "Bearer fixture-claims-bob");
    let sent: Value = serde_json::from_slice(&body).unwrap_or(Value::Null);
    let gate = if uri.path() == "/repos/owner/repo/issues" {
        fake.lock().unwrap().pause_scan.clone()
    } else {
        None
    };
    if let Some(gate) = gate {
        gate.entered.notify_one();
        gate.resume.notified().await;
    }
    let mut fake = fake.lock().unwrap();
    let path: Vec<_> = uri.path().trim_matches('/').split('/').collect();
    let value = match path.as_slice() {
        ["user"] => json!({"login":"fixture-user"}),
        ["repos", "owner", "repo", "issues"] => {
            let rows: Vec<_> = fake
                .comments
                .iter()
                .filter(|(n, _)| !fake.closed.contains(n))
                .map(|(n, c)| json!({"number":n,"comments":c.len()}))
                .collect();
            json!(rows)
        }
        ["repos", "owner", "repo", "issues", n] => {
            let n: u64 = n.parse().unwrap();
            if method == Method::PATCH {
                fake.assignments.push(sent.clone());
            }
            json!({"number":n,"state":if fake.closed.contains(&n){"closed"}else{"open"},
                "comments":fake.comments.get(&n).map_or(0,Vec::len),"assignees":[{"login":"other-user"}]})
        }
        ["repos", "owner", "repo", "issues", n, "comments"] => {
            let n: u64 = n.parse().unwrap();
            if method == Method::POST {
                if fake.fail_comment {
                    return (
                        StatusCode::BAD_GATEWAY,
                        Json(json!({"message":"fixture failure"})),
                    )
                        .into_response();
                }
                fake.comments.entry(n).or_default().push(json!({"body":sent["body"],"created_at":oa_auth::app::github_time_text((now()/1000) as u64)}));
                json!({"id":1})
            } else {
                let page: usize = uri
                    .query()
                    .unwrap_or_default()
                    .split('&')
                    .find_map(|p| p.strip_prefix("page=").and_then(|n| n.parse().ok()))
                    .unwrap_or(1);
                json!(
                    fake.comments
                        .get(&n)
                        .into_iter()
                        .flatten()
                        .skip((page - 1) * 100)
                        .take(100)
                        .collect::<Vec<_>>()
                )
            }
        }
        ["graphql"] => {
            if sent["query"].as_str().unwrap().starts_with("mutation") {
                fake.moves.push(sent["variables"].clone());
                json!({"data":{"updateProjectV2ItemFieldValue":{"projectV2Item":{"id":"item"}}}})
            } else {
                let items:Vec<_> = ["one","two"].iter().map(|id|json!({"id":format!("item-{id}"),"project":{"id":id,"fields":{"pageInfo":{"hasNextPage":false},"nodes":[{"id":"status","name":"Status","options":[{"id":"ready","name":"Todo"},{"id":"working","name":"In Progress"},{"id":"done","name":"Done"}]}]}}})).collect();
                json!({"data":{"repository":{"issue":{"projectItems":{"pageInfo":{"hasNextPage":false},"nodes":items}}}}})
            }
        }
        _ => panic!("unexpected GitHub request: {method} {uri}"),
    };
    Json(value).into_response()
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (origin, task)
}

#[tokio::test]
async fn sdk_routes_join_private_runs_enforce_batches_and_retry_github_updates() {
    let fake = Arc::new(Mutex::new(Fake::default()));
    let (gh_base, gh_task) = serve(Router::new().fallback(github).with_state(fake.clone())).await;
    let (auth_base, auth_task) = serve(
        Router::new()
            .route("/v1/account", get(auth))
            .route("/v1/session", get(auth_session))
            .route("/v1/account/github/token", post(token)),
    )
    .await;
    let root = tempfile::tempdir().unwrap();
    let path = root.path().canonicalize().unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
    let key = path.join("csrf");
    std::fs::write(&key, [42; 32]).unwrap();
    std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o600)).unwrap();
    let config_path = path.join("cloud.json");
    std::fs::write(&config_path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":auth_base,"csrf_secret":key})).unwrap()).unwrap();
    std::fs::set_permissions(&config_path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(path.join("tasks"));
    config.chat_store = Arc::new(Store::local(path.join("chats")));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&config_path).unwrap(),
    ));
    config.github = Some(Arc::new(oa_auth::GithubApp {
        client_id: "fixture".into(),
        redirect_url: format!("{gh_base}/auth/github/callback"),
        endpoints: oa_auth::Endpoints::at(&gh_base),
    }));
    let app = App(Arc::new(crate::Inner { config }));
    let (base, task) = serve(
        routes()
            .merge(crate::runs::routes())
            .with_state(app)
            .route("/v1/account", get(auth)),
    )
    .await;
    let client = Client::with_timeout(
        Deployment::new(&format!("{base}/v1"), &base).unwrap(),
        std::time::Duration::from_secs(5),
    )
    .unwrap();
    let a = client
        .authenticate(&format!("sess_{}", "a".repeat(64)))
        .await
        .unwrap();
    let b = client
        .authenticate(&format!("sess_{}", "b".repeat(64)))
        .await
        .unwrap();
    let issues = client.issues(&a).unwrap();
    let other = client.issues(&b).unwrap();
    let cancel = CancellationToken::new();
    let report = beat("chat-a", "native-a");
    let no_run = issues.claim("owner/repo", 1, "chat:chat-a", &cancel).await;
    assert!(
        matches!(&no_run, Err(SdkError::Refused { status: 409, .. })),
        "{no_run:?}"
    );
    client
        .runs(&a)
        .unwrap()
        .heartbeat(&report, &cancel)
        .await
        .unwrap();
    assert!(matches!(
        other.claim("owner/repo", 1, "chat:chat-a", &cancel).await,
        Err(SdkError::Refused { status: 409, .. })
    ));
    // A failed comment releases its pending slot so a retry can finish.
    fake.lock().unwrap().fail_comment = true;
    assert!(
        issues
            .claim("owner/repo", 1, "chat:chat-a", &cancel)
            .await
            .is_err()
    );
    fake.lock().unwrap().fail_comment = false;
    let first = issues
        .claim("owner/repo", 1, "chat:chat-a", &cancel)
        .await
        .unwrap();
    assert_eq!(first.run.as_deref(), Some("chat-a"));
    assert_eq!(
        issues
            .claim("owner/repo", 1, "chat:chat-a", &cancel)
            .await
            .unwrap()
            .claimed_at,
        first.claimed_at
    );
    assert_eq!(fake.lock().unwrap().comments[&1].len(), 1);
    assert!(matches!(
        issues.claim("owner/repo", 2, "chat:chat-a", &cancel).await,
        Err(SdkError::Refused { status: 409, .. })
    ));
    let r = request("chat:chat-a", &[1, 2, 3]);
    issues
        .claim_with("owner/repo", 2, &r, &cancel)
        .await
        .unwrap();
    issues
        .claim_with("owner/repo", 3, &r, &cancel)
        .await
        .unwrap();
    let mut force = request("chat:chat-a", &[]);
    force.force = true;
    assert!(matches!(
        issues.claim_with("owner/repo", 4, &force, &cancel).await,
        Err(SdkError::Refused { status: 409, .. })
    ));
    let listing = issues.claims("owner/repo", &cancel).await.unwrap();
    assert_eq!(listing.len(), 3);
    assert!(listing.iter().all(|c| c.run.as_deref() == Some("chat-a")));
    assert!(
        other
            .claims("owner/repo", &cancel)
            .await
            .unwrap()
            .iter()
            .all(|c| c.run.is_none())
    );
    issues
        .release(
            "owner/repo",
            2,
            Some("Claimed earlier. <!-- openagents-coder-claim -->"),
            &cancel,
        )
        .await
        .unwrap();
    let count = fake.lock().unwrap().comments[&2].len();
    issues
        .release("owner/repo", 2, None, &cancel)
        .await
        .unwrap();
    assert_eq!(fake.lock().unwrap().comments[&2].len(), count);
    fake.lock().unwrap().closed.extend([1, 3]);
    issues
        .claim("owner/repo", 4, "chat:chat-a", &cancel)
        .await
        .unwrap();
    issues
        .board("owner/repo", 4, BoardStatus::Done, &cancel)
        .await
        .unwrap();
    let state = fake.lock().unwrap();
    assert!(
        state
            .assignments
            .iter()
            .any(|v| v["assignees"] == json!(["other-user", "fixture-user"]))
    );
    assert!(
        state
            .assignments
            .iter()
            .any(|v| v["assignees"] == json!(["other-user"]))
    );
    for p in ["one", "two"] {
        assert!(state.moves.iter().any(|v| v["p"] == p && v["o"] == "done"));
    }
    drop(state);
    // Ending a run during GitHub reads must prevent a later claim write.
    issues
        .release("owner/repo", 4, None, &cancel)
        .await
        .unwrap();
    let gate = Arc::new(Gate::default());
    fake.lock().unwrap().pause_scan = Some(gate.clone());
    let stop = async {
        gate.entered.notified().await;
        client
            .runs(&a)
            .unwrap()
            .end(&report.run, &cancel)
            .await
            .unwrap();
        fake.lock().unwrap().pause_scan = None;
        gate.resume.notify_one();
    };
    let (result, ()) = tokio::time::timeout(std::time::Duration::from_secs(10), async {
        tokio::join!(issues.claim("owner/repo", 5, "chat:chat-a", &cancel), stop)
    })
    .await
    .expect("the paused claim and run end must both finish");
    assert!(matches!(result, Err(SdkError::Refused { status: 409, .. })));
    assert!(!fake.lock().unwrap().comments.contains_key(&5));
    let raw = reqwest::Client::new();
    let bad = raw
        .put(format!("{base}/v1/repos/owner/repo/issues/no-number/claim"))
        .bearer_auth(format!("sess_{}", "a".repeat(64)))
        .json(&request("chat:chat-a", &[]))
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        bad.json::<Value>().await.unwrap()["error"]["code"],
        "invalid_claim"
    );
    assert_eq!(
        raw.get(format!("{base}/v1/repos/owner/repo/claims"))
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::UNAUTHORIZED
    );
    // A partial newest page must not shorten the 1,000-comment search window.
    let mut history = vec![json!({"body":"Progress","created_at":"2026-10-18T01:00:00Z"}); 1001];
    history[1] = json!({"body":"Claimed: working on this.","created_at":"2026-10-18T01:00:00Z"});
    fake.lock().unwrap().comments.insert(6, history);
    assert!(
        issues
            .claims("owner/repo", &cancel)
            .await
            .unwrap()
            .iter()
            .any(|c| c.issue == 6)
    );
    {
        let mut state = fake.lock().unwrap();
        let comments = state.comments.get_mut(&6).unwrap();
        comments.swap(0, 1);
    }
    assert!(matches!(
        issues.claims("owner/repo", &cancel).await,
        Err(SdkError::Refused { status: 422, .. })
    ));
    task.abort();
    auth_task.abort();
    gh_task.abort();
}
