//! Selected native service transport and private evidence fixtures.
use super::*;
use atif::{Call, Log, Session, Source, Step};
use gym::sales_evidence as evidence;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};
use tenancy::accounts::team_reports::{Evidence, Reference};
use tenancy::{Accounts, Role};

async fn start(host: &mut Host) {
    let state = ServeState::open(host.config.clone()).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    host.address = format!("http://{}", listener.local_addr().unwrap());
    let (shutdown, receive) = oneshot::channel();
    let router = serve::router(state.clone());
    host.server = Some(tokio::spawn(async move {
        axum::serve(listener, router)
            .with_graceful_shutdown(async {
                let _ = receive.await;
            })
            .await
            .unwrap();
    }));
    host.shutdown = Some(shutdown);
    host.state = Some(state);
}
async fn host() -> Host {
    let mut host = Host::new(10_000, |_, _| {}).await;
    host.stop().await;
    let root = host.dir.path().join("private-evidence");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    host.config.team_reports = Some(gateway::team_reports::Config {
        evidence_root: root.canonicalize().unwrap(),
    });
    start(&mut host).await;
    host
}
fn root(host: &Host) -> &Path {
    &host.config.team_reports.as_ref().unwrap().evidence_root
}
fn retain(root: &Path, name: &str, bytes: &[u8]) -> evidence::Reference {
    fs::write(root.join(name), bytes).unwrap();
    fs::set_permissions(root.join(name), fs::Permissions::from_mode(0o600)).unwrap();
    evidence::Reference {
        path: name.into(),
        sha256: evidence::digest(bytes),
    }
}
fn task_attempt(
    host: &Host,
    id: &str,
    receipt: &receipts::ExecutionReceipt,
    output: &str,
    status: evidence::Status,
    accepted: bool,
) -> evidence::Attempt {
    let file = root(host).join(format!("{id}.jsonl"));
    let mut log = Log::create_at(
        &file,
        &Session::opening(
            id,
            "kev-0.6b",
            "native-gateway",
            "protected-customer-project",
            "synthetic-v1",
        ),
    )
    .unwrap();
    let mut call = Call {
        id: "native-call".into(),
        name: "decision".into(),
        arguments: request(),
        output: output.into(),
        outcome: atif::Outcome::Completed,
        milliseconds: 1,
        purpose: Some("protected-task-purpose".into()),
        extra: Default::default(),
    };
    call.extra
        .insert("schema".into(), json!(atif::document::DECISION_CALL_SCHEMA));
    call.extra
        .insert("request_id".into(), json!(receipt.request));
    call.extra
        .insert("native_attempt".into(), json!(receipt.attempt));
    call.extra
        .insert("native_receipt".into(), json!(receipt.digest));
    call.extra
        .insert("model".into(), json!(receipt.served.model));
    let mut step = Step::said(Source::Agent, "protected-customer-prompt");
    step.call = Some(call);
    log.append(&step).unwrap();
    log.finish(atif::log::ENDED).unwrap();
    drop(log);
    fs::set_permissions(&file, fs::Permissions::from_mode(0o600)).unwrap();
    let trace = evidence::Reference {
        path: format!("{id}.jsonl"),
        sha256: evidence::digest(&fs::read(file).unwrap()),
    };
    let artifact = retain(root(host), &format!("{id}.response"), output.as_bytes());
    let check = retain(
        root(host),
        &format!("{id}.check"),
        b"protected-exact-independent-check",
    );
    let acceptance = accepted.then(|| evidence::Acceptance {
        candidate_digest: artifact.sha256.clone(),
        independent_checker: "protected-independent-reviewer".into(),
        check_review: retain(
            root(host),
            &format!("{id}.review"),
            b"fixture independent check review",
        ),
        customer_decision: retain(
            root(host),
            &format!("{id}.decision"),
            b"synthetic acceptance, no real customer",
        ),
    });
    evidence::Attempt {
        id: id.into(),
        kind: evidence::AttemptKind::Primary,
        parent: None,
        executor: "protected-executor".into(),
        trace,
        artifact,
        checks: [(
            "protected-check".into(),
            evidence::Check {
                check_digest: "c".repeat(64),
                status,
                evidence: Some(check),
            },
        )]
        .into(),
        setup_ms: 1,
        queue_ms: 2,
        check_ms: 3,
        support_ms: 4,
        costs: vec![],
        compute: None,
        acceptance,
    }
}
fn binding(
    host: &Host,
    receipt: &receipts::ExecutionReceipt,
    output: &str,
    accepted: bool,
) -> Evidence {
    let id = &receipt.request;
    let candidate = task_attempt(
        host,
        &format!("{id}-candidate"),
        receipt,
        output,
        if accepted {
            evidence::Status::Passed
        } else {
            evidence::Status::Failed
        },
        accepted,
    );
    let baseline = task_attempt(
        host,
        &format!("{id}-baseline"),
        receipt,
        output,
        evidence::Status::Passed,
        false,
    );
    let task = evidence::Task {
        id: format!("{id}-protected-task"),
        task_digest: "d".repeat(64),
        check_digests: [("protected-check".into(), "c".repeat(64))].into(),
        baseline: vec![baseline],
        candidate: vec![candidate.clone()],
    };
    let inventory = evidence::Inventory {
        schema: "openagents.gym.sales-inventory.v1".into(),
        attempts: [
            (
                format!("{}/baseline", task.id),
                task.baseline.iter().map(|a| a.id.clone()).collect(),
            ),
            (
                format!("{}/candidate", task.id),
                task.candidate.iter().map(|a| a.id.clone()).collect(),
            ),
        ]
        .into(),
    };
    let manifest = evidence::Manifest {
        schema: evidence::SCHEMA.into(),
        offer_version: "synthetic-kev-offer-v1".into(),
        source_revision: "a".repeat(40),
        baseline_method: "protected-baseline-method".into(),
        candidate_method: "protected-candidate-method".into(),
        retrospective_selection: false,
        inventory: retain(
            root(host),
            &format!("{id}.inventory"),
            &serde_json::to_vec(&inventory).unwrap(),
        ),
        gym_store: None,
        tasks: vec![task.clone()],
        skipped_evidence: vec![],
    };
    let manifest = retain(
        root(host),
        &format!("{id}.manifest"),
        &serde_json::to_vec(&manifest).unwrap(),
    );
    Evidence {
        schema: tenancy::accounts::team_reports::SCHEMA.into(),
        receipt: receipt.digest.clone(),
        manifest: Reference {
            path: manifest.path,
            sha256: manifest.sha256,
        },
        task: task.id,
        candidate: candidate.id,
    }
}
async fn report(host: &Host, token: &str, workspace: &str, suffix: &str) -> reqwest::Response {
    reqwest::Client::new()
        .get(format!(
            "{}/v1/workspaces/{workspace}/reports{suffix}",
            host.address
        ))
        .bearer_auth(token)
        .send()
        .await
        .unwrap()
}
async fn attach(host: &Host, token: &str, b: &Evidence) -> reqwest::Response {
    reqwest::Client::new()
        .post(format!(
            "{}/v1/workspaces/{}/reports/evidence",
            host.address, host.workspace
        ))
        .bearer_auth(token)
        .json(b)
        .send()
        .await
        .unwrap()
}
#[tokio::test]
async fn accepted_failed_native_tasks_exact_statements_current_sources_and_duplicate_attachment() {
    let mut host = host().await;
    let response = host.call("accepted-native").await;
    assert_eq!(response.status(), 200);
    let output = response.text().await.unwrap();
    let receipt = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "accepted-native")
        .unwrap();
    assert!(receipt.member.is_some());
    assert!(receipt.timing.queued_ms.is_some());
    let accepted = binding(&host, &receipt, &output, true);
    let first = attach(&host, &host.token, &accepted).await;
    assert_eq!(
        first.status(),
        200,
        "{}",
        first.text().await.unwrap_or_default()
    );
    let before = host.balance().await;
    let account_revision = Accounts::open(host.dir.path())
        .unwrap()
        .store()
        .unwrap()
        .digest;
    let retry = attach(&host, &host.token, &accepted).await;
    assert_eq!(retry.status(), 200);
    assert_eq!(host.balance().await, before);
    assert_eq!(
        Accounts::open(host.dir.path())
            .unwrap()
            .store()
            .unwrap()
            .digest,
        account_revision
    );
    let output = host.call("failed-native").await.text().await.unwrap();
    let failed_receipt = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "failed-native")
        .unwrap();
    let failed = binding(&host, &failed_receipt, &output, false);
    assert_eq!(attach(&host, &host.token, &failed).await.status(), 200);
    // Duplicate physical receipt lines remain one native task and create no charge.
    use std::io::Write;
    let mut file = fs::OpenOptions::new()
        .append(true)
        .open(host.dir.path().join("receipts.jsonl"))
        .unwrap();
    writeln!(file, "{}", serde_json::to_string(&receipt).unwrap()).unwrap();
    drop(file);
    let value: Value = report(&host, &host.token, &host.workspace, "/export")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(value["totals"]["tasks"], 2);
    assert_eq!(value["totals"]["accepted"], 1);
    assert_eq!(value["totals"]["failed"], 1);
    assert_eq!(value["totals"]["known_charges"], 22);
    assert_eq!(value["totals"]["unknown_provider_costs"], 2);
    assert_eq!(value["totals"]["known_provider_costs"], 0);
    assert!(
        value["rows"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["statement_reference"] == value["statement_reference"])
    );
    for r in value["rows"].as_array().unwrap() {
        assert_eq!(r["payer_workspace"], host.workspace);
        assert_eq!(r["served_artifact"], identity().artifact_signature);
        assert!(r["wait_ms"].is_number());
        assert_eq!(r["evidence"]["status"], "current_verified");
        assert_eq!(r["provider_cost"], Value::Null);
        assert_eq!(r["plugin_release"], Value::Null);
    }
    let encoded = serde_json::to_string(&value).unwrap();
    for private in [
        "protected-",
        "synthetic acceptance",
        &host.token,
        "accepted-native",
        "failed-native",
    ] {
        assert!(!encoded.contains(private), "{private}");
    }
    fs::write(
        root(&host).join("accepted-native-candidate.review"),
        b"changed review",
    )
    .unwrap();
    let changed: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(changed["totals"]["accepted"], 0);
    assert!(
        changed["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["evidence"]["status"] == "source_unavailable_or_changed")
    );
    assert_eq!(attach(&host, &host.token, &accepted).await.status(), 403);
    host.stop().await;
    start(&mut host).await;
    let restarted: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(restarted["totals"]["tasks"], 2);
    assert_eq!(restarted["totals"]["known_charges"], 22);
    assert_eq!(restarted["totals"]["accepted"], 0);
}
#[tokio::test]
async fn combined_source_budget_preserves_delivery_and_refuses_partial_acceptance() {
    let host = host().await;
    for name in ["budget-1", "budget-2", "budget-3", "budget-4"] {
        let response = host.call(name).await;
        assert_eq!(response.status(), 200);
        let output = response.text().await.unwrap();
        let receipt = host
            .receipts()
            .into_iter()
            .find(|r| r.request == name)
            .unwrap();
        let evidence = binding(&host, &receipt, &output, true);
        // Each independent attachment has its own bounded verification request.
        assert_eq!(attach(&host, &host.token, &evidence).await.status(), 200);
    }
    let before = host.balance().await;
    let response: Value = report(&host, &host.token, &host.workspace, "/export")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(response["totals"]["tasks"], 4);
    assert_eq!(response["totals"]["known_charges"], 44);
    assert_eq!(response["totals"]["accepted"], 3);
    assert_eq!(response["totals"]["delivered"], 1);
    let incomplete = response["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["evidence"]["status"] == "read_budget_unavailable")
        .unwrap();
    assert_eq!(incomplete["evidence"]["accepted"], false);
    assert_eq!(incomplete["evidence"]["comparison"], Value::Null);
    assert_eq!(incomplete["state"], "delivered");
    assert_eq!(incomplete["charged"], 11);
    assert_eq!(host.balance().await, before);
}

#[tokio::test]
async fn live_submitted_running_waits_and_restart_unavailable_are_distinct_from_holds() {
    let mut host = host().await;
    host.backend.card_block.store(true, Ordering::SeqCst);
    let address = host.address.clone();
    let token = host.token.clone();
    let workspace = host.workspace.clone();
    let job = tokio::spawn(async move { send(&address, &token, &workspace, "live-native").await });
    host.backend.card_entered.notified().await;
    let submitted: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(submitted["rows"][0]["state"], "submitted");
    assert_eq!(submitted["rows"][0]["wait_ms"], Value::Null);
    assert_eq!(submitted["rows"][0]["hold_phase"], "held");
    host.backend.block.store(true, Ordering::SeqCst);
    host.backend.card_resume.notify_one();
    host.backend.entered.notified().await;
    let running: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(running["rows"][0]["state"], "running");
    assert!(running["rows"][0]["wait_ms"].is_number());
    assert_eq!(running["rows"][0]["hold_phase"], "held");
    // Unknown native response expense retains the held obligation.
    *host.backend.reply.lock().unwrap() = json!({"model":"kev-0.6b","answers":{}});
    host.backend.resume.notify_one();
    assert_eq!(job.await.unwrap().status(), 200);
    host.stop().await;
    start(&mut host).await;
    let after: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(after["rows"][0]["state"], "delivered");
    assert_eq!(after["rows"][0]["hold_phase"], "unknown");
    assert_eq!(after["rows"][0]["charged"], Value::Null);
    assert_eq!(after["totals"]["unknown_charges"], 1);
    host.stop().await;
    // A reserve recovered without its producer/receipt is not evidence of running work.
    {
        let mut ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
        ledger
            .apply(Mutation {
                workspace: host.workspace.clone(),
                source: "synthetic-unfinished".into(),
                audit: "fake restart boundary".into(),
                operation: Operation::Reserve {
                    attempt: "unfinished#1".into(),
                    request_digest: receipts::execution::digest_request(&request()),
                    price: priced().price,
                    maximum_usage: priced().maximum_usage,
                },
            })
            .unwrap();
    }
    start(&mut host).await;
    let unknown: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert!(
        unknown["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["state"] == "unavailable"
                && r["wait_ms"] == Value::Null
                && r["original_member"] == Value::Null)
    );
}
#[tokio::test]
async fn original_member_scoping_current_rights_revocation_redaction_and_browser_export() {
    let mut host = host().await;
    host.stop().await;
    let accounts = Accounts::open(host.dir.path()).unwrap();
    let owner = accounts
        .account_of_principal(&format!(
            "key:{}",
            keys::authenticate(
                host.dir.path(),
                Registry::open(host.dir.path()).unwrap().manifest(),
                &host.token
            )
            .unwrap()
            .key_id
        ))
        .unwrap()
        .unwrap();
    let workspace = accounts
        .create_workspace(
            &owner,
            "protected-team-label",
            tenancy::WorkspaceKind::Organization,
            "buyer",
            None,
        )
        .unwrap()
        .id;
    let registry = Registry::open(host.dir.path()).unwrap();
    let issued = keys::issue(host.dir.path(), registry.manifest(), "buyer").unwrap();
    let colleague = accounts
        .create_account(
            "protected-colleague-label",
            &[format!("key:{}", issued.key.id)],
        )
        .unwrap();
    let invite = accounts
        .invite(&owner, &workspace, Role::Member, 3600)
        .unwrap();
    accounts.accept(&colleague.id, &invite.token).unwrap();
    {
        let mut ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
        for (source, operation) in [
            (
                "create-team",
                Operation::Create {
                    currency: "USD".into(),
                    spend_limit: 10_000,
                    topups_allowed: false,
                },
            ),
            (
                "credit-team",
                Operation::Credit {
                    amount: 10_000,
                    credit_kind: CreditKind::Grant,
                },
            ),
        ] {
            ledger
                .apply(Mutation {
                    workspace: workspace.clone(),
                    source: source.into(),
                    audit: "isolated team fixture".into(),
                    operation,
                })
                .unwrap();
        }
    }
    host.workspace = workspace.clone();
    start(&mut host).await;
    assert_eq!(host.call("owner-task").await.status(), 200);
    assert_eq!(
        send(&host.address, &issued.token, &workspace, "member-task")
            .await
            .status(),
        200
    );
    // A retained legacy receipt and unresolved native hold have no original account.
    // Current key labels and matching task names cannot give this history to a Member.
    host.stop().await;
    let mut legacy = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "owner-task")
        .unwrap();
    legacy.member = None;
    legacy.request = "protected-legacy-request".into();
    legacy.attempt_id = "protected-legacy-request#1".into();
    legacy.seal();
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(host.dir.path().join("receipts.jsonl"))
            .unwrap();
        writeln!(file, "{}", serde_json::to_string(&legacy).unwrap()).unwrap();
        file.sync_all().unwrap();
        let mut ledger = Ledger::open(&host.config.money.as_ref().unwrap().ledger).unwrap();
        ledger
            .apply(Mutation {
                workspace: workspace.clone(),
                source: "legacy-unattributed-native-hold".into(),
                audit: "isolated unknown history; no external funding".into(),
                operation: Operation::Reserve {
                    attempt: "protected-legacy-request#1".into(),
                    request_digest: legacy.request_digest.clone(),
                    price: priced().price,
                    maximum_usage: priced().maximum_usage,
                },
            })
            .unwrap();
    }
    start(&mut host).await;
    let owner_report: Value = report(&host, &host.token, &workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(owner_report["totals"]["tasks"], 3);
    let member_report: Value = report(&host, &issued.token, &workspace, "/export")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(member_report["scope"], "own_original_tasks");
    assert_eq!(member_report["totals"]["tasks"], 1);
    assert_eq!(
        member_report["rows"][0]["original_member"]["account"],
        colleague.id
    );
    assert_ne!(
        member_report["statement_reference"],
        owner_report["statement_reference"]
    );
    assert_eq!(
        report(&host, &issued.token, &host.foreign, "")
            .await
            .status(),
        403
    );
    let receipt = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "member-task")
        .unwrap();
    let b = binding(
        &host,
        &receipt,
        &serde_json::to_string(&host.backend.reply.lock().unwrap().clone()).unwrap(),
        true,
    );
    assert_eq!(attach(&host, &issued.token, &b).await.status(), 403);
    let owner_receipt = host
        .receipts()
        .into_iter()
        .find(|r| r.request == "owner-task")
        .unwrap();
    let legacy_get = |token: String, suffix: String| {
        let url = format!("{}/v1/workspaces/{workspace}/usage{suffix}", host.address);
        async move {
            reqwest::Client::new()
                .get(url)
                .bearer_auth(token)
                .send()
                .await
                .unwrap()
        }
    };
    let summary: Value = legacy_get(issued.token.clone(), String::new())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(summary["totals"]["calls"], 1);
    assert_eq!(summary["cost"]["retail"], 11);
    assert!(summary["outstanding"].as_array().unwrap().is_empty());
    assert_eq!(summary["entitlement"], Value::Null);
    assert_eq!(summary["disclosure"]["other_workspace"], Value::Null);
    let owner_summary: Value = legacy_get(host.token.clone(), String::new())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(owner_summary["totals"]["calls"], 3);
    assert_eq!(owner_summary["outstanding"].as_array().unwrap().len(), 1);
    let activity: Value = legacy_get(issued.token.clone(), "/activity".into())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(activity["items"].as_array().unwrap().len(), 1);
    assert_eq!(activity["items"][0]["request"], "member-task");
    let timeseries: Value = legacy_get(issued.token.clone(), "/timeseries".into())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(
        timeseries["days"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| d["calls"].as_u64().unwrap())
            .sum::<u64>(),
        1
    );
    for digest in [&owner_receipt.digest, &legacy.digest] {
        assert_eq!(
            legacy_get(issued.token.clone(), format!("/receipts/{digest}"))
                .await
                .status(),
            404
        );
        assert_eq!(
            legacy_get(host.token.clone(), format!("/receipts/{digest}"))
                .await
                .status(),
            200
        );
    }
    let own_detail = legacy_get(
        issued.token.clone(),
        format!("/receipts/{}", receipt.digest),
    )
    .await;
    assert_eq!(own_detail.status(), 200);
    assert_eq!(own_detail.headers()["cache-control"], "no-store");
    let exported = legacy_get(issued.token.clone(), "/export".into()).await;
    assert_eq!(exported.headers()["cache-control"], "no-store");
    let text = exported.text().await.unwrap();
    assert_eq!(text.lines().count(), 1);
    assert_eq!(
        receipts::execution::ExecutionReceipt::parse(text.trim())
            .unwrap()
            .request,
        "member-task"
    );
    assert!(!text.contains("owner-task") && !text.contains("protected-"));
    let member_session = tenancy::Sessions::open(host.dir.path())
        .unwrap()
        .mutate(|book, _, now| book.issue(tenancy::workspaces::UserId(colleague.id.clone()), now))
        .unwrap();
    let browser_get = |suffix: String| {
        let url = format!("{}/dashboard/w/{workspace}{suffix}", host.address);
        let cookie = format!("oa_session={}", member_session.once);
        async move {
            reqwest::Client::new()
                .get(url)
                .header("cookie", cookie)
                .send()
                .await
                .unwrap()
        }
    };
    for suffix in ["", "/usage", "/activity"] {
        let response = browser_get(suffix.into()).await;
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()["cache-control"], "no-store");
        let text = response.text().await.unwrap();
        assert!(
            !text.contains("owner-task")
                && !text.contains("protected-")
                && !text.contains(&owner_receipt.digest)
        );
        if suffix.is_empty() {
            assert!(text.contains("Team work report"));
            assert!(!text.contains("Credit added") && !text.contains("Available"));
        }
    }
    assert_eq!(
        browser_get(format!("/receipts/{}", receipt.digest))
            .await
            .status(),
        200
    );
    assert_eq!(
        browser_get(format!("/receipts/{}", owner_receipt.digest))
            .await
            .status(),
        404
    );
    assert_eq!(
        browser_get(format!("/receipts/{}", legacy.digest))
            .await
            .status(),
        404
    );

    // The same current member gains workspace visibility only while native Admin rights exist.
    accounts
        .set_role(&owner, &workspace, &colleague.id, Role::Admin)
        .unwrap();
    let admin: Value = legacy_get(issued.token.clone(), String::new())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(admin["totals"]["calls"], 3);
    let admin_report: Value = report(&host, &issued.token, &workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(admin_report["totals"]["tasks"], 3);
    accounts
        .set_role(&owner, &workspace, &colleague.id, Role::Member)
        .unwrap();
    let narrowed: Value = legacy_get(issued.token.clone(), String::new())
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(narrowed["totals"]["calls"], 1);
    // A bounded global scan cannot silently claim complete own-task history.
    let path = host.dir.path().join("receipts.jsonl");
    let original = fs::read(&path).unwrap();
    {
        use std::io::Write;
        let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
        file.write_all(&vec![b'\n'; 100_001]).unwrap();
        file.sync_all().unwrap();
    }
    assert_eq!(
        legacy_get(issued.token.clone(), "/activity".into())
            .await
            .status(),
        503
    );
    fs::write(&path, original).unwrap();
    assert_eq!(
        legacy_get(issued.token.clone(), "/activity".into())
            .await
            .status(),
        200
    );
    accounts
        .remove_member(&owner, &workspace, &colleague.id)
        .unwrap();
    assert_eq!(
        report(&host, &issued.token, &workspace, "").await.status(),
        403
    );
    assert_eq!(
        report(&host, &issued.token, &workspace, "/export")
            .await
            .status(),
        403
    );
    for suffix in ["", "/activity", "/timeseries", "/export"] {
        assert_eq!(
            legacy_get(issued.token.clone(), suffix.into())
                .await
                .status(),
            403
        );
    }
    for suffix in ["", "/usage", "/activity", "/reports/export"] {
        assert_eq!(browser_get(suffix.into()).await.status(), 403);
    }
    // Owner history retains the original member after removal; it never relabels to the current key owner.
    let history: Value = report(&host, &host.token, &workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(history["totals"]["tasks"], 3);
    assert!(
        history["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["original_member"]["account"] == colleague.id)
    );
    let body = serde_json::to_string(&history).unwrap();
    assert!(!body.contains("protected-"));
    assert!(!body.contains(&issued.token));
    let sessions = tenancy::Sessions::open(host.dir.path()).unwrap();
    let session = sessions
        .mutate(|book, _access, now| book.issue(tenancy::workspaces::UserId(owner.clone()), now))
        .unwrap();
    let page = reqwest::Client::new()
        .get(format!("{}/dashboard/w/{workspace}/reports", host.address))
        .header("cookie", format!("oa_session={}", session.once))
        .send()
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    let html = page.text().await.unwrap();
    assert!(html.contains("Team work report"));
    assert!(html.contains("unknown"));
    assert!(!html.contains("protected-"));
    let exported = reqwest::Client::new()
        .get(format!(
            "{}/dashboard/w/{workspace}/reports/export",
            host.address
        ))
        .header("cookie", format!("oa_session={}", session.once))
        .send()
        .await
        .unwrap();
    assert_eq!(exported.status(), 200);
    let export: Value = exported.json().await.unwrap();
    assert_eq!(export["totals"]["tasks"], 3);
    let expired = sessions
        .mutate(|book, _, now| {
            book.issue(
                tenancy::workspaces::UserId(owner.clone()),
                now.saturating_sub(40_000),
            )
        })
        .unwrap();
    assert_eq!(
        report(&host, &expired.once, &workspace, "/export")
            .await
            .status(),
        403
    );
    keys::revoke(host.dir.path(), &issued.key.id).unwrap();
    assert_eq!(
        report(&host, &issued.token, &workspace, "").await.status(),
        403
    );
}

#[tokio::test]
async fn native_source_replacement_refuses_reads_and_refunds_and_zero_expense_stay_separate() {
    let mut host = host().await;
    assert_eq!(host.call("native-refund").await.status(), 200);
    host.stop().await;
    let ledger_path = &host.config.money.as_ref().unwrap().ledger;
    {
        let mut ledger = Ledger::open(ledger_path).unwrap();
        ledger
            .apply(Mutation {
                workspace: host.workspace.clone(),
                source: "fixture-refund".into(),
                audit: "synthetic accounting refund, no actual funds".into(),
                operation: Operation::Refund {
                    attempt: "native-refund#1".into(),
                    amount: 3,
                },
            })
            .unwrap();
        ledger
            .apply(Mutation {
                workspace: host.workspace.clone(),
                source: "fixture-zero-reserve".into(),
                audit: "operator fixture only".into(),
                operation: Operation::Reserve {
                    attempt: "legacy-zero#1".into(),
                    request_digest: receipts::execution::digest_request(&request()),
                    price: priced().price,
                    maximum_usage: priced().maximum_usage,
                },
            })
            .unwrap();
        ledger
            .apply(Mutation {
                workspace: host.workspace.clone(),
                source: "fixture-zero-settle".into(),
                audit: "operator fixture, explicitly known zero expense".into(),
                operation: Operation::Settle {
                    attempt: "legacy-zero#1".into(),
                    usage: [(Resource::InputTokens, 3)].into(),
                    receipt: "legacy-zero#1".into(),
                    provider_cost: Some(0),
                    hosting_cost: Some(0),
                },
            })
            .unwrap();
    }
    start(&mut host).await;
    let v: Value = report(&host, &host.token, &host.workspace, "")
        .await
        .json()
        .await
        .unwrap();
    assert_eq!(v["totals"]["known_charges"], 22);
    assert_eq!(v["totals"]["refunded"], 3);
    assert_eq!(v["totals"]["unknown_provider_costs"], 1);
    assert_eq!(v["totals"]["known_provider_costs"], 0);
    assert!(
        v["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["provider_cost"] == 0 && r["state"] == "unavailable")
    );
    assert_eq!(v["wallet_liquidity"], Value::Null);
    let receipt_path = host.dir.path().join("receipts.jsonl");
    let retained = host.dir.path().join("original-receipts");
    fs::rename(&receipt_path, &retained).unwrap();
    fs::copy(&retained, &receipt_path).unwrap();
    assert_eq!(
        report(&host, &host.token, &host.workspace, "/export")
            .await
            .status(),
        403
    );
    fs::remove_file(&receipt_path).unwrap();
    fs::rename(&retained, &receipt_path).unwrap();
    let money_path = host.config.money.as_ref().unwrap().ledger.clone();
    let retained = host.dir.path().join("original-money");
    fs::rename(&money_path, &retained).unwrap();
    fs::copy(&retained, &money_path).unwrap();
    assert_eq!(
        report(&host, &host.token, &host.workspace, "")
            .await
            .status(),
        403
    );
    fs::remove_file(&money_path).unwrap();
    fs::rename(&retained, &money_path).unwrap();
    assert_eq!(
        report(&host, &host.token, &host.workspace, "")
            .await
            .status(),
        200
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[ignore = "Runs isolated Chrome through the supported browser wrapper."]
async fn browser_client_reads_redacted_report_and_current_cookie_export() {
    let host = host().await;
    assert_eq!(host.call("browser-native-task").await.status(), 200);
    let accounts = Accounts::open(host.dir.path()).unwrap();
    let key = keys::authenticate(
        host.dir.path(),
        Registry::open(host.dir.path()).unwrap().manifest(),
        &host.token,
    )
    .unwrap();
    let owner = accounts
        .account_of_principal(&format!("key:{}", key.key_id))
        .unwrap()
        .unwrap();
    let sessions = tenancy::Sessions::open(host.dir.path()).unwrap();
    let session = sessions
        .mutate(|book, _, now| book.issue(tenancy::workspaces::UserId(owner.clone()), now))
        .unwrap();
    let input = root(&host).join("browser-input.json");
    fs::write(
        &input,
        serde_json::to_vec(
            &json!({"origin":host.address,"workspace":host.workspace,"token":session.once}),
        )
        .unwrap(),
    )
    .unwrap();
    fs::set_permissions(&input, fs::Permissions::from_mode(0o600)).unwrap();
    let script = root(&host).join("browser-check.mjs");
    fs::write(&script,r#"
import fs from 'node:fs';
const input=JSON.parse(fs.readFileSync(process.argv[2],'utf8'));
const port=process.env.OPENAGENTS_CHROME_PORT;
if(!port) throw Error('The isolated browser port is required.');
const target=await(await fetch(`http://127.0.0.1:${port}/json/new?about:blank`,{method:'PUT'})).json();
const ws=new WebSocket(target.webSocketDebuggerUrl);
await new Promise((resolve,reject)=>{ws.addEventListener('open',resolve,{once:true});ws.addEventListener('error',reject,{once:true});});
let serial=0;const pending=new Map();
ws.addEventListener('message',e=>{const m=JSON.parse(e.data);const p=pending.get(m.id);if(p){pending.delete(m.id);m.error?p.reject(Error('Browser protocol refusal.')):p.resolve(m.result);}});
function call(method,params={}){return new Promise((resolve,reject)=>{const id=++serial;pending.set(id,{resolve,reject});ws.send(JSON.stringify({id,method,params}));});}
const evaluate=async expression=>(await call('Runtime.evaluate',{expression,returnByValue:true})).result.value;
async function until(check){for(let n=0;n<80;n++){const value=await evaluate('document.body.innerText');if(check(value))return value;await new Promise(r=>setTimeout(r,50));}throw Error('The current report did not load.');}
await call('Network.enable');await call('Page.enable');
await call('Network.setCookie',{name:'oa_session',value:input.token,url:input.origin,path:'/',httpOnly:true,sameSite:'Lax'});
await call('Page.navigate',{url:`${input.origin}/dashboard/w/${input.workspace}/reports`});
const text=await until(v=>v.includes('Team work report'));
if(!text.includes('delivered')||!text.includes('unknown')||text.includes('browser-native-task')||text.includes('Synthetic fixture state')||text.includes(input.token))throw Error('The report was not correctly scoped and redacted.');
await evaluate("document.querySelector('a[href$=\"/reports/export\"]').click()");
const exported=JSON.parse(await until(v=>{try{return JSON.parse(v).schema==='openagents.team-report.v1';}catch{return false;}}));
if(exported.totals.tasks!==1||exported.production_qualification!==false||exported.rows[0].charged!==11||exported.rows[0].evidence.accepted!==false)throw Error('The exact native export disagrees.');
await call('Network.deleteCookies',{name:'oa_session',url:input.origin});
await call('Page.navigate',{url:`${input.origin}/dashboard/w/${input.workspace}/reports/export`});
const denied=JSON.parse(await until(v=>{try{return !!JSON.parse(v).error;}catch{return false;}}));
if(denied.error.code!=='team_report_unavailable')throw Error('Removed browser authority still exported.');
ws.close();console.log('Isolated native report, redaction, export, and removed-cookie refusal passed.');
"#).unwrap();
    fs::set_permissions(&script, fs::Permissions::from_mode(0o600)).unwrap();
    let cli = std::env::var_os("OPENAGENTS_BROWSER_BIN")
        .expect("Select the supported isolated browser wrapper.");
    let status = tokio::process::Command::new(cli)
        .args(["browser", "run", "--json", "--"])
        .arg("node")
        .arg(&script)
        .arg(&input)
        .kill_on_drop(true)
        .status()
        .await
        .unwrap();
    assert!(status.success());
}
