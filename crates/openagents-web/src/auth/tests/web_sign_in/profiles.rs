//! Public profiles (#12254) through the whole site: a key-only account
//! claims a name in Settings, its browser extension signs the profile,
//! the site publishes it to a relay, and `/@{name}` and NIP-05's
//! `/.well-known/nostr.json` show it. The relay is a small in-test one;
//! every key is a throwaway test key.

use std::sync::Mutex;

use futures_util::{SinkExt as _, StreamExt as _};
use nostr::domain::Event;
use serde_json::Value;
use tokio_tungstenite::tungstenite::Message;

use super::*;

/// A relay that keeps every event it's sent and answers `REQ` by
/// `authors` and `kinds`.
struct Relay {
    url: String,
    events: Arc<Mutex<Vec<Event>>>,
}

fn matches(filter: &Value, event: &Event) -> bool {
    let has = |field: &str, value: Value| {
        filter
            .get(field)
            .and_then(Value::as_array)
            .is_none_or(|list| list.contains(&value))
    };
    let tagged = filter
        .get("#p")
        .and_then(Value::as_array)
        .is_none_or(|list| event.tag_values("p").any(|p| list.contains(&json!(p))));
    has("authors", json!(event.pubkey)) && has("kinds", json!(event.kind)) && tagged
}

impl Relay {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}", listener.local_addr().unwrap());
        let events: Arc<Mutex<Vec<Event>>> = Arc::default();
        let held = events.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                let held = held.clone();
                tokio::spawn(async move {
                    let Ok(mut socket) = tokio_tungstenite::accept_async(stream).await else {
                        return;
                    };
                    while let Some(Ok(Message::Text(text))) = socket.next().await {
                        let frame: Value = serde_json::from_str(&text).unwrap();
                        let replies = match frame[0].as_str() {
                            Some("EVENT") => {
                                let event: Event =
                                    serde_json::from_value(frame[1].clone()).unwrap();
                                let ok = event.validate_crypto().is_ok();
                                let id = event.id.clone();
                                if ok {
                                    held.lock().unwrap().push(event);
                                }
                                vec![json!(["OK", id, ok, ""])]
                            }
                            Some("REQ") => {
                                let sub = frame[1].clone();
                                let mut out: Vec<Value> = held
                                    .lock()
                                    .unwrap()
                                    .iter()
                                    .filter(|e| matches(&frame[2], e))
                                    .map(|e| json!(["EVENT", sub, e]))
                                    .collect();
                                out.push(json!(["EOSE", sub]));
                                out
                            }
                            _ => Vec::new(),
                        };
                        for reply in replies {
                            if socket
                                .send(Message::Text(reply.to_string().into()))
                                .await
                                .is_err()
                            {
                                return;
                            }
                        }
                    }
                });
            }
        });
        Self { url, events }
    }

    fn put(&self, event: Event) {
        self.events.lock().unwrap().push(event);
    }
}

/// Sign `browser` in with `signer` through the extension route.
async fn extension_sign_in(world: &World, browser: &mut Browser, signer: &RelaySigner) {
    let (_, challenge) = call(
        world,
        browser,
        "GET",
        "/v1/device/nostr/challenge",
        None,
        None,
        None,
    )
    .await;
    let challenge = challenge["challenge"].as_str().unwrap().to_string();
    let url = format!("{ORIGIN}/login/key/extension");
    let body = json!({"return_to": "/settings"}).to_string();
    let signed = nip98(signer, &url, &challenge, body.as_bytes());
    let (status, done) = call(
        world,
        browser,
        "POST",
        "/login/key/extension",
        Some(ORIGIN),
        Some(&signed),
        Some(body.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
}

fn attribute(html: &str, name: &str) -> String {
    html.split_once(&format!("{name}=\""))
        .unwrap()
        .1
        .split('"')
        .next()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn a_key_names_itself_publishes_its_profile_and_shows_it_publicly() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let (pat, friend, stranger) = (key(21), key(22), key(23));
    // Who Pat follows, and the friend's own profile, already on the relay.
    relay.put(friend.sign(
        100,
        0,
        Vec::new(),
        json!({"name": "friend", "display_name": "A Friend"}).to_string(),
    ));
    relay.put(pat.sign(
        100,
        3,
        vec![Tag::new(vec!["p".into(), friend.pubkey().to_string()])],
        String::new(),
    ));

    let mut browser = Browser::default();
    extension_sign_in(&world, &mut browser, &pat).await;

    // Settings: pick a name.
    let settings = browser.get(&world, "/settings/profile").await;
    assert_eq!(settings.status, StatusCode::OK, "{}", settings.body);
    assert!(settings.body.contains("Pick a name"));
    crate::copy_guard::assert_plain("/settings/profile", &settings.body);
    let csrf = form_field(&settings.body, "/settings/profile/name", "csrf");
    let request = form_field(&settings.body, "/settings/profile/name", "request");
    let named = browser
        .post(
            &world,
            "/settings/profile/name",
            &[("csrf", &csrf), ("request", &request), ("name", "Pat-Test")],
        )
        .await;
    assert_eq!(named.status, StatusCode::SEE_OTHER, "{}", named.body);
    let reserved = browser
        .post(
            &world,
            "/settings/profile/name",
            &[("csrf", &csrf), ("request", &request), ("name", "admin")],
        )
        .await;
    assert_eq!(reserved.status, StatusCode::BAD_REQUEST);
    assert!(reserved.body.contains("reserved"));

    // NIP-05 answers the name with the key and our relay.
    let mut anyone = Browser::default();
    let (status, nip05) = call(
        &world,
        &mut anyone,
        "GET",
        "/.well-known/nostr.json?name=pat-test",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(nip05["names"]["pat-test"], pat.pubkey());
    let (_, nobody) = call(
        &world,
        &mut anyone,
        "GET",
        "/.well-known/nostr.json?name=nobody",
        None,
        None,
        None,
    )
    .await;
    assert_eq!(nobody, json!({"names": {}}));

    // The extension signs what the draft answers; the site publishes it.
    let settings = browser.get(&world, "/settings/profile").await;
    assert!(settings.body.contains("pat-test@openagents.com"));
    let ticket = json!({
        "csrf": attribute(&settings.body, "data-csrf"),
        "request": attribute(&settings.body, "data-request"),
    });
    let mut draft_body = ticket.clone();
    draft_body["fields"] = json!({
        "display_name": "Pat Test",
        "picture": "https://example.com/pat.png",
        "about": "I test things.",
        "website": "",
    });
    let draft_bytes = draft_body.to_string();
    let (status, draft) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/draft",
        Some(ORIGIN),
        None,
        Some(draft_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{draft}");
    assert_eq!(draft["pubkey"], pat.pubkey());
    let unsigned = &draft["event"];
    let content = unsigned["content"].as_str().unwrap();
    assert!(content.contains("pat-test@openagents.com"));
    let signed = pat.sign(
        unsigned["created_at"].as_u64().unwrap(),
        0,
        Vec::new(),
        content.to_string(),
    );

    // A key that isn't the account's is refused.
    let mut wrong = ticket.clone();
    wrong["event"] = serde_json::to_value(stranger.sign(
        unsigned["created_at"].as_u64().unwrap(),
        0,
        Vec::new(),
        content.to_string(),
    ))
    .unwrap();
    let wrong_bytes = wrong.to_string();
    let (status, _) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/publish",
        Some(ORIGIN),
        None,
        Some(wrong_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    let mut publish = ticket.clone();
    publish["event"] = serde_json::to_value(&signed).unwrap();
    let publish_bytes = publish.to_string();
    let (status, published) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/publish",
        Some(ORIGIN),
        None,
        Some(publish_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");
    assert_eq!(published["href"], "/@pat-test");
    assert!(
        relay
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.id == signed.id)
    );

    // Signed out, with no script: the page shows what the key signed.
    let page = anyone.get(&world, "/@pat-test").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    for shown in [
        "Pat Test",
        "@pat-test",
        "pat-test@openagents.com",
        "I test things.",
        "src=\"/@pat-test/picture\"",
        "A Friend",
        "Follows 1",
    ] {
        assert!(page.body.contains(shown), "{shown}: {}", page.body);
    }
    assert!(
        !page.body.contains("<script src"),
        "the page needs no script"
    );
    crate::copy_guard::assert_plain("/@pat-test", &page.body);

    // The key's own address moves to its name; an unnamed key has a page.
    let npub = crate::profiles::npub(&pat.pubkey());
    let moved = anyone.get(&world, &format!("/@{npub}")).await;
    assert_eq!(moved.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(moved.location(), "/@pat-test");
    let friend_page = anyone
        .get(
            &world,
            &format!("/@{}", crate::profiles::npub(&friend.pubkey())),
        )
        .await;
    assert_eq!(friend_page.status, StatusCode::OK);
    assert!(friend_page.body.contains("A Friend"));

    // An older GitHub profile address moves to the name.
    let store = accounts(&world);
    let account = tenancy::accounts::names::resolve(&store, "pat-test")
        .unwrap()
        .account;
    let github: tenancy::accounts::identities::GithubProfile =
        serde_json::from_value(json!({"id": 99001, "login": "pat-gh"})).unwrap();
    tenancy::Accounts::open(&world.root.path().join("accounts"))
        .unwrap()
        .link_github(&account, &github)
        .unwrap();
    let old = anyone.get(&world, "/u/pat-gh").await;
    assert_eq!(old.status, StatusCode::PERMANENT_REDIRECT, "{}", old.body);
    assert_eq!(old.location(), "/@pat-test");

    for missing in ["/@nobody-here", "/@a", "/@npub1nope"] {
        assert_eq!(
            anyone.get(&world, missing).await.status,
            StatusCode::NOT_FOUND,
            "{missing}"
        );
    }
}

#[tokio::test]
async fn a_github_login_still_has_its_page() {
    let world = key_world().await;
    let mut browser = Browser::default();
    let signed_in = browser
        .through_github(&world, "%2F", "login=octo-local")
        .await;
    assert!(signed_in.body.contains("Logged in"), "{}", signed_in.body);
    let mut anyone = Browser::default();
    let page = anyone.get(&world, "/u/octo-local").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("@octo-local"));
    assert_eq!(
        anyone.get(&world, "/u/nobody").await.status,
        StatusCode::NOT_FOUND
    );
}

/// Sign `signer` in and claim `name` in Settings; answers the session.
async fn named(world: &World, browser: &mut Browser, signer: &RelaySigner, name: &str) -> String {
    extension_sign_in(world, browser, signer).await;
    let settings = browser.get(world, "/settings/profile").await;
    let csrf = form_field(&settings.body, "/settings/profile/name", "csrf");
    let request = form_field(&settings.body, "/settings/profile/name", "request");
    let done = browser
        .post(
            world,
            "/settings/profile/name",
            &[("csrf", &csrf), ("request", &request), ("name", name)],
        )
        .await;
    assert_eq!(done.status, StatusCode::SEE_OTHER, "{}", done.body);
    browser.0["oa_cloud_session"].clone()
}

#[tokio::test]
async fn a_published_agent_gets_its_own_key_an_owner_and_a_page() {
    use openagents_sdk::agent_lab::{Pin, PublishAgent, published_id, templates};

    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let (pat, stranger) = (key(21), key(23));
    let mut browser = Browser::default();
    let token = named(&world, &mut browser, &pat, "pat-test").await;
    let bearer = format!("Bearer {token}");

    // Save an agent and publish it.
    let account = tenancy::accounts::names::resolve(&accounts(&world), "pat-test")
        .unwrap()
        .account;
    let owner = crate::chat_store::account_owner(&account);
    let store = crate::chat_store::Store::local(world.root.path().join("chats"));
    let template = templates::foreman();
    let saved = crate::custom_agents::mutate(
        &store,
        &owner,
        crate::custom_agents::Mutation::Create {
            request_id: "a".repeat(64),
            spec: template.draft.agent_spec().unwrap(),
        },
    )
    .await
    .unwrap()
    .unwrap();
    let request_id = "b".repeat(64);
    let id = published_id(&request_id);
    let body = serde_json::to_vec(&PublishAgent {
        request_id,
        definition: Pin {
            id: saved.id,
            revision: saved.revision,
        },
        template: template.clone(),
    })
    .unwrap();
    let (status, published) = call(
        &world,
        &mut Browser::default(),
        "POST",
        "/v1/published-agents",
        None,
        Some(&bearer),
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{published}");

    // What the owner signs.
    let mut anyone = Browser::default();
    let owner_path = format!("/v1/published-agents/{id}/owner");
    let (status, about) = call(&world, &mut anyone, "GET", &owner_path, None, None, None).await;
    assert_eq!(status, StatusCode::OK, "{about}");
    let agent = about["agent"].as_str().unwrap().to_string();
    assert_ne!(agent, pat.pubkey());
    assert_eq!(about["owner"], Value::Null);
    let conditions = about["conditions"].as_str().unwrap();
    let mint = |byte: u8| {
        let secret = secp256k1::SecretKey::from_byte_array([byte; 32]).unwrap();
        nostr::domain::mint_owner_attestation(&secret, &agent, conditions)
            .unwrap()
            .tag()
            .as_slice()
            .to_vec()
    };

    // A key that isn't on the account can't own it.
    let theirs = serde_json::to_vec(&json!({"auth": mint(23)})).unwrap();
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "PUT",
        &owner_path,
        None,
        Some(&bearer),
        Some(&theirs),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let _ = stranger;

    // Pat's own key owns it; the agent's events reach the relay.
    let mine = serde_json::to_vec(&json!({"auth": mint(21)})).unwrap();
    let (status, set) = call(
        &world,
        &mut Browser::default(),
        "PUT",
        &owner_path,
        None,
        Some(&bearer),
        Some(&mine),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{set}");
    assert_eq!(set["published"], 1);
    let events = relay.events.lock().unwrap().clone();
    let profile = events
        .iter()
        .find(|e| e.pubkey == agent && e.kind == 0)
        .expect("the agent's kind 0");
    assert_eq!(
        nostr::agent_public::profile_owner(profile).as_deref(),
        Some(pat.pubkey())
    );
    let content: Value = serde_json::from_str(&profile.content).unwrap();
    assert_eq!(content["bot"], true);
    let definition = events
        .iter()
        .find(|e| e.pubkey == agent && e.kind == nostr::agent_public::DEFINITION_KIND)
        .expect("the public definition");
    let read = nostr::agent_public::read_definition(definition).unwrap();
    assert_eq!(
        (read.owner.as_str(), read.id.as_str()),
        (pat.pubkey(), id.as_str())
    );

    // The owner's profile lists it; its page names the owner; its key moves to it.
    let page = anyone.get(&world, "/@pat-test").await;
    assert!(
        page.body.contains(&format!("href=\"/agent/{id}\"")),
        "{}",
        page.body
    );
    let agent_page = anyone.get(&world, &format!("/agent/{id}")).await;
    assert_eq!(agent_page.status, StatusCode::OK, "{}", agent_page.body);
    assert!(agent_page.body.contains("Owned by"));
    assert!(agent_page.body.contains("@pat-test"));
    crate::copy_guard::assert_plain("/agent/{id}", &agent_page.body);
    let by_key = anyone
        .get(&world, &format!("/agent/{}", crate::profiles::npub(&agent)))
        .await;
    assert_eq!(by_key.status, StatusCode::PERMANENT_REDIRECT);
    assert_eq!(by_key.location(), format!("/agent/{id}"));
    let export = anyone
        .get(&world, &format!("/agent/{id}/definition.json"))
        .await;
    assert_eq!(export.status, StatusCode::OK);
    assert!(
        export.body.contains("openagents.agent-definition-export"),
        "{}",
        export.body
    );

    // Retiring it takes it off the owner's list, and its page says so.
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "DELETE",
        &owner_path,
        None,
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let page = anyone.get(&world, "/@pat-test").await;
    assert!(!page.body.contains(&format!("href=\"/agent/{id}\"")));
    let agent_page = anyone.get(&world, &format!("/agent/{id}")).await;
    assert!(agent_page.body.contains("Its owner retired this agent."));
}
