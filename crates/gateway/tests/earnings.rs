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
    _state: Option<Arc<ServeState>>,
    server: tokio::task::JoinHandle<Result<(), std::io::Error>>,
}

async fn deploy(qualified: bool) -> Deployment {
    let dir = tempfile::tempdir().unwrap();
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
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
    let server = tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
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
        _state: Some(state),
        server,
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
    for path in serve::mounted_paths(d._state.as_deref().unwrap()) {
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

#[tokio::test]
async fn native_referrer_statement_is_current_private_and_disabled_without_activation() {
    for enabled in [false, true] {
        let mut d = deploy(true).await;
        d.server.abort();
        let _ = (&mut d.server).await;
        drop(d._state.take());
        let accounts = Accounts::open(d.dir.path()).unwrap();
        let source = accounts
            .create_referrer(
                &d.alice_account,
                tenancy::accounts::referrals::Kind::Person,
                "Synthetic commission recipient",
            )
            .unwrap();
        let party = format!("referrer:{}", source.id);
        let path = d.dir.path().join("pay.sqlite");
        let mut ledger = Ledger::open(&path).unwrap();
        // This fixture exercises the HTTP permission boundary over a trusted
        // journal. Separate native plugin fixtures verify its original source.
        let a = pay_ledger::commission::Admission {
            schema: pay_ledger::commission::SCHEMA.into(),
            id: "d".repeat(64),
            ledger_origin: ledger.origin().unwrap(),
            payment_hash: "e".repeat(64),
            request_hash: "f".repeat(64),
            authorization: "1".repeat(64),
            buyer_account: "private-buyer".into(),
            buyer_workspace: "private-buyer-workspace".into(),
            operator_account: "private-merchant".into(),
            operator_workspace: "private-merchant-workspace".into(),
            customer: "private-customer".into(),
            referrer: source.id.clone(),
            party: party.clone(),
            agreement: "private-accepted-agreement".into(),
            terms: "private-terms".into(),
            contract: "private-bilateral-contract".into(),
            offer_digest: "private-offer".into(),
            invoice: "private-invoice".into(),
            receiver: "private-receiver".into(),
            payer: "private-payer".into(),
            plugin: "synthetic-plugin".into(),
            release: "synthetic-release".into(),
            author: "synthetic-author".into(),
            author_fee_msat: 1000,
            price_msat: 10000,
            numerator: 1,
            denominator: 2,
            exact_rounding: false,
            hold_secs: 0,
            minimum_msat: 1000,
            destinations: vec!["spark".into()],
            costs: [
                "model", "compute", "payment", "delivery", "support", "other",
            ]
            .into_iter()
            .map(|category| pay_ledger::commission::Cost {
                category: category.into(),
                amount_msat: Some(0),
                provenance: "operator-declared".into(),
                evidence: "private-cost-policy".into(),
            })
            .collect(),
            cost_policy: "private-policy".into(),
            admitted_at: 1_900_000_000,
        };
        ledger.admit_commission(&a).unwrap();
        ledger
            .record_settlement(SettlementInput {
                key: a.payment_hash.clone(),
                resource: "/v1/plugins/synthetic-plugin/invoke?buyer=private".into(),
                plugin_id: Some(a.plugin.clone()),
                release_id: Some(a.release.clone()),
                price_msat: 10000,
                received_msat: 10000,
                rail: Rail::Lightning,
                payer_alias: Some("private-payer-alias".into()),
                settled_at: 1_900_000_000,
                split: Split::Plugin {
                    author: a.author.clone(),
                    fee_msat: 1000,
                },
            })
            .unwrap();
        ledger
            .observe_commission(&a.id, &"a".repeat(64), Some(true), 1_900_000_001)
            .unwrap();
        ledger
            .reverse_commission(&a.id, &"b".repeat(64), &"c".repeat(64), 2000, 1_900_000_002)
            .unwrap();
        drop(ledger);
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600)).unwrap();
        let config:Config=serde_json::from_value(json!({"v":SCHEMA,"listen":"127.0.0.1:0","registry":d.dir.path(),"accounts":{},"earnings":{"commissions":enabled,"ledger":path,"grants":[{"party":party,"account":d.alice_account,"workspace":d.workspace}],"rails":{"lightning":"synthetic-qualified-rail","spark":"synthetic-qualified-rail"}}})).unwrap();
        let state = ServeState::open(config).unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
        let get = |path: String, token: &str| {
            reqwest::Client::new()
                .get(format!("{origin}{path}"))
                .bearer_auth(token)
                .send()
        };
        let index: Value = get("/v1/earnings".into(), &d.alice)
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            index["payees"].as_array().unwrap().len(),
            usize::from(enabled)
        );
        let url = format!("/v1/earnings/{party}/export");
        let reply = get(url.clone(), &d.alice).await.unwrap();
        if !enabled {
            assert_eq!(reply.status(), StatusCode::FORBIDDEN);
            continue;
        }
        assert_eq!(reply.status(), StatusCode::OK);
        assert_eq!(reply.headers()["cache-control"], "no-store");
        let text = reply.text().await.unwrap();
        for value in [
            "private-buyer",
            "private-customer",
            "private-invoice",
            "private-payer",
            "private-cost-policy",
            "private-policy",
            "private-accepted-agreement",
            "buyer=private",
        ] {
            assert!(!text.contains(value), "{value}");
        }
        let body: Value = serde_json::from_str(&text).unwrap();
        assert_eq!(
            body["capabilities"]["commissions"],
            "native_plugin_merchant"
        );
        assert_eq!(body["commission"]["original_earned_msat"], 4500);
        assert_eq!(body["commission"]["reversed_msat"], 900);
        assert_eq!(
            get(url.clone(), &d.bob).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let refused = reqwest::Client::new()
            .put(format!("{origin}/v1/earnings/{party}/destination"))
            .bearer_auth(&d.key)
            .json(&json!({"expected_version":0,"value":"recipient@example.com"}))
            .send()
            .await
            .unwrap();
        assert_eq!(refused.status(), StatusCode::BAD_REQUEST);
        assert!(
            Ledger::open(&path)
                .unwrap()
                .account_payout(&party)
                .unwrap()
                .is_none()
        );
        accounts
            .offer_referrer_migration(&d.alice_account, &source.id, &d.owner)
            .unwrap();
        accounts
            .accept_referrer_migration(&d.owner, &source.id)
            .unwrap();
        let migrated = reqwest::Client::new()
            .put(format!("{origin}/v1/earnings/{party}/destination"))
            .bearer_auth(&d.key)
            .json(&json!({"expected_version":0,"value":"recipient@example.com"}))
            .send()
            .await
            .unwrap();
        assert_eq!(migrated.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            get(url, &d.alice).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let index: Value = get("/v1/earnings".into(), &d.alice)
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(index["payees"], json!([]));
    }
}
