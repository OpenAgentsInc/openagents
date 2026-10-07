use super::*;
#[path = "../../commercial-spend/tests/support/mod.rs"]
mod support;
use pay_ledger::shared::{Intent, Operation as SharedOp, Outcome as SharedOutcome};
use support::{Fixture, NativeInput};
async fn start(host: &mut Host) {
    let state = ServeState::open(host.config.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    host.address = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, receive) = oneshot::channel();
    let router = serve::router(state.clone());
    host.server = Some(tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async move {
                let _ = receive.await;
            })
            .await
            .unwrap();
    }));
    host.shutdown = Some(shutdown);
    host.state = Some(state);
}
async fn stop(host: &mut Host) {
    let stopped = Arc::downgrade(host.state.as_ref().unwrap());
    host.stop().await;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while stopped.strong_count() != 0 {
        assert!(
            std::time::Instant::now() < deadline,
            "Original request did not finish retaining its receipt before restart"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
async fn fixture() -> (Host, Fixture) {
    let mut host = Host::new(0, |_, _| {}).await;
    stop(&mut host).await;
    let registry = Registry::open(host.dir.path()).unwrap();
    let accounts = tenancy::Accounts::open(host.dir.path()).unwrap();
    let member = accounts
        .authenticate_key(registry.manifest(), &host.workspace, &host.token)
        .unwrap();
    let shared = Fixture::new(Some(NativeInput {
        directory: host.dir.path().into(),
        account: member.account,
        workspace: host.workspace.clone(),
        tenant: "buyer".into(),
        token: host.token.clone(),
        money: host.config.money.as_ref().unwrap().ledger.clone(),
    }));
    shared.fund("gateway-funding", 2);
    start(&mut host).await;
    (host, shared)
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_native_http_requires_the_actual_callers_spend_scope() {
    let mut host = Host::new(0, |_, _| {}).await;
    stop(&mut host).await;
    let registry = Registry::open(host.dir.path()).unwrap();
    let accounts = tenancy::Accounts::open(host.dir.path()).unwrap();
    let member = accounts
        .authenticate_key(registry.manifest(), &host.workspace, &host.token)
        .unwrap();
    let mut principals = accounts.store().unwrap().accounts[&member.account]
        .principals
        .clone();
    let mut issued = Vec::new();
    for actions in [vec!["inference"], vec!["inference", "shared-spend"]] {
        let key = tenancy::keys::issue_scoped(
            host.dir.path(),
            registry.manifest(),
            "buyer",
            None,
            Some(tenancy::keys::Scopes {
                models: Some([DOOR.to_owned()].into()),
                actions: Some(actions.into_iter().map(str::to_owned).collect()),
            }),
        )
        .unwrap();
        principals.push(format!("key:{}", key.key.id));
        issued.push(key.token);
    }
    support::write(
        &host.dir.path().join("keys.json"),
        &std::fs::read(host.dir.path().join("keys.json")).unwrap(),
    );
    accounts
        .update_principals(&member.account, &principals)
        .unwrap();
    let shared = Fixture::new(Some(NativeInput {
        directory: host.dir.path().into(),
        account: member.account,
        workspace: host.workspace.clone(),
        tenant: "buyer".into(),
        token: host.token.clone(),
        money: host.config.money.as_ref().unwrap().ledger.clone(),
    }));
    shared.fund("gateway-scope-funding", 2);
    start(&mut host).await;
    let book = pay_ledger::Ledger::open_read_only(&shared.ledger).unwrap();
    let before = book.compute_balance("retail").unwrap();
    let denied = send(&host.address, &issued[0], &host.workspace, "scope-no-spend").await;
    assert_eq!(denied.status(), 403);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 0);
    assert_eq!(book.compute_balance("retail").unwrap(), before);
    let admitted = send(&host.address, &issued[1], &host.workspace, "scope-spend").await;
    let status = admitted.status();
    let body: Value = admitted.json().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    assert_eq!(book.compute_balance("retail").unwrap().settled_msat, 11);
    stop(&mut host).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_native_http_charge_replay_omitted_config_and_projection_lag_preserve_original_funds()
 {
    let (mut host, shared) = fixture().await;
    let response = host.call("shared-call").await;
    let status = response.status();
    let body: Value = response.json().await.unwrap();
    assert_eq!(status, 200, "{body}");
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    let id = Intent::stable_id(&shared.gateway_binding, "shared-call#1");
    let outcome: SharedOutcome = serde_json::from_value(
        shared
            .gateway
            .call(SharedOp::Observe { id: id.clone() })
            .unwrap(),
    )
    .unwrap();
    assert_eq!(outcome.state, "settled");
    assert_eq!(outcome.hold.charge_msat, Some(11));
    assert_eq!(outcome.hold.released_msat(), Some(HOLD as i64 - 11));
    let receipt = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "shared-call")
        .unwrap();
    assert_eq!(receipt.shared_spend.as_ref().unwrap().intent, id);
    assert_eq!(
        receipt.shared_spend.as_ref().unwrap().digest,
        outcome.intent.digest()
    );
    assert_eq!(
        receipt.shared_spend.as_ref().unwrap().mode,
        shared.gateway_binding.mode()
    );
    let balance = pay_ledger::Ledger::open_read_only(&shared.ledger)
        .unwrap()
        .compute_balance("retail")
        .unwrap();
    assert_eq!(
        (
            balance.credited_msat,
            balance.available_msat,
            balance.settled_msat
        ),
        (2000, 1989, 11)
    );
    assert_eq!(host.balance().await["balance"]["available"], 0);
    assert_ne!(host.call("shared-call").await.status(), 200);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    stop(&mut host).await;
    let native = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
    let hold = native.hold(&host.workspace, "shared-call#1").unwrap();
    assert_eq!(hold.shared.as_ref().unwrap().intent, id);
    assert_eq!(hold.retail_charge, Some(11));
    assert_eq!(
        hold.shared.as_ref().unwrap().mode,
        shared.gateway_binding.mode()
    );
    drop(native);
    // A canonical append whose native projection was lost blocks another request.
    let intent = pay_ledger::shared::Intent {
        id: Intent::stable_id(&shared.gateway_binding, "missing-native#1"),
        binding: shared.gateway_binding.clone(),
        native_attempt: "missing-native#1".into(),
        quote: "exact-original-price".into(),
        execution: "missing-native#1".into(),
        terms: "sealed-original-request".into(),
        maximum_units: 1,
        fee_cap_msat: 0,
        invoice: None,
        liability: pay_ledger::shared::Liability::NativeService {
            resource: "openagents.gateway.systemone.v1".into(),
        },
        admitted_at: commercial_spend::now(),
    };
    let projection = shared
        .gateway
        .call(SharedOp::SourceOutcomes {
            after: 0,
            through: None,
        })
        .unwrap();
    shared
        .gateway
        .call(SharedOp::Reserve {
            intent,
            projection_head: projection["through"].as_u64().unwrap(),
            actor: pay_ledger::shared::GatewayActor {
                credential: host.token.clone(),
                door: DOOR.into(),
            },
        })
        .unwrap();
    start(&mut host).await;
    assert_ne!(host.call("after-lost-projection").await.status(), 200);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    stop(&mut host).await;
    // Removing the monetary configuration cannot turn the retained alias into free work.
    host.config.money = None;
    start(&mut host).await;
    assert_ne!(host.call("omitted-custody-config").await.status(), 200);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    stop(&mut host).await;
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shared_native_http_lost_response_restarts_without_releasing_unknown_or_forwarding_twice() {
    let (mut host, shared) = fixture().await;
    host.backend.block.store(true, Ordering::SeqCst);
    let address = host.address.clone();
    let token = host.token.clone();
    let workspace = host.workspace.clone();
    let call = tokio::spawn(async move { send(&address, &token, &workspace, "lost-shared").await });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        host.backend.entered.notified(),
    )
    .await
    .unwrap();
    call.abort();
    let _ = call.await;
    let id = Intent::stable_id(&shared.gateway_binding, "lost-shared#1");
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        let out: SharedOutcome = serde_json::from_value(
            shared
                .gateway
                .call(SharedOp::Observe { id: id.clone() })
                .unwrap(),
        )
        .unwrap();
        if out.state == "unknown" {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "expected retained original uncertainty, got {out:?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    host.backend.resume.notify_one();
    stop(&mut host).await;
    start(&mut host).await;
    assert_ne!(host.call("lost-shared").await.status(), 200);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    let book = pay_ledger::Ledger::open_read_only(&shared.ledger).unwrap();
    let balance = book.compute_balance("retail").unwrap();
    assert_eq!(
        (balance.available_msat, balance.held_msat),
        (2000 - HOLD as i64, HOLD as i64)
    );
    stop(&mut host).await;
}
