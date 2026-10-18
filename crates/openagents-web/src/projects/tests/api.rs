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

#[tokio::test]
async fn project_contract_preserves_partial_changes_and_cursor_errors() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let (status, failed) = api
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({
                "name":"Created before protection failed", "protection":"sealed"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(failed["error"]["code"], "not_available");
    let (_, list) = api.json(&world, "GET", "/v1/projects", Value::Null).await;
    let id = list["data"][0]["id"].as_str().unwrap();
    assert_eq!(list["data"][0]["name"], "Created before protection failed");
    let (status, _) = api
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({
                "protection":"only_me", "name":"\n"
            }),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (_, after) = api
        .json(&world, "GET", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(after["protection"], "only_me");
    assert_eq!(after["name"], "Created before protection failed");
    assert_eq!(
        api.json(&world, "DELETE", &format!("/v1/projects/{id}"), Value::Null)
            .await
            .0,
        StatusCode::OK
    );

    let mut ids = vec![];
    for _ in 0..2 {
        let response = world
            .site
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/projects")
                    .header(header::HOST, world.host.as_str())
                    .header(header::CONTENT_TYPE, "application/json")
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", api.token.as_ref().unwrap()),
                    )
                    .header("Idempotency-Key", "same-request")
                    .body(Body::from(json!({"name":"Duplicate name"}).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let body: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 65536).await.unwrap()).unwrap();
        ids.push(body["id"].as_str().unwrap().to_owned());
    }
    assert_ne!(ids[0], ids[1]);
    let (status, ambiguous) = api
        .json(
            &world,
            "GET",
            "/v1/projects/name:Duplicate%20name",
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(ambiguous["error"]["code"], "ambiguous_name");
    let (_, page) = api
        .json(&world, "GET", "/v1/projects?limit=0", Value::Null)
        .await;
    assert_eq!(page["data"].as_array().unwrap().len(), 1);
    assert!(page["next"].is_string());
    let (status, cursor) = api
        .json(&world, "GET", "/v1/projects?after=unknown", Value::Null)
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(cursor["error"]["code"], "invalid_cursor");
    let mut stranger = Browser::default();
    stranger.sign_in(&world, "quiet-local").await;
    let other = Api::of(&stranger);
    let response = world
        .site
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/v1/projects/{}", ids[0]))
                .header(header::HOST, world.host.as_str())
                .header(
                    header::AUTHORIZATION,
                    format!("Bearer {}", other.token.as_ref().unwrap()),
                )
                .header("X-Workspace-Id", world.account_of("octo-local"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    for id in ids {
        assert_eq!(
            api.json(&world, "DELETE", &format!("/v1/projects/{id}"), Value::Null)
                .await
                .0,
            StatusCode::OK
        );
    }
    assert_eq!(
        api.json(&world, "GET", "/v1/projects", Value::Null).await.1["data"],
        json!([])
    );
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
    assert_eq!(body["repository"], "octo-local/hello-world");
    assert_eq!(body["repositories"][0]["primary"], true);

    let (status, access) = me
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/access"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        access["data"],
        json!([{
            "repository":"octo-local/hello-world", "access":true, "message":null
        }])
    );

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

/// `POST /v1/threads {project}` (#11542): an app starts a chat inside one
/// of the person's projects, as starting a chat in a project on the web
/// does; another account's project, or an unknown one, is refused and
/// starts nothing; a reply can't move a chat.
#[tokio::test]
async fn an_app_starts_a_chat_inside_its_own_project() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    connect_public(&mut browser, &world, "octo-local").await;
    let me = Api::of(&browser);
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"repository": "octo-local/hello-world"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let project = body["id"].as_str().unwrap().to_owned();

    let id = "55555555-5555-4555-8555-555555555555";
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": id, "text": "Testing. Hello.", "project": "name:hello-world"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["id"], id);
    let owner = account_owner_of(&world, "octo-local");
    let saved = world.store.load(&owner, id).await.unwrap().unwrap();
    assert_eq!(
        saved.conversation.project.as_deref(),
        Some(project.as_str())
    );
    let (status, body) = me
        .json(
            &world,
            "GET",
            &format!("/v1/threads?project={project}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"][0]["id"], id);

    // Someone else's project, by id: not theirs, and no chat.
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let them = Api::of(&other);
    let theirs = "66666666-6666-4666-8666-666666666666";
    let (status, body) = them
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": theirs, "text": "Hello", "project": project}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let their_owner = account_owner_of(&world, "quiet-local");
    assert!(
        world
            .store
            .load(&their_owner, theirs)
            .await
            .unwrap()
            .is_none()
    );

    // A reply carries no project.
    let (status, _) = me
        .json(
            &world,
            "POST",
            &format!("/v1/threads/{id}/messages"),
            json!({"request_id": "77777777-7777-4777-8777-777777777777", "text": "Hi", "project": project}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// #11541: a synced Coder chat files under the project for its repository,
/// and the sign-in token moves a chat into a project or out of it with
/// `PATCH /v1/threads/{id}`; a moved chat stays where the person put it.
#[tokio::test]
async fn synced_chats_file_under_their_repository_and_move_with_the_token() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    connect_public(&mut browser, &world, "octo-local").await;
    let me = Api::of(&browser);
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

    // Coder syncs a chat run in a checkout of that repository.
    let upload = |repository: &str| {
        json!({"computer": "Studio", "title": "Fix the build",
               "messages": [{"role": "user", "text": "Fix it"}],
               "repository": repository})
    };
    let (status, body) = me
        .json(
            &world,
            "PUT",
            "/v1/threads/synced/s-filed",
            upload("Octo-Local/Hello-World"),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let chat = body["chat"].as_str().unwrap().to_owned();
    let (_, listed) = me
        .json(
            &world,
            "GET",
            "/v1/threads?project=name:hello-world",
            Value::Null,
        )
        .await;
    assert_eq!(listed["data"][0]["id"], chat.as_str(), "{listed}");
    // Another repository, or none, files nowhere.
    let (_, body) = me
        .json(
            &world,
            "PUT",
            "/v1/threads/synced/s-other",
            upload("octo-local/other"),
        )
        .await;
    let other = body["chat"].as_str().unwrap().to_owned();
    let (_, loose) = me
        .json(&world, "GET", "/v1/threads?project=none", Value::Null)
        .await;
    assert_eq!(loose["data"][0]["id"], other.as_str(), "{loose}");

    // The person takes the filed chat out; the next sync leaves it out.
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{chat}"),
            json!({"project": null}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["project"], Value::Null);
    let mut more = upload("octo-local/hello-world");
    more["messages"]
        .as_array_mut()
        .unwrap()
        .push(json!({"role": "assistant", "text": "Done."}));
    let (status, _) = me
        .json(&world, "PUT", "/v1/threads/synced/s-filed", more)
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, listed) = me
        .json(
            &world,
            "GET",
            "/v1/threads?project=name:hello-world",
            Value::Null,
        )
        .await;
    assert_eq!(listed["data"], json!([]), "{listed}");

    // Moved in by name, then by id.
    for reference in ["name:hello-world".to_owned(), id.clone()] {
        let (status, body) = me
            .json(
                &world,
                "PATCH",
                &format!("/v1/threads/{other}"),
                json!({"project": reference}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["project"], id.as_str());
    }
    // Not a project of mine, a bad body, or another account's token.
    let (status, _) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{other}"),
            json!({"project": "prj_0000000000000000"}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = me
        .json(&world, "PATCH", &format!("/v1/threads/{other}"), json!({}))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut stranger = Browser::default();
    stranger.sign_in(&world, "quiet-local").await;
    let (status, _) = Api::of(&stranger)
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{other}"),
            json!({"project": null}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

/// #12026: the sign-in token pins and archives a chat with
/// `PATCH /v1/threads/{id}`, as the web's sidebar does: pins keep the
/// order they were made in, and an empty or mistyped body is refused.
#[tokio::test]
async fn the_token_pins_and_archives_a_chat_like_the_sidebar() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let me = Api::of(&browser);
    let mut ids = vec![];
    for name in ["s-first", "s-second"] {
        let (status, body) = me
            .json(
                &world,
                "PUT",
                &format!("/v1/threads/synced/{name}"),
                json!({"computer": "Studio", "title": "Fix the build",
                       "messages": [{"role": "user", "text": "Fix it"}]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        ids.push(body["chat"].as_str().unwrap().to_owned());
    }
    let owner = account_owner_of(&world, "octo-local");
    let pinned_at = |id: &str| {
        let store = world.store.clone();
        let owner = owner.clone();
        let id = id.to_owned();
        async move {
            let saved = store.load(&owner, &id).await.unwrap().unwrap();
            (
                saved.conversation.pinned_unix,
                saved.conversation.archived_unix,
            )
        }
    };
    for id in &ids {
        let (status, body) = me
            .json(
                &world,
                "PATCH",
                &format!("/v1/threads/{id}"),
                json!({"pinned": true}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["pinned"], true);
        assert_eq!(body["archived"], false);
    }
    let (first, _) = pinned_at(&ids[0]).await;
    let (second, _) = pinned_at(&ids[1]).await;
    assert!(second.unwrap() > first.unwrap(), "{first:?} {second:?}");
    // Pinning again changes nothing.
    let (status, _) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{}", ids[0]),
            json!({"pinned": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pinned_at(&ids[0]).await.0, first);

    // Archive and unpin at once, then restore.
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{}", ids[0]),
            json!({"pinned": false, "archived": true}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["pinned"].clone(), body["archived"].clone()),
        (json!(false), json!(true))
    );
    let (pin, archived) = pinned_at(&ids[0]).await;
    assert!(pin.is_none() && archived.is_some());
    let (_, listed) = me.json(&world, "GET", "/v1/threads", Value::Null).await;
    assert!(
        !listed.to_string().contains(ids[0].as_str()),
        "an archived chat leaves the list: {listed}"
    );
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{}", ids[0]),
            json!({"archived": false}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["archived"], false);
    assert!(pinned_at(&ids[0]).await.1.is_none());

    // Nothing asked, a mistyped flag, or another account's token.
    for bad in [json!({}), json!({"pinned": "yes"}), json!({"archived": 1})] {
        let (status, body) = me
            .json(&world, "PATCH", &format!("/v1/threads/{}", ids[1]), bad)
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    }
    let mut stranger = Browser::default();
    stranger.sign_in(&world, "quiet-local").await;
    let (status, _) = Api::of(&stranger)
        .json(
            &world,
            "PATCH",
            &format!("/v1/threads/{}", ids[1]),
            json!({"archived": true}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(pinned_at(&ids[1]).await.1.is_none());
}

/// 2026-10-13: a project needs no repository; it can hold several, one the
/// primary; the same repository can be in several projects; names may
/// repeat, and a name two projects share asks which one.
#[tokio::test]
async fn projects_need_no_repository_and_share_them() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let me = Api::of(&browser);

    // A name is enough, even before GitHub is connected.
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "finances", "instructions": "Keep it short."}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let bare = body["id"].as_str().unwrap().to_owned();
    assert_eq!(body["repositories"], json!([]));
    assert_eq!(body["repository"], Value::Null);
    assert_eq!(body["default_branch"], Value::Null);
    assert_eq!(body["instructions"], "Keep it short.");
    let (status, body) = me.json(&world, "POST", "/v1/projects", json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // A chat started in it is a plain chat in the project.
    let chat = "88888888-8888-4888-8888-888888888888";
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/threads",
            json!({"request_id": chat, "text": "Hello", "project": "name:finances"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let owner = account_owner_of(&world, "octo-local");
    let saved = world.store.load(&owner, chat).await.unwrap().unwrap();
    assert_eq!(saved.conversation.project.as_deref(), Some(bare.as_str()));

    // Two repositories: the first is the primary until another is made it.
    connect_private(&mut browser, &world, "octo-local").await;
    let me = Api::of(&browser);
    let add = |repository: &'static str, primary: bool| {
        let (me, world, bare) = (&me, &world, &bare);
        async move {
            me.json(
                world,
                "POST",
                &format!("/v1/projects/{bare}/repositories"),
                json!({"repository": repository, "primary": primary}),
            )
            .await
        }
    };
    let (status, body) = add("octo-local/hello-world", false).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["repository"], "octo-local/hello-world");
    let (status, body) = add("acme/storefront", false).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let primaries = |body: &Value| -> Vec<(String, bool)> {
        body["repositories"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                (
                    r["repository"].as_str().unwrap().to_owned(),
                    r["primary"].as_bool().unwrap(),
                )
            })
            .collect()
    };
    assert_eq!(
        primaries(&body),
        [
            ("octo-local/hello-world".to_owned(), true),
            ("acme/storefront".to_owned(), false)
        ]
    );
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{bare}"),
            json!({"primary_repository": "acme/storefront", "name": "money"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["name"], "money");
    assert_eq!(body["repository"], "acme/storefront");
    // Removing the primary makes the first one left the primary.
    let (status, body) = me
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{bare}/repositories/acme/storefront"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        primaries(&body),
        [("octo-local/hello-world".to_owned(), true)]
    );
    let (status, body) = me
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{bare}/repositories/acme/storefront"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");

    // The same repository again, in a new project with the same name:
    // creating never reuses one.
    let (status, body) = me
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "money", "repositories": ["octo-local/hello-world"]}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    let second = body["id"].as_str().unwrap().to_owned();
    assert_ne!(second, bare);
    let (status, body) = me
        .json(&world, "GET", "/v1/projects/name:money", Value::Null)
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "ambiguous_name");
    let message = body["error"]["message"].as_str().unwrap();
    assert!(
        message.starts_with("2 projects are called money:")
            && message.contains(&bare)
            && message.contains(&second),
        "{message}"
    );
    let (status, body) = me.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["data"].as_array().unwrap().len(), 2);
}

/// Message text with newlines, tabs, and other control characters comes
/// back from the threads API as strict JSON: every control character
/// escaped, none raw, so a strict parser (Python's `json.loads`) reads
/// it.
#[tokio::test]
async fn thread_text_with_control_characters_comes_back_as_strict_json() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let me = Api::of(&browser);
    let text = "Line one\nLine two\tafter a tab\r\nA bell \u{7} and \u{1f} end";
    let (status, body) = me
        .json(
            &world,
            "PUT",
            "/v1/threads/synced/s-control",
            json!({"computer": "Studio", "title": "Tabs\tand\nnewlines",
                   "messages": [{"role": "user", "text": text},
                                {"role": "assistant", "text": text}]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let chat = body["chat"].as_str().unwrap().to_owned();
    for path in [
        format!("/v1/threads/{chat}"),
        format!("/v1/threads/{chat}/messages"),
        "/v1/threads".to_owned(),
    ] {
        let (status, _, bytes) = me.call(&world, "GET", &path, Body::empty()).await;
        assert_eq!(status, StatusCode::OK, "{path}");
        assert!(
            !bytes.iter().any(|byte| *byte < 0x20),
            "{path} has a raw control character"
        );
        let parsed: Value = serde_json::from_slice(&bytes).unwrap();
        if path.ends_with(&chat) {
            assert_eq!(parsed["messages"][0]["text"], text, "{parsed}");
        }
    }
}
