use super::*;
use crate::projects::tests::{Browser, World, world};
use axum::{body::Body, http::Request};
use std::io::Cursor;

struct Client {
    browser: Browser,
    csrf: String,
}
impl Client {
    async fn signed(world: &World, name: &str) -> Self {
        let mut browser = Browser::default();
        browser.sign_in(world, name).await;
        let mut me = Self {
            browser,
            csrf: String::new(),
        };
        let (_, session) = me
            .call(world, "GET", "/v1/verse/studio/session", Value::Null)
            .await;
        assert_eq!(session["signed_in"], true, "{session}");
        me.csrf = session["csrf"].as_str().unwrap().into();
        me
    }
    async fn call(
        &mut self,
        world: &World,
        method: &str,
        path: &str,
        mut value: Value,
    ) -> (StatusCode, Value) {
        if path.starts_with("/v1/verse/studio/")
            && let Some(object) = value.as_object_mut()
        {
            object.insert("csrf".into(), self.csrf.clone().into());
        }
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header("content-type", "application/json")
            .header("origin", world.origin());
        // Existing account project APIs use the bearer path; studio exercises cookie + CSRF.
        if !path.starts_with("/v1/verse/studio/") {
            request = request.header(
                "authorization",
                format!("Bearer {}", self.browser.0["oa_cloud_session"]),
            );
        }
        let response = self
            .browser
            .send(
                world,
                request,
                if value.is_null() {
                    Body::empty()
                } else {
                    Body::from(value.to_string())
                },
            )
            .await;
        (
            response.status,
            serde_json::from_str(&response.body).unwrap_or_else(|_| json!({"text":response.body})),
        )
    }
}
fn document() -> ProjectDocument {
    serde_json::from_value(json!({"schema":"openagents.verse.project.v1","namespace":"haunted-house","template":{"world":"fixture-house","engine_release":"fixture-5","asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64)},"source_revision":0,"entities":{},"rooms":{},"removed_walls":[],"materials":{},"environment":{"fogDensity":0.01},"source_draft":"repairable unfinished draft","active_source":aethyrforge_host::DEFAULT_EFFECT,"layers":[],"originals":[]})).unwrap()
}
fn save(command: &str, version: u64, d: &ProjectDocument) -> Value {
    json!({"command_id":command,"expected_version":version,"document":d,"summary":"Changed the house"})
}

#[tokio::test]
async fn accepted_revisions_retry_conflict_and_compensate_through_real_project_membership() {
    let world = world().await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut bob = Client::signed(&world, "quiet-local").await;
    let (status, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Saved house"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{made}");
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let mut d = document();
    let (status, first) = alice
        .call(&world, "POST", &path, save("first", 0, &d))
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["saved"]["actor"], world.account_of("octo-local"));
    assert_eq!(
        bob.call(&world, "GET", &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, invite) = alice
        .call(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role":"editor"}),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED, "{invite}");
    let (status, joined) = bob
        .call(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code":invite["url"]}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{joined}");
    d.environment.insert("fogDensity".into(), 0.02);
    assert_eq!(
        alice
            .call(&world, "POST", &path, save("alice", 1, &d))
            .await
            .0,
        StatusCode::OK
    );
    let (status, stale) = bob.call(&world, "POST", &path, save("bob", 1, &d)).await;
    assert_eq!(status, StatusCode::CONFLICT, "{stale}");
    d.environment.insert("exposure".into(), 0.8);
    let (status, accepted) = bob.call(&world, "POST", &path, save("bob", 2, &d)).await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["saved"]["actor"], world.account_of("quiet-local"));
    assert_eq!(
        bob.call(&world, "POST", &path, save("bob", 2, &d)).await.1,
        accepted
    );
    let (status, undone) = alice
        .call(
            &world,
            "POST",
            &format!("{path}/undo"),
            json!({"command_id":"undo-alice","expected_version":3}),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{undone}");
    assert_eq!(
        undone["saved"]["document"]["environment"],
        json!({"fogDensity":0.01,"exposure":0.8})
    );
    let (_, reopened) = bob.call(&world, "GET", &path, Value::Null).await;
    assert_eq!(reopened["head"]["version"], 4);
    assert_eq!(
        reopened["head"]["document"]["source_draft"],
        "repairable unfinished draft"
    );
    assert_eq!(reopened["history"].as_array().unwrap().len(), 4);
    let member = world.account_of("quiet-local");
    let (status, removed) = alice
        .call(
            &world,
            "DELETE",
            &format!("/v1/projects/{id}/members/{member}"),
            Value::Null,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{removed}");
    assert_eq!(
        bob.call(&world, "GET", &path, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        bob.call(&world, "POST", &path, save("revoked", 4, &d))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn personal_checkpoints_are_separate_and_cookie_writes_require_csrf() {
    let world = world().await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut bob = Client::signed(&world, "quiet-local").await;
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Shared views"}),
        )
        .await;
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let (_, invited) = alice
        .call(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role":"viewer"}),
        )
        .await;
    assert_eq!(
        bob.call(
            &world,
            "POST",
            "/v1/projects/invitations/accept",
            json!({"code":invited["url"]})
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        bob.call(&world, "POST", &path, save("viewer", 0, &document()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let checkpoint = ProjectCheckpoint {
        project: id.into(),
        namespace: WorldNamespace::HauntedHouse,
        branch: "main".into(),
        saved_version: 0,
        visit: aethyrforge_host::StudioVisit {
            owner: Some("spoofed".into()),
            note: "My private note".into(),
            ..Default::default()
        },
    };
    let cp = format!("{path}/checkpoint");
    assert_eq!(
        bob.call(&world, "POST", &cp, json!({"checkpoint":checkpoint}))
            .await
            .0,
        StatusCode::OK
    );
    let (_, kept) = bob.call(&world, "GET", &cp, Value::Null).await;
    assert_eq!(
        kept["checkpoint"]["visit"]["owner"],
        world.account_of("quiet-local")
    );
    assert_eq!(
        alice.call(&world, "GET", &cp, Value::Null).await.1["checkpoint"],
        Value::Null
    );
    let correct = alice.csrf.clone();
    alice.csrf = "invalid".into();
    assert_eq!(
        alice
            .call(&world, "POST", &path, save("no-csrf", 0, &document()))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    alice.csrf = correct;
    assert_eq!(
        alice.call(&world, "GET", &path, Value::Null).await.1["head"],
        Value::Null
    );
}

#[tokio::test]
async fn material_images_are_private_bounded_verified_and_required_before_save() {
    let world = world().await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut other = Client::signed(&world, "quiet-local").await;
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Painted house"}),
        )
        .await;
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let mut pixels = Vec::new();
    let mut state = 23u32;
    for _ in 0..256 * 256 * 4 {
        state = state.wrapping_mul(1664525).wrapping_add(1013904223);
        pixels.push((state >> 24) as u8);
    }
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::from_raw(256, 256, pixels)
        .unwrap()
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let bytes = bytes.into_inner();
    assert!(bytes.len() > 65536);
    let sha = aethyrforge_host::project_hash(&bytes);
    let resource = format!("{path}/resources/{sha}");
    let mut doc = document();
    doc.layers.push(aethyrforge_host::MaterialLayer {
        material: "fur".into(),
        surface: "wolf".into(),
        texture: aethyrforge_host::ProjectResource {
            sha256: sha.clone(),
            bytes: bytes.len() as u64,
            media_type: "image/png".into(),
        },
        uv_set: 0,
        kind: "paint".into(),
    });
    assert_eq!(
        alice
            .call(&world, "POST", &path, save("missing-image", 0, &doc))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let response = alice
        .browser
        .send(
            &world,
            Request::builder()
                .method("POST")
                .uri(&resource)
                .header("origin", world.origin())
                .header("content-type", "image/png")
                .header("x-verse-csrf", &alice.csrf),
            Body::from(bytes.clone()),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    assert_eq!(
        alice
            .call(&world, "POST", &path, save("with-image", 0, &doc))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        other.call(&world, "GET", &resource, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    let response = alice
        .browser
        .send(
            &world,
            Request::builder()
                .method("POST")
                .uri(&resource)
                .header("origin", world.origin())
                .header("x-verse-csrf", &alice.csrf),
            Body::from("not a PNG"),
        )
        .await;
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    let response = alice
        .browser
        .send(
            &world,
            Request::builder()
                .method("POST")
                .uri(&resource)
                .header("origin", world.origin())
                .header("x-verse-csrf", "wrong"),
            Body::from(bytes),
        )
        .await;
    assert_eq!(response.status, StatusCode::FORBIDDEN);
}
