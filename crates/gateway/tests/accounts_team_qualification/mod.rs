//! Joined recovery, policy, budget, and export qualification uses private fixtures.
use super::*;

fn identity() -> tenancy::Expected {
    tenancy::Expected {
        model: "kev-0.6b".into(),
        adapter: None,
        artifact_signature: artifact('b'),
        execution: [
            ("backend", "cpu"),
            ("dtype", "f32"),
            ("head_dtype", "f32"),
            ("attention", "eager-block-causal-v1"),
            ("bucket_size", "0"),
            ("lora_merge", "fp32-before-cast-v1"),
            ("option_isolation", "false"),
            ("max_state", "8192"),
            ("max_branch", "8192"),
        ]
        .into_iter()
        .map(|(k, v)| (k.into(), v.into()))
        .collect(),
    }
}
async fn qualified_cards() -> axum::Json<Value> {
    let expected = identity();
    axum::Json(
        json!({"models":[{"id":expected.model,"artifact_identity":{"digest":expected.artifact_signature},"execution":expected.execution,
        "batching":{"kind":"caller-loop"},"limits":{"context_tokens":1000,"concurrent_calls":1},
        "metering":receipts::decision_metering::Metering::kev_packed_input(1000)}]}),
    )
}

async fn selected_offer(d: &mut Deployment, backend: &Arc<Backend>) {
    let registry = Registry::open(d.dir.path()).unwrap();
    let mut manifest = registry.manifest().clone();
    let binding = manifest
        .tenants
        .get_mut("acme")
        .unwrap()
        .doors
        .get_mut("acme-kev")
        .unwrap();
    binding.artifact = identity();
    binding.capacity = Some(tenancy::Capacity {
        concurrency: Some(1),
        requests_per_minute: Some(60),
        ..Default::default()
    });
    manifest.sequence += 1;
    manifest.supersedes = Some(manifest.digest.clone());
    Registry::update(d.dir.path(), manifest).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let router = axum::Router::new()
        .route("/v1/models", axum::routing::get(qualified_cards))
        .route("/v1/systemone", axum::routing::post(infer))
        .with_state(backend.clone());
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
    d.config
        .money
        .as_mut()
        .unwrap()
        .doors
        .get_mut("acme-kev")
        .unwrap()
        .offer = Some(gateway::decision_offer::SelectedOffer {
        schema: gateway::decision_offer::SCHEMA.into(),
        id: "qualified-team-native-offer".into(),
        version: "synthetic-v1".into(),
        identity: identity(),
        requests_per_minute: 60,
    });
}

async fn export(d: &Deployment, token: &str, workspace: &str) -> (StatusCode, Value) {
    let response = reqwest::Client::new()
        .get(format!(
            "{}/v1/workspaces/{workspace}/reports/export",
            d.address
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status();
    let body = response.json().await.unwrap();
    (status, body)
}

#[tokio::test]
async fn recovered_member_keeps_original_policy_budget_and_export_scope_without_restoring_revoked_access()
 {
    use std::os::unix::fs::PermissionsExt;
    let (mut d, owner, backend) = host().await;
    let mut member = join(&d, "protected-member-label").await;
    let outsider = join(&d, "protected-other-account-label").await;
    let invitation = sdk(&d, &owner)
        .account()
        .team(&owner.account)
        .invite(&owner.workspace, jev::TeamRole::Member, 60)
        .await
        .unwrap();
    sdk(&d, &member)
        .account()
        .team(&member.account)
        .accept(&owner.workspace, &invitation.token)
        .await
        .unwrap();
    assert!(
        sdk(&d, &outsider)
            .account()
            .team(&outsider.account)
            .accept(&owner.workspace, &invitation.token)
            .await
            .is_err()
    );
    member.workspace = owner.workspace.clone();
    selected_offer(&mut d, &backend).await;
    let evidence = d.dir.path().join("qualified-team-evidence");
    std::fs::create_dir(&evidence).unwrap();
    std::fs::set_permissions(&evidence, std::fs::Permissions::from_mode(0o700)).unwrap();
    d.config.team_reports = Some(gateway::team_reports::Config {
        evidence_root: evidence.canonicalize().unwrap(),
    });
    d.config.money.as_mut().unwrap().hierarchical_budgets = true;
    restart(&mut d).await;
    let limit = json!({"cap":10000,"alert_at":5000});
    let policy = json!({"schema":tenancy::money::budgets::SCHEMA,"version":1,"currency":"USD","scale":tenancy::money::budgets::SCALE,"route":tenancy::money::budgets::ROUTE,"effective_from":0,"workspace":limit,"teams":{"delivery":limit},"people":{
        (owner.account.clone()):{"team":"delivery","limit":limit},
        (member.account.clone()):{"team":"delivery","limit":limit}
    }});
    let response = reqwest::Client::new()
        .put(format!(
            "{}/v1/workspaces/{}/budgets",
            d.address, owner.workspace
        ))
        .bearer_auth(&owner.session_token)
        .json(&json!({"request":"qualified-budget","expected_policy":null,"policy":policy}))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let budget: Value = response.json().await.unwrap();
    let body = call("acme-kev");
    let reviewed = review(
        &d,
        &owner,
        None,
        1,
        vec![rule(&d, &body)],
        unix_now() + 3600,
    )
    .await;
    let registry = Registry::open(d.dir.path()).unwrap();
    let reader = keys::issue_scoped(
        d.dir.path(),
        registry.manifest(),
        "acme",
        Some("qualified-reader"),
        Some(keys::Scopes {
            models: Some(["acme-kev".into()].into()),
            actions: Some(["accounts".into()].into()),
        }),
    )
    .unwrap();
    let accounts = Accounts::open(d.dir.path()).unwrap();
    let mut principals = accounts.store().unwrap().accounts[&member.account]
        .principals
        .clone();
    principals.push(format!("key:{}", reader.key.id));
    accounts
        .update_principals(&member.account, &principals)
        .unwrap();
    let readonly = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(reader.token.clone()),
    )
    .unwrap();
    assert!(
        readonly
            .account()
            .review_team_policy(
                &member.account,
                &owner.workspace,
                &Change {
                    expected_digest: Some(reviewed.reference.digest.clone()),
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
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", d.address))
            .bearer_auth(&reader.token)
            .header("x-workspace-id", &owner.workspace)
            .header("idempotency-key", "reader-spend")
            .json(&body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        send(&d, &owner, "qualified-owner-task", &body).await.0,
        StatusCode::OK
    );
    assert_eq!(
        send(&d, &member, "qualified-member-task", &body).await.0,
        StatusCode::OK
    );
    let original = receipt(&d, "qualified-member-task");
    assert_eq!(original.member.as_ref().unwrap().account, member.account);
    assert_eq!(
        original.team_policy.as_ref().unwrap().policy.digest,
        reviewed.reference.digest
    );
    let (_, before) = export(&d, &member.session_token, &owner.workspace).await;
    assert_eq!(before["scope"], "own_original_tasks");
    assert_eq!(before["rows"].as_array().unwrap().len(), 1);
    assert_eq!(before["rows"][0]["receipt"], original.digest);
    assert_eq!(
        before["rows"][0]["original_member"]["account"],
        member.account
    );
    assert_eq!(before["rows"][0]["charged"], 3);
    assert_eq!(
        before["rows"][0]["team_policy_reference"],
        reviewed.reference.digest
    );
    assert_eq!(
        before["rows"][0]["budget_policy_reference"],
        budget["budget"]["policy"]
    );
    assert_eq!(
        export(&d, &reader.token, &owner.workspace).await.1["rows"],
        before["rows"]
    );
    for forbidden in [
        &owner.key_token,
        &member.key_token,
        &member.session_token,
        &outsider.key_token,
        "protected-member-label",
        "protected-other-account-label",
        "owner-reviewed-private",
    ] {
        assert!(!serde_json::to_string(&before).unwrap().contains(forbidden));
    }
    assert_eq!(
        export(&d, &outsider.session_token, &owner.workspace)
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, recovery) = post(
        &d,
        &format!("/v1/workspaces/{}/recovery", owner.workspace),
        Some(&owner.session_token),
        &json!({"account":member.account}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let recovery_token = recovery["token"].as_str().unwrap();
    let (status, recovered) = post(
        &d,
        "/v1/recovery/redeem",
        None,
        &json!({"token":recovery_token}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(recovered["account"], member.account);
    let fresh = recovered["key_token"].as_str().unwrap();
    assert_eq!(
        post(
            &d,
            "/v1/recovery/redeem",
            None,
            &json!({"token":recovery_token})
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        export(&d, &member.session_token, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        export(&d, &member.key_token, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        export(&d, &reader.token, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, after) = export(&d, fresh, &owner.workspace).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(after["rows"], before["rows"]);
    // Recovery replaces credentials, not the original execution actor or policy.
    assert_eq!(receipt(&d, "qualified-member-task").digest, original.digest);
    let narrowed = review(
        &d,
        &owner,
        Some(reviewed.reference.digest.clone()),
        2,
        vec![],
        unix_now() + 3600,
    )
    .await;
    assert_ne!(narrowed.reference.digest, reviewed.reference.digest);
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/systemone", d.address))
            .bearer_auth(fresh)
            .header("x-workspace-id", &owner.workspace)
            .header("idempotency-key", "after-narrowing")
            .json(&body),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(backend.bodies.lock().unwrap().len(), 2);
    let budget_read = reqwest::Client::new()
        .get(format!(
            "{}/v1/workspaces/{}/budgets?requested=1",
            d.address, owner.workspace
        ))
        .bearer_auth(fresh)
        .send()
        .await
        .unwrap();
    assert_eq!(budget_read.status(), StatusCode::OK);
    let budget_read: Value = budget_read.json().await.unwrap();
    assert_eq!(budget_read["budget"]["policy"], budget["budget"]["policy"]);
    assert_eq!(
        budget_read["budget"]["people"].as_object().unwrap().len(),
        1
    );
    let (status, pending_recovery) = post(
        &d,
        &format!("/v1/workspaces/{}/recovery", owner.workspace),
        Some(&owner.session_token),
        &json!({"account":member.account}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);
    let (status, _) = remove(
        &d,
        &format!(
            "/v1/workspaces/{}/members/{}",
            owner.workspace, member.account
        ),
        &owner.session_token,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, revoked_recovery) = post(
        &d,
        "/v1/recovery/redeem",
        None,
        &json!({"token":pending_recovery["token"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked_recovery["account"], member.account);
    let revoked_key = revoked_recovery["key_token"].as_str().unwrap();
    assert_eq!(get(&d, "/v1/account", revoked_key).await.0, StatusCode::OK);
    assert_eq!(
        export(&d, revoked_key, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        export(&d, fresh, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        export(&d, &member.session_token, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    restart(&mut d).await;
    assert_eq!(
        export(&d, revoked_key, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        export(&d, fresh, &owner.workspace).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, owner_export) = export(&d, &owner.session_token, &owner.workspace).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(owner_export["rows"].as_array().unwrap().len(), 2);
    assert!(
        owner_export["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["receipt"] == original.digest)
    );
    assert_eq!(backend.bodies.lock().unwrap().len(), 2);
    d.server.abort();
}
