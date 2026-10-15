//! CAD Lab is part of Pro (#11693), over HTTP: a Free account and a
//! signed-out visitor get the upsell or a typed `402` on every design and
//! edit route, Pro and admins pass, and designs already made stay
//! readable.

use super::*;
use crate::chat_store::{Store, account_owner};

/// `/cad/bracket` saved for `account`: one version with its glTF.
async fn seed(store: &Store, account: &str) {
    let scope = crate::cad_web::Scope::personal(&account_owner(account)).unwrap();
    let index = json!({"schema": "openagents.web.cad-index.v1", "docs": {"bracket": {
        "title": "Bracket", "kind": "part", "versions": 1,
        "history": [{"n": 1, "summary": "New part", "created": 1_760_400_000}]}}});
    for (rest, bytes) in [
        ("index.json", serde_json::to_vec(&index).unwrap()),
        (
            "bracket/v1/version.json",
            br#"{"n":1,"summary":"New part","report":{"mass":{"mass_kg":0.02}}}"#.to_vec(),
        ),
        ("bracket/v1/model.glb", b"glTF".to_vec()),
    ] {
        store
            .write_key(&scope.key(rest).unwrap(), bytes, None)
            .await
            .unwrap();
    }
}

struct Lab {
    fixture: Fixture,
    store: Arc<Store>,
    _root: tempfile::TempDir,
}

/// A site whose plan meter has alice on Pro; bob is Free.
async fn lab() -> Lab {
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::local(root.path().join("chats")));
    let mut journal = retail_cloud::journal::Journal::in_memory().unwrap();
    let start = now() as i64 - 60;
    retail_cloud::environment::record_period(
        &mut journal,
        &retail_cloud::environment::Period {
            account: "alice".into(),
            plan: retail_cloud::environment::plan().version,
            start,
            end: start + 30 * 86_400,
        },
    )
    .unwrap();
    let chats = store.clone();
    let fixture = fixture_with(move |config| {
        config.plan = Some(Arc::new(crate::plan::Plans::with_meter(
            Some(journal),
            None,
        )));
        config.chat_store = chats;
    })
    .await;
    for account in ["alice", "bob"] {
        seed(&store, account).await;
    }
    Lab {
        fixture,
        store,
        _root: root,
    }
}

/// [`request`] with extra headers.
async fn send(
    site: &Router,
    method: Method,
    path: &str,
    cookies: &Cookies,
    form: Option<&str>,
    extra: &[(&str, &str)],
) -> Answer {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::HOST, HOST)
        .header(header::ACCEPT, "text/html")
        .header(header::ORIGIN, ORIGIN)
        .header("sec-fetch-site", "same-origin");
    if !cookies.0.is_empty() {
        request = request.header(header::COOKIE, cookies.header());
    }
    if form.is_some() {
        request = request.header(header::CONTENT_TYPE, "application/x-www-form-urlencoded");
    }
    for (name, value) in extra {
        request = request.header(*name, *value);
    }
    let response = site
        .clone()
        .oneshot(
            request
                .body(Body::from(form.unwrap_or("").to_owned()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let body = String::from_utf8_lossy(
        &to_bytes(response.into_body(), 4 * 1024 * 1024)
            .await
            .unwrap(),
    )
    .into_owned();
    Answer {
        status,
        headers,
        body,
    }
}

fn plan_required(answer: &Answer) {
    assert_eq!(
        answer.status,
        StatusCode::PAYMENT_REQUIRED,
        "{}",
        answer.body
    );
    let body: serde_json::Value = serde_json::from_str(&answer.body).unwrap();
    assert_eq!(body["error"]["code"], "plan_required");
    assert_eq!(body["error"]["plan"], "pro");
    assert_eq!(body["error"]["upgrade_url"], "/settings#settings-plan");
}

/// Every edit route, posted with the page's own ticket.
async fn edits(
    site: &Router,
    cookies: &Cookies,
    csrf: &str,
    extra: &[(&str, &str)],
) -> Vec<Answer> {
    let pull = form(&[
        ("csrf", csrf),
        ("project", ""),
        ("version", "1"),
        ("part", "body"),
        ("face", "top"),
        ("delta", "1"),
    ]);
    let verse = form(&[
        ("csrf", csrf),
        ("project", ""),
        ("version", "1"),
        ("target", "verse"),
    ]);
    let publish = form(&[
        ("csrf", csrf),
        ("project", ""),
        ("version", "1"),
        ("controls", "[]"),
        ("mode", "free"),
    ]);
    let mut out = Vec::new();
    for (path, body) in [
        ("/cad/bracket/pull", pull),
        ("/cad/bracket/send", verse),
        ("/cad/bracket/configurator", publish),
    ] {
        out.push(send(site, Method::POST, path, cookies, Some(&body), extra).await);
    }
    let mut import = extra.to_vec();
    import.push(("x-csrf-token", csrf));
    out.push(
        send(
            site,
            Method::POST,
            "/cad/bracket/import?part=body&feature=f1",
            cookies,
            None,
            &import,
        )
        .await,
    );
    out
}

#[tokio::test]
async fn free_and_signed_out_get_the_upsell_and_a_402_and_keep_their_designs() {
    let lab = lab().await;
    let site = &lab.fixture.site;

    // Signed out: the upsell, with the trial and Log in.
    let page = send(site, Method::GET, "/cad", &Cookies::default(), None, &[]).await;
    assert_eq!(page.status, StatusCode::OK);
    plain(&page);
    assert!(page.body.contains(crate::cad_gate::TITLE), "{}", page.body);
    assert!(page.body.contains("Start 3-day free trial"));
    assert!(page.body.contains("Log in"));
    assert!(!page.body.contains("id=\"cad-form\""));

    // Free: the upsell, no Log in, and their designs.
    let bob = login(&lab.fixture, "bob").await;
    let page = send(site, Method::GET, "/cad", &bob, None, &[]).await;
    assert_eq!(page.status, StatusCode::OK);
    plain(&page);
    assert!(page.body.contains(crate::cad_gate::TITLE), "{}", page.body);
    assert!(page.body.contains("href=\"/settings#settings-plan\""));
    assert!(!page.body.contains(">Log in<"));
    assert!(page.body.contains("href=\"/cad/bracket\""), "{}", page.body);

    // The design stays viewable, downloadable and deletable.
    let doc = send(site, Method::GET, "/cad/bracket", &bob, None, &[]).await;
    assert_eq!(doc.status, StatusCode::OK, "{}", doc.body);
    plain(&doc);
    assert!(doc.body.contains(crate::cad_gate::TITLE));
    assert!(doc.body.contains("Delete this design"));
    assert!(!doc.body.contains("data-cad-pull="));
    let file = send(
        site,
        Method::GET,
        "/cad/bracket/file?v=1&name=model.glb&download=1",
        &bob,
        None,
        &[],
    )
    .await;
    assert_eq!(file.status, StatusCode::OK);
    assert_eq!(file.body, "glTF");
    // A check on the design runs nothing without Pro.
    let checked = send(
        site,
        Method::GET,
        "/cad/bracket?check=print",
        &bob,
        None,
        &[],
    )
    .await;
    assert_eq!(checked.status, StatusCode::OK);
    assert!(!checked.body.contains("Print checks"));

    // Every edit answers 402 plan_required.
    let csrf = field(form_at(&doc.body, "/cad/bracket/delete"), "csrf");
    for answer in edits(site, &bob, &csrf, &[]).await {
        plan_required(&answer);
    }
    // Sharing with sliders shows the upsell instead of the form.
    let share = send(
        site,
        Method::GET,
        "/cad/bracket/configurator",
        &bob,
        None,
        &[],
    )
    .await;
    assert_eq!(share.status, StatusCode::OK, "{}", share.body);
    plain(&share);
    assert!(share.body.contains(crate::cad_gate::TITLE));
    assert!(!share.body.contains("data-cad-config-form"));
    // Taking a placed part out of a world needs no plan.
    let remove = form(&[
        ("csrf", &csrf),
        ("project", ""),
        ("version", "1"),
        ("target", "remove"),
    ]);
    let removed = send(
        site,
        Method::POST,
        "/cad/bracket/send",
        &bob,
        Some(&remove),
        &[],
    )
    .await;
    assert_ne!(removed.status, StatusCode::PAYMENT_REQUIRED);
    // Deleting it needs no plan either.
    let delete = form(&[("csrf", &csrf), ("project", "")]);
    let deleted = send(
        site,
        Method::POST,
        "/cad/bracket/delete",
        &bob,
        Some(&delete),
        &[],
    )
    .await;
    assert_eq!(deleted.status, StatusCode::SEE_OTHER, "{}", deleted.body);
    let index = crate::cad_web::load_index(
        &lab.store,
        &crate::cad_web::Scope::personal(&account_owner("bob")).unwrap(),
    )
    .await
    .unwrap();
    assert!(index.docs.is_empty());
}

#[tokio::test]
async fn pro_and_admins_pass_and_a_test_run_can_ask_to_be_free() {
    let lab = lab().await;
    let site = &lab.fixture.site;
    let alice = login(&lab.fixture, "alice").await;
    let page = send(site, Method::GET, "/cad", &alice, None, &[]).await;
    assert_eq!(page.status, StatusCode::OK);
    assert!(!page.body.contains(crate::cad_gate::TITLE), "{}", page.body);
    let doc = send(site, Method::GET, "/cad/bracket", &alice, None, &[]).await;
    assert!(!doc.body.contains(crate::cad_gate::TITLE));
    let csrf = field(form_at(&doc.body, "/cad/bracket/delete"), "csrf");
    // Past the plan: no engine runs in this test, so nothing is made.
    for answer in edits(site, &alice, &csrf, &[]).await {
        assert_ne!(
            answer.status,
            StatusCode::PAYMENT_REQUIRED,
            "{}",
            answer.body
        );
    }

    // The same Pro account on a test run asking to be Free is Free.
    let free = [
        ("x-openagents-test", "1"),
        ("x-openagents-test-plan", "free"),
    ];
    let page = send(site, Method::GET, "/cad", &alice, None, &free).await;
    assert!(page.body.contains(crate::cad_gate::TITLE), "{}", page.body);
    for answer in edits(site, &alice, &csrf, &free).await {
        plan_required(&answer);
    }
    // Without the test-run header the plan header does nothing.
    let page = send(
        site,
        Method::GET,
        "/cad",
        &alice,
        None,
        &[("x-openagents-test-plan", "free")],
    )
    .await;
    assert!(!page.body.contains(crate::cad_gate::TITLE));

    // A site admin passes without a plan.
    lab.fixture
        .state
        .lock()
        .unwrap()
        .admins
        .insert("bob".into());
    let bob = login(&lab.fixture, "bob").await;
    let page = send(site, Method::GET, "/cad", &bob, None, &[]).await;
    assert!(!page.body.contains(crate::cad_gate::TITLE), "{}", page.body);
}
