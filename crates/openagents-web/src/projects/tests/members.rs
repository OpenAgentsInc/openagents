//! Shared projects over `/v1` (#11585), through the whole site and the
//! local account service: the owner invites, a second account previews
//! and joins, reads the project, its files, and its notes by role, and
//! loses it all on the request after it is removed; an `only_me` project
//! can't be shared; leaving and giving the project away.

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

    async fn raw(
        &self,
        world: &World,
        method: &str,
        path: &str,
        body: Body,
    ) -> (StatusCode, Vec<u8>) {
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
        let bytes = to_bytes(response.into_body(), 32 * 1024 * 1024)
            .await
            .unwrap()
            .to_vec();
        (status, bytes)
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
        let (status, bytes) = self.raw(world, method, path, body).await;
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }
}

async fn signed_in(world: &World, login: &str) -> Api {
    let mut browser = Browser::default();
    browser.sign_in(world, login).await;
    Api::of(&browser)
}

#[tokio::test]
async fn an_invited_member_works_by_role_and_loses_access_when_removed() {
    let world = world().await;
    let owner = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;

    let (status, made) = owner
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "household", "instructions": "Be brief."}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let id = made["id"].as_str().unwrap().to_owned();
    assert_eq!(made["role"], "owner");
    assert_eq!(made["shared"], false);
    let (_, note) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "Plan", "text": "Pay the rent."}}),
        )
        .await;
    let note = note["id"].as_str().unwrap().to_owned();

    // Not in it: 404 everywhere, never 403.
    for (method, path) in [
        ("GET", format!("/v1/projects/{id}")),
        ("GET", format!("/v1/projects/{id}/members")),
        ("GET", format!("/v1/projects/{id}/attachments")),
        ("POST", format!("/v1/projects/{id}/invitations")),
    ] {
        let (status, body) = other.json(&world, method, &path, json!({})).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{method} {path}: {body}");
    }

    // A viewer's link: previewed, then used.
    let (status, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{invited}");
    let url = invited["url"].as_str().unwrap().to_owned();
    assert!(url.contains("/projects/join/prjinv_"), "{url}");
    assert_eq!(invited["role"], "viewer");
    let (status, offered) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/preview",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{offered}");
    assert_eq!(offered["project"]["name"], "household");
    assert_eq!(offered["role"], "viewer");
    assert_eq!(offered["member"], Value::Null);
    let (status, joined) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    assert_eq!(joined["role"], "viewer");
    assert_eq!(joined["shared"], true);
    // The link was single-use.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": url}),
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    assert_eq!(body["error"]["code"], "invitation_not_found");

    // The viewer lists it with their role, and reads its notes.
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"][0]["id"], id.as_str());
    assert_eq!(listed["data"][0]["role"], "viewer");
    assert_eq!(listed["data"][0]["instructions"], "Be brief.");
    assert_eq!(listed["data"][0]["counts"]["documents"], 1);
    let (status, doc) = other
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/documents/{note}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{doc}");
    assert_eq!(doc["text"], "Pay the rent.");
    let (_, people) = other
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/members"),
            Value::Null,
        )
        .await;
    assert_eq!(people["data"].as_array().unwrap().len(), 2, "{people}");
    assert!(people.get("invitations").is_none());
    let me = people["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["you"] == true)
        .unwrap()["account"]
        .as_str()
        .unwrap()
        .to_owned();

    // ...and changes nothing.
    for (method, path, body) in [
        (
            "PATCH",
            format!("/v1/projects/{id}"),
            json!({"instructions": "x"}),
        ),
        (
            "PATCH",
            format!("/v1/projects/{id}"),
            json!({"name": "mine"}),
        ),
        (
            "POST",
            format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "t", "text": "x"}}),
        ),
        (
            "DELETE",
            format!("/v1/projects/{id}/attachments/{note}"),
            Value::Null,
        ),
        ("POST", format!("/v1/projects/{id}/invitations"), json!({})),
        (
            "DELETE",
            format!("/v1/projects/{id}?detach_threads=true"),
            Value::Null,
        ),
        (
            "POST",
            "/v1/threads".to_owned(),
            json!({"request_id": "6f0c3c40-6a7e-4f43-9d55-5a1f3f7c4b10", "text": "hi", "project": id}),
        ),
    ] {
        let (status, body) = other.json(&world, method, &path, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {body}");
        assert_eq!(body["error"]["code"], "project_role", "{body}");
    }

    // An editor adds notes and changes instructions, but doesn't rename.
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}/members/{me}"),
            json!({"role": "editor"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (status, body) = other
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"instructions": "Be kind."}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["instructions"], "Be kind.");
    let (status, _) = other
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"name": "mine"}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, added) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"document": {"title": "Groceries", "text": "Milk."}}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{added}");
    // The owner sees the editor's note: it is kept with the project.
    let (_, kept) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{id}/attachments"),
            Value::Null,
        )
        .await;
    assert_eq!(kept["data"].as_array().unwrap().len(), 2, "{kept}");
    // An editor's file moves into the project, and both read it.
    let (status, _, upload) = {
        let (status, bytes) = other
            .raw(
                &world,
                "POST",
                "/v1/files?purpose=project&filename=list.txt",
                Body::from("eggs"),
            )
            .await;
        (status, (), serde_json::from_slice::<Value>(&bytes).unwrap())
    };
    assert_eq!(status, StatusCode::CREATED, "{upload}");
    let file = upload["id"].as_str().unwrap().to_owned();
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/attachments"),
            json!({"file": file}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    for reader in [&owner, &other] {
        let (status, bytes) = reader
            .raw(
                &world,
                "GET",
                &format!("/v1/files/{file}/content?project={id}"),
                Body::empty(),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(bytes, b"eggs");
    }

    // Removed: the next request finds nothing.
    let (status, body) = owner
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}/members/{me}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"], json!([]));
    for path in [
        format!("/v1/projects/{id}"),
        format!("/v1/projects/{id}/documents/{note}"),
        format!("/v1/files/{file}/content?project={id}"),
    ] {
        let (status, _) = other.json(&world, "GET", &path, Value::Null).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn leaving_giving_away_and_only_me_projects() {
    let world = world().await;
    let owner = signed_in(&world, "octo-local").await;
    let other = signed_in(&world, "quiet-local").await;
    let (_, made) = owner
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name": "taxes", "protection": "only_me"}),
        )
        .await;
    let id = made["id"].as_str().unwrap().to_owned();
    // Files that open only on the owner's devices: no invitations.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(
        body["error"]["message"],
        "Files in this project open only on your devices. Change it to standard to share it."
    );
    owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "standard"}),
        )
        .await;
    let (_, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
        )
        .await;
    let (status, joined) = other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": invited["code"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    assert_eq!(joined["role"], "editor");
    // Shared: it can't go back to only_me.
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "only_me"}),
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    // The owner can't leave; the member can.
    let (status, body) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/leave"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let (status, body) = other
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/leave"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["object"], "project.left");
    let (status, _) = other
        .json(&world, "GET", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Back in, then given the project: the old owner edits.
    let (_, invited) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role": "viewer"}),
        )
        .await;
    other
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code": invited["code"]}),
        )
        .await;
    let (_, people) = owner
        .json(
            &world,
            "GET",
            &format!("/v1/projects/name:taxes/members"),
            Value::Null,
        )
        .await;
    let theirs = people["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["you"] == false)
        .unwrap()["account"]
        .as_str()
        .unwrap()
        .to_owned();
    let (status, body) = owner
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}/members/{theirs}"),
            json!({"role": "owner"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["role"], "editor");
    let (_, listed) = other.json(&world, "GET", "/v1/projects", Value::Null).await;
    assert_eq!(listed["data"][0]["role"], "owner");
    let (status, _) = owner
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({}),
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
