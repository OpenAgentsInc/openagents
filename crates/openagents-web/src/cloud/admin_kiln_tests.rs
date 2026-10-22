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

/// POST `body` as JSON, as Kiln's page does.
async fn post_json(site: &Router, path: &str, cookies: &Cookies, body: &str) -> Answer {
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ORIGIN, format!("http://{HOST}"));
    if !cookies.0.is_empty() {
        request = request.header(header::COOKIE, cookies.header());
    }
    let response = site
        .clone()
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8(
        to_bytes(response.into_body(), 1024 * 1024)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    Answer {
        status,
        headers,
        body,
    }
}

#[tokio::test]
async fn players_get_kiln_and_a_vouched_key_when_the_site_has_its_kiln_key() {
    let dir = tempfile::tempdir().unwrap();
    let build = dir.path().join("kiln");
    std::fs::create_dir_all(build.join("pkg")).unwrap();
    std::fs::write(build.join("index.html"), PAGE).unwrap();
    std::fs::write(build.join("pkg/kiln_bg.wasm"), b"\0asm").unwrap();
    let mut kiln = crate::pages::admin::Kiln::new(build, &"ab".repeat(32), "", "");
    let secret = secp256k1::SecretKey::from_byte_array([9; 32]).unwrap();
    let site_key = secp256k1::Keypair::from_secret_key(&secp256k1::Secp256k1::new(), &secret)
        .x_only_public_key()
        .0
        .to_string();
    kiln.site_key = Some(secret);
    let f = fixture_with(|c| c.kiln = Some(Arc::new(kiln))).await;
    let player = Cookies(BTreeMap::from([("oa_cloud_session".into(), token("bob"))]));
    let away = Cookies::default();

    // The terminal offers Kiln to a signed-in player, and its files answer.
    let me = request(&f.site, Method::GET, "/home/me", &player, None, None).await;
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    assert_eq!(
        (me["admin"].clone(), me["kiln"].clone()),
        (json!(false), json!(true))
    );
    let wasm = request(
        &f.site,
        Method::GET,
        "/kiln/pkg/kiln_bg.wasm",
        &player,
        None,
        None,
    )
    .await;
    assert_eq!(wasm.status, StatusCode::OK);
    // Kiln's own page stays for admins; nobody signed out gets a file.
    for (path, who) in [
        ("/kiln/index.html", &player),
        ("/kiln/pkg/kiln_bg.wasm", &away),
        ("/admin/kiln/pkg/kiln_bg.wasm", &player),
    ] {
        let answer = request(&f.site, Method::GET, path, who, None, None).await;
        assert_eq!(answer.status, StatusCode::NOT_FOUND, "{path}");
    }

    // The site vouches for the player's browser key for a day.
    let agent = secp256k1::Keypair::from_secret_key(
        &secp256k1::Secp256k1::new(),
        &secp256k1::SecretKey::from_byte_array([7; 32]).unwrap(),
    )
    .x_only_public_key()
    .0
    .to_string();
    let body = json!({"key": agent}).to_string();
    let vouched = post_json(&f.site, "/kiln/identity", &player, &body).await;
    assert_eq!(vouched.status, StatusCode::OK, "{}", vouched.body);
    let credential: kiln::identity::Credential = serde_json::from_str(&vouched.body).unwrap();
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let mandate = kiln::identity::verify_credential(&credential, &agent, now).unwrap();
    assert_eq!(mandate.owner, site_key);
    assert!(mandate.actions.iter().any(|a| a == "kiln.test"));
    // Signed out, a form post, or another site's page gets nothing.
    let refused = post_json(&f.site, "/kiln/identity", &away, &body).await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    let form = request(
        &f.site,
        Method::POST,
        "/kiln/identity",
        &player,
        Some("key=x"),
        None,
    )
    .await;
    assert_eq!(form.status, StatusCode::FORBIDDEN);
    // A day's vouches run out.
    for _ in 0..5 {
        post_json(&f.site, "/kiln/identity", &player, &body).await;
    }
    let spent = post_json(&f.site, "/kiln/identity", &player, &body).await;
    assert_eq!(spent.status, StatusCode::TOO_MANY_REQUESTS);
    crate::copy_guard::assert_plain("/kiln/identity", &spent.body);
}

#[tokio::test]
async fn without_the_site_key_players_get_no_kiln() {
    let dir = tempfile::tempdir().unwrap();
    let build = dir.path().join("kiln");
    std::fs::create_dir_all(build.join("pkg")).unwrap();
    std::fs::write(build.join("index.html"), PAGE).unwrap();
    std::fs::write(build.join("pkg/kiln_bg.wasm"), b"\0asm").unwrap();
    let kiln = crate::pages::admin::Kiln::new(build, "", "", "");
    let f = fixture_with(|c| c.kiln = Some(Arc::new(kiln))).await;
    let player = Cookies(BTreeMap::from([("oa_cloud_session".into(), token("bob"))]));
    let me = request(&f.site, Method::GET, "/home/me", &player, None, None).await;
    let me: serde_json::Value = serde_json::from_str(&me.body).unwrap();
    assert_eq!(me["kiln"], false);
    let wasm = request(
        &f.site,
        Method::GET,
        "/kiln/pkg/kiln_bg.wasm",
        &player,
        None,
        None,
    )
    .await;
    assert_eq!(wasm.status, StatusCode::NOT_FOUND);
}
