//! Admin pages in GPUI: authorization precedes redirects and every JSON
//! read/write, and cookie writes retain their same-origin/CSRF checks.
use super::*;
use serde_json::Value;

async fn api(
    f: &Fixture,
    who: &str,
    method: Method,
    path: &str,
    body: Value,
    csrf: Option<&str>,
    same_origin: bool,
) -> (StatusCode, Value) {
    let mut req = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::COOKIE, format!("oa_cloud_session={}", token(who)))
        .header(header::CONTENT_TYPE, "application/json");
    if same_origin {
        req = req.header("sec-fetch-site", "same-origin");
    }
    if let Some(csrf) = csrf {
        req = req.header(crate::cookie_api::CSRF_HEADER, csrf);
    }
    let response = f
        .site
        .clone()
        .oneshot(req.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    assert!(
        response.headers()[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("no-store")
    );
    let bytes = to_bytes(response.into_body(), 4 * 1024 * 1024)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

#[tokio::test]
async fn admin_pages_authorize_before_redirect_and_cookie_api_checks_every_action() {
    let dir = tempfile::tempdir().unwrap();
    // The connections store refuses a root reached through a symlink, and
    // macOS's temporary directory is under /var, a link to /private/var.
    let root = dir.path().canonicalize().unwrap();
    let build = root.join("gpui");
    std::fs::create_dir(&build).unwrap();
    for name in [
        "openagents_web_chat.js",
        "openagents_web_chat_bg.wasm",
        "start.js",
    ] {
        std::fs::write(build.join(name), "fixture").unwrap();
    }
    let access = Arc::new(
        crate::cloud::connections::Store::open(
            &root.join("connections"),
            oa_seal::Keyring::scratch("test").unwrap().0,
        )
        .unwrap(),
    );
    let f = fixture_with(|c| {
        c.gpui_chat = Some(build);
        c.connections = Some(access);
        c.features = crate::features::Features::default();
    })
    .await;
    f.state.lock().unwrap().admins.insert("alice".into());
    let admin = Cookies(BTreeMap::from([(
        "oa_cloud_session".into(),
        token("alice"),
    )]));
    let plain = Cookies(BTreeMap::from([("oa_cloud_session".into(), token("bob"))]));
    for (path, page, api_path) in [
        ("/admin/analytics", "admin-analytics", "/v1/admin/analytics"),
        (
            "/admin/attribution",
            "admin-attribution",
            "/v1/admin/attribution",
        ),
        (
            "/admin/access/drive-connections",
            "admin-drive-access",
            "/v1/admin/access/drive-connections",
        ),
    ] {
        let path = format!("{path}?ui=gpui");
        for cookies in [&plain, &Cookies::default()] {
            let denied = request(&f.site, Method::GET, &path, cookies, None, None).await;
            // Signed-out Drive HTML retains its login hop; a member gets 404.
            if !cookies.0.is_empty() {
                assert_eq!(
                    denied.status,
                    StatusCode::NOT_FOUND,
                    "{path}: {}",
                    denied.body
                );
            }
            assert!(
                !denied.headers.contains_key(header::LOCATION)
                    || !denied.headers[header::LOCATION]
                        .to_str()
                        .unwrap()
                        .starts_with("/gpui")
            );
        }
        let moved = request(&f.site, Method::GET, &path, &admin, None, None).await;
        assert_eq!(
            moved.status,
            StatusCode::SEE_OTHER,
            "{path}: {}",
            moved.body
        );
        assert_eq!(
            moved.headers[header::LOCATION],
            format!("/gpui?page={page}")
        );
        let fallback_path = path.replace("ui=gpui", "ui=html");
        let kept = request(&f.site, Method::GET, &fallback_path, &admin, None, None).await;
        assert_eq!(kept.status, StatusCode::OK, "{}", kept.body);
        let shell = request(
            &f.site,
            Method::GET,
            &format!("/chat?ui=gpui&page={page}"),
            &admin,
            None,
            None,
        )
        .await;
        assert_eq!(shell.status, StatusCode::OK, "{}", shell.body);
        assert!(shell.body.contains(&fallback_path));
        let denied = request(
            &f.site,
            Method::GET,
            &format!("/chat?ui=gpui&page={page}"),
            &plain,
            None,
            None,
        )
        .await;
        assert_eq!(denied.status, StatusCode::NOT_FOUND);
        let (status, report) =
            api(&f, "alice", Method::GET, api_path, Value::Null, None, true).await;
        assert_eq!(status, StatusCode::OK, "{api_path}: {report}");
        assert!(!report.to_string().contains(CANARY));
        assert!(
            !report.to_string().contains("Commission"),
            "Hidden affiliate figures stay hidden."
        );
        assert_eq!(
            api(&f, "bob", Method::GET, api_path, Value::Null, None, true)
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            api(&f, "alice", Method::GET, api_path, Value::Null, None, false)
                .await
                .0,
            StatusCode::FORBIDDEN
        );
    }
    let shell = request(&f.site, Method::GET, "/chat?ui=gpui", &admin, None, None).await;
    let csrf = shell
        .body
        .split("name=\"openagents-csrf\" content=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let path = "/v1/admin/access/drive-connections";
    let add = json!({"email":"test+admin@example.com"});
    assert_eq!(
        api(&f, "alice", Method::POST, path, add.clone(), None, true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        api(&f, "bob", Method::POST, path, add.clone(), Some(csrf), true)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, report) = api(&f, "alice", Method::POST, path, add, Some(csrf), true).await;
    assert_eq!(status, StatusCode::CREATED, "{report}");
    assert!(
        report["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["email"] == "test+admin@example.com")
    );
    let remove = format!("{path}/test%2Badmin%40example.com");
    assert_eq!(
        api(
            &f,
            "alice",
            Method::DELETE,
            &remove,
            Value::Null,
            None,
            true
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let (status, report) = api(
        &f,
        "alice",
        Method::DELETE,
        &remove,
        Value::Null,
        Some(csrf),
        true,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{report}");
    assert!(
        !report["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["email"] == "test+admin@example.com")
    );
    for path in ["/board?ui=gpui", "/limited?ui=gpui", "/revoked?ui=gpui"] {
        for cookies in [&admin, &plain, &Cookies::default()] {
            assert_eq!(
                request(&f.site, Method::GET, path, cookies, None, None)
                    .await
                    .status,
                StatusCode::NOT_FOUND
            );
        }
    }
}
