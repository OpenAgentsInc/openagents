//! `/v1/projects` for apps and agents (#11471), through the whole site:
//! the person's own sign-in token lists, shows, adds, and removes their
//! projects and reads a project's threads and their messages;
//! another account sees none of it, and no token is refused as the app API
//! refuses.

use super::*;
use serde_json::Value;

struct Api {
    token: Option<String>,
}

impl Api {
    fn of(browser: &Browser) -> Self {
        Self {
            token: browser.0.get("oa_cloud_session").cloned(),
        }
    }

    async fn call(
        &self,
        world: &World,
        method: &str,
        path: &str,
        body: Body,
    ) -> (StatusCode, HeaderMap, Vec<u8>) {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, world.host.as_str())
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = &self.token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let response = world
            .site
            .clone()
            .oneshot(request.body(body).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec();
        (status, headers, bytes)
    }

    async fn json(
        &self,
        world: &World,
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let body = if body.is_null() {
            Body::empty()
        } else {
            Body::from(body.to_string())
        };
        let (status, _, bytes) = self.call(world, method, path, body).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

fn chat(owner: &str, id: &str, title: &str, project: Option<&str>, updated: u64) -> Conversation {
    serde_json::from_value(json!({
        "id": id,
        "owner": owner,
        "revision": 1,
        "title": title,
        "messages": [{"role": "user", "text": title}],
        "pending": null,
        "requests": [],
        "updated_unix": updated,
        "project": project,
    }))
    .unwrap()
}

#[tokio::test]
async fn the_sign_in_token_reads_and_changes_its_own_projects_only() {
    let world = world().await;
    assert!(crate::upstream::owned("/v1/projects"));
    assert!(crate::upstream::owned("/v1/projects/prj_0/attachments"));
    assert!(!crate::upstream::owned("/v1/projectsx"));

    // No token: refused in the app API's words.
    let (status, body) = Api { token: None }
        .json(&world, "GET", "/v1/projects", Value::Null)
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"]["code"], "signed_out");

    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    connect_public(&mut browser, &world, "octo-local").await;
    let me = Api::of(&browser);
    let (status, body) = me.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"data": [], "next": null}));

    // Add a project.
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"repository": "octo-local/hello-world"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let id = body["id"].as_str().unwrap().to_owned();
    assert_eq!(body["object"], "project");
    assert_eq!(body["name"], "hello-world");
    assert!(body["created_at"].as_str().unwrap().ends_with('Z'));
    let (status, body) = me
        .json(&world, "POST", "/v1/projects", json!({"name": "finances"}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Chats: two in the project, one outside it, one of someone else's.
    let owner = account_owner_of(&world, "octo-local");
    let first = "11111111-1111-4111-8111-111111111111";
    let second = "22222222-2222-4222-8222-222222222222";
    let loose = "33333333-3333-4333-8333-333333333333";
    world
        .store
        .create(&chat(&owner, first, "Budget for October", Some(&id), 100))
        .await
        .unwrap();
    world
        .store
        .create(&chat(&owner, second, "Taxes", Some(&id), 200))
        .await
        .unwrap();
    world
        .store
        .create(&chat(&owner, loose, "Elsewhere", None, 300))
        .await
        .unwrap();
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let them = Api::of(&other);
    let their_owner = account_owner_of(&world, "quiet-local");
    world
        .store
        .create(&chat(
            &their_owner,
            "44444444-4444-4444-8444-444444444444",
            "Their plan",
            Some(&id),
            400,
        ))
        .await
        .unwrap();

    let (status, body) = me.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["id"], id.as_str());
    assert_eq!(body["data"][0]["counts"]["threads"], 2);
    assert_eq!(body["next"], Value::Null);

    // By id and by name.
    for reference in [id.clone(), "name:hello-world".to_owned()] {
        let (status, body) = me
            .json(
                &world,
                "GET",
                &format!("/v1/projects/{reference}"),
                Value::Null,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["id"], id.as_str());
    }
    let (status, _) = me
        .json(&world, "GET", "/v1/projects/name:nope", Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // The project's threads, oldest activity first, one page at a time.
    let (status, body) = me
        .json(
            &world,
            "GET",
            "/v1/threads?project=name:hello-world&limit=1",
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["title"], "Budget for October");
    assert_eq!(body["data"][0]["object"], "thread");
    assert_eq!(body["next"], first);
    let (status, body) = me
        .json(
            &world,
            "GET",
            &format!("/v1/threads?project={id}&after={first}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let titles: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["title"].as_str().unwrap())
        .collect();
    assert_eq!(titles, ["Taxes"]);
    assert_eq!(body["next"], Value::Null);
    assert!(!body.to_string().contains("Their plan"));
    let (status, body) = me
        .json(&world, "GET", "/v1/threads?project=none", Value::Null)
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["title"], "Elsewhere");
    let (status, body) = me
        .json(
            &world,
            "GET",
            &format!("/v1/threads/{second}/messages"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["content"][0]["text"], "Taxes");
    assert_eq!(body["data"][0]["role"], "user");

    // Someone else's token: the project and its threads aren't theirs.
    for path in [
        format!("/v1/projects/{id}"),
        format!("/v1/threads?project={id}"),
        format!("/v1/threads/{second}/messages"),
    ] {
        let (status, body) = them.json(&world, "GET", &path, Value::Null).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}: {body}");
    }
    let (status, _) = them
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}?detach_threads=true"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Removing a project with threads in it asks first; then the threads
    // stay, outside any project.
    let (status, body) = me
        .json(&world, "DELETE", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "project_has_threads");
    let (status, body) = me
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}?detach_threads=true"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body, json!({"id": id, "object": "project.deleted"}));
    let (status, _) = me
        .json(&world, "GET", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let chats = world.store.list(&owner).await.unwrap();
    assert_eq!(chats.len(), 3);
    assert!(chats.iter().all(|chat| chat.project.is_none()));
}
