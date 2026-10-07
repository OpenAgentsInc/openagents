use super::*;
use axum::http::StatusCode;
use tenancy::money::budgets::{Limit, Person, Policy, ROUTE, SCALE, SCHEMA as BUDGET_SCHEMA};

const SECOND: &str = "buyer-team-kev";
struct Member {
    account: String,
    token: String,
}

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

async fn team_host() -> (Host, String, Member) {
    let mut host = Host::new(10_000, |_, _| {}).await;
    host.stop().await;
    let registry = Registry::open(host.dir.path()).unwrap();
    let owner = tenancy::Accounts::open(host.dir.path())
        .unwrap()
        .account_of_principal(&format!(
            "key:{}",
            keys::authenticate(host.dir.path(), registry.manifest(), &host.token)
                .unwrap()
                .key_id
        ))
        .unwrap()
        .unwrap();
    let mut manifest = registry.manifest().clone();
    let binding = manifest.tenants["buyer"].doors[DOOR].clone();
    manifest
        .tenants
        .get_mut("buyer")
        .unwrap()
        .doors
        .insert(SECOND.into(), binding);
    manifest.sequence += 1;
    manifest.supersedes = Some(manifest.digest.clone());
    let registry = Registry::update(host.dir.path(), manifest).unwrap();
    let issued = keys::issue(host.dir.path(), registry.manifest(), "buyer").unwrap();
    let accounts = tenancy::Accounts::open(host.dir.path()).unwrap();
    let member = accounts
        .create_account("Synthetic colleague", &[format!("key:{}", issued.key.id)])
        .unwrap();
    host.workspace = accounts
        .create_workspace(
            &owner,
            "Synthetic budget team",
            tenancy::WorkspaceKind::Organization,
            "buyer",
            Some(8),
        )
        .unwrap()
        .id;
    let invite = accounts
        .invite(&owner, &host.workspace, tenancy::Role::Member, 3_600)
        .unwrap();
    accounts.accept(&member.id, &invite.token).unwrap();
    {
        let mut ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
        for (source, operation) in [
            (
                "create",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: 10_000,
                    topups_allowed: false,
                },
            ),
            (
                "grant",
                Operation::Credit {
                    amount: 10_000,
                    credit_kind: CreditKind::Grant,
                },
            ),
        ] {
            ledger
                .apply(Mutation {
                    workspace: host.workspace.clone(),
                    source: format!("synthetic-team:{source}"),
                    audit: "Synthetic qualification only".into(),
                    operation,
                })
                .unwrap();
        }
    }
    host.config
        .doors
        .insert(SECOND.into(), host.config.doors[DOOR].clone());
    let money = host.config.money.as_mut().unwrap();
    money.hierarchical_budgets = true;
    money.doors.insert(SECOND.into(), priced());
    start(&mut host).await;
    (
        host,
        owner,
        Member {
            account: member.id,
            token: issued.token,
        },
    )
}

fn policy(owner: &str, member: &str, workspace: u64, team: u64, person: u64) -> Policy {
    let limit = |cap| Limit {
        cap,
        alert_at: cap / 2,
    };
    Policy {
        schema: BUDGET_SCHEMA.into(),
        version: 1,
        currency: "USD".into(),
        scale: SCALE,
        route: ROUTE.into(),
        effective_from: 0,
        workspace: limit(workspace),
        teams: [("delivery".into(), limit(team))].into(),
        people: [owner, member]
            .into_iter()
            .map(|account| {
                (
                    account.into(),
                    Person {
                        team: "delivery".into(),
                        limit: limit(person),
                    },
                )
            })
            .collect(),
    }
}

async fn publish(
    host: &Host,
    token: &str,
    id: &str,
    expected: Option<&str>,
    policy: &Policy,
) -> reqwest::Response {
    reqwest::Client::new()
        .put(format!(
            "{}/v1/workspaces/{}/budgets",
            host.address, host.workspace
        ))
        .bearer_auth(token)
        .json(&json!({"request":id,"expected_policy":expected,"policy":policy}))
        .send()
        .await
        .unwrap()
}
async fn read(host: &Host, token: &str, requested: u64) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!(
            "{}/v1/workspaces/{}/budgets?requested={requested}",
            host.address, host.workspace
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
}
async fn send_door(
    address: &str,
    token: &str,
    workspace: &str,
    id: &str,
    door: &str,
) -> reqwest::Response {
    let mut body = request();
    body["model"] = json!(door);
    reqwest::Client::new()
        .post(format!("{address}/v1/systemone"))
        .bearer_auth(token)
        .header("x-workspace-id", workspace)
        .header("idempotency-key", id)
        .header("x-attempt", "1")
        .json(&body)
        .send()
        .await
        .unwrap()
}

#[tokio::test]
async fn native_budget_concurrent_members_enforce_one_team_bound_keep_unknown_and_private_alerts() {
    let (mut host, owner, member) = team_host().await;
    let absent = host.call("missing-policy").await;
    assert_eq!(absent.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        absent.json::<Value>().await.unwrap()["error"]["code"],
        "budget_unavailable"
    );
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 0);
    for route in ["/v1/classify", "/v1/jobs"] {
        let disabled = reqwest::Client::new()
            .post(format!("{}{route}", host.address))
            .bearer_auth(&host.token)
            .header("x-workspace-id", &host.workspace)
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(disabled.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(
            disabled.json::<Value>().await.unwrap()["error"]["code"],
            "budget_route_disabled"
        );
    }
    let p = policy(&owner, &member.account, 5_000, 600, 1_000);
    let installed: Value = publish(&host, &host.token, "policy1", None, &p)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(installed["budget"]["scope"], "workspace");
    assert!(!installed["ledger_head"].as_str().unwrap().is_empty());
    let policy_digest = installed["budget"]["policy"].as_str().unwrap().to_string();
    let head = installed["ledger_head"].clone();
    let replay: Value = publish(&host, &host.token, "policy1", None, &p)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(replay["ledger_head"], head);
    assert_eq!(
        publish(&host, &member.token, "raise", Some(&policy_digest), &p)
            .await
            .status(),
        StatusCode::CONFLICT
    );
    host.backend.block.store(true, Ordering::SeqCst);
    host.backend
        .reply
        .lock()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("usage");
    let mut a = {
        let address = host.address.clone();
        let token = host.token.clone();
        let workspace = host.workspace.clone();
        tokio::spawn(async move {
            send_door(&address, &token, &workspace, "owner-parallel", DOOR).await
        })
    };
    let mut b = {
        let address = host.address.clone();
        let token = member.token.clone();
        let workspace = host.workspace.clone();
        tokio::spawn(async move {
            send_door(&address, &token, &workspace, "member-parallel", SECOND).await
        })
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        host.backend.entered.notified(),
    )
    .await
    .unwrap();
    let (remaining, refused) =
        tokio::select! { r = &mut a => (b, r.unwrap()), r = &mut b => (a, r.unwrap()) };
    assert_eq!(refused.status(), StatusCode::TOO_MANY_REQUESTS);
    let refused: Value = refused.json().await.unwrap();
    assert_eq!(refused["error"]["code"], "budget_exhausted");
    assert_eq!(refused["budget_alert"]["bound"]["bound"]["level"], "team");
    assert_eq!(refused["budget_alert"]["bound"]["bound"]["remaining"], 152);
    assert!(
        !refused["budget_alert"]["ledger_head"]
            .as_str()
            .unwrap()
            .is_empty()
    );
    let alert: Value = read(&host, &member.token, HOLD).await.json().await.unwrap();
    assert_eq!(alert["budget"]["teams"]["delivery"]["reserved"], HOLD);
    assert_eq!(alert["budget"]["blocked"]["bound"]["level"], "team");
    assert_eq!(alert["budget"]["people"].as_object().unwrap().len(), 1);
    assert!(alert["budget"]["people"].get(&owner).is_none());
    assert!(alert["policy_document"].is_null());
    host.backend.resume.notify_one();
    let answered = remaining.await.unwrap();
    assert_eq!(answered.headers()["x-settlement"], "outstanding");
    let _: Value = answered.json().await.unwrap();
    let alert: Value = read(&host, &member.token, HOLD).await.json().await.unwrap();
    assert_eq!(alert["budget"]["teams"]["delivery"]["unknown"], HOLD);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    let receipts = host.receipts();
    let original = receipts
        .iter()
        .find(|r| r.outcome == receipts::execution::Outcome::Answered)
        .unwrap()
        .request
        .clone();
    let replay = host.call(&original).await;
    assert!(replay.status().is_client_error());
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    host.stop().await;
    start(&mut host).await;
    let restarted: Value = read(&host, &member.token, HOLD).await.json().await.unwrap();
    assert_eq!(restarted["budget"]["teams"]["delivery"]["unknown"], HOLD);
    assert_eq!(restarted["budget"]["blocked"]["bound"]["remaining"], 152);
}

#[tokio::test]
async fn native_budget_lowered_version_and_current_membership_do_not_relabel_original_hold() {
    let (mut host, owner, member) = team_host().await;
    let first = policy(&owner, &member.account, 5_000, 5_000, 1_000);
    let installed: Value = publish(&host, &host.token, "policy1", None, &first)
        .await
        .json()
        .await
        .unwrap();
    let original_digest = installed["budget"]["policy"].as_str().unwrap().to_string();
    host.backend
        .reply
        .lock()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("usage");
    let response = send_door(
        &host.address,
        &member.token,
        &host.workspace,
        "member-original",
        SECOND,
    )
    .await;
    assert_eq!(response.headers()["x-settlement"], "outstanding");
    let _: Value = response.json().await.unwrap();
    let mut lowered = first.clone();
    lowered.version = 2;
    lowered.people.get_mut(&member.account).unwrap().limit = Limit {
        cap: 100,
        alert_at: 50,
    };
    let updated = publish(
        &host,
        &host.token,
        "lowered",
        Some(&original_digest),
        &lowered,
    )
    .await;
    assert_eq!(updated.status(), StatusCode::OK);
    let updated: Value = updated.json().await.unwrap();
    let person = &updated["budget"]["people"][&member.account];
    assert_eq!(person["used"], HOLD);
    assert_eq!(person["remaining"], 0);
    assert_eq!(person["alert"], "exceeded");
    let response = send_door(
        &host.address,
        &member.token,
        &host.workspace,
        "after-lowering",
        SECOND,
    )
    .await;
    assert_eq!(
        response.json::<Value>().await.unwrap()["error"]["code"],
        "budget_exhausted"
    );
    assert_eq!(
        publish(
            &host,
            &host.token,
            "stale-owner",
            Some(&original_digest),
            &first
        )
        .await
        .status(),
        StatusCode::CONFLICT
    );
    tenancy::Accounts::open(host.dir.path())
        .unwrap()
        .remove_member(&owner, &host.workspace, &member.account)
        .unwrap();
    assert_eq!(
        read(&host, &member.token, HOLD).await.status(),
        StatusCode::FORBIDDEN
    );
    let response = send_door(
        &host.address,
        &member.token,
        &host.workspace,
        "removed",
        SECOND,
    )
    .await;
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    host.stop().await;
    let ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
    let hold = ledger.hold(&host.workspace, "member-original#1").unwrap();
    let admission = hold.budget.as_ref().unwrap();
    assert_eq!(admission.person, member.account);
    assert_eq!(admission.team, "delivery");
    assert_eq!(admission.policy, original_digest);
    assert_eq!(hold.phase, tenancy::money::Phase::Unknown);
}

#[tokio::test]
async fn activating_native_budget_retains_preexisting_unknown_workspace_liability() {
    let (mut host, owner, member) = team_host().await;
    host.stop().await;
    host.config.money.as_mut().unwrap().hierarchical_budgets = false;
    start(&mut host).await;
    host.backend
        .reply
        .lock()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("usage");
    let legacy = host.call("before-activation").await;
    assert_eq!(legacy.headers()["x-settlement"], "outstanding");
    let _: Value = legacy.json().await.unwrap();
    host.stop().await;
    host.config.money.as_mut().unwrap().hierarchical_budgets = true;
    start(&mut host).await;
    let p = policy(&owner, &member.account, 600, 600, 600);
    let installed: Value = publish(&host, &host.token, "activate-with-liability", None, &p)
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(installed["budget"]["unattributed_used"], HOLD);
    assert_eq!(installed["budget"]["workspace"]["unknown"], HOLD);
    assert_eq!(
        installed["budget"]["people"][&member.account]["unknown"],
        HOLD
    );
    let blocked = send_door(
        &host.address,
        &member.token,
        &host.workspace,
        "after-activation",
        SECOND,
    )
    .await;
    assert_eq!(
        blocked.json::<Value>().await.unwrap()["error"]["code"],
        "budget_exhausted"
    );
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    host.stop().await;
    let ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
    assert!(
        ledger
            .hold(&host.workspace, "before-activation#1")
            .unwrap()
            .budget
            .is_none()
    );
    assert_eq!(ledger.holds(&host.workspace).len(), 1);
}

struct Child(std::process::Child);
impl Drop for Child {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

async fn process(host: &Host) -> (Child, String) {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let listen = listener.local_addr().unwrap().to_string();
    drop(listener);
    let config = host.dir.path().join("process-gateway.json");
    let money = host.config.money.as_ref().unwrap();
    std::fs::write(&config, serde_json::to_vec(&json!({"v":gateway::config::SCHEMA,"listen":listen,"registry":host.dir.path(),"require_workspace_membership":true,"accounts":{},"money":{"hierarchical_budgets":true,"ledger":money.ledger,"doors":money.doors},"doors":host.config.doors.iter().map(|(name, door)| (name.clone(), json!({"endpoint":door.endpoint}))).collect::<BTreeMap<_,_>>(),"forward_timeout_ms":10_000})).unwrap()).unwrap();
    let stderr = tempfile::NamedTempFile::new_in(host.dir.path()).unwrap();
    let mut child = Child(
        std::process::Command::new(env!("CARGO_BIN_EXE_gateway"))
            .args(["--config", config.to_str().unwrap()])
            .env_clear()
            .env("HOME", host.dir.path())
            .current_dir(host.dir.path())
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(stderr.reopen().unwrap())
            .spawn()
            .unwrap(),
    );
    let address = format!("http://{listen}");
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(status) = child.0.try_wait().unwrap() {
                panic!(
                    "Isolated gateway exited before readiness ({status}): {}",
                    std::fs::read_to_string(stderr.path()).unwrap_or_default()
                );
            }
            if reqwest::get(format!("{address}/healthz"))
                .await
                .is_ok_and(|r| r.status().is_success())
            {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    (child, address)
}

#[tokio::test]
async fn killed_native_gateway_keeps_dispatch_liability_and_cannot_retry_or_overspend_after_restart()
 {
    let (mut host, owner, member) = team_host().await;
    let p = policy(&owner, &member.account, 600, 600, 600);
    assert_eq!(
        publish(&host, &host.token, "policy1", None, &p)
            .await
            .status(),
        StatusCode::OK
    );
    host.stop().await;
    host.backend.block.store(true, Ordering::SeqCst);
    let (mut child, address) = process(&host).await;
    let call = {
        let address = address.clone();
        let token = member.token.clone();
        let workspace = host.workspace.clone();
        tokio::spawn(async move {
            reqwest::Client::new()
                .post(format!("{address}/v1/systemone"))
                .bearer_auth(token)
                .header("x-workspace-id", workspace)
                .header("idempotency-key", "crashed-dispatch")
                .header("x-attempt", "1")
                .json(&request())
                .send()
                .await
        })
    };
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        host.backend.entered.notified(),
    )
    .await
    .unwrap();
    let writer_pid = child.0.id();
    child.0.kill().unwrap();
    child.0.wait().unwrap();
    assert!(call.await.unwrap().is_err());
    let money_path = &host.config.money.as_ref().unwrap().ledger;
    let quota_path = host.dir.path().join("quota-ledger.jsonl");
    let money_before = std::fs::read_to_string(money_path).unwrap();
    let quota_before = std::fs::read_to_string(&quota_path).unwrap();
    let original: Vec<Mutation> = money_before
        .lines()
        .map(|line| serde_json::from_str::<Value>(line).unwrap())
        .map(|entry| serde_json::from_value(entry["mutation"].clone()).unwrap())
        .filter(|mutation: &Mutation| {
            mutation.workspace == host.workspace
                && matches!(&mutation.operation, Operation::ReserveScoped { attempt, .. } if attempt == "crashed-dispatch#1")
        })
        .collect();
    assert_eq!(original.len(), 1);
    // The native quota journal uses a persistent PID marker. Its documented
    // recovery requires confirming the old writer is gone. Only this exact
    // waited child is eligible; neither journal nor monetary lock is removed.
    let marker = host.dir.path().join("quota-ledger.lock");
    assert_eq!(
        std::fs::read_to_string(&marker).unwrap(),
        format!("pid {writer_pid}\n")
    );
    std::fs::remove_file(marker).unwrap();
    assert_eq!(std::fs::read_to_string(money_path).unwrap(), money_before);
    assert_eq!(std::fs::read_to_string(&quota_path).unwrap(), quota_before);
    host.backend.resume.notify_one();
    let (restarted, address) = process(&host).await;
    let alert: Value = reqwest::Client::new()
        .get(format!(
            "{address}/v1/workspaces/{}/budgets?requested={HOLD}",
            host.workspace
        ))
        .bearer_auth(&member.token)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(alert["budget"]["workspace"]["unknown"], HOLD);
    assert_eq!(alert["budget"]["workspace"]["remaining"], 152);
    assert_eq!(alert["budget"]["people"][&member.account]["unknown"], HOLD);
    assert_eq!(alert["budget"]["teams"]["delivery"]["unknown"], HOLD);
    assert_eq!(alert["budget"]["policy"], p.digest().unwrap());
    let money_recovered = std::fs::read_to_string(money_path).unwrap();
    let retry = send_door(
        &address,
        &member.token,
        &host.workspace,
        "crashed-dispatch",
        DOOR,
    )
    .await;
    assert_eq!(
        retry.json::<Value>().await.unwrap()["error"]["code"],
        "idempotency_conflict"
    );
    let new = send_door(
        &address,
        &host.token,
        &host.workspace,
        "new-after-crash",
        SECOND,
    )
    .await;
    assert_eq!(
        new.json::<Value>().await.unwrap()["error"]["code"],
        "budget_exhausted"
    );
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    assert_eq!(
        std::fs::read_to_string(money_path).unwrap(),
        money_recovered
    );
    assert!(
        std::fs::read_to_string(&quota_path)
            .unwrap()
            .starts_with(quota_before.as_str())
    );
    drop(restarted);
    let ledger = Ledger::open(money_path).unwrap();
    assert_eq!(ledger.holds(&host.workspace).len(), 1);
    let hold = ledger.hold(&host.workspace, "crashed-dispatch#1").unwrap();
    let Operation::ReserveScoped {
        budget,
        price,
        request_digest,
        maximum_usage,
        ..
    } = &original[0].operation
    else {
        unreachable!()
    };
    assert_eq!(hold.budget.as_ref(), Some(budget));
    assert_eq!(&hold.price, price);
    assert_eq!(&hold.request_digest, request_digest);
    assert_eq!(&hold.maximum_usage, maximum_usage);
    assert_eq!(hold.phase, tenancy::money::Phase::Unknown);
    assert_eq!(hold.reserved, HOLD);
    assert_eq!(ledger.budget_policy(&host.workspace).unwrap(), &p);
}

#[tokio::test]
async fn exact_team_policy_and_budget_share_native_admission_without_releasing_old_liability() {
    use receipts::team_policy::{Change, PlacementKind, Rule, Terms};
    let (mut host, owner, member) = team_host().await;
    host.stop().await;
    host.config.team_policy = Some(gateway::team_policy::Config {
        doors: [DOOR, SECOND]
            .into_iter()
            .map(|door| {
                (
                    door.into(),
                    gateway::team_policy::Backend {
                        endpoint: host.config.doors[door].endpoint.clone(),
                        placement: PlacementKind::LocalGateway,
                    },
                )
            })
            .collect(),
    });
    start(&mut host).await;
    let budget = policy(&owner, &member.account, 5_000, 4_000, 3_000);
    assert_eq!(
        publish(&host, &host.token, "combined-budget", None, &budget)
            .await
            .status(),
        StatusCode::OK
    );
    let registry = Registry::open(host.dir.path()).unwrap();
    let rules = [DOOR, SECOND]
        .into_iter()
        .map(|door| {
            let mut body = request();
            body["model"] = json!(door);
            let admission = registry.authorize(Some("buyer"), door).unwrap();
            Rule {
                effect: gateway::team_policy::effect(&host.config, &admission, door, &body)
                    .unwrap(),
                data_classes: vec!["isolated-owner-reviewed-input".into()],
            }
        })
        .collect();
    let review = |change: Change| {
        reqwest::Client::new()
            .put(format!(
                "{}/v1/workspaces/{}/team-policy",
                host.address, host.workspace
            ))
            .bearer_auth(&host.token)
            .json(&change)
            .send()
    };
    let first = review(Change {
        expected_digest: None,
        terms: Terms {
            version: 1,
            expires_unix: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_secs()
                + 60,
            rules,
        },
    })
    .await
    .unwrap();
    assert_eq!(first.status(), StatusCode::OK);
    let original_policy: Value = first.json().await.unwrap();
    let response = host.call("combined-once").await;
    assert_eq!(response.status(), StatusCode::OK);
    let _: Value = response.json().await.unwrap();
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 1);
    assert_eq!(
        host.backend.requests.lock().unwrap()[0]["model"],
        "kev-0.6b"
    );
    let original = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "combined-once")
        .unwrap();
    assert_eq!(original.team_policy.as_ref().unwrap().policy.version, 1);
    host.backend
        .reply
        .lock()
        .unwrap()
        .as_object_mut()
        .unwrap()
        .remove("usage");
    assert_eq!(host.call("combined-unknown").await.status(), StatusCode::OK);
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 2);
    host.backend.reply.lock().unwrap()["usage"] = json!({"input_tokens":3,"output_tokens":99});
    let before: Value = read(&host, &host.token, 0).await.json().await.unwrap();
    assert_eq!(before["budget"]["workspace"]["unknown"], HOLD);
    assert_eq!(before["budget"]["workspace"]["settled_net"], 11);
    let used = before["budget"]["workspace"]["used"].clone();
    host.backend.card_block.store(true, Ordering::SeqCst);
    let address = host.address.clone();
    let token = member.token.clone();
    let workspace = host.workspace.clone();
    let waiting = tokio::spawn(async move {
        send_door(
            &address,
            &token,
            &workspace,
            "combined-undispatched",
            SECOND,
        )
        .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(5),
        host.backend.card_entered.notified(),
    )
    .await
    .unwrap();
    let during: Value = read(&host, &host.token, 0).await.json().await.unwrap();
    assert_eq!(during["budget"]["workspace"]["reserved"], HOLD);
    let revoked = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        review(Change {
            expected_digest: Some(
                original_policy["reference"]["digest"]
                    .as_str()
                    .unwrap()
                    .into(),
            ),
            terms: Terms {
                version: 2,
                expires_unix: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_secs()
                    + 60,
                rules: vec![],
            },
        }),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        revoked.status(),
        StatusCode::OK,
        "Policy review must not wait on Money or backend response."
    );
    tenancy::Accounts::open(host.dir.path())
        .unwrap()
        .remove_member(&owner, &host.workspace, &member.account)
        .unwrap();
    host.backend.card_block.store(false, Ordering::SeqCst);
    host.backend.card_resume.notify_one();
    let denied = waiting.await.unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    assert_eq!(
        denied.json::<Value>().await.unwrap()["error"]["code"],
        "team_policy_denied"
    );
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 2);
    let after: Value = read(&host, &host.token, 0).await.json().await.unwrap();
    assert_eq!(after["budget"]["workspace"]["used"], used);
    assert_eq!(after["budget"]["workspace"]["reserved"], 0);
    assert_eq!(after["budget"]["workspace"]["unknown"], HOLD);
    assert_eq!(after["budget"]["workspace"]["settled_net"], 11);
    let head = after["ledger_head"].clone();
    for id in ["combined-once", "combined-unknown"] {
        assert_eq!(host.call(id).await.status(), StatusCode::FORBIDDEN);
    }
    assert_eq!(host.backend.forwards.load(Ordering::SeqCst), 2);
    let retry: Value = read(&host, &host.token, 0).await.json().await.unwrap();
    assert_eq!(retry["ledger_head"], head);
    assert_eq!(
        host.receipts()
            .into_iter()
            .find(|r| r.digest == original.digest)
            .unwrap()
            .team_policy,
        original.team_policy
    );
    host.stop().await;
    let ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
    assert_eq!(
        ledger
            .hold(&host.workspace, "combined-once#1")
            .unwrap()
            .phase,
        tenancy::money::Phase::Settled
    );
    assert_eq!(
        ledger
            .hold(&host.workspace, "combined-unknown#1")
            .unwrap()
            .phase,
        tenancy::money::Phase::Unknown
    );
    assert_eq!(
        ledger
            .hold(&host.workspace, "combined-undispatched#1")
            .unwrap()
            .phase,
        tenancy::money::Phase::Released
    );
    assert_eq!(
        ledger
            .hold(&host.workspace, "combined-unknown#1")
            .unwrap()
            .budget
            .as_ref()
            .unwrap()
            .policy,
        budget.digest().unwrap()
    );
}
