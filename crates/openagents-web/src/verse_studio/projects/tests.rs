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

#[tokio::test]
async fn contributor_history_uses_account_members_and_current_signed_test_credits() {
    use crate::projects::tests::{Options, world_with};
    use nostr::{domain::RelaySigner, earnings};
    use std::collections::BTreeMap;
    use std::sync::{
        Arc, Mutex,
        atomic::{AtomicUsize, Ordering},
    };
    let fixture = tempfile::tempdir().unwrap();
    let links = fixture.path().join("operator-reward-links.json");
    std::fs::write(&links,r#"{"schema":"openagents.verse.reward-links.v1","rule":"operator-recorded-credit-link.v1","test_ledger":true,"links":[]}"#).unwrap();
    let events = Arc::new(Mutex::new(BTreeMap::<String, nostr::domain::Event>::new()));
    let calls = Arc::new(AtomicUsize::new(0));
    let fail = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let route = {
        let events = events.clone();
        let calls = calls.clone();
        let fail = fail.clone();
        get(move |Path((party, source)): Path<(String, String)>| {
            let events = events.clone();
            let calls = calls.clone();
            let fail = fail.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                if fail.load(Ordering::SeqCst) {
                    return answer(
                        StatusCode::SERVICE_UNAVAILABLE,
                        json!({"error":{"message":"Synthetic test ledger unavailable"}}),
                    );
                }
                let row = events.lock().unwrap().get(&source).cloned();
                match row {
                    Some(event) if earnings::read(&event).unwrap().party == party => answer(
                        StatusCode::OK,
                        json!({"events":[event.clone()],"issuers":[event.pubkey]}),
                    ),
                    _ => answer(StatusCode::NOT_FOUND, json!({})),
                }
            }
        })
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(
            listener,
            Router::new().route("/v1/earnings/public/{party}/sources/{source}", route),
        )
        .await
        .unwrap();
    });
    let world = world_with(Options {
        verse_reward_links: Some(links.clone()),
        verse_reward_service: Some(origin),
        ..Default::default()
    })
    .await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut bob = Client::signed(&world, "quiet-local").await;
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Contribution evidence"}),
        )
        .await;
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let (_, invited) = alice
        .call(
            &world,
            "POST",
            &format!("/v1/projects/{id}/invitations"),
            json!({"role":"editor"}),
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
    let mut d = document();
    d.source_draft = "failed WGSL draft".into();
    let mut revisions = Vec::new();
    for (index, who) in [0, 1, 1, 0].into_iter().enumerate() {
        d.environment
            .insert("fogDensity".into(), 0.01 + index as f64 * 0.001);
        if index > 0 {
            d.source_draft = d.active_source.clone();
        }
        let client = if who == 0 { &mut alice } else { &mut bob };
        let (status, v) = client
            .call(
                &world,
                "POST",
                &path,
                save(&format!("saved-{index}"), index as u64, &d),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{v}");
        revisions.push(serde_json::from_value::<SavedRevision>(v["saved"].clone()).unwrap());
    }
    let history_path = format!("{path}/history");
    let (_, before) = alice.call(&world, "GET", &history_path, Value::Null).await;
    assert_eq!(before["data"][2]["summary"], "World · Fog");
    assert_eq!(before["data"][2]["saved_summary"], "Changed the house");
    assert_ne!(before["data"][1]["summary"], before["data"][2]["summary"]);
    assert_eq!(before["data"][0]["source_state"], "draft");
    assert_eq!(before["data"][0]["application_state"], "not_recorded");
    assert_eq!(before["data"][0]["actor"]["name"], "You");
    assert_ne!(before["data"][1]["actor"]["name"], "You");
    assert!(
        before["data"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["earnings"]["state"] == "not_recorded")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let signer = RelaySigner::from_secret_hex(&"04".repeat(32)).unwrap();
    let mut mappings = Vec::new();
    for (index, state) in [
        earnings::State::Pending,
        earnings::State::Earned,
        earnings::State::Reversed,
    ]
    .into_iter()
    .enumerate()
    {
        let source = format!("{index:064x}");
        let party = if index == 0 { "11" } else { "22" }.repeat(32);
        let receipt = earnings::Receipt {
            v: earnings::SCHEMA.into(),
            party: party.clone(),
            source: source.clone(),
            category: earnings::Category::Pylon,
            state,
            amount_msat: 12001 + index as u64,
            network: "bitcoin".into(),
            recorded_at: now() - 10,
            evidence: None,
            plugin: None,
            release: None,
        };
        events
            .lock()
            .unwrap()
            .insert(source.clone(), receipt.event(&signer, now() - 1).unwrap());
        let revision = &revisions[index];
        mappings.push(json!({"contribution":aethyrforge_host::contribution_id(id,WorldNamespace::HauntedHouse,"main",revision),"actor":revision.actor,"party":party,"source":source}));
    }
    std::fs::write(&links,json!({"schema":"openagents.verse.reward-links.v1","rule":"operator-recorded-credit-link.v1","test_ledger":true,"links":mappings}).to_string()).unwrap();
    for (version, state) in [(1, "pending"), (2, "confirmed"), (3, "reversed")] {
        let url = format!("{history_path}/{version}/earnings");
        let (status, value) = alice.call(&world, "GET", &url, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        assert_eq!(value["earnings"]["credits"][0]["state"], state);
        assert_eq!(value["earnings"]["credits"][0]["unit"], "msat");
        assert_eq!(value["earnings"]["credits"][0]["test_ledger"], true);
        assert_eq!(
            alice.call(&world, "GET", &url, Value::Null).await.1,
            value,
            "repeated read cannot duplicate a credit"
        );
    }
    assert_eq!(
        alice
            .call(
                &world,
                "GET",
                &format!("{history_path}/4/earnings"),
                Value::Null
            )
            .await
            .1["earnings"]["state"],
        "not_recorded"
    );
    let source = format!("{:064x}", 1);
    let old = events.lock().unwrap()[&source].clone();
    let mut receipt = earnings::read(&old).unwrap();
    receipt.state = earnings::State::Paid;
    events
        .lock()
        .unwrap()
        .insert(source, receipt.event(&signer, now()).unwrap());
    assert_eq!(
        alice
            .call(
                &world,
                "GET",
                &format!("{history_path}/2/earnings"),
                Value::Null
            )
            .await
            .1["earnings"]["credits"][0]["state"],
        "paid"
    );
    let (_, inspect) = bob
        .call(&world, "GET", &format!("{history_path}/2"), Value::Null)
        .await;
    assert_eq!(inspect["version"], 2);
    assert!(!inspect["changes"].as_array().unwrap().is_empty());
    let mut guest = Client {
        browser: Browser::default(),
        csrf: String::new(),
    };
    let count = calls.load(Ordering::SeqCst);
    assert_eq!(
        guest
            .call(
                &world,
                "GET",
                &format!("{history_path}/2/earnings"),
                Value::Null
            )
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(calls.load(Ordering::SeqCst), count);
    assert_eq!(
        alice
            .call(
                &world,
                "POST",
                &format!("{history_path}/2/earnings"),
                json!({"amount":999999})
            )
            .await
            .0,
        StatusCode::METHOD_NOT_ALLOWED
    );
    fail.store(true, Ordering::SeqCst);
    assert_eq!(
        alice
            .call(
                &world,
                "GET",
                &format!("{history_path}/2/earnings"),
                Value::Null
            )
            .await
            .1["earnings"]["state"],
        "unavailable"
    );
    assert!(!before.to_string().contains('@'));
    server.abort();
}

fn large_material_restore() -> (
    ProjectDocument,
    Vec<aethyrforge_host::MaterialBundle>,
    Vec<u8>,
) {
    use aethyrforge_host::{
        MaterialBundle, MaterialDocument, MaterialImage, MaterialTarget, WorldEntity,
    };
    let mut pixels = vec![0; 256 * 256 * 4];
    let mut random = 23u32;
    for pixel in &mut pixels[..256 * 8 * 4] {
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        *pixel = (random >> 24) as u8;
    }
    let mut bytes = Cursor::new(Vec::new());
    image::RgbaImage::from_raw(256, 256, pixels)
        .unwrap()
        .write_to(&mut bytes, image::ImageFormat::Png)
        .unwrap();
    let bytes = bytes.into_inner();
    assert!(bytes.len() > 4 * 1024 && bytes.len() <= aethyrforge_host::MAX_MATERIAL_PNG);
    let image = MaterialImage::new(&bytes).unwrap();
    let mut document = document();
    let mut bundles = Vec::new();
    for i in 0..8 {
        let entity = format!("wolf-{i}");
        document.entities.insert(
            entity.clone(),
            WorldEntity {
                asset: "custom:haunt:werewolf".into(),
                position: [i as f64, 0., 0.],
                yaw: 0.,
                scale: 1.,
            },
        );
        let mut material = MaterialDocument::blank(MaterialTarget::werewolf_face(&entity));
        material.generated.image = Some(image.resource.clone());
        material.handmade.image = Some(image.resource.clone());
        material.keep_in(&mut document).unwrap();
        bundles.push(MaterialBundle {
            document: material,
            generated: Some(image.clone()),
            handmade: Some(image.clone()),
        });
    }
    (document, bundles, bytes)
}

#[tokio::test]
async fn large_material_restore_fits_grants_and_authorization_and_oversize_keeps_the_grant() {
    use aethyrforge_host::{Action, MAX_ACTION_BYTES, MAX_AUTHORIZATION_BYTES, MAX_BYTES};
    let world = world().await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let (document, materials, _) = large_material_restore();
    let archive = aethyrforge_host::MaterialProjectArchive {
        schema: "openagents.verse.project-export.v2".into(),
        document: document.clone(),
        materials: materials.clone(),
    };
    let retained = serde_json::to_string_pretty(&archive).unwrap();
    let reopened: aethyrforge_host::MaterialProjectArchive =
        serde_json::from_str(&retained).unwrap();
    assert_eq!(reopened, archive);
    assert!(
        matches!(reopened.restore(None).unwrap(), Action::RestoreMaterialProject { materials, .. } if materials.len() == 8)
    );
    let mut missing = reopened.clone();
    missing.materials[0].generated = None;
    assert!(missing.restore(None).is_err());
    let mut different = reopened;
    different.materials[0].document.generated.strength = 0.25;
    assert!(different.restore(None).is_err());
    let action = Action::RestoreMaterialProject {
        document,
        checkpoint: None,
        materials,
    };
    action.validate().unwrap();
    let envelope = aethyrforge_host::Envelope {
        protocol: aethyrforge_host::PROTOCOL.into(),
        version: aethyrforge_host::VERSION,
        session: "material-session".into(),
        sequence: 3,
        message: aethyrforge_host::Message::Request {
            request_id: "request-3".into(),
            project: aethyrforge_host::Project {
                id: "haunted-house".into(),
                branch: "main".into(),
                revision: 0,
            },
            action: action.clone(),
        },
    };
    let frame = envelope.encode().unwrap();
    assert_eq!(
        aethyrforge_host::Envelope::decode(&frame).unwrap(),
        envelope
    );
    let action_bytes = serde_json::to_vec(&action).unwrap().len();
    assert!(action_bytes > 65_536 && action_bytes <= MAX_ACTION_BYTES);
    let release = crate::pages::new_lander_house::engine_release().unwrap();
    let issue = json!({"tab":"material-budget-tab","session":"material-session","release":release,"project":{"id":"haunted-house","branch":"main","revision":0},"action":action,"confirmed":true});
    let (status, made) = alice
        .call(&world, "POST", "/v1/verse/studio/grants", issue.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{made}");
    let authorize = json!({"csrf":alice.csrf,"tab":"material-budget-tab","grant":made["grant"],"action":action});
    let encoded = authorize.to_string();
    assert!(encoded.len() > 65_536 && encoded.len() < MAX_BYTES);
    // Whitespace keeps the JSON valid while crossing only the HTTP body bound.
    for (path, mut body) in [("grants", issue.clone()), ("authorize", authorize.clone())] {
        body["csrf"] = alice.csrf.clone().into();
        let mut padded = body.to_string();
        padded.extend(std::iter::repeat_n(
            ' ',
            MAX_AUTHORIZATION_BYTES + 1 - padded.len(),
        ));
        let response = alice
            .browser
            .send(
                &world,
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/verse/studio/{path}"))
                    .header("content-type", "application/json")
                    .header("origin", world.origin()),
                Body::from(padded),
            )
            .await;
        assert_eq!(
            response.status,
            StatusCode::PAYLOAD_TOO_LARGE,
            "{}",
            response.body
        );
    }
    let (status, allowed) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/authorize",
            authorize.clone(),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{allowed}");
    assert_eq!(allowed["replay"], false);
    assert_eq!(
        alice
            .call(&world, "POST", "/v1/verse/studio/authorize", authorize)
            .await
            .1["replay"],
        true
    );
}

#[tokio::test]
async fn material_save_rejects_an_oversized_source_reference_without_accepting_a_revision() {
    let world = world().await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Material bounds"}),
        )
        .await;
    let path = format!(
        "/v1/verse/studio/projects/{}/haunted-house/main",
        made["id"].as_str().unwrap()
    );
    let (mut document, materials, bytes) = large_material_restore();
    let image = materials[0].generated.as_ref().unwrap();
    let response = alice
        .browser
        .send(
            &world,
            Request::builder()
                .method("POST")
                .uri(format!("{path}/resources/{}", image.resource.sha256))
                .header("origin", world.origin())
                .header("content-type", "image/png")
                .header("x-verse-csrf", &alice.csrf),
            Body::from(bytes),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let (status, accepted) = alice
        .call(
            &world,
            "POST",
            &path,
            save("material-in-bound", 0, &document),
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    let mut source = materials[0].document.clone();
    source.generated.image.as_mut().unwrap().bytes =
        (aethyrforge_host::MAX_MATERIAL_PNG + 1) as u64;
    let saved_source = document
        .originals
        .iter_mut()
        .find(|s| s.name == format!("material-v1-{}.json", source.key()))
        .unwrap();
    saved_source.text = serde_json::to_string(&source).unwrap();
    saved_source.sha256 = aethyrforge_host::project_hash(saved_source.text.as_bytes());
    document
        .layers
        .iter_mut()
        .find(|l| l.surface == format!("material-{}", source.key()) && l.kind == "generated")
        .unwrap()
        .texture = source.generated.image.clone().unwrap();
    let (status, refused) = alice
        .call(
            &world,
            "POST",
            &path,
            save("material-over-bound", 1, &document),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    let mut wrong_size = Cursor::new(Vec::new());
    image::RgbaImage::new(128, 128)
        .write_to(&mut wrong_size, image::ImageFormat::Png)
        .unwrap();
    let wrong_size = wrong_size.into_inner();
    let wrong_resource = aethyrforge_host::ProjectResource {
        sha256: aethyrforge_host::project_hash(&wrong_size),
        bytes: wrong_size.len() as u64,
        media_type: "image/png".into(),
    };
    let response = alice
        .browser
        .send(
            &world,
            Request::builder()
                .method("POST")
                .uri(format!("{path}/resources/{}", wrong_resource.sha256))
                .header("origin", world.origin())
                .header("content-type", "image/png")
                .header("x-verse-csrf", &alice.csrf),
            Body::from(wrong_size),
        )
        .await;
    assert_eq!(response.status, StatusCode::OK, "{}", response.body);
    let mut wrong_document: ProjectDocument =
        serde_json::from_value(accepted["saved"]["document"].clone()).unwrap();
    let mut wrong_material = materials[0].document.clone();
    wrong_material.generated.image = Some(wrong_resource);
    wrong_material.keep_in(&mut wrong_document).unwrap();
    assert_eq!(
        alice
            .call(
                &world,
                "POST",
                &path,
                save("wrong-face-dimensions", 1, &wrong_document)
            )
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    let (_, reopened) = alice.call(&world, "GET", &path, Value::Null).await;
    assert_eq!(reopened["head"], accepted["saved"]);
    assert_eq!(reopened["history"].as_array().unwrap().len(), 1);
}

fn publish_runtime() -> aethyrforge_host::PublishRuntime {
    serde_json::from_value(json!({"engine_url":"https://engine.example/","sdk_url":format!("https://engine.example/sdk/{}/aethyrforge_host.js","c".repeat(64)),"sdk_sha256":"c".repeat(64),"engine_release":"fixture-5","template_world":"fixture-house","engine_artifact_sha256":"d".repeat(64),"engine_abi":"aethyrforge.browser.v1","project_format":1,"protocol_version":7,"asset_set_sha256":"a".repeat(64),"scene_sha256":"b".repeat(64),"artifacts":[{"path":"/private-assets/scene.json","resource":{"sha256":"e".repeat(64),"bytes":100,"media_type":"application/json"}}],"material_ids":[],"rights":["fantasy-props-cc0","openagents-project-runtime","medieval-town-licensed-runtime"]})).unwrap()
}
#[tokio::test]
async fn publication_uses_saved_bytes_and_pinned_guest_links_with_retry_failure_revoke_and_rollback()
 {
    let world = crate::projects::tests::world_with(crate::projects::tests::Options {
        verse_publish_runtime: Some(publish_runtime()),
        ..Default::default()
    })
    .await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut bob = Client::signed(&world, "quiet-local").await;
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Playable house"}),
        )
        .await;
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let publications = format!("{path}/publications");
    let mut d = document();
    d.source_draft = d.active_source.clone();
    d.originals.push(aethyrforge_host::OriginalImport {
        name: "private-notes.txt".into(),
        text: "Private editable import".into(),
        sha256: aethyrforge_host::project_hash(b"Private editable import"),
    });
    let (_, saved) = alice
        .call(&world, "POST", &path, save("save-1", 0, &d))
        .await;
    let mut publish = json!({"command_id":"publish-1","expected_sequence":0,"operation":"publish","saved_version":1,"document_sha256":saved["saved"]["document_sha256"],"public":true,"confirm_runtime_distribution":true,"release":null});
    assert_eq!(
        bob.call(&world, "POST", &publications, publish.clone())
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    let (status, first) = alice
        .call(&world, "POST", &publications, publish.clone())
        .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(
        alice
            .call(&world, "POST", &publications, publish.clone())
            .await
            .1,
        first,
        "Lost-response retry does not create a second publication"
    );
    async fn guest(world: &World, path: &str) -> (StatusCode, Value) {
        let r = Browser::default()
            .send(world, Request::builder().uri(path), Body::empty())
            .await;
        (
            r.status,
            serde_json::from_str(&r.body).unwrap_or_else(|_| json!({"text":r.body})),
        )
    }
    let world_id = first["world"].as_str().unwrap();
    let first_id = first["event"]["release"].as_str().unwrap();
    let first_path = format!("/v1/verse/play/{world_id}/{first_id}");
    let (status, pinned) = guest(&world, &first_path).await;
    assert_eq!(status, StatusCode::OK, "{pinned}");
    assert!(
        pinned["manifest"]["content"]["document"]["originals"]
            .as_array()
            .unwrap()
            .is_empty(),
        "Private imported files are absent from player bytes"
    );
    assert_eq!(pinned["manifest"]["runtime"]["protocol_version"], 7);
    let (status, page) = guest(&world, &format!("/new-lander/play/{world_id}/{first_id}")).await;
    assert_eq!(status, StatusCode::OK);
    let page = page["text"].as_str().unwrap();
    assert!(page.contains("id=\"world-retry\""));
    assert!(
        page.contains("id=\"engine\" hidden"),
        "The base scene starts hidden before SDK initialization"
    );
    d.environment.insert("fogDensity".into(), 0.03);
    let (_, saved) = alice
        .call(&world, "POST", &path, save("save-2", 1, &d))
        .await;
    publish["command_id"] = "publish-2".into();
    publish["expected_sequence"] = 1.into();
    publish["saved_version"] = 2.into();
    publish["document_sha256"] = saved["saved"]["document_sha256"].clone();
    let (_, second) = alice
        .call(&world, "POST", &publications, publish.clone())
        .await;
    assert_eq!(second["book"]["sequence"], 2, "{second}");
    assert_eq!(
        guest(&world, &first_path).await.1,
        pinned,
        "Later saves/publications do not change an opened version"
    );
    // Saved source can be repairable. A matching pair of invalid active/draft
    // strings must never be mistaken for a compiled playable candidate.
    let mut invalid_doc = d.clone();
    invalid_doc.source_draft = "fn oa_material( this is not WGSL".into();
    invalid_doc.active_source = invalid_doc.source_draft.clone();
    let (status, bad_saved) = alice
        .call(&world, "POST", &path, save("bad-source", 2, &invalid_doc))
        .await;
    assert_eq!(status, StatusCode::OK, "{bad_saved}");
    let mut bad_publish = publish.clone();
    bad_publish["command_id"] = "bad-active".into();
    bad_publish["expected_sequence"] = 2.into();
    bad_publish["saved_version"] = 3.into();
    bad_publish["document_sha256"] = bad_saved["saved"]["document_sha256"].clone();
    assert_eq!(
        alice
            .call(&world, "POST", &publications, bad_publish.clone())
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    invalid_doc = d.clone();
    invalid_doc
        .environment
        .insert("unsupportedLighting".into(), 1.);
    let (_, bad_saved) = alice
        .call(&world, "POST", &path, save("bad-values", 3, &invalid_doc))
        .await;
    bad_publish["command_id"] = "bad-values".into();
    bad_publish["saved_version"] = 4.into();
    bad_publish["document_sha256"] = bad_saved["saved"]["document_sha256"].clone();
    assert_eq!(
        alice
            .call(&world, "POST", &publications, bad_publish)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        alice
            .call(&world, "GET", &publications, Value::Null)
            .await
            .1["book"],
        second["book"]
    );
    let failed_before = second["book"].clone();
    publish["command_id"] = "missing".into();
    publish["expected_sequence"] = 2.into();
    publish["document_sha256"] = "f".repeat(64).into();
    assert_eq!(
        alice
            .call(&world, "POST", &publications, publish.clone())
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        alice
            .call(&world, "GET", &publications, Value::Null)
            .await
            .1["book"],
        failed_before
    );
    publish["command_id"] = "rollback".into();
    publish["operation"] = "rollback".into();
    publish["release"] = first_id.into();
    let (_, rolled) = alice
        .call(&world, "POST", &publications, publish.clone())
        .await;
    assert_eq!(rolled["book"]["channel"], first_id);
    assert_eq!(rolled["book"]["sequence"], 3);
    publish["command_id"] = "revoke".into();
    publish["operation"] = "revoke".into();
    publish["expected_sequence"] = 3.into();
    let (_, revoked) = alice.call(&world, "POST", &publications, publish).await;
    assert_eq!(revoked["book"]["channel"], Value::Null);
    assert_eq!(guest(&world, &first_path).await.0, StatusCode::NOT_FOUND);
    let second_id = second["event"]["release"].as_str().unwrap();
    assert_eq!(
        guest(&world, &format!("/v1/verse/play/{world_id}/{second_id}"))
            .await
            .0,
        StatusCode::OK
    );
}

#[tokio::test]
async fn tutorial_award_requires_owner_choice_return_and_survives_membership_failure() {
    let world = crate::projects::tests::world_with(crate::projects::tests::Options {
        verse_publish_runtime: Some(publish_runtime()),
        ..Default::default()
    })
    .await;
    let mut alice = Client::signed(&world, "octo-local").await;
    let mut bob = Client::signed(&world, "quiet-local").await;
    const T: &str = "/v1/verse/studio/tutorial";
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    let (_, made) = alice
        .call(
            &world,
            "POST",
            "/v1/verse/studio/projects",
            json!({"name":"Tutorial house"}),
        )
        .await;
    let id = made["id"].as_str().unwrap();
    let path = format!("/v1/verse/studio/projects/{id}/haunted-house/main");
    let mut d = document();
    d.source_draft = d.active_source.clone();
    let (_, saved) = alice
        .call(&world, "POST", &path, save("tutorial-save", 0, &d))
        .await;
    let (status,published)=alice.call(&world,"POST",&format!("{path}/publications"),json!({"command_id":"tutorial-publish","expected_sequence":0,"operation":"publish","saved_version":1,"document_sha256":saved["saved"]["document_sha256"],"public":true,"confirm_runtime_distribution":true,"release":null})).await;
    assert_eq!(status, StatusCode::OK, "{published}");
    let mut proof = json!({"operation":"published","world":published["world"],"release":published["event"]["release"]});
    assert_eq!(
        bob.call(&world, "POST", T, proof.clone()).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        alice.call(&world, "POST", T, proof.clone()).await.0,
        StatusCode::OK
    );
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"return"}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    proof["operation"] = "share".into();
    assert_eq!(alice.call(&world, "POST", T, proof).await.0, StatusCode::OK);
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"return"}))
            .await
            .0,
        StatusCode::OK
    );
    let (status, revoked)=alice.call(&world,"POST",&format!("{path}/publications"),json!({"command_id":"tutorial-revoke","expected_sequence":1,"operation":"revoke","saved_version":1,"document_sha256":saved["saved"]["document_sha256"],"public":true,"confirm_runtime_distribution":true,"release":published["event"]["release"]})).await;
    assert_eq!(status, StatusCode::OK, "{revoked}");
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    d.environment.insert("fogDensity".into(), 0.015);
    let (_, saved_b) = alice
        .call(&world, "POST", &path, save("tutorial-save-b", 1, &d))
        .await;
    let (status,published_b)=alice.call(&world,"POST",&format!("{path}/publications"),json!({"command_id":"tutorial-publish-b","expected_sequence":2,"operation":"publish","saved_version":2,"document_sha256":saved_b["saved"]["document_sha256"],"public":true,"confirm_runtime_distribution":true,"release":null})).await;
    assert_eq!(status, StatusCode::OK, "{published_b}");
    let (status,choice_b)=alice.call(&world,"POST",T,json!({"operation":"play","world":published_b["world"],"release":published_b["event"]["release"]})).await;
    assert_eq!(status, StatusCode::OK, "{choice_b}");
    assert_eq!(
        choice_b["progress"]["returned_unix"],
        Value::Null,
        "A new explicit choice requires a new return"
    );
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"return"}))
            .await
            .0,
        StatusCode::OK
    );
    let mut second_tab = Client {
        browser: Browser(alice.browser.0.clone()),
        csrf: alice.csrf.clone(),
    };
    let (one, two) = tokio::join!(
        alice.call(&world, "POST", T, json!({"operation":"allocate"})),
        second_tab.call(&world, "POST", T, json!({"operation":"allocate"}))
    );
    let (status, first) = one;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(two.0, StatusCode::OK, "{}", two.1);
    assert_eq!(two.1, first, "Two tabs allocate exactly the same planet");
    let planet = first["progress"]["planet"].clone();
    assert!(planet["id"].as_str().unwrap().starts_with("planet_"));
    assert_ne!(planet["project"], made["id"]);
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .1,
        first
    );
    let store = world.root().join("accounts/projects/store.json");
    let retained = std::fs::read(&store).unwrap();
    // Fail the membership service after authentication, then restore its exact records.
    std::fs::write(&store, b"not a project store").unwrap();
    let (status, failed) = alice.call(&world, "GET", T, Value::Null).await;
    assert!(status.is_server_error(), "{status}: {failed}");
    assert_ne!(failed["error"]["code"], "planet_removed");
    assert!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0
            .is_server_error()
    );
    std::fs::write(&store, retained).unwrap();
    let (status, recovered) = alice.call(&world, "GET", T, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{recovered}");
    assert_eq!(recovered["planet_available"], true);
    assert_eq!(recovered["progress"]["planet"], planet);
    let (_, renamed) = alice
        .call(
            &world,
            "POST",
            T,
            json!({"operation":"name","name":"Moon garden"}),
        )
        .await;
    assert_eq!(renamed["progress"]["planet"]["name"], "Moon garden");
    let project = planet["project"].as_str().unwrap();
    assert_eq!(
        alice
            .call(
                &world,
                "DELETE",
                &format!("/v1/projects/{project}"),
                Value::Null
            )
            .await
            .0,
        StatusCode::OK
    );
    let (status, deleted) = alice.call(&world, "GET", T, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{deleted}");
    assert_eq!(deleted["planet_available"], false);
    assert_eq!(deleted["progress"]["planet"]["id"], planet["id"]);
    assert_eq!(
        alice
            .call(&world, "POST", T, json!({"operation":"allocate"}))
            .await
            .0,
        StatusCode::GONE
    );
}
