//! The profile and owner tag against a local relay and a fake website.
//! Every key here is a test key; nothing reaches a real relay.
use super::*;
use axum::{
    Json, Router,
    extract::{Path as UrlPath, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use futures::{SinkExt, StreamExt};
use nostr::domain::RelaySigner;
use openagents::{CredentialRecord, CredentialStore, Deployment, StorageError};
use tokio_tungstenite::tungstenite::Message;

fn identity(byte: u8) -> Identity {
    Identity::from_entropy(&[byte; 16]).unwrap()
}

/// A relay on loopback that keeps what it takes. With `auth`, it asks for
/// NIP-42 first and refuses events until it gets it.
async fn relay(auth: bool) -> (String, Arc<Mutex<Vec<Event>>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("ws://{}", listener.local_addr().unwrap());
    let held = Arc::new(Mutex::new(Vec::<Event>::new()));
    let store = held.clone();
    tokio::spawn(async move {
        while let Ok((stream, _)) = listener.accept().await {
            let store = store.clone();
            tokio::spawn(async move {
                let mut ws = tokio_tungstenite::accept_async(stream).await.unwrap();
                let say = |v: Value| Message::Text(v.to_string());
                if auth {
                    ws.send(say(json!(["AUTH", "challenge-1"]))).await.unwrap();
                }
                let mut authed = false;
                while let Some(Ok(Message::Text(text))) = ws.next().await {
                    let frame: Value = serde_json::from_str(&text).unwrap();
                    match frame[0].as_str() {
                        Some("EVENT") => {
                            let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            if auth && !authed {
                                ws.send(say(json!([
                                    "OK",
                                    event.id,
                                    false,
                                    "auth-required: send AUTH first"
                                ])))
                                .await
                                .unwrap();
                                continue;
                            }
                            let ok = event.validate_crypto().is_ok();
                            if ok {
                                store.lock().unwrap().push(event.clone());
                            }
                            ws.send(say(json!(["OK", event.id, ok, ""]))).await.unwrap();
                        }
                        Some("AUTH") => {
                            let event: Event = serde_json::from_value(frame[1].clone()).unwrap();
                            authed = event.kind == 22_242
                                && event.validate_crypto().is_ok()
                                && event.tag_values("challenge").next() == Some("challenge-1");
                            ws.send(say(json!(["OK", event.id, authed, ""])))
                                .await
                                .unwrap();
                        }
                        Some("REQ") => {
                            let sub = frame[1].clone();
                            let authors: Vec<String> =
                                serde_json::from_value(frame[2]["authors"].clone()).unwrap();
                            let found: Vec<Event> = store
                                .lock()
                                .unwrap()
                                .iter()
                                .filter(|e| authors.contains(&e.pubkey))
                                .cloned()
                                .collect();
                            for event in found {
                                ws.send(say(json!(["EVENT", sub, event]))).await.unwrap();
                            }
                            ws.send(say(json!(["EOSE", sub]))).await.unwrap();
                        }
                        _ => {}
                    }
                }
            });
        }
    });
    (url, held)
}

struct Volatile;
impl CredentialStore for Volatile {
    fn load(&self, _: &Deployment) -> Result<Option<CredentialRecord>, StorageError> {
        Ok(None)
    }
    fn save(&self, _: &CredentialRecord) -> Result<(), StorageError> {
        Ok(())
    }
    fn clear(&self, _: &Deployment) -> Result<(), StorageError> {
        Ok(())
    }
}

#[derive(Default)]
struct Site {
    name: Mutex<Option<(String, String)>>,
    owner: Mutex<Option<String>>,
}

fn agent_key() -> RelaySigner {
    RelaySigner::from_secret_hex(&"3".repeat(64)).unwrap()
}

/// The website's name and owner routes, as the apps call them.
async fn site() -> (Arc<Controller>, Arc<Site>, tempfile::TempDir) {
    let state = Arc::new(Site::default());
    let router = Router::new()
        .route(
            "/v1/account",
            get(|| async {
                Json(json!({"account":{"id":"acct_profile_fixture","label":"Fixture"},"workspaces":[]}))
            }),
        )
        .route(
            "/v1/device/name",
            get(|State(s): State<Arc<Site>>| async move {
                let held = s.name.lock().unwrap().clone();
                Json(match held {
                    Some((name, pubkey)) => json!({"name": name, "pubkey": pubkey}),
                    None => json!({"name": null}),
                })
            })
            .put(|State(s): State<Arc<Site>>, Json(body): Json<Value>| async move {
                let name = body["name"].as_str().unwrap_or_default().to_owned();
                if name == "taken" {
                    return (
                        StatusCode::CONFLICT,
                        Json(json!({"error": {"code": "name_taken", "message": "x"}})),
                    )
                        .into_response();
                }
                let pubkey = body["pubkey"].as_str().unwrap_or_default().to_owned();
                *s.name.lock().unwrap() = Some((name.clone(), pubkey.clone()));
                Json(json!({"name": name, "pubkey": pubkey})).into_response()
            }),
        )
        .route(
            "/v1/published-agents/{id}/owner",
            get(|UrlPath(_id): UrlPath<String>| async move {
                let agent = agent_key();
                let conditions = nostr::agent_public::conditions_until(now() + 365 * 24 * 60 * 60);
                Json(json!({
                    "agent": agent.pubkey(),
                    "npub": "npub1agent",
                    "conditions": conditions,
                    "preimage": format!("nostr:agent-auth:{}:{conditions}", agent.pubkey()),
                    "owner": null,
                    "revoked": false,
                }))
            })
            .put(
                |State(s): State<Arc<Site>>, Json(body): Json<Value>| async move {
                    let auth: Vec<String> = serde_json::from_value(body["auth"].clone()).unwrap();
                    match nostr::agent_public::check_owner_tag(&agent_key(), &Tag::new(auth), now())
                    {
                        Ok(owner) => {
                            *s.owner.lock().unwrap() = Some(owner.clone());
                            Json(json!({
                                "agent": agent_key().pubkey(),
                                "npub": "npub1agent",
                                "owner": owner,
                                "published": 1,
                            }))
                            .into_response()
                        }
                        Err(_) => (
                            StatusCode::BAD_REQUEST,
                            Json(json!({"error": {"code": "invalid", "message": "x"}})),
                        )
                            .into_response(),
                    }
                },
            ),
        )
        .with_state(state.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let client = Client::new(Deployment::new(&format!("{origin}/v1"), &origin).unwrap()).unwrap();
    let root = tempfile::tempdir().unwrap();
    let controller = Controller::new(client, Arc::new(Volatile), root.path().join("account"));
    controller
        .import_credential(format!("oa_sk_{}", "1".repeat(64)))
        .await;
    (controller, state, root)
}

#[tokio::test]
async fn a_profile_is_published_over_the_earlier_one_and_read_back() {
    let (url, held) = relay(false).await;
    let me = identity(1);
    // Another app set a banner and a name earlier.
    let earlier = me.relay_signer().sign(
        100,
        0,
        vec![],
        json!({"name": "old", "banner": "https://example.com/b.png"}).to_string(),
    );
    publish(&url, &me, &earlier).await.unwrap();
    let edit = ProfileEdit {
        name: "alice".into(),
        picture: "https://example.com/a.png".into(),
        about: "Builds agents.".into(),
    };
    let event = profile_event(&me, &edit, Some("alice"), Some(&earlier), 50).unwrap();
    assert!(event.created_at > earlier.created_at);
    publish(&url, &me, &event).await.unwrap();
    let newest = newest_profile(&url, &me.pubkey_hex())
        .await
        .unwrap()
        .unwrap();
    assert_eq!(newest.id, event.id);
    let read = Metadata::read(&newest.content);
    assert_eq!(read.name.as_deref(), Some("alice"));
    assert_eq!(read.nip05.as_deref(), Some("alice@openagents.com"));
    assert_eq!(read.picture.as_deref(), Some("https://example.com/a.png"));
    assert_eq!(read.banner.as_deref(), Some("https://example.com/b.png"));
    assert_eq!(held.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn a_relay_that_asks_for_auth_gets_it_and_takes_the_profile() {
    let (url, held) = relay(true).await;
    let me = identity(2);
    let event = profile_event(&me, &ProfileEdit::default(), None, None, now()).unwrap();
    publish(&url, &me, &event).await.unwrap();
    assert_eq!(held.lock().unwrap()[0].id, event.id);
}

#[test]
fn a_bad_picture_or_long_bio_is_refused_before_signing() {
    let me = identity(3);
    let bad = ProfileEdit {
        picture: "http://example.com/a.png".into(),
        ..ProfileEdit::default()
    };
    assert!(profile_event(&me, &bad, None, None, 1).is_err());
    let long = ProfileEdit {
        about: "a".repeat(metadata::ABOUT_MAX + 1),
        ..ProfileEdit::default()
    };
    assert!(profile_event(&me, &long, None, None, 1).is_err());
}

#[tokio::test]
async fn the_form_claims_the_name_and_publishes_the_profile() {
    let (url, held) = relay(false).await;
    let (controller, site, _root) = site().await;
    let me = Arc::new(identity(4));
    let key: KeySource = Arc::new({
        let me = me.clone();
        move || Some(me.clone())
    });
    let form = ProfileForm::with_relay(controller, key, url);
    form.load().await.unwrap();
    let view = form.view();
    let field = |name: &str, value: &str, view: &View<Intent>| FieldChange {
        activation: Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: name.into(),
        },
        value: value.into(),
    };
    form.change_field(&field("name", "Alice", &view)).unwrap();
    let view = form.view();
    form.change_field(&field("picture", "https://example.com/a.png", &view))
        .unwrap();
    let view = form.view();
    form.activate(&Activation {
        instance: view.instance.clone(),
        revision: view.revision,
        node: "save".into(),
    })
    .await
    .unwrap();
    assert_eq!(
        site.name.lock().unwrap().clone(),
        Some(("alice".into(), me.pubkey_hex()))
    );
    let shown = form.shown().unwrap();
    assert_eq!(shown.address.as_deref(), Some("alice@openagents.com"));
    assert_eq!(shown.page, "https://openagents.com/@alice");
    let published = held.lock().unwrap()[0].clone();
    assert_eq!(published.pubkey, me.pubkey_hex());
    assert_eq!(
        Metadata::read(&published.content).nip05.as_deref(),
        Some("alice@openagents.com")
    );
    // A taken name is refused and nothing more is published.
    let view = form.view();
    form.change_field(&field("name", "taken", &view)).unwrap();
    let view = form.view();
    let refused = form
        .activate(&Activation {
            instance: view.instance.clone(),
            revision: view.revision,
            node: "save".into(),
        })
        .await
        .unwrap_err();
    assert_eq!(refused, "Someone already has that name. Try another.");
    assert_eq!(held.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn the_owner_tag_checks_out_for_the_agent_key_and_is_renewed_when_due() {
    let (controller, site, root) = site().await;
    let me = identity(5);
    let lease = controller.lease().unwrap();
    let id = "a".repeat(64);
    let owned = own(lease.client(), lease.session(), &me, &id)
        .await
        .unwrap();
    assert_eq!(site.owner.lock().unwrap().clone(), Some(me.pubkey_hex()));
    assert!(owned.until > now() + 300 * 24 * 60 * 60);
    let dir = root.path().join("lab");
    remember(&dir, &owned);
    assert!(due(&dir, now()).is_empty());
    assert_eq!(due(&dir, owned.until - RENEW_BEFORE), vec![id.clone()]);
    remember(
        &dir,
        &Owned {
            until: now() + 60,
            ..owned
        },
    );
    *site.owner.lock().unwrap() = None;
    assert_eq!(
        renew_due(&dir, lease.client(), lease.session(), &me).await,
        1
    );
    assert_eq!(site.owner.lock().unwrap().clone(), Some(me.pubkey_hex()));
    assert!(due(&dir, now()).is_empty());
}

#[test]
fn the_form_copy_reads_plainly() {
    for text in [
        copy::TITLE,
        copy::NAME,
        copy::PICTURE,
        copy::ABOUT,
        copy::SAVE,
        copy::SAVED,
        copy::LOADING,
        copy::NOT_READY,
        UNREACHABLE,
        NOT_SAVED,
    ] {
        assert!(oa_copy::violations(text, &[]).is_empty(), "{text}");
    }
}
