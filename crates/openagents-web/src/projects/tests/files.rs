//! A project's private data over `/v1` (#11471): files uploaded and
//! attached, text documents, the `only_me` protection that keeps files in
//! the person's vault, and another account seeing none of it.

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

impl Api {
    async fn upload(&self, world: &World, name: &str, bytes: &[u8]) -> (StatusCode, Value) {
        let (status, _, body) = self
            .call(
                world,
                "POST",
                &format!("/v1/files?purpose=project&filename={name}"),
                Body::from(bytes.to_vec()),
            )
            .await;
        (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
    }
}

#[tokio::test]
async fn file_contract_rechecks_roles_without_a_project_query() {
    let world = world().await;
    let mut first = Browser::default();
    first.sign_in(&world, "octo-local").await;
    let original = Api::of(&first);
    let mut second = Browser::default();
    second.sign_in(&world, "quiet-local").await;
    let next = Api::of(&second);
    let (_, project) = original
        .json(
            &world,
            "POST",
            "/v1/projects",
            json!({"name":"Shared file rights"}),
        )
        .await;
    let project = project["id"].as_str().unwrap();
    let (status, file) = original
        .upload(&world, "example.txt", b"example file")
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let file = file["id"].as_str().unwrap();
    assert_eq!(
        original
            .json(
                &world,
                "POST",
                &format!("/v1/projects/{project}/attachments"),
                json!({"file":file})
            )
            .await
            .0,
        StatusCode::CREATED
    );
    let (_, invite) = original
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{project}/invitations"),
            json!({}),
        )
        .await;
    assert_eq!(
        next.json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code":invite["code"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    let original_id = world.account_of("octo-local");
    let next_id = world.account_of("quiet-local");
    assert_eq!(
        original
            .json(
                &world,
                "PATCH",
                &format!("/v1/projects/{project}/members/{next_id}"),
                json!({"role":"owner"})
            )
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        next.json(
            &world,
            "PATCH",
            &format!("/v1/projects/{project}/members/{original_id}"),
            json!({"role":"viewer"})
        )
        .await
        .0,
        StatusCode::OK
    );
    let direct = format!("/v1/files/{file}");
    assert_eq!(
        original.json(&world, "GET", &direct, Value::Null).await.0,
        StatusCode::OK
    );
    let (status, denied) = original.json(&world, "DELETE", &direct, Value::Null).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(denied["error"]["code"], "project_role");
    assert_eq!(
        next.json(
            &world,
            "DELETE",
            &format!("/v1/projects/{project}/members/{original_id}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    for path in [direct.clone(), format!("{direct}/content")] {
        assert_eq!(
            original.call(&world, "GET", &path, Body::empty()).await.0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(
        original
            .json(&world, "DELETE", &direct, Value::Null)
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        next.call(
            &world,
            "GET",
            &format!("{direct}/content?project={project}"),
            Body::empty()
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        next.json(
            &world,
            "DELETE",
            &format!("{direct}?project={project}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        next.json(
            &world,
            "DELETE",
            &format!("/v1/projects/{project}"),
            Value::Null
        )
        .await
        .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn file_contract_rechecks_sessions_after_external_revocation() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let (status, file) = api.upload(&world, "example.txt", b"example file").await;
    assert_eq!(status, StatusCode::CREATED);
    let path = format!("/v1/files/{}", file["id"].as_str().unwrap());
    assert_eq!(
        api.json(&world, "GET", &path, Value::Null).await.0,
        StatusCode::OK
    );
    tenancy::sessions::Sessions::open(&world.root().join("accounts"))
        .unwrap()
        .mutate(|book, _, now| {
            let id = book
                .session_of_token(api.token.as_ref().unwrap())
                .unwrap()
                .id
                .clone();
            book.logout(&id, now)
        })
        .unwrap();
    for path in [path.clone(), format!("{path}/content")] {
        assert_eq!(
            api.call(&world, "GET", &path, Body::empty()).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    let mut fresh = Browser::default();
    fresh.sign_in(&world, "octo-local").await;
    assert_eq!(
        Api::of(&fresh)
            .json(&world, "DELETE", &path, Value::Null)
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn a_project_keeps_files_documents_and_vault_files_for_its_owner_only() {
    let world = world().await;
    assert!(crate::upstream::owned("/v1/files"));
    assert!(crate::upstream::owned("/v1/files/file_0/content"));
    assert!(!crate::upstream::owned("/v1/filesx"));

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
    assert_eq!(body["protection"], "standard");
    let attachments = format!("/v1/projects/{id}/attachments");

    // Upload, then attach.
    let csv = b"date,amount\n2026-09-01,12.50\n";
    let (status, file) = me.upload(&world, "september.csv", csv).await;
    assert_eq!(status, StatusCode::CREATED, "{file}");
    let file_id = file["id"].as_str().unwrap().to_owned();
    assert!(file_id.starts_with("file_"));
    assert_eq!(file["mime_type"], "text/csv");
    assert_eq!(file["size_bytes"], csv.len());
    assert_eq!(file["project"], Value::Null);
    let (status, body) = me
        .json(&world, "POST", &attachments, json!({"file": file_id}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["type"], "file");
    assert_eq!(body["filename"], "september.csv");

    // The bytes come back as they went.
    let (status, headers, bytes) = me
        .call(
            &world,
            "GET",
            &format!("/v1/files/{file_id}/content"),
            Body::empty(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(bytes, csv);
    assert_eq!(headers[header::CONTENT_TYPE], "text/csv");
    assert!(
        headers[header::CONTENT_DISPOSITION]
            .to_str()
            .unwrap()
            .contains("september.csv")
    );

    // A document, and one that holds a key is refused.
    let (status, doc) = me
        .json(
            &world,
            "POST",
            &attachments,
            json!({"document": {"title": "Budget notes", "text": "Rent is due on the 1st."}}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{doc}");
    let doc_id = doc["id"].as_str().unwrap().to_owned();
    assert!(doc_id.starts_with("pdoc_"));
    let (status, body) = me
        .json(
            &world,
            "GET",
            &format!("/v1/projects/name:hello-world/documents/{doc_id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["text"], "Rent is due on the 1st.");
    let (status, body) = me
        .json(
            &world,
            "POST",
            &attachments,
            json!({"document": {"title": "keys", "text": "AKIAIOSFODNN7EXAMPLE wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"}}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");

    // Listed oldest first, and counted on the project.
    let (status, body) = me.json(&world, "GET", &attachments, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let kinds: Vec<&str> = body["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|a| a["type"].as_str().unwrap())
        .collect();
    assert_eq!(kinds.len(), 2, "{body}");
    assert!(kinds.contains(&"file") && kinds.contains(&"document"));
    let (_, project) = me
        .json(&world, "GET", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(project["counts"]["files"], 1, "{project}");
    assert_eq!(project["counts"]["documents"], 1, "{project}");

    // Someone else sees none of it.
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let them = Api::of(&other);
    for path in [
        attachments.clone(),
        format!("/v1/files/{file_id}"),
        format!("/v1/files/{file_id}/content"),
        format!("/v1/projects/{id}/documents/{doc_id}"),
    ] {
        let (status, _) = them.json(&world, "GET", &path, Value::Null).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
    let (status, _) = them
        .json(
            &world,
            "DELETE",
            &format!("/v1/files/{file_id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    // Only me: no standard file or document goes in; a vault file does,
    // and only one that is in the caller's own vault.
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "sealed"}),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (status, body) = me
        .json(
            &world,
            "PATCH",
            &format!("/v1/projects/{id}"),
            json!({"protection": "only_me"}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["protection"], "only_me");
    let (_, another) = me.upload(&world, "october.csv", csv).await;
    let (status, body) = me
        .json(&world, "POST", &attachments, json!({"file": another["id"]}))
        .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "only_me_project");
    let object = "ab".repeat(32);
    let (status, body) = me
        .json(&world, "POST", &attachments, json!({"vault": object}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let owner = account_owner_of(&world, "octo-local");
    let key = Store::owner_key(&owner, &format!("vault/objects/{object}")).unwrap();
    world
        .store
        .write_key(&key, vec![7; 4096], None)
        .await
        .unwrap();
    let (status, body) = me
        .json(&world, "POST", &attachments, json!({"vault": object}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    assert_eq!(body["type"], "vault");
    assert_eq!(body["size_bytes"], 4096);
    assert!(body.get("filename").is_none());
    let (status, body) = them
        .json(&world, "POST", &attachments, json!({"vault": object}))
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, body) = me.json(&world, "GET", &attachments, Value::Null).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 3, "{body}");
    assert_eq!(body["protection"], "only_me");

    // A vault file deleted from the vault leaves the listing.
    let (_, generation) = world.store.read_key(&key).await.unwrap().unwrap();
    world.store.delete_key(&key, &generation).await.unwrap();
    let (_, body) = me.json(&world, "GET", &attachments, Value::Null).await;
    assert_eq!(body["data"].as_array().unwrap().len(), 2, "{body}");

    // Removing an attachment removes the file for good.
    let (status, body) = me
        .json(
            &world,
            "DELETE",
            &format!("{attachments}/{file_id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["object"], "attachment.deleted");
    let (status, _) = me
        .json(&world, "GET", &format!("/v1/files/{file_id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = me
        .json(
            &world,
            "DELETE",
            &format!("{attachments}/{doc_id}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (_, body) = me.json(&world, "GET", &attachments, Value::Null).await;
    assert_eq!(body["data"], json!([]), "{body}");

    // Removing the project forgets what it held.
    let (status, _) = me
        .json(&world, "DELETE", &format!("/v1/projects/{id}"), Value::Null)
        .await;
    assert_eq!(status, StatusCode::OK);
}

mod uploads;
