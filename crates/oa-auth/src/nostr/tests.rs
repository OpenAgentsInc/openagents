use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nostr::domain::{RelaySigner, Tag};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tenancy::sessions::{SessionBook, Sessions};

use super::*;

const ORIGIN: &str = "https://openagents.com";
const SIGN_IN: &str = "https://openagents.com/v1/device/nostr";
const LINK: &str = "https://openagents.com/v1/device/keys";

fn signer(seed: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{seed:02x}").repeat(32)).unwrap()
}

/// What the client signs; each field can be made wrong on purpose.
struct Request<'a> {
    signer: &'a RelaySigner,
    created_at: u64,
    url: &'a str,
    method: &'a str,
    challenge: Option<&'a str>,
    payload: Option<&'a [u8]>,
}

impl Request<'_> {
    fn event(&self) -> nostr::domain::Event {
        let mut tags = vec![
            Tag::new(vec!["u".into(), self.url.into()]),
            Tag::new(vec!["method".into(), self.method.into()]),
        ];
        if let Some(challenge) = self.challenge {
            tags.push(Tag::new(vec!["challenge".into(), challenge.into()]));
        }
        if let Some(payload) = self.payload {
            tags.push(Tag::new(vec![
                "payload".into(),
                hex(&Sha256::digest(payload)),
            ]));
        }
        self.signer
            .sign(self.created_at, 27_235, tags, String::new())
    }

    fn encoded(&self) -> String {
        STANDARD.encode(serde_json::to_vec(&self.event()).unwrap())
    }

    fn header(&self) -> String {
        format!("Nostr {}", self.encoded())
    }
}

fn world() -> (tempfile::TempDir, NostrAuth) {
    let dir = tempfile::tempdir().unwrap();
    tenancy::Accounts::install(dir.path()).unwrap();
    Sessions::install(dir.path(), SessionBook::new(3600, 3600)).unwrap();
    let auth = NostrAuth::open(dir.path(), &[ORIGIN.to_string()]).unwrap();
    (dir, auth)
}

const BODY: &[u8] = br#"{"app":"OpenAgents","computer":"Pat's iPhone"}"#;

fn sign_in(auth: &NostrAuth, key: &RelaySigner) -> Answer {
    let now = now();
    let (challenge, _) = auth.issue(Purpose::SignIn, now);
    let header = Request {
        signer: key,
        created_at: now,
        url: SIGN_IN,
        method: "POST",
        challenge: Some(&challenge),
        payload: Some(BODY),
    }
    .header();
    auth.sign_in(None, "signup", Some(&header), BODY, now)
}

#[test]
fn a_valid_event_signs_in_and_creates_the_account_once() {
    let (dir, auth) = world();
    let key = signer(1);
    let first = sign_in(&auth, &key);
    assert_eq!(first.status, 200, "{}", first.body);
    assert_eq!(first.body["created"], true);
    assert_eq!(first.body["pubkey"], key.pubkey());
    assert_eq!(first.body["session"]["computer"], "Pat's iPhone");
    let token = first.body["token"].as_str().unwrap();
    assert!(token.starts_with("sess_"));
    let lifetime = first.body["session"]["expires_at"].as_u64().unwrap()
        - first.body["session"]["created_at"].as_u64().unwrap();
    assert_eq!(lifetime, tenancy::sessions::APP_SESSION_TTL);

    let second = sign_in(&auth, &key);
    assert_eq!(second.status, 200, "{}", second.body);
    assert_eq!(second.body["created"], false);
    assert_eq!(second.body["account"]["id"], first.body["account"]["id"]);
    let store = tenancy::Accounts::open(dir.path())
        .unwrap()
        .store()
        .unwrap();
    assert_eq!(store.accounts.len(), 1);
    let workspace = store.workspaces.values().next().unwrap();
    assert_eq!(workspace.tenant, "signup");
    let sessions = Sessions::open(dir.path()).unwrap().store().unwrap();
    assert!(sessions.book.session_of_token(token).is_some());
}

#[test]
fn wrong_url_method_time_challenge_payload_or_signature_is_refused() {
    let (dir, auth) = world();
    let key = signer(2);
    let now = now();
    let (challenge, _) = auth.issue(Purpose::SignIn, now);
    let (link_challenge, _) = auth.issue(Purpose::Link("acct_someone"), now);
    let good = || Request {
        signer: &key,
        created_at: now,
        url: SIGN_IN,
        method: "POST",
        challenge: Some(&challenge),
        payload: Some(BODY),
    };
    let check =
        |request: Request<'_>| auth.sign_in(None, "signup", Some(&request.header()), BODY, now);
    let cases = [
        Request {
            url: "https://example.com/v1/device/nostr",
            ..good()
        },
        Request {
            url: "https://openagents.com/v1/device/apple",
            ..good()
        },
        Request {
            method: "GET",
            ..good()
        },
        Request {
            created_at: now - 120,
            ..good()
        },
        Request {
            challenge: None,
            ..good()
        },
        Request {
            challenge: Some("v1.9999999999.abc.def"),
            ..good()
        },
        Request {
            challenge: Some(&link_challenge),
            ..good()
        },
        Request {
            payload: Some(b"{}"),
            ..good()
        },
        Request {
            payload: None,
            ..good()
        },
    ];
    for (n, request) in cases.into_iter().enumerate() {
        let answer = check(request);
        assert_eq!(answer.status, 401, "case {n}: {}", answer.body);
        assert_eq!(answer.body["error"]["code"], "sign_in_denied");
    }
    // A forged signature: the event signed by one key, claiming another.
    let mut event = good().event();
    event.pubkey = signer(3).pubkey().to_string();
    let forged = format!(
        "Nostr {}",
        STANDARD.encode(serde_json::to_vec(&event).unwrap())
    );
    assert_eq!(
        auth.sign_in(None, "signup", Some(&forged), BODY, now)
            .status,
        401
    );
    // A challenge from another deployment (another key) is refused.
    let other = tempfile::tempdir().unwrap();
    let (theirs, _) = NostrAuth::open(other.path(), &[])
        .unwrap()
        .issue(Purpose::SignIn, now);
    let answer = check(Request {
        challenge: Some(&theirs),
        ..good()
    });
    assert_eq!(answer.status, 401);
    // An expired challenge is refused.
    let (old, _) = auth.issue(Purpose::SignIn, now - CHALLENGE_TTL - 5);
    let answer = check(Request {
        challenge: Some(&old),
        ..good()
    });
    assert_eq!(answer.status, 401);
    // No header at all.
    assert_eq!(auth.sign_in(None, "signup", None, BODY, now).status, 401);
    // Nothing was made by any of these.
    let store = tenancy::Accounts::open(dir.path())
        .unwrap()
        .store()
        .unwrap();
    assert!(store.accounts.is_empty());
    // The same event at the right place goes through.
    assert_eq!(check(good()).status, 200);
}

#[test]
fn the_request_host_is_accepted_beside_the_configured_origins() {
    let (_dir, auth) = world();
    let key = signer(4);
    let now = now();
    let (challenge, _) = auth.issue(Purpose::SignIn, now);
    let header = Request {
        signer: &key,
        created_at: now,
        url: "http://127.0.0.1:8080/v1/sessions/nostr",
        method: "POST",
        challenge: Some(&challenge),
        payload: Some(b""),
    }
    .header();
    let refused = auth.sign_in(Some("127.0.0.1:9090"), "signup", Some(&header), b"", now);
    assert_eq!(refused.status, 401);
    let answer = auth.sign_in(Some("127.0.0.1:8080"), "signup", Some(&header), b"", now);
    assert_eq!(answer.status, 200, "{}", answer.body);
    assert_eq!(answer.body["session"]["app"], "OpenAgents");
}

#[test]
fn a_replayed_event_or_challenge_is_refused() {
    let (_dir, auth) = world();
    let key = signer(5);
    let now = now();
    let (challenge, _) = auth.issue(Purpose::SignIn, now);
    let request = Request {
        signer: &key,
        created_at: now,
        url: SIGN_IN,
        method: "POST",
        challenge: Some(&challenge),
        payload: Some(BODY),
    };
    let header = request.header();
    assert_eq!(
        auth.sign_in(None, "signup", Some(&header), BODY, now)
            .status,
        200
    );
    let replay = auth.sign_in(None, "signup", Some(&header), BODY, now);
    assert_eq!(replay.status, 409);
    assert_eq!(replay.body["error"]["code"], "proof_used");
    // A new event over the same challenge is refused too.
    let again = Request {
        created_at: now + 1,
        ..request
    }
    .header();
    assert_eq!(
        auth.sign_in(None, "signup", Some(&again), BODY, now).status,
        409
    );
    // The burn survives reopening the stores.
    let reopened = NostrAuth::open(&auth.dir, &[ORIGIN.to_string()]).unwrap();
    assert_eq!(
        reopened
            .sign_in(None, "signup", Some(&header), BODY, now)
            .status,
        409
    );
}

fn link_proof(auth: &NostrAuth, key: &RelaySigner, account: &str) -> Vec<u8> {
    let now = now();
    let (challenge, _) = auth.issue(Purpose::Link(account), now);
    let proof = Request {
        signer: key,
        created_at: now,
        url: LINK,
        method: "POST",
        challenge: Some(&challenge),
        payload: None,
    }
    .encoded();
    serde_json::to_vec(&json!({"proof": proof})).unwrap()
}

#[test]
fn keys_are_added_listed_and_removed_but_never_the_last() {
    let (dir, auth) = world();
    let phone = signer(6);
    let desk = signer(7);
    let signed = sign_in(&auth, &phone);
    let account = signed.body["account"]["id"].as_str().unwrap().to_string();

    // A proof made for another account's challenge doesn't add the key.
    let theirs = link_proof(&auth, &desk, "acct_someoneelse");
    let answer = auth.link(None, &account, &theirs, now());
    assert_eq!(answer.status, 400);
    assert_eq!(answer.body["error"]["code"], "key_proof_denied");
    assert_eq!(auth.link(None, &account, b"{}", now()).status, 400);

    let proof = link_proof(&auth, &desk, &account);
    let added = auth.link(None, &account, &proof, now());
    assert_eq!(added.status, 200, "{}", added.body);
    assert_eq!(added.body["key"]["pubkey"], desk.pubkey());
    assert!(
        added.body["key"]["npub"]
            .as_str()
            .unwrap()
            .starts_with("npub1")
    );
    assert_eq!(added.body["keys"].as_array().unwrap().len(), 2);
    assert_eq!(auth.link(None, &account, &proof, now()).status, 409);

    // The added key signs in to the same account.
    let desk_sign_in = sign_in(&auth, &desk);
    assert_eq!(desk_sign_in.body["account"]["id"], account.as_str());
    assert_eq!(desk_sign_in.body["created"], false);

    // A key that holds another account is refused.
    let stranger = signer(8);
    let other = sign_in(&auth, &stranger).body["account"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let taken = auth.link(
        None,
        &account,
        &link_proof(&auth, &stranger, &account),
        now(),
    );
    assert_eq!(taken.status, 409);
    assert_eq!(taken.body["error"]["code"], "identity_taken");

    let listed = auth.keys(&account);
    let keys: Vec<&str> = listed.body["keys"]
        .as_array()
        .unwrap()
        .iter()
        .map(|k| k["pubkey"].as_str().unwrap())
        .collect();
    assert_eq!(keys, vec![phone.pubkey(), desk.pubkey()]);

    // Removing: by npub, then the last one stays.
    let npub = listed.body["keys"][0]["npub"].as_str().unwrap();
    let removed = auth.unlink(&account, npub);
    assert_eq!(removed.status, 200, "{}", removed.body);
    assert_eq!(removed.body["removed"]["pubkey"], phone.pubkey());
    let last = auth.unlink(&account, desk.pubkey());
    assert_eq!(last.status, 409);
    assert_eq!(last.body["error"]["code"], "last_key");
    assert_eq!(auth.unlink(&account, stranger.pubkey()).status, 404);
    assert_eq!(auth.unlink(&account, "nonsense").status, 404);
    assert_eq!(auth.unlink(&other, stranger.pubkey()).status, 409);

    // The removed key no longer reaches the account: it makes a new one.
    let back = sign_in(&auth, &phone);
    assert_eq!(back.body["created"], true);
    assert_ne!(back.body["account"]["id"], account.as_str());
    let _ = dir;
}

#[test]
fn an_account_made_another_way_adds_its_first_key() {
    let (dir, auth) = world();
    let accounts = tenancy::Accounts::open(dir.path()).unwrap();
    let older = accounts
        .create_account("Pat", &["key:0000000000000000".to_string()])
        .unwrap();
    let key = signer(9);
    let added = auth.link(None, &older.id, &link_proof(&auth, &key, &older.id), now());
    assert_eq!(added.status, 200, "{}", added.body);
    let signed = sign_in(&auth, &key);
    assert_eq!(signed.body["account"]["id"], older.id.as_str());
    assert_eq!(signed.body["created"], false);
    let listed: Value = auth.keys(&older.id).body;
    assert_eq!(listed["keys"].as_array().unwrap().len(), 1);
}

#[test]
fn keys_parse_as_hex_or_npub() {
    let key = signer(10);
    let hex_key = key.pubkey().to_string();
    assert_eq!(parse_pubkey(&hex_key.to_uppercase()), Some(hex_key.clone()));
    let npub = key_view(&hex_key)["npub"].as_str().unwrap().to_string();
    assert_eq!(parse_pubkey(&npub), Some(hex_key));
    assert_eq!(parse_pubkey("npub1nope"), None);
}

/// The website's own sign-in paths (#12001) get a browser session, with no
/// app label, as GitHub sign-in does.
#[test]
fn a_website_sign_in_gets_a_browser_session() {
    let (dir, auth) = world();
    let key = signer(7);
    let now = now();
    for path in WEB_SIGN_IN_PATHS {
        let (challenge, _) = auth.issue(Purpose::SignIn, now);
        let url = format!("{ORIGIN}{path}");
        let body = br#"{"request":"0123"}"#;
        let header = Request {
            signer: &key,
            created_at: now,
            url: &url,
            method: "POST",
            challenge: Some(&challenge),
            payload: Some(body),
        }
        .header();
        let answer = auth.sign_in(None, "signup", Some(&header), body, now);
        assert_eq!(answer.status, 200, "{}", answer.body);
        assert!(
            answer.body["session"].get("app").is_none(),
            "{}",
            answer.body
        );
        let token = answer.body["token"].as_str().unwrap();
        let book = Sessions::open(dir.path()).unwrap().store().unwrap().book;
        assert!(book.session_of_token(token).unwrap().app.is_none());
    }
    // The app's own path still gets an app session.
    let answer = sign_in(&auth, &key);
    assert_eq!(answer.body["session"]["app"], "OpenAgents");
}
