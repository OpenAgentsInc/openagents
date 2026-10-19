use super::*;
use std::fs;
use tempfile::TempDir;
const NOW: u64 = 1_900_000_000;
fn clock() -> u64 {
    NOW
}
fn later() -> u64 {
    NOW + 200
}
fn retained(root: &Path, path: &str, bytes: &[u8]) -> Reference {
    fs::write(root.join(path), bytes).unwrap();
    Reference {
        path: path.into(),
        sha256: digest(bytes),
    }
}
fn pin(name: &str, revision: u64) -> Pin {
    Pin {
        id: name.into(),
        revision,
    }
}
fn scope() -> Scope {
    Scope {
        product: "Coder".into(),
        offer_version: intake::OFFER.into(),
        release: "a".repeat(40),
    }
}
fn fixture() -> (TempDir, Store, Access, SourceInput) {
    let dir = TempDir::new().unwrap();
    let mut store = Store::open_with_clock(&dir.path().join("host"), clock).unwrap();
    let credential = dir.path().join("owner-token");
    store.initialize("owner", &credential).unwrap();
    let access = store
        .authenticate(&Store::read_credential(&credential).unwrap())
        .unwrap();
    let input = SourceInput {
        schema: SOURCE_SCHEMA.into(),
        pin: pin("supported", 1),
        scope: scope(),
        root: dir.path().into(),
        evidence: Evidence::Capability {
            contract: retained(dir.path(), "contract", b"synthetic maintained contract"),
            reviewed_fact: "Coder retains a checked patch for this scoped workflow.".into(),
        },
        readiness: Readiness::Implemented,
        limits: vec!["One declared repository, checks, supported client, and payer.".into()],
        review: retained(
            dir.path(),
            "source-review",
            b"synthetic owner review of exact source and disclosure rights",
        ),
        expires_at: NOW + 100,
        activation: None,
    };
    (dir, store, access, input)
}
fn claim(dir: &TempDir, source: &Pin, name: &str, purpose: Purpose) -> ClaimInput {
    ClaimInput {
        pin: pin(name, 1),
        source: source.clone(),
        purpose,
        playbook: retained(dir.path(), "playbook", b"reviewed playbook v1"),
        expires_at: NOW + 100,
        review: retained(
            dir.path(),
            &format!("claim-review-{name}"),
            b"synthetic owner review of exact claim",
        ),
    }
}
fn allowed(view: &ClaimView) -> &Allowed {
    match &view.verdict {
        Verdict::Allowed { claim } => claim,
        other => panic!("expected allowed: {other:?}"),
    }
}
fn reason(view: &ClaimView) -> Rejection {
    rejection(&view.verdict).unwrap()
}
fn draft(store: &mut Store, access: &Access, name: &str) -> Draft {
    let view = store
        .current_claims(access, &[pin(name, 1)], &scope().release)
        .unwrap()
        .remove(0);
    store
        .compose_claim_draft(
            access,
            "draft",
            vec![DraftPin {
                claim: view.pin,
                reviewed_sha256: view.reviewed_sha256.unwrap(),
            }],
            &scope().release,
        )
        .unwrap()
}
#[test]
fn reviewed_capability_and_restart_keep_exact_owner_history_without_source_paths() {
    let (dir, mut store, access, input) = fixture();
    let src = store.review_claim_source(&access, input.clone()).unwrap();
    assert_eq!(
        store
            .review_claim_source(&access, input)
            .unwrap()
            .input_sha256,
        src.input_sha256
    );
    let c = claim(&dir, &src.input.pin, "fact", Purpose::Capability);
    store.review_claim(&access, c.clone()).unwrap();
    let d = draft(&mut store, &access, "fact");
    assert!(d.content.contains("One declared repository"));
    assert!(!d.content.contains(dir.path().to_str().unwrap()));
    let history = store.claim_history(&access, 0, 100).unwrap();
    assert_eq!(history.len(), 2);
    assert!(
        !serde_json::to_string(&history)
            .unwrap()
            .contains("synthetic maintained contract")
    );
    let secret = Store::read_credential(&dir.path().join("owner-token")).unwrap();
    drop(store);
    let mut store = Store::open_with_clock(&dir.path().join("host"), clock).unwrap();
    let access = store.authenticate(&secret).unwrap();
    assert_eq!(
        store
            .validate_claim_draft(&access, "draft", &scope().release)
            .unwrap()
            .sha256,
        d.sha256
    );
    assert_eq!(
        store.review_claim(&access, c).unwrap().input.pin,
        pin("fact", 1)
    );
    assert_eq!(store.claim_history(&access, 0, 100).unwrap().len(), 2);
}
#[test]
fn current_source_playbook_release_expiry_and_withdrawal_invalidate_drafts() {
    let (dir, mut store, access, input) = fixture();
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(
            &access,
            claim(&dir, &input.pin, "fact", Purpose::Capability),
        )
        .unwrap();
    draft(&mut store, &access, "fact");
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("fact", 1)], &"b".repeat(40))
                .unwrap()[0]
        ),
        Rejection::ReleaseMismatch
    );
    fs::write(dir.path().join("playbook"), b"changed playbook").unwrap();
    assert!(
        store
            .validate_claim_draft(&access, "draft", &scope().release)
            .unwrap_err()
            .contains("ChangedEvidence")
    );
    fs::write(dir.path().join("playbook"), b"reviewed playbook v1").unwrap();
    fs::remove_file(dir.path().join("contract")).unwrap();
    assert!(
        store
            .validate_claim_draft(&access, "draft", &scope().release)
            .unwrap_err()
            .contains("MissingEvidence")
    );
    fs::write(
        dir.path().join("contract"),
        b"synthetic maintained contract",
    )
    .unwrap();
    store
        .withdraw_claim_revision(&access, &input.pin, true, "capability withdrawn")
        .unwrap();
    store
        .withdraw_claim_revision(&access, &input.pin, true, "same retry")
        .unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("fact", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::Withdrawn
    );
    let history = store.claim_history(&access, 0, 100).unwrap();
    assert_eq!(
        history.iter().filter(|r| r.operation == "withdraw").count(),
        1
    );
    assert!(
        history
            .iter()
            .any(|r| r.reason == Some(Rejection::ChangedEvidence))
    );
    assert!(
        history
            .iter()
            .any(|r| r.reason == Some(Rejection::MissingEvidence))
    );
    store.clock = later;
    // Withdrawal remains stronger than expiry; a new nonwithdrawn revision expires.
    let mut second = input;
    second.pin = pin("supported", 2);
    second.expires_at = NOW + 300;
    store.clock = clock;
    store.review_claim_source(&access, second.clone()).unwrap();
    let mut second_claim = claim(&dir, &second.pin, "second", Purpose::Capability);
    second_claim.expires_at = NOW + 100;
    store.review_claim(&access, second_claim).unwrap();
    store.clock = later;
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("second", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::Expired
    );
}
#[test]
fn rejected_claims_are_retained_and_cannot_gain_approval_or_accept_launch_language() {
    let (dir, mut store, access, mut input) = fixture();
    if let Evidence::Capability { reviewed_fact, .. } = &mut input.evidence {
        *reviewed_fact = "Guaranteed savings, launch ready.".into();
    }
    let rejected_source = store.review_claim_source(&access, input.clone()).unwrap();
    assert!(matches!(
        rejected_source.reviewed,
        Verdict::Rejected {
            reason: Rejection::UnsupportedWording
        }
    ));
    assert_eq!(
        store.claim_history(&access, 0, 100).unwrap()[0].reason,
        Some(Rejection::UnsupportedWording)
    );
    input.pin.revision = 2;
    if let Evidence::Capability { reviewed_fact, .. } = &mut input.evidence {
        *reviewed_fact = "The pilot costs USD 250.".into();
    }
    assert!(matches!(
        store
            .review_claim_source(&access, input.clone())
            .unwrap()
            .reviewed,
        Verdict::Rejected {
            reason: Rejection::UnsupportedWording
        }
    ));
    if let Evidence::Capability { reviewed_fact, .. } = &mut input.evidence {
        *reviewed_fact = "Coder retains a checked patch for this scoped workflow.".into();
    }
    assert!(
        store
            .review_claim_source(&access, input.clone())
            .unwrap_err()
            .contains("immutable")
    );
    input.pin.revision = 3;
    store.review_claim_source(&access, input.clone()).unwrap();
    let launch = store
        .review_claim(&access, claim(&dir, &input.pin, "launch", Purpose::Launch))
        .unwrap();
    assert!(matches!(
        launch.reviewed,
        Verdict::Unavailable {
            reason: Rejection::UnqualifiedLaunch,
            ..
        }
    ));
    let mut absent = claim(&dir, &input.pin, "missing", Purpose::Capability);
    absent.review.path = "missing-review".into();
    let rejected = store.review_claim(&access, absent.clone()).unwrap();
    assert!(matches!(
        rejected.reviewed,
        Verdict::Rejected {
            reason: Rejection::MissingEvidence
        }
    ));
    fs::write(
        dir.path().join("missing-review"),
        b"synthetic owner review of exact claim",
    )
    .unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("missing", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::MissingEvidence
    );
    absent.expires_at += 1;
    assert!(
        store
            .review_claim(&access, absent)
            .unwrap_err()
            .contains("immutable")
    );
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("absent", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::UnknownClaim
    );
    let launch_view = store
        .current_claims(&access, &[pin("launch", 1)], &scope().release)
        .unwrap()
        .remove(0);
    assert!(
        store
            .compose_claim_draft(
                &access,
                "launch",
                vec![DraftPin {
                    claim: launch_view.pin,
                    reviewed_sha256: launch_view.reviewed_sha256.unwrap()
                }],
                &scope().release
            )
            .is_err()
    );
}
fn activate(dir: &TempDir, source: &mut SourceInput, primary: &str, payer: &str) {
    let qualification = retained(
        dir.path(),
        "qualification",
        b"synthetic exact funded/resource or commercial qualification; not owner activation",
    );
    let record = Activation {
        schema: "openagents.sales.claim-activation.v1".into(),
        scope: source.scope.clone(),
        source_sha256: primary.into(),
        reviewer: "owner".into(),
        approved: true,
        reviewed_at: NOW,
        expires_at: source.expires_at,
        payer: payer.into(),
        qualification,
    };
    source.activation = Some(retained(
        dir.path(),
        "activation",
        &serde_json::to_vec(&record).unwrap(),
    ));
    source.readiness = Readiness::Available;
}
#[test]
fn authoritative_retail_quotes_preserve_price_units_payers_caps_and_require_activation() {
    let (dir, mut store, access, mut input) = fixture();
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../route-contract/fixtures/price-book-v1.json"
    ));
    let book = retained(dir.path(), "retail-book", bytes);
    input.evidence = Evidence::RetailPrice {
        book: book.clone(),
        computer: "retail-boat-large-v1".into(),
        task: "retail-repo-change-v1".into(),
        max_seconds: 3600,
    };
    input.readiness = Readiness::Proposed;
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(&access, claim(&dir, &input.pin, "proposed", Purpose::Price))
        .unwrap();
    let unavailable = store
        .current_claims(&access, &[pin("proposed", 1)], &scope().release)
        .unwrap()
        .remove(0);
    assert!(matches!(
        unavailable.verdict,
        Verdict::Unavailable {
            reason: Rejection::ProposedCommercialTerms,
            proposed_price: Some(PriceTerms::Retail { .. })
        }
    ));
    input.pin.revision = 2;
    activate(&dir, &mut input, &book.sha256, "buyer");
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(&access, claim(&dir, &input.pin, "price", Purpose::Price))
        .unwrap();
    let view = store
        .current_claims(&access, &[pin("price", 1)], &scope().release)
        .unwrap()
        .remove(0);
    let Some(PriceTerms::Retail { quote }) = &allowed(&view).price else {
        panic!()
    };
    assert_eq!(quote.version, "retail-2026-10-06.1");
    assert_eq!(quote.max_sats, 244);
    assert_eq!(quote.lines.len(), 3);
    assert!(matches!(
        quote.lines[2].payer,
        route_contract::price_book::QuotePayer::CallerKey { .. }
    ));
    draft(&mut store, &access, "price");
    fs::write(
        dir.path().join("retail-book"),
        b"changed current price book",
    )
    .unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("price", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::ChangedEvidence
    );
    assert!(
        store
            .validate_claim_draft(&access, "draft", &scope().release)
            .is_err()
    );
    store.clock = later;
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("price", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::Expired
    );
}
#[test]
fn first_offer_is_proposed_even_after_review_until_exact_separate_commercial_activation() {
    let (dir, mut store, access, mut input) = fixture();
    let template = retained(
        dir.path(),
        "pilot-kit",
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../docs/sales/pilot-kit.json"
        )),
    );
    input.evidence = Evidence::PilotOffer {
        template: template.clone(),
    };
    input.readiness = Readiness::Proposed;
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(&access, claim(&dir, &input.pin, "proposed", Purpose::Price))
        .unwrap();
    let view = store
        .current_claims(&access, &[pin("proposed", 1)], &scope().release)
        .unwrap()
        .remove(0);
    let Verdict::Unavailable {
        proposed_price: Some(PriceTerms::Service { terms }),
        ..
    } = view.verdict
    else {
        panic!()
    };
    assert_eq!(terms.service_fee_minor_units, 25000);
    assert_eq!(terms.currency_scale, 100);
    assert_eq!(terms.promotional_credits, 0);
    input.pin.revision = 2;
    activate(&dir, &mut input, &template.sha256, "buyer");
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(
            &access,
            claim(&dir, &input.pin, "activated-fixture", Purpose::Price),
        )
        .unwrap();
    let view = store
        .current_claims(&access, &[pin("activated-fixture", 1)], &scope().release)
        .unwrap()
        .remove(0);
    assert!(
        allowed(&view)
            .wording
            .contains("buyer pays their provider separately")
    );
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("proposed", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::StaleRevision
    );
}
fn comparison(dir: &TempDir) -> (Reference, Reference, String) {
    use sales_evidence::*;
    let mut rows = Vec::new();
    for (name, status) in [
        ("baseline", Status::Passed),
        ("failed", Status::Failed),
        ("repair", Status::Passed),
    ] {
        let path = dir.path().join(format!("{name}.atif"));
        let session = atif::Session::opening(
            name,
            "synthetic-model",
            "synthetic-provider",
            "private-customer",
            "revision",
        );
        let mut log = atif::Log::create_at(&path, &session).unwrap();
        log.append(&atif::Step::said(
            atif::Source::User,
            "private-customer task",
        ))
        .unwrap();
        log.finish(atif::log::ENDED).unwrap();
        drop(log);
        let artifact = retained(dir.path(), &format!("{name}.patch"), name.as_bytes());
        let mut attempt = Attempt {
            id: name.into(),
            kind: if name == "repair" {
                AttemptKind::Repair
            } else {
                AttemptKind::Primary
            },
            parent: (name == "repair").then(|| "failed".into()),
            executor: "executor".into(),
            trace: Reference {
                path: format!("{name}.atif"),
                sha256: digest(&fs::read(path).unwrap()),
            },
            artifact: artifact.clone(),
            checks: BTreeMap::from([(
                "check".into(),
                Check {
                    check_digest: "c".repeat(64),
                    status,
                    evidence: Some(retained(
                        dir.path(),
                        &format!("{name}.check"),
                        b"frozen checks",
                    )),
                },
            )]),
            setup_ms: 10,
            queue_ms: 20,
            check_ms: 30,
            support_ms: 40,
            costs: vec![],
            compute: None,
            acceptance: None,
        };
        if name == "repair" {
            attempt.acceptance = Some(Acceptance {
                candidate_digest: artifact.sha256,
                independent_checker: "checker".into(),
                check_review: retained(dir.path(), "independent", b"independent exact checks"),
                customer_decision: retained(dir.path(), "customer", b"private-customer accepted"),
            });
            attempt.costs = vec![Cost {
                component: CostComponent::Provider,
                basis: CostBasis::Billed,
                unit: "USD_millionths".into(),
                amount: Some(17),
                evidence: Some(retained(dir.path(), "bill", b"synthetic bill")),
                price: None,
            }];
        }
        rows.push(attempt);
    }
    let manifest = Manifest {
        schema: SCHEMA.into(),
        offer_version: intake::OFFER.into(),
        source_revision: scope().release,
        baseline_method: "manual".into(),
        candidate_method: "Coder".into(),
        retrospective_selection: false,
        inventory: retained(
            dir.path(),
            "inventory",
            &serde_json::to_vec(&Inventory {
                schema: "openagents.gym.sales-inventory.v1".into(),
                attempts: BTreeMap::from([
                    ("task/baseline".into(), vec!["baseline".into()]),
                    (
                        "task/candidate".into(),
                        vec!["failed".into(), "repair".into()],
                    ),
                ]),
            })
            .unwrap(),
        ),
        gym_store: None,
        tasks: vec![Task {
            id: "task".into(),
            task_digest: "b".repeat(64),
            check_digests: BTreeMap::from([("check".into(), "c".repeat(64))]),
            baseline: vec![rows.remove(0)],
            candidate: rows,
        }],
        skipped_evidence: vec!["hosting invoice unknown".into()],
    };
    let bytes = serde_json::to_vec(&manifest).unwrap();
    let report = rebuild(dir.path(), &bytes).unwrap();
    let report_sha = digest(&serde_json::to_vec_pretty(&report).unwrap());
    let public = PublicReview {
        schema: "openagents.gym.sales-review.v1".into(),
        report_digest: report_sha.clone(),
        reviewer: "owner".into(),
        approved: true,
    };
    (
        retained(dir.path(), "comparison", &bytes),
        retained(
            dir.path(),
            "public-review",
            &serde_json::to_vec(&public).unwrap(),
        ),
        report_sha,
    )
}
#[test]
fn comparative_claims_rebuild_all_attempts_costs_checks_and_private_acceptance() {
    let (dir, mut store, access, mut input) = fixture();
    let (manifest, public_review, report_sha256) = comparison(&dir);
    input.evidence = Evidence::Comparison {
        manifest,
        public_review,
        report_sha256,
    };
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(
            &access,
            claim(&dir, &input.pin, "comparison", Purpose::Comparison),
        )
        .unwrap();
    let view = store
        .current_claims(&access, &[pin("comparison", 1)], &scope().release)
        .unwrap()
        .remove(0);
    let result = allowed(&view);
    let report = result.comparison.as_ref().unwrap();
    assert_eq!(report.candidate.attempts, 2);
    assert_eq!(report.candidate.repairs, 1);
    assert_eq!(report.candidate.failed_checks, 1);
    assert_eq!(report.candidate.accepted_tasks, 1);
    assert_eq!(report.candidate.setup_ms, 20);
    assert_eq!(report.candidate.queue_ms, 40);
    assert_eq!(report.candidate.check_ms, 60);
    assert_eq!(report.candidate.support_ms, 80);
    assert!(
        report
            .candidate
            .costs
            .iter()
            .any(|c| c.known_subtotal == 17 && c.basis == sales_evidence::CostBasis::Billed)
    );
    assert!(report.candidate.costs.iter().any(|c| c.unknown_items > 0));
    let output = serde_json::to_string(result).unwrap();
    for private in [
        "private-customer",
        "synthetic-model",
        "synthetic-provider",
        dir.path().to_str().unwrap(),
        "checker",
    ] {
        assert!(!output.contains(private));
    }
    draft(&mut store, &access, "comparison");
    fs::write(dir.path().join("failed.check"), b"changed failed check").unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("comparison", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::UnsupportedComparison
    );
    assert!(
        store
            .validate_claim_draft(&access, "draft", &scope().release)
            .is_err()
    );
    fs::write(dir.path().join("failed.check"), b"frozen checks").unwrap();
    fs::remove_file(dir.path().join("independent")).unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("comparison", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::UnsupportedComparison
    );
}
#[test]
fn source_and_claim_revisions_role_bounds_and_path_admission_are_enforced() {
    let (dir, mut store, access, input) = fixture();
    store.review_claim_source(&access, input.clone()).unwrap();
    store
        .review_claim(
            &access,
            claim(&dir, &input.pin, "fact", Purpose::Capability),
        )
        .unwrap();
    let reader_file = dir.path().join("reader-token");
    store
        .issue(&access, "reader", Role::Reader, &reader_file)
        .unwrap();
    let reader = store
        .authenticate(&Store::read_credential(&reader_file).unwrap())
        .unwrap();
    assert!(
        store
            .current_claims(&reader, &[pin("fact", 1)], &scope().release)
            .is_ok()
    );
    assert!(store.review_claim_source(&reader, input.clone()).is_err());
    assert!(
        store
            .withdraw_claim_revision(&reader, &input.pin, true, "reason")
            .is_err()
    );
    assert!(store.claim_history(&reader, 0, 10).is_err());
    assert!(
        store
            .current_claims(&access, &vec![pin("fact", 1); 9], &scope().release)
            .is_err()
    );
    let mut second = input.clone();
    second.pin.revision = 2;
    store.review_claim_source(&access, second).unwrap();
    assert_eq!(
        reason(
            &store
                .current_claims(&access, &[pin("fact", 1)], &scope().release)
                .unwrap()[0]
        ),
        Rejection::StaleRevision
    );
    let mut different = input.clone();
    different.limits.push("changed".into());
    assert!(
        store
            .review_claim_source(&access, different)
            .unwrap_err()
            .contains("immutable")
    );
    let reference = Reference {
        path: "../outside".into(),
        sha256: "a".repeat(64),
    };
    assert_eq!(
        read(dir.path(), &reference).unwrap_err(),
        Rejection::MissingEvidence
    );
    fs::write(dir.path().join("empty"), b"").unwrap();
    assert_eq!(
        read(
            dir.path(),
            &Reference {
                path: "empty".into(),
                sha256: digest(b"")
            }
        )
        .unwrap_err(),
        Rejection::MissingEvidence
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(dir.path().join("contract"), dir.path().join("linked")).unwrap();
        let reference = Reference {
            path: "linked".into(),
            sha256: digest(b"synthetic maintained contract"),
        };
        assert_eq!(
            read(dir.path(), &reference).unwrap_err(),
            Rejection::MissingEvidence
        );
    }
    fs::write(dir.path().join("oversized"), vec![b'x'; MAX_FILE + 1]).unwrap();
    assert_eq!(
        read(
            dir.path(),
            &Reference {
                path: "oversized".into(),
                sha256: "a".repeat(64)
            }
        )
        .unwrap_err(),
        Rejection::MissingEvidence
    );
    store.revoke(&access, "reader").unwrap();
    assert!(
        store
            .current_claims(&reader, &[pin("fact", 1)], &scope().release)
            .is_err()
    );
}

#[test]
fn a_reused_price_version_cannot_acquire_different_terms_under_another_source_id() {
    let (dir, mut store, access, mut input) = fixture();
    let mut fixture: serde_json::Value = serde_json::from_slice(include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../route-contract/fixtures/price-book-v1.json"
    )))
    .unwrap();
    let first = retained(
        dir.path(),
        "retail-current",
        &serde_json::to_vec(&fixture).unwrap(),
    );
    input.evidence = Evidence::RetailPrice {
        book: first,
        computer: "retail-boat-large-v1".into(),
        task: "retail-repo-change-v1".into(),
        max_seconds: 3600,
    };
    input.readiness = Readiness::Proposed;
    store.review_claim_source(&access, input.clone()).unwrap();
    fixture["book"]["classes"][0]["compute_msats_per_second"] = serde_json::json!(41);
    let changed = retained(
        dir.path(),
        "retail-current",
        &serde_json::to_vec(&fixture).unwrap(),
    );
    input.pin = pin("different-source", 1);
    if let Evidence::RetailPrice { book, .. } = &mut input.evidence {
        *book = changed;
    }
    let rejected = store.review_claim_source(&access, input.clone()).unwrap();
    assert!(matches!(
        rejected.reviewed,
        Verdict::Rejected {
            reason: Rejection::PriceVersionReuse
        }
    ));
    fixture["book"]["version"] = serde_json::json!("retail-2030-01-01.2");
    let next = retained(
        dir.path(),
        "retail-current",
        &serde_json::to_vec(&fixture).unwrap(),
    );
    input.pin.revision = 2;
    if let Evidence::RetailPrice { book, .. } = &mut input.evidence {
        *book = next;
    }
    assert!(matches!(
        store.review_claim_source(&access, input).unwrap().reviewed,
        Verdict::Unavailable {
            reason: Rejection::ProposedCommercialTerms,
            ..
        }
    ));
    assert!(
        store
            .claim_history(&access, 0, 100)
            .unwrap()
            .iter()
            .any(|r| r.reason == Some(Rejection::PriceVersionReuse))
    );
}
