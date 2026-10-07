//! Real HTTP signup and current-account source custody use synthetic identities.
use super::*;
use tenancy::accounts::referrals::{CONSENT, Outcome};

#[tokio::test]
async fn referral_link_survives_signup_without_private_cross_account_reads() {
    let d = deploy(Some(account_config(None)), true).await;
    let alice = join(&d, "Source owner").await;
    let bob = join(&d, "Other").await;
    let before = Accounts::open(d.dir.path())
        .unwrap()
        .store()
        .unwrap()
        .sequence;
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/account/referrers", d.address))
            .bearer_auth(&alice.session_token)
            .header("x-openagents-referral-account", &bob.account)
            .json(&json!({"kind":"person","label":"Mismatched selected account"})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        Accounts::open(d.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .sequence,
        before
    );
    let (status, _) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/account/acquisition", d.address))
            .bearer_auth(&alice.session_token)
            .header("x-openagents-referral-account", &bob.account),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, created) = post(
        &d,
        "/v1/account/referrers",
        Some(&alice.session_token),
        &json!({"kind":"person","label":"Private introduction notes"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let id = created["referral"]["id"].as_str().unwrap();
    let path = format!("/v1/account/referrers/{id}");
    let (status, denied) = get(&d, &path, &bob.session_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!denied.to_string().contains("Private introduction"));
    let (status, issued) = post(
        &d,
        &format!("{path}/link"),
        Some(&alice.session_token),
        &json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{issued}");
    let token = issued["referral"]["token"].as_str().unwrap();
    let public = issued["referral"]["path"].as_str().unwrap();
    assert!(!public.contains(id));
    assert!(!public.contains(&alice.account));
    let (status, terms) =
        exchange(reqwest::Client::new().get(format!("{}{public}", d.address))).await;
    assert_eq!(status, StatusCode::OK, "{terms}");
    let source_input = json!({"request":"signup-with-referral","token":token,"consent":true,"consent_version":CONSENT});
    let (status, signup) = post(
        &d,
        "/v1/accounts",
        None,
        &json!({"label":"Introduced buyer","acquisition":source_input}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{signup}");
    assert_eq!(signup["acquisition"]["referrer"]["id"], id);
    assert_eq!(signup["acquisition"]["outcome"], "captured");
    let buyer_token = signup["session_token"].as_str().unwrap();
    let sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(jev::ApiKey::new(buyer_token)),
    )
    .unwrap();
    let sdk_source = sdk.account().acquisition().await.unwrap().unwrap();
    assert_eq!(sdk_source.account, signup["account"]["id"]);
    assert_eq!(sdk_source.referrer.unwrap().id, id);
    let (status, source) = get(&d, "/v1/account/acquisition", buyer_token).await;
    assert_eq!(status, StatusCode::OK, "{source}");
    assert_eq!(source["referral"], signup["acquisition"]);
    let store = Accounts::open(d.dir.path()).unwrap();
    let seq = store.store().unwrap().sequence;
    let (status, duplicate) = post(
        &d,
        "/v1/account/acquisition",
        Some(buyer_token),
        &source_input,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(duplicate, source);
    assert_eq!(store.store().unwrap().sequence, seq);
    let (status, _) = post(
        &d,
        "/v1/account/acquisition",
        Some(buyer_token),
        &json!({"request":"changed","token":null,"consent":false,"consent_version":null}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = post(
        &d,
        "/v1/accounts",
        None,
        &json!({"label":"Replayed signup","acquisition":source_input}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    // An attacker cannot select the buyer on the private account read.
    let (status, other) = get(&d, "/v1/account/acquisition", &bob.session_token).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(other["referral"]["account"], bob.account);
    assert_eq!(other["referral"]["outcome"], "missing");
    assert!(!other.to_string().contains(id));
    let (status, _) = remove(&d, &format!("{path}/link"), &bob.session_token).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = remove(&d, &format!("{path}/link"), &alice.session_token).await;
    assert_eq!(status, StatusCode::OK);
    let (status, disabled) = post(&d, "/v1/accounts", None, &json!({"label":"Disabled source","acquisition":{"request":"disabled-signup","token":token,"consent":true,"consent_version":CONSENT}})).await;
    assert_eq!(status, StatusCode::CREATED);
    assert_eq!(disabled["acquisition"]["outcome"], "disabled");
    assert!(disabled["acquisition"]["referrer"].is_null());
    assert_eq!(
        store
            .acquisition(signup["account"]["id"].as_str().unwrap())
            .unwrap()
            .unwrap()
            .outcome,
        Outcome::Captured
    );
    // No source route accepts a caller-selected account or source-only upgrade.
    let (status, _) = post(
        &d,
        "/v1/account/referrers",
        Some(&bob.session_token),
        &json!({"kind":"agent","label":"Pretend operator","source_only":true}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    d.server.abort();
}
