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
    has("ids", json!(event.id))
        && has("authors", json!(event.pubkey))
        && has("kinds", json!(event.kind))
        && tagged
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

    // The Grid home links the signed-in person to their named profile.
    let me = browser.get(&world, "/home/me").await;
    assert_eq!(me.status, StatusCode::OK);
    assert_eq!(me.headers[header::CACHE_CONTROL], "no-store");
    let me: Value = serde_json::from_str(&me.body).unwrap();
    assert_eq!(me["signed_in"], true);
    assert_eq!(me["name"], "pat-test");
    assert_eq!(me["profile"], "/@pat-test");
    // /new applies the chat feature flags and the person's saved UI choice.
    assert_eq!(me["chat"], "/chat");
    // The Grid's avatar carries the profile name and can ask for its
    // owner tag (#12406).
    assert_eq!(me["verse_name"], "pat-test");
    assert!(me["csrf"].as_str().is_some_and(|c| !c.is_empty()), "{me}");

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

    // Work the agent did and a review of it.
    let job = receipt(&key(31), &agent);
    let review = label(&key(32), &agent, "reliable");
    relay.put(job.clone());
    relay.put(review.clone());

    // Runtime references name exact signed releases. A saved runtime change
    // republishes the agent definition rather than matching by model name.
    let release: Event = serde_json::from_str(include_str!(
        "../../../../../att-release/tests/fixtures/layertree-signet/release.json"
    ))
    .unwrap();
    let runtime_path = format!("/v1/published-agents/{id}/runtime");
    let invalid = serde_json::to_vec(&json!({"release": "short"})).unwrap();
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "PUT",
        &runtime_path,
        None,
        Some(&bearer),
        Some(&invalid),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Give this addressable definition a later timestamp than the owner update.
    tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    let runtime = serde_json::to_vec(&json!({"release": release.id})).unwrap();
    let (status, linked) = call(
        &world,
        &mut Browser::default(),
        "PUT",
        &runtime_path,
        None,
        Some(&bearer),
        Some(&runtime),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{linked}");
    assert_eq!(linked["published"], 1);
    let linked_definition: Event = serde_json::from_value(linked["definition"].clone()).unwrap();
    assert_eq!(
        nostr::agent_public::read_definition(&linked_definition)
            .unwrap()
            .runtime
            .release
            .as_deref(),
        Some(release.id.as_str())
    );
    relay.put(release.clone());
    let rebuild: Event = serde_json::from_str(include_str!(
        "../../../../../att-release/tests/fixtures/layertree-signet/gcp-rebuild.json"
    ))
    .unwrap();
    relay.put(rebuild.clone());

    // Only a chosen, sealed receipt signed by this owner can be published.
    // Merely reading a page publishes nothing.
    let mut execution =
        receipts::ExecutionReceipt::for_attempt("http", "selected-request", 1, "sha256:request");
    execution.attempt_id = "selected-attempt".into();
    execution.served.model = "test-model".into();
    execution.outcome = receipts::Outcome::Answered;
    execution.seal();
    let selected = receipts_nostr::event(
        &pat,
        &linked_definition,
        &execution,
        crate::cloud::session::now(),
    )
    .unwrap();
    let receipts_path = format!("/v1/published-agents/{id}/receipts");
    let stranger_event = stranger.sign(
        selected.created_at,
        selected.kind,
        selected.tags.clone(),
        selected.content.clone(),
    );
    let bad = serde_json::to_vec(&stranger_event).unwrap();
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "POST",
        &receipts_path,
        None,
        Some(&bearer),
        Some(&bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let changed = pat.sign(
        selected.created_at,
        selected.kind,
        selected.tags.clone(),
        selected
            .content
            .replace("selected-attempt", "another-attempt"),
    );
    let bad = serde_json::to_vec(&changed).unwrap();
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "POST",
        &receipts_path,
        None,
        Some(&bearer),
        Some(&bad),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let chosen = serde_json::to_vec(&selected).unwrap();
    let (status, done) = call(
        &world,
        &mut Browser::default(),
        "POST",
        &receipts_path,
        None,
        Some(&bearer),
        Some(&chosen),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(done["published"], 1);
    assert_eq!(done["id"], selected.id);
    assert!(
        relay
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| serde_json::to_value(e).unwrap() == serde_json::to_value(&selected).unwrap())
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
    assert!(
        agent_page
            .body
            .contains(&format!("href=\"/event/{}\"", job.id))
    );
    assert!(
        agent_page
            .body
            .contains(&format!("href=\"/event/{}\"", review.id))
    );
    assert!(agent_page.body.contains("reliable"));
    assert!(
        agent_page
            .body
            .contains(&format!("href=\"/event/{}\"", release.id))
    );
    assert!(
        agent_page
            .body
            .contains(&format!("href=\"/event/{}\"", selected.id))
    );
    assert!(
        agent_page
            .body
            .contains("Chosen and published by the owner.")
    );
    assert!(
        agent_page
            .body
            .contains(&format!("href=\"/event/{}\"", rebuild.id))
    );
    let before = relay.events.lock().unwrap().len();
    let gpui = anyone.get(&world, &format!("/agent/{id}?ui=gpui")).await;
    assert_eq!(gpui.status, StatusCode::OK);
    assert!(
        gpui.body
            .contains(&format!("href=\"/event/{}\"", selected.id))
    );
    assert_eq!(relay.events.lock().unwrap().len(), before);
    let signed_page = anyone.get(&world, &format!("/event/{}", selected.id)).await;
    assert_eq!(signed_page.status, StatusCode::OK);
    assert!(signed_page.body.contains(&execution.digest));

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
    assert!(!agent_page.body.contains("agent-gateway-receipts"));
    assert!(!agent_page.body.contains("agent-attestations"));
    let (status, _) = call(
        &world,
        &mut Browser::default(),
        "POST",
        &receipts_path,
        None,
        Some(&bearer),
        Some(&chosen),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

/// An `eval-check` quest by `referee`, an evaluator's result, a checker's
/// check, and the awards for them, as `xp-ledger`'s own tests build them.
fn xp_world(referee: &RelaySigner, evaluator: &str, checker: &str) -> Vec<Event> {
    use xp_ledger::eval::fixture::{Run, published, release, signer};
    const AT: u64 = 1_790_000_000;
    let suite = release(&signer("suite-author"), "project-map-tests", AT - 5_000);
    let subject = release(&signer("ext-author"), "project-map", AT - 5_000);
    let spec = json!({
        "id": "ext-eval.project-map.check",
        "version": 1,
        "season": {"id": "ext-eval-s1", "opens_at": AT - 10_000, "closes_at": AT + 100_000},
        "title": "Check a Project map result",
        "objective": "Rerun the Project map test set on the same tool release and confirm a published result.",
        "acceptance": {
            "rule": "eval-check",
            "suite": {"id": suite.id, "pubkey": suite.pubkey, "kind": suite.kind},
            "subject": {"id": subject.id, "pubkey": subject.pubkey, "kind": subject.kind},
            "max_awards": 500,
        },
        "reference": null,
        "award": {"checker": 50, "evaluator": 25, "suite-author": 25},
    });
    let parts = nostr::xp::quest(&spec).unwrap();
    let quest = referee.sign(AT - 10_000, parts.kind, parts.tags, parts.content);
    let result = published(&signer(evaluator), &Run::better(&suite, &subject), None, AT);
    let check = published(
        &signer(checker),
        &Run::better(&suite, &subject),
        Some(&result.id),
        AT + 10,
    );
    let awards: Vec<Event> = nostr::xp::eval_check_awards(&quest, &result, &check, &[], AT + 100)
        .unwrap()
        .into_iter()
        .map(|p| referee.sign(AT + 100, p.kind, p.tags, p.content))
        .collect();
    let mut all = vec![suite, subject, quest, result, check];
    all.extend(awards);
    all
}

fn receipt(buyer: &RelaySigner, provider: &str) -> Event {
    use nostr::pylon::{Lane, Outcome, RECEIPT_V, Receipt, UnitKind, Units};
    let body = Receipt {
        v: RECEIPT_V.into(),
        requires: Vec::new(),
        meta: None,
        buyer: buyer.pubkey().into(),
        provider: provider.into(),
        pylon: "box-1".into(),
        lane: Lane::CjConversation,
        capability: "chat".into(),
        request: "1".repeat(64),
        request_digest: "2".repeat(64),
        result_digest: None,
        started_at: 1_790_000_000,
        finished_at: 1_790_000_010,
        units: Units {
            kind: UnitKind::Jobs,
            count: 1,
        },
        outcome: Outcome::Accepted,
        payment: None,
    };
    nostr::pylon::receipt_event(buyer, &body, 1_790_000_010).unwrap()
}

fn label(by: &RelaySigner, about: &str, value: &str) -> Event {
    by.sign(
        1_790_000_020,
        1985,
        vec![
            Tag::new(vec!["L".into(), "openagents.review".into()]),
            Tag::new(vec!["l".into(), value.into(), "openagents.review".into()]),
            Tag::new(vec!["p".into(), about.into()]),
        ],
        String::new(),
    )
}

#[tokio::test]
async fn a_profile_shows_xp_receipts_and_labels_each_linked_to_its_event() {
    use xp_ledger::eval::fixture::{pubkey, signer};
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    // A real referee credits Bob; a forger "credits" Erin.
    for event in xp_world(&signer("referee"), "alice", "bob") {
        relay.put(event);
    }
    for event in xp_world(&signer("forger"), "dave", "erin") {
        relay.put(event);
    }
    let bob = pubkey("bob");
    let job = receipt(&key(31), &bob);
    let review = label(&key(32), &bob, "helpful");
    relay.put(job.clone());
    relay.put(review.clone());

    let mut anyone = Browser::default();
    let page = anyone
        .get(&world, &format!("/@{}", crate::profiles::npub(&bob)))
        .await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    for shown in [
        "XP 50",
        "Check a Project map result",
        "Jobs done",
        "chat",
        "Reviews",
        "helpful",
    ] {
        assert!(page.body.contains(shown), "{shown}: {}", page.body);
    }
    assert!(page.body.contains(&format!("href=\"/event/{}\"", job.id)));
    assert!(
        page.body
            .contains(&format!("href=\"/event/{}\"", review.id))
    );
    let award = relay
        .events
        .lock()
        .unwrap()
        .iter()
        .find(|e| e.kind == nostr::xp::AWARD_KIND && e.pubkey == pubkey("referee"))
        .unwrap()
        .clone();
    assert!(
        page.body.contains(&format!("/event/{}", award.id)),
        "{}",
        page.body
    );
    crate::copy_guard::assert_plain("/@{npub}", &page.body);

    // The forged award counts for no one.
    let erin = anyone
        .get(
            &world,
            &format!("/@{}", crate::profiles::npub(&pubkey("erin"))),
        )
        .await;
    assert!(!erin.body.contains("XP "), "{}", erin.body);

    // Each link opens the signed event.
    let opened = anyone.get(&world, &format!("/event/{}", job.id)).await;
    assert_eq!(opened.status, StatusCode::OK, "{}", opened.body);
    assert!(opened.body.contains("Signed event"));
    assert!(opened.body.contains(&job.id));
    assert_eq!(
        anyone
            .get(&world, &format!("/event/{}", "0".repeat(64)))
            .await
            .status,
        StatusCode::NOT_FOUND
    );
}

#[test]
fn a_forged_award_counts_for_no_one() {
    use xp_ledger::eval::fixture::{pubkey, signer};
    let referees = vec![pubkey("referee")];
    let forged = xp_world(&signer("forger"), "alice", "bob");
    let (xp, awards) = crate::profiles::evidence::xp_of(&pubkey("bob"), &forged, &referees);
    assert_eq!((xp, awards.len()), (0, 0));
    let real = xp_world(&signer("referee"), "alice", "bob");
    let (xp, awards) = crate::profiles::evidence::xp_of(&pubkey("bob"), &real, &referees);
    assert_eq!((xp, awards.len()), (50, 1));
}

/// A published NIP-EXT listing of `<by>:<slug>` and the release it names.
fn plugin(by: &RelaySigner, slug: &str, title: &str) -> (Event, Event) {
    let release = xp_ledger::eval::fixture::release(by, slug, 1_790_000_000);
    let body = json!({
        "v": 1, "requires": [], "type": "listing",
        "package": format!("{}:{slug}", by.pubkey()),
        "state": "published",
        "release": {"id": release.id, "pubkey": release.pubkey, "kind": release.kind},
        "title": title,
        "description": "Does one thing.",
    });
    let listing = by.sign(
        1_790_000_001,
        nostr::ext::LISTING_KIND,
        vec![
            Tag::new(vec!["d".into(), slug.into()]),
            Tag::new(vec!["t".into(), "oa:ext:listing:v1".into()]),
        ],
        body.to_string(),
    );
    nostr::ext::parse_record(&listing).unwrap();
    (release, listing)
}

#[tokio::test]
async fn explore_lists_agents_and_plugins_from_relay_events() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let author = key(41);
    let (release, listing) = plugin(&author, "word-count", "Word count");
    relay.put(release.clone());
    relay.put(listing.clone());
    // Two agents by one owner; the one with signed evidence ranks first.
    let owner = key(42);
    let secret = |b: u8| secp256k1::SecretKey::from_byte_array([b; 32]).unwrap();
    let mut ids = Vec::new();
    for (n, (agent_seed, title)) in [(43u8, "Quiet agent"), (44u8, "Busy agent")]
        .into_iter()
        .enumerate()
    {
        let agent = key(agent_seed);
        let tag = nostr::domain::mint_owner_attestation(
            &secret(42),
            agent.pubkey(),
            &nostr::agent_public::conditions_until(4_000_000_000),
        )
        .unwrap()
        .tag();
        let id = format!("{:02x}", n + 1).repeat(32);
        let doc = json!({"experiment": {"title": title}}).to_string();
        relay.put(
            nostr::agent_public::definition_event(
                &agent,
                &tag,
                owner.pubkey(),
                &id,
                "https://example.com/d.json",
                &doc,
                1_790_000_000,
            )
            .unwrap(),
        );
        ids.push((id, agent));
    }
    relay.put(receipt(&key(45), ids[1].1.pubkey()));
    relay.put(label(&key(46), ids[1].1.pubkey(), "great"));

    let mut anyone = Browser::default();
    let page = anyone.get(&world, "/explore").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(!page.body.contains("<script src"));
    crate::copy_guard::assert_plain("/explore", &page.body);
    for shown in ["Word count", "Does one thing.", "Quiet agent", "Busy agent"] {
        assert!(page.body.contains(shown), "{shown}: {}", page.body);
    }
    assert!(
        page.body
            .contains(&format!("href=\"/event/{}\"", release.id))
    );
    assert!(page.body.contains(&format!("href=\"/agent/{}\"", ids[0].0)));
    let busy = page.body.find("Busy agent").unwrap();
    let quiet = page.body.find("Quiet agent").unwrap();
    assert!(busy < quiet, "signed evidence ranks first");

    // The same plugins the CLI's search reads.
    let events = relay.events.lock().unwrap().clone();
    let cli =
        plugin_registry::listings::without_samples(plugin_registry::listings::published(events));
    assert_eq!(cli.len(), 1);
    assert_eq!(cli[0].package, format!("{}:word-count", author.pubkey()));
}

/// Answers with the question back, so a test can read what it was asked.
struct Echo;

impl crate::published_agents::social::Answerer for Echo {
    fn answer(
        &self,
        _instructions: String,
        question: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Option<String>> + Send>> {
        Box::pin(async move { Some(format!("You said: {question}")) })
    }
}

/// A NIP-17 message from `from` (secret byte `seed`) to `to`.
fn private_message(seed: u8, to: &str, text: &str) -> Event {
    let from = secp256k1::SecretKey::from_byte_array([seed; 32]).unwrap();
    let author = key(seed).pubkey().to_string();
    let reader: secp256k1::XOnlyPublicKey = to.parse().unwrap();
    let rumor = nostr::nip17::chat_rumor(
        &author,
        1_790_000_000,
        text,
        vec![Tag::new(vec!["p".into(), to.into()])],
    )
    .unwrap();
    let sealed = nostr::nip17::seal(&rumor, &from, &reader, 1_790_000_000, [3; 32], None).unwrap();
    let wrapper = secp256k1::SecretKey::from_byte_array([99; 32]).unwrap();
    nostr::nip17::gift_wrap(&sealed, &wrapper, &reader, 1_790_000_000, [4; 32], None).unwrap()
}

#[tokio::test]
async fn people_follow_fork_and_talk_with_an_agent() {
    use openagents_sdk::agent_lab::{ForkAgent, Pin, PublishAgent, published_id, templates};
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let pat = key(21);
    let mut browser = Browser::default();
    let token = named(&world, &mut browser, &pat, "pat-test").await;
    let bearer = format!("Bearer {token}");
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
    let mut api = Browser::default();
    let (status, _) = call(
        &world,
        &mut api,
        "POST",
        "/v1/published-agents",
        None,
        Some(&bearer),
        Some(&body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let owner_path = format!("/v1/published-agents/{id}/owner");
    let (_, about) = call(&world, &mut api, "GET", &owner_path, None, None, None).await;
    let agent = about["agent"].as_str().unwrap().to_string();
    let tag = nostr::domain::mint_owner_attestation(
        &secp256k1::SecretKey::from_byte_array([21; 32]).unwrap(),
        &agent,
        about["conditions"].as_str().unwrap(),
    )
    .unwrap()
    .tag();
    let set = serde_json::to_vec(&json!({"auth": tag.as_slice()})).unwrap();
    let (status, _) = call(
        &world,
        &mut api,
        "PUT",
        &owner_path,
        None,
        Some(&bearer),
        Some(&set),
    )
    .await;
    assert_eq!(status, StatusCode::OK);

    // Follow the agent: the extension signs the kind 3 the site drafts.
    let agent_npub = crate::profiles::npub(&agent);
    let page = browser.get(&world, &format!("/@{agent_npub}/follow")).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    crate::copy_guard::assert_plain("/@{npub}/follow", &page.body);
    let ticket = json!({
        "csrf": attribute(&page.body, "data-csrf"),
        "request": attribute(&page.body, "data-request"),
        "fields": {"follow": agent_npub},
    });
    let ticket_bytes = ticket.to_string();
    let (status, draft) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/draft",
        Some(ORIGIN),
        None,
        Some(ticket_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{draft}");
    assert_eq!(draft["event"]["kind"], 3);
    let tags: Vec<Tag> = serde_json::from_value(draft["event"]["tags"].clone()).unwrap();
    let follows = pat.sign(
        draft["event"]["created_at"].as_u64().unwrap(),
        3,
        tags,
        String::new(),
    );
    let mut publish = ticket.clone();
    publish["event"] = serde_json::to_value(&follows).unwrap();
    let publish_bytes = publish.to_string();
    let (status, done) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/publish",
        Some(ORIGIN),
        None,
        Some(publish_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{done}");
    let mut anyone = Browser::default();
    let profile = anyone.get(&world, "/@pat-test").await;
    assert!(profile.body.contains("Follows 1"), "{}", profile.body);

    // Fork it from its page, and through the API with its upstream kept.
    let fork_page = browser.get(&world, &format!("/agent/{id}/fork")).await;
    assert_eq!(fork_page.status, StatusCode::OK, "{}", fork_page.body);
    let csrf = form_field(&fork_page.body, &format!("/agent/{id}/fork"), "csrf");
    let forked = browser
        .post(&world, &format!("/agent/{id}/fork"), &[("csrf", &csrf)])
        .await;
    assert_eq!(forked.status, StatusCode::OK, "{}", forked.body);
    assert!(forked.body.contains("Your copy is saved"));
    assert!(forked.body.contains(&format!("href=\"/agent/{id}\"")));
    let request = serde_json::to_vec(&ForkAgent {
        request_id: "c".repeat(64),
        name: "Mine".into(),
    })
    .unwrap();
    let (status, copy) = call(
        &world,
        &mut api,
        "POST",
        &format!("/v1/published-agents/{id}/fork"),
        None,
        Some(&bearer),
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    let def = copy["definition"]["id"].as_str().unwrap().to_string();
    let (status, upstream) = call(
        &world,
        &mut api,
        "GET",
        &format!("/v1/agent-definitions/{def}/upstream"),
        None,
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{upstream}");
    assert_eq!(upstream["forked_from"]["published"], id);

    // An agent only its public definition describes forks too.
    let elsewhere = key(51);
    let elsewhere_tag = nostr::domain::mint_owner_attestation(
        &secp256k1::SecretKey::from_byte_array([52; 32]).unwrap(),
        elsewhere.pubkey(),
        &nostr::agent_public::conditions_until(4_000_000_000),
    )
    .unwrap()
    .tag();
    let remote_id = "e".repeat(64);
    let document = agent_graph::definition::of(
        &template,
        "remote",
        &remote_id,
        &agent_graph::lab::Notes::default(),
    );
    relay.put(
        nostr::agent_public::definition_event(
            &elsewhere,
            &elsewhere_tag,
            key(52).pubkey(),
            &remote_id,
            "https://example.com/d.json",
            &serde_json::to_string(&document).unwrap(),
            1_790_000_000,
        )
        .unwrap(),
    );
    let request = serde_json::to_vec(&ForkAgent {
        request_id: "d".repeat(64),
        name: "Theirs".into(),
    })
    .unwrap();
    let (status, copy) = call(
        &world,
        &mut api,
        "POST",
        &format!("/v1/published-agents/{remote_id}/fork"),
        None,
        Some(&bearer),
        Some(&request),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{copy}");
    assert_eq!(copy["forked_from"]["published"], remote_id);

    // A room mention gets an answer signed by the agent with its owner tag.
    let mut config = crate::Config::development(world.root.path().join("tasks"));
    config.chat_store = std::sync::Arc::new(store.clone());
    config.agent_key_seed = Some([7; 32]);
    config.profile_relays = vec![relay.url.clone()];
    let app = crate::App(std::sync::Arc::new(crate::Inner { config }));
    let room = crate::published_agents::social::agent_room(&id);
    let asker = key(53);
    let mention = asker.sign(
        1_790_000_000,
        9,
        vec![
            Tag::new(vec!["h".into(), room.clone()]),
            Tag::new(vec!["p".into(), agent.clone()]),
        ],
        "What do you do?".into(),
    );
    let reply = crate::published_agents::social::respond(&app, &Echo, &mention, 1_790_000_100)
        .await
        .expect("an answer");
    assert_eq!(reply.pubkey, agent);
    assert_eq!(reply.kind, 9);
    assert_eq!(reply.content, "You said: What do you do?");
    assert_eq!(reply.tag_values("h").next(), Some(room.as_str()));
    assert_eq!(reply.tag_values("q").next(), Some(mention.id.as_str()));
    let attested = nostr::domain::verify_owner_attestation(&reply)
        .unwrap()
        .unwrap();
    assert_eq!(attested.owner_pubkey, pat.pubkey());
    crate::published_agents::social::send_reply(&app, &reply).await;
    assert!(
        relay
            .events
            .lock()
            .unwrap()
            .iter()
            .any(|e| e.id == reply.id)
    );
    // Not a mention, no answer.
    let chatter = asker.sign(
        1_790_000_000,
        9,
        vec![Tag::new(vec!["h".into(), room])],
        "hi".into(),
    );
    assert!(
        crate::published_agents::social::respond(&app, &Echo, &chatter, 1_790_000_100)
            .await
            .is_none()
    );

    // Private messages: the owner by default, then anyone once allowed.
    let stranger = private_message(53, &agent, "hello?");
    assert!(
        crate::published_agents::social::respond(&app, &Echo, &stranger, 1_790_000_100)
            .await
            .is_none()
    );
    let from_owner = private_message(21, &agent, "status?");
    let wrapped = crate::published_agents::social::respond(&app, &Echo, &from_owner, 1_790_000_100)
        .await
        .expect("a private answer");
    let opened = nostr::nip17::open_direct_message(
        &wrapped,
        &secp256k1::SecretKey::from_byte_array([21; 32]).unwrap(),
    )
    .unwrap();
    assert_eq!(opened.pubkey, agent);
    assert_eq!(opened.content, "You said: status?");
    let anyone_body = br#"{"from":"anyone"}"#;
    let (status, _) = call(
        &world,
        &mut api,
        "PUT",
        &format!("/v1/published-agents/{id}/messages"),
        None,
        Some(&bearer),
        Some(anyone_body),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        crate::published_agents::social::respond(&app, &Echo, &stranger, 1_790_000_100)
            .await
            .is_some()
    );
}

#[tokio::test]
async fn the_browser_asks_and_the_phone_approves_and_pays() {
    use coder_access::spend::{Grant, Purpose, RECEIPT, Receipt, Settlement};
    use nostr::x402::test_invoice::{described, signed_at};
    let world = key_world().await;
    let mut browser = Browser::default();
    extension_sign_in(&world, &mut browser, &key(61)).await;
    let bearer = format!("Bearer {}", browser.0["oa_cloud_session"]);
    let mut api = Browser::default();

    // Nothing to ask with until the phone grants it.
    let now = crate::cloud::session::now();
    let invoice = signed_at(
        "lnbc10u",
        described([5; 32], "Search API call", 600),
        false,
        false,
        now,
    );
    let ask = serde_json::to_vec(&json!({
        "payment": invoice,
        "purpose": Purpose::X402Purchase,
        "resource": "https://example.com/search",
    }))
    .unwrap();
    let (status, _) = call(
        &world,
        &mut api,
        "POST",
        "/v1/wallet/browser/requests",
        None,
        Some(&bearer),
        Some(&ask),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);

    // The phone grants this browser the default bounds.
    let (status, shown) = call(
        &world,
        &mut api,
        "GET",
        "/v1/wallet/browser",
        None,
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{shown}");
    let grantee = shown["grantee"].as_str().unwrap().to_string();
    let phone = "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9";
    let grant = Grant::request_mode("22".repeat(32), phone, &grantee, 1, now);
    let wrong = Grant::request_mode("23".repeat(32), phone, phone_other(), 1, now);
    for (g, want) in [(&wrong, StatusCode::BAD_REQUEST), (&grant, StatusCode::OK)] {
        let body = serde_json::to_vec(&json!({"grant": g})).unwrap();
        let (status, _) = call(
            &world,
            &mut api,
            "PUT",
            "/v1/wallet/browser/grant",
            None,
            Some(&bearer),
            Some(&body),
        )
        .await;
        assert_eq!(status, want);
    }

    // The browser asks; the request carries the invoice and nothing secret.
    let (status, asked) = call(
        &world,
        &mut api,
        "POST",
        "/v1/wallet/browser/requests",
        None,
        Some(&bearer),
        Some(&ask),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{asked}");
    let request = asked["request"].clone();
    let id = request["request"].as_str().unwrap().to_string();
    assert_eq!(request["amount_msat"], 1_000_000);
    let text = String::from_utf8(ask.clone()).unwrap();
    assert!(!text.contains("nsec") && !text.contains("secret"));
    let big = signed_at(
        "lnbc200u",
        described([6; 32], "Big", 600),
        false,
        false,
        now,
    );
    let too_much =
        serde_json::to_vec(&json!({"payment": big, "purpose": Purpose::X402Purchase})).unwrap();
    let (status, _) = call(
        &world,
        &mut api,
        "POST",
        "/v1/wallet/browser/requests",
        None,
        Some(&bearer),
        Some(&too_much),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);

    // The phone pays and proves it; a wrong preimage is refused.
    let receipt = |proof: [u8; 32]| Receipt {
        v: RECEIPT.into(),
        requires: vec![],
        request: id.clone(),
        grant: grant.grant.clone(),
        outcome: Settlement::Paid,
        code: None,
        payment_id: Some("pay-1".into()),
        amount_msat: Some(1_000_000),
        fees_msat: Some(10),
        proof: Some(proof.iter().map(|b| format!("{b:02x}")).collect()),
        remaining: None,
        at: now,
    };
    let path = format!("/v1/wallet/browser/requests/{id}/receipt");
    let forged = serde_json::to_vec(&json!({"receipt": receipt([9; 32])})).unwrap();
    let (status, _) = call(
        &world,
        &mut api,
        "POST",
        &path,
        None,
        Some(&bearer),
        Some(&forged),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let paid = serde_json::to_vec(&json!({"receipt": receipt([5; 32])})).unwrap();
    let (status, settled) = call(
        &world,
        &mut api,
        "POST",
        &path,
        None,
        Some(&bearer),
        Some(&paid),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{settled}");
    let (status, read) = call(
        &world,
        &mut api,
        "GET",
        &format!("/v1/wallet/browser/requests/{id}"),
        None,
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(read["receipt"]["outcome"], "paid");
    assert_eq!(read["receipt"]["proof"], "05".repeat(32));
}

fn phone_other() -> &'static str {
    "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5"
}

#[tokio::test]
async fn verse_labels_avatars_and_the_chat_never_loads_it() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let (pat, nobody) = (key(71), key(72));
    relay.put(pat.sign(
        100,
        0,
        Vec::new(),
        json!({"name": "pat", "display_name": "Pat Verse"}).to_string(),
    ));
    let mut anyone = Browser::default();
    let (status, names) = call(
        &world,
        &mut anyone,
        "GET",
        &format!(
            "/verse/names?keys={},{},not-a-key",
            pat.pubkey(),
            crate::profiles::npub(nobody.pubkey())
        ),
        None,
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(names["names"][pat.pubkey()]["name"], "Pat Verse");
    assert!(names["names"].get(nobody.pubkey()).is_none());
    let verse = anyone.get(&world, "/verse").await;
    assert_eq!(verse.status, StatusCode::OK);
    for page in ["/chat", "/home/classic", "/lander10", "/explore"] {
        let body = anyone.get(&world, page).await.body;
        assert!(!body.contains("everglade"), "{page} loads Verse");
    }
}

#[tokio::test]
async fn a_signed_in_persons_verse_avatar_carries_their_profile_name() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let pat = key(81);
    relay.put(pat.sign(
        100,
        0,
        Vec::new(),
        json!({"name": "pat", "display_name": "Pat In Verse"}).to_string(),
    ));
    let mut browser = Browser::default();
    named(&world, &mut browser, &pat, "pat-verse").await;
    let page = browser.get(&world, "/verse").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(
        page.body.contains("data-name=\"Pat In Verse\""),
        "{}",
        page.body
    );
    let mut anyone = Browser::default();
    let signed_out = anyone.get(&world, "/verse").await;
    assert!(!signed_out.body.contains("data-name="));
}

#[tokio::test]
async fn follows_keep_petnames_profiles_show_a_feed_and_notes_point_at_shares() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let (pat, alice, bob) = (key(91), key(92), key(93));
    relay.put(pat.sign(
        100,
        3,
        vec![Tag::new(vec![
            "p".into(),
            alice.pubkey().to_string(),
            "wss://relay.example.com".into(),
            "alice".into(),
        ])],
        String::new(),
    ));
    relay.put(alice.sign(200, 1, Vec::new(), "Hello from Alice".into()));
    let mut browser = Browser::default();
    named(&world, &mut browser, &pat, "pat-feed").await;

    // The profile shows what the people it follows posted.
    let mut anyone = Browser::default();
    let page = anyone.get(&world, "/@pat-feed").await;
    assert!(
        page.body.contains("From people they follow"),
        "{}",
        page.body
    );
    assert!(page.body.contains("Hello from Alice"));

    // A new follow keeps the old one's petname and relay hint.
    let bob_npub = crate::profiles::npub(bob.pubkey());
    let follow = browser.get(&world, &format!("/@{bob_npub}/follow")).await;
    let ticket = json!({
        "csrf": attribute(&follow.body, "data-csrf"),
        "request": attribute(&follow.body, "data-request"),
    });
    let mut draft_body = ticket.clone();
    draft_body["fields"] = json!({"follow": bob_npub});
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
    assert_eq!(
        draft["event"]["tags"],
        json!([
            ["p", alice.pubkey(), "wss://relay.example.com", "alice"],
            ["p", bob.pubkey()]
        ])
    );

    // A note may only point at a shared chat here; an unknown share has none.
    let mut announce = ticket.clone();
    announce["fields"] = json!({"announce": "f".repeat(32)});
    let announce_bytes = announce.to_string();
    let (status, _) = call(
        &world,
        &mut browser,
        "POST",
        "/settings/profile/draft",
        Some(ORIGIN),
        None,
        Some(announce_bytes.as_bytes()),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let now = crate::cloud::session::now();
    for (content, want) in [
        ("Look: https://api.openagents.com/share/abc", StatusCode::OK),
        ("Buy my coin https://scam.example", StatusCode::BAD_REQUEST),
    ] {
        let mut publish = ticket.clone();
        publish["event"] =
            serde_json::to_value(pat.sign(now, 1, Vec::new(), content.into())).unwrap();
        let bytes = publish.to_string();
        let (status, _) = call(
            &world,
            &mut browser,
            "POST",
            "/settings/profile/publish",
            Some(ORIGIN),
            None,
            Some(bytes.as_bytes()),
        )
        .await;
        assert_eq!(status, want, "{content}");
    }
}

#[tokio::test]
async fn a_browsers_verse_key_is_owned_by_the_persons_key_and_the_grid_lists_places() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let pat = key(0x61);
    let mut browser = Browser::default();
    let token = named(&world, &mut browser, &pat, "pat-grid").await;
    let bearer = format!("Bearer {token}");

    // The signed-in page carries the write value the module asks with.
    let page = browser.get(&world, "/verse").await;
    assert!(page.body.contains("data-csrf=\""), "{}", page.body);

    // The browser offers its key; the app signs; a stranger's tag is refused.
    let grid = key(0x62);
    let mut api = Browser::default();
    let offer = serde_json::to_vec(&json!({"pubkey": grid.pubkey()})).unwrap();
    let (status, offered) = call(
        &world,
        &mut api,
        "POST",
        "/v1/verse/key",
        None,
        Some(&bearer),
        Some(&offer),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{offered}");
    assert_eq!(offered["auth"], Value::Null);
    let conditions = offered["conditions"].as_str().unwrap().to_string();
    let mint = |byte: u8| {
        let secret = secp256k1::SecretKey::from_byte_array([byte; 32]).unwrap();
        nostr::domain::mint_owner_attestation(&secret, grid.pubkey(), &conditions)
            .unwrap()
            .tag()
            .as_slice()
            .to_vec()
    };
    let theirs = serde_json::to_vec(&json!({"auth": mint(0x63)})).unwrap();
    let (status, _) = call(
        &world,
        &mut api,
        "PUT",
        "/v1/verse/key",
        None,
        Some(&bearer),
        Some(&theirs),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mine = serde_json::to_vec(&json!({"auth": mint(0x61)})).unwrap();
    let (status, owned) = call(
        &world,
        &mut api,
        "PUT",
        "/v1/verse/key",
        None,
        Some(&bearer),
        Some(&mine),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{owned}");
    let tag = Tag::new(serde_json::from_value(owned["auth"].clone()).unwrap());
    assert_eq!(
        nostr::agent_public::check_owner_tag_for(grid.pubkey(), &tag, crate::cloud::session::now())
            .unwrap(),
        pat.pubkey()
    );
    let (_, read) = call(
        &world,
        &mut api,
        "GET",
        "/v1/verse/key",
        None,
        Some(&bearer),
        None,
    )
    .await;
    assert_eq!(read["auth"], owned["auth"]);

    // The Grid's places come from the same relay events /explore reads.
    let agent = key(0x64);
    let agent_tag = nostr::domain::mint_owner_attestation(
        &secp256k1::SecretKey::from_byte_array([0x61; 32]).unwrap(),
        agent.pubkey(),
        &nostr::agent_public::conditions_until(4_000_000_000),
    )
    .unwrap()
    .tag();
    relay.put(
        nostr::agent_public::definition_event(
            &agent,
            &agent_tag,
            pat.pubkey(),
            &"c".repeat(64),
            "https://example.com/d.json",
            &json!({"experiment": {"title": "Grid Helper"}}).to_string(),
            1_790_000_000,
        )
        .unwrap(),
    );
    let (status, places) = call(&world, &mut api, "GET", "/verse/places", None, None, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(places["agents"][0]["name"], "Grid Helper");
}

#[tokio::test]
async fn explore_lists_the_skills_the_directory_signed() {
    let relay = Relay::start().await;
    let world = key_world_with(vec![relay.url.clone()]).await;
    let mut config = crate::Config::development(world.root.path().join("tasks"));
    config.agent_key_seed = Some([7; 32]);
    let app = crate::App(std::sync::Arc::new(crate::Inner { config }));
    let directory = crate::profiles::skills::directory(&app).unwrap();
    let author = key(0x71);
    let skill = |name: &str| nostr::skill_listing::Skill {
        name: name.into(),
        version: "1.0.0".into(),
        digest: format!("sha256:{}", "a".repeat(64)),
        url: format!("https://openagents.com/v1/skills/{name}/versions/1.0.0/SKILL.md"),
        license: "MIT".into(),
        category: "coding".into(),
        tags: vec![],
    };
    relay.put(
        nostr::skill_listing::listing_event(
            &directory,
            &skill("repo-map"),
            Some(author.pubkey()),
            1_790_000_000,
        )
        .unwrap(),
    );
    // Another key's listing isn't the directory's, so it doesn't show.
    relay.put(
        nostr::skill_listing::listing_event(
            &key(0x72),
            &skill("forged-skill"),
            None,
            1_790_000_000,
        )
        .unwrap(),
    );
    let mut anyone = Browser::default();
    let page = anyone.get(&world, "/explore").await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    assert!(page.body.contains("repo-map"), "{}", page.body);
    assert!(page.body.contains(&format!(
        "href=\"/@{}\"",
        crate::profiles::npub(author.pubkey())
    )));
    assert!(!page.body.contains("forged-skill"));
}

/// With the GPUI chat on, the account's pages open in it (#12329,
/// #12330, #12331); the
/// HTML pages stay for every other browser and for `?ui=html`.
#[tokio::test]
async fn account_pages_open_in_the_gpui_app_when_it_is_on() {
    let world = key_world().await;
    let mut browser = Browser::default();
    extension_sign_in(&world, &mut browser, &key(0x61)).await;
    let html = browser.get(&world, "/settings/traces").await;
    assert_eq!(html.status, StatusCode::OK, "{}", html.body);
    browser.0.insert("oa_chat_ui".into(), "gpui".into());
    for (page, gpui) in [
        ("/settings/traces", "/gpui?page=traces"),
        ("/settings/traces/abc", "/gpui?page=traces/abc"),
        ("/settings/agents", "/gpui?page=agents"),
        ("/projects", "/gpui?page=projects"),
        ("/projects/proj_1", "/gpui?page=projects/proj_1"),
        ("/work", "/gpui?work=work"),
        ("/work/run_1", "/gpui?work=work:run_1"),
        ("/settings/mac-jobs", "/gpui?work=mac"),
        ("/settings/mac-jobs/job_1", "/gpui?work=mac:job_1"),
        ("/labs/image/doc_1", "/gpui?page=labs/image/doc_1/0/me"),
        (
            "/labs/video/doc_2?v=3&project=p9",
            "/gpui?page=labs/video/doc_2/3/project:p9",
        ),
    ] {
        let moved = browser.get(&world, page).await;
        assert!(moved.status.is_redirection(), "{page}: {}", moved.status);
        assert_eq!(moved.location(), gpui);
    }
    let kept = browser.get(&world, "/settings/traces?ui=html").await;
    assert_eq!(kept.status, StatusCode::OK);
}
