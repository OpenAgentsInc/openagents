use receipts::service_sale::{Admission, Evidence, Invoice, Reference};
use serde_json::{Value, json};
use sha2::Digest;
use std::fs;
use std::path::Path;

pub fn retain(root: &Path, name: &str, bytes: &[u8]) -> Reference {
    fs::write(root.join(name), bytes).unwrap();
    Reference {
        path: name.into(),
        sha256: sha2::Sha256::digest(bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect(),
    }
}
pub fn doc(root: &Path, name: &str, value: Value) -> Reference {
    retain(root, name, &serde_json::to_vec(&value).unwrap())
}
pub struct Comparison {
    pub manifest: Reference,
    pub report: Reference,
    pub candidate: Reference,
    pub check: Reference,
    pub decision: Reference,
    pub frozen_checks: Vec<Reference>,
}
pub fn admission(
    root: &Path,
    now: u64,
    lead: &str,
    account: &str,
    offer: &str,
    comparison: Comparison,
) -> Admission {
    let runbook = retain(
        root,
        "service-runbook",
        b"synthetic bounded operator runbook",
    );
    let support_ack = retain(
        root,
        "service-support-ack",
        b"operator accepted the synthetic support boundary",
    );
    let agreement = doc(
        root,
        "service-agreement.json",
        json!({
            "schema":"openagents.sales.pilot-agreement.v1", "id":"synthetic-agreement", "version":"synthetic-v1", "pipeline_lead":lead,"pipeline_revision":1,
            "offer_version":offer,"customer":{"account":account,"acceptance_decision_maker":"buyer","workflow_owner":"buyer"},
            "responsibility":{"accepted_delivery_human":"operator","ownership_acceptance_reference":"synthetic delivery agreement"},
            "scope":{"input_rights_reference":"synthetic input rights","independent_checker":"checker","task_digest":"b".repeat(64),
            "frozen_check_refs":comparison.frozen_checks,"baseline_inventory_reference":"inventory.json","baseline_manifest_reference":comparison.manifest.path},
            "commercial":{"kind":"service_invoice_after_acceptance","currency":"USD","currency_scale":100,
            "service_fee_minor_units":25000,"invoice_due_calendar_days":7,"owner_price_approval_reference":"synthetic owner price approval",
                "external_payment_route_reference":"synthetic-external-route","provider_payer":"buyer","provider_budget_reference":"synthetic buyer budget"},
            "data":{"invoice_consent_retention_reference":"synthetic invoice consent; host retention remains bounded",
                "consent_reference":"synthetic specific consent","permitted_use":"one synthetic service"}
        }),
    );
    let agreement_acceptance = doc(
        root,
        "service-agreement-acceptance.json",
        json!({
            "schema":"openagents.sales.pilot-agreement-acceptance.v1", "agreement_reference":agreement.path,
            "exact_agreement_sha256":agreement.sha256,"owner_reference":"synthetic owner agreement acceptance",
            "buyer_reference":"synthetic buyer agreement acceptance","accepted_at":now-5
        }),
    );
    let review = doc(
        root,
        "service-review.json",
        json!({
            "schema":"openagents.sales.pilot-review.v1","pipeline_lead":lead,"expected_pipeline_revision":1,
            "agreement_reference":agreement.path,"agreement_sha256":agreement.sha256,"reviewed_at":now-4,
            "evidence":{"candidate_sha256":comparison.candidate.sha256,"report_sha256":comparison.report.sha256,
                "sales_evidence_manifest_reference":comparison.manifest.path,"sales_evidence_report_reference":comparison.report.path,
                "runbook_reference":runbook.path,"independent_check_review_reference":comparison.check.path,
                "all_failed_repair_retry_attempts_included":true},
            "customer_decision":{"decision":"accept","decision_maker":"buyer","at":now-4,
                "accepted_candidate_sha256":comparison.candidate.sha256}
        }),
    );
    let handoff = doc(
        root,
        "service-handoff.json",
        json!({
            "schema":"openagents.sales.delivery-handoff.v1","pipeline_lead":lead,"expected_pipeline_revision":1,
            "customer_account":account,"offer_version":offer,"agreement":agreement,"agreement_acceptance":agreement_acceptance,
            "pilot_review":review,"sales_evidence_manifest":comparison.manifest,"sales_evidence_report":comparison.report,
            "candidate_sha256":comparison.candidate.sha256,"runbook":runbook,"delivered_at":now-3,
            "accepted_checks":[comparison.check],"deliverables":[{"id":"result","kind":"patch","version":"synthetic-v1","artifact":comparison.candidate}],
            "support":{"responsible_human":"operator","contact_reference":"synthetic support contact","business_hours":"synthetic hours",
                "response_boundary":"synthetic bounded response","included_work":"one synthetic correction",
                "out_of_scope_route":"new agreement","ends_at":now+100},
            "cleanup_plan":[
                {"id":"temporary-credentials","class":"credentials","target_reference":"synthetic-temporary-key",
                    "responsible_human":"operator","due_at":now+50,"operation_reference":"synthetic-revoke-key"},
                {"id":"test-data","class":"test_data","target_reference":"synthetic-test-rows",
                    "responsible_human":"operator","due_at":now+50,"operation_reference":"synthetic-delete-rows"},
                {"id":"local-copies","class":"local_copies","target_reference":"synthetic-local-copy",
                    "responsible_human":"operator","due_at":now+50,"operation_reference":"synthetic-remove-copy"}
            ]
        }),
    );
    let customer_acceptance = doc(
        root,
        "service-customer-acceptance.json",
        json!({
            "schema":"openagents.sales.delivery-acceptance.v1","handoff":handoff,"decision":"accept","decision_maker":"buyer",
            "decision_evidence":comparison.decision,"at":now-2,"accepted_candidate_sha256":comparison.candidate.sha256,
            "accepted_runbook":runbook,"accepted_deliverable_ids":["result"],"unresolved_defects":[]
        }),
    );
    let support_acceptance = doc(
        root,
        "service-support-acceptance.json",
        json!({
            "schema":"openagents.sales.support-acceptance.v1","handoff":handoff,"responsible_human":"operator",
            "acceptance_evidence":support_ack,"accepted_at":now-2
        }),
    );
    let invoice_evidence = retain(
        root,
        "service-invoice",
        b"synthetic external invoice metadata; no bank or card details",
    );
    Admission {
        id: "synthetic-sale".into(),
        offer_version: offer.into(),
        invoice: Invoice {
            id: "synthetic-invoice".into(),
            external_reference: "synthetic-external-invoice".into(),
            currency: "USD".into(),
            currency_scale: 100,
            amount_minor: 25000,
            issued_at: now - 1,
            due_at: now - 1 + 7 * 86400,
            payment_route_reference: "synthetic-external-route".into(),
            evidence: invoice_evidence,
        },
        sources: Evidence {
            agreement,
            agreement_acceptance,
            pilot_review: review,
            handoff,
            customer_acceptance,
            support_acceptance,
        },
        fulfillment: None,
    }
}
pub fn fulfillment(
    root: &Path,
    now: u64,
    admission: &Admission,
) -> receipts::service_sale::Fulfillment {
    use receipts::service_sale::{Fulfillment, FulfillmentTrigger};
    let source: Value =
        serde_json::from_slice(&fs::read(root.join(&admission.sources.agreement.path)).unwrap())
            .unwrap();
    let agreement = doc(
        root,
        "fulfillment-agreement.json",
        json!({"schema":"openagents.sales.fulfillment-agreement.v1",
        "id":"synthetic-fulfillment","customer_account":source["customer"]["account"],"offer_version":admission.offer_version,
        "service_invoice_id":admission.invoice.id,"responsible_human":"operator","amount_minor":5000,"currency":"USD",
        "currency_scale":100,"trigger":"verified_service_payment"}),
    );
    let proof = retain(
        root,
        "fulfillment-acceptance-proof",
        b"operator accepted separately priced synthetic fulfillment",
    );
    let acceptance = doc(
        root,
        "fulfillment-acceptance.json",
        json!({"schema":"openagents.sales.fulfillment-acceptance.v1",
        "agreement_sha256":agreement.sha256,"responsible_human":"operator","accepted_at":now,"acceptance_evidence":proof}),
    );
    Fulfillment {
        id: "synthetic-fulfillment".into(),
        responsible_human: "operator".into(),
        currency: "USD".into(),
        currency_scale: 100,
        amount_minor: 5000,
        trigger: FulfillmentTrigger::VerifiedServicePayment,
        agreement,
        acceptance,
        bill: None,
        payment: None,
    }
}
pub fn fulfillment_input(
    root: &Path,
    now: u64,
    paid: bool,
) -> receipts::service_sale::FulfillmentInput {
    let proof = retain(
        root,
        "fulfillment-bill-proof",
        b"synthetic separately billed fee",
    );
    let bill = doc(
        root,
        "fulfillment-bill.json",
        json!({"schema":"openagents.sales.fulfillment-bill.v1",
        "obligation_id":"synthetic-fulfillment","amount_minor":5000,"currency":"USD","currency_scale":100,"evidence":proof}),
    );
    let payment=paid.then(|| {
        let proof=retain(root,"fulfillment-payment-proof",b"synthetic exact separate payment; no referral split");
        doc(root,"fulfillment-paid.json",json!({"schema":"openagents.sales.fulfillment-payment.v1",
            "obligation_id":"synthetic-fulfillment","amount_minor":5000,"currency":"USD","currency_scale":100,
            "evidence":proof,"verified_by":"operator","verified_at":now}))
    });
    receipts::service_sale::FulfillmentInput { bill, payment }
}
