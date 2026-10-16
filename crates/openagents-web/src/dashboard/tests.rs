use super::*;
use axum::{
    body::{Body, to_bytes},
    http::Request,
};
use tower::ServiceExt;

async fn call(
    world: &crate::projects::tests::World,
    browser: &crate::projects::tests::Browser,
    method: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let token = browser.0.get("oa_cloud_session").unwrap();
    let response = world
        .site
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("host", &world.host)
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(serde_json::to_vec(&body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        response.headers().get("cache-control").unwrap(),
        "no-store, private"
    );
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn account_isolation_template_visibility_versions_reports_and_removal_through_real_handlers()
{
    use crate::projects::tests::{Browser, world};
    let world = world().await;
    let mut a = Browser::default();
    let mut b = Browser::default();
    a.sign_in(&world, "octo-local").await;
    b.sign_in(&world, "quiet-local").await;
    let dashboard = Dashboard::starter("private-home");
    let library = Library {
        revision: 0,
        default: Some(dashboard.id.clone()),
        dashboards: vec![dashboard.clone()],
    };
    let write = json!(SaveLibrary {
        request_id: format!("{:064x}", 101),
        expected_revision: 0,
        library
    });
    let (status, saved) = call(&world, &a, "PUT", "/v1/home/dashboards", write.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(saved["revision"], 1);
    assert_eq!(
        call(&world, &a, "PUT", "/v1/home/dashboards", write)
            .await
            .1,
        saved
    );
    assert!(
        call(&world, &b, "GET", "/v1/home/dashboards", Value::Null)
            .await
            .1["dashboards"]
            .as_array()
            .unwrap()
            .is_empty()
    );
    let id = format!("{:064x}", 102);
    let url = format!("/v1/home/templates/{id}");
    let package = dashboard.export("Shared work".into()).unwrap();
    let publish = |request_id, revision, visibility, package| {
        json!(Publish {
            request_id: format!("{request_id:064x}"),
            expected_revision: revision,
            package,
            visibility
        })
    };
    assert_eq!(
        call(
            &world,
            &a,
            "PUT",
            &url,
            publish(103, 0, Visibility::Private, package.clone())
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&world, &b, "GET", &url, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, unlisted) = call(
        &world,
        &a,
        "PUT",
        &url,
        publish(104, 1, Visibility::Unlisted, package.clone()),
    )
    .await;
    assert_eq!(unlisted["version"], 2);
    assert_eq!(
        call(&world, &b, "GET", &url, Value::Null).await.1["owned"],
        false
    );
    assert!(
        !call(&world, &b, "GET", "/v1/home/templates", Value::Null)
            .await
            .1["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == id)
    );
    assert_eq!(
        call(
            &world,
            &b,
            "PUT",
            &url,
            publish(105, 2, Visibility::Public, package.clone())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &world,
            &a,
            "PUT",
            &url,
            publish(106, 1, Visibility::Public, package.clone())
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    let (_, public) = call(
        &world,
        &a,
        "PUT",
        &url,
        publish(107, 2, Visibility::Public, package),
    )
    .await;
    assert_eq!(public["version"], 3);
    assert!(
        call(
            &world,
            &b,
            "GET",
            "/v1/home/templates?q=Shared",
            Value::Null
        )
        .await
        .1["data"]
            .as_array()
            .unwrap()
            .iter()
            .any(|v| v["id"] == id)
    );
    assert_eq!(
        call(&world, &b, "GET", &format!("{url}?version=2"), Value::Null)
            .await
            .1["version"],
        2
    );
    assert_eq!(
        call(
            &world,
            &b,
            "POST",
            &format!("{url}/report"),
            json!(Report {
                request_id: format!("{:064x}", 108),
                reason: "The description is misleading.".into()
            })
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(
            &world,
            &b,
            "DELETE",
            &url,
            json!(Remove {
                request_id: format!("{:064x}", 109),
                expected_revision: 3
            })
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(
            &world,
            &a,
            "DELETE",
            &url,
            json!(Remove {
                request_id: format!("{:064x}", 110),
                expected_revision: 3
            })
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        call(&world, &b, "GET", &url, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        call(&world, &b, "GET", &format!("{url}?version=2"), Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn saved_layouts_survive_restart_and_receipts_prevent_lost_response_duplicates() {
    let root = tempfile::tempdir().unwrap();
    let store = Store::local(root.path().into());
    let owner = crate::chat_store::account_owner("acct-home-test");
    let key = Store::owner_key(&owner, "home.json").unwrap();
    let request = format!("{:064x}", 1);
    let change = |b: &mut Book| {
        b.library.revision += 1;
        Ok(json!(b.library))
    };
    assert_eq!(
        mutate(&store, &key, &owner, &request, "first", change)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        mutate(&store, &key, &owner, &request, "first", change)
            .await
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        mutate(&store, &key, &owner, &request, "different", change)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    let reopened = Store::local(root.path().into());
    assert_eq!(load(&reopened, &key).await.unwrap().0.library.revision, 1);
    let other =
        Store::owner_key(&crate::chat_store::account_owner("acct-other"), "home.json").unwrap();
    assert_eq!(load(&reopened, &other).await.unwrap().0.library.revision, 0);
}

#[test]
fn count_and_destination_query_share_filters_and_exclude_pull_requests() {
    let filter = IssueFilter {
        scope: Scope::default(),
        mine: true,
        assignee: None,
        labels: vec!["bug".into()],
        page: 1,
    };
    let query = issues::query(
        &filter,
        &["OpenAgentsInc/openagents".into()],
        Some("someone"),
    );
    assert_eq!(
        query,
        "is:issue is:open repo:OpenAgentsInc/openagents assignee:someone label:\"bug\""
    );
}
