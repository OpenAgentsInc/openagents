//! Actual native HTTP and installed-client policy gates use isolated books.
use super::*;
use receipts::team_policy::{Change, PlacementKind, Rule, Terms, digest};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
struct Backend {
    cards_blocked: AtomicBool,
    response_blocked: AtomicBool,
    entered: Notify,
    resume: Notify,
    response_entered: Notify,
    response_resume: Notify,
    bodies: std::sync::Mutex<Vec<String>>,
}
async fn cards(axum::extract::State(b): axum::extract::State<Arc<Backend>>) -> axum::Json<Value> {
    if b.cards_blocked.load(Ordering::SeqCst) {
        b.entered.notify_one();
        b.resume.notified().await;
    }
    axum::Json(
        json!({"models":[{"id":"kev-0.6b","artifact_identity":{"digest":artifact('b')},"execution":{}}]}),
    )
}
async fn infer(
    axum::extract::State(b): axum::extract::State<Arc<Backend>>,
    body: axum::body::Bytes,
) -> axum::Json<Value> {
    b.bodies
        .lock()
        .unwrap()
        .push(String::from_utf8(body.to_vec()).unwrap());
    b.response_entered.notify_one();
    if b.response_blocked.load(Ordering::SeqCst) {
        b.response_resume.notified().await;
    }
    axum::Json(
        json!({"model":"kev-0.6b","answers":{"q1":{"type":"noul","noul":0.75}},"usage":{"input_tokens":3,"output_tokens":0}}),
    )
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
async fn host() -> (Deployment, Joined, Arc<Backend>) {
    let mut d = deploy(Some(account_config(None)), true).await;
    let mut owner = join(&d, "Policy owner").await;
    owner.workspace = Accounts::open(d.dir.path())
        .unwrap()
        .create_workspace(
            &owner.account,
            "Policy team",
            tenancy::accounts::WorkspaceKind::Organization,
            "acme",
            Some(8),
        )
        .unwrap()
        .id;
    let b = Arc::new(Backend {
        cards_blocked: AtomicBool::new(false),
        response_blocked: AtomicBool::new(false),
        entered: Notify::new(),
        resume: Notify::new(),
        response_entered: Notify::new(),
        response_resume: Notify::new(),
        bodies: std::sync::Mutex::new(vec![]),
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new()
        .route("/v1/models", axum::routing::get(cards))
        .route("/v1/systemone", axum::routing::post(infer))
        .with_state(b.clone());
    tokio::spawn(axum::serve(listener, router).into_future());
    d.config.doors.get_mut("acme-kev").unwrap().endpoint = endpoint.clone();
    d.config.team_policy = Some(gateway::team_policy::Config {
        doors: [(
            "acme-kev".into(),
            gateway::team_policy::Backend {
                endpoint,
                placement: PlacementKind::LocalGateway,
            },
        )]
        .into(),
    });
    let path = d.dir.path().join("policy-money.jsonl");
    let mut ledger = tenancy::money::Ledger::open(&path).unwrap();
    for (source, operation) in [
        (
            "create",
            tenancy::money::Operation::Create {
                currency: "USD".into(),
                spend_limit: 10000,
                topups_allowed: false,
            },
        ),
        (
            "synthetic-credit",
            tenancy::money::Operation::Credit {
                amount: 10000,
                credit_kind: tenancy::money::CreditKind::Grant,
            },
        ),
    ] {
        ledger
            .apply(tenancy::money::Mutation {
                workspace: owner.workspace.clone(),
                source: source.into(),
                audit: "isolated policy acceptance".into(),
                operation,
            })
            .unwrap();
    }
    drop(ledger);
    d.config.money=Some(serde_json::from_value(json!({"ledger":path,"doors":{"acme-kev":{"price":{"version":"policy-fixture-1","currency":"USD","model":"kev-0.6b","capacity":"dedicated","policy":gateway::money::POLICY,"rates":{"input-tokens":{"millionths":1,"per_units":1}}},"maximum_usage":{"input-tokens":1000}}}})).unwrap());
    restart(&mut d).await;
    (d, owner, b)
}
fn sdk(d: &Deployment, u: &Joined) -> jev::Client {
    jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(u.session_token.clone()),
    )
    .unwrap()
}
fn rule(d: &Deployment, request: &Value) -> Rule {
    let registry = Registry::open(d.dir.path()).unwrap();
    let admission = registry.authorize(Some("acme"), "acme-kev").unwrap();
    Rule {
        effect: gateway::team_policy::effect(&d.config, &admission, "acme-kev", request).unwrap(),
        data_classes: vec!["owner-reviewed-private".into()],
    }
}
async fn review(
    d: &Deployment,
    u: &Joined,
    previous: Option<String>,
    version: u64,
    rules: Vec<Rule>,
    expiry: u64,
) -> jev::TeamPolicyView {
    sdk(d, u)
        .account()
        .review_team_policy(
            &u.account,
            &u.workspace,
            &Change {
                expected_digest: previous,
                terms: Terms {
                    version,
                    expires_unix: expiry,
                    rules,
                },
            },
        )
        .await
        .unwrap()
}
async fn send(d: &Deployment, u: &Joined, id: &str, body: &Value) -> (StatusCode, Value) {
    exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", d.address))
            .bearer_auth(&u.session_token)
            .header("x-workspace-id", &u.workspace)
            .header("idempotency-key", id)
            .json(body),
    )
    .await
}
fn receipt(d: &Deployment, id: &str) -> receipts::ExecutionReceipt {
    std::fs::read_to_string(d.dir.path().join("receipts.jsonl"))
        .unwrap()
        .lines()
        .map(|l| receipts::ExecutionReceipt::parse(l).unwrap())
        .find(|r| r.request == id && r.team_policy.is_some())
        .unwrap()
}
#[tokio::test]
async fn exact_provenance_denies_every_substituted_effect_and_canonicalizes_disclosed_bytes() {
    let (d, u, b) = host().await;
    let body = call("acme-kev");
    let allowed = rule(&d, &body);
    let mut previous = None;
    for field in 0..8 {
        let mut altered = allowed.clone();
        match field {
            0 => altered.effect.release = digest(&"substituted-release"),
            1 => altered.effect.model = Some("forbidden-model".into()),
            2 => altered.effect.recipients = vec![digest(&"foreign-provider")],
            3 => altered.effect.source.request = digest(&"different-source"),
            4 => altered.effect.source.material = digest(&"different-material"),
            5 => altered.effect.placement.kind = PlacementKind::CloudGateway,
            6 => altered.effect.placement.identity = digest(&"customer-host"),
            _ => {
                altered.effect.capability = receipts::team_policy::Capability::Plugin;
                altered.effect.model = None;
                altered.effect.plugin = Some(receipts::team_policy::PluginPin {
                    publisher: "a".repeat(64),
                    release: digest(&"plugin-release"),
                    module: digest(&"wasm"),
                });
            }
        }
        let v = review(
            &d,
            &u,
            previous,
            field + 1,
            vec![altered],
            unix_now() + 3600,
        )
        .await;
        previous = Some(v.reference.digest);
        let (status, refused) = send(&d, &u, &format!("substitution-{field}"), &body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{refused}");
        assert_eq!(code(&refused), "team_policy_denied");
        assert!(b.bodies.lock().unwrap().is_empty());
    }
    let v = review(&d, &u, previous, 9, vec![allowed], unix_now() + 3600).await;
    let injected = json!({"model":"acme-kev","state":"private context injection","questions":body["questions"]});
    assert_eq!(
        send(&d, &u, "private-injection", &injected).await.0,
        StatusCode::FORBIDDEN
    );
    let raw = format!(
        "{{\"state\":\"unreviewed-shadowed-secret\",{}",
        serde_json::to_string(&body)
            .unwrap()
            .trim_start_matches('{')
    );
    let response = reqwest::Client::new()
        .post(format!("{}/v1/systemone", d.address))
        .bearer_auth(&u.session_token)
        .header("x-workspace-id", &u.workspace)
        .header("idempotency-key", "canonical")
        .header("content-type", "application/json")
        .body(raw)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    assert!(!b.bodies.lock().unwrap()[0].contains("unreviewed-shadowed-secret"));
    let original = receipt(&d, "canonical");
    assert_eq!(
        original.team_policy.as_ref().unwrap().policy.digest,
        v.reference.digest
    );
    assert_eq!(
        send(&d, &u, "canonical", &body).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    for path in ["/v1/classify", "/v1/jobs"] {
        let (status,_)=post(&d,path,Some(&u.session_token),&json!({"model":"acme-kev","capacity":"dedicated","inputs":[{"id":"a","state":"unreviewed"}],"questions":{"q":{"type":"noul","instructions":"x"}}})).await;
        assert!(!status.is_success());
        assert_eq!(b.bodies.lock().unwrap().len(), 1);
    }
}
#[tokio::test]
async fn changed_policy_during_model_verification_releases_only_undispatched_new_work() {
    let (d, u, b) = host().await;
    let body = call("acme-kev");
    let original = review(&d, &u, None, 1, vec![rule(&d, &body)], unix_now() + 3600).await;
    b.cards_blocked.store(true, Ordering::SeqCst);
    let address = d.address.clone();
    let token = u.session_token.clone();
    let workspace = u.workspace.clone();
    let copy = body.clone();
    let task = tokio::spawn(async move {
        exchange(
            reqwest::Client::new()
                .post(format!("{address}/v1/systemone"))
                .bearer_auth(token)
                .header("x-workspace-id", workspace)
                .header("idempotency-key", "raced")
                .json(&copy),
        )
        .await
    });
    b.entered.notified().await;
    let next = review(
        &d,
        &u,
        Some(original.reference.digest.clone()),
        2,
        vec![],
        unix_now() + 3600,
    )
    .await;
    b.resume.notify_one();
    let (status, value) = task.await.unwrap();
    assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
    assert!(b.bodies.lock().unwrap().is_empty());
    let r = receipt(&d, "raced");
    assert_eq!(
        r.team_policy.unwrap().policy.digest,
        original.reference.digest
    );
    assert_ne!(next.reference.digest, original.reference.digest);
    let ledger = d._state.as_ref().unwrap();
    let _ = ledger;
    // Native outcome reads retain the original admission after policy revocation.
    let recovered = sdk(&d, &u)
        .account()
        .purchase_receipt(&u.workspace, &r.digest)
        .await
        .unwrap();
    assert_eq!(
        recovered
            .receipt
            .team_policy
            .as_ref()
            .unwrap()
            .policy
            .version,
        1
    );
}
#[tokio::test]
async fn current_credentials_expiry_and_unknown_reconnect_refuse_new_dispatch() {
    let (mut d, u, b) = host().await;
    let body = call("acme-kev");
    let original = review(&d, &u, None, 1, vec![rule(&d, &body)], unix_now() + 3600).await;
    b.response_blocked.store(true, Ordering::SeqCst);
    let address = d.address.clone();
    let token = u.session_token.clone();
    let workspace = u.workspace.clone();
    let copy = body.clone();
    let mut task = tokio::spawn(async move {
        reqwest::Client::new()
            .post(format!("{address}/v1/systemone"))
            .bearer_auth(token)
            .header("x-workspace-id", workspace)
            .header("idempotency-key", "lost")
            .timeout(std::time::Duration::from_secs(20))
            .json(&copy)
            .send()
            .await
    });
    tokio::select! {
        entered = tokio::time::timeout(std::time::Duration::from_secs(10), b.response_entered.notified()) => {
            entered.expect("The reviewed backend must receive the request before disconnection.");
        }
        response = &mut task => panic!("The request ended before backend handoff: {response:?}"),
    }
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    review(
        &d,
        &u,
        Some(original.reference.digest),
        2,
        vec![],
        unix_now() + 3600,
    )
    .await;
    b.response_resume.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let recorded = std::fs::read_to_string(d.dir.path().join("receipts.jsonl"))
                .unwrap_or_default()
                .lines()
                .filter_map(|line| receipts::ExecutionReceipt::parse(line).ok())
                .any(|receipt| receipt.request == "lost" && receipt.team_policy.is_some());
            if recorded {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("The disconnected native attempt must retain its original receipt.");
    restart(&mut d).await;
    assert_eq!(send(&d, &u, "lost", &body).await.0, StatusCode::FORBIDDEN);
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    let r = receipt(&d, "lost");
    assert_eq!(r.team_policy.unwrap().policy.version, 1);
    let v = sdk(&d, &u)
        .account()
        .team_policy(&u.account, &u.workspace)
        .await
        .unwrap();
    let expiry = unix_now() + 1;
    review(
        &d,
        &u,
        Some(v.reference.digest),
        3,
        vec![rule(&d, &body)],
        expiry,
    )
    .await;
    while unix_now() < expiry {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }
    assert_eq!(
        send(&d, &u, "expired", &body).await.0,
        StatusCode::FORBIDDEN
    );
    let now = sdk(&d, &u)
        .account()
        .team_policy(&u.account, &u.workspace)
        .await
        .unwrap();
    review(
        &d,
        &u,
        Some(now.reference.digest),
        4,
        vec![rule(&d, &body)],
        unix_now() + 3600,
    )
    .await;
    let registry = Registry::open(d.dir.path()).unwrap();
    let key = keys::authenticate(d.dir.path(), registry.manifest(), &u.key_token).unwrap();
    keys::revoke(d.dir.path(), &key.key_id).unwrap();
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", d.address))
            .bearer_auth(&u.key_token)
            .header("x-workspace-id", &u.workspace)
            .json(&body),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn revoked_members_and_read_only_keys_cannot_dispatch_or_review() {
    let (d, u, b) = host().await;
    let mut colleague = join(&d, "Member").await;
    let a = Accounts::open(d.dir.path()).unwrap();
    let invite = a
        .invite(&u.account, &u.workspace, tenancy::Role::Member, 3600)
        .unwrap();
    a.accept_into(&colleague.account, &invite.token, &u.workspace)
        .unwrap();
    colleague.workspace = u.workspace.clone();
    let body = call("acme-kev");
    let policy = review(&d, &u, None, 1, vec![rule(&d, &body)], unix_now() + 3600).await;
    let read = sdk(&d, &colleague)
        .account()
        .team_policy(&colleague.account, &colleague.workspace)
        .await
        .unwrap();
    assert!(read.reviewed.is_none());
    assert!(
        sdk(&d, &colleague)
            .account()
            .review_team_policy(
                &colleague.account,
                &colleague.workspace,
                &Change {
                    expected_digest: Some(policy.reference.digest),
                    terms: Terms {
                        version: 2,
                        expires_unix: unix_now() + 3600,
                        rules: vec![]
                    }
                }
            )
            .await
            .is_err()
    );
    let registry = Registry::open(d.dir.path()).unwrap();
    let readonly = keys::issue_scoped(
        d.dir.path(),
        registry.manifest(),
        "acme",
        Some("native-readonly"),
        Some(keys::Scopes {
            models: Some(["acme-kev".into()].into()),
            actions: Some(["accounts".into(), "models".into()].into()),
        }),
    )
    .unwrap();
    let old = a.store().unwrap().accounts[&colleague.account].clone();
    let mut principals = old.principals;
    principals.push(format!("key:{}", readonly.key.id));
    a.update_principals(&colleague.account, &principals)
        .unwrap();
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", d.address))
            .bearer_auth(readonly.token)
            .header("x-workspace-id", &u.workspace)
            .json(&body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    b.cards_blocked.store(true, Ordering::SeqCst);
    let address = d.address.clone();
    let token = colleague.session_token.clone();
    let workspace = u.workspace.clone();
    let copy = body.clone();
    let pending = tokio::spawn(async move {
        exchange(
            reqwest::Client::new()
                .post(format!("{address}/v1/systemone"))
                .bearer_auth(token)
                .header("x-workspace-id", workspace)
                .header("idempotency-key", "member-race")
                .json(&copy),
        )
        .await
    });
    b.entered.notified().await;
    a.remove_member(&u.account, &u.workspace, &colleague.account)
        .unwrap();
    b.resume.notify_one();
    assert_eq!(pending.await.unwrap().0, StatusCode::FORBIDDEN);
    assert!(b.bodies.lock().unwrap().is_empty());
}
#[tokio::test]
async fn qualified_loopback_routes_ignore_ambient_proxy() {
    if std::env::var_os("OPENAGENTS_REV41_PROXY_CHILD").is_some() {
        let (mut d, u, b) = host().await;
        let listener = tokio::net::TcpListener::bind("[::1]:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let router = axum::Router::new()
            .route("/v1/models", axum::routing::get(cards))
            .route("/v1/systemone", axum::routing::post(infer))
            .with_state(b.clone());
        tokio::spawn(axum::serve(listener, router).into_future());
        d.config.doors.get_mut("acme-kev").unwrap().endpoint = endpoint.clone();
        d.config
            .team_policy
            .as_mut()
            .unwrap()
            .doors
            .get_mut("acme-kev")
            .unwrap()
            .endpoint = endpoint;
        restart(&mut d).await;
        let body = call("acme-kev");
        review(&d, &u, None, 1, vec![rule(&d, &body)], unix_now() + 3600).await;
        let (status, response) = send(&d, &u, "direct-local-recipient", &body).await;
        assert_eq!(status, StatusCode::OK, "{response}");
        assert_eq!(b.bodies.lock().unwrap().len(), 1);
        return;
    }
    // The caller's IPv4 API bypasses the proxy; its reviewed IPv6 recipient does not.
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = seen.clone();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new().fallback(move || {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            StatusCode::BAD_GATEWAY
        }
    });
    let server = tokio::spawn(axum::serve(listener, router).into_future());
    let home = tempfile::tempdir().unwrap();
    let output = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        tokio::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "team_policy::qualified_loopback_routes_ignore_ambient_proxy",
                "--nocapture",
            ])
            .env("OPENAGENTS_REV41_PROXY_CHILD", "1")
            .env("HTTP_PROXY", &proxy)
            .env("http_proxy", &proxy)
            .env("HTTPS_PROXY", &proxy)
            .env("https_proxy", &proxy)
            .env("ALL_PROXY", &proxy)
            .env("all_proxy", &proxy)
            .env("NO_PROXY", "127.0.0.1")
            .env("no_proxy", "127.0.0.1")
            .env("HOME", home.path())
            .current_dir(home.path())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .unwrap()
    .unwrap();
    server.abort();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        seen.load(Ordering::SeqCst),
        0,
        "Reviewed input reached an ambient proxy."
    );
}

#[tokio::test]
async fn reviewed_post_redirect_cannot_disclose_to_another_recipient_or_replay() {
    let (mut d, u, b) = host().await;
    let seen = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = seen.clone();
    let target = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let location = format!("http://{}/v1/systemone", target.local_addr().unwrap());
    let sink = axum::Router::new().fallback(move || {
        let count = count.clone();
        async move {
            count.fetch_add(1, Ordering::SeqCst);
            StatusCode::OK
        }
    });
    let sink_server = tokio::spawn(axum::serve(target, sink).into_future());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let received = b.clone();
    let router = axum::Router::new()
        .route("/v1/models", axum::routing::get(cards))
        .route(
            "/v1/systemone",
            axum::routing::post(move |body: axum::body::Bytes| {
                let received = received.clone();
                let location = location.clone();
                async move {
                    received
                        .bodies
                        .lock()
                        .unwrap()
                        .push(String::from_utf8(body.to_vec()).unwrap());
                    (
                        StatusCode::TEMPORARY_REDIRECT,
                        [(axum::http::header::LOCATION, location)],
                    )
                }
            }),
        )
        .with_state(b.clone());
    let redirect_server = tokio::spawn(axum::serve(listener, router).into_future());
    d.config.doors.get_mut("acme-kev").unwrap().endpoint = endpoint.clone();
    d.config
        .team_policy
        .as_mut()
        .unwrap()
        .doors
        .get_mut("acme-kev")
        .unwrap()
        .endpoint = endpoint;
    restart(&mut d).await;
    let body = call("acme-kev");
    review(&d, &u, None, 1, vec![rule(&d, &body)], unix_now() + 3600).await;
    assert_eq!(
        send(&d, &u, "redirected-original", &body).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        receipt(&d, "redirected-original")
            .team_policy
            .unwrap()
            .policy
            .version,
        1
    );
    assert_eq!(
        send(&d, &u, "redirected-original", &body).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    assert_eq!(seen.load(Ordering::SeqCst), 0);
    redirect_server.abort();
    sink_server.abort();
}

fn private(path: &std::path::Path, bytes: &[u8]) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
async fn cli(
    binary: &std::ffi::OsStr,
    root: &std::path::Path,
    home: &std::path::Path,
    args: Vec<String>,
) -> (bool, Value) {
    let result = customer_process(binary.to_owned(), root.to_owned(), home.to_owned(), args).await;
    let out = String::from_utf8(result.stdout).unwrap();
    assert!(
        !out.contains("sess_") && !out.contains("oak_"),
        "Credential appeared in selected CLI output"
    );
    let value = serde_json::from_str(&out)
        .unwrap_or_else(|_| json!({"stderr":String::from_utf8_lossy(&result.stderr).to_string()}));
    (result.status.success(), value)
}
#[tokio::test]
#[ignore = "requires the freshly built immutable installed CLI"]
async fn installed_cli_reviews_exact_policy_refuses_changed_and_plugin_routes_and_reads_original_receipt()
 {
    let binary = std::env::var_os("OPENAGENTS_REV41_TEST_CLI")
        .expect("Set OPENAGENTS_REV41_TEST_CLI to a fresh immutable openagents binary.");
    let (d, u, b) = host().await;
    let home = tempfile::tempdir().unwrap();
    let root = home.path().join("customer");
    let key = home.path().join("session.key");
    private(&key, u.session_token.as_bytes());
    assert!(
        cli(
            &binary,
            &root,
            home.path(),
            vec![
                "import".into(),
                "--alias".into(),
                "owner".into(),
                "--input".into(),
                key.to_str().unwrap().into()
            ]
        )
        .await
        .0
    );
    assert!(
        cli(
            &binary,
            &root,
            home.path(),
            vec![
                "select".into(),
                "--origin".into(),
                d.address.clone(),
                "--alias".into(),
                "owner".into(),
                "--account".into(),
                u.account.clone(),
                "--workspace".into(),
                u.workspace.clone(),
                "--door".into(),
                "acme-kev".into()
            ]
        )
        .await
        .0
    );
    let body = call("acme-kev");
    let intent = home.path().join("policy.json");
    private(
        &intent,
        &serde_json::to_vec(&Change {
            expected_digest: None,
            terms: Terms {
                version: 1,
                expires_unix: unix_now() + 3600,
                rules: vec![rule(&d, &body)],
            },
        })
        .unwrap(),
    );
    let (ok, policy) = cli(
        &binary,
        &root,
        home.path(),
        vec![
            "policy".into(),
            "review".into(),
            "--input".into(),
            intent.to_str().unwrap().into(),
        ],
    )
    .await;
    assert!(ok, "{policy}");
    assert_eq!(policy["reference"]["version"], 1);
    let input = home.path().join("input.json");
    private(&input, &serde_json::to_vec(&body).unwrap());
    let (ok, quoted) = cli(
        &binary,
        &root,
        home.path(),
        vec![
            "quote".into(),
            "--purchase".into(),
            "original".into(),
            "--input".into(),
            input.to_str().unwrap().into(),
        ],
    )
    .await;
    assert!(ok, "{quoted}");
    assert_eq!(quoted["quote"]["context"]["team_policy"]["version"], 1);
    let digest = quoted["quote_digest"]
        .as_str()
        .unwrap_or_else(|| panic!("Missing quote digest {quoted}"));
    assert!(
        cli(
            &binary,
            &root,
            home.path(),
            vec![
                "approve".into(),
                "--purchase".into(),
                "original".into(),
                "--digest".into(),
                digest.into()
            ]
        )
        .await
        .0
    );
    let (ok, pending) = cli(
        &binary,
        &root,
        home.path(),
        vec![
            "quote".into(),
            "--purchase".into(),
            "changed".into(),
            "--input".into(),
            input.to_str().unwrap().into(),
        ],
    )
    .await;
    assert!(ok, "{pending}");
    let frozen = pending["quote_digest"].as_str().unwrap();
    b.response_blocked.store(true, Ordering::SeqCst);
    let mut process = tokio::process::Command::new(&binary)
        .args([
            "--json",
            "customer",
            "invoke",
            "--purchase",
            "original",
            "--root",
        ])
        .arg(&root)
        .env("HOME", home.path())
        .env_remove("OPENAGENTS_API_KEY")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    tokio::time::timeout(
        std::time::Duration::from_secs(10),
        b.response_entered.notified(),
    )
    .await
    .expect("The approved original request must reach its reviewed recipient.");
    process.kill().await.unwrap();
    assert!(!process.wait_with_output().await.unwrap().status.success());
    b.response_blocked.store(false, Ordering::SeqCst);
    b.response_resume.notify_one();
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            let raw = std::fs::read_to_string(d.dir.path().join("receipts.jsonl")).unwrap();
            if raw.lines().any(|l| {
                receipts::ExecutionReceipt::parse(l)
                    .is_ok_and(|r| r.request == "original" && r.team_policy.is_some())
            }) {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
    })
    .await
    .expect("The original native receipt must survive the killed client.");
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    private(
        &intent,
        &serde_json::to_vec(&Change {
            expected_digest: Some(policy["reference"]["digest"].as_str().unwrap().into()),
            terms: Terms {
                version: 2,
                expires_unix: unix_now() + 3600,
                rules: vec![],
            },
        })
        .unwrap(),
    );
    assert!(
        cli(
            &binary,
            &root,
            home.path(),
            vec![
                "policy".into(),
                "review".into(),
                "--input".into(),
                intent.to_str().unwrap().into()
            ]
        )
        .await
        .0
    );
    let (ok, history) = cli(
        &binary,
        &root,
        home.path(),
        vec!["show".into(), "--purchase".into(), "original".into()],
    )
    .await;
    assert!(ok, "{history}");
    assert_eq!(history["quote"]["context"]["team_policy"]["version"], 1);
    let (ok, recovered) = cli(
        &binary,
        &root,
        home.path(),
        vec!["reconcile".into(), "--purchase".into(), "original".into()],
    )
    .await;
    assert!(ok, "{recovered}");
    assert_eq!(
        recovered["receipt"]["digest"],
        receipt(&d, "original").digest
    );
    assert_eq!(recovered["quote"]["context"]["team_policy"]["version"], 1);
    assert!(
        !cli(
            &binary,
            &root,
            home.path(),
            vec![
                "approve".into(),
                "--purchase".into(),
                "changed".into(),
                "--digest".into(),
                frozen.into()
            ]
        )
        .await
        .0
    );
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
    let count = Accounts::open(d.dir.path())
        .unwrap()
        .store()
        .unwrap()
        .sequence;
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(binary)
            .args(["--json", "plugin", "purchase", "quote", "--root"])
            .arg(root)
            .args([
                "--purchase",
                "unsupported",
                "--plugin",
                "arbitrary",
                "--input",
            ])
            .arg(input)
            .args([
                "--wallet-home",
                "/unavailable-wallet",
                "--max-msat",
                "100",
                "--max-fee-msat",
                "0",
            ])
            .env("HOME", home.path())
            .output()
            .unwrap()
    })
    .await
    .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("team policy"));
    assert_eq!(
        Accounts::open(d.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .sequence,
        count
    );
    assert_eq!(b.bodies.lock().unwrap().len(), 1);
}
