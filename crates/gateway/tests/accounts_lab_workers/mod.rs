use super::*;

fn enrollment(space: &str) -> Value {
    json!({"space":space,"lifetime_ms":60000,"capabilities":{
        "worker":"fixture","placement":"enrolled","cpu_millis":1000,"memory_bytes":134217728,
        "gpu":false,"tools":[],"jobs":["echo/shout"],"local_assets":[],"max_seconds":600,
    }})
}

#[tokio::test]
async fn job_only_keys_cannot_enter_account_routes_and_end_with_the_issuing_session() {
    let d = deploy(Some(account_config(None)), false).await;
    let alice = join(&d, "Worker Alice").await;
    let bob = join(&d, "Worker Bob").await;
    let (status, body) = post(
        &d,
        "/v1/account/lab-workers",
        Some(&alice.session_token),
        &enrollment("me"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = body["token"].as_str().unwrap();
    let id = body["worker"]["id"].as_str().unwrap();
    assert_eq!(body["worker"]["account"], alice.account);
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        get(&d, "/v1/account", token).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post(
            &d,
            "/v1/account/sign-in-keys",
            Some(token),
            &json!({"name":"forbidden"})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        remove(
            &d,
            &format!("/v1/account/lab-workers/{id}"),
            &bob.session_token
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        remove(&d, "/v1/session", &alice.session_token).await.0,
        StatusCode::OK
    );
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
async fn worker_revocation_and_project_deletion_are_checked_before_each_access() {
    let d = deploy(Some(account_config(None)), false).await;
    let alice = join(&d, "Worker project owner").await;
    let (status, project) = post(
        &d,
        "/v1/projects",
        Some(&alice.session_token),
        &json!({"name":"Worker fixture"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let id = project["project"]["id"]
        .as_str()
        .or_else(|| project["id"].as_str())
        .unwrap();
    let (status, body) = post(
        &d,
        "/v1/account/lab-workers",
        Some(&alice.session_token),
        &enrollment(&format!("project:{id}")),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = body["token"].as_str().unwrap();
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        remove(&d, &format!("/v1/projects/{id}"), &alice.session_token)
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, body) = post(
        &d,
        "/v1/account/lab-workers",
        Some(&alice.session_token),
        &enrollment("me"),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let token = body["token"].as_str().unwrap();
    let id = body["worker"]["id"].as_str().unwrap();
    assert_eq!(
        remove(
            &d,
            &format!("/v1/account/lab-workers/{id}"),
            &alice.session_token
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        post(&d, "/v1/account/lab-worker-access", Some(token), &json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
}
