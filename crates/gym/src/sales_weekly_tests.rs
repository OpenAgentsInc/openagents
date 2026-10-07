use super::*;
use receipts::sales_funnel::{
    Admission, Consent, Event, EventInput, EvidenceClass, FailureInput, FinancialIdentity, Journey,
    TaskAttribution,
};
use std::fs;
use tempfile::TempDir;
const AT: u64 = 1_790_986_000;
fn retain(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    fs::write(root.join(name), bytes).unwrap();
    Reference {
        path: name.into(),
        sha256: evidence::digest(bytes),
    }
}
fn doc<T: Serialize>(root: &Path, name: &str, value: &T) -> Reference {
    retain(root, name, &serde_json::to_vec(value).unwrap())
}
fn convert(r: &evidence::Reference) -> Reference {
    Reference {
        path: r.path.clone(),
        sha256: r.sha256.clone(),
    }
}
fn event(id: &str, at: u64, kind: Kind) -> Event {
    Event {
        input: EventInput {
            id: id.into(),
            at,
            kind,
        },
        recorded_by: "operator".into(),
        recorded_at: at.max(AT + 5),
        command_digest: "e".repeat(64),
    }
}
fn task_event(
    root: &Path,
    name: &str,
    at: u64,
    source: Reference,
    account: Option<&str>,
    cohort: &str,
    task_id: &str,
) -> Event {
    let checked = evidence::rebuild(root, &fs::read(root.join(&source.path)).unwrap()).unwrap();
    let task = checked
        .manifest
        .tasks
        .iter()
        .find(|t| t.id == task_id)
        .unwrap();
    let accepted = task.candidate.iter().find_map(|a| a.acceptance.as_ref());
    let attribution = doc(
        root,
        &format!("{name}-attribution.json"),
        &TaskAttribution {
            schema: "openagents.sales.task-account-attribution.v1".into(),
            account: account.map(Into::into),
            offer_version: checked.manifest.offer_version.clone(),
            cohort: cohort.into(),
            manifest_digest: source.sha256.clone(),
            task: task_id.into(),
            customer_decision: accepted.map(|a| convert(&a.customer_decision)),
        },
    );
    event(
        name,
        at,
        Kind::Task {
            manifest: source,
            report: doc(root, &format!("{name}-report.json"), &checked),
            attribution,
            task: task_id.into(),
        },
    )
}
fn financial_pair(root: &Path, m: &finance::Manifest, name: &str) -> Pair {
    let bytes = serde_json::to_vec(m).unwrap();
    let checked = finance::rebuild(root, &bytes).unwrap();
    Pair {
        manifest: retain(root, &format!("{name}-manifest.json"), &bytes),
        report: doc(root, &format!("{name}-report.json"), &checked),
    }
}
fn freeze(root: &Path, m: &mut finance::Manifest) {
    m.inventory = {
        let r = doc(
            root,
            "current-financial-inventory.json",
            &finance::Inventory {
                schema: "openagents.gym.sales-finance-inventory.v1".into(),
                entries: m
                    .offers
                    .iter()
                    .flat_map(|o| o.entries.iter().map(|e| e.id.clone()))
                    .collect(),
                complete: true,
            },
        );
        evidence::Reference {
            path: r.path,
            sha256: r.sha256,
        }
    };
}
fn setup(assisted: bool) -> (TempDir, Manifest, Export, finance::Manifest) {
    let (dir, mut financial, service) = if assisted {
        let (d, m, e) = finance::tests::service_fixture();
        (d, m, Some(e.sale))
    } else {
        let (d, m) = finance::tests::fixture(pay_ledger::Rail::Lightning);
        (d, m, None)
    };
    let root = dir.path();
    financial.owner = "operator".into();
    financial.period_start = AT + 50 - WEEK;
    financial.period_end = AT + 100;
    financial.offers[0].entries[0].adjustments.clear();
    let pair = financial_pair(root, &financial, "initial-financial");
    let source = if let Some(service) = &service {
        FinancialIdentity::ServiceSale {
            sale: service.admission.id.clone(),
        }
    } else {
        FinancialIdentity::Settlement { key: "paid".into() }
    };
    let admission = Admission {
        id: "journey".into(),
        offer_version: financial.offers[0].version.clone(),
        cohort: financial.offers[0].cohort.clone(),
        lane: if assisted {
            Lane::Assisted
        } else {
            Lane::SelfServe
        },
        classification: EvidenceClass::Fixture,
        consent: Consent {
            evidence: retain(
                root,
                "telemetry-consent",
                b"synthetic separate tracking/count aggregation permission",
            ),
            at: AT - 10,
            expires_at: AT + 100,
            aggregate_counts: true,
        },
    };
    let mut events = vec![event(
        "acquisition",
        AT - 9,
        Kind::Acquisition {
            source: Some("synthetic permitted introduction".into()),
            evidence: retain(root, "acquisition-source", b"synthetic source observation"),
        },
    )];
    if assisted {
        events.push(event(
            "pilot",
            AT - 5,
            Kind::PilotAgreed {
                evidence: retain(
                    root,
                    "pilot-observation",
                    b"synthetic agreed pilot observation; not acceptance",
                ),
            },
        ));
    } else {
        events.push(event(
            "install",
            AT - 7,
            Kind::Install {
                client: "synthetic-cli".into(),
                evidence: retain(
                    root,
                    "install-observation",
                    b"synthetic install observation; not activation",
                ),
            },
        ));
        events.push(event(
            "provider",
            AT - 6,
            Kind::ProviderActivation {
                provider: "synthetic-provider".into(),
                evidence: retain(
                    root,
                    "provider-observation",
                    b"synthetic provider observation; not accepted work",
                ),
            },
        ));
    }
    events.push(task_event(
        root,
        "task",
        AT,
        convert(&financial.comparisons["comparison"]),
        Some(&financial.offers[0].account),
        &financial.offers[0].cohort,
        "task",
    ));
    events.push(event(
        "purchase",
        AT,
        Kind::Purchase {
            financial_offer: financial.offers[0].id.clone(),
            entry: financial.offers[0].entries[0].id.clone(),
            source,
            evidence: pair.report.clone(),
        },
    ));
    let export = Export {
        schema: funnel::EXPORT_SCHEMA.into(),
        journey: Journey {
            schema: funnel::SCHEMA.into(),
            admission,
            pipeline_lead: "synthetic-lead".into(),
            pipeline_revision_at_admission: 1,
            account: financial.offers[0].account.clone(),
            admitted_by: "operator".into(),
            admitted_at: AT - 10,
            admission_command_digest: "d".repeat(64),
            admitted_recipients: vec!["human:operator".into()],
            retain_until: AT + 100,
            events,
            failures: vec![],
        },
        services: service
            .into_iter()
            .map(|s| (s.admission.id.clone(), s))
            .collect(),
        exported_by: "operator".into(),
        exported_at: AT + 50,
    };
    let manifest = Manifest {
        schema: SCHEMA.into(),
        owner: "operator".into(),
        period_start: AT + 50 - WEEK,
        period_end: AT + 50,
        generated_at: AT + 50,
        journeys: vec![doc(root, "journey-export.json", &export)],
        finance: Some(pair),
        gaps: vec![],
    };
    (dir, manifest, export, financial)
}
fn save(root: &Path, m: &mut Manifest, e: &Export) {
    m.journeys[0] = doc(root, "journey-export.json", e);
}
fn failure(root: &Path, export: &mut Export, event: &str, reason: FailureReason) {
    export.journey.failures.push(Failure {
        input: FailureInput {
            event: event.into(),
            reason,
            responsible_human: "operator".into(),
            next_action: "privately reconcile the source; no customer contact".into(),
            due_at: AT + 60,
            evidence: retain(
                root,
                &format!("{event}-failure"),
                b"synthetic accepted failure responsibility",
            ),
            resolved: false,
        },
        recorded_by: "operator".into(),
        recorded_at: AT + 5,
        command_digest: "f".repeat(64),
    });
}
#[test]
fn weekly_self_serve_and_assisted_source_histories_are_separate_and_not_commercial_activation() {
    for assisted in [false, true] {
        let (dir, m, _, _) = setup(assisted);
        let bytes = serde_json::to_vec(&m).unwrap();
        let report = rebuild(dir.path(), &bytes, AT + 50).unwrap();
        assert_eq!(report.cohorts.len(), 1);
        assert_eq!(report.cohorts[0].classification, EvidenceClass::Fixture);
        assert_eq!(report.cohorts[0].cumulative.settled_purchases, 1);
        assert_eq!(report.cohorts[0].cumulative.accepted_tasks, 1);
        assert_eq!(report.cohorts[0].cumulative.repeat_purchases, 0);
        assert_eq!(
            report.cohorts[0].cumulative.assisted_pilots,
            u64::from(assisted)
        );
        assert_eq!(
            report.cohorts[0].cumulative.install_observations,
            u64::from(!assisted)
        );
        assert_eq!(report.cohorts[0].accepted_to_purchased.numerator, 1);
        assert!(!report.commercial_activation_attested);
        assert!(
            report
                .contribution_scope
                .contains("weekly contribution unknown")
        );
        assert_eq!(
            serde_json::to_value(&report).unwrap(),
            serde_json::to_value(rebuild(dir.path(), &bytes, AT + 50).unwrap()).unwrap()
        );
        let stages = &report.journeys[0].history;
        assert!(
            stages
                .iter()
                .filter(|s| s.stage.ends_with("observation") || s.stage == "assisted_pilot")
                .all(|s| s.state == State::OwnerObservation)
        );
    }
}
#[test]
fn weekly_unknown_consent_source_payment_and_failed_work_do_not_become_success() {
    let (dir, mut m, mut export, financial) = setup(true);
    let root = dir.path();
    export.journey.admission.consent.expires_at = AT - 11;
    save(root, &mut m, &export);
    assert!(rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).is_err());
    export.journey.admission.consent.expires_at = AT + 100;
    if let Kind::Acquisition { source, .. } = &mut export.journey.events[0].input.kind {
        *source = None;
    }
    save(root, &mut m, &export);
    assert!(
        rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50)
            .unwrap_err()
            .contains("responsible human")
    );
    failure(
        root,
        &mut export,
        "acquisition",
        FailureReason::UnknownAttribution,
    );
    m.finance = None;
    failure(root, &mut export, "purchase", FailureReason::PaymentUnknown);
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 0);
    assert_eq!(report.cohorts[0].cumulative.unknown_purchases, 1);
    assert_eq!(report.cohorts[0].cumulative.actionable_failures, 2);
    assert!(report.contribution_scope.starts_with("unknown"));
    m.finance = Some(financial_pair(root, &financial, "financial-restored"));
    export.journey.failures[0].input.responsible_human = "".into();
    save(root, &mut m, &export);
    assert!(rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).is_err());
}
#[test]
fn weekly_refunds_and_stale_financial_snapshots_cannot_preserve_paid_conversion() {
    let (dir, mut m, mut export, mut financial) = setup(true);
    let root = dir.path();
    let sale = export.services.get_mut("synthetic-sale").unwrap();
    sale.payments.push(receipts::service_sale::Verification {
        input: receipts::service_sale::PaymentInput {
            disposition: receipts::service_sale::Disposition::Reversed,
            external_reference: Some("synthetic-payment".into()),
            paid_minor: Some(25000),
            reversed_minor: Some(25000),
            evidence: retain(
                root,
                "verified-refund",
                b"synthetic owner verified exact full refund",
            ),
        },
        verified_by: "operator".into(),
        verified_at: AT + 10,
        command_digest: "a".repeat(64),
    });
    let current_sale = sale.clone();
    save(root, &mut m, &export);
    assert!(
        rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50)
            .unwrap_err()
            .contains("current canonical custody")
    );
    let current = receipts::service_sale::Export {
        schema: receipts::service_sale::EXPORT_SCHEMA.into(),
        sale: current_sale,
        exported_by: "operator".into(),
        exported_at: AT + 10,
    };
    financial.offers[0].entries[0].source = finance::Source::ServiceSale {
        export: {
            let r = doc(root, "refunded-service.json", &current);
            evidence::Reference {
                path: r.path,
                sha256: r.sha256,
            }
        },
    };
    m.finance = Some(financial_pair(root, &financial, "refunded-financial"));
    failure(root, &mut export, "purchase", FailureReason::Refunded);
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 0);
    assert_eq!(report.cohorts[0].cumulative.refunded_purchases, 1);
    assert_eq!(report.cohorts[0].cumulative.repeat_purchases, 0);
    assert_eq!(
        report.journeys[0].history.last().unwrap().state,
        State::Refunded
    );
}
#[test]
fn weekly_account_attribution_and_current_scope_refuse_relabeling_and_duplicate_purchase_observations()
 {
    let (dir, mut m, mut export, _) = setup(false);
    let root = dir.path();
    export.journey.account = "wrong-account".into();
    save(root, &mut m, &export);
    assert!(
        rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50)
            .unwrap_err()
            .contains("attribution")
    );
    export.journey.account = "private-account".into();
    let mut duplicate = export.journey.events.last().unwrap().clone();
    duplicate.input.id = "purchase-again".into();
    export.journey.events.push(duplicate);
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 1);
    assert_eq!(report.cohorts[0].cumulative.repeat_purchases, 0);
    assert_eq!(
        report.journeys[0].history[report.journeys[0].history.len() - 2].state,
        State::Superseded
    );
    export.journey.admission.lane = Lane::Assisted;
    export.journey.account = "private-account".into();
    export.journey.events.push(event(
        "declined",
        AT + 1,
        Kind::CustomerDecision {
            decision: Decision::Decline,
            evidence: retain(
                root,
                "customer-declined",
                b"synthetic explicit declined pilot decision",
            ),
        },
    ));
    failure(
        root,
        &mut export,
        "declined",
        FailureReason::CustomerDeclined,
    );
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.assisted_declines, 1);
}
#[test]
fn weekly_public_counts_require_current_consent_exact_review_and_delay_and_redact_private_details()
{
    let (dir, m, _, _) = setup(true);
    let root = dir.path();
    let bytes =
        serde_json::to_vec(&rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap())
            .unwrap();
    let mut review = Review {
        schema: "openagents.gym.sales-weekly-review.v1".into(),
        owner: "operator".into(),
        report_digest: evidence::digest(&bytes),
        approved: true,
        reviewed_at: AT + 50,
        release_at: AT + 60,
    };
    assert!(project(&bytes, &review, AT + 59).is_err());
    let aggregate = project(&bytes, &review, AT + 60).unwrap();
    let public = serde_json::to_string(&aggregate).unwrap();
    for private in [
        "private-account",
        "private-cohort",
        "private-offer",
        "operator",
        "synthetic-lead",
        "250000000",
        "period_start",
        "due_at",
        "recorded_at",
        "purchase-again",
    ] {
        assert!(!public.contains(private), "{private}");
    }
    assert!(!aggregate.commercial_activation_attested);
    assert_eq!(aggregate.lanes[0].classification, EvidenceClass::Fixture);
    review.report_digest = "f".repeat(64);
    assert!(project(&bytes, &review, AT + 60).is_err());
    review.report_digest = evidence::digest(&bytes);
    assert!(project(&bytes, &review, AT + 101).is_err());
    let mut report: Report = serde_json::from_slice(&bytes).unwrap();
    report.sources[0].journey.admission.consent.aggregate_counts = false;
    let declined = serde_json::to_vec(&report).unwrap();
    review.report_digest = evidence::digest(&declined);
    assert!(project(&declined, &review, AT + 60).is_err());
}

fn repeat_fixture(distinct: bool) -> (TempDir, Manifest) {
    use pay_ledger::{Ledger, Rail, SettlementInput, Split};
    let (dir, mut m, mut export, mut financial) = setup(false);
    let root = dir.path();
    let first_ledger = retain(
        root,
        "first-ledger.sqlite",
        &fs::read(root.join("ledger.sqlite")).unwrap(),
    );
    if let finance::Source::Settlement { ledger, .. } = &mut financial.offers[0].entries[0].source {
        *ledger = evidence::Reference {
            path: first_ledger.path,
            sha256: first_ledger.sha256,
        };
    }
    let mut ledger = Ledger::open(&root.join("ledger.sqlite")).unwrap();
    ledger
        .record_settlement(SettlementInput {
            key: "repeat-payment".into(),
            resource: "/synthetic-repeat".into(),
            plugin_id: Some("synthetic-plugin".into()),
            release_id: Some("synthetic-release".into()),
            price_msat: 1000,
            received_msat: 900,
            rail: Rail::Lightning,
            payer_alias: Some("private-customer-alias".into()),
            settled_at: (AT + WEEK) as i64,
            split: Split::Plugin {
                author: "synthetic-author".into(),
                fee_msat: 300,
            },
        })
        .unwrap();
    drop(ledger);
    let second_ledger = retain(
        root,
        "ledger.sqlite",
        &fs::read(root.join("ledger.sqlite")).unwrap(),
    );
    let attribution = doc(
        root,
        "repeat-financial-attribution.json",
        &finance::Attribution {
            schema: "openagents.sales.financial-attribution.v1".into(),
            ledger_digest: second_ledger.sha256.clone(),
            settlement: "repeat-payment".into(),
            account: financial.offers[0].account.clone(),
            offer_version: financial.offers[0].version.clone(),
        },
    );
    let mut study: evidence::Manifest =
        serde_json::from_slice(&fs::read(root.join("comparison.json")).unwrap()).unwrap();
    study.tasks[0].id = "repeat-task".into();
    let task = &mut study.tasks[0];
    for attempt in task.baseline.iter_mut().chain(&mut task.candidate) {
        for cost in &mut attempt.costs {
            let name = format!("repeat-{}-{:?}-bill", attempt.id, cost.component);
            let proof = retain(root, &name, name.as_bytes());
            cost.evidence = Some(evidence::Reference {
                path: proof.path,
                sha256: proof.sha256,
            });
        }
    }
    if distinct {
        let candidate = study.tasks[0]
            .candidate
            .iter_mut()
            .find(|a| a.acceptance.is_some())
            .unwrap();
        let session = atif::Session::opening(
            "repeat-accepted-session",
            "synthetic-model",
            "synthetic-provider",
            "synthetic-fixture",
            "revision",
        );
        let path = root.join("repeat-accepted.jsonl");
        let mut log = atif::Log::create_at(&path, &session).unwrap();
        log.append(&atif::Step::said(
            atif::Source::User,
            "distinct synthetic accepted task",
        ))
        .unwrap();
        log.finish(atif::log::ENDED).unwrap();
        drop(log);
        let trace = retain(root, "repeat-accepted.jsonl", &fs::read(&path).unwrap());
        candidate.trace = evidence::Reference {
            path: trace.path,
            sha256: trace.sha256,
        };
        let artifact = retain(
            root,
            "repeat-accepted.patch",
            b"distinct synthetic accepted patch",
        );
        candidate.artifact = evidence::Reference {
            path: artifact.path,
            sha256: artifact.sha256,
        };
        let acceptance = candidate.acceptance.as_mut().unwrap();
        acceptance.candidate_digest = candidate.artifact.sha256.clone();
        let check = retain(
            root,
            "repeat-independent-review",
            b"synthetic independent checker accepted distinct candidate",
        );
        acceptance.check_review = evidence::Reference {
            path: check.path,
            sha256: check.sha256,
        };
        let customer = retain(
            root,
            "repeat-customer-decision",
            b"synthetic same buyer accepted distinct candidate",
        );
        acceptance.customer_decision = evidence::Reference {
            path: customer.path,
            sha256: customer.sha256,
        };
    }
    let inventory = doc(
        root,
        "repeat-task-inventory.json",
        &evidence::Inventory {
            schema: "openagents.gym.sales-inventory.v1".into(),
            attempts: BTreeMap::from([
                (
                    "repeat-task/baseline".into(),
                    study.tasks[0]
                        .baseline
                        .iter()
                        .map(|a| a.id.clone())
                        .collect(),
                ),
                (
                    "repeat-task/candidate".into(),
                    study.tasks[0]
                        .candidate
                        .iter()
                        .map(|a| a.id.clone())
                        .collect(),
                ),
            ]),
        },
    );
    study.inventory = evidence::Reference {
        path: inventory.path,
        sha256: inventory.sha256,
    };
    let source = doc(root, "repeat-comparison.json", &study);
    financial.comparisons.insert(
        "repeat-comparison".into(),
        evidence::Reference {
            path: source.path.clone(),
            sha256: source.sha256.clone(),
        },
    );
    let mut second = financial.offers[0].entries[0].clone();
    second.id = "repeat-entry".into();
    second.at = AT + WEEK;
    second.source = finance::Source::Settlement {
        ledger: evidence::Reference {
            path: second_ledger.path,
            sha256: second_ledger.sha256,
        },
        key: "repeat-payment".into(),
        attribution: evidence::Reference {
            path: attribution.path,
            sha256: attribution.sha256,
        },
    };
    second.task.as_mut().unwrap().comparison = "repeat-comparison".into();
    second.task.as_mut().unwrap().task = "repeat-task".into();
    second.expenses.clear();
    second.incidents.clear();
    financial.offers[0].entries.push(second);
    financial.period_end = AT + WEEK + 100;
    freeze(root, &mut financial);
    let pair = financial_pair(root, &financial, "repeat-financial");
    export.journey.admission.consent.expires_at = AT + 2 * WEEK;
    export.journey.retain_until = AT + 2 * WEEK;
    export.journey.events.push(task_event(
        root,
        "repeat-task",
        AT + WEEK,
        source,
        Some("private-account"),
        "private-cohort",
        "repeat-task",
    ));
    export.journey.events.push(event(
        "repeat-purchase",
        AT + WEEK,
        Kind::Purchase {
            financial_offer: "private-offer".into(),
            entry: "repeat-entry".into(),
            source: FinancialIdentity::Settlement {
                key: "repeat-payment".into(),
            },
            evidence: pair.report.clone(),
        },
    ));
    export.exported_at = AT + WEEK + 50;
    m.period_start = AT + 50;
    m.period_end = AT + WEEK + 50;
    m.generated_at = AT + WEEK + 50;
    m.finance = Some(pair);
    save(root, &mut m, &export);
    (dir, m)
}

#[test]
fn weekly_repeat_and_retention_need_distinct_accepted_sources_for_the_same_buyer() {
    for distinct in [true, false] {
        let (dir, m) = repeat_fixture(distinct);
        let report = rebuild(dir.path(), &serde_json::to_vec(&m).unwrap(), AT + WEEK + 50).unwrap();
        let cohort = &report.cohorts[0];
        assert_eq!(cohort.cumulative.settled_purchases, 2);
        assert_eq!(cohort.period.settled_purchases, 1);
        assert_eq!(
            cohort.cumulative.accepted_tasks,
            if distinct { 2 } else { 1 }
        );
        assert_eq!(cohort.cumulative.repeat_purchases, u64::from(distinct));
        assert_eq!(cohort.period.repeat_purchases, u64::from(distinct));
        assert_eq!(cohort.retained_buyers.denominator, 1);
        assert_eq!(cohort.retained_buyers.numerator, u64::from(distinct));
        let bytes = serde_json::to_vec(&report).unwrap();
        assert_eq!(
            bytes,
            serde_json::to_vec(
                &rebuild(dir.path(), &serde_json::to_vec(&m).unwrap(), AT + WEEK + 50).unwrap()
            )
            .unwrap()
        );
    }
}

#[test]
fn weekly_copied_task_in_another_journey_cannot_raise_conversion_or_accepted_timing() {
    let (dir, mut m, mut export, financial) = setup(false);
    let root = dir.path();
    export.journey.admission.id = "copied-journey".into();
    export.journey.admission.cohort = "copied-cohort".into();
    export.journey.events.pop();
    export.journey.events[3] = task_event(
        root,
        "copied-task",
        AT,
        convert(&financial.comparisons["comparison"]),
        Some("private-account"),
        "copied-cohort",
        "task",
    );
    m.journeys.push(doc(root, "copied-journey.json", &export));
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    let copied = report
        .cohorts
        .iter()
        .find(|c| c.cohort == "copied-cohort")
        .unwrap();
    assert_eq!(copied.cumulative.accepted_tasks, 0);
    assert_eq!(copied.acquired_to_accepted.numerator, 0);
    assert_eq!(copied.acquired_to_accepted.denominator, 1);
    assert_eq!(copied.accepted_to_purchased.denominator, 0);
    let row = report
        .journeys
        .iter()
        .find(|j| j.id == "copied-journey")
        .unwrap();
    assert_eq!(row.history.last().unwrap().state, State::Duplicate);
    assert_eq!(row.acquisition_to_accepted_seconds, None);
    assert_eq!(row.accepted_to_purchase_seconds, None);
}

#[test]
fn weekly_unknown_buyer_failed_candidate_and_unused_funding_cannot_activate() {
    let (dir, mut m, mut export, mut financial) = setup(false);
    let root = dir.path();
    export.journey.events[3] = task_event(
        root,
        "task",
        AT,
        convert(&financial.comparisons["comparison"]),
        None,
        "private-cohort",
        "task",
    );
    failure(root, &mut export, "task", FailureReason::UnknownAttribution);
    failure(root, &mut export, "purchase", FailureReason::PaymentUnknown);
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.accepted_tasks, 0);
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 0);

    let mut study: evidence::Manifest =
        serde_json::from_slice(&fs::read(root.join("comparison.json")).unwrap()).unwrap();
    study.tasks[0].candidate[1].acceptance = None;
    study.tasks[0].candidate[1]
        .checks
        .get_mut("check")
        .unwrap()
        .status = evidence::Status::Failed;
    let source = doc(root, "failed-comparison.json", &study);
    export.journey.events[3] = task_event(
        root,
        "failed-task",
        AT,
        source.clone(),
        Some("private-account"),
        "private-cohort",
        "task",
    );
    export.journey.failures.clear();
    failure(root, &mut export, "failed-task", FailureReason::TaskFailed);
    failure(root, &mut export, "purchase", FailureReason::TaskFailed);
    financial.comparisons.insert(
        "comparison".into(),
        evidence::Reference {
            path: source.path,
            sha256: source.sha256,
        },
    );
    financial.offers[0].entries[0].delivery = finance::Delivery::Failed;
    m.finance = Some(financial_pair(root, &financial, "failed-financial"));
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(report.cohorts[0].cumulative.failed_tasks, 1);
    assert_eq!(report.cohorts[0].cumulative.accepted_tasks, 0);
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 0);

    let offer_version = financial.offers[0].version.clone();
    let entry = &mut financial.offers[0].entries[0];
    let receipt = finance::CommercialReceipt {
        schema: "openagents.sales.commercial-receipt.v1".into(),
        id: "unused-funding".into(),
        account: "private-account".into(),
        offer_version,
        at: AT,
        kind: finance::CollectionKind::UnspentFunding,
        unit: entry.terms.unit.clone(),
        contractual_charge: 0,
        collected: 1000,
        terms_digest: entry.terms.evidence.sha256.clone(),
        evidence: {
            let r = retain(
                root,
                "funding-proof",
                b"synthetic unused balance credit; no accepted usage",
            );
            evidence::Reference {
                path: r.path,
                sha256: r.sha256,
            }
        },
    };
    let receipt = doc(root, "funding.json", &receipt);
    entry.terms.contractual_charge = 0;
    entry.source = finance::Source::Commercial {
        receipt: evidence::Reference {
            path: receipt.path,
            sha256: receipt.sha256,
        },
    };
    m.finance = Some(financial_pair(root, &financial, "funding-financial"));
    if let Kind::Purchase { source, .. } = &mut export.journey.events[4].input.kind {
        *source = FinancialIdentity::Commercial {
            receipt: "unused-funding".into(),
        };
    }
    export.journey.failures[1].input.reason = FailureReason::PaymentUnknown;
    save(root, &mut m, &export);
    let report = rebuild(root, &serde_json::to_vec(&m).unwrap(), AT + 50).unwrap();
    assert_eq!(
        report.journeys[0].history.last().unwrap().state,
        State::Funding
    );
    assert_eq!(report.cohorts[0].cumulative.settled_purchases, 0);
    assert_eq!(report.cohorts[0].cumulative.repeat_purchases, 0);
}
