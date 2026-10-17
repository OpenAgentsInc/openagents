//! Signing in with a Nostr key (#11993) over HTTP: the NIP-98 sign-in,
//! the account's keys, and an existing GitHub account adding its first key.
use base64::Engine as _;
use base64::engine::general_purpose::STANDARD;
use nostr::domain::{RelaySigner, Tag};
use sha2::{Digest, Sha256};

use super::*;

fn signer(seed: u8) -> RelaySigner {
    RelaySigner::from_secret_hex(&format!("{seed:02x}").repeat(32)).unwrap()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

fn proof(key: &RelaySigner, url: &str, challenge: &str, payload: Option<&[u8]>) -> String {
    let mut tags = vec![
        Tag::new(vec!["u".into(), url.into()]),
        Tag::new(vec!["method".into(), "POST".into()]),
        Tag::new(vec!["challenge".into(), challenge.into()]),
    ];
    if let Some(body) = payload {
        tags.push(Tag::new(vec![
            "payload".into(),
            hex::encode(Sha256::digest(body)),
        ]));
    }
    let event = key.sign(now(), 27_235, tags, String::new());
    STANDARD.encode(serde_json::to_vec(&event).unwrap())
}

async fn challenge(d: &Deployment, path: &str, token: Option<&str>) -> String {
    let mut request = reqwest::Client::new().get(format!("{}{path}", d.address));
    if let Some(token) = token {
        request = request.bearer_auth(token);
    }
    let (status, body) = exchange(request).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["challenge"].as_str().unwrap().to_string()
}

/// Sign in with `key` through `/v1/sessions/nostr`, over `body`.
async fn sign_in_raw(
    d: &Deployment,
    key: &RelaySigner,
    body: &[u8],
) -> (StatusCode, Value, String) {
    let challenge = challenge(d, "/v1/sessions/nostr/challenge", None).await;
    let url = format!("{}/v1/sessions/nostr", d.address);
    let header = format!("Nostr {}", proof(key, &url, &challenge, Some(body)));
    let (status, answer) = exchange(
        reqwest::Client::new()
            .post(&url)
            .header("authorization", &header)
            .header("content-type", "application/json")
            .body(body.to_vec()),
    )
    .await;
    (status, answer, header)
}

async fn link(d: &Deployment, token: &str, key: &RelaySigner) -> (StatusCode, Value) {
    let challenge = challenge(d, "/v1/account/keys/challenge", Some(token)).await;
    let url = format!("{}/v1/account/keys", d.address);
    post(
        d,
        "/v1/account/keys",
        Some(token),
        &json!({"proof": proof(key, &url, &challenge, None)}),
    )
    .await
}

#[tokio::test]
async fn a_key_signs_in_to_one_account_whose_session_works() {
    let mut accounts = account_config(None);
    // No invite list or sign-up switch applies: the key is the account.
    accounts.open_signup = false;
    let d = deploy(Some(accounts), true).await;
    let key = signer(1);
    let body = br#"{"app":"OpenAgents","computer":"Pat's iPhone"}"#;
    let (status, first, header) = sign_in_raw(&d, &key, body).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["created"], true);
    let token = first["token"].as_str().unwrap();
    let (status, view) = get(&d, "/v1/account", token).await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["account"]["id"], first["account"]["id"]);
    assert_eq!(view["workspaces"].as_array().unwrap().len(), 1);
    // A key-only account that isn't attested gets no free cloud time (#11994).
    assert_eq!(view["account"]["free_cloud"], "none");

    // The same request again is a replay.
    let (status, replay) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/sessions/nostr", d.address))
            .header("authorization", &header)
            .body(body.to_vec()),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{replay}");
    assert_eq!(code(&replay), "proof_used");

    let (status, second, _) = sign_in_raw(&d, &key, body).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["created"], false);
    assert_eq!(second["account"]["id"], first["account"]["id"]);

    // A body that doesn't match its hash is refused, and so is no header.
    let challenge = challenge(&d, "/v1/sessions/nostr/challenge", None).await;
    let url = format!("{}/v1/sessions/nostr", d.address);
    let wrong = format!("Nostr {}", proof(&key, &url, &challenge, Some(b"{}")));
    let (status, refused) = exchange(
        reqwest::Client::new()
            .post(&url)
            .header("authorization", wrong)
            .body(body.to_vec()),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(code(&refused), "sign_in_denied");
    let (status, _) = post(&d, "/v1/sessions/nostr", None, &json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);

    // Keys: list, add, remove, never the last.
    let (status, listed) = get(&d, "/v1/account/keys", token).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["keys"][0]["pubkey"], key.pubkey());
    let desk = signer(2);
    let (status, added) = link(&d, token, &desk).await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["keys"].as_array().unwrap().len(), 2);
    let (status, _) = remove(&d, &format!("/v1/account/keys/{}", key.pubkey()), token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, last) = remove(&d, &format!("/v1/account/keys/{}", desk.pubkey()), token).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(code(&last), "last_key");
    // Key routes need a session.
    let (status, _) = get(&d, "/v1/account/keys", "sess_nope").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn an_existing_github_account_adds_a_key_with_its_session() {
    let d = deploy(Some(account_config(None)), true).await;
    let profile: tenancy::accounts::identities::GithubProfile =
        serde_json::from_value(json!({"id": 4242, "login": "pat"})).unwrap();
    let github = Accounts::open(d.dir.path())
        .unwrap()
        .sign_in_github(&profile, "acme")
        .unwrap();
    let issued = Sessions::open(d.dir.path())
        .unwrap()
        .mutate(|book, _, now| {
            book.issue_app(
                tenancy::workspaces::UserId::from(github.account.id.as_str()),
                tenancy::sessions::AppLabel::new("OpenAgents", "Browser").unwrap(),
                now,
            )
        })
        .unwrap();
    let key = signer(3);
    let (status, added) = link(&d, &issued.once, &key).await;
    assert_eq!(status, StatusCode::OK, "{added}");
    assert_eq!(added["key"]["pubkey"], key.pubkey());
    // The key now signs in to the GitHub account.
    let (status, signed, _) = sign_in_raw(&d, &key, b"").await;
    assert_eq!(status, StatusCode::OK, "{signed}");
    assert_eq!(signed["account"]["id"], github.account.id.as_str());
    assert_eq!(signed["created"], false);
    // A key that already holds another account can't be added.
    let other = signer(4);
    assert_eq!(sign_in_raw(&d, &other, b"").await.0, StatusCode::OK);
    let (status, taken) = link(&d, &issued.once, &other).await;
    assert_eq!(status, StatusCode::CONFLICT, "{taken}");
    assert_eq!(code(&taken), "identity_taken");
}

/// Sign in with `key`, the body made from the challenge.
async fn sign_in_over(
    d: &Deployment,
    key: &RelaySigner,
    forwarded: &str,
    body: impl Fn(&str) -> Value,
) -> (StatusCode, Value, Option<String>) {
    let challenge = challenge(d, "/v1/sessions/nostr/challenge", None).await;
    let url = format!("{}/v1/sessions/nostr", d.address);
    let body = serde_json::to_vec(&body(&challenge)).unwrap();
    let header = format!("Nostr {}", proof(key, &url, &challenge, Some(&body)));
    let response = reqwest::Client::new()
        .post(&url)
        .header("authorization", &header)
        .header("content-type", "application/json")
        .header("x-forwarded-for", forwarded)
        .body(body)
        .send()
        .await
        .unwrap();
    let status = StatusCode::from_u16(response.status().as_u16()).unwrap();
    let wait = response
        .headers()
        .get("retry-after")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    (status, response.json().await.unwrap(), wait)
}

/// A solution to the sign-in proof of work at `bits`.
fn solve(challenge: &str, bits: u8) -> String {
    (0_u64..)
        .map(|n| n.to_string())
        .find(|s| gateway::key_accounts::check_pow(challenge, s, bits).is_ok())
        .unwrap()
}

#[tokio::test]
async fn a_new_unattested_key_needs_a_proof_of_work_and_fits_the_daily_caps() {
    let mut accounts = account_config(None);
    accounts.key_accounts = gateway::key_accounts::KeyAccounts {
        difficulty_bits: 16,
        per_ip_per_day: 1,
        per_day: 2,
        ..Default::default()
    };
    let d = deploy(Some(accounts), true).await;
    let app = |c: &str| json!({"app": "OpenAgents", "computer": "Desk", "pow": solve(c, 16)});

    // No proof, or a wrong one: refused, and no account is made.
    let (status, refused, _) = sign_in_over(
        &d,
        &signer(10),
        "10.0.0.1",
        |_| json!({"app": "OpenAgents"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(code(&refused), "proof_required");
    let (status, refused, _) =
        sign_in_over(&d, &signer(10), "10.0.0.1", |_| json!({"pow": "nope"})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(code(&refused), "invalid_proof");

    // With one: the account is made.
    let (status, made, _) = sign_in_over(&d, &signer(10), "10.0.0.1", app).await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(made["created"], true);
    // The same key signs in again with no proof: it has its account.
    let (status, again, _) = sign_in_over(
        &d,
        &signer(10),
        "10.0.0.1",
        |_| json!({"app": "OpenAgents"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["created"], false);

    // The address made all it may today.
    let (status, capped, wait) = sign_in_over(&d, &signer(11), "10.0.0.1", app).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{capped}");
    assert_eq!(code(&capped), "signup_limit");
    assert!(wait.is_some_and(|w| w.parse::<u64>().unwrap() <= 86_400));
    // Another address may, until everyone's cap for the day is reached.
    let (status, _, _) = sign_in_over(&d, &signer(11), "10.9.0.1", app).await;
    assert_eq!(status, StatusCode::OK);
    let (status, capped, _) = sign_in_over(&d, &signer(12), "10.8.0.1", app).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{capped}");
    assert!(
        capped["error"]["message"]
            .as_str()
            .unwrap()
            .contains("today")
    );
}

#[tokio::test]
async fn an_attestation_that_does_not_verify_is_refused() {
    let d = deploy(Some(account_config(None)), true).await;
    let (status, refused, _) = sign_in_over(
        &d,
        &signer(20),
        "10.0.0.2",
        |_| json!({"app_attest": {"key_id": "AAAA", "attestation": "o2NmbXQ="}}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(code(&refused), "invalid_attestation");
    let (status, refused, _) = sign_in_over(
        &d,
        &signer(20),
        "10.0.0.2",
        |_| json!({"app_attest": "nope"}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{refused}");
    assert_eq!(code(&refused), "invalid_attestation");
}
