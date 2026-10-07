//! Attribution uses the native account owner, current credentials, and real HTTP.
use super::*;
use tenancy::accounts::referrals::attribution::{Policy, RULE};

pub(super) fn policy() -> Policy {
    Policy::new("synthetic-v1".into(), "The two synthetic parties agree to retain this relationship across credentials, personal-to-team conversion, and ownership transfer. Corrections require explicit mutual review. This fixture grants no commission.".into()).unwrap()
}
pub(super) fn proposal(policy: &Policy, referrer: &str, request: &str) -> Value {
    json!({"request":request,"policy_digest":policy.digest,"introduction":"early_agreement","referrer":referrer,"evidence":[{"reference":"private-early-agreement-1","digest":format!("sha256:{}","a".repeat(64))}],"reason":"Both parties confirm their prior agreement.","consent":true,"expected_decision":null})
}
pub(super) async fn restart(d: &mut Deployment) {
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
#[tokio::test]
async fn attribution_terms_confirmation_transfer_and_private_export_use_canonical_accounts() {
    let mut d = deploy(Some(account_config(None)), true).await;
    let source = join(&d, "Referrer").await;
    let customer = join(&d, "Introduced customer").await;
    let other = join(&d, "Unrelated customer").await;
    let p = policy();
    let store = Accounts::open(d.dir.path()).unwrap();
    store.publish_attribution_policy(&p).unwrap();
    let (_, r) = post(
        &d,
        "/v1/account/referrers",
        Some(&source.key_token),
        &json!({"kind":"person","label":"Private source label"}),
    )
    .await;
    let id = r["referral"]["id"].as_str().unwrap();
    let input = proposal(&p, id, "import-early");
    let (status, pending) = post(
        &d,
        "/v1/account/attribution",
        Some(&customer.key_token),
        &input,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{pending}");
    let reviewed = pending["referral"]["digest"].as_str().unwrap();
    assert_eq!(pending["referral"]["review"], "awaiting_confirmation");
    let sequence = store.store().unwrap().sequence;
    assert_eq!(
        post(
            &d,
            "/v1/account/attribution",
            Some(&customer.key_token),
            &input
        )
        .await
        .1,
        pending
    );
    assert_eq!(store.store().unwrap().sequence, sequence);
    let confirm = json!({"customer":customer.account,"decision":reviewed});
    let (status, denied) = post(
        &d,
        "/v1/account/attribution/confirm",
        Some(&other.key_token),
        &confirm,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(!denied.to_string().contains("private-early"));
    let (status, accepted) = post(
        &d,
        "/v1/account/attribution/confirm",
        Some(&source.key_token),
        &confirm,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{accepted}");
    assert_eq!(accepted["referral"]["commission_eligibility"], false);
    assert!(accepted["referral"].get("evidence").is_none());
    let sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(customer.key_token.as_str()),
    )
    .unwrap();
    let original = sdk
        .account()
        .for_referrals_account(&customer.account)
        .attribution()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(original.customer, customer.account);
    assert_eq!(original.decisions.len(), 2);
    assert_eq!(original.binding.as_ref().unwrap().referrer.id, id);
    let team = sdk
        .account()
        .team(&customer.account)
        .create("Converted team", 3)
        .await
        .unwrap();
    let workspace = team["id"].as_str().unwrap();
    let grant = sdk
        .account()
        .team(&customer.account)
        .invite(workspace, jev::TeamRole::Member, 60)
        .await
        .unwrap();
    let next = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(source.key_token.as_str()),
    )
    .unwrap();
    next.account()
        .team(&source.account)
        .accept(workspace, &grant.token)
        .await
        .unwrap();
    assert!(
        next.account()
            .workspace_attribution(workspace)
            .await
            .is_err()
    );
    sdk.account()
        .team(&customer.account)
        .transfer(workspace, &source.account)
        .await
        .unwrap();
    next.account()
        .team(&source.account)
        .remove(workspace, &customer.account)
        .await
        .unwrap();
    let retained = next
        .account()
        .workspace_attribution(workspace)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(retained.binding, original.binding.clone().unwrap());
    assert!(
        sdk.account()
            .workspace_attribution(workspace)
            .await
            .is_err()
    );
    let other_sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(other.key_token.as_str()),
    )
    .unwrap();
    assert_eq!(other_sdk.account().attribution().await.unwrap(), None);
    assert!(
        other_sdk
            .account()
            .workspace_attribution(workspace)
            .await
            .is_err()
    );
    restart(&mut d).await;
    assert_eq!(
        next.account()
            .workspace_attribution(workspace)
            .await
            .unwrap(),
        Some(retained)
    );
    assert_eq!(sdk.account().attribution().await.unwrap(), Some(original));
    d.server.abort();
}

pub(super) fn configure_money(d: &mut Deployment, customers: &[&Joined]) -> std::path::PathBuf {
    use tenancy::money::{Ledger, Mutation, Operation, Price, Rate, Resource};
    let path = d.dir.path().join("referral-money.jsonl");
    let mut ledger = Ledger::open(&path).unwrap();
    for customer in customers {
        ledger
            .apply(Mutation {
                workspace: customer.workspace.clone(),
                source: "create".into(),
                audit: "synthetic-referral-fixture".into(),
                operation: Operation::Create {
                    currency: "USD".into(),
                    spend_limit: 100_000,
                    topups_allowed: false,
                },
            })
            .unwrap();
    }
    drop(ledger);
    d.config.money = Some(gateway::money::Money {
        hierarchical_budgets: false,
        ledger: path.clone(),
        doors: [(
            "acme-kev".into(),
            gateway::money::Priced {
                offer: None,
                price: Price {
                    version: "fixture-1".into(),
                    currency: "USD".into(),
                    model: "kev-0.6b".into(),
                    capacity: "dedicated".into(),
                    policy: gateway::money::POLICY.into(),
                    rates: [(
                        Resource::InputTokens,
                        Rate {
                            millionths: 1,
                            per_units: 1,
                        },
                    )]
                    .into(),
                },
                maximum_usage: [(Resource::InputTokens, 1000)].into(),
            },
        )]
        .into(),
    });
    path
}
async fn referral(
    cli: &super::super::team::Installed,
    command: &str,
    input: Option<Value>,
    options: &[(&str, &str)],
) -> (bool, Value) {
    let mut words = vec!["referral".into(), command.into()];
    if let Some(input) = input {
        let file = cli.directory.path().join("referral-input.json");
        super::super::team::private_file(&file, &serde_json::to_vec(&input).unwrap());
        words.extend(["--input".into(), file.to_str().unwrap().into()]);
    }
    for (name, value) in options {
        words.extend([format!("--{name}"), (*value).into()]);
    }
    cli.run(words).await
}

#[tokio::test]
#[ignore = "requires freshly built customer and local policy binaries"]
async fn installed_attribution_keeps_two_customers_distinct_through_team_transfer_and_key_recovery()
{
    use super::super::team::{Installed, command, private_file};
    let binary =
        std::env::var_os("OPENAGENTS_REV28_TEST_CLI").expect("Set OPENAGENTS_REV28_TEST_CLI.");
    let policy_binary =
        std::env::var_os("OPENAGENTS_REV28_POLICY_CLI").expect("Set OPENAGENTS_REV28_POLICY_CLI.");
    let mut d = deploy(Some(account_config(None)), true).await;
    let source = join(&d, "Source manager").await;
    let customer = join(&d, "Private customer").await;
    let money = configure_money(&mut d, &[&source, &customer]);
    restart(&mut d).await;
    let before_money = std::fs::read(&money).unwrap();
    let a = Installed::new(&binary);
    let b = Installed::new(&binary);
    a.import("key", &source.key_token).await;
    b.import("key", &customer.key_token).await;
    a.select(&d.address, &source.account, &source.workspace, "key")
        .await;
    b.select(&d.address, &customer.account, &customer.workspace, "key")
        .await;
    let p = policy();
    let file = a.directory.path().join("terms.json");
    private_file(
        &file,
        &serde_json::to_vec(&json!({"version":p.version,"terms":p.terms})).unwrap(),
    );
    let output = std::process::Command::new(&policy_binary)
        .args(["publish", "--registry"])
        .arg(d.dir.path())
        .arg("--input")
        .arg(&file)
        .env("HOME", a.directory.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let published: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(published["digest"], p.digest);
    assert_eq!(published["rule"], RULE);
    let (_, created) = referral(
        &a,
        "create",
        Some(json!({"kind":"person","label":"Private source"})),
        &[],
    )
    .await;
    let id = created["id"].as_str().unwrap();
    let (_, link) = referral(&a, "link", None, &[("referrer", id)]).await;
    assert!(!link["url"].as_str().unwrap().contains(&customer.account));
    assert_eq!(
        referral(&b, "policy", None, &[]).await.1["digest"],
        p.digest
    );
    let mut input = proposal(&p, id, "early-one");
    input["request"] = json!(format!("import-{}", "\"".repeat(121)));
    input["reason"] = json!(format!("Review {}", "\"".repeat(505)));
    input["evidence"] = json!(
        (0..8)
            .map(|index| json!({
                "reference": format!("private-{index}-{}", "\"".repeat(118)),
                "digest": format!("sha256:{}", "a".repeat(64)),
            }))
            .collect::<Vec<_>>()
    );
    assert!(serde_json::to_vec(&input).unwrap().len() > 4096);
    let (ok, pending) = referral(&b, "propose", Some(input.clone()), &[]).await;
    assert!(ok, "{pending}");
    assert_eq!(pending["review"], "awaiting_confirmation");
    assert_eq!(referral(&b, "propose", Some(input), &[]).await.1, pending);
    let (_, confirmed) = referral(
        &a,
        "confirm",
        Some(json!({"customer":customer.account,"decision":pending["digest"]})),
        &[],
    )
    .await;
    assert_eq!(confirmed["status"], "accepted");
    assert!(confirmed.get("source").is_none());
    let (_, original) = referral(&b, "attribution", None, &[]).await;
    assert_eq!(original["binding"]["referrer"]["id"], id);
    assert_eq!(referral(&a, "attribution", None, &[]).await.1, Value::Null);
    let (_, created) = b
        .change(
            command(
                "team-create",
                &d.address,
                &customer.account,
                "key",
                json!({"kind":"create","name":"Converted team","seats":3}),
            ),
            None,
        )
        .await;
    let workspace = created["result"]["id"].as_str().unwrap();
    assert_eq!(
        referral(&b, "workspace", None, &[("workspace", workspace)])
            .await
            .1["binding"],
        original["binding"]
    );
    let (_, invitation) = b
        .change(
            command(
                "invite",
                &d.address,
                &customer.account,
                "key",
                json!({"kind":"invite","workspace":workspace,"role":"member","ttl_secs":60}),
            ),
            None,
        )
        .await;
    let invitation_file = std::path::Path::new(invitation["invitation_file"].as_str().unwrap());
    assert!(
        a.change(
            command(
                "join",
                &d.address,
                &source.account,
                "key",
                json!({"kind":"accept","workspace":workspace})
            ),
            Some(invitation_file)
        )
        .await
        .0
    );
    assert!(
        !referral(&a, "workspace", None, &[("workspace", workspace)])
            .await
            .0
    );
    assert!(
        b.change(
            command(
                "transfer",
                &d.address,
                &customer.account,
                "key",
                json!({"kind":"transfer","workspace":workspace,"account":source.account})
            ),
            None
        )
        .await
        .0
    );
    assert!(
        a.change(
            command(
                "remove-original",
                &d.address,
                &source.account,
                "key",
                json!({"kind":"remove","workspace":workspace,"account":customer.account})
            ),
            None
        )
        .await
        .0
    );
    assert!(
        !referral(&b, "workspace", None, &[("workspace", workspace)])
            .await
            .0
    );
    assert_eq!(
        referral(&a, "workspace", None, &[("workspace", workspace)])
            .await
            .1["binding"],
        original["binding"]
    );
    let (status, recovery) = post(
        &d,
        &format!("/v1/workspaces/{}/recovery", customer.workspace),
        Some(&customer.key_token),
        &json!({"account":customer.account}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{recovery}");
    let token = recovery["token"].as_str().unwrap();
    let token_file = b.directory.path().join("recovery.secret");
    private_file(&token_file, token.as_bytes());
    let intent_file = b.directory.path().join("recover.json");
    private_file(&intent_file, &serde_json::to_vec(&json!({"id":"restore-key","origin":d.address,"account":customer.account,"credential_alias":null,"action":{"kind":"recover","workspace":customer.workspace,"output_alias":"restored"}})).unwrap());
    let restored = b
        .ok(vec![
            "change".into(),
            "--input".into(),
            intent_file.to_str().unwrap().into(),
            "--recovery-token".into(),
            token_file.to_str().unwrap().into(),
        ])
        .await;
    assert_eq!(restored["status"], "applied");
    assert!(!referral(&b, "attribution", None, &[]).await.0);
    b.select(
        &d.address,
        &customer.account,
        &customer.workspace,
        "restored",
    )
    .await;
    assert_eq!(referral(&b, "attribution", None, &[]).await.1, original);
    restart(&mut d).await;
    assert_eq!(referral(&b, "attribution", None, &[]).await.1, original);
    assert_eq!(
        referral(&a, "workspace", None, &[("workspace", workspace)])
            .await
            .1["binding"],
        original["binding"]
    );
    assert_eq!(std::fs::read(&money).unwrap(), before_money);
    d.server.abort();
}
