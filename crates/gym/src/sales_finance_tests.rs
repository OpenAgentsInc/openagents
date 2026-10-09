use super::*;
use crate::sales_evidence::{self as evidence, Cost, CostBasis, CostComponent};
use pay_ledger::{Ledger, Rail, SettlementInput, Split};
use std::os::unix::fs::PermissionsExt;
use tempfile::TempDir;
mod service_sources {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../receipts/tests/support/service_sale.rs"
    ));
}

const AT: u64 = 1_790_986_000;
fn retained(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    fs::write(root.join(name), bytes).unwrap();
    Reference {
        path: name.into(),
        sha256: evidence::digest(bytes),
    }
}
fn reference<T: Serialize>(root: &Path, name: &str, value: &T) -> Reference {
    retained(root, name, &serde_json::to_vec(value).unwrap())
}
fn expense_record(root: &Path, id: &str, class: ExpenseClass, amount: u64) -> Expense {
    Expense {
        id: id.into(),
        class,
        basis: Basis::Billed,
        unit: "msat".into(),
        amount: Some(amount),
        payer: Payer::OpenAgents,
        evidence: Some(retained(root, id, id.as_bytes())),
        price: None,
    }
}
fn freeze(root: &Path, m: &mut Manifest) {
    m.inventory = reference(
        root,
        "finance-inventory.json",
        &Inventory {
            schema: "openagents.gym.sales-finance-inventory.v1".into(),
            entries: m
                .offers
                .iter()
                .flat_map(|o| o.entries.iter().map(|e| e.id.clone()))
                .collect(),
            complete: true,
        },
    );
}
pub(crate) fn fixture(rail: Rail) -> (TempDir, Manifest) {
    let (dir, mut study) = evidence::tests::fixture();
    let root = dir.path();
    fs::set_permissions(root, fs::Permissions::from_mode(0o700)).unwrap();
    study.skipped_evidence.clear();
    for task in &mut study.tasks {
        for attempt in task.baseline.iter_mut().chain(&mut task.candidate) {
            attempt.costs = [
                (CostComponent::Provider, 20),
                (CostComponent::Compute, 10),
                (CostComponent::Support, 5),
            ]
            .into_iter()
            .map(|(component, amount)| {
                let name = format!("{}-{component:?}-bill", attempt.id);
                Cost {
                    component,
                    basis: CostBasis::Billed,
                    unit: "msat".into(),
                    amount: Some(amount),
                    evidence: Some(retained(root, &name, name.as_bytes())),
                    price: None,
                }
            })
            .collect();
        }
    }
    let study_ref = reference(root, "comparison.json", &study);
    let path = root.join("ledger.sqlite");
    let mut ledger = Ledger::open(&path).unwrap();
    let rule = pay_ledger::V1
        .replace("version = 1", "version = 2")
        .replace("2026-10-02", "2026-10-03")
        .replace(
            "first_paid_call_msat = 1_000_000",
            "first_paid_call_msat = 100",
        )
        .replace("launch_match_bps = 10000", "launch_match_bps = 1000");
    ledger.load_rule(&rule, &pay_ledger::digest(&rule)).unwrap();
    let key = if rail == Rail::Balance {
        "debit:paid"
    } else {
        "paid"
    };
    ledger
        .record_settlement(SettlementInput {
            key: key.into(),
            resource: "/private/resource?secret-query".into(),
            plugin_id: Some("synthetic-plugin".into()),
            release_id: Some("synthetic-release".into()),
            price_msat: 1000,
            received_msat: 900,
            rail,
            payer_alias: Some("private-customer-alias".into()),
            settled_at: AT as i64,
            split: Split::Plugin {
                author: "synthetic-author".into(),
                fee_msat: 300,
            },
        })
        .unwrap();
    drop(ledger);
    let ledger_ref = retained(root, "ledger.sqlite", &fs::read(&path).unwrap());
    let attribution = reference(
        root,
        "attribution.json",
        &Attribution {
            schema: "openagents.sales.financial-attribution.v1".into(),
            ledger_digest: ledger_ref.sha256.clone(),
            settlement: key.into(),
            account: "private-account".into(),
            offer_version: study.offer_version.clone(),
        },
    );
    let terms = Terms {
        version: "private-price-v1".into(),
        evidence: retained(root, "terms", b"synthetic retained contract"),
        unit: "msat".into(),
        contractual_charge: 1000,
        billable_failure: false,
    };
    let entry = Entry {
        id: "paid-entry".into(),
        at: AT,
        terms,
        source: Source::Settlement {
            ledger: ledger_ref,
            key: key.into(),
            attribution,
        },
        delivery: Delivery::Accepted,
        delivery_evidence: retained(
            root,
            "delivery",
            b"synthetic delivered candidate acceptance",
        ),
        task: Some(TaskLink {
            comparison: "comparison".into(),
            task: "task".into(),
            payer: Payer::OpenAgents,
            include_baseline_costs: false,
            allocation_evidence: retained(
                root,
                "allocation",
                b"synthetic operator-paid candidate; historical baseline excluded",
            ),
        }),
        expenses: vec![
            expense_record(root, "setup-bill", ExpenseClass::Setup, 10),
            expense_record(root, "onboarding-bill", ExpenseClass::Onboarding, 4),
            expense_record(root, "repair-bill", ExpenseClass::Repair, 6),
            expense_record(root, "support-case-bill", ExpenseClass::Support, 12),
        ],
        adjustments: vec![
            Adjustment {
                id: "refund".into(),
                kind: AdjustmentKind::Refund,
                target: AdjustmentTarget::EarnedCharge,
                amount: 50,
                evidence: retained(root, "refund", b"synthetic refund paid"),
            },
            Adjustment {
                id: "reverse-refund".into(),
                kind: AdjustmentKind::RefundReversal,
                target: AdjustmentTarget::EarnedCharge,
                amount: 20,
                evidence: retained(root, "reverse-refund", b"synthetic refund reversal"),
            },
            Adjustment {
                id: "loss".into(),
                kind: AdjustmentKind::Loss,
                target: AdjustmentTarget::EarnedCharge,
                amount: 10,
                evidence: retained(root, "loss", b"synthetic irrecoverable loss"),
            },
        ],
        incidents: vec![Incident {
            id: "support-case".into(),
            kind: IncidentKind::Support,
            responsible_human: "private-support-human".into(),
            elapsed_ms: 1200,
            evidence: retained(root, "support-case", b"synthetic support completed"),
            expense_ids: vec!["support-case-bill".into()],
        }],
    };
    let mut manifest = Manifest {
        schema: SCHEMA.into(),
        owner: "private-owner".into(),
        period_start: AT - 10,
        period_end: AT + 100,
        inventory: Reference {
            path: String::new(),
            sha256: String::new(),
        },
        comparisons: BTreeMap::from([("comparison".into(), study_ref)]),
        offers: vec![Offer {
            id: "private-offer".into(),
            version: study.offer_version,
            account: "private-account".into(),
            cohort: "private-cohort".into(),
            entries: vec![entry],
            no_cost: BTreeMap::from([(ExpenseClass::Payment,retained(root,"no-additional-payment-fee",b"synthetic period has no additional outbound payment fee; inbound fee stays netted"))]),
            assumptions: vec!["synthetic candidate delivery costs only".into()],
        }],
        gaps: vec![],
    };
    freeze(root, &mut manifest);
    (dir, manifest)
}
fn build(dir: &TempDir, m: &Manifest) -> Report {
    rebuild(dir.path(), &serde_json::to_vec(m).unwrap()).unwrap()
}
pub(crate) fn service_fixture() -> (TempDir, Manifest, receipts::service_sale::Export) {
    use receipts::service_sale::{self as service, Disposition, Verification};
    let (dir, mut m) = fixture(Rail::Lightning);
    let root = dir.path();
    let mut study: evidence::Manifest =
        serde_json::from_slice(&fs::read(root.join("comparison.json")).unwrap()).unwrap();
    let frozen_check = retained(
        root,
        "service-frozen-command",
        b"synthetic frozen check command",
    );
    for task in &mut study.tasks {
        task.check_digests
            .insert("check".into(), frozen_check.sha256.clone());
        for attempt in task.baseline.iter_mut().chain(&mut task.candidate) {
            attempt.checks.get_mut("check").unwrap().check_digest = frozen_check.sha256.clone();
            for cost in &mut attempt.costs {
                cost.unit = "USD_millionths".into();
            }
        }
    }
    let manifest = reference(root, "comparison.json", &study);
    m.comparisons.insert("comparison".into(), manifest.clone());
    let checked =
        evidence::rebuild(root, &fs::read(root.join("comparison.json")).unwrap()).unwrap();
    let report = reference(root, "service-checked-report.json", &checked);
    let accepted = &study.tasks[0].candidate[1];
    let accept = accepted.acceptance.as_ref().unwrap();
    let convert = |r: &Reference| service::Reference {
        path: r.path.clone(),
        sha256: r.sha256.clone(),
    };
    let admission = service_sources::admission(
        root,
        AT,
        "synthetic-lead",
        &m.offers[0].account,
        &m.offers[0].version,
        service_sources::Comparison {
            manifest: convert(&manifest),
            report: convert(&report),
            candidate: convert(&accepted.artifact),
            check: convert(&accept.check_review),
            decision: convert(&accept.customer_decision),
            frozen_checks: vec![convert(&frozen_check)],
        },
    );
    let facts = service::verify_sources(
        &admission,
        "synthetic-lead",
        &m.offers[0].account,
        1,
        AT,
        |r| Ok(fs::read(root.join(&r.path)).unwrap()),
    )
    .unwrap();
    let payment = service_sources::retain(
        root,
        "service-paid",
        b"synthetic owner verified exact external invoice collection",
    );
    let sale = service::Sale {
        schema: service::SCHEMA.into(),
        admission,
        pipeline_lead: "synthetic-lead".into(),
        pipeline_revision_at_admission: 1,
        account: m.offers[0].account.clone(),
        admitted_by: "operator".into(),
        admitted_at: AT,
        admission_command_digest: "d".repeat(64),
        admitted_recipients: vec!["human:operator".into()],
        retain_until: AT + 100,
        facts,
        payments: vec![Verification {
            input: service::PaymentInput {
                disposition: Disposition::Paid,
                external_reference: Some("synthetic-payment".into()),
                paid_minor: Some(25000),
                reversed_minor: None,
                evidence: payment,
            },
            verified_by: "operator".into(),
            verified_at: AT,
            command_digest: "e".repeat(64),
        }],
        fulfillment_reconciliations: vec![],
    };
    let export = service::Export {
        schema: service::EXPORT_SCHEMA.into(),
        sale,
        exported_by: "operator".into(),
        exported_at: AT,
    };
    let entry = &mut m.offers[0].entries[0];
    entry.source = Source::ServiceSale {
        export: reference(root, "service-export.json", &export),
    };
    entry.terms.evidence = convert_back(&export.sale.admission.sources.agreement);
    entry.terms.unit = "USD_millionths".into();
    entry.terms.contractual_charge = 250_000_000;
    entry.task.as_mut().unwrap().payer = Payer::Customer;
    for expense in &mut entry.expenses {
        expense.unit = "USD_millionths".into();
    }
    entry.adjustments.clear();
    entry.incidents.clear();
    m.offers[0].no_cost.insert(
        ExpenseClass::Promotion,
        retained(
            root,
            "no-service-promotion",
            b"synthetic service has no promotional award",
        ),
    );
    (dir, m, export)
}
fn convert_back(r: &receipts::service_sale::Reference) -> Reference {
    Reference {
        path: r.path.clone(),
        sha256: r.sha256.clone(),
    }
}
fn service_snapshot(dir: &TempDir, m: &mut Manifest, export: &receipts::service_sale::Export) {
    m.offers[0].entries[0].source = Source::ServiceSale {
        export: reference(dir.path(), "service-export.json", export),
    };
}
#[test]
fn verified_service_acceptance_exact_refunds_and_unknowns_join_checked_delivery() {
    use receipts::service_sale::{Disposition, PaymentInput, Verification};
    let (dir, mut m, mut export) = service_fixture();
    let report = build(&dir, &m);
    let view = &report.offers[0];
    assert_eq!(
        view.revenue["USD_millionths"].earned_openagents,
        250_000_000
    );
    assert_eq!(
        view.contribution_known_subtotal["USD_millionths"],
        249_999_968
    );
    assert_eq!(view.profitable, Some(true));
    for (name, disposition, refund) in [
        ("refund", Disposition::Reversed, Some(101)),
        ("restored", Disposition::Paid, Some(51)),
        ("unknown", Disposition::Unknown, None),
    ] {
        export.sale.payments.push(Verification {
            input: PaymentInput {
                disposition,
                external_reference: Some("synthetic-payment".into()),
                paid_minor: if disposition == Disposition::Unknown {
                    None
                } else {
                    Some(25000)
                },
                reversed_minor: refund,
                evidence: service_sources::retain(dir.path(), name, name.as_bytes()),
            },
            verified_by: "operator".into(),
            verified_at: AT,
            command_digest: "f".repeat(64),
        });
    }
    service_snapshot(&dir, &mut m, &export);
    let report = build(&dir, &m);
    let view = &report.offers[0];
    let r = &view.revenue["USD_millionths"];
    assert_eq!(
        (r.gross_collected, r.refunds, r.refund_reversals),
        (250_000_000, 1_010_000, 500_000)
    );
    assert_eq!(
        view.contribution_known_subtotal["USD_millionths"],
        249_489_968
    );
    assert_eq!(view.unresolved_payments, 1);
    assert_eq!(view.profitable, None);
    export.sale.payments.clear();
    service_snapshot(&dir, &mut m, &export);
    let view = &build(&dir, &m).offers[0];
    let r = &view.revenue["USD_millionths"];
    assert_eq!((r.collected, r.earned_openagents), (0, 0));
    assert_eq!(r.unearned_charge, 250_000_000);
    assert_eq!(view.profitable, None);
}
#[test]
fn service_adapter_refuses_generic_receipts_fake_comparisons_and_duplicate_snapshots() {
    let (dir, mut m, export) = service_fixture();
    let generic = non_revenue(
        dir.path(),
        &m,
        "generic-service",
        CollectionKind::Service,
        250,
    );
    m.offers[0].entries.push(generic);
    freeze(dir.path(), &mut m);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("REV-18")
    );
    m.offers[0].entries.pop();
    freeze(dir.path(), &mut m);
    let mut duplicate = export.clone();
    duplicate.exported_at += 1;
    let mut entry = m.offers[0].entries[0].clone();
    entry.id = "another-entry".into();
    entry.source = Source::ServiceSale {
        export: reference(dir.path(), "newer-service-snapshot.json", &duplicate),
    };
    m.offers[0].entries.push(entry);
    freeze(dir.path(), &mut m);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("duplicate authoritative")
    );
    m.offers[0].entries.pop();
    freeze(dir.path(), &mut m);
    m.offers[0].entries[0].delivery = Delivery::Failed;
    assert!(rebuild(dir.path(), &serde_json::to_vec(&m).unwrap()).is_err());
    m.offers[0].entries[0].delivery = Delivery::Accepted;
    // Rebuild every linkage around the changed report to prove digest presence
    // alone cannot substitute for the independent comparison reconstruction.
    let mut changed = export;
    let fake = service_sources::retain(dir.path(), "fake-comparison-report.json", b"{}");
    let mut handoff: serde_json::Value = serde_json::from_slice(
        &fs::read(
            dir.path()
                .join(&changed.sale.admission.sources.handoff.path),
        )
        .unwrap(),
    )
    .unwrap();
    let mut review: serde_json::Value = serde_json::from_slice(
        &fs::read(
            dir.path()
                .join(&changed.sale.admission.sources.pilot_review.path),
        )
        .unwrap(),
    )
    .unwrap();
    review["evidence"]["report_sha256"] = serde_json::json!(fake.sha256);
    review["evidence"]["sales_evidence_report_reference"] = serde_json::json!(fake.path);
    changed.sale.admission.sources.pilot_review =
        service_sources::doc(dir.path(), "changed-review.json", review);
    handoff["sales_evidence_report"] = serde_json::to_value(&fake).unwrap();
    handoff["pilot_review"] =
        serde_json::to_value(&changed.sale.admission.sources.pilot_review).unwrap();
    changed.sale.admission.sources.handoff =
        service_sources::doc(dir.path(), "changed-handoff.json", handoff);
    for r in [
        &mut changed.sale.admission.sources.customer_acceptance,
        &mut changed.sale.admission.sources.support_acceptance,
    ] {
        let mut ack: serde_json::Value =
            serde_json::from_slice(&fs::read(dir.path().join(&r.path)).unwrap()).unwrap();
        ack["handoff"] = serde_json::to_value(&changed.sale.admission.sources.handoff).unwrap();
        *r = service_sources::doc(dir.path(), &format!("changed-{}", r.path), ack);
    }
    changed.sale.facts = receipts::service_sale::verify_sources(
        &changed.sale.admission,
        &changed.sale.pipeline_lead,
        &changed.sale.account,
        1,
        AT,
        |r| Ok(fs::read(dir.path().join(&r.path)).unwrap()),
    )
    .unwrap();
    service_snapshot(&dir, &mut m, &changed);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("reverified")
    );
}
#[test]
fn service_fulfillment_obligation_is_separately_priced_billed_and_paid_without_a_split() {
    use receipts::service_sale::FulfillmentVerification;
    let (dir, mut m, mut export) = service_fixture();
    export.sale.admission.fulfillment = Some(service_sources::fulfillment(
        dir.path(),
        AT,
        &export.sale.admission,
    ));
    service_snapshot(&dir, &mut m, &export);
    let view = &build(&dir, &m).offers[0];
    assert_eq!(
        view.revenue["USD_millionths"].fulfillment_liability,
        50_000_000
    );
    assert_eq!(view.profitable, None);
    assert!(
        view.costs
            .iter()
            .any(|c| c.class == ExpenseClass::Fulfillment && c.unknown_items == 1)
    );
    export
        .sale
        .fulfillment_reconciliations
        .push(FulfillmentVerification {
            input: service_sources::fulfillment_input(dir.path(), AT, false),
            verified_by: "operator".into(),
            verified_at: AT,
            command_digest: "1".repeat(64),
        });
    service_snapshot(&dir, &mut m, &export);
    let view = &build(&dir, &m).offers[0];
    assert_eq!(
        view.revenue["USD_millionths"].fulfillment_liability,
        50_000_000
    );
    assert_eq!(
        view.contribution_known_subtotal["USD_millionths"],
        199_999_968
    );
    assert_eq!(view.profitable, Some(true));
    export
        .sale
        .fulfillment_reconciliations
        .push(FulfillmentVerification {
            input: service_sources::fulfillment_input(dir.path(), AT, true),
            verified_by: "operator".into(),
            verified_at: AT,
            command_digest: "2".repeat(64),
        });
    service_snapshot(&dir, &mut m, &export);
    let view = &build(&dir, &m).offers[0];
    assert_eq!(view.revenue["USD_millionths"].fulfillment_liability, 0);
    assert_eq!(
        view.contribution_known_subtotal["USD_millionths"],
        199_999_968
    );
    export.sale.payments.clear();
    service_snapshot(&dir, &mut m, &export);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("trigger")
    );
}
#[test]
fn btc_fulfillment_keeps_usd_contribution_unknown_without_inferred_fx() {
    use serde_json::{Value, json};
    let (dir, mut m, mut export) = service_fixture();
    let mut fulfillment = service_sources::fulfillment(dir.path(), AT, &export.sale.admission);
    let mut agreement: Value =
        serde_json::from_slice(&fs::read(dir.path().join(&fulfillment.agreement.path)).unwrap())
            .unwrap();
    agreement["currency"] = json!("BTC");
    agreement["currency_scale"] = json!(100_000_000_000u64);
    agreement["amount_minor"] = json!(10_000);
    let agreement = reference(dir.path(), "btc-fulfillment-agreement.json", &agreement);
    let mut accepted: Value =
        serde_json::from_slice(&fs::read(dir.path().join(&fulfillment.acceptance.path)).unwrap())
            .unwrap();
    accepted["agreement_sha256"] = json!(agreement.sha256);
    let accepted = reference(dir.path(), "btc-fulfillment-acceptance.json", &accepted);
    fulfillment.currency = "BTC".into();
    fulfillment.currency_scale = 100_000_000_000;
    fulfillment.amount_minor = 10_000;
    fulfillment.agreement = receipts::service_sale::Reference {
        path: agreement.path,
        sha256: agreement.sha256,
    };
    fulfillment.acceptance = receipts::service_sale::Reference {
        path: accepted.path,
        sha256: accepted.sha256,
    };
    export.sale.admission.fulfillment = Some(fulfillment);
    service_snapshot(&dir, &mut m, &export);
    let view = &build(&dir, &m).offers[0];
    assert_eq!(view.profitable, None);
    assert!(
        view.costs
            .iter()
            .any(|c| c.class == ExpenseClass::Fulfillment
                && c.unit == "USD_millionths"
                && c.unknown_items == 1
                && c.known_subtotal == 0)
    );
    assert_eq!(
        view.contribution_known_subtotal["USD_millionths"],
        249_999_968
    );
}
#[test]
fn service_invoice_scope_cannot_substitute_different_frozen_checks() {
    let (dir, mut m, mut export) = service_fixture();
    let f = &export.sale.facts;
    let wrong = service_sources::retain(
        dir.path(),
        "different-frozen-command",
        b"a different check from the agreed comparison",
    );
    export.sale.admission = service_sources::admission(
        dir.path(),
        AT,
        &export.sale.pipeline_lead,
        &export.sale.account,
        &m.offers[0].version,
        service_sources::Comparison {
            manifest: f.comparison_manifest.clone(),
            report: f.comparison_report.clone(),
            candidate: receipts::service_sale::Reference {
                path: "candidate-repair.patch".into(),
                sha256: f.candidate_sha256.clone(),
            },
            check: f.accepted_checks[0].clone(),
            decision: f.customer_decision_evidence.clone(),
            frozen_checks: vec![wrong],
        },
    );
    export.sale.facts = receipts::service_sale::verify_sources(
        &export.sale.admission,
        &export.sale.pipeline_lead,
        &export.sale.account,
        1,
        AT,
        |r| Ok(fs::read(dir.path().join(&r.path)).unwrap()),
    )
    .unwrap();
    service_snapshot(&dir, &mut m, &export);
    m.offers[0].entries[0].terms.evidence = convert_back(&export.sale.admission.sources.agreement);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("reverified")
    );
}
#[test]
fn authoritative_splits_failures_incentives_reversals_and_support_rebuild_exactly() {
    let (dir, m) = fixture(Rail::Lightning);
    let a = build(&dir, &m);
    let b = build(&dir, &m);
    assert_eq!(
        serde_json::to_vec(&a).unwrap(),
        serde_json::to_vec(&b).unwrap()
    );
    let view = &a.offers[0];
    let r = &view.revenue["msat"];
    assert_eq!(
        (r.collected, r.inbound_fees_already_net, r.author_liability),
        (900, 100, 300)
    );
    assert_eq!(r.gross_collected, 1000);
    assert_eq!(
        (
            r.author_allocated,
            r.promotion_allocated,
            r.promotion_liability
        ),
        (300, 130, 130)
    );
    assert_eq!(
        (
            r.earned_openagents,
            r.promotions_already_allocated,
            r.funded_promotions
        ),
        (570, 30, 100)
    );
    assert_eq!(view.contribution_known_subtotal["msat"], 328);
    assert_eq!(view.profitable, Some(true));
    assert_eq!(view.incident_ms, 1200);
    assert_eq!(a.cohorts[0].contribution_known_subtotal["msat"], 328);
    assert!(a.source_digests.contains_key("candidate-failed.jsonl"));
    assert_eq!(a.commissions, "unavailable");
}
#[test]
fn missing_attempt_costs_and_incomplete_coverage_never_claim_profit() {
    let (dir, mut m) = fixture(Rail::Lightning);
    let mut study: evidence::Manifest =
        serde_json::from_slice(&fs::read(dir.path().join("comparison.json")).unwrap()).unwrap();
    study.tasks[0].candidate[0]
        .costs
        .retain(|c| c.component != CostComponent::Support);
    m.comparisons.insert(
        "comparison".into(),
        reference(dir.path(), "comparison.json", &study),
    );
    let r = build(&dir, &m);
    assert_eq!(r.offers[0].profitable, None);
    assert!(
        r.offers[0]
            .costs
            .iter()
            .any(|c| c.class == ExpenseClass::Support
                && c.basis == Basis::Unknown
                && c.unknown_items == 1)
    );
    m.gaps
        .push("replacement hosting bill not yet issued".into());
    assert_eq!(build(&dir, &m).cohorts[0].profitable, None);
}
#[test]
fn pending_deliveries_missing_classes_and_shared_roots_refuse_profitability() {
    let (dir, mut m) = fixture(Rail::Lightning);
    m.offers[0].entries[0].delivery = Delivery::Pending;
    let report = build(&dir, &m);
    assert_eq!(report.offers[0].unresolved_deliveries, 1);
    assert_eq!(report.offers[0].revenue["msat"].earned_openagents, 0);
    assert_eq!(report.offers[0].profitable, None);
    m.offers[0].no_cost.remove(&ExpenseClass::Payment);
    assert!(
        build(&dir, &m).offers[0]
            .missing_cost_classes
            .contains(&ExpenseClass::Payment)
    );
    fs::set_permissions(dir.path(), fs::Permissions::from_mode(0o755)).unwrap();
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("private directory")
    );
}
fn non_revenue(root: &Path, m: &Manifest, id: &str, kind: CollectionKind, value: u64) -> Entry {
    let charge = if kind == CollectionKind::Agreement {
        250
    } else {
        0
    };
    let terms = Terms {
        version: "funding-terms".into(),
        evidence: retained(root, &format!("{id}-terms"), id.as_bytes()),
        unit: "msat".into(),
        contractual_charge: charge,
        billable_failure: false,
    };
    let receipt = CommercialReceipt {
        schema: "openagents.sales.commercial-receipt.v1".into(),
        id: id.into(),
        account: m.offers[0].account.clone(),
        offer_version: m.offers[0].version.clone(),
        at: AT,
        kind,
        unit: "msat".into(),
        contractual_charge: charge,
        collected: value,
        terms_digest: terms.evidence.sha256.clone(),
        evidence: retained(
            root,
            &format!("{id}-payment"),
            format!("payment:{id}").as_bytes(),
        ),
    };
    Entry {
        id: id.into(),
        at: AT,
        terms,
        source: Source::Commercial {
            receipt: reference(root, &format!("{id}-receipt.json"), &receipt),
        },
        delivery: Delivery::Pending,
        delivery_evidence: retained(root, &format!("{id}-status"), b"funding is not delivery"),
        task: None,
        expenses: vec![],
        adjustments: vec![],
        incidents: vec![],
    }
}
#[test]
fn topups_free_trials_agreements_unspent_funding_and_balance_debits_stay_separate() {
    let (dir, mut m) = fixture(Rail::Balance);
    for (id, kind, value) in [
        ("topup", CollectionKind::TopUp, 5000),
        ("agreement", CollectionKind::Agreement, 0),
        ("trial", CollectionKind::FreeTrial, 0),
        ("unused", CollectionKind::UnspentFunding, 4100),
    ] {
        let entry = non_revenue(dir.path(), &m, id, kind, value);
        m.offers[0].entries.push(entry);
    }
    m.offers[0].entries[1].adjustments.push(Adjustment {
        id: "funding-refund".into(),
        kind: AdjustmentKind::Refund,
        target: AdjustmentTarget::Funding,
        amount: 500,
        evidence: retained(
            dir.path(),
            "funding-refund",
            b"synthetic unused funding refund",
        ),
    });
    freeze(dir.path(), &mut m);
    let report = build(&dir, &m);
    let r = &report.offers[0].revenue["msat"];
    assert_eq!(
        (
            r.collected,
            r.funding_collected,
            r.purchased_balance_consumed,
            r.unspent_funding
        ),
        (5000, 5000, 900, 4100)
    );
    assert_eq!(r.funding_refunds, 500);
    assert_eq!(r.agreed_future_charge, 250);
    assert_eq!(r.gross_collected, 5000);
    assert_eq!(r.earned_openagents, 570);
    assert_eq!(report.offers[0].contribution_known_subtotal["msat"], 328);
}
#[test]
fn accepted_evidence_is_required_and_contract_billable_failure_is_explicit() {
    let (dir, mut m) = fixture(Rail::Lightning);
    let mut study: evidence::Manifest =
        serde_json::from_slice(&fs::read(dir.path().join("comparison.json")).unwrap()).unwrap();
    study.tasks[0].candidate[1].acceptance = None;
    m.comparisons.insert(
        "comparison".into(),
        reference(dir.path(), "comparison.json", &study),
    );
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("accepted")
    );
    m.offers[0].entries[0].delivery = Delivery::Failed;
    let report = build(&dir, &m);
    assert_eq!(report.offers[0].revenue["msat"].earned_openagents, 0);
    assert_eq!(report.offers[0].revenue["msat"].unearned_charge, 1000);
    m.offers[0].entries[0].terms.billable_failure = true;
    let report = build(&dir, &m);
    assert_eq!(report.offers[0].revenue["msat"].earned_openagents, 570);
    assert_eq!(report.offers[0].revenue["msat"].billable_failures, 1);
}
#[test]
fn bills_estimates_grants_and_capacity_remain_inspectable_without_cash_conversion() {
    let (dir, mut m) = fixture(Rail::Lightning);
    let root = dir.path();
    let mut estimate = expense_record(root, "forecast", ExpenseClass::Compute, 999);
    estimate.basis = Basis::Estimate;
    estimate.price = Some(evidence::Price {
        version: "price-v1".into(),
        provenance: retained(root, "forecast-price", b"synthetic price terms"),
    });
    let mut capacity = expense_record(root, "capacity", ExpenseClass::Compute, 77);
    capacity.basis = Basis::SubscriptionCapacity;
    capacity.unit = "subscription_capacity_units".into();
    let mut grant = expense_record(root, "grant", ExpenseClass::Provider, 88);
    grant.basis = Basis::ProviderGrant;
    grant.unit = "provider_grant_units".into();
    m.offers[0].entries[0]
        .expenses
        .extend([estimate, capacity, grant]);
    let report = build(&dir, &m);
    let view = &report.offers[0];
    assert_eq!(view.profitable, None);
    assert_eq!(view.contribution_known_subtotal["msat"], 328);
    for basis in [
        Basis::Billed,
        Basis::Estimate,
        Basis::SubscriptionCapacity,
        Basis::ProviderGrant,
    ] {
        assert!(view.costs.iter().any(|c| c.basis == basis));
    }
}
#[test]
fn successive_ledger_snapshots_cannot_recount_the_same_payment_or_balance_debit() {
    for rail in [Rail::Lightning, Rail::Balance] {
        let (dir, mut m) = fixture(rail);
        let root = dir.path();
        fs::copy(root.join("ledger.sqlite"), root.join("later.sqlite")).unwrap();
        let mut ledger = Ledger::open(root.join("later.sqlite")).unwrap();
        ledger
            .record_settlement(SettlementInput {
                key: if rail == Rail::Balance {
                    "debit:later"
                } else {
                    "later"
                }
                .into(),
                resource: "/another/resource".into(),
                plugin_id: None,
                release_id: None,
                price_msat: 100,
                received_msat: 100,
                rail,
                payer_alias: None,
                settled_at: AT as i64 + 1,
                split: Split::OpenAgents,
            })
            .unwrap();
        drop(ledger);
        let later = retained(
            root,
            "later.sqlite",
            &fs::read(root.join("later.sqlite")).unwrap(),
        );
        let Source::Settlement {
            ledger: original,
            key,
            ..
        } = &m.offers[0].entries[0].source
        else {
            unreachable!();
        };
        assert_ne!(original.sha256, later.sha256);
        let attribution = reference(
            root,
            "later-attribution.json",
            &Attribution {
                schema: "openagents.sales.financial-attribution.v1".into(),
                ledger_digest: later.sha256.clone(),
                settlement: key.clone(),
                account: "different-private-account".into(),
                offer_version: "different-offer-v1".into(),
            },
        );
        let mut duplicate = m.offers[0].clone();
        duplicate.id = "different-offer".into();
        duplicate.account = "different-private-account".into();
        duplicate.version = "different-offer-v1".into();
        let entry = &mut duplicate.entries[0];
        entry.id = "same-payment-different-entry".into();
        entry.source = Source::Settlement {
            ledger: later,
            key: key.clone(),
            attribution,
        };
        entry.delivery = Delivery::Pending;
        entry.task = None;
        entry.expenses.clear();
        entry.adjustments.clear();
        entry.incidents.clear();
        m.offers.push(duplicate);
        freeze(root, &mut m);
        assert!(
            rebuild(root, &serde_json::to_vec(&m).unwrap())
                .unwrap_err()
                .contains("duplicate settlement source")
        );
    }
}
#[test]
fn duplicate_sources_changed_evidence_wrong_scope_and_incomplete_inventory_refuse() {
    let (dir, mut m) = fixture(Rail::Lightning);
    let mut clone = m.offers[0].entries[0].clone();
    clone.id = "duplicate-entry".into();
    m.offers[0].entries.push(clone);
    freeze(dir.path(), &mut m);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("duplicate settlement")
    );
    m.offers[0].entries.pop();
    freeze(dir.path(), &mut m);
    let mut cost = m.offers[0].entries[0].expenses[0].clone();
    cost.id = "duplicate-bill".into();
    m.offers[0].entries[0].expenses.push(cost);
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("duplicate billed")
    );
    m.offers[0].entries[0].expenses.last_mut().unwrap().class = ExpenseClass::Compute;
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("duplicate billed")
    );
    m.offers[0].entries[0].expenses.pop();
    m.inventory = reference(
        dir.path(),
        "finance-inventory.json",
        &Inventory {
            schema: "openagents.gym.sales-finance-inventory.v1".into(),
            entries: vec![],
            complete: true,
        },
    );
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("frozen inventory")
    );
    freeze(dir.path(), &mut m);
    m.offers[0].account = "other-private-account".into();
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("attribution")
    );
    m.offers[0].account = "private-account".into();
    fs::write(dir.path().join("delivery"), b"changed").unwrap();
    assert!(
        rebuild(dir.path(), &serde_json::to_vec(&m).unwrap())
            .unwrap_err()
            .contains("digest mismatch")
    );
}
#[test]
fn only_exact_owner_review_exports_scrubbed_aggregates_and_cli_output_is_private() {
    let (dir, m) = fixture(Rail::Lightning);
    let report = build(&dir, &m);
    let bytes = serde_json::to_vec(&report).unwrap();
    let mut review = Review {
        schema: "openagents.gym.sales-finance-review.v1".into(),
        report_digest: evidence::digest(&bytes),
        owner: m.owner.clone(),
        approved: true,
    };
    let wire = serde_json::to_string(&project(&bytes, &review).unwrap()).unwrap();
    for private in [
        "private-account",
        "private-cohort",
        "private-owner",
        "private-offer",
        "private-support-human",
        "paid-entry",
        "secret-query",
        "synthetic-author",
        "ledger.sqlite",
    ] {
        assert!(!wire.contains(private));
    }
    review.owner = "unauthorized-reviewer".into();
    assert!(project(&bytes, &review).is_err());
    review.owner = m.owner.clone();
    review.approved = false;
    assert!(project(&bytes, &review).is_err());
    review.approved = true;
    review.report_digest = "0".repeat(64);
    assert!(project(&bytes, &review).is_err());
    let mut leaked = report.clone();
    leaked.cohorts[0]
        .revenue
        .insert("private-account-denomination".into(), Revenue::default());
    let leaked = serde_json::to_vec(&leaked).unwrap();
    review.report_digest = evidence::digest(&leaked);
    assert!(project(&leaked, &review).is_err());
    let manifest = dir.path().join("finance-manifest.json");
    fs::write(&manifest, serde_json::to_vec(&m).unwrap()).unwrap();
    let output = dir.path().join("private-report.json");
    let args = vec![
        "--root".into(),
        dir.path().to_str().unwrap().into(),
        "--manifest".into(),
        manifest.to_str().unwrap().into(),
        "--output".into(),
        output.to_str().unwrap().into(),
    ];
    command(&args).unwrap();
    assert_eq!(
        fs::metadata(&output).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(command(&args).is_err());
}
