//! The vault's proof tests (#11240), with the real client core
//! (`oa-vault-web`, the code the browser runs) driving the real routes:
//!
//! - the server keeps ciphertext only: a canary file's bytes, name and
//!   digests appear in no file any server in the test world wrote;
//! - nothing the server or an operator holds opens the vault;
//! - deleting a file leaves kept ciphertext undecryptable;
//! - the recovery code opens the vault on a new device;
//! - another account can't read or change it.

use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use oa_vault::index::{Add, Kind};
use oa_vault_web::Session;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use super::api::TOKEN_HEADER;
use crate::projects::tests::{Answer, Browser, World, world};

const CANARY: &[u8] =
    b"CANARY-7f3a91 Bank statement. Closing balance 4242.17. Account 99-1234-5678.";
const CANARY_NAME: &str = "canary-statement-7f3a91.txt";

fn token(page: &str) -> String {
    let at = page
        .find("data-csrf=\"")
        .expect("the page carries its token")
        + "data-csrf=\"".len();
    page[at..at + page[at..].find('"').unwrap()].to_owned()
}

async fn json_to(
    browser: &mut Browser,
    world: &World,
    path: &str,
    csrf: &str,
    body: Value,
) -> Answer {
    browser
        .send(
            world,
            Request::post(path)
                .header(header::ORIGIN, world.origin())
                .header("sec-fetch-site", "same-origin")
                .header(TOKEN_HEADER, csrf)
                .header(header::CONTENT_TYPE, "application/json"),
            Body::from(body.to_string()),
        )
        .await
}

async fn put_object(
    browser: &mut Browser,
    world: &World,
    csrf: &str,
    id: &str,
    bytes: Vec<u8>,
) -> Answer {
    browser
        .send(
            world,
            Request::put(format!("/vault/api/objects/{id}"))
                .header(header::ORIGIN, world.origin())
                .header("sec-fetch-site", "same-origin")
                .header(TOKEN_HEADER, csrf)
                .header(header::CONTENT_TYPE, "application/octet-stream"),
            Body::from(bytes),
        )
        .await
}

async fn get_object(browser: &mut Browser, world: &World, id: &str) -> (StatusCode, Vec<u8>) {
    browser
        .send_bytes(
            world,
            Request::get(format!("/vault/api/objects/{id}")),
            Body::empty(),
        )
        .await
}

async fn state(browser: &mut Browser, world: &World) -> Value {
    let answer = browser.get(world, "/vault/api/state").await;
    assert_eq!(answer.status, StatusCode::OK, "{}", answer.body);
    serde_json::from_str::<Value>(&answer.body).unwrap()["vault"].clone()
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

fn unb64(text: &str) -> Vec<u8> {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD
        .decode(text)
        .unwrap()
}

/// A signed-in browser with a new vault: a passkey slot (PRF output
/// `[2; 32]`) and a recovery code. Returns the page token, the session and
/// the recovery words.
async fn with_vault(world: &World, login: &str) -> (Browser, String, Session, String) {
    let mut browser = Browser::default();
    browser.sign_in(world, login).await;
    let page = browser.get(world, super::PAGE).await;
    assert_eq!(page.status, StatusCode::OK, "{}", page.body);
    let csrf = token(&page.body);
    let session = Session::create().unwrap();
    let recovery: Value = serde_json::from_str(&session.recovery_slot(100).unwrap()).unwrap();
    let passkey: Value = serde_json::from_str(
        &session
            .passkey_slot(
                "Passkey · Mac",
                100,
                "127.0.0.1",
                b"credential",
                &[1; 32],
                &[2; 32],
            )
            .unwrap(),
    )
    .unwrap();
    let created = json_to(
        &mut browser,
        world,
        "/vault/api/create",
        &csrf,
        json!({ "vault": session.vault(), "slots": [passkey, recovery["slot"]], "index": b64(&session.index_blob().unwrap()) }),
    )
    .await;
    assert_eq!(created.status, StatusCode::CREATED, "{}", created.body);
    let words = recovery["words"].as_str().unwrap().to_owned();
    (browser, csrf, session, words)
}

fn add(name: &str) -> Add<'_> {
    Add {
        kind: Kind::File,
        name,
        media: Some("text/plain"),
        project: None,
        about: Vec::new(),
        route: None,
        created_at: 200,
    }
}

/// Upload `plain` as a new file and advance the index; returns its id.
async fn upload(
    browser: &mut Browser,
    world: &World,
    csrf: &str,
    session: &mut Session,
    name: &str,
    plain: &[u8],
) -> String {
    let (id, bytes) = session.add(add(name), plain).unwrap();
    let put = put_object(browser, world, csrf, &id, bytes).await;
    assert_eq!(put.status, StatusCode::CREATED, "{}", put.body);
    let index = json_to(
        browser,
        world,
        "/vault/api/index",
        csrf,
        json!({ "after": session.epoch(), "blob": b64(&session.pending_blob().unwrap()), "delete": [] }),
    )
    .await;
    assert_eq!(index.status, StatusCode::OK, "{}", index.body);
    session.commit();
    id
}

/// Every byte of every file under `dir`.
fn every_file(dir: &std::path::Path, out: &mut Vec<(std::path::PathBuf, Vec<u8>)>) {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            every_file(&path, out);
        } else if let Ok(bytes) = std::fs::read(&path) {
            out.push((path, bytes));
        }
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    !needle.is_empty()
        && haystack
            .windows(needle.len())
            .any(|window| window == needle)
}

#[tokio::test]
async fn the_server_stores_ciphertext_only_and_no_canary_anywhere() {
    let world = world().await;
    let (mut browser, csrf, mut session, words) = with_vault(&world, "octo-local").await;

    // The page: only this site's script, WebAssembly, the local model
    // server, and no HTML sinks.
    let page = browser.get(&world, super::PAGE).await;
    let policy = page.headers[header::CONTENT_SECURITY_POLICY]
        .to_str()
        .unwrap();
    assert!(
        policy.contains("script-src 'self' 'wasm-unsafe-eval'"),
        "{policy}"
    );
    assert!(
        policy.contains("require-trusted-types-for 'script'"),
        "{policy}"
    );
    assert!(
        policy.contains("connect-src 'self' http://127.0.0.1:8080"),
        "{policy}"
    );
    assert!(!page.body.contains("<script>"), "no inline script");
    assert!(page.body.contains("Vault (only you)"));
    crate::copy_guard::assert_plain(super::PAGE, &page.body);

    let id = upload(
        &mut browser,
        &world,
        &csrf,
        &mut session,
        CANARY_NAME,
        CANARY,
    )
    .await;

    // A new device opens it with the recovery code alone.
    let kept = state(&mut browser, &world).await;
    let recovery = kept["slots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|slot| slot["method"] == "recovery")
        .unwrap();
    let mut other =
        Session::unlock_recovery(session.vault(), &recovery.to_string(), &words).unwrap();
    other
        .load_index(&unb64(kept["index"]["blob"].as_str().unwrap()), 0)
        .unwrap();
    let (status, stored) = get_object(&mut browser, &world, &id).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other.open(&id, &stored).unwrap().as_slice(), CANARY);
    assert!(other.rows(None).unwrap().contains(CANARY_NAME));

    // No file any server wrote holds the canary, its name, or a digest or
    // encoding of it.
    let needles: Vec<(&str, Vec<u8>)> = vec![
        ("plaintext", CANARY.to_vec()),
        ("marker", b"CANARY-7f3a91".to_vec()),
        ("name", CANARY_NAME.as_bytes().to_vec()),
        (
            "sha256 hex",
            Sha256::digest(CANARY)
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
                .into_bytes(),
        ),
        ("sha256 raw", Sha256::digest(CANARY).to_vec()),
        ("base64", b64(CANARY).into_bytes()),
        (
            "words",
            words
                .split(' ')
                .take(6)
                .collect::<Vec<_>>()
                .join(" ")
                .into_bytes(),
        ),
    ];
    let mut files = Vec::new();
    every_file(world.root(), &mut files);
    assert!(
        files
            .iter()
            .any(|(path, _)| path.to_string_lossy().contains("/vault/objects/")),
        "the object is on disk"
    );
    for (path, bytes) in &files {
        for (what, needle) in &needles {
            assert!(
                !contains(bytes, needle),
                "{what} found in {}",
                path.display()
            );
        }
    }
}

#[tokio::test]
async fn nothing_the_server_or_an_operator_holds_opens_the_vault() {
    let world = world().await;
    let (mut browser, csrf, mut session, _) = with_vault(&world, "octo-local").await;
    let id = upload(
        &mut browser,
        &world,
        &csrf,
        &mut session,
        CANARY_NAME,
        CANARY,
    )
    .await;
    let kept = state(&mut browser, &world).await;

    // Everything stored, read the way an operator with full storage access
    // would: the slots, the index and the object.
    let mut files = Vec::new();
    every_file(world.root(), &mut files);
    let stored = files
        .iter()
        .filter(|(path, _)| path.to_string_lossy().contains("/vault/"))
        .collect::<Vec<_>>();
    assert!(stored.len() >= 3, "{stored:?}");
    let (header, _) =
        oa_vault::object::parse(&get_object(&mut browser, &world, &id).await.1).unwrap();
    assert!(header.wraps.is_empty(), "the stored object carries no key");
    assert!(header.core.media.is_none());

    // Every slot refuses every secret the server could hold or guess: its
    // own CSRF key, zeros, and each other value it stores.
    let mut guesses: Vec<Vec<u8>> = vec![vec![0; 32], vec![21; 32], vec![0xff; 32]];
    for (_, bytes) in &stored {
        guesses.push(Sha256::digest(bytes).to_vec());
    }
    for slot in kept["slots"].as_array().unwrap() {
        for guess in &guesses {
            assert!(Session::unlock(session.vault(), &slot.to_string(), guess).is_err());
        }
    }

    // The server's routes take no key material: the request bodies name
    // none, and the module never handles one.
    let api = include_str!("api.rs");
    let bodies = &api[api.find("pub(crate) struct CreateBody").unwrap()..];
    for word in ["vmk:", "secret:", "words:", "prf", "user_wrap", "dek:"] {
        assert!(!bodies.contains(word), "a request body names {word}");
    }
    let service = [api, include_str!("mod.rs"), include_str!("page.rs")].concat();
    for word in [
        "Vmk",
        "Session::",
        ".open(",
        "open_object",
        "recovery::Code",
        "user_wrap_key",
        "index_key",
    ] {
        assert!(!service.contains(word), "the service touches {word}");
    }
}

#[tokio::test]
async fn deleting_a_file_leaves_its_ciphertext_undecryptable() {
    let world = world().await;
    let (mut browser, csrf, mut session, words) = with_vault(&world, "octo-local").await;
    let id = upload(
        &mut browser,
        &world,
        &csrf,
        &mut session,
        CANARY_NAME,
        CANARY,
    )
    .await;
    let keep = upload(
        &mut browser,
        &world,
        &csrf,
        &mut session,
        "other.txt",
        b"still here",
    )
    .await;

    // An attacker kept a copy of the ciphertext and the index before the
    // delete.
    let (_, ciphertext) = get_object(&mut browser, &world, &id).await;
    let before = state(&mut browser, &world).await;
    let old_blob = unb64(before["index"]["blob"].as_str().unwrap());

    session.remove(&id).unwrap();
    let deleted = json_to(
        &mut browser,
        &world,
        "/vault/api/index",
        &csrf,
        json!({ "after": session.epoch(), "blob": b64(&session.pending_blob().unwrap()), "delete": [id] }),
    )
    .await;
    assert_eq!(deleted.status, StatusCode::OK, "{}", deleted.body);
    session.commit();

    // The object and the old index are gone from storage.
    let (status, _) = get_object(&mut browser, &world, &id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let mut files = Vec::new();
    every_file(world.root(), &mut files);
    assert!(
        !files.iter().any(|(_, bytes)| *bytes == old_blob),
        "the old index is deleted"
    );
    assert!(
        !files.iter().any(|(_, bytes)| *bytes == ciphertext),
        "the object is deleted"
    );

    // With the person's own keys and the current index, the kept
    // ciphertext opens to nothing; the other file still opens.
    let after = state(&mut browser, &world).await;
    let recovery = after["slots"]
        .as_array()
        .unwrap()
        .iter()
        .find(|slot| slot["method"] == "recovery")
        .unwrap();
    let mut device =
        Session::unlock_recovery(session.vault(), &recovery.to_string(), &words).unwrap();
    device
        .load_index(&unb64(after["index"]["blob"].as_str().unwrap()), 0)
        .unwrap();
    assert!(device.open(&id, &ciphertext).is_err());
    let (_, other) = get_object(&mut browser, &world, &keep).await;
    assert_eq!(
        device.open(&keep, &other).unwrap().as_slice(),
        b"still here"
    );
    // The deleted file's key is in no index the service still has.
    assert!(!device.rows(None).unwrap().contains(CANARY_NAME));
    // An index older than this device has seen is refused.
    assert!(device.load_index(&old_blob, device.epoch()).is_err());
}

#[tokio::test]
async fn another_account_can_neither_read_nor_change_the_vault() {
    let world = world().await;
    let (mut owner, csrf, mut session, _) = with_vault(&world, "octo-local").await;
    let id = upload(&mut owner, &world, &csrf, &mut session, CANARY_NAME, CANARY).await;

    let mut other = Browser::default();
    other.sign_in(&world, "quiet-local").await;
    let page = other.get(&world, super::PAGE).await;
    let other_csrf = token(&page.body);
    assert_eq!(state(&mut other, &world).await, Value::Null);
    let (status, body) = get_object(&mut other, &world, &id).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(!contains(&body, CANARY));
    let write = json_to(
        &mut other,
        &world,
        "/vault/api/index",
        &other_csrf,
        json!({ "after": 2, "blob": "AA==", "delete": [id] }),
    )
    .await;
    assert_ne!(write.status, StatusCode::OK);
    let delete = json_to(
        &mut other,
        &world,
        "/vault/api/delete",
        &other_csrf,
        json!({}),
    )
    .await;
    assert_eq!(delete.status, StatusCode::NO_CONTENT);
    // The owner's vault is untouched.
    let (status, _) = get_object(&mut owner, &world, &id).await;
    assert_eq!(status, StatusCode::OK);
    // The owner's token doesn't work for the other account.
    let stolen = json_to(&mut other, &world, "/vault/api/delete", &csrf, json!({})).await;
    assert_eq!(stolen.status, StatusCode::FORBIDDEN);
    // Signed out, nothing.
    let mut nobody = Browser::default();
    let (status, _) = get_object(&mut nobody, &world, &id).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn writes_need_the_page_token_and_enough_keys() {
    let world = world().await;
    let mut browser = Browser::default();
    browser.sign_in(&world, "octo-local").await;
    let page = browser.get(&world, super::PAGE).await;
    let csrf = token(&page.body);
    let session = Session::create().unwrap();
    let passkey: Value = serde_json::from_str(
        &session
            .passkey_slot("Passkey", 1, "127.0.0.1", b"c", &[1; 32], &[2; 32])
            .unwrap(),
    )
    .unwrap();
    let body = json!({ "vault": session.vault(), "slots": [passkey], "index": b64(&session.index_blob().unwrap()) });
    // A passkey alone is refused.
    let alone = json_to(
        &mut browser,
        &world,
        "/vault/api/create",
        &csrf,
        body.clone(),
    )
    .await;
    assert_eq!(alone.status, StatusCode::BAD_REQUEST, "{}", alone.body);
    assert!(alone.body.contains("recovery code"));
    // Without the page's token, refused.
    let forged = json_to(&mut browser, &world, "/vault/api/create", "forged", body).await;
    assert_eq!(forged.status, StatusCode::FORBIDDEN);
    // Fast answers need Gemini on the server; the test server has none.
    let fast = json_to(
        &mut browser,
        &world,
        "/vault/api/answer",
        &csrf,
        json!({ "question": "What's the balance?", "files": [{ "name": "a.txt", "media": "text/plain", "data": b64(b"balance 1") }] }),
    )
    .await;
    assert_eq!(
        fast.status,
        StatusCode::SERVICE_UNAVAILABLE,
        "{}",
        fast.body
    );
}

#[tokio::test]
async fn plaintext_is_refused_and_settings_links_the_vault() {
    let world = world().await;
    let (mut browser, csrf, session, _) = with_vault(&world, "octo-local").await;
    let id = oa_vault::new_id().unwrap();
    let put = put_object(&mut browser, &world, &csrf, &id, CANARY.to_vec()).await;
    assert_eq!(put.status, StatusCode::BAD_REQUEST);
    // A sealed object for another vault is refused too.
    let mut other = Session::create().unwrap();
    let (foreign, bytes) = other.add(add("x"), b"x").unwrap();
    let put = put_object(&mut browser, &world, &csrf, &foreign, bytes).await;
    assert_eq!(put.status, StatusCode::BAD_REQUEST);
    let _ = session;
    let settings = browser.get(&world, crate::settings::PAGE).await;
    assert!(
        settings.body.contains("Vault (only you)"),
        "{}",
        settings.body
    );
    assert!(settings.body.contains(super::PAGE));
}

#[test]
fn the_api_answers_no_one_else() {
    assert!(super::owns("/settings/vault"));
    assert!(super::owns("/vault/api/state"));
    assert!(super::owns("/projects/abc/vault"));
    assert!(!super::owns("/projects/abc/sources"));
}
