//! Kiln for site admins at `/admin/kiln/` (#12440): only an admin gets the
//! page and its files, every answer there carries the cross-origin
//! isolation headers and nothing else on the site does, `config.json`
//! holds only the runner's public values, and `/home/me` tells the Grid's
//! terminal whether to offer Kiln.
use super::*;

const PAGE: &str = "<!doctype html><title>Kiln</title><script type=\"module\">\n  import init, { start } from './pkg/kiln.js';\n  await init();\n  start();\n</script>";

#[tokio::test]
async fn kiln_is_served_isolated_to_site_admins_only() {
    let dir = tempfile::tempdir().unwrap();
    let build = dir.path().join("kiln");
    std::fs::create_dir_all(build.join("pkg")).unwrap();
    std::fs::create_dir_all(build.join("tools")).unwrap();
    std::fs::write(build.join("index.html"), PAGE).unwrap();
    std::fs::write(build.join("worker.js"), "self.onmessage = () => {};").unwrap();
    std::fs::write(build.join("pkg/kiln_bg.wasm"), b"\0asm").unwrap();
    std::fs::write(build.join("pkg/kiln_bg.wasm.br"), b"compressed").unwrap();
    let tool = format!("tools/{}.wasm", "0f".repeat(32));
    std::fs::write(build.join(&tool), b"\0asm").unwrap();
    let runner = "ab".repeat(32);
    let kiln = crate::pages::admin::Kiln::new(build, &runner, "", "");
    let f = fixture_with(|c| c.kiln = Some(Arc::new(kiln))).await;
    f.state.lock().unwrap().admins.insert("alice".into());
    let admin = Cookies(BTreeMap::from([(
        "oa_cloud_session".into(),
        token("alice"),
    )]));
    let member = Cookies(BTreeMap::from([("oa_cloud_session".into(), token("bob"))]));
    let isolated = |answer: &Answer| {
        answer
            .headers
            .get("cross-origin-opener-policy")
            .map(|v| v.to_str().unwrap())
            == Some("same-origin")
            && answer
                .headers
                .get("cross-origin-embedder-policy")
                .map(|v| v.to_str().unwrap())
                == Some("require-corp")
    };

    // The page.
    let page = request(&f.site, Method::GET, "/admin/kiln/", &admin, None, None).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert_eq!(page.body, PAGE);
    assert!(isolated(&page));
    let policy = page.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    let inline = "\n  import init, { start } from './pkg/kiln.js';\n  await init();\n  start();\n";
    assert!(
        policy.contains(&crate::pages::admin::script_hash(inline)),
        "{policy}"
    );
    assert!(policy.contains("frame-ancestors 'self'"));
    let bare = request(&f.site, Method::GET, "/admin/kiln", &admin, None, None).await;
    assert_eq!(bare.headers[header::LOCATION], "/admin/kiln/");

    // Its files.
    for path in ["/admin/kiln/worker.js", "/admin/kiln/pkg/kiln_bg.wasm"] {
        let file = request(&f.site, Method::GET, path, &admin, None, None).await;
        assert_eq!(file.status, StatusCode::OK, "{path}");
        assert!(isolated(&file), "{path}");
    }
    let tool = request(
        &f.site,
        Method::GET,
        &format!("/admin/kiln/{tool}"),
        &admin,
        None,
        None,
    )
    .await;
    assert_eq!(tool.status, StatusCode::OK);
    assert!(
        tool.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .contains("immutable")
    );
    let config = request(
        &f.site,
        Method::GET,
        "/admin/kiln/config.json",
        &admin,
        None,
        None,
    )
    .await;
    assert_eq!(config.status, StatusCode::OK);
    assert!(isolated(&config));
    let config: serde_json::Value = serde_json::from_str(&config.body).unwrap();
    assert_eq!(
        config,
        json!({"runner": runner, "repo": "", "relay": "wss://relay.openagents.com"})
    );
    for missing in ["/admin/kiln/secret.txt", "/admin/kiln/../index.html"] {
        let answer = request(&f.site, Method::GET, missing, &admin, None, None).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{missing}");
    }

    // Anyone else.
    let refused = request(&f.site, Method::GET, "/admin/kiln/", &member, None, None).await;
    assert_eq!(refused.status, StatusCode::FORBIDDEN);
    assert!(refused.body.contains("This page is for OpenAgents staff."));
    assert!(!refused.body.contains("kiln.js"));
    for path in ["/admin/kiln/config.json", "/admin/kiln/pkg/kiln_bg.wasm"] {
        for who in [&member, &Cookies::default()] {
            let answer = request(&f.site, Method::GET, path, who, None, None).await;
            assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
            assert!(!answer.body.contains(&runner));
        }
    }
    let away = request(
        &f.site,
        Method::GET,
        "/admin/kiln/",
        &Cookies::default(),
        None,
        None,
    )
    .await;
    assert_eq!(away.status, StatusCode::SEE_OTHER);
    assert_eq!(
        away.headers[header::LOCATION],
        "/login?return_to=%2Fadmin%2Fkiln%2F"
    );

    // The rest of the site isn't isolated.
    for path in ["/admin", "/", "/home/me"] {
        let answer = request(&f.site, Method::GET, path, &admin, None, None).await;
        assert!(
            !answer.headers.contains_key("cross-origin-embedder-policy"),
            "{path}"
        );
        assert!(
            !answer.headers.contains_key("cross-origin-opener-policy"),
            "{path}"
        );
    }
    let hub = request(&f.site, Method::GET, "/admin", &admin, None, None).await;
    assert_eq!(hub.status, StatusCode::OK, "{}", hub.body);
    assert!(hub.body.contains("href=\"/admin/kiln/\""));
    assert!(!hub.body.contains("isn't installed"));

    // `/home/me` tells the terminal who is an admin.
    let me = |who: &Cookies| {
        let site = f.site.clone();
        let who = who.clone();
        async move {
            let answer = request(&site, Method::GET, "/home/me", &who, None, None).await;
            serde_json::from_str::<serde_json::Value>(&answer.body).unwrap()
        }
    };
    assert_eq!(me(&admin).await["admin"], true);
    assert_eq!(me(&member).await["admin"], false);
    assert_eq!(me(&Cookies::default()).await.get("admin"), None);
}

#[tokio::test]
async fn without_a_build_the_hub_says_kiln_isnt_installed() {
    let f = fixture_with(|_| {}).await;
    f.state.lock().unwrap().admins.insert("alice".into());
    let admin = Cookies(BTreeMap::from([(
        "oa_cloud_session".into(),
        token("alice"),
    )]));
    let hub = request(&f.site, Method::GET, "/admin", &admin, None, None).await;
    assert_eq!(hub.status, StatusCode::OK);
    assert!(hub.body.contains("It isn't installed on this server."));
    let page = request(&f.site, Method::GET, "/admin/kiln/", &admin, None, None).await;
    assert_eq!(page.status, StatusCode::SERVICE_UNAVAILABLE);
    crate::copy_guard::assert_plain("/admin/kiln/", &page.body);
}
