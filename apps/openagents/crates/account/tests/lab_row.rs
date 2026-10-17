//! Live-session Lab rows (#11948): a card from a Lab tool, read through the
//! signed-in account without a saved chat.
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use openagents_account::{
    Controller,
    lab_cards::{LiveCard, Media, NoteError, live_card_in_script},
    lab_row::LabRow,
    openagents::{CancellationToken, Client, CredentialRecord, CredentialStore, Deployment},
};
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
    time::Duration,
};

#[derive(Default)]
struct Store(Mutex<Option<CredentialRecord>>);
impl CredentialStore for Store {
    fn load(
        &self,
        _: &Deployment,
    ) -> Result<Option<CredentialRecord>, openagents_account::openagents::StorageError> {
        Ok(self.0.lock().unwrap().clone())
    }
    fn save(
        &self,
        r: &CredentialRecord,
    ) -> Result<(), openagents_account::openagents::StorageError> {
        *self.0.lock().unwrap() = Some(r.clone());
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), openagents_account::openagents::StorageError> {
        *self.0.lock().unwrap() = None;
        Ok(())
    }
}

const MODEL: &[u8] = b"glTF model bytes";

#[derive(Default)]
struct Server {
    notes: Mutex<Vec<Value>>,
    status: Mutex<Option<String>>,
    event: tokio::sync::Notify,
    spaces: Mutex<Vec<String>>,
}

fn card(n: u64) -> Value {
    let mut card: Value = serde_json::from_str(include_str!(
        "../../../../../crates/lab-kit/fixtures/cards/model3d.json"
    ))
    .unwrap();
    card["version"] = json!(n);
    card["summary"] = json!(format!("Take {n}"));
    card["page"] = json!(format!("/labs/cad/fan-bracket?v={n}"));
    card
}

fn version(n: u64) -> Value {
    let content = json!({"n": n});
    json!({"document":"fan-bracket","number":n,"parent":(n > 1).then(|| n - 1),
        "author":"acct_a","summary":format!("Take {n}"),"created_unix_ms":n * 1000,
        "schema":"cad.v1","content_sha256":lab_kit::digest(&content),"content":content})
}

async fn version_card(
    State(s): State<Arc<Server>>,
    Path((_, n)): Path<(String, u64)>,
    Query(q): Query<HashMap<String, String>>,
) -> Response {
    s.spaces.lock().unwrap().push(q["space"].clone());
    if n > 3 {
        return StatusCode::NOT_FOUND.into_response();
    }
    let mut card = card(n);
    card["notes"] = json!(
        s.notes
            .lock()
            .unwrap()
            .iter()
            .filter(|note| note["version"] == n)
            .map(|note| json!({"locator":note["locator"],"author":"acct_a","text":note["text"]}))
            .collect::<Vec<_>>()
    );
    card["state_text"] = json!(*s.status.lock().unwrap());
    Json(card).into_response()
}

async fn versions(Query(q): Query<HashMap<String, String>>) -> Response {
    // Two pages: 1..=2, then 3.
    let after: u64 = q.get("after").map_or(0, |a| a.parse().unwrap());
    let (data, next) = if after == 0 {
        (vec![version(1), version(2)], Some(2))
    } else {
        (vec![version(3)], None)
    };
    Json(json!({"data":data,"next":next})).into_response()
}

async fn note(State(s): State<Arc<Server>>, Json(body): Json<Value>) -> Response {
    s.notes.lock().unwrap().push(body);
    (StatusCode::CREATED, Json(json!({"saved":true}))).into_response()
}

async fn events(State(s): State<Arc<Server>>, h: HeaderMap) -> Response {
    let event =
        |cursor: u64| json!({"cursor":cursor,"document":"fan-bracket","at_unix_ms":1,"change":{}});
    if h.get("accept").is_none_or(|a| a != "text/event-stream") {
        return Json(json!({"data":[event(1)],"next":null})).into_response();
    }
    s.event.notified().await;
    let body = format!(
        "id: 2\nevent: changes\ndata: {}\n\n",
        json!({"data":[event(2)]})
    );
    ([("content-type", "text/event-stream")], body).into_response()
}

async fn artifact(Path(id): Path<String>, Query(q): Query<HashMap<String, String>>) -> Response {
    use sha2::{Digest, Sha256};
    assert_eq!(q["space"], "project:team");
    let (media, bytes): (&str, &[u8]) = if id == "art_glb_v4" {
        ("model/gltf-binary", MODEL)
    } else {
        ("image/png", b"poster")
    };
    Json(json!({"artifact":{"id":id,"version":{"document":"fan-bracket","version":3},"role":"render",
        "media_type":media,"sha256":format!("{:x}", Sha256::digest(bytes)),"size":bytes.len(),
        "class":"public","no_training":false},
        "url":format!("/v1/labs/artifacts/{id}/content?space=project:team"),"expires_seconds":null}))
    .into_response()
}

async fn content(Path(id): Path<String>) -> Response {
    if id == "art_glb_v4" {
        MODEL.to_vec().into_response()
    } else {
        b"poster".to_vec().into_response()
    }
}

async fn setup() -> (Arc<Controller>, Arc<Server>, tempfile::TempDir) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = Arc::new(Server::default());
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"acct_a","label":"Fixture"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/labs/documents/{id}/versions/{n}/card",
            get(version_card),
        )
        .route("/v1/labs/documents/{id}/versions", get(versions))
        .route("/v1/labs/documents/{id}/notes", post(note))
        .route("/v1/labs/documents/{id}/events", get(events))
        .route("/v1/labs/artifacts/{id}", get(artifact))
        .route("/v1/labs/artifacts/{id}/content", get(content))
        .with_state(server.clone());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let session = client
        .authenticate(&format!("oa_sk_{}", "1".repeat(64)))
        .await
        .unwrap();
    let root = tempfile::tempdir().unwrap();
    let store = Arc::new(Store::default());
    store.save(&session.record()).unwrap();
    let c = Controller::new(client, store, root.path().join("signed-out"));
    c.restore().await;
    (c, server, root)
}

/// The marker a live session's Lab visual starts with.
fn script(space: &str, n: u64) -> String {
    format!(
        "{}{}\nreturn function() return ui.text {{ \"Fan bracket\" }} end\n",
        openagents_account::lab_cards::LIVE_MARKER,
        json!({"space":space,"card":card(n)})
    )
}

#[test]
fn live_markers_carry_a_typed_card_and_its_space() {
    let live = live_card_in_script(&script("project:team", 3)).unwrap();
    assert_eq!(live.space, "project:team");
    assert_eq!(live.card.document, "fan-bracket");
    assert_eq!(live.card.version, 3);
    // Not a marker, the older Video Lab marker, or a card without its schema.
    assert!(live_card_in_script("return function() end").is_none());
    assert!(live_card_in_script("-- coder:video_lab {\"card\":\"video_lab\"}").is_none());
    let mut wrong = card(3);
    wrong["schema"] = json!("lab.card.v0");
    assert!(
        live_card_in_script(&format!(
            "{}{}",
            openagents_account::lab_cards::LIVE_MARKER,
            json!({"space":"me","card":wrong})
        ))
        .is_none()
    );
    assert!(live_card_in_script(&script("", 3)).is_none());
}

#[tokio::test]
async fn a_live_row_reads_its_files_lists_versions_and_shows_one() {
    let (c, server, _root) = setup().await;
    let live: LiveCard = live_card_in_script(&script("project:team", 3)).unwrap();
    let row = LabRow::open(c.lease().unwrap(), live).unwrap();
    assert_eq!(row.space(), "project:team");
    let page = row.page().unwrap();
    assert!(page.ends_with("/labs/cad/fan-bracket?v=3"), "{page}");
    // The model and poster come through the account; files the card doesn't
    // name read nothing.
    assert_eq!(
        row.media("art_glb_v4").await,
        Some(Media::Model(MODEL.to_vec()))
    );
    assert_eq!(
        row.media("art_hero_v4").await,
        Some(Media::Image(b"poster".to_vec()))
    );
    assert_eq!(row.media("art_other").await, None);

    // Every page of versions, newest first.
    let versions = row.versions().await.unwrap();
    let numbers: Vec<u64> = versions.iter().map(|v| v.number).collect();
    assert_eq!(numbers, vec![3, 2, 1]);
    assert_eq!(versions[1].label(), "v2 · Take 2");

    // Picking a version shows its card; one that can't be read keeps it.
    let mut updates = row.watch();
    assert!(row.show_version(2).await);
    assert!(updates.has_changed().unwrap());
    updates.mark_unchanged();
    assert_eq!(row.card().unwrap().version, 2);
    assert_eq!(row.card().unwrap().summary, "Take 2");
    assert!(!row.show_version(9).await);
    assert!(!row.show_version(0).await);
    assert_eq!(row.card().unwrap().version, 2);

    // A note goes on the shown version.
    assert_eq!(row.add_note(None, " ").await, Err(NoteError::Empty));
    row.add_note(Some(4.0), "Thicker").await.unwrap();
    let posted = server.notes.lock().unwrap()[0].clone();
    assert_eq!(posted["version"], 2);
    assert_eq!(posted["space"], "project:team");
    assert_eq!(row.card().unwrap().notes.last().unwrap().text, "Thicker");
    assert!(
        server
            .spaces
            .lock()
            .unwrap()
            .iter()
            .all(|s| s == "project:team")
    );

    // Signing out drops the card and reads nothing.
    c.sign_out().await;
    assert!(row.card().is_none());
    assert!(row.page().is_none());
    assert_eq!(row.media("art_hero_v4").await, None);
    assert!(row.versions().await.is_none());
}

#[tokio::test]
async fn a_live_row_follows_its_document_until_stopped_or_signed_out() {
    let (c, server, _root) = setup().await;
    let live = live_card_in_script(&script("project:team", 3)).unwrap();
    let row = LabRow::open(c.lease().unwrap(), live).unwrap();
    *server.status.lock().unwrap() = Some("Rendering".into());
    let stop = CancellationToken::new();
    let follow = tokio::spawn({
        let (row, stop) = (row.clone(), stop.clone());
        async move { row.follow(&stop).await }
    });
    let wait_for = |want: &'static str| {
        let row = row.clone();
        async move {
            tokio::time::timeout(Duration::from_secs(10), async {
                while row.card().and_then(|c| c.state_text).as_deref() != Some(want) {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                }
            })
            .await
            .expect(want);
        }
    };
    wait_for("Rendering").await;
    *server.status.lock().unwrap() = Some("Done".into());
    let notify = tokio::spawn({
        let server = server.clone();
        async move {
            loop {
                tokio::time::sleep(Duration::from_millis(50)).await;
                server.event.notify_waiters();
            }
        }
    });
    wait_for("Done").await;
    stop.cancel();
    tokio::time::timeout(Duration::from_secs(5), follow)
        .await
        .expect("stopping ends the follower")
        .unwrap();

    // A follower also ends when the account signs out.
    let follow = tokio::spawn({
        let row = row.clone();
        async move { row.follow(&CancellationToken::new()).await }
    });
    tokio::time::sleep(Duration::from_millis(100)).await;
    let old = c.lease().unwrap();
    c.sign_out().await;
    tokio::time::timeout(Duration::from_secs(5), follow)
        .await
        .expect("signing out ends the follower")
        .unwrap();
    notify.abort();
    // A lease from before the sign-out opens nothing.
    assert!(LabRow::open(old, live_card_in_script(&script("me", 3)).unwrap()).is_none());
}
