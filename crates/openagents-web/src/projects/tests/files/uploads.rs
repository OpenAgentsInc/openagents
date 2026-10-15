//! Real local account, quota, chunk retry, hash, sharing, and download checks.
use super::*;
use sha2::{Digest, Sha256};
const CHUNK: usize = 8 * 1024 * 1024;
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
async fn project(api: &Api, world: &World) -> String {
    let (status, body) = api
        .json(world, "POST", "/v1/projects", json!({"name":"Film files"}))
        .await;
    assert_eq!(status, StatusCode::CREATED, "{body}");
    body["id"].as_str().unwrap().into()
}
async fn start(
    api: &Api,
    world: &World,
    project: &str,
    bytes: &[u8],
    request: char,
) -> (StatusCode, Value) {
    api.json(world,"POST","/v1/files/uploads",json!({"project":project,"filename":"render.mp4","size_bytes":bytes.len(),"sha256":hash(bytes),"request_id":request.to_string().repeat(64)})).await
}
async fn part(
    api: &Api,
    world: &World,
    project: &str,
    id: &str,
    index: u64,
    bytes: &[u8],
) -> (StatusCode, Value) {
    let request = Request::put(format!(
        "/v1/files/uploads/{id}/parts/{index}?project={project}"
    ))
    .header(header::HOST, &world.host)
    .header(
        header::AUTHORIZATION,
        format!("Bearer {}", api.token.as_deref().unwrap()),
    )
    .header("x-part-sha256", hash(bytes))
    .body(Body::from(bytes.to_vec()))
    .unwrap();
    let response = world.site.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 32768).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}
#[tokio::test]
async fn upload_admins_resume_large_files_with_current_rights_and_checked_streaming() {
    let world = world_with(Options {
        invite: Some(json!({"github":[{"id":583231,"admin":true},{"id":9000001}]})),
        ..Default::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let admin = Api::of(&browser);
    let prj = project(&admin, &world).await;
    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let regular = Api::of(&other);
    let other_prj = project(&regular, &world).await;
    let mut bytes = vec![255u8; 11 * 1024 * 1024 + 13];
    bytes[0] = 0;
    let (status, _) = start(&regular, &world, &other_prj, &bytes, 'a').await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, created) = start(&admin, &world, &prj, &bytes, 'b').await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let id = created["id"].as_str().unwrap();
    let (status, _) = part(&regular, &world, &other_prj, id, 0, &bytes[..CHUNK]).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = part(&admin, &world, &prj, id, 1, &bytes[CHUNK..]).await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, first) = part(&admin, &world, &prj, id, 0, &bytes[..CHUNK]).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["uploaded_bytes"], CHUNK);
    let (status, retried) = part(&admin, &world, &prj, id, 0, &bytes[..CHUNK]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(retried["uploaded_bytes"], CHUNK);
    let (status, resumed) = start(&admin, &world, &prj, &bytes, 'b').await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(resumed["next_part"], 1);
    let disk = Store::local(world.root().join("chats"));
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let key =
        Store::owner_key(&owner, &format!("project-files/uploads/{id}/session.json")).unwrap();
    let persisted = disk.read_key(&key).await.unwrap().unwrap().0;
    assert_eq!(
        serde_json::from_slice::<Value>(&persisted).unwrap()["offset"],
        CHUNK
    );
    let (status, _) = admin
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = part(&admin, &world, &prj, id, 1, &bytes[CHUNK..]).await;
    assert_eq!(status, StatusCode::OK);
    let (status, file) = admin
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{file}");
    assert_eq!(file["size_bytes"], bytes.len());
    assert_eq!(file["sha256"], hash(&bytes));
    let file_id = file["id"].as_str().unwrap();
    let (status, again) = admin
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(again, file);
    let (status, _, download) = admin
        .call(
            &world,
            "GET",
            &format!("/v1/files/{file_id}/content?project={prj}"),
            Body::empty(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(download, bytes);
    let (status, _) = regular
        .json(
            &world,
            "GET",
            &format!("/v1/files/{file_id}?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, invite) = admin
        .json(
            &world,
            "POST",
            &format!("/v1/projects/{prj}/invitations"),
            json!({"role":"viewer"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = regular
        .json(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code":invite["url"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, download) = regular
        .call(
            &world,
            "GET",
            &format!("/v1/files/{file_id}/content?project={prj}"),
            Body::empty(),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(download.len(), bytes.len());
    assert_eq!(hash(&download), hash(&bytes));
    let (status, _) = part(&regular, &world, &prj, id, 1, &bytes[CHUNK..]).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
#[tokio::test]
async fn upload_quota_cancel_and_configured_unbounded_file_sizes() {
    let world = world_with(Options {
        uploads: Some(crate::ProjectUploadPolicy {
            max_file_bytes: None,
            account_bytes: Some(20),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let prj = project(&api, &world).await;
    let bytes = vec![255u8; 12];
    let (status, first) = start(&api, &world, &prj, &bytes, 'c').await;
    assert_eq!(status, StatusCode::CREATED, "{first}");
    let (status, retry) = start(&api, &world, &prj, &bytes, 'c').await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(first["id"], retry["id"]);
    let (status, _) = start(&api, &world, &prj, &bytes, 'd').await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    let (status, _) = api
        .json(
            &world,
            "DELETE",
            &format!(
                "/v1/files/uploads/{}?project={prj}",
                first["id"].as_str().unwrap()
            ),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = start(&api, &world, &prj, &bytes, 'd').await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = api.upload(&world, "small.bin", &bytes).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}
#[tokio::test]
async fn upload_checks_complete_hash_and_secret_shapes_across_parts() {
    let world = world_with(Options {
        uploads: Some(crate::ProjectUploadPolicy {
            max_file_bytes: None,
            ..Default::default()
        }),
        ..Default::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let prj = project(&api, &world).await;
    let bytes = vec![255u8; 3];
    let (_, created) = start(&api, &world, &prj, &bytes, 'e').await;
    let id = created["id"].as_str().unwrap();
    let (status, _) = part(&api, &world, &prj, id, 0, &[254; 3]).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = api
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "digest_mismatch");
    // Construct a non-working token shape, split exactly across the part boundary.
    let mut text = vec![b' '; CHUNK - 3];
    text.extend_from_slice(b"sk-");
    text.extend_from_slice(&[b'x'; 24]);
    let (_, created) = start(&api, &world, &prj, &text, 'f').await;
    let id = created["id"].as_str().unwrap();
    assert_eq!(
        part(&api, &world, &prj, id, 0, &text[..CHUNK]).await.0,
        StatusCode::OK
    );
    assert_eq!(
        part(&api, &world, &prj, id, 1, &text[CHUNK..]).await.0,
        StatusCode::OK
    );
    let (status, body) = api
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "looks_like_a_secret");
    let (status, listed) = api
        .json(
            &world,
            "GET",
            &format!("/v1/projects/{prj}/attachments"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert!(listed["data"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn upload_expiry_restarts_and_project_delete_releases_chunks_and_quota() {
    let world = world_with(Options {
        uploads: Some(crate::ProjectUploadPolicy {
            max_file_bytes: None,
            account_bytes: Some(12),
            ..Default::default()
        }),
        ..Default::default()
    })
    .await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let api = Api::of(&browser);
    let prj = project(&api, &world).await;
    let bytes = vec![255u8; 12];
    let (_, first) = start(&api, &world, &prj, &bytes, 'a').await;
    let id = first["id"].as_str().unwrap();
    let owner = crate::chat_store::account_owner(&world.account_of("octo-local"));
    let key =
        Store::owner_key(&owner, &format!("project-files/uploads/{id}/session.json")).unwrap();
    let (raw, generation) = world.store.read_key(&key).await.unwrap().unwrap();
    let mut raw: Value = serde_json::from_slice(&raw).unwrap();
    raw["expires"] = json!(1);
    world
        .store
        .write_key(&key, serde_json::to_vec(&raw).unwrap(), Some(&generation))
        .await
        .unwrap();
    let (status, restarted) = start(&api, &world, &prj, &bytes, 'a').await;
    assert_eq!(status, StatusCode::CREATED, "{restarted}");
    assert_eq!(restarted["uploaded_bytes"], 0);
    assert_eq!(
        part(&api, &world, &prj, id, 0, &bytes).await.0,
        StatusCode::OK
    );
    let (status, file) = api
        .json(
            &world,
            "POST",
            &format!("/v1/files/uploads/{id}/complete?project={prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{file}");
    let (status, _) = api
        .json(
            &world,
            "DELETE",
            &format!("/v1/projects/{prj}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let part = Store::owner_key(&owner, &format!("project-files/uploads/{id}/parts/0/0")).unwrap();
    assert!(world.store.read_key(&part).await.unwrap().is_none());
    let prj = project(&api, &world).await;
    assert_eq!(
        start(&api, &world, &prj, &bytes, 'b').await.0,
        StatusCode::CREATED
    );
}
