//! Native membership and installed two-customer acceptance use isolated services.
use super::*;

#[tokio::test]
async fn team_accept_pins_workspace_and_current_account_before_changing_membership() {
    let d = deploy(Some(account_config(None)), true).await;
    let ada = join(&d, "Champion").await;
    let bob = join(&d, "Colleague").await;
    let sdk = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(ada.session_token.clone()),
    )
    .unwrap();
    let team = sdk
        .account()
        .team(&ada.account)
        .create("Team", 3)
        .await
        .unwrap();
    let workspace = team["id"].as_str().unwrap();
    let grant = sdk
        .account()
        .team(&ada.account)
        .invite(workspace, jev::TeamRole::Member, 60)
        .await
        .unwrap();
    let store = Accounts::open(d.dir.path()).unwrap();
    let sequence = store.store().unwrap().sequence;
    for role in [None, Some("admin")] {
        let mut body = json!({"workspace":workspace,"token":grant.token.expose()});
        if let Some(role) = role {
            body["role"] = json!(role);
        }
        let (status, _) = post(
            &d,
            "/v1/invitations/accept-reviewed",
            Some(&bob.session_token),
            &body,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
    }
    let (status, _) = exchange(
        reqwest::Client::new()
            .post(format!("{}/v1/invitations/accept", d.address))
            .bearer_auth(&bob.session_token)
            .header("x-openagents-team-account", &ada.account)
            .json(&json!({"workspace":workspace,"token":grant.token.expose()})),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    let (status, _) = post(
        &d,
        "/v1/invitations/accept",
        Some(&bob.session_token),
        &json!({"workspace":bob.workspace,"token":grant.token.expose()}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(store.store().unwrap().sequence, sequence);
    assert!(store.authorize(workspace, &bob.account).is_err());
    let buyer = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(bob.session_token.clone()),
    )
    .unwrap();
    let accepted = buyer
        .account()
        .team(&bob.account)
        .accept(workspace, &grant.token)
        .await
        .unwrap();
    assert_eq!(accepted.account, bob.account);
    assert!(
        buyer
            .account()
            .team(&bob.account)
            .accept(workspace, &grant.token)
            .await
            .is_err()
    );
    assert!(
        buyer
            .account()
            .team(&bob.account)
            .role(workspace, &ada.account, jev::TeamRole::Member)
            .await
            .is_err()
    );
    assert!(
        sdk.account()
            .team(&ada.account)
            .remove(workspace, &ada.account)
            .await
            .is_err()
    );
    sdk.account()
        .team(&ada.account)
        .role(workspace, &bob.account, jev::TeamRole::Admin)
        .await
        .unwrap();
    assert_eq!(
        buyer
            .account()
            .team(&bob.account)
            .members(workspace)
            .await
            .unwrap()
            .role,
        "admin"
    );
    sdk.account()
        .team(&ada.account)
        .role(workspace, &bob.account, jev::TeamRole::Member)
        .await
        .unwrap();
    let pending = sdk
        .account()
        .team(&ada.account)
        .invite(workspace, jev::TeamRole::Member, 60)
        .await
        .unwrap();
    sdk.account()
        .team(&ada.account)
        .withdraw(workspace, &pending.invitation.id)
        .await
        .unwrap();
    let carole = join(&d, "Other invitee").await;
    let other = jev::Client::new(
        jev::Config::new()
            .base_url(&d.address)
            .api_key(carole.session_token),
    )
    .unwrap();
    assert!(
        other
            .account()
            .team(&carole.account)
            .accept(workspace, &pending.token)
            .await
            .is_err()
    );
    let expiring = sdk
        .account()
        .team(&ada.account)
        .invite(workspace, jev::TeamRole::Member, 1)
        .await
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
    assert!(
        other
            .account()
            .team(&carole.account)
            .accept(workspace, &expiring.token)
            .await
            .is_err()
    );
    sdk.account()
        .team(&ada.account)
        .remove(workspace, &bob.account)
        .await
        .unwrap();
    assert!(
        buyer
            .account()
            .team(&bob.account)
            .members(workspace)
            .await
            .is_err()
    );
    d.server.abort();
}

fn private_file(path: &std::path::Path, bytes: &[u8]) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
fn command(id: &str, origin: &str, account: &str, alias: &str, action: Value) -> Value {
    json!({"id":id,"origin":origin,"account":account,"credential_alias":alias,"action":action})
}
struct Installed {
    binary: std::ffi::OsString,
    directory: tempfile::TempDir,
}
impl Installed {
    fn new(binary: &std::ffi::OsStr) -> Self {
        Self {
            binary: binary.to_owned(),
            directory: tempfile::tempdir().unwrap(),
        }
    }
    fn root(&self) -> std::path::PathBuf {
        self.directory.path().join("customer")
    }
    async fn run(&self, args: Vec<String>) -> (bool, Value) {
        let output = customer_process(
            self.binary.clone(),
            self.root(),
            self.directory.path().to_owned(),
            args,
        )
        .await;
        let text = String::from_utf8(output.stdout).unwrap();
        assert!(
            !text.contains("sess_")
                && !text.contains("oak_")
                && !text.split('"').any(|value| {
                    value.starts_with("inv_")
                        && value.split_once('.').is_some_and(|(_, secret)| {
                            secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit())
                        })
                }),
            "Secret material appeared in CLI output"
        );
        let value = serde_json::from_str(&text).unwrap_or_else(
            |_| json!({"stderr":String::from_utf8_lossy(&output.stderr).to_string()}),
        );
        (output.status.success(), value)
    }
    async fn ok(&self, args: Vec<String>) -> Value {
        let (ok, value) = self.run(args).await;
        assert!(ok, "{value}");
        value
    }
    async fn import(&self, alias: &str, token: &str) {
        let path = self.directory.path().join(format!("{alias}.key"));
        private_file(&path, token.as_bytes());
        self.ok(vec![
            "import".into(),
            "--alias".into(),
            alias.into(),
            "--input".into(),
            path.to_str().unwrap().into(),
        ])
        .await;
    }
    async fn change(&self, value: Value, invitation: Option<&std::path::Path>) -> (bool, Value) {
        let path = self.directory.path().join("intent.json");
        private_file(&path, &serde_json::to_vec(&value).unwrap());
        let mut args = vec![
            "team".into(),
            "change".into(),
            "--input".into(),
            path.to_str().unwrap().into(),
        ];
        if let Some(file) = invitation {
            args.extend(["--invitation".into(), file.to_str().unwrap().into()]);
        }
        self.run(args).await
    }
    async fn select(&self, origin: &str, account: &str, workspace: &str, alias: &str) {
        self.ok(vec![
            "select".into(),
            "--origin".into(),
            origin.into(),
            "--alias".into(),
            alias.into(),
            "--account".into(),
            account.into(),
            "--workspace".into(),
            workspace.into(),
            "--door".into(),
            "acme-kev".into(),
        ])
        .await;
    }
}

#[tokio::test]
#[ignore = "requires a freshly built installed customer CLI"]
async fn installed_team_flow_joins_once_switches_payers_and_recovers_only_current_rights() {
    use tenancy::money::{CreditKind, Ledger, Mutation, Operation, Price, Rate, Resource};
    let binary = std::env::var_os("OPENAGENTS_REV38_TEST_CLI")
        .expect("Set OPENAGENTS_REV38_TEST_CLI to the freshly built openagents binary.");
    let mut d = deploy(Some(account_config(None)), true).await;
    let ada = join(&d, "Champion").await;
    let bob = join(&d, "Colleague").await;
    let champion = Installed::new(&binary);
    let colleague = Installed::new(&binary);
    champion.import("session", &ada.session_token).await;
    colleague.import("session", &bob.session_token).await;
    let create = command(
        "create-team",
        &d.address,
        &ada.account,
        "session",
        json!({"kind":"create","name":"Team","seats":3}),
    );
    let (ok, created) = champion.change(create, None).await;
    assert!(ok, "{created}");
    let workspace = created["result"]["id"].as_str().unwrap().to_owned();
    let ledger_path = d.dir.path().join("team-money.jsonl");
    let mut ledger = Ledger::open(&ledger_path).unwrap();
    for workspace in [&ada.workspace, &bob.workspace, &workspace] {
        for (source, operation) in [
            (
                "create",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: 100_000,
                    topups_allowed: false,
                },
            ),
            (
                "synthetic-grant",
                Operation::Credit {
                    amount: 100_000,
                    credit_kind: CreditKind::Grant,
                },
            ),
        ] {
            ledger
                .apply(Mutation {
                    workspace: workspace.clone(),
                    source: source.into(),
                    audit: "synthetic-team-acceptance".into(),
                    operation,
                })
                .unwrap();
        }
    }
    drop(ledger);
    d.config.money = Some(gateway::money::Money {
        ledger: ledger_path.clone(),
        doors: [(
            "acme-kev".into(),
            gateway::money::Priced {
                offer: None,
                price: Price {
                    version: "team-fixture-1".into(),
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
    d.server.abort();
    let _ = (&mut d.server).await;
    d._state.take();
    let state = ServeState::open(d.config.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind(d.address.strip_prefix("http://").unwrap())
        .await
        .unwrap();
    d.server = tokio::spawn(axum::serve(listener, serve::router(state.clone())).into_future());
    d._state = Some(state);
    champion
        .select(&d.address, &ada.account, &ada.workspace, "session")
        .await;
    colleague
        .select(&d.address, &bob.account, &bob.workspace, "session")
        .await;
    let (ok, withdrawn) = champion
        .change(
            command(
                "invite-withdrawn",
                &d.address,
                &ada.account,
                "session",
                json!({"kind":"invite","workspace":workspace,"role":"member","ttl_secs":60}),
            ),
            None,
        )
        .await;
    assert!(ok, "{withdrawn}");
    let file = std::path::PathBuf::from(withdrawn["invitation_file"].as_str().unwrap());
    assert!(
        champion
            .change(
                command(
                    "withdraw-invitation",
                    &d.address,
                    &ada.account,
                    "session",
                    json!({"kind":"withdraw","workspace":workspace,"invitation":withdrawn["result"]["id"]}),
                ),
                None,
            )
            .await
            .0
    );
    let (ok, refused) = colleague
        .change(
            command(
                "accept-withdrawn",
                &d.address,
                &bob.account,
                "session",
                json!({"kind":"accept","workspace":workspace}),
            ),
            Some(&file),
        )
        .await;
    assert!(!ok);
    assert_eq!(refused["status"], "refused");
    let input = champion.directory.path().join("decision.json");
    private_file(&input, &serde_json::to_vec(&call("acme-kev")).unwrap());
    let personal = champion
        .ok(vec![
            "quote".into(),
            "--purchase".into(),
            "personal-before-team".into(),
            "--input".into(),
            input.to_str().unwrap().into(),
        ])
        .await;
    let invitation = command(
        "invite-one",
        &d.address,
        &ada.account,
        "session",
        json!({"kind":"invite","workspace":workspace,"role":"member","ttl_secs":60}),
    );
    let (ok, issued) = champion.change(invitation.clone(), None).await;
    assert!(ok, "{issued}");
    let file = std::path::PathBuf::from(issued["invitation_file"].as_str().unwrap());
    let bytes = std::fs::read(&file).unwrap();
    let secret: Value = serde_json::from_slice(&bytes).unwrap();
    assert!(secret["token"].as_str().unwrap().starts_with("inv_"));
    let before = Accounts::open(d.dir.path()).unwrap().store().unwrap();
    let sequence_before = before.sequence;
    let epoch_before = before.workspaces[&workspace].members_epoch;
    let money_before = std::fs::read(&ledger_path).unwrap();
    for (id, target, role) in [
        ("accept-mislabeled-role", workspace.as_str(), "admin"),
        (
            "accept-mislabeled-workspace",
            bob.workspace.as_str(),
            "member",
        ),
    ] {
        let mut mislabeled = secret.clone();
        mislabeled["invitation"]["workspace"] = json!(target);
        mislabeled["invitation"]["role"] = json!(role);
        let bad_file = colleague.directory.path().join("mislabeled.json");
        private_file(&bad_file, &serde_json::to_vec(&mislabeled).unwrap());
        let (ok, denied) = colleague
            .change(
                command(
                    id,
                    &d.address,
                    &bob.account,
                    "session",
                    json!({"kind":"accept","workspace":target}),
                ),
                Some(&bad_file),
            )
            .await;
        assert!(!ok, "{denied}");
        assert_eq!(denied["status"], "refused");
        let accounts = Accounts::open(d.dir.path()).unwrap();
        let after = accounts.store().unwrap();
        assert_eq!(after.sequence, sequence_before);
        assert_eq!(after.workspaces[&workspace].members_epoch, epoch_before);
        assert!(accounts.authorize(&workspace, &bob.account).is_err());
        assert_eq!(
            serde_json::to_value(&after).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert_eq!(std::fs::read(&ledger_path).unwrap(), money_before);
        assert_eq!(std::fs::read(&file).unwrap(), bytes);
    }
    let accept = command(
        "accept-one",
        &d.address,
        &bob.account,
        "session",
        json!({"kind":"accept","workspace":workspace}),
    );
    let (ok, accepted) = colleague.change(accept.clone(), Some(&file)).await;
    assert!(ok, "{accepted}");
    let sequence = Accounts::open(d.dir.path())
        .unwrap()
        .store()
        .unwrap()
        .sequence;
    assert!(colleague.change(accept, Some(&file)).await.0);
    assert_eq!(
        Accounts::open(d.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .sequence,
        sequence
    );
    colleague
        .ok(vec![
            "team".into(),
            "switch".into(),
            "--workspace".into(),
            workspace.clone(),
            "--door".into(),
            "acme-kev".into(),
        ])
        .await;
    let members = colleague
        .ok(vec![
            "team".into(),
            "members".into(),
            "--workspace".into(),
            workspace.clone(),
        ])
        .await;
    assert_eq!(members["team"]["role"], "member");
    assert_eq!(members["purchase_context"]["payer_workspace"], workspace);
    assert!(!members.to_string().contains(ada.key_token.as_str()));
    let bad = command(
        "unauthorized-role",
        &d.address,
        &bob.account,
        "session",
        json!({"kind":"role","workspace":workspace,"account":ada.account,"role":"member"}),
    );
    let (ok, denied) = colleague.change(bad, None).await;
    assert!(!ok);
    assert_eq!(denied["status"], "refused");
    let input = colleague.directory.path().join("decision.json");
    private_file(&input, &serde_json::to_vec(&call("acme-kev")).unwrap());
    let quote = colleague
        .ok(vec![
            "quote".into(),
            "--purchase".into(),
            "team-task".into(),
            "--input".into(),
            input.to_str().unwrap().into(),
        ])
        .await;
    assert_eq!(quote["quote"]["context"]["payer_workspace"], workspace);
    colleague
        .ok(vec![
            "approve".into(),
            "--purchase".into(),
            "team-task".into(),
            "--digest".into(),
            quote["quote_digest"].as_str().unwrap().into(),
        ])
        .await;
    let result = colleague
        .ok(vec![
            "invoke".into(),
            "--purchase".into(),
            "team-task".into(),
        ])
        .await;
    assert_eq!(result["purchase"]["receipt"]["settlement"], "settled");
    assert_eq!(
        result["purchase"]["quote"]["context"]["payer_workspace"],
        workspace
    );
    assert!(
        !colleague
            .run(vec![
                "invoke".into(),
                "--purchase".into(),
                "team-task".into()
            ])
            .await
            .0
    );
    let remove = command(
        "remove-colleague",
        &d.address,
        &ada.account,
        "session",
        json!({"kind":"remove","workspace":workspace,"account":bob.account}),
    );
    assert!(champion.change(remove, None).await.0);
    assert!(!colleague.run(vec!["current".into()]).await.0);
    assert!(
        !colleague
            .run(vec![
                "team".into(),
                "members".into(),
                "--workspace".into(),
                workspace.clone()
            ])
            .await
            .0
    );
    let (status, _) = post(
        &d,
        &format!("/v1/workspaces/{workspace}/recovery"),
        Some(&ada.session_token),
        &json!({"account":bob.account}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    colleague.import("personal-key", &bob.key_token).await;
    let recovery = colleague.directory.path().join("restore.json");
    private_file(&recovery,&serde_json::to_vec(&json!({"id":"restore-current-session","origin":d.address,"account":bob.account,"credential_alias":"personal-key","action":{"kind":"sign-in","output_alias":"restored"}})).unwrap());
    colleague
        .ok(vec![
            "change".into(),
            "--input".into(),
            recovery.to_str().unwrap().into(),
        ])
        .await;
    colleague
        .select(&d.address, &bob.account, &bob.workspace, "restored")
        .await;
    assert!(
        !colleague
            .run(vec![
                "team".into(),
                "inspect".into(),
                "--operation".into(),
                "accept-one".into()
            ])
            .await
            .0
    );
    assert!(
        !colleague
            .run(vec![
                "team".into(),
                "switch".into(),
                "--workspace".into(),
                workspace.clone(),
                "--door".into(),
                "acme-kev".into()
            ])
            .await
            .0
    );
    let (_, issued) = champion
        .change(
            command(
                "invite-again",
                &d.address,
                &ada.account,
                "session",
                json!({"kind":"invite","workspace":workspace,"role":"member","ttl_secs":60}),
            ),
            None,
        )
        .await;
    let file = std::path::PathBuf::from(issued["invitation_file"].as_str().unwrap());
    assert!(
        colleague
            .change(
                command(
                    "accept-again",
                    &d.address,
                    &bob.account,
                    "restored",
                    json!({"kind":"accept","workspace":workspace})
                ),
                Some(&file)
            )
            .await
            .0
    );
    colleague
        .ok(vec![
            "team".into(),
            "switch".into(),
            "--workspace".into(),
            workspace.clone(),
            "--door".into(),
            "acme-kev".into(),
        ])
        .await;
    let original = colleague
        .ok(vec![
            "team".into(),
            "inspect".into(),
            "--operation".into(),
            "accept-one".into(),
        ])
        .await;
    assert_eq!(original["command"]["credential_alias"], "session");
    assert_eq!(original["command"]["account"], bob.account);
    assert_eq!(original["status"], "applied");
    assert!(original["invitation_file"].is_null());
    let (_, transfer) = champion
        .change(
            command(
                "transfer-owner",
                &d.address,
                &ada.account,
                "session",
                json!({"kind":"transfer","workspace":workspace,"account":bob.account}),
            ),
            None,
        )
        .await;
    assert_eq!(transfer["status"], "applied");
    let (ok, last) = champion
        .change(
            command(
                "remove-last-owner",
                &d.address,
                &ada.account,
                "session",
                json!({"kind":"remove","workspace":workspace,"account":bob.account}),
            ),
            None,
        )
        .await;
    assert!(!ok);
    assert_eq!(last["status"], "refused");
    let retained: Value =
        serde_json::from_slice(&std::fs::read(champion.root().join("state.json")).unwrap())
            .unwrap();
    assert_eq!(
        retained["purchases"]["personal-before-team"]["quote"],
        personal["quote"]
    );
    assert_eq!(
        retained["purchases"]["personal-before-team"]["quote"]["context"]["payer_workspace"],
        ada.workspace
    );
    // The owning gateway commits the invitation, while the transport loses
    // its acknowledgment. Reopening the installed client must not repeat it.
    let posts = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let destination = d.address.clone();
    let effects = posts.clone();
    let proxy=axum::Router::new().fallback(move |method:axum::http::Method,uri:axum::http::Uri,headers:axum::http::HeaderMap,body:axum::body::Bytes| {
        let destination=destination.clone();let effects=effects.clone();
        async move {
            use axum::response::IntoResponse;
            let changed=method==axum::http::Method::POST && uri.path().ends_with("/invitations");
            let response=reqwest::Client::new().request(method,format!("{destination}{uri}")).headers(headers).body(body).send().await.unwrap();
            let status=response.status();let body:Value=response.json().await.unwrap();
            if changed {
                assert_eq!(status,StatusCode::CREATED);
                effects.fetch_add(1,std::sync::atomic::Ordering::SeqCst);
                return (StatusCode::INTERNAL_SERVER_ERROR,axum::Json(json!({"error":{"code":"fixture_ack_lost","message":"The effect was not acknowledged."}}))).into_response();
            }
            (status,axum::Json(body)).into_response()
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy_origin = format!("http://{}", listener.local_addr().unwrap());
    let proxy_task = tokio::spawn(axum::serve(listener, proxy).into_future());
    let mut intent = command(
        "lost-invitation",
        &proxy_origin,
        &ada.account,
        "session",
        json!({"kind":"invite","workspace":workspace,"role":"member","ttl_secs":60}),
    );
    let (ok, lost) = champion.change(intent.clone(), None).await;
    assert!(!ok);
    assert_eq!(lost["status"], "unknown");
    assert!(lost["invitation_file"].is_null());
    let (ok, repeated) = champion.change(intent.clone(), None).await;
    assert!(!ok);
    assert_eq!(repeated["status"], "unknown");
    intent["id"] = json!("renamed-lost-invitation");
    assert!(!champion.change(intent, None).await.0);
    assert_eq!(posts.load(std::sync::atomic::Ordering::SeqCst), 1);
    proxy_task.abort();
    d.server.abort();
}
