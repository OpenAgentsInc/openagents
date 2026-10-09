//! Adding repositories through the GitHub App (#11056), through the whole
//! site: Install on repositories authorizes the App, GitHub's install
//! page picks repositories and sends the person back to the setup URL,
//! and only the chosen repositories can be added.

use super::*;
use oa_auth::fake::FakeApp;

const APP_CLIENT: &str = "Iv23liWebTestApp";
const APP_SECRET: &str = "web-test-app-secret";
const SETUP_URL: &str = "http://127.0.0.1:4300/auth/github/setup";

async fn world_with_app() -> (World, String) {
    let fake = Fake::new(CLIENT, SECRET, REDIRECT, vec![fake::octo(), fake::quiet()]);
    let github_origin = fake.spawn().await.unwrap();
    let credentials = fake::credentials(&github_origin, CLIENT, SECRET, REDIRECT).unwrap();
    let app_credentials = fake::app_credentials(
        &github_origin,
        777,
        "openagents-web-test",
        APP_CLIENT,
        APP_SECRET,
        REDIRECT,
    )
    .unwrap();
    fake.with_app(FakeApp::of(&app_credentials, APP_SECRET, SETUP_URL));
    let install = app_credentials.install();
    let client =
        oa_auth::AppClient::with_cache(app_credentials, Arc::new(oa_auth::TokenCache::new()))
            .unwrap();
    let oauth = credentials.app.clone();
    let root = tempfile::tempdir().unwrap();
    let stores = root.path().join("accounts");
    let service = LocalService::install(
        &stores,
        oa_auth::Github::new(credentials).unwrap(),
        "signup",
        3600,
    )
    .unwrap()
    .with_app(client);
    let account_service = service.spawn().await.unwrap();
    let private = root.path().canonicalize().unwrap().join("private");
    std::fs::create_dir(&private).unwrap();
    std::fs::set_permissions(&private, std::fs::Permissions::from_mode(0o700)).unwrap();
    let secret = private.join("csrf.key");
    std::fs::write(&secret, [21; 32]).unwrap();
    std::fs::set_permissions(&secret, std::fs::Permissions::from_mode(0o600)).unwrap();
    let path = private.join("cloud.json");
    std::fs::write(
        &path,
        serde_json::to_vec(&json!({"schema":"openagents.cloud.web-config.v1","public_origin":ORIGIN,"account_service":account_service,"csrf_secret":secret})).unwrap(),
    )
    .unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
    let mut config = crate::Config::development(root.path().join("tasks"));
    let store = Arc::new(Store::local(root.path().join("chats")));
    config.chat_store = store.clone();
    config.cloud = Some(Arc::new(
        crate::cloud::session::CloudSession::load(&path).unwrap(),
    ));
    config.github = Some(Arc::new(oauth));
    let install_url = install.install_url();
    config.github_install = Some(Arc::new(install));
    (
        World {
            _root: root,
            site: crate::router(config),
            fake,
            store,
            github: reqwest::Client::builder()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            host: HOST.to_owned(),
            origin: ORIGIN.to_owned(),
        },
        install_url,
    )
}

#[tokio::test]
async fn installing_the_app_lists_only_the_chosen_repositories() {
    let (world, install_url) = world_with_app().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;

    let page = browser.get(&world, PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body.contains("Install on repositories"),
        "{}",
        page.body
    );
    assert!(!page.body.contains("Public repositories only"));
    crate::copy_guard::assert_plain(PAGE, &page.body);

    // Install on repositories: first authorize the App (its own client).
    let start = browser.get(&world, INSTALL).await;
    assert_eq!(start.status, StatusCode::SEE_OTHER, "{}", start.body);
    let authorize = url::Url::parse(start.location()).unwrap();
    assert!(
        authorize
            .query_pairs()
            .any(|(k, v)| k == "client_id" && v == APP_CLIENT)
    );
    assert!(browser.0["oa_auth_flow"].ends_with(".install"));
    let at_github = world
        .github
        .get(format!("{}&login=octo-local", start.location()))
        .send()
        .await
        .unwrap();
    let back = at_github.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    let callback = browser
        .get(&world, back.strip_prefix(ORIGIN).unwrap())
        .await;
    assert_eq!(callback.status, StatusCode::OK, "{}", callback.body);
    let next = callback.body[callback.body.find(FINISH).unwrap()..]
        .split('"')
        .next()
        .unwrap()
        .replace("&amp;", "&");
    // Installed nowhere yet: on to GitHub's page for picking repositories.
    let finished = browser.get(&world, &next).await;
    assert_eq!(finished.status, StatusCode::SEE_OTHER, "{}", finished.body);
    assert_eq!(finished.location(), install_url);

    // GitHub's install page, then its setup URL back here.
    let installed = world
        .github
        .get(format!(
            "{install_url}?login=octo-local&repos=octo-local/hello-world"
        ))
        .send()
        .await
        .unwrap();
    let setup = installed.headers()["location"]
        .to_str()
        .unwrap()
        .to_string();
    assert!(setup.starts_with(SETUP_URL), "{setup}");
    let back = browser
        .get(&world, setup.strip_prefix(ORIGIN).unwrap())
        .await;
    assert_eq!(back.status, StatusCode::SEE_OTHER, "{}", back.body);
    assert_eq!(back.location(), PAGE);

    let page = browser.get(&world, PAGE).await;
    assert!(
        page.body.contains("Connected to GitHub as"),
        "{}",
        page.body
    );
    assert!(page.body.contains("Choose repositories on GitHub"));
    crate::copy_guard::assert_plain(PAGE, &page.body);
    let list = browser.get(&world, "/projects/repositories?page=1").await;
    assert!(
        list.body.contains("octo-local/hello-world"),
        "{}",
        list.body
    );
    assert!(!list.body.contains("acme/storefront"));
    assert!(!list.body.contains("octo-local/secret-plans"));
    let csrf = hidden(
        &list.body,
        r#"<form method="post" action="/projects">"#,
        "csrf",
    );
    let added = browser
        .post(
            &world,
            PAGE,
            &[("csrf", &csrf), ("repository", "octo-local/hello-world")],
        )
        .await;
    assert_eq!(added.status, StatusCode::SEE_OTHER, "{}", added.body);
    // A repository not chosen on GitHub says so.
    let refused = browser
        .post(
            &world,
            PAGE,
            &[("csrf", &csrf), ("repository", "acme/storefront")],
        )
        .await;
    assert_eq!(refused.status, StatusCode::NOT_FOUND, "{}", refused.body);
    assert!(
        refused
            .body
            .contains("isn&#39;t installed on that repository")
            || refused.body.contains("isn't installed on that repository"),
        "{}",
        refused.body
    );
    let page = browser.get(&world, PAGE).await;
    assert!(page.body.contains("Your projects"), "{}", page.body);

    // Choosing more repositories goes straight to GitHub now.
    let again = browser.get(&world, INSTALL).await;
    assert_eq!(again.status, StatusCode::SEE_OTHER);
    assert_eq!(again.location(), install_url);
}
