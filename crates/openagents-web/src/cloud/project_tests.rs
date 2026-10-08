//! Project HTTP acceptance through a real isolated resident observer.

use super::*;

const PROJECT: &str = "/cloud/app/hosts/resident/projects/synthetic-project";
async fn connected() -> (Fixture, Resident, Cookies) {
    let mut fixture = fixture().await;
    let native = resident_with_projects(
        &mut fixture,
        coder_access::Rights::new([coder_access::Right::Observe]).unwrap(),
        false,
        true,
    )
    .await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    (fixture, native, cookies)
}
fn directory(fixture: &Fixture) -> PathBuf {
    fixture
        .config
        .cloud_build
        .as_ref()
        .unwrap()
        .parent()
        .unwrap()
        .into()
}
fn links(html: &str) -> Vec<String> {
    html.split("href=\"")
        .skip(1)
        .filter_map(|value| value.split('"').next())
        .map(|value| value.replace("&amp;", "&"))
        .collect()
}
async fn get(fixture: &Fixture, cookies: &Cookies, path: &str) -> Answer {
    request(&fixture.site, Method::GET, path, cookies, None, None).await
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_graph_preserves_held_claims_and_reconstructs_original_native_config() {
    let (fixture, native, cookies) = connected().await;
    let ledger = directory(&fixture).join("project-supervisor/scheduler-ledger.json");
    let before = std::fs::read(&ledger).unwrap();
    let list = get(&fixture, &cookies, "/cloud/app/hosts/resident/projects").await;
    assert_eq!(list.status, StatusCode::OK, "{}", list.body);
    private(&list);
    assert!(list.body.contains("Synthetic supervised project"));
    let page = get(&fixture, &cookies, PROJECT).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    private(&page);
    for evidence in [
        "another-owner",
        "Dependency held has not completed",
        "backpressure",
        "executor_slots",
        "excluded_issues",
        "Native goals",
        "unknown",
    ] {
        assert!(page.body.contains(evidence), "missing {evidence}");
    }
    assert_eq!(std::fs::read(&ledger).unwrap(), before);
    assert!(
        !directory(&fixture)
            .join("project-supervisor/scheduler-ledger.lock")
            .exists()
    );
    assert!(!directory(&fixture).join("browser-controls").exists());
    let urls = links(&page.body)
        .into_iter()
        .filter(|link| link.contains("/original?q="))
        .collect::<Vec<_>>();
    let config_url = urls
        .iter()
        .find(|link| {
            let encoded = link.split("?q=").nth(1).unwrap();
            let value: Value =
                serde_json::from_slice(&URL_SAFE_NO_PAD.decode(encoded).unwrap()).unwrap();
            value["original"]["id"] == "configuration"
        })
        .unwrap();
    let mut current = config_url.clone();
    let mut original = Vec::new();
    let mut chunks = 0;
    loop {
        let chunk = get(&fixture, &cookies, &current).await;
        assert_eq!(chunk.status, StatusCode::OK, "{}", chunk.body);
        private(&chunk);
        let downloaded = get(&fixture, &cookies, &format!("{current}&download=yes")).await;
        assert_eq!(downloaded.status, StatusCode::OK);
        assert_eq!(
            downloaded.headers[header::CONTENT_TYPE],
            "application/octet-stream"
        );
        assert_eq!(
            downloaded.headers[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
        assert!(downloaded.body.len() <= coder_access::project::MAX_CHUNK_BYTES);
        original.extend_from_slice(downloaded.body.as_bytes());
        chunks += 1;
        match links(&chunk.body)
            .into_iter()
            .find(|link| link.contains("/original?q=") && !link.contains("download="))
        {
            Some(next) => current = next,
            None => break,
        }
    }
    assert!(chunks > 1);
    assert_eq!(
        original,
        std::fs::read(directory(&fixture).join("project-supervisor.json")).unwrap()
    );
    let response = request(
        &fixture.site,
        Method::POST,
        PROJECT,
        &cookies,
        Some(""),
        Some(ORIGIN),
    )
    .await;
    assert_eq!(response.status, StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(std::fs::read(&ledger).unwrap(), before);
    native.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn project_alias_originals_and_private_standing_retire_on_native_or_account_change() {
    let (fixture, native, cookies) = connected().await;
    let page = get(&fixture, &cookies, PROJECT).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let original = links(&page.body)
        .into_iter()
        .find(|link| link.contains("/original?q="))
        .unwrap();
    let pin = resource_descriptor(&page.body);
    let endpoint = pin["endpoint"].as_str().unwrap().to_owned();
    let bob = login(&fixture, "bob").await;
    for url in [PROJECT, original.as_str(), endpoint.as_str()] {
        let response = get(&fixture, &bob, url).await;
        assert_eq!(response.status, StatusCode::FORBIDDEN);
        assert!(!response.body.contains("another-owner"));
    }
    let ledger = directory(&fixture).join("project-supervisor/scheduler-ledger.json");
    let mut value: Value = serde_json::from_slice(&std::fs::read(&ledger).unwrap()).unwrap();
    value["sequence"] = json!(10);
    private_file(&ledger, &serde_json::to_vec(&value).unwrap());
    for url in [&original, &endpoint] {
        let response = get(&fixture, &cookies, url).await;
        assert_eq!(response.status, StatusCode::CONFLICT, "{}", response.body);
        assert!(!response.body.contains("another-owner"));
    }
    let fresh = get(&fixture, &cookies, PROJECT).await;
    assert_eq!(fresh.status, StatusCode::OK, "{}", fresh.body);
    assert!(fresh.body.contains("Sequence 10"));
    native.authority.revoke(&native.device, now()).unwrap();
    let response = get(&fixture, &cookies, PROJECT).await;
    assert!(matches!(
        response.status,
        StatusCode::FORBIDDEN | StatusCode::SERVICE_UNAVAILABLE
    ));
    assert!(!response.body.contains("another-owner"));
    native.stop().await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn native_project_policy_is_required_independently_of_observe() {
    let mut fixture = fixture().await;
    let native = resident(&mut fixture).await;
    let mut cookies = login(&fixture, "alice").await;
    choose_personal(&fixture, &mut cookies).await;
    let page = get(&fixture, &cookies, PROJECT).await;
    assert_ne!(page.status, StatusCode::OK);
    assert!(!page.body.contains("another-owner"));
    native.stop().await;
}
