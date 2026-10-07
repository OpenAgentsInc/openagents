//! Real HTTP and dashboard checks over isolated account and payee state.

mod common;

use axum::http::StatusCode;
use gateway::config::{Config, SCHEMA};
use gateway::serve::{self, ServeState};
use pay_ledger::{Ledger, PayoutState, Rail, SettlementInput, Split};
use serde_json::{Value, json};
use std::sync::Arc;
use tenancy::workspaces::UserId;
use tenancy::{Accounts, Registry, Role, SessionBook, Sessions, WorkspaceKind, keys};

struct Deployment {
    dir: tempfile::TempDir,
    address: String,
    alice: String,
    bob: String,
    alice_account: String,
    owner: String,
    workspace: String,
    key: String,
    key_id: String,
    _state: Arc<ServeState>,
}

async fn deploy(qualified: bool) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    let registry =
        Registry::install(dir.path(), common::manifest(&common::artifact('b'), None)).unwrap();
    let accounts = Accounts::install(dir.path()).unwrap();
    let owner = accounts.create_account("Owner", &[]).unwrap();
    let alice_account = accounts.create_account("Alice", &[]).unwrap();
    let bob_account = accounts.create_account("Bob", &[]).unwrap();
    let workspace = accounts
        .create_workspace(
            &owner.id,
            "Authors",
            WorkspaceKind::Organization,
            "acme",
            None,
        )
        .unwrap();
    for account in [&alice_account, &bob_account] {
        let invite = accounts
            .invite(&owner.id, &workspace.id, Role::Member, 3600)
            .unwrap();
        accounts.accept(&account.id, &invite.token).unwrap();
    }
    let key = keys::issue(dir.path(), registry.manifest(), "acme").unwrap();
    accounts
        .update_principals(&alice_account.id, &[format!("key:{}", key.key.id)])
        .unwrap();
    let sessions = Sessions::install(dir.path(), SessionBook::new(3600, 3600)).unwrap();
    let alice = sessions
        .mutate(|book, _, now| book.issue(UserId::from(alice_account.id.as_str()), now))
        .unwrap()
        .once;
    let bob = sessions
        .mutate(|book, _, now| book.issue(UserId::from(bob_account.id.as_str()), now))
        .unwrap()
        .once;
    let path = dir.path().join("pay.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    for (party, key) in [
        ("alice", "private-payment-hash"),
        ("bob", "other-private-hash"),
    ] {
        ledger
            .record_settlement(SettlementInput {
                key: key.into(),
                resource: format!("/v1/plugins/{party}/invoke?credential=private"),
                plugin_id: Some(party.into()),
                release_id: Some(format!("{party}-release")),
                price_msat: 20_555,
                received_msat: 20_555,
                rail: Rail::Lightning,
                payer_alias: Some("private-payer".into()),
                settled_at: 1_792_022_400,
                split: Split::Plugin {
                    author: party.into(),
                    fee_msat: 15_555,
                },
            })
            .unwrap();
    }
    ledger
        .change_account_payout("alice", 0, "alice@example.com", 1_792_022_400)
        .unwrap();
    let shares = ledger.available_shares("alice").unwrap();
    let owed = ledger
        .reserve_payout("alice-attempt", "alice", &shares, 1_792_022_400)
        .unwrap();
    ledger
        .begin_send(
            "alice-attempt",
            "wallet-reference",
            Some("private-invoice"),
            owed / 1000 * 1000,
            1_792_022_401,
        )
        .unwrap();
    ledger
        .finish_payout(
            "alice-attempt",
            PayoutState::Unknown,
            None,
            Some("private-rail-error"),
            1_792_022_402,
        )
        .unwrap();
    drop(ledger);
    let config:Config=serde_json::from_value(json!({"v":SCHEMA,"listen":"127.0.0.1:0","registry":dir.path(),"accounts":{},"earnings":{"ledger":path,"grants":[
        {"party":"alice","account":alice_account.id,"workspace":workspace.id},
        {"party":"bob","account":bob_account.id,"workspace":workspace.id}
    ],"rails":if qualified {json!({"lightning":"synthetic-fixture-qualification"})} else {json!({})}}})).unwrap();
    let state = ServeState::open(config).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    Deployment {
        dir,
        address,
        alice,
        bob,
        alice_account: alice_account.id,
        owner: owner.id,
        workspace: workspace.id,
        key: key.token,
        key_id: key.key.id,
        _state: state,
    }
}

async fn get(d: &Deployment, path: &str, token: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!("{}{path}", d.address))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
}

async fn change(d: &Deployment, token: &str, version: u64, value: &str) -> reqwest::Response {
    reqwest::Client::new()
        .put(format!("{}/v1/earnings/alice/destination", d.address))
        .bearer_auth(token)
        .json(&json!({"expected_version":version,"value":value}))
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn private_statement_export_and_reconciliation_are_payee_scoped() {
    let d = deploy(true).await;
    let response = get(&d, "/v1/earnings/alice?limit=1", &d.alice).await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["unit"], "msat");
    assert_eq!(body["capabilities"]["commissions"], "unavailable");
    assert_eq!(
        body["statement"]["figures"]["earned_msat"],
        body["statement"]["figures"]["reserved_msat"]
    );
    assert_eq!(body["statement"]["payouts"][0]["state"], "unknown");
    assert_eq!(
        body["statement"]["payouts"][0]["wallet_reference"],
        "wallet-reference"
    );
    assert_eq!(
        get(&d, "/v1/earnings/alice", &d.bob).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        change(&d, &d.bob, 1, "thief@example.com").await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        get(&d, "/v1/earnings/bob/payouts/alice-attempt", &d.bob)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    let response = get(&d, "/v1/earnings/alice/export", &d.alice).await;
    assert!(
        response.headers()["content-disposition"]
            .to_str()
            .unwrap()
            .contains("attachment")
    );
    let text = response.text().await.unwrap();
    for secret in [
        "private-payment-hash",
        "private-payer",
        "private-invoice",
        "private-rail-error",
        "credential=private",
        "other-private-hash",
        "bob-release",
    ] {
        assert!(!text.contains(secret), "{secret}: {text}");
    }
    assert_eq!(
        get(&d, "/v1/earnings/alice?limit=201", &d.alice)
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        get(&d, "/v1/earnings/alice/payouts/alice-attempt", &d.alice)
            .await
            .status(),
        StatusCode::OK
    );
    let catalog: Value = reqwest::get(format!("{}/api-catalog.json", d.address))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    for path in serve::mounted_paths(&d._state) {
        assert!(
            catalog["routes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|r| r["path"] == path)
                || !path.starts_with("/v1/earnings") && !path.starts_with("/dashboard/earnings"),
            "Missing catalog path {path}"
        );
    }
}

#[tokio::test]
async fn destination_changes_conflict_without_rewriting_the_unknown_attempt() {
    let d = deploy(true).await;
    let response = change(&d, &d.alice, 1, "changed@example.com").await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: Value = response.json().await.unwrap();
    assert_eq!(body["destination"]["setting"]["version"], 2);
    assert_eq!(
        body["destination"]["effective"]["rail_status"],
        "owner_qualified"
    );
    assert_eq!(
        change(&d, &d.alice, 1, "stale@example.com").await.status(),
        StatusCode::CONFLICT
    );
    let body: Value = get(&d, "/v1/earnings/alice/payouts/alice-attempt", &d.alice)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["payout"]["destination"], "lud16:alice@example.com");
    assert_eq!(body["payout"]["state"], "unknown");
    assert_eq!(
        change(&d, &d.alice, 2, &format!("02{}", "ab".repeat(32)))
            .await
            .status(),
        StatusCode::BAD_REQUEST
    );
    let unsupported = deploy(false).await;
    assert_eq!(
        change(&unsupported, &unsupported.alice, 1, "changed@example.com")
            .await
            .status(),
        StatusCode::SERVICE_UNAVAILABLE
    );
}

#[tokio::test]
async fn revoked_keys_sessions_and_workspace_members_lose_access() {
    let d = deploy(true).await;
    assert_eq!(
        get(&d, "/v1/earnings/alice", &d.key).await.status(),
        StatusCode::OK
    );
    keys::revoke(d.dir.path(), &d.key_id).unwrap();
    assert_eq!(
        get(&d, "/v1/earnings/alice", &d.key).await.status(),
        StatusCode::UNAUTHORIZED
    );
    Sessions::open(d.dir.path())
        .unwrap()
        .mutate(|book, _, now| {
            book.revoke_all(&UserId::from(d.alice_account.as_str()), now);
            Ok(())
        })
        .unwrap();
    assert_eq!(
        get(&d, "/v1/earnings/alice", &d.alice).await.status(),
        StatusCode::UNAUTHORIZED
    );
    let d = deploy(true).await;
    Accounts::open(d.dir.path())
        .unwrap()
        .remove_member(&d.owner, &d.workspace, &d.alice_account)
        .unwrap();
    assert_eq!(
        get(&d, "/v1/earnings/alice", &d.alice).await.status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        change(&d, &d.alice, 1, "changed@example.com")
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    let body: Value = get(&d, "/v1/earnings", &d.alice)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(body["payees"], json!([]));
}

#[tokio::test]
async fn dashboard_has_private_export_and_session_bound_destination_form() {
    let d = deploy(true).await;
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    let cookie = format!("oa_session={}", d.alice);
    let response = client
        .get(format!("{}/dashboard/earnings/alice", d.address))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "no-store");
    let text = response.text().await.unwrap();
    assert!(text.contains("Export this page") && text.contains("Unknown payouts stay reserved"));
    assert!(!text.contains("private-invoice") && !text.contains("private-payer"));
    let csrf = text
        .split("name=\"csrf\" value=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let path = format!("{}/dashboard/earnings/alice/destination", d.address);
    let bad = client
        .post(&path)
        .header("cookie", &cookie)
        .form(&[
            ("expected_version", "1"),
            ("value", "changed@example.com"),
            ("csrf", "forged"),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(bad.status(), StatusCode::FORBIDDEN);
    let good = client
        .post(&path)
        .header("cookie", &cookie)
        .form(&[
            ("expected_version", "1"),
            ("value", "changed@example.com"),
            ("csrf", csrf),
        ])
        .send()
        .await
        .unwrap();
    assert_eq!(good.status(), StatusCode::SEE_OTHER);
    let response = client
        .get(format!("{}/dashboard/earnings/alice/export", d.address))
        .header("cookie", &cookie)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let denied = client
        .get(format!("{}/dashboard/earnings/alice", d.address))
        .header("cookie", format!("oa_session={}", d.bob))
        .send()
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn oversized_private_exports_refuse_with_a_bounded_response() {
    let d = deploy(true).await;
    let mut ledger = Ledger::open(d.dir.path().join("pay.sqlite")).unwrap();
    ledger
        .record_settlement(SettlementInput {
            key: "oversized-private-hash".into(),
            resource: "x".repeat(1_048_577),
            plugin_id: Some("large".into()),
            release_id: None,
            price_msat: 5_000,
            received_msat: 5_000,
            rail: Rail::Lightning,
            payer_alias: None,
            settled_at: 1_792_022_400,
            split: Split::Plugin {
                author: "alice".into(),
                fee_msat: 0,
            },
        })
        .unwrap();
    let response = get(&d, "/v1/earnings/alice/export", &d.alice).await;
    assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    let body = response.bytes().await.unwrap();
    assert!(body.len() < 1024);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["error"]["code"],
        "earnings_export_too_large"
    );
    assert_eq!(
        get(&d, "/v1/earnings/alice/export?limit=1", &d.alice)
            .await
            .status(),
        StatusCode::OK
    );
}
