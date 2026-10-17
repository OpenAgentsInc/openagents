use super::*;
use axum::{Json, body::Body, http::Request, routing::get};
use std::sync::atomic::{AtomicBool, Ordering};
use tower::ServiceExt;

struct Tasks(Vec<tokio::task::JoinHandle<()>>);
impl Drop for Tasks {
    fn drop(&mut self) {
        for t in &self.0 {
            t.abort();
        }
    }
}
async fn serve(router: Router) -> (String, tokio::task::JoinHandle<()>) {
    let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = l.local_addr().unwrap();
    (
        format!("http://{addr}"),
        tokio::spawn(async move {
            axum::serve(l, router).await.unwrap();
        }),
    )
}
fn token(who: &str) -> String {
    format!("sess_{}", if who == "alice" { "a" } else { "b" }.repeat(64))
}
#[tokio::test]
async fn fresh_account_authority_and_no_legacy_forwarding() {
    let revoked = Arc::new(AtomicBool::new(false));
    let flag = revoked.clone();
    let auth = Router::new().route(
        "/v1/account",
        get(move |headers: HeaderMap| {
            let revoked = flag.clone();
            async move {
                let a = headers
                    .get("authorization")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or_default();
                if revoked.load(Ordering::SeqCst) {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"error":{"code":"signed_out"}})),
                    );
                }
                let account = if a == format!("Bearer {}", token("alice")) {
                    "alice"
                } else if a == format!("Bearer {}", token("bob")) {
                    "bob"
                } else {
                    return (
                        StatusCode::FORBIDDEN,
                        Json(json!({"error":{"code":"forbidden"}})),
                    );
                };
                (
                    StatusCode::OK,
                    Json(json!({"account":{"id":account,"admin":false},"workspaces":[]})),
                )
            }
        }),
    );
    let (base, auth_task) = serve(auth).await;
    let _tasks = Tasks(vec![auth_task]);
    let dir = tempfile::tempdir().unwrap();
    let private = dir.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    let key = private.join("csrf");
    std::fs::write(&key, [9; 32]).unwrap();
    let path = private.join("cloud.json");
    std::fs::write(&path,serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":"http://127.0.0.1:4300","account_service":base,"csrf_secret":key})).unwrap()).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        for p in [&key, &path] {
            std::fs::set_permissions(p, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let mut config = crate::Config::development(dir.path().join("tasks"));
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    config.agent_accounts = vec!["alice".into(), "bob".into()];
    config.lab_backend = Some(Arc::new(
        lab_store::backend::FsBackend::open(dir.path().join("labs")).unwrap(),
    ));
    let app = App(Arc::new(crate::Inner { config }));
    let router = routes(&app).with_state(app);
    let make = |who: &str, method: &str, path: &str, body: Value| {
        Request::builder()
            .method(method)
            .uri(path)
            .header("Authorization", format!("Bearer {}", token(who)))
            .header("Content-Type", "application/json")
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap()
    };
    let doc = json!({"id":"example","kind":"echo","schema":"echo.v1","title":"Synthetic","content":{"text":"first"},"summary":"Created"});
    assert_eq!(
        router
            .clone()
            .oneshot(make("alice", "POST", "/v1/labs/documents", doc))
            .await
            .unwrap()
            .status(),
        201
    );
    assert_eq!(
        router
            .clone()
            .oneshot(make(
                "bob",
                "GET",
                "/v1/labs/documents/example",
                Value::Null
            ))
            .await
            .unwrap()
            .status(),
        404
    );
    assert_eq!(
        router
            .clone()
            .oneshot(make(
                "alice",
                "GET",
                "/v1/labs/documents/example",
                Value::Null
            ))
            .await
            .unwrap()
            .status(),
        200
    );
    revoked.store(true, Ordering::SeqCst);
    assert_eq!(
        router
            .oneshot(make(
                "alice",
                "GET",
                "/v1/labs/documents/example",
                Value::Null
            ))
            .await
            .unwrap()
            .status(),
        401
    );
    for (path, _, _, _, _) in lab_api::contract::OPERATIONS {
        assert!(
            crate::upstream::owned(
                &path
                    .replace("{id}", "example")
                    .replace("{kind}", "echo")
                    .replace("{name}", "echo_set")
                    .replace("{n}", "1")
            ),
            "{path}"
        );
    }
}
