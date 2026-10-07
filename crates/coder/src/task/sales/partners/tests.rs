use super::*;
use crate::task::sales::{COMMAND_SCHEMA, Command, Details, Input, Operation, Receipt};
use std::fs;
use std::os::unix::fs::PermissionsExt;

mod sources {
    include!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../receipts/tests/support/service_sale.rs"
    ));
}
fn now() -> u64 {
    1000
}
fn later() -> u64 {
    1400
}
fn after_delivery() -> u64 {
    1020
}
fn after_handoff_expiry() -> u64 {
    1201
}
struct Fixture {
    work: tempfile::TempDir,
    store: Store,
    owner: Access,
    partner: Access,
    outsider: Access,
    lead: String,
    root: std::path::PathBuf,
}
fn fixture() -> Fixture {
    let work = tempfile::tempdir().unwrap();
    let mut store = Store::open_with_clock(&work.path().join("host"), now).unwrap();
    store
        .initialize("operator", &work.path().join("owner"))
        .unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&work.path().join("owner")).unwrap())
        .unwrap();
    let mut grant = |human: &str| {
        let path = work.path().join(human);
        store.issue(&owner, human, Role::Writer, &path).unwrap();
        store
            .authenticate(&Store::read_credential(&path).unwrap())
            .unwrap()
    };
    let partner = grant("partner");
    let outsider = grant("outsider");
    let details: Details = serde_json::from_value(json!({
        "account":"synthetic-account","jurisdiction":"synthetic jurisdiction",
        "permission":{"state":"granted","reference":"synthetic-consent","recorded_at":999,"expires_at":1500,"channels":["email"]},
        "workflow":"synthetic accepted workflow","baseline_reference":"synthetic baseline",
        "data":{"recipients":["human:operator","human:partner"],"permitted_use":"private partner preparation","retain_until":2000},
        "stage":"qualified","next":{"description":"review private terms","due_at":1200},"customer_decision":null,"readers":[]
    })).unwrap();
    let input = Input {
        contact: "email:synthetic@example.invalid".into(),
        source: "synthetic direct permission".into(),
        source_at: 999,
        details,
    };
    let bytes = command(
        "lead",
        None,
        0,
        Operation::Create {
            input,
            ownership_acceptance: "synthetic ownership".into(),
        },
    );
    let lead = store.apply(&owner, &bytes).unwrap().lead;
    let root = work.path().join("sources");
    fs::create_dir(&root).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700)).unwrap();
    Fixture {
        work,
        store,
        owner,
        partner,
        outsider,
        lead,
        root,
    }
}
fn command(id: &str, lead: Option<&str>, revision: u64, operation: Operation) -> Vec<u8> {
    serde_json::to_vec(&Command {
        schema: COMMAND_SCHEMA.into(),
        id: id.into(),
        lead: lead.map(str::to_owned),
        expected_revision: revision,
        operation,
    })
    .unwrap()
}
fn apply(f: &mut Fixture, actor: &str, id: &str, operation: Operation) -> Result<Receipt> {
    let access = match actor {
        "operator" => &f.owner,
        "partner" => &f.partner,
        _ => &f.outsider,
    };
    let revision = f.store.state.leads[&f.lead].revision;
    f.store.apply_with_evidence_root(
        access,
        &command(id, Some(&f.lead), revision, operation),
        Some(&f.root),
    )
}
fn evidence(f: &Fixture, name: &str) -> Reference {
    sources::retain(&f.root, name, name.as_bytes())
}
fn approve(f: &mut Fixture, mut p: Proposal) -> Proposal {
    let digest = f.store.partner_digest(&f.owner, &f.lead, &p).unwrap()["proposal_sha256"]
        .as_str()
        .unwrap()
        .to_owned();
    p.approval = sources::doc(
        &f.root,
        &format!("{}-approval.json", p.id),
        json!({"schema":"openagents.sales.partner-approval.v1",
        "pipeline_lead":f.lead,"assignment":p.id,"proposal_sha256":digest,"approved_by":"operator","approved_at":1000,"allow_private_assignment":true}),
    );
    p
}
fn discovery(f: &mut Fixture, id: &str) -> Proposal {
    let p = Proposal {
        id: id.into(),
        recipient_human: "partner".into(),
        expires_at: 1300,
        next: NextAction {
            description: "prepare one consented introduction".into(),
            due_at: 1100,
        },
        terms: Terms::Discovery {
            brief: evidence(f, "brief"),
            permitted_use: "private partner preparation".into(),
        },
        consent: evidence(f, "consent"),
        provenance: evidence(f, "provenance"),
        approval: Reference {
            path: "placeholder.json".into(),
            sha256: "0".repeat(64),
        },
        commission: None,
    };
    approve(f, p)
}
fn action(
    f: &mut Fixture,
    actor: &str,
    id: &str,
    assignment: &str,
    action: Action,
) -> Result<Receipt> {
    apply(
        f,
        actor,
        id,
        Operation::AdvancePartner {
            assignment: assignment.into(),
            action,
        },
    )
}
fn accept(f: &mut Fixture, p: &Proposal) -> Receipt {
    let digest =
        f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap()["invitation"]["proposal_sha256"]
            .as_str()
            .unwrap()
            .into();
    let proof = evidence(f, "recipient-accepted");
    action(
        f,
        "partner",
        "accept",
        &p.id,
        Action::Accept {
            proposal_sha256: digest,
            evidence: proof,
        },
    )
    .unwrap()
}

#[test]
fn discovery_requires_exact_recipient_acceptance_and_retains_no_payment_or_lead_grant() {
    let mut f = fixture();
    let p = discovery(&mut f, "introduction");
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    let invitation = f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap();
    assert!(invitation.get("assignment").is_none());
    assert!(!invitation.to_string().contains("brief"));
    assert!(f.store.show(&f.partner, &f.lead).is_err());
    assert!(f.store.partner_show(&f.outsider, &f.lead, &p.id).is_err());
    let proof = evidence(&f, "ack");
    assert!(
        action(
            &mut f,
            "operator",
            "owner-cannot-accept",
            &p.id,
            Action::Accept {
                proposal_sha256: invitation["invitation"]["proposal_sha256"]
                    .as_str()
                    .unwrap()
                    .into(),
                evidence: proof.clone()
            }
        )
        .is_err()
    );
    assert!(
        action(
            &mut f,
            "partner",
            "different-proposal",
            &p.id,
            Action::Accept {
                proposal_sha256: "a".repeat(64),
                evidence: proof
            }
        )
        .is_err()
    );
    let receipt = accept(&mut f, &p);
    let revision = receipt.revision - 1;
    let bytes = command(
        "accept",
        Some(&f.lead),
        revision,
        Operation::AdvancePartner {
            assignment: p.id.clone(),
            action: Action::Accept {
                proposal_sha256: invitation["invitation"]["proposal_sha256"]
                    .as_str()
                    .unwrap()
                    .into(),
                evidence: evidence(&f, "recipient-accepted"),
            },
        },
    );
    assert_eq!(
        f.store
            .apply_with_evidence_root(&f.partner, &bytes, None)
            .unwrap(),
        receipt
    );
    let view = f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap();
    assert_eq!(view["assignment"]["status"], "accepted");
    assert_eq!(view["authority_granted"], false);
    assert!(f.store.show(&f.partner, &f.lead).is_err());
    assert!(f.store.state.leads[&f.lead].service_sales.is_empty());
    let lead = f.lead.clone();
    let root = f.work.path().join("host");
    drop(f.store);
    let mut store = Store::open_with_clock(&root, now).unwrap();
    let partner = store
        .authenticate(&Store::read_credential(&f.work.path().join("partner")).unwrap())
        .unwrap();
    assert_eq!(
        store.partner_show(&partner, &lead, &p.id).unwrap()["assignment"]["proposal_sha256"],
        invitation["invitation"]["proposal_sha256"]
    );
}

#[test]
fn introduction_handoff_is_explicit_and_retries_create_no_new_obligation() {
    let mut f = fixture();
    let p = discovery(&mut f, "intro");
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    accept(&mut f, &p);
    let proof = evidence(&f, "handoff");
    assert!(
        action(
            &mut f,
            "partner",
            "wrong-target",
            &p.id,
            Action::ProposeHandoff {
                target: "outsider".into(),
                expires_at: 1200,
                evidence: proof.clone()
            }
        )
        .is_err()
    );
    action(
        &mut f,
        "partner",
        "handoff",
        &p.id,
        Action::ProposeHandoff {
            target: "operator".into(),
            expires_at: 1200,
            evidence: proof.clone(),
        },
    )
    .unwrap();
    assert!(
        action(
            &mut f,
            "partner",
            "self-accept",
            &p.id,
            Action::AcceptHandoff {
                evidence: proof.clone()
            }
        )
        .is_err()
    );
    action(
        &mut f,
        "operator",
        "owner-handoff-ack",
        &p.id,
        Action::AcceptHandoff {
            evidence: proof.clone(),
        },
    )
    .unwrap();
    assert_eq!(
        f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap()["assignment"]["status"],
        "completed"
    );
    assert!(
        action(
            &mut f,
            "operator",
            "duplicate-new-id",
            &p.id,
            Action::AcceptHandoff { evidence: proof }
        )
        .is_err()
    );
    assert_eq!(f.store.state.leads[&f.lead].partner_assignments.len(), 1);
    assert!(f.store.state.leads[&f.lead].service_sales.is_empty());
}

#[test]
fn altered_terms_refusal_timeout_and_revocation_cannot_reuse_approval() {
    let mut f = fixture();
    let p = discovery(&mut f, "intro");
    let mut changed = p.clone();
    changed.recipient_human = "operator".into();
    assert!(
        apply(
            &mut f,
            "operator",
            "changed-recipient",
            Operation::ProposePartner { proposal: changed }
        )
        .is_err()
    );
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    let digest =
        f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap()["invitation"]["proposal_sha256"]
            .as_str()
            .unwrap()
            .into();
    let proof = evidence(&f, "refusal");
    action(
        &mut f,
        "partner",
        "refuse",
        &p.id,
        Action::Refuse {
            proposal_sha256: digest,
            evidence: proof,
        },
    )
    .unwrap();
    assert_eq!(
        f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap()["invitation"]["status"],
        "refused"
    );
    let p = discovery(&mut f, "timeout");
    apply(
        &mut f,
        "operator",
        "timeout-propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    let root = f.work.path().join("host");
    drop(f.store);
    let mut store = Store::open_with_clock(&root, later).unwrap();
    let owner = store
        .authenticate(&Store::read_credential(&f.work.path().join("owner")).unwrap())
        .unwrap();
    assert_eq!(
        store.partner_show(&owner, &f.lead, &p.id).unwrap()["assignment"]["status"],
        "timed_out"
    );
    drop(store);
    let mut f = fixture();
    let p = discovery(&mut f, "revoked");
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    f.store.revoke(&f.owner, "partner").unwrap();
    assert!(f.store.partner_show(&f.partner, &f.lead, &p.id).is_err());
    assert_eq!(
        f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap()["assignment"]["status"],
        "cancelled"
    );
}

#[test]
fn current_data_scope_changes_retire_prior_assignment_without_widening_it() {
    let mut f = fixture();
    let p = discovery(&mut f, "intro");
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    accept(&mut f, &p);
    let original = f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap();
    let mut details = f.store.state.leads[&f.lead].details.clone();
    details.data.permitted_use = "separately approved use".into();
    details.permission.reference = "fresh-consent".into();
    apply(
        &mut f,
        "operator",
        "changed-scope",
        Operation::Update { details },
    )
    .unwrap();
    let current = f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap();
    assert_eq!(current["assignment"]["status"], "cancelled");
    assert_eq!(
        current["assignment"]["data"],
        original["assignment"]["data"]
    );
    let proof = evidence(&f, "late-action");
    assert!(
        action(
            &mut f,
            "partner",
            "late-next",
            &p.id,
            Action::Next {
                next: p.next,
                evidence: proof
            }
        )
        .is_err()
    );
    assert!(
        f.store
            .partner_export(
                &f.outsider,
                &f.lead,
                &p.id,
                &f.work.path().join("outside-export")
            )
            .is_err()
    );
}

#[test]
fn commission_attribution_is_explicit_and_survives_responsibility_handoff() {
    let mut f = fixture();
    let mut p = discovery(&mut f, "referred");
    let commission = sources::doc(
        &f.root,
        "commission.json",
        json!({"schema":"openagents.sales.partner-commission-reference.v1",
        "uses_commission":false,"accepted":true,"attribution_id":"original-attribution","referrer_id":"original-referrer"}),
    );
    p.commission = Some(Commission {
        agreement: commission,
        attribution_id: "original-attribution".into(),
        referrer_id: "original-referrer".into(),
    });
    p = approve(&mut f, p);
    assert!(
        apply(
            &mut f,
            "operator",
            "no-explicit-commission",
            Operation::ProposePartner {
                proposal: p.clone()
            }
        )
        .is_err()
    );
    p.commission.as_mut().unwrap().agreement = sources::doc(
        &f.root,
        "accepted-commission.json",
        json!({"schema":"openagents.sales.partner-commission-reference.v1",
        "uses_commission":true,"accepted":true,"attribution_id":"original-attribution","referrer_id":"original-referrer"}),
    );
    p = approve(&mut f, p);
    apply(
        &mut f,
        "operator",
        "explicit-commission",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    accept(&mut f, &p);
    let proof = evidence(&f, "handoff");
    action(
        &mut f,
        "partner",
        "handoff",
        &p.id,
        Action::ProposeHandoff {
            target: "operator".into(),
            expires_at: 1200,
            evidence: proof.clone(),
        },
    )
    .unwrap();
    action(
        &mut f,
        "operator",
        "handoff-ack",
        &p.id,
        Action::AcceptHandoff { evidence: proof },
    )
    .unwrap();
    let view = f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap();
    assert_eq!(
        view["assignment"]["proposal"]["commission"]["attribution_id"],
        "original-attribution"
    );
    assert_eq!(view["commission_eligibility_verified"], false);
    assert_eq!(view["authority_granted"], false);
}

#[test]
fn expired_handoff_keeps_its_original_target_evidence_and_requires_a_new_decision() {
    let mut f = fixture();
    let p = discovery(&mut f, "intro");
    apply(
        &mut f,
        "operator",
        "propose",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    accept(&mut f, &p);
    let proof = evidence(&f, "handoff");
    action(
        &mut f,
        "partner",
        "handoff",
        &p.id,
        Action::ProposeHandoff {
            target: "operator".into(),
            expires_at: 1200,
            evidence: proof.clone(),
        },
    )
    .unwrap();
    f.store.clock = after_handoff_expiry;
    let view = f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap();
    assert!(view["assignment"]["handoff"].is_null());
    assert_eq!(
        view["assignment"]["events"][1]["handoff"]["target"],
        "operator"
    );
    assert_eq!(view["assignment"]["status"], "accepted");
    assert!(
        action(
            &mut f,
            "operator",
            "late-accept",
            &p.id,
            Action::AcceptHandoff { evidence: proof }
        )
        .is_err()
    );
}

#[test]
fn changed_missing_and_linked_private_sources_refuse_without_creating_an_assignment() {
    use std::os::unix::fs::symlink;
    let mut f = fixture();
    let p = discovery(&mut f, "intro");
    fs::write(f.root.join("brief"), b"changed").unwrap();
    assert!(
        apply(
            &mut f,
            "operator",
            "changed-source",
            Operation::ProposePartner {
                proposal: p.clone()
            }
        )
        .is_err()
    );
    fs::remove_file(f.root.join("brief")).unwrap();
    assert!(
        apply(
            &mut f,
            "operator",
            "missing-source",
            Operation::ProposePartner {
                proposal: p.clone()
            }
        )
        .is_err()
    );
    fs::write(f.work.path().join("outside-brief"), b"brief").unwrap();
    symlink(f.work.path().join("outside-brief"), f.root.join("brief")).unwrap();
    assert!(
        apply(
            &mut f,
            "operator",
            "linked-source",
            Operation::ProposePartner { proposal: p }
        )
        .is_err()
    );
    assert!(f.store.state.leads[&f.lead].partner_assignments.is_empty());
}

#[test]
fn fulfillment_reuses_exact_canonical_delivery_checks_support_and_payment_reference() {
    fulfillment_case(FulfillmentCase::Valid);
}

#[test]
fn fulfillment_refuses_a_different_scoped_deliverable() {
    fulfillment_case(FulfillmentCase::WrongDeliverable);
}

#[test]
fn fulfillment_refuses_customer_acceptance_before_partner_acceptance() {
    fulfillment_case(FulfillmentCase::EarlierAcceptance);
}

enum FulfillmentCase {
    Valid,
    WrongDeliverable,
    EarlierAcceptance,
}
fn fulfillment_case(case: FulfillmentCase) {
    let mut f = fixture();
    let candidate = evidence(&f, "candidate.patch");
    let check = evidence(&f, "protected-check");
    let comparison = sources::Comparison {
        manifest: evidence(&f, "comparison"),
        report: evidence(&f, "report"),
        candidate: candidate.clone(),
        check: evidence(&f, "independent-check"),
        decision: evidence(&f, "customer-decision"),
        frozen_checks: vec![check.clone()],
    };
    let mut admission = sources::admission(
        &f.root,
        if matches!(case, FulfillmentCase::EarlierAcceptance) {
            995
        } else {
            1010
        },
        &f.lead,
        "synthetic-account",
        "offer-v1",
        comparison,
    );
    let mut obligation = sources::fulfillment(&f.root, 1000, &admission);
    let support_boundary = evidence(&f, "support-boundary");
    let scoped_deliverable = if matches!(case, FulfillmentCase::WrongDeliverable) {
        evidence(&f, "different-deliverable")
    } else {
        candidate.clone()
    };
    let scope = sources::doc(
        &f.root,
        "partner-scope.json",
        json!({"schema":"openagents.sales.partner-fulfillment-scope.v1",
        "deliverable":scoped_deliverable,"protected_checks":[check],"revision_limit":1,"rework_limit":1,"support_human":"operator","support_boundary":support_boundary}),
    );
    let mut agreement: Value =
        serde_json::from_slice(&fs::read(f.root.join(&obligation.agreement.path)).unwrap())
            .unwrap();
    agreement["responsible_human"] = json!("partner");
    agreement["partner_scope"] = json!(scope);
    obligation.agreement = sources::doc(&f.root, "partner-fulfillment-agreement.json", agreement);
    obligation.responsible_human = "partner".into();
    obligation.acceptance = sources::doc(
        &f.root,
        "partner-fulfillment-acceptance.json",
        json!({"schema":"openagents.sales.fulfillment-acceptance.v1",
        "agreement_sha256":obligation.agreement.sha256,"responsible_human":"partner","accepted_at":1000,"acceptance_evidence":evidence(&f,"fulfiller-ack")}),
    );
    admission.fulfillment = Some(obligation.clone());
    let mut p = discovery(&mut f, "fulfill");
    p.terms = Terms::Fulfillment {
        brief: evidence(&f, "fulfillment-brief"),
        scope,
        offer_version: admission.offer_version.clone(),
        service_sale: admission.id.clone(),
        invoice_id: admission.invoice.id.clone(),
        obligation: obligation.clone(),
    };
    p = approve(&mut f, p);
    apply(
        &mut f,
        "operator",
        "propose-fulfill",
        Operation::ProposePartner {
            proposal: p.clone(),
        },
    )
    .unwrap();
    accept(&mut f, &p);
    let report = sources::doc(
        &f.root,
        "partner-delivery.json",
        json!({"schema":"openagents.sales.partner-delivery.v1","assignment":p.id,
        "proposal_sha256":f.store.partner_show(&f.owner,&f.lead,&p.id).unwrap()["assignment"]["proposal_sha256"],
        "service_sale":admission.id,"candidate_sha256":candidate.sha256,"revision_count":1,"rework_count":0}),
    );
    assert!(
        action(
            &mut f,
            "partner",
            "no-canonical-sale",
            &p.id,
            Action::Deliver {
                evidence: report.clone()
            }
        )
        .is_err()
    );
    f.store.clock = after_delivery;
    apply(
        &mut f,
        "operator",
        "record-sale",
        Operation::RecordServiceSale {
            admission: admission.clone(),
        },
    )
    .unwrap();
    let mut invalid_report: Value =
        serde_json::from_slice(&fs::read(f.root.join(&report.path)).unwrap()).unwrap();
    invalid_report["rework_count"] = json!(2);
    let too_much_rework = sources::doc(&f.root, "too-much-rework.json", invalid_report);
    assert!(
        action(
            &mut f,
            "partner",
            "too-much-rework",
            &p.id,
            Action::Deliver {
                evidence: too_much_rework
            }
        )
        .is_err()
    );
    let delivery = action(
        &mut f,
        "partner",
        "deliver",
        &p.id,
        Action::Deliver { evidence: report },
    );
    if !matches!(case, FulfillmentCase::Valid) {
        assert!(
            delivery
                .unwrap_err()
                .contains("same canonical service terms")
        );
        assert_eq!(
            f.store.state.leads[&f.lead].partner_assignments[&p.id].status,
            Status::Accepted
        );
        assert_eq!(f.store.state.leads[&f.lead].service_sales.len(), 1);
        return;
    }
    delivery.unwrap();
    let handoff = evidence(&f, "support-handoff");
    action(
        &mut f,
        "partner",
        "handoff",
        &p.id,
        Action::ProposeHandoff {
            target: "operator".into(),
            expires_at: 1200,
            evidence: handoff.clone(),
        },
    )
    .unwrap();
    action(
        &mut f,
        "operator",
        "support-accept",
        &p.id,
        Action::AcceptHandoff { evidence: handoff },
    )
    .unwrap();
    let view = f.store.partner_show(&f.owner, &f.lead, &p.id).unwrap();
    assert_eq!(view["assignment"]["status"], "completed");
    assert_eq!(view["assignment"]["delivery_sale"], admission.id);
    assert_eq!(f.store.state.leads[&f.lead].service_sales.len(), 1);
    assert!(
        f.store
            .service_show(&f.owner, &f.lead, &admission.id)
            .unwrap()
            .payments
            .is_empty()
    );
    assert_eq!(
        f.store
            .service_show(&f.owner, &f.lead, &admission.id)
            .unwrap()
            .admission
            .fulfillment,
        Some(obligation)
    );
    let paid = receipts::service_sale::PaymentInput {
        disposition: receipts::service_sale::Disposition::Paid,
        external_reference: Some("synthetic-service-payment".into()),
        paid_minor: Some(25000),
        reversed_minor: None,
        evidence: evidence(&f, "payment-verification"),
    };
    apply(
        &mut f,
        "operator",
        "reconcile-service-payment",
        Operation::ReconcileServicePayment {
            sale: admission.id.clone(),
            payment: paid,
        },
    )
    .unwrap();
    let billed = sources::fulfillment_input(&f.root, 1020, true);
    apply(
        &mut f,
        "operator",
        "reconcile-fulfillment",
        Operation::ReconcileServiceFulfillment {
            sale: admission.id.clone(),
            fulfillment: billed,
        },
    )
    .unwrap();
    let view = f.store.partner_show(&f.partner, &f.lead, &p.id).unwrap();
    assert!(view["canonical_fulfillment"]["bill"].is_object());
    assert!(view["canonical_fulfillment"]["payment"].is_object());
    assert_eq!(view["live_payment_qualified"], false);
    assert_eq!(f.store.state.leads[&f.lead].service_sales.len(), 1);
}
