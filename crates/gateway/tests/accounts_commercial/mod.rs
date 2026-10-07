//! Current native source mappings stay separate from account and money admission.
use super::*;
use commercial_accounts::{Entry, NativeSources, NativeStore, Policy};
use receipts::purchase::{Approval, CommercialProduct, HEADER, Quote};
use std::{
    os::unix::fs::PermissionsExt,
    path::Path,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering},
    time::Duration,
};
use tenancy::{
    Role, WorkspaceKind,
    accounts::commercial::{Product, Source},
};

fn private(path: &Path, bytes: &[u8]) {
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
fn canonical_customer(accounts: &Accounts, label: &str, principal: &str) -> (String, String) {
    let account = accounts.create_account(label, &[principal.into()]).unwrap();
    let workspace = accounts
        .create_workspace(
            &account.id,
            label,
            WorkspaceKind::Personal,
            "canonical",
            None,
        )
        .unwrap();
    (account.id, workspace.id)
}
async fn restart(d: &mut Deployment) {
    d.server.abort();
    let _ = (&mut d.server).await;
    d._state.take();
    let state = ServeState::open(d.config.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind(d.address.strip_prefix("http://").unwrap())
        .await
        .unwrap();
    d.server = tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    d._state = Some(state);
}
#[derive(Default)]
struct VerificationGate {
    armed: AtomicBool,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
    forwards: AtomicUsize,
}
async fn verification_backend() -> (String, Arc<VerificationGate>) {
    let gate = Arc::new(VerificationGate::default());
    let verifying = gate.clone();
    let forwarding = gate.clone();
    let router = axum::Router::new()
        .route(
            "/v1/models",
            axum::routing::get(move || {
                let verifying = verifying.clone();
                async move {
                    if verifying.armed.swap(false, Ordering::SeqCst) {
                        verifying.entered.notify_one();
                        verifying.release.notified().await;
                    }
                    axum::Json(json!({"models":[{"id":"kev-0.6b","name":"kev-0.6b",
                    "artifact_identity":{"digest":artifact('b')},"execution":{}}]}))
                }
            }),
        )
        .route(
            "/v1/systemone",
            axum::routing::post(move || {
                let forwarding = forwarding.clone();
                async move {
                    forwarding.forwards.fetch_add(1, Ordering::SeqCst);
                    axum::Json(answer())
                }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(axum::serve(listener, router).into_future());
    (endpoint, gate)
}
fn receipt(d: &Deployment, request: Option<&str>) -> receipts::execution::ExecutionReceipt {
    std::fs::read_to_string(d.dir.path().join("receipts.jsonl"))
        .unwrap()
        .lines()
        .rev()
        .map(|line| receipts::execution::ExecutionReceipt::parse(line).unwrap())
        .find(|receipt| request.is_none_or(|request| receipt.request == request))
        .expect("the actual request sealed its receipt")
}
async fn native_balance(d: &Deployment, customer: &Joined) -> Value {
    let (status, position) = exchange(
        reqwest::Client::new()
            .get(format!("{}/v1/balance", d.address))
            .bearer_auth(&customer.session_token)
            .header("x-workspace-id", &customer.workspace),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{position}");
    assert!(position["balance"]["available"].as_u64().is_some());
    position["balance"].clone()
}
#[tokio::test]
async fn native_commercial_selection_preserves_payers_and_requires_current_review() {
    exercise(None).await;
}
#[tokio::test]
#[ignore = "requires a freshly built installed customer CLI"]
async fn installed_commercial_selection_preserves_original_quote_through_migration() {
    exercise(Some(
        std::env::var_os("OPENAGENTS_REV19_TEST_CLI")
            .expect("Set OPENAGENTS_REV19_TEST_CLI to the fresh binary."),
    ))
    .await;
}
async fn exercise(binary: Option<std::ffi::OsString>) {
    use tenancy::money::{CreditKind, Ledger, Mutation, Operation, Price, Rate, Resource};
    let mut d = deploy(Some(account_config(None)), true).await;
    std::fs::set_permissions(d.dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let ada = join(&d, "Duplicate public label").await;
    let bob = join(&d, "Duplicate public label").await;
    let (status, issued) = post(
        &d,
        &format!("/v1/workspaces/{}/keys", ada.workspace),
        Some(&ada.session_token),
        &json!({"name":"inference-only", "scopes":{"models":["acme-kev"], "actions":["inference"]}}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let inference_key = issued["key_token"].as_str().unwrap().to_owned();
    std::fs::set_permissions(
        d.dir.path().join("keys.json"),
        std::fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let canonical_dir = tempfile::tempdir().unwrap();
    std::fs::set_permissions(canonical_dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let canonical = Accounts::install(canonical_dir.path()).unwrap();
    let (customer, workspace) = canonical_customer(
        &canonical,
        "Independent canonical identity",
        "key:aaaaaaaaaaaaaaaa",
    );
    let (other, _other_workspace) = canonical_customer(
        &canonical,
        "Independent canonical identity",
        "key:bbbbbbbbbbbbbbbb",
    );
    let native = Accounts::open(d.dir.path()).unwrap();
    let native_owner = native.authorize(&ada.workspace, &ada.account).unwrap();
    let canonical_owner = canonical.authorize(&workspace, &customer).unwrap();
    let key_file = canonical_dir.path().join("native-key");
    private(&key_file, ada.key_token.as_bytes());
    let source = Source {
        product: Product::Gateway,
        issuer: "actual-gateway-fixture".into(),
        account: ada.account.clone(),
        workspace: Some(ada.workspace.clone()),
    };
    let plugin = Source {
        product: Product::Plugin,
        ..source.clone()
    };
    let principal = format!(
        "key:{}",
        ada.key_token
            .strip_prefix("oak_")
            .unwrap()
            .split('.')
            .next()
            .unwrap()
    );
    let now = unix_now();
    let entry = Entry {
        operator: "fixture-operator".into(),
        source: source.clone(),
        customer: customer.clone(),
        workspace: workspace.clone(),
        canonical_owner: customer.clone(),
        canonical_owner_epoch: canonical_owner.epoch,
        canonical_members_epoch: canonical_owner.members_epoch,
        principal,
        credential_file: key_file.clone(),
        generation: 1,
        native_owner: Some(ada.account.clone()),
        native_owner_epoch: Some(native_owner.epoch),
        native_members_epoch: Some(native_owner.members_epoch),
        reviewed_at: now,
        valid_until: now + 3600,
        previous_authority: None,
    };
    let mut entries = vec![
        entry.clone(),
        Entry {
            source: plugin.clone(),
            ..entry
        },
    ];
    let policy = canonical_dir.path().join("policy.json");
    let write_policy = |entries: &[Entry]| {
        private(
            &policy,
            &serde_json::to_vec(&Policy {
                schema: commercial_accounts::SCHEMA.into(),
                operator: "fixture-operator".into(),
                entries: entries.to_vec(),
            })
            .unwrap(),
        )
    };
    write_policy(&entries);
    let native_config = commercial_accounts::Config {
        policy: policy.clone(),
        stores: vec![NativeStore::Tenancy {
            issuer: source.issuer.clone(),
            directory: d.dir.path().into(),
        }],
    };
    let adapter = NativeSources::open(canonical_dir.path(), &native_config).unwrap();
    let review = canonical
        .review_commercial(
            "actual-commercial",
            &customer,
            &workspace,
            &customer,
            None,
            &[source.clone(), plugin.clone()],
            &adapter,
        )
        .unwrap();
    canonical
        .admit_commercial(&review, &review.digest, &adapter)
        .unwrap();
    // Monetary books retain native workspaces, independent of the mapping.
    let ledger_path = d.dir.path().join("commercial-money.jsonl");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    for (ws, amount) in [(&ada.workspace, 100_000), (&bob.workspace, 200_000)] {
        for (id, operation) in [
            (
                "create",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: amount,
                    topups_allowed: false,
                },
            ),
            (
                "synthetic",
                Operation::Credit {
                    amount,
                    credit_kind: CreditKind::Grant,
                },
            ),
        ] {
            ledger
                .apply(Mutation {
                    workspace: ws.clone(),
                    source: id.into(),
                    audit: "isolated-commercial-fixture".into(),
                    operation,
                })
                .unwrap();
        }
    }
    drop(ledger);
    d.config.money = Some(gateway::money::Money {
        ledger: ledger_path.clone(),
        doors: BTreeMap::from([(
            "acme-kev".into(),
            gateway::money::Priced {
                offer: None,
                price: Price {
                    version: "commercial-price".into(),
                    currency: "USD".into(),
                    model: "kev-0.6b".into(),
                    capacity: "dedicated".into(),
                    policy: gateway::money::POLICY.into(),
                    rates: BTreeMap::from([(
                        Resource::InputTokens,
                        Rate {
                            millionths: 1,
                            per_units: 1,
                        },
                    )]),
                },
                maximum_usage: BTreeMap::from([(Resource::InputTokens, 100)]),
            },
        )]),
    });
    d.config.commercial = Some(config::Commercial {
        canonical_directory: canonical_dir.path().into(),
        issuer: source.issuer.clone(),
        native: native_config.clone(),
    });
    let (endpoint, verification) = verification_backend().await;
    d.config.doors.get_mut("acme-kev").unwrap().endpoint = endpoint;
    restart(&mut d).await;
    let (status, _) = get(
        &d,
        &format!("/v1/workspaces/{}/commercial/gateway", ada.workspace),
        &inference_key,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = decide(&d, Some(&inference_key), Some(&ada.workspace), "acme-kev").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(jev::ApiKey::new(&ada.session_token)),
    )
    .unwrap();
    let context = sdk
        .account()
        .purchase_context(&ada.workspace, "acme-kev")
        .await
        .unwrap();
    let attribution = context.commercial.as_ref().unwrap();
    assert_eq!(receipt(&d, None).commercial.as_ref(), Some(attribution));
    assert_eq!(attribution.customer, customer);
    assert_eq!(attribution.workspace, workspace);
    assert_ne!(attribution.customer, context.account);
    assert_eq!(context.account, ada.account);
    assert_eq!(context.payer_workspace, ada.workspace);
    assert_eq!(
        sdk.account()
            .commercial_selection(&ada.account, &ada.workspace, CommercialProduct::Gateway)
            .await
            .unwrap(),
        context.commercial
    );
    let plugin_ref = sdk
        .account()
        .commercial_selection(&ada.account, &ada.workspace, CommercialProduct::Plugin)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(plugin_ref.digest, attribution.digest);
    assert_eq!(plugin_ref.source.product, CommercialProduct::Plugin);
    let (status, other_reply) = get(
        &d,
        &format!("/v1/workspaces/{}/commercial/gateway", ada.workspace),
        &bob.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!other_reply.to_string().contains(&customer));
    let (status, own) = get(
        &d,
        &format!("/v1/workspaces/{}/commercial/gateway", bob.workspace),
        &bob.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(own.is_null());
    let context_json = serde_json::to_value(&context).unwrap();
    for secret in [
        &ada.key_token,
        &ada.session_token,
        key_file.to_str().unwrap(),
        policy.to_str().unwrap(),
    ] {
        assert!(!context_json.to_string().contains(secret));
    }
    let root = tempfile::tempdir().unwrap();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let quote = Quote {
        id: "before-migration".into(),
        context: context.clone(),
        request_digest: receipts::execution::digest_request(&call("acme-kev")),
        created_at_ms: now * 1000,
        expires_at_ms: now * 1000 + 60_000,
    };
    let approval = Approval {
        quote,
        approved_at_ms: now * 1000,
    };
    let mut frozen = None;
    if let Some(binary) = &binary {
        let token_file = root.path().join("input-key");
        private(&token_file, ada.session_token.as_bytes());
        cli(
            binary,
            root.path(),
            vec![
                "import".into(),
                "--alias".into(),
                "original".into(),
                "--input".into(),
                token_file.to_str().unwrap().into(),
            ],
        )
        .await;
        cli(
            binary,
            root.path(),
            vec![
                "select".into(),
                "--origin".into(),
                d.address.clone(),
                "--alias".into(),
                "original".into(),
                "--account".into(),
                ada.account.clone(),
                "--workspace".into(),
                ada.workspace.clone(),
                "--door".into(),
                "acme-kev".into(),
            ],
        )
        .await;
        let input = root.path().join("input.json");
        private(&input, &serde_json::to_vec(&call("acme-kev")).unwrap());
        frozen = Some(
            cli(
                binary,
                root.path(),
                vec![
                    "quote".into(),
                    "--purchase".into(),
                    "before-migration".into(),
                    "--input".into(),
                    input.to_str().unwrap().into(),
                ],
            )
            .await,
        );
        let output = cli(
            binary,
            root.path(),
            vec!["commercial".into(), "--product".into(), "plugin".into()],
        )
        .await;
        assert_eq!(output["commercial"]["digest"], attribution.digest);
        assert_eq!(output["commercial"]["source"]["product"], "plugin");
    }
    let team = canonical
        .create_workspace(
            &customer,
            "Canonical team",
            WorkspaceKind::Organization,
            "canonical",
            Some(4),
        )
        .unwrap();
    let owner = canonical.authorize(&team.id, &customer).unwrap();
    assert_eq!(owner.role, Role::Owner);
    let balance_before = native_balance(&d, &ada).await;
    let forwards_before = verification.forwards.load(Ordering::SeqCst);
    verification.armed.store(true, Ordering::SeqCst);
    let pending = reqwest::Client::new()
        .post(format!("{}/v1/systemone", d.address))
        .bearer_auth(&inference_key)
        .header("x-workspace-id", &ada.workspace)
        .header("idempotency-key", "headerless-before-migration")
        .json(&call("acme-kev"));
    let pending = tokio::spawn(async move { pending.send().await.unwrap() });
    tokio::time::timeout(Duration::from_secs(3), verification.entered.notified())
        .await
        .expect("the call reached verification after retaining its first mapping");
    for e in &mut entries {
        let preceding = review
            .sources
            .iter()
            .find(|r| r.source == e.source)
            .unwrap();
        e.workspace = team.id.clone();
        e.canonical_owner_epoch = owner.epoch;
        e.canonical_members_epoch = owner.members_epoch;
        e.generation = 2;
        e.previous_authority = Some(preceding.digest());
    }
    write_policy(&entries);
    // A policy update cannot quietly relabel the previously admitted revision.
    assert!(
        sdk.account()
            .purchase_context(&ada.workspace, "acme-kev")
            .await
            .is_err()
    );
    let migrated = canonical
        .review_commercial(
            "actual-commercial",
            &customer,
            &team.id,
            &customer,
            Some(&customer),
            &[source.clone(), plugin.clone()],
            &adapter,
        )
        .unwrap();
    canonical
        .admit_commercial(&migrated, &migrated.digest, &adapter)
        .unwrap();
    verification.release.notify_one();
    let response = pending.await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert!(response.headers().get(HEADER).is_none());
    assert_eq!(
        verification.forwards.load(Ordering::SeqCst),
        forwards_before
    );
    let refused = receipt(&d, Some("headerless-before-migration"));
    assert_eq!(refused.commercial.as_ref(), Some(attribution));
    assert_eq!(refused.outcome, receipts::execution::Outcome::Refused);
    assert_eq!(refused.cause.as_deref(), Some("purchase_changed"));
    let balance_after = native_balance(&d, &ada).await;
    assert_eq!(balance_after, balance_before);
    let money_before = std::fs::read(&ledger_path).unwrap();
    let fresh = sdk
        .account()
        .purchase_context(&ada.workspace, "acme-kev")
        .await
        .unwrap();
    assert_eq!(fresh.commercial.as_ref().unwrap().workspace, team.id);
    assert_eq!(fresh.commercial.as_ref().unwrap().revision, 2);
    assert_eq!(fresh.payer_workspace, ada.workspace);
    assert!(
        approval
            .validate_current(&fresh, &approval.quote.request_digest, now * 1000)
            .is_err()
    );
    let response = reqwest::Client::new()
        .post(format!("{}/v1/systemone", d.address))
        .bearer_auth(&ada.session_token)
        .header("x-workspace-id", &ada.workspace)
        .header("idempotency-key", &approval.quote.id)
        .header(HEADER, serde_json::to_string(&approval).unwrap())
        .json(&call("acme-kev"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    if let (Some(binary), Some(frozen)) = (&binary, &frozen) {
        let output = customer_process(
            binary.clone(),
            root.path().into(),
            root.path().join("home"),
            vec![
                "approve".into(),
                "--purchase".into(),
                "before-migration".into(),
                "--digest".into(),
                frozen["quote_digest"].as_str().unwrap().into(),
            ],
        )
        .await;
        assert!(!output.status.success());
        let output = cli(
            binary,
            root.path(),
            vec![
                "show".into(),
                "--purchase".into(),
                "before-migration".into(),
            ],
        )
        .await;
        assert_eq!(&output, frozen);
    }
    assert_eq!(std::fs::read(&ledger_path).unwrap(), money_before);
    let invitation = canonical
        .invite(&customer, &team.id, Role::Admin, 3600)
        .unwrap();
    canonical.accept(&other, &invitation.token).unwrap();
    canonical
        .transfer_ownership(&customer, &team.id, &other)
        .unwrap();
    canonical
        .remove_member(&other, &team.id, &customer)
        .unwrap();
    assert!(native.authorize(&ada.workspace, &ada.account).is_ok());
    assert!(sdk.account().details().await.is_ok());
    let plugin_source = receipts::purchase::CommercialSource {
        product: CommercialProduct::Plugin,
        issuer: plugin.issuer.clone(),
        account: ada.account.clone(),
        workspace: Some(ada.workspace.clone()),
    };
    // Canonical membership revocation cannot withhold an original native outcome read.
    assert!(
        sdk.account()
            .commercial_selection(&ada.account, &ada.workspace, CommercialProduct::Plugin)
            .await
            .is_err()
    );
    let reader = sdk.account().plugin_reader(&plugin_source).await.unwrap();
    assert_eq!(reader.source, plugin_source);
    assert_eq!(reader.tenant, context.tenant);
    assert_eq!(reader.role, "owner");
    // Bypassing the buyer preflight still rechecks the canonical source before money or dispatch.
    let response = reqwest::Client::new()
        .post(format!("{}/v1/systemone", d.address))
        .bearer_auth(&ada.session_token)
        .header("x-workspace-id", &ada.workspace)
        .header("idempotency-key", "no-client-preflight")
        .json(&call("acme-kev"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let (status, _) = decide(&d, Some(&inference_key), Some(&ada.workspace), "acme-kev").await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(std::fs::read(&ledger_path).unwrap(), money_before);
    canonical
        .retire_commercial("actual-commercial", &other, &migrated.digest)
        .unwrap();
    assert_eq!(
        sdk.account().plugin_reader(&plugin_source).await.unwrap(),
        reader
    );
    assert!(
        sdk.account()
            .commercial_selection(&ada.account, &ada.workspace, CommercialProduct::Plugin)
            .await
            .unwrap()
            .is_none()
    );
    let (status, _) = decide(&d, Some(&inference_key), Some(&ada.workspace), "acme-kev").await;
    assert_eq!(status, StatusCode::CONFLICT);
    // A native Member can read its own original source until native membership is revoked.
    let native_team = native
        .create_workspace(
            &ada.account,
            "Native reader team",
            WorkspaceKind::Organization,
            &context.tenant,
            Some(4),
        )
        .unwrap();
    let invitation = native
        .invite(&ada.account, &native_team.id, Role::Member, 3600)
        .unwrap();
    native.accept(&bob.account, &invitation.token).unwrap();
    let bob_sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(jev::ApiKey::new(&bob.key_token)),
    )
    .unwrap();
    let member_source = receipts::purchase::CommercialSource {
        account: bob.account.clone(),
        workspace: Some(native_team.id.clone()),
        ..plugin_source.clone()
    };
    assert_eq!(
        bob_sdk
            .account()
            .plugin_reader(&member_source)
            .await
            .unwrap()
            .role,
        "member"
    );
    native
        .remove_member(&ada.account, &native_team.id, &bob.account)
        .unwrap();
    assert!(
        bob_sdk
            .account()
            .plugin_reader(&member_source)
            .await
            .is_err()
    );
    assert_eq!(std::fs::read(&ledger_path).unwrap(), money_before);
    let key_id = ada
        .key_token
        .strip_prefix("oak_")
        .unwrap()
        .split('.')
        .next()
        .unwrap();
    keys::revoke(d.dir.path(), key_id).unwrap();
    let revoked_sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(jev::ApiKey::new(&ada.key_token)),
    )
    .unwrap();
    assert!(
        revoked_sdk
            .account()
            .plugin_reader(&plugin_source)
            .await
            .is_err()
    );
    assert!(
        revoked_sdk
            .account()
            .purchase_context(&ada.workspace, "acme-kev")
            .await
            .is_err()
    );
    assert!(
        revoked_sdk
            .account()
            .commercial_selection(&ada.account, &ada.workspace, CommercialProduct::Plugin)
            .await
            .is_err()
    );
    assert_eq!(
        canonical.store().unwrap().commercial.bindings["actual-commercial"][0],
        review
    );
    assert_eq!(std::fs::read(&ledger_path).unwrap(), money_before);
    d.server.abort();
}

async fn cli(binary: &std::ffi::OsString, root: &Path, args: Vec<String>) -> Value {
    let output = customer_process(binary.clone(), root.into(), root.join("home"), args).await;
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}
