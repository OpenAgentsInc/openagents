//! Real bounded processes over synthetic source, with separate buyer authority.
use super::*;
use coder::task::{self, RequestedConfiguration, TaskIntent, Workspace};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn git(directory: &Path, args: &[&str]) -> String {
    let result = std::process::Command::new("/usr/bin/git")
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .current_dir(directory)
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    String::from_utf8(result.stdout).unwrap().trim().into()
}
fn repository(root: &Path, name: &str, filename: &str, bytes: &[u8]) -> PathBuf {
    let repo = root.join(format!("{name}-repository"));
    std::fs::create_dir(&repo).unwrap();
    git(&repo, &["init", "-q"]);
    std::fs::write(repo.join(filename), bytes).unwrap();
    git(&repo, &["add", filename]);
    git(
        &repo,
        &[
            "-c",
            "user.name=Fixture",
            "-c",
            "user.email=fixture@example.invalid",
            "commit",
            "-qm",
            "Synthetic source",
        ],
    );
    let work = root.join(format!("{name}-workspace"));
    git(
        &repo,
        &["worktree", "add", "--detach", "-q", work.to_str().unwrap()],
    );
    work.canonicalize().unwrap()
}
fn intent(workspace: &Path, title: &str) -> TaskIntent {
    TaskIntent {
        title: title.into(),
        prompt: title.into(),
        workspace: Workspace {
            path: workspace.to_string_lossy().into(),
            source_revision: Some(git(workspace, &["rev-parse", "HEAD"])),
        },
        configuration: RequestedConfiguration {
            adapter: "bounded-command".into(),
            model: None,
        },
        images: Vec::new(),
    }
}
fn make_grant(intent: &TaskIntent, id: &str, program: &Path, args: Vec<String>) -> Value {
    json!({"schema":task::owner::GRANT_SCHEMA,"task_id":id,"intent_digest":nostr::contracts::digest_bytes(&serde_json::to_vec(intent).unwrap()),"expected_revision":1,"expected_source_snapshot":coder_boundary::Snapshot::observe(Path::new(&intent.workspace.path)).digest(),"program":program.canonicalize().unwrap(),"arguments":args,"write_workspace":id=="labor-request","wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456,"requirements":null})
}
async fn transmit(
    f: &Fixture,
    url: &str,
    buyer: &mut store::Store,
    provider: &mut store::Store,
    value: &Value,
    schema: &str,
    from_buyer: bool,
) -> Event {
    let (from, to) = if from_buyer {
        (&f.buyer, &f.provider)
    } else {
        (&f.provider, &f.buyer)
    };
    let event = sealed(value, schema, from, to, f.now);
    transport::publish(url, from, &event).await.unwrap();
    for (key, store) in [(&f.buyer, buyer), (&f.provider, provider)] {
        let received = transport::fetch(url, key, &event.id).await.unwrap();
        assert_eq!(
            store.receive(received, f.now, f.blobs.clone()).unwrap(),
            "applied"
        );
    }
    event
}

#[tokio::test]
async fn bounded_coding_order_runs_separate_buyer_check_and_accepts_over_relay() {
    Box::pin(roundtrip(false, false)).await;
}
#[tokio::test]
async fn paid_order_runs_actual_provider_checker_and_exact_central_funding() {
    Box::pin(roundtrip(true, false)).await;
}
#[tokio::test]
async fn paid_failed_actual_checker_retains_rejection_and_cannot_invoice_or_accrue() {
    Box::pin(roundtrip(true, true)).await;
}
async fn roundtrip(paid: bool, failed_check: bool) {
    let retained = std::env::var_os("CODER_LABOR_ACCEPTANCE_DIR").map(PathBuf::from);
    let root = if let Some(parent) = &retained {
        std::fs::create_dir_all(parent).unwrap();
        tempfile::Builder::new()
            .prefix("labor-")
            .tempdir_in(parent)
            .unwrap()
    } else {
        tempfile::tempdir().unwrap()
    };
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    let expected = b"pub fn answer() -> u32 { 42 }\n";
    let workspace = repository(
        root.path(),
        "provider",
        "answer.rs",
        b"pub fn answer() -> u32 { 41 }\n",
    );
    let checker_workspace = repository(root.path(), "buyer-check", "expected.rs", expected);
    let task_intent = intent(
        &workspace,
        "Repair answer() to return 42; deliver the changed Rust file.",
    );
    let grant = make_grant(
        &task_intent,
        "labor-request",
        Path::new("/bin/sh"),
        vec![
            "-c".into(),
            format!(
                "printf 'pub fn answer() -> u32 {{ {} }}\\n' > answer.rs",
                if failed_check { 43 } else { 42 }
            ),
        ],
    );
    let input = json!({"v":"coder.free-labor.command.v1","requires":[],"intent":task_intent,"source_snapshot":grant["expected_source_snapshot"],"expected_output_digest":nostr::contracts::digest_bytes(expected)});
    let requirements = json!({"v":"coder.free-labor.requirements.v1","requires":[],"program_digest":nostr::contracts::digest_bytes(&std::fs::read(grant["program"].as_str().unwrap()).unwrap()),"arguments_digest":nostr::contracts::digest_bytes(&jcs(&grant["arguments"]).unwrap()),"write_workspace":true,"wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456});
    let mut f = fixture_with_paid(Some((input, requirements)), paid);
    let (url, relay, events) = relay::start().await;
    let buyer_path = root.path().join("buyer");
    let provider_path = root.path().join("provider");
    let tasks_path = root.path().join("provider-tasks");
    let pipeline = paid.then(|| super::paid_pipeline::Fixture::new(root.path(), f.now, 10_000));
    let mut paid_evidence = f.blobs.clone();
    if let Some(pipeline) = &pipeline {
        for (k, v) in &pipeline.evidence.0 {
            paid_evidence.0.insert(k.clone(), v.clone());
        }
        let setup = paid_setup(&f, pipeline, root.path(), &checker_workspace, &buyer_path);
        pipeline.write_grant(&setup, f.now, true);
        f.setup.paid = Some(setup);
        let mut changed = f.setup.clone();
        changed.paid.as_mut().unwrap().worker.price_msat += 1;
        assert!(Book::new(changed, f.provider).is_err());
        let mut changed = f.setup.clone();
        changed.paid.as_mut().unwrap().partner.obligation.trigger =
            receipts::service_sale::FulfillmentTrigger::VerifiedServicePayment;
        assert!(Book::new(changed, f.provider).is_err());
        let quote = private_artifact::open(&f.events[1], &f.buyer).unwrap();
        let mut losing: Value = serde_json::from_slice(quote.inline_bytes().unwrap()).unwrap();
        losing["body"]["quote_id"] = json!("d".repeat(64));
        let mut book = Book::new(f.setup.clone(), f.provider).unwrap();
        book.receive(&f.events[0], f.now, &Blobs::default())
            .unwrap();
        assert!(
            book.receive(
                &sealed(&losing, mkt::RECORD_SCHEMA, &f.provider, &f.buyer, f.now),
                f.now,
                &Blobs::default()
            )
            .is_err()
        );
        assert!(book.order().is_none());
    }
    let mut buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    for event in &f.events {
        let sender = if event.pubkey == public(&f.buyer) {
            &f.buyer
        } else {
            &f.provider
        };
        transport::publish(&url, sender, event).await.unwrap();
        for (key, store) in [(&f.buyer, &mut buyer), (&f.provider, &mut provider)] {
            let received = transport::fetch(&url, key, &event.id).await.unwrap();
            assert_eq!(
                store.receive(received, f.now, Blobs::default()).unwrap(),
                "applied"
            );
        }
    }
    let mut scratch = agreed(&f, f.provider);
    let execute = link(&mut f, &mut scratch);
    let linkage = scratch
        .records
        .resolve(scratch.records.link.as_ref().unwrap())
        .unwrap()
        .clone();
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &linkage,
        records::LINK,
        true,
    )
    .await;
    let grant_bytes = serde_json::to_vec(&grant).unwrap();
    // Fault injection at the durable-intent-before-submit boundary. This is
    // an interrupted journal fixture, not a claim that an OS crash occurred.
    let pending_path = root.path().join("interrupted-provider");
    drop(store::Store::open(&pending_path, f.setup.clone(), f.provider).unwrap());
    let pending_tasks = root.path().join("interrupted-tasks");
    drop(task::Store::open(&pending_tasks).unwrap());
    let mut pending: Value =
        serde_json::from_slice(&std::fs::read(provider_path.join("labor.json")).unwrap()).unwrap();
    pending["dispatch"] = json!({"execute":execute,"grant_digest":nostr::contracts::digest_bytes(&grant_bytes),"task_id":"labor-request","task_directory":pending_tasks,"observation":null,"state":"unknown"});
    std::fs::write(
        pending_path.join("labor.json"),
        serde_json::to_vec(&pending).unwrap(),
    )
    .unwrap();
    let mut interrupted = store::Store::open(&pending_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(interrupted.reconcile().unwrap().state, "unknown");
    let pending_result = if let Some(p) = &pipeline {
        interrupted
            .dispatch_paid(
                &mut authority(p, &paid_evidence),
                execute.clone(),
                &grant_bytes,
                &pending_tasks,
                f.now,
            )
            .await
    } else {
        interrupted
            .dispatch(execute.clone(), &grant_bytes, &pending_tasks, f.now)
            .await
    };
    assert_eq!(pending_result.unwrap().state, "unknown");
    assert!(
        task::Store::open(&pending_tasks)
            .unwrap()
            .show("labor-request")
            .is_err()
    );
    drop(interrupted);
    let result = if let Some(p) = &pipeline {
        assert!(
            provider
                .dispatch(execute.clone(), &grant_bytes, &tasks_path, f.now)
                .await
                .is_err()
        );
        provider
            .dispatch_paid(
                &mut authority(p, &paid_evidence),
                execute.clone(),
                &grant_bytes,
                &tasks_path,
                f.now,
            )
            .await
    } else {
        provider
            .dispatch(execute.clone(), &grant_bytes, &tasks_path, f.now)
            .await
    }
    .unwrap();
    assert_eq!(result.state, "finished");
    let task = result.observation.as_ref().unwrap();
    let produced =
        task::artifact::read(&tasks_path, "labor-request", Path::new("answer.rs")).unwrap();
    if failed_check {
        assert_ne!(produced, expected);
    } else {
        assert_eq!(produced, expected);
    }
    let first_run = task.run.as_ref().unwrap().clone();
    let repeated = if let Some(p) = &pipeline {
        provider
            .dispatch_paid(
                &mut authority(p, &paid_evidence),
                execute.clone(),
                &grant_bytes,
                &tasks_path,
                f.now,
            )
            .await
    } else {
        provider
            .dispatch(execute.clone(), &grant_bytes, &tasks_path, f.now)
            .await
    }
    .unwrap();
    assert_eq!(repeated.observation.unwrap().run.unwrap(), first_run);
    drop(provider);
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(provider.reconcile().unwrap().state, "finished");
    let output = put(
        &mut f.blobs,
        json!({"v":"openagents.free-labor.patch.v1","base":task_intent.workspace.source_revision,"source_snapshot":grant["expected_source_snapshot"],"candidate_snapshot":first_run.result.as_ref().unwrap().candidate_snapshot,"files":[{"path":"answer.rs","utf8":String::from_utf8(produced.clone()).unwrap()}]}),
        "openagents.free-labor.patch.v1",
    );
    let observation = put(
        &mut f.blobs,
        serde_json::to_value(&result).unwrap(),
        "openagents.free-labor.dispatch.v1",
    );
    let runs = run_evidence(&mut f, &provider.book, &output, &observation, "completed");
    let submission = json!({"v":records::SUBMISSION,"requires":[],"issuer":public(&f.provider),"order":order_value(&provider.book),"number":0,"previous":null,"rework":null,"executions":[provider.book.records.link],"deliverables":[{"id":"patch","content":output}],"run_evidence":runs,"limitations":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &submission,
        records::SUBMISSION,
        false,
    )
    .await;
    f.now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let delivery = json!({"v":records::DELIVERY,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"received_at":f.now,"available":true});
    // The receiver supplies its observed receipt time, never the provider's timestamp.
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &delivery,
        records::DELIVERY,
        true,
    )
    .await;

    // This checker workspace is never writable by the provider. The expected
    // bytes were chosen before dispatch; worker output enters as data only.
    assert_eq!(
        f.blobs.get(&f.setup.admission.input).unwrap()["expected_output_digest"],
        nostr::contracts::digest_bytes(
            &std::fs::read(checker_workspace.join("expected.rs")).unwrap()
        )
    );
    if !paid {
        std::fs::write(checker_workspace.join("candidate.rs"), &produced).unwrap();
    }
    let checker_intent = intent(
        &checker_workspace,
        "Compare retained candidate bytes with the frozen buyer acceptance fixture.",
    );
    let checker_grant = make_grant(
        &checker_intent,
        "buyer-check",
        Path::new("/usr/bin/cmp"),
        vec!["expected.rs".into(), "candidate.rs".into()],
    );
    let checker_directory = root.path().join("buyer-check-tasks");
    if !paid {
        let mut tasks = task::Store::open(&checker_directory).unwrap();
        tasks.apply(&serde_json::to_vec(&json!({"schema":task::COMMAND_SCHEMA,"command_id":"check-one","task_id":"buyer-check","expected_revision":null,"action":{"type":"submit","intent":checker_intent}})).unwrap()).unwrap();
    }
    let check = if let Some(p) = &pipeline {
        buyer
            .verify_paid_delivery(&mut authority(p, &paid_evidence), f.now)
            .await
            .unwrap()
            .observation
            .unwrap()
    } else {
        task::owner::execute(
            &checker_directory,
            &serde_json::to_vec(&checker_grant).unwrap(),
        )
        .await
        .unwrap()
    };
    let checked = check.run.as_ref().unwrap().result.as_ref().unwrap();
    assert_eq!(checked.exit_code, Some(if failed_check { 1 } else { 0 }));
    assert!(!checked.output_incomplete);
    let check_evidence = put(
        &mut f.blobs,
        serde_json::to_value(&check).unwrap(),
        "openagents.free-labor.check-observation.v1",
    );
    let verdict = if failed_check { "failed" } else { "passed" };
    let criteria = json!([{"id":"result","verdict":verdict,"evidence":[check_evidence]}]);
    let receipt = put(
        &mut f.blobs,
        json!({"v":"openagents.free-labor.checker.v1","requires":[],"submission":buyer.book.records.submission,"checker":f.setup.admission.checker,"lock":artifact_value(&buyer.book.policy().lock),"input":f.setup.admission.input,"criteria":criteria,"verdict":verdict,"elapsed_ms":checked.elapsed_ms,"cost_usd":null,"evidence":[check_evidence],"limitations":observation}),
        "openagents.free-labor.checker.v1",
    );
    let verification = json!({"v":records::VERIFICATION,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"policy":artifact_value(&buyer.book.labor().acceptance_policy),"checker_receipts":[receipt],"criteria":criteria,"verdict":verdict,"limitations":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &verification,
        records::VERIFICATION,
        true,
    )
    .await;
    assert!(buyer.book.records.acceptance.is_none());
    if failed_check {
        let p = pipeline.as_ref().unwrap();
        let rejected = json!({"v":records::REVIEW,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"verification":buyer.book.records.verification,"decision":"reject","criteria":["result"],"reason":observation});
        transmit(
            &f,
            &url,
            &mut buyer,
            &mut provider,
            &rejected,
            records::REVIEW,
            true,
        )
        .await;
        let wallet = FakeWallet {
            now: f.now,
            payment: Default::default(),
            issued: Default::default(),
            lookup_destination: Default::default(),
        };
        assert!(
            provider
                .prepare_worker_invoice(&mut authority(p, &paid_evidence), &wallet, f.now)
                .is_err()
        );
        let mut ledger =
            pay_ledger::Ledger::open(root.path().join("failed-central.sqlite")).unwrap();
        assert!(
            provider
                .reconcile_worker_funding(
                    &mut authority(p, &paid_evidence),
                    &wallet,
                    &mut ledger,
                    f.now
                )
                .is_err()
        );
        assert_eq!(wallet.issued.load(std::sync::atomic::Ordering::Relaxed), 0);
        assert!(provider.book.records.acceptance.is_none());
        assert_eq!(
            provider
                .book
                .records
                .resolve(provider.book.records.review.as_ref().unwrap())
                .unwrap()["decision"],
            "reject"
        );
        relay.abort();
        return;
    }
    let review = json!({"v":records::REVIEW,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"verification":buyer.book.records.verification,"decision":"accept","criteria":[],"reason":observation});
    transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &review,
        records::REVIEW,
        true,
    )
    .await;
    let acceptance = json!({"v":records::ACCEPTANCE,"requires":[],"issuer":public(&f.buyer),"order":order_value(&buyer.book),"submission":buyer.book.records.submission,"verification":buyer.book.records.verification,"outcome":"accepted","basis":"buyer_acceptance","review":buyer.book.records.review,"resolution":null,"amount_due_msat":buyer.book.market().price_msat,"supersedes":[],"evidence":[buyer.book.records.delivery]});
    let final_event = transmit(
        &f,
        &url,
        &mut buyer,
        &mut provider,
        &acceptance,
        records::ACCEPTANCE,
        true,
    )
    .await;
    assert_eq!(
        provider
            .receive(final_event.clone(), f.now, Blobs::default())
            .unwrap(),
        "duplicate"
    );
    let retained_events = events.lock().await.clone();
    let count = retained_events.len();
    std::fs::write(
        root.path().join("relay-events.json"),
        serde_json::to_vec_pretty(&retained_events).unwrap(),
    )
    .unwrap();
    relay.abort();
    drop(buyer);
    drop(provider);
    let buyer = store::Store::open(&buyer_path, f.setup.clone(), f.buyer).unwrap();
    let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
    assert_eq!(
        buyer.book.records.acceptance,
        provider.book.records.acceptance
    );
    assert!(buyer.book.records.acceptance.is_some());
    if paid {
        let (replacement, second, _) = relay::start().await;
        for event in retained_events.values() {
            let sender = if event.pubkey == public(&f.buyer) {
                &f.buyer
            } else {
                &f.provider
            };
            transport::publish(&replacement, sender, event)
                .await
                .unwrap();
        }
        let recovered = transport::fetch(&replacement, &f.provider, &final_event.id)
            .await
            .unwrap();
        assert_eq!(
            provider
                .receive(recovered, f.now, Blobs::default())
                .unwrap(),
            "duplicate"
        );
        second.abort();
    }
    if let Some(p) = &pipeline {
        paid_funding(&mut provider, &f, p, &paid_evidence, root.path());
        if let Some(binary) = std::env::var_os("CODER_LABOR_CLI") {
            cli_paid(
                Path::new(&binary),
                &f,
                p,
                &paid_evidence,
                root.path(),
                &provider_path,
                &buyer_path,
                &grant,
            );
        }
    }
    let mut report = json!({"schema":if paid {"openagents.paid-labor-acceptance-fixture.v1"} else {"openagents.free-labor-acceptance-fixture.v1"},"synthetic_source":true,"independent_operators":false,"production_relay":false,"encrypted_events":count,"provider_elapsed_ms":first_run.result.unwrap().elapsed_ms,"buyer_checker_elapsed_ms":checked.elapsed_ms,"inference_calls":0,"all_in_cost_usd":null,"cost_reason":"coordination, host CPU, storage, energy, and fees are unmetered","order_price_msat":provider.book.market().price_msat,"payment":if paid {"authenticated_fake_inbound_funding"} else {"not_applicable"},"paid":if paid {provider.paid_report().unwrap()} else {Value::Null},"actual_cli_processes":paid && std::env::var_os("CODER_LABOR_CLI").is_some(),"acceptance_event":final_event.id,"source":"one synthetic Rust file","result":"accepted","duplicate_execution":"same retained run","restart":"both roles reconstructed acceptance after relay shutdown"});
    if !paid {
        report["free_order_price_msat"] = json!(0);
    }
    std::fs::write(
        root.path().join("receipt.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    if retained.is_some() {
        let path = root.keep();
        eprintln!("Retained synthetic labor evidence at {}", path.display());
    }
}

fn authority<'a>(
    p: &'a super::paid_pipeline::Fixture,
    evidence: &'a Blobs,
) -> crate::paid::PipelineAuthority<'a> {
    crate::paid::PipelineAuthority {
        root: &p.host,
        credential: &p.owner_credential,
        evidence,
    }
}
fn hex(value: &str) -> String {
    value.strip_prefix("sha256:").unwrap_or(value).into()
}
fn paid_setup(
    f: &Fixture,
    p: &super::paid_pipeline::Fixture,
    root: &Path,
    checker_workspace: &Path,
    buyer_path: &Path,
) -> crate::paid::Setup {
    use pay_ledger::markets::{Deadlines, bids, worker};
    let opened = private_artifact::open(&f.setup.terms, &f.provider).unwrap();
    let market = mkt::parse_terms(opened.inline_bytes().unwrap()).unwrap();
    let raw = f.blobs.resolve(&market.profile_terms).unwrap();
    let labor = labor::parse_labor_terms(
        &jcs(raw).unwrap(),
        &labor::Parties {
            buyer: market.buyer.clone(),
            provider: market.provider.clone(),
            worker: market.worker.clone(),
        },
        Some(&market),
    )
    .unwrap();
    let policy = labor::parse_acceptance_policy(
        &jcs(f.blobs.resolve(&labor.acceptance_policy).unwrap()).unwrap(),
    )
    .unwrap();
    let request = private_artifact::open(&f.events[0], &f.provider).unwrap();
    let rfq = bids::Request {
        rfq: hex(&request.artifact().digest),
        capability: hex(&labor.execution.target.artifact.digest),
        source: hex(&labor.execution.input.digest),
        disclosure: hex(&labor.execution.context.digest),
        max_all_in_msat: 11_000,
        expires_at: (f.now + 90) as i64,
    };
    let bid = bids::Bid {
        quote: "b".repeat(64),
        rfq: rfq.rfq.clone(),
        provider: market.provider.clone(),
        labor_terms: hex(&market.profile_terms.digest),
        capability: rfq.capability.clone(),
        source: rfq.source.clone(),
        disclosure: rfq.disclosure.clone(),
        price_msat: 10_000,
        fee_limit_msat: 1_000,
        coordination_cost_msat: 0,
        expires_at: (f.now + 100) as i64,
        available_capacity: 1,
    };
    let selection = bids::Selection {
        bid_fingerprint: bid.fingerprint().unwrap(),
        quote: bid.quote.clone(),
        order: "c".repeat(64),
        admission: hex(p.grant_evidence["digest"].as_str().unwrap()),
        payer: market.buyer.clone(),
        approved_price_msat: 10_000,
        approved_fee_limit_msat: 1_000,
    };
    crate::paid::Setup {
        schema: "coder.paid-labor.setup.v1".into(),
        worker: worker::WorkerTerms {
            profile: worker::PROFILE.into(),
            buyer: market.buyer,
            provider: market.provider,
            buyer_operator: "synthetic-buyer-operator".into(),
            provider_operator: "synthetic-provider-operator".into(),
            market: f.setup.market.clone(),
            order: selection.order.clone(),
            labor_terms: bid.labor_terms.clone(),
            source: rfq.source.clone(),
            checker: hex(&policy.checker.artifact.digest),
            disclosure: rfq.disclosure.clone(),
            execution_requirements: hex(&labor.execution.requirements.digest),
            cancellation_policy: hex(&nostr::contracts::digest_bytes(
                &jcs(&raw["cancellation"]).unwrap(),
            )),
            delivery_rights: hex(&labor.rights.digest),
            price_msat: 10_000,
            fee_limit_msat: 1_000,
            capacity_units: 1,
            max_rework: 0,
            deadlines: Deadlines {
                delivery: (f.now + 300) as i64,
                review: (f.now + 400) as i64,
                dispute: (f.now + 450) as i64,
                resolution: (f.now + 500) as i64,
                payment: (f.now + 600) as i64,
                retain_until: (f.now + 900) as i64,
            },
        },
        rfq,
        bids: vec![bid],
        selection,
        selected_at: f.now,
        policy_file: p.policy_file.clone(),
        partner: p.partner.clone(),
        admission_issuer: public(&p.admission_issuer),
        policy_epoch: 1,
        central_node: secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &key(8)).to_string(),
        destination_kind: "node".into(),
        destination_value: secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &f.provider)
            .to_string(),
        checker: crate::paid::Checker {
            labor_directory: buyer_path.into(),
            task_directory: root.join("buyer-check-tasks"),
            task_id: "buyer-check".into(),
            intent: intent(
                checker_workspace,
                "Compare retained candidate bytes with the frozen buyer acceptance fixture.",
            ),
            program: Path::new("/usr/bin/cmp").canonicalize().unwrap(),
            program_sha256: hex(&nostr::contracts::digest_bytes(
                &std::fs::read("/usr/bin/cmp").unwrap(),
            )),
            expected_file: "expected.rs".into(),
            candidate_file: "candidate.rs".into(),
            provider_file: "answer.rs".into(),
            wall_seconds: 10,
        },
    }
}
struct FakeWallet {
    now: u64,
    payment: std::sync::Mutex<Option<openagents_wallet::PaymentRecord>>,
    issued: std::sync::atomic::AtomicU32,
    lookup_destination: std::sync::Mutex<Option<(PathBuf, pay_ledger::Payee)>>,
}
impl openagents_wallet::LightningWallet for FakeWallet {
    fn node_id(&self) -> String {
        secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &key(8)).to_string()
    }
    fn receive_exact(
        &self,
        amount: u64,
        description: [u8; 32],
        expiry: u32,
    ) -> std::result::Result<openagents_wallet::IssuedInvoice, openagents_wallet::WalletError> {
        use nostr::x402::test_invoice as inv;
        use sha2::{Digest, Sha256};
        self.issued
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let hash: [u8; 32] = Sha256::digest([7; 32]).into();
        let mut fields = inv::tag(1, &inv::words(&hash));
        fields.extend(inv::tag(16, &inv::words(&[2; 32])));
        fields.extend(inv::tag(23, &inv::words(&description)));
        fields.extend(inv::tag(6, &inv::number(expiry as u64)));
        let bolt11 = inv::signed_by(
            [8; 32],
            &format!("lnbc{}p", amount * 10),
            fields,
            true,
            false,
            self.now,
        );
        Ok(openagents_wallet::IssuedInvoice {
            bolt11,
            payment_hash: hash.iter().map(|n| format!("{n:02x}")).collect(),
            amount_msat: amount,
            description_hash: description.iter().map(|n| format!("{n:02x}")).collect(),
            expiry_secs: expiry,
            pay_to: self.node_id(),
        })
    }
    fn lookup(
        &self,
        _: [u8; 32],
    ) -> std::result::Result<Option<openagents_wallet::PaymentRecord>, openagents_wallet::WalletError>
    {
        if let Some((path, destination)) = self.lookup_destination.lock().unwrap().take() {
            pay_ledger::Ledger::open(path)
                .unwrap()
                .register_payee(destination)
                .unwrap();
        }
        Ok(self.payment.lock().unwrap().clone())
    }
    fn pay(
        &self,
        _: &str,
        _: u64,
        _: std::time::Duration,
    ) -> std::result::Result<openagents_wallet::Proof, openagents_wallet::WalletError> {
        panic!("receiver fixture must never pay")
    }
    fn balance(
        &self,
    ) -> std::result::Result<openagents_wallet::Balance, openagents_wallet::WalletError> {
        panic!("no balance read needed")
    }
    fn channels(
        &self,
    ) -> std::result::Result<Vec<openagents_wallet::Channel>, openagents_wallet::WalletError> {
        panic!("no channel read needed")
    }
    fn funding_address(&self) -> std::result::Result<String, openagents_wallet::WalletError> {
        panic!("no address needed")
    }
    fn open_channel(
        &self,
        _: &str,
        _: &str,
        _: u64,
        _: bool,
    ) -> std::result::Result<String, openagents_wallet::WalletError> {
        panic!("no channel action")
    }
    fn close_channel(
        &self,
        _: &str,
        _: &str,
        _: bool,
    ) -> std::result::Result<(), openagents_wallet::WalletError> {
        panic!("no channel action")
    }
}
fn paid_funding(
    provider: &mut store::Store,
    f: &Fixture,
    p: &super::paid_pipeline::Fixture,
    evidence: &Blobs,
    root: &Path,
) {
    use openagents_wallet::{PaymentDirection, PaymentStatus};
    let wallet = FakeWallet {
        now: f.now,
        payment: Default::default(),
        issued: Default::default(),
        lookup_destination: Default::default(),
    };
    let mut ledger = pay_ledger::Ledger::open(root.join("central.sqlite")).unwrap();
    let setup = f.setup.paid.as_ref().unwrap();
    let acceptance = provider.book.records.acceptance.take();
    assert!(
        provider
            .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
            .is_err()
    );
    assert_eq!(wallet.issued.load(std::sync::atomic::Ordering::Relaxed), 0);
    provider.book.records.acceptance = acceptance;
    let checked_path = setup.checker.labor_directory.join("labor.json");
    let checked_bytes = std::fs::read(&checked_path).unwrap();
    let mut forged: Value = serde_json::from_slice(&checked_bytes).unwrap();
    forged["paid"]["checker"]["observation"]["run"]["admission"]["grant"]["arguments"] =
        json!(["expected.rs", "expected.rs"]);
    std::fs::write(&checked_path, serde_json::to_vec(&forged).unwrap()).unwrap();
    assert!(
        provider
            .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
            .is_err()
    );
    assert_eq!(wallet.issued.load(std::sync::atomic::Ordering::Relaxed), 0);
    std::fs::write(&checked_path, checked_bytes).unwrap();
    // An interrupted external invoice request is not permission to issue a
    // replacement. This is a persisted intent fault fixture, not a crash run.
    let before_invoice = provider.document.paid.clone();
    provider.document.paid.invoice_preparation_started = true;
    provider.save().unwrap();
    assert!(
        provider
            .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
            .is_err()
    );
    assert_eq!(wallet.issued.load(std::sync::atomic::Ordering::Relaxed), 0);
    provider.document.paid = before_invoice;
    provider.save().unwrap();
    ledger
        .register_payee(pay_ledger::Payee {
            party: setup.worker.provider.clone(),
            destination_kind: setup.destination_kind.clone(),
            destination_value: setup.destination_value.clone(),
            source: "synthetic independent admitted provider".into(),
            verified_at: f.now as i64,
        })
        .unwrap();
    let invoice = provider
        .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
        .unwrap();
    assert_eq!(
        provider
            .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
            .unwrap(),
        invoice
    );
    assert_eq!(wallet.issued.load(std::sync::atomic::Ordering::Relaxed), 1);
    assert_eq!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap()
            .funding_state
            .as_deref(),
        Some("unknown")
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    let payment = openagents_wallet::PaymentRecord {
        payment_hash: invoice.payment_hash.clone(),
        direction: PaymentDirection::Inbound,
        status: PaymentStatus::Succeeded,
        amount_msat: Some(10_000),
        // Match wallet::ldk::record for a succeeded inbound Bolt11 receive:
        // it retains amount/hash/time/preimage but no invoice or routing fee.
        fee_msat: None,
        preimage: Some("07".repeat(32)),
        bolt11: None,
        updated_at: f.now,
    };
    let previous = provider.document.paid.clone();
    let mut failed = payment.clone();
    failed.status = PaymentStatus::Failed;
    *wallet.payment.lock().unwrap() = Some(failed);
    assert_eq!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap()
            .funding_state
            .as_deref(),
        Some("failed")
    );
    *wallet.payment.lock().unwrap() = Some(payment.clone());
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .is_err()
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    // Independent fault branch restores the unknown journal snapshot; the
    // production API never clears a known failure to try another payment.
    provider.document.paid = previous;
    provider.save().unwrap();
    let mut mismatched_invoice = payment.clone();
    mismatched_invoice.bolt11 = Some("a different retained invoice".into());
    *wallet.payment.lock().unwrap() = Some(mismatched_invoice);
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .is_err()
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    let poisoned_path = root.join("poisoned-provider");
    drop(store::Store::open(&poisoned_path, f.setup.clone(), f.provider).unwrap());
    std::fs::write(
        poisoned_path.join("labor.json"),
        std::fs::read(provider.dir.join("labor.json")).unwrap(),
    )
    .unwrap();
    let mut poisoned = store::Store::open(&poisoned_path, f.setup.clone(), f.provider).unwrap();
    let held_lock = poisoned_path.join("held.lock");
    std::fs::rename(poisoned_path.join("labor.lock"), &held_lock).unwrap();
    private_bytes(&poisoned_path.join("labor.lock"), b"");
    assert!(poisoned.save().is_err());
    std::fs::rename(held_lock, poisoned_path.join("labor.lock")).unwrap();
    assert!(
        poisoned
            .prepare_worker_invoice(&mut authority(p, evidence), &wallet, f.now)
            .unwrap_err()
            .contains("reopening")
    );
    *wallet.payment.lock().unwrap() = Some(payment.clone());
    assert!(
        poisoned
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap_err()
            .contains("reopening")
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    drop(poisoned);
    let mut bad = payment.clone();
    bad.amount_msat = Some(10_001);
    *wallet.payment.lock().unwrap() = Some(bad);
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .is_err()
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    *wallet.payment.lock().unwrap() = Some(payment.clone());
    *wallet.lookup_destination.lock().unwrap() = Some((
        root.join("central.sqlite"),
        pay_ledger::Payee {
            party: setup.worker.provider.clone(),
            destination_kind: setup.destination_kind.clone(),
            destination_value: secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &key(4))
                .to_string(),
            source: "synthetic lookup-time destination change".into(),
            verified_at: f.now as i64,
        },
    ));
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap_err()
            .contains("payout destination")
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    ledger
        .register_payee(pay_ledger::Payee {
            party: setup.worker.provider.clone(),
            destination_kind: setup.destination_kind.clone(),
            destination_value: setup.destination_value.clone(),
            source: "synthetic restored admitted destination".into(),
            verified_at: f.now as i64,
        })
        .unwrap();
    p.write_grant(setup, f.now, false);
    *wallet.payment.lock().unwrap() = Some(payment.clone());
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .is_err()
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    let mut destination = setup.clone();
    destination.destination_value =
        secp256k1::PublicKey::from_secret_key(&Secp256k1::new(), &key(4)).to_string();
    p.write_grant(&destination, f.now, true);
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .is_err()
    );
    assert!(ledger.settlement(&invoice.payment_hash).unwrap().is_none());
    p.write_grant(setup, f.now, true);
    let state = provider
        .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
        .unwrap();
    assert_eq!(state.funding_state.as_deref(), Some("funded"));
    assert!(state.funding_observation.as_ref().unwrap().bolt11.is_none());
    assert!(
        state
            .funding_observation
            .as_ref()
            .unwrap()
            .fee_msat
            .is_none()
    );
    assert!(provider.paid_report().unwrap()["all_in_known_msat"].is_null());
    assert_eq!(
        ledger
            .settlement_liabilities(&invoice.payment_hash)
            .unwrap()
            .iter()
            .map(|s| s.amount_msat)
            .sum::<i64>(),
        10_000
    );
    assert_eq!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap()
            .settlement_key,
        state.settlement_key
    );
    let mut rehydrated = payment.clone();
    rehydrated.bolt11 = Some(invoice.bolt11.clone());
    *wallet.payment.lock().unwrap() = Some(rehydrated);
    assert_eq!(
        json!(
            provider
                .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
                .unwrap()
        ),
        json!(state)
    );
    *wallet.payment.lock().unwrap() = Some(payment.clone());
    for (costs, expected) in [
        (
            json!({"coordination_msat":50,"execution_msat":20,"checker_msat":10,"failed_attempts_msat":30,"payment_fees_msat":0,"evidence":[p.grant_evidence]}),
            Some(10_110),
        ),
        (
            json!({"coordination_msat":null,"execution_msat":null,"checker_msat":null,"failed_attempts_msat":null,"payment_fees_msat":null,"evidence":[p.grant_evidence]}),
            None,
        ),
    ] {
        let notice = json!({"v":crate::paid::NOTICE_SCHEMA,"requires":[],"issuer":public(&f.provider),"order":order_value(&provider.book),"kind":"costs","responsible_human":setup.partner.support_human,"next_action":"Retain synthetic declared cost coverage; actual operator costs remain unqualified.","due_at":f.now+20,"evidence":[p.grant_evidence],"costs":costs});
        provider
            .receive(
                sealed(
                    &notice,
                    crate::paid::NOTICE_SCHEMA,
                    &f.provider,
                    &f.buyer,
                    f.now,
                ),
                f.now,
                p.evidence.clone(),
            )
            .unwrap();
        assert_eq!(
            provider.paid_report().unwrap()["all_in_known_msat"],
            json!(expected)
        );
    }
    let notice = json!({"v":crate::paid::NOTICE_SCHEMA,"requires":[],"issuer":public(&f.buyer),"order":order_value(&provider.book),"kind":"cancel","responsible_human":setup.partner.support_human,"next_action":"Keep accepted payment obligation; stop any new work.","due_at":f.now+20,"evidence":[p.grant_evidence],"costs":null});
    provider
        .receive(
            sealed(
                &notice,
                crate::paid::NOTICE_SCHEMA,
                &f.buyer,
                &f.provider,
                f.now,
            ),
            f.now,
            p.evidence.clone(),
        )
        .unwrap();
    assert!(provider.book.paid_halted());
    assert_eq!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now)
            .unwrap()
            .funding_state
            .as_deref(),
        Some("funded")
    );
    let mut changed = payment;
    changed.updated_at += 1;
    *wallet.payment.lock().unwrap() = Some(changed);
    assert!(
        provider
            .reconcile_worker_funding(&mut authority(p, evidence), &wallet, &mut ledger, f.now + 1)
            .is_err()
    );
    assert_eq!(
        ledger
            .settlement_liabilities(&invoice.payment_hash)
            .unwrap()
            .iter()
            .map(|s| s.amount_msat)
            .sum::<i64>(),
        10_000
    );
}

#[tokio::test]
async fn paid_running_command_stops_on_signed_cancellation_zero_rework_or_current_revocation() {
    for mode in ["cancel", "request_rework", "revoke"] {
        let root = tempfile::tempdir().unwrap();
        let workspace = repository(
            root.path(),
            "provider",
            "answer.rs",
            b"pub fn answer() -> u32 { 41 }\n",
        );
        let checker_workspace = repository(
            root.path(),
            "buyer-check",
            "expected.rs",
            b"pub fn answer() -> u32 { 42 }\n",
        );
        let task_intent = intent(&workspace, "One bounded paid synthetic command.");
        let grant = make_grant(
            &task_intent,
            "labor-request",
            Path::new("/bin/sh"),
            vec![
                "-c".into(),
                "sleep 3; printf 'pub fn answer() -> u32 { 42 }\\n' > answer.rs".into(),
            ],
        );
        let input = json!({"v":"coder.free-labor.command.v1","requires":[],"intent":task_intent,"source_snapshot":grant["expected_source_snapshot"],"expected_output_digest":nostr::contracts::digest_bytes(b"pub fn answer() -> u32 { 42 }\n")});
        let requirements = json!({"v":"coder.free-labor.requirements.v1","requires":[],"program_digest":nostr::contracts::digest_bytes(&std::fs::read(grant["program"].as_str().unwrap()).unwrap()),"arguments_digest":nostr::contracts::digest_bytes(&jcs(&grant["arguments"]).unwrap()),"write_workspace":true,"wall_seconds":10,"stream_bytes":4096,"memory_bytes":268435456});
        let mut f = fixture_with_paid(Some((input, requirements)), true);
        let p = super::paid_pipeline::Fixture::new(root.path(), f.now, 10_000);
        let buyer_path = root.path().join("buyer");
        let setup = paid_setup(&f, &p, root.path(), &checker_workspace, &buyer_path);
        p.write_grant(&setup, f.now, true);
        f.setup.paid = Some(setup.clone());
        let mut evidence = f.blobs.clone();
        for (k, v) in &p.evidence.0 {
            evidence.0.insert(k.clone(), v.clone());
        }
        let provider_path = root.path().join("provider");
        let mut provider = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
        for event in &f.events {
            provider
                .receive(event.clone(), f.now, Blobs::default())
                .unwrap();
        }
        let mut book = agreed(&f, f.provider);
        let execute = link(&mut f, &mut book);
        let linkage = book
            .records
            .resolve(book.records.link.as_ref().unwrap())
            .unwrap()
            .clone();
        provider
            .receive(
                sealed(&linkage, records::LINK, &f.buyer, &f.provider, f.now),
                f.now,
                f.blobs.clone(),
            )
            .unwrap();
        let note = json!({"v":crate::paid::NOTICE_SCHEMA,"requires":[],"issuer":public(&f.buyer),"order":order_value(&provider.book),"kind":if mode=="revoke" {"cancel"} else {mode},"responsible_human":setup.partner.support_human,"next_action":"Stop this unaccepted work; preserve its execution and costs for support review.","due_at":f.now+30,"evidence":[p.grant_evidence],"costs":null});
        let event = sealed(
            &note,
            crate::paid::NOTICE_SCHEMA,
            &f.buyer,
            &f.provider,
            f.now,
        );
        let tasks = root.path().join("provider-tasks");
        let grant_bytes = serde_json::to_vec(&grant).unwrap();
        let mut admitted = authority(&p, &evidence);
        let running =
            provider.dispatch_paid(&mut admitted, execute.clone(), &grant_bytes, &tasks, f.now);
        let narrowing = async {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if mode == "revoke" {
                p.write_grant(&setup, f.now, false);
            } else {
                store::Store::queue_paid_notice(
                    &provider_path,
                    f.setup.clone(),
                    f.provider,
                    event,
                    p.evidence.clone(),
                    f.now,
                )
                .unwrap();
            }
        };
        let (result, ()) = tokio::join!(running, narrowing);
        let result = result.unwrap();
        assert_ne!(result.state, "finished", "{mode}");
        assert!(provider.book.records.acceptance.is_none());
        let actual = task::owner::recover(&tasks, "labor-request").unwrap();
        assert!(
            actual
                .run
                .as_ref()
                .unwrap()
                .result
                .as_ref()
                .unwrap()
                .stop_requested,
            "{mode}"
        );
        assert_eq!(
            std::fs::read(workspace.join("answer.rs")).unwrap(),
            b"pub fn answer() -> u32 { 41 }\n"
        );
        assert!(
            provider
                .prepare_worker_invoice(
                    &mut authority(&p, &evidence),
                    &FakeWallet {
                        now: f.now,
                        payment: Default::default(),
                        issued: Default::default(),
                        lookup_destination: Default::default()
                    },
                    f.now
                )
                .is_err()
        );
        if mode != "revoke" {
            assert!(provider.book.paid_halted());
            assert_eq!(provider.book.paid_notices.len(), 1);
        } else {
            assert!(provider.paid_state().interruption.is_some());
        }
        drop(provider);
        let reopened = store::Store::open(&provider_path, f.setup.clone(), f.provider).unwrap();
        assert!(reopened.book.records.acceptance.is_none());
        if mode != "revoke" {
            assert!(reopened.book.paid_halted());
        }
    }
}

impl openagents_wallet::resident::Served for FakeWallet {
    fn status(&self) -> Value {
        json!({"fixture":true,"node_id":openagents_wallet::LightningWallet::node_id(self)})
    }
    fn buy_channel(
        &self,
        _: u64,
        _: u64,
        _: u32,
        _: bool,
    ) -> std::result::Result<Value, openagents_wallet::WalletError> {
        panic!("no channel action")
    }
    fn channel_order(&self, _: &str) -> std::result::Result<Value, openagents_wallet::WalletError> {
        panic!("no channel action")
    }
    fn send_onchain(
        &self,
        _: &str,
        _: u64,
    ) -> std::result::Result<String, openagents_wallet::WalletError> {
        panic!("no outgoing payment")
    }
}
fn private_bytes(path: &Path, bytes: &[u8]) {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(bytes).unwrap();
    file.sync_all().unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
}
fn cli_paid(
    binary: &Path,
    f: &Fixture,
    p: &super::paid_pipeline::Fixture,
    evidence: &Blobs,
    root: &Path,
    provider: &Path,
    buyer: &Path,
    grant: &Value,
) {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let cli_home = root.join("cli-home");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&cli_home)
        .unwrap();
    let keys = root.join("cli-keys");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&keys)
        .unwrap();
    private_bytes(
        &keys.join("buyer.key"),
        f.buyer.display_secret().to_string().as_bytes(),
    );
    private_bytes(
        &keys.join("provider.key"),
        f.provider.display_secret().to_string().as_bytes(),
    );
    let books = root.join("cli-books");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&books)
        .unwrap();
    for (name, source) in [("buyer", buyer), ("provider", provider)] {
        let dir = books.join(name);
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        private_bytes(
            &dir.join("setup.json"),
            &serde_json::to_vec(&f.setup).unwrap(),
        );
        let journal = dir.join("journal");
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&journal)
            .unwrap();
        for file in ["labor.json", "labor.lock"] {
            private_bytes(
                &journal.join(file),
                &std::fs::read(source.join(file)).unwrap(),
            );
        }
    }
    let sources = root.join("cli-authority-evidence.json");
    private_bytes(&sources, &serde_json::to_vec(evidence).unwrap());
    let wallet_home = root.join("cli-wallet");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&wallet_home)
        .unwrap();
    // Darwin's Unix socket path bound is shorter than the retained session
    // root. A private short alias changes no wallet custody or source path.
    let scratch =
        PathBuf::from(std::env::var_os("OPENAGENTS_SCRATCH").expect("leased scratch root"));
    let alias = tempfile::Builder::new()
        .prefix("lus-")
        .tempdir_in(scratch.parent().unwrap())
        .unwrap();
    std::os::unix::fs::symlink(root, alias.path().join("s")).unwrap();
    let wallet_home = alias.path().join("s/cli-wallet");
    let document: Value =
        serde_json::from_slice(&std::fs::read(provider.join("labor.json")).unwrap()).unwrap();
    let payment: openagents_wallet::PaymentRecord =
        serde_json::from_value(document["paid"]["funding_observation"].clone()).unwrap();
    let wallet = std::sync::Arc::new(FakeWallet {
        now: f.now,
        payment: std::sync::Mutex::new(Some(payment)),
        issued: Default::default(),
        lookup_destination: Default::default(),
    });
    let server = openagents_wallet::resident::Server::bind(&wallet_home).unwrap();
    let stop = server.stop_flag();
    let serving = std::thread::spawn(move || server.run(wallet));
    let ledger = root.join("central.sqlite");
    std::fs::set_permissions(&ledger, std::fs::Permissions::from_mode(0o600)).unwrap();
    let invoke = |profile: &str, args: &[&str], authorized: bool| {
        let mut command = std::process::Command::new(binary);
        command
            .args(["--json", "labor"])
            .args(args)
            .args(["--as", profile]);
        if authorized {
            command
                .arg("--pipeline")
                .arg(&p.host)
                .arg("--credential")
                .arg(&p.owner_credential)
                .arg("--authority-evidence")
                .arg(&sources);
        }
        command
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .env("HOME", &cli_home)
            .env("VERSE_HOME", &keys)
            .env("LABOR_HOME", &books)
            .env("OPENAGENTS_TASKS", cli_home.join("tasks"))
            .current_dir(&cli_home)
            .output()
            .unwrap()
    };
    let checked = invoke("buyer", &["verify", "buyer"], true);
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&checked.stdout).unwrap()["checker_passed"],
        true
    );
    let invoice = invoke(
        "provider",
        &[
            "invoice",
            "provider",
            "--wallet-home",
            wallet_home.to_str().unwrap(),
        ],
        true,
    );
    assert!(
        invoice.status.success(),
        "{}",
        String::from_utf8_lossy(&invoice.stderr)
    );
    for _ in 0..2 {
        let funded = invoke(
            "provider",
            &[
                "fund",
                "provider",
                "--wallet-home",
                wallet_home.to_str().unwrap(),
                "--ledger",
                ledger.to_str().unwrap(),
            ],
            true,
        );
        assert!(
            funded.status.success(),
            "{}",
            String::from_utf8_lossy(&funded.stderr)
        );
        let body: Value = serde_json::from_slice(&funded.stdout).unwrap();
        assert_eq!(body["funding"]["funding_state"], "funded");
        assert!(body["funding"]["funding_observation"]["preimage"].is_null());
        assert!(body["paid"]["all_in_known_msat"].is_null());
    }
    let source = root.join("cli-execute.json");
    private_bytes(
        &source,
        &serde_json::to_vec(&document["dispatch"]["execute"]).unwrap(),
    );
    let grant_file = root.join("cli-grant.json");
    private_bytes(&grant_file, &serde_json::to_vec(grant).unwrap());
    let refused = invoke(
        "provider",
        &[
            "execute",
            "provider",
            source.to_str().unwrap(),
            "--grant",
            grant_file.to_str().unwrap(),
        ],
        false,
    );
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stdout).contains("requires --pipeline"));
    let replay = invoke(
        "provider",
        &[
            "execute",
            "provider",
            source.to_str().unwrap(),
            "--grant",
            grant_file.to_str().unwrap(),
        ],
        true,
    );
    // A postacceptance cancellation preserves funding but refuses new dispatch.
    assert!(!replay.status.success());
    let state = invoke("provider", &["check", "provider"], false);
    assert!(state.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&state.stdout).unwrap()["paid"]["funding"]["funding_state"],
        "funded"
    );
    let ledger = pay_ledger::Ledger::open_read_only(&ledger).unwrap();
    let hash = document["paid"]["invoice"]["payment_hash"]
        .as_str()
        .unwrap();
    assert_eq!(
        ledger
            .settlement_liabilities(hash)
            .unwrap()
            .iter()
            .map(|s| s.amount_msat)
            .sum::<i64>(),
        10_000
    );
    stop.store(true, std::sync::atomic::Ordering::Relaxed);
    serving.join().unwrap();
}
