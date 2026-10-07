//! Side-effect-free checks for the manual pilot and handoff source contracts.

use super::{Admission, Reference, digest, identifier};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Facts {
    pub task_digest: String,
    pub frozen_checks: Vec<Reference>,
    pub candidate_sha256: String,
    pub runbook: Reference,
    pub comparison_manifest: Reference,
    pub comparison_report: Reference,
    pub accepted_checks: Vec<Reference>,
    pub deliverables: Vec<Reference>,
    pub accepted_at: u64,
    pub customer_decision_maker: String,
    pub customer_decision_evidence: Reference,
    pub support_human: String,
    pub invoice_retention_reference: String,
}
impl Facts {
    pub fn validate(&self) -> Result<(), String> {
        digest(&self.task_digest)?;
        digest(&self.candidate_sha256)?;
        for value in [
            &self.customer_decision_maker,
            &self.support_human,
            &self.invoice_retention_reference,
        ] {
            identifier(value)?;
        }
        for r in [
            &self.runbook,
            &self.comparison_manifest,
            &self.comparison_report,
            &self.customer_decision_evidence,
        ]
        .into_iter()
        .chain(&self.accepted_checks)
        .chain(&self.frozen_checks)
        .chain(&self.deliverables)
        {
            r.validate()?;
        }
        if self.accepted_checks.is_empty()
            || self.frozen_checks.is_empty()
            || self.frozen_checks.len() > 8
            || self.accepted_checks.len() > 8
            || self.deliverables.is_empty()
            || self.deliverables.len() > 16
        {
            return Err("accepted service evidence exceeds bounds".into());
        }
        Ok(())
    }
}

fn value<'a>(doc: &'a Value, field: &str) -> Result<&'a Value, String> {
    field.split('.').try_fold(doc, |v, key| {
        v.get(key)
            .ok_or_else(|| "required service source field is missing".into())
    })
}
fn string<'a>(doc: &'a Value, field: &str) -> Result<&'a str, String> {
    let s = value(doc, field)?
        .as_str()
        .ok_or("required service source text is missing")?;
    identifier(s)?;
    Ok(s)
}
fn number(doc: &Value, field: &str) -> Result<u64, String> {
    value(doc, field)?
        .as_u64()
        .ok_or_else(|| "required service source integer is missing".into())
}
fn reference(doc: &Value, field: &str) -> Result<Reference, String> {
    let r: Reference = serde_json::from_value(value(doc, field)?.clone())
        .map_err(|_| "malformed service source reference")?;
    r.validate()?;
    Ok(r)
}
fn refs(doc: &Value, field: &str, max: usize) -> Result<Vec<Reference>, String> {
    let values = value(doc, field)?
        .as_array()
        .ok_or("missing service evidence list")?;
    if values.is_empty() || values.len() > max {
        return Err("service evidence list exceeds bound".into());
    }
    values
        .iter()
        .map(|v| {
            let r: Reference =
                serde_json::from_value(v.clone()).map_err(|_| "malformed service evidence list")?;
            r.validate()?;
            Ok(r)
        })
        .collect()
}
fn read<F>(r: &Reference, resolver: &mut F) -> Result<Vec<u8>, String>
where
    F: FnMut(&Reference) -> Result<Vec<u8>, String>,
{
    r.validate()?;
    let bytes = resolver(r)?;
    if bytes.is_empty()
        || bytes.len() > 8 * 1024 * 1024
        || format!("{:x}", Sha256::digest(&bytes)) != r.sha256
    {
        return Err("service evidence is missing, oversized, or changed".into());
    }
    Ok(bytes)
}
fn json<F>(r: &Reference, schema: &str, resolver: &mut F) -> Result<Value, String>
where
    F: FnMut(&Reference) -> Result<Vec<u8>, String>,
{
    let bytes = read(r, resolver)?;
    if bytes.len() > 65536 {
        return Err("private service contract exceeds document bound".into());
    }
    let v: Value =
        serde_json::from_slice(&bytes).map_err(|_| "malformed private service contract")?;
    if string(&v, "schema")? != schema {
        return Err("unsupported private service contract schema".into());
    }
    Ok(v)
}
fn same(a: &Reference, b: &Reference) -> Result<(), String> {
    if a != b {
        Err("private service source linkage disagrees".into())
    } else {
        Ok(())
    }
}
fn before(doc: &Value, field: &str, now: u64) -> Result<u64, String> {
    let at = number(doc, field)?;
    if at > now {
        return Err("private service evidence is in the future".into());
    }
    Ok(at)
}

/// Verify frozen manual sources against the admitted customer, invoice, and
/// scope. The caller owns access, bounded reads, and independent task checks.
pub fn verify_sources<F>(
    admission: &Admission,
    lead: &str,
    account: &str,
    revision: u64,
    now: u64,
    mut resolver: F,
) -> Result<Facts, String>
where
    F: FnMut(&Reference) -> Result<Vec<u8>, String>,
{
    admission.validate()?;
    let s = &admission.sources;
    let agreement = json(
        &s.agreement,
        "openagents.sales.pilot-agreement.v1",
        &mut resolver,
    )?;
    let agreement_acceptance = json(
        &s.agreement_acceptance,
        "openagents.sales.pilot-agreement-acceptance.v1",
        &mut resolver,
    )?;
    let review = json(
        &s.pilot_review,
        "openagents.sales.pilot-review.v1",
        &mut resolver,
    )?;
    let handoff = json(
        &s.handoff,
        "openagents.sales.delivery-handoff.v1",
        &mut resolver,
    )?;
    let customer = json(
        &s.customer_acceptance,
        "openagents.sales.delivery-acceptance.v1",
        &mut resolver,
    )?;
    let support = json(
        &s.support_acceptance,
        "openagents.sales.support-acceptance.v1",
        &mut resolver,
    )?;
    let invoice = &admission.invoice;
    if string(&agreement, "pipeline_lead")? != lead
        || string(&review, "pipeline_lead")? != lead
        || string(&handoff, "pipeline_lead")? != lead
        || string(&agreement, "customer.account")? != account
        || string(&handoff, "customer_account")? != account
        || string(&agreement, "offer_version")? != admission.offer_version
        || string(&handoff, "offer_version")? != admission.offer_version
        || number(&agreement, "pipeline_revision")? > revision
        || number(&review, "expected_pipeline_revision")? > revision
        || number(&handoff, "expected_pipeline_revision")? > revision
        || string(&agreement, "commercial.kind")? != "service_invoice_after_acceptance"
        || string(&agreement, "commercial.currency")? != invoice.currency
        || number(&agreement, "commercial.currency_scale")? != invoice.currency_scale
        || number(&agreement, "commercial.service_fee_minor_units")? != invoice.amount_minor
        || string(&agreement, "commercial.external_payment_route_reference")?
            != invoice.payment_route_reference
    {
        return Err("service invoice/account/offer differs from the accepted pilot scope".into());
    }
    string(&agreement, "commercial.owner_price_approval_reference")?;
    let due_days = number(&agreement, "commercial.invoice_due_calendar_days")?;
    let due_at = due_days
        .checked_mul(86400)
        .and_then(|days| invoice.issued_at.checked_add(days))
        .ok_or("service due date overflow")?;
    if due_days == 0 || invoice.due_at != due_at {
        return Err("service invoice due date differs from accepted terms".into());
    }
    for field in [
        "id",
        "version",
        "customer.workflow_owner",
        "responsibility.accepted_delivery_human",
        "responsibility.ownership_acceptance_reference",
        "scope.input_rights_reference",
        "scope.independent_checker",
        "scope.baseline_inventory_reference",
        "scope.baseline_manifest_reference",
        "commercial.provider_budget_reference",
        "data.consent_reference",
        "data.permitted_use",
    ] {
        string(&agreement, field)?;
    }
    let task_digest = string(&agreement, "scope.task_digest")?.to_owned();
    digest(&task_digest)?;
    let frozen_checks = refs(&agreement, "scope.frozen_check_refs", 8)?;
    for r in &frozen_checks {
        read(r, &mut resolver)?;
    }
    string(&agreement_acceptance, "owner_reference")?;
    string(&agreement_acceptance, "buyer_reference")?;
    let agreement_at = before(&agreement_acceptance, "accepted_at", now)?;
    if string(&agreement_acceptance, "agreement_reference")? != s.agreement.path
        || string(&agreement_acceptance, "exact_agreement_sha256")? != s.agreement.sha256
        || string(&review, "agreement_reference")? != s.agreement.path
        || string(&review, "agreement_sha256")? != s.agreement.sha256
    {
        return Err("service agreement acceptance or review names another version".into());
    }
    same(&reference(&handoff, "agreement")?, &s.agreement)?;
    same(
        &reference(&handoff, "agreement_acceptance")?,
        &s.agreement_acceptance,
    )?;
    same(&reference(&handoff, "pilot_review")?, &s.pilot_review)?;
    same(&reference(&customer, "handoff")?, &s.handoff)?;
    same(&reference(&support, "handoff")?, &s.handoff)?;
    let candidate = string(&handoff, "candidate_sha256")?.to_owned();
    digest(&candidate)?;
    let runbook = reference(&handoff, "runbook")?;
    let manifest = reference(&handoff, "sales_evidence_manifest")?;
    let report = reference(&handoff, "sales_evidence_report")?;
    if string(&review, "customer_decision.decision")? != "accept"
        || string(&customer, "decision")? != "accept"
        || string(&review, "evidence.candidate_sha256")? != candidate
        || string(&review, "customer_decision.accepted_candidate_sha256")? != candidate
        || string(&customer, "accepted_candidate_sha256")? != candidate
        || string(&review, "evidence.report_sha256")? != report.sha256
        || string(&review, "evidence.sales_evidence_manifest_reference")? != manifest.path
        || string(&review, "evidence.sales_evidence_report_reference")? != report.path
        || string(&review, "evidence.runbook_reference")? != runbook.path
        || value(
            &review,
            "evidence.all_failed_repair_retry_attempts_included",
        )?
        .as_bool()
            != Some(true)
        || !value(&customer, "unresolved_defects")?
            .as_array()
            .is_some_and(Vec::is_empty)
    {
        return Err("service delivery lacks exact accepted and complete pilot evidence".into());
    }
    same(&reference(&customer, "accepted_runbook")?, &runbook)?;
    let customer_human = string(&customer, "decision_maker")?;
    if customer_human != string(&agreement, "customer.acceptance_decision_maker")?
        || customer_human != string(&review, "customer_decision.decision_maker")?
    {
        return Err("service result decision differs from the agreed customer".into());
    }
    let accepted_at = before(&customer, "at", now)?;
    let delivered_at = before(&handoff, "delivered_at", now)?;
    let reviewed_at = before(&review, "reviewed_at", now)?;
    let decision_at = before(&review, "customer_decision.at", now)?;
    if agreement_at > reviewed_at
        || reviewed_at > delivered_at
        || decision_at < reviewed_at
        || decision_at > delivered_at
        || accepted_at < delivered_at
        || invoice.issued_at < accepted_at
        || invoice.issued_at < reviewed_at
        || invoice.issued_at > now
    {
        return Err("service invoice precedes accepted delivery or is in the future".into());
    }
    let checks = refs(&handoff, "accepted_checks", 8)?;
    if !checks.iter().any(|r| {
        r.path == string(&review, "evidence.independent_check_review_reference").unwrap_or("")
    }) {
        return Err("service handoff omits the independent review reference".into());
    }
    let deliverables = value(&handoff, "deliverables")?
        .as_array()
        .ok_or("service deliverables are missing")?;
    if deliverables.is_empty() || deliverables.len() > 16 {
        return Err("service deliverable bound".into());
    }
    let mut ids = BTreeSet::new();
    let mut artifacts = Vec::new();
    for d in deliverables {
        if !ids.insert(string(d, "id")?.to_owned()) {
            return Err("duplicate service deliverable".into());
        }
        string(d, "kind")?;
        string(d, "version")?;
        artifacts.push(reference(d, "artifact")?);
    }
    let accepted_ids = value(&customer, "accepted_deliverable_ids")?
        .as_array()
        .ok_or("accepted deliverable identities are missing")?;
    let accepted_ids = accepted_ids
        .iter()
        .map(|v| {
            v.as_str()
                .map(str::to_owned)
                .ok_or("malformed accepted deliverable identity")
        })
        .collect::<Result<Vec<_>, _>>()?;
    if accepted_ids.len() != ids.len() || accepted_ids.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err("customer did not accept every exact service deliverable".into());
    }
    let support_human = string(&handoff, "support.responsible_human")?;
    if string(&support, "responsible_human")? != support_human {
        return Err("service support owner has not accepted the handoff".into());
    }
    for field in [
        "support.contact_reference",
        "support.business_hours",
        "support.response_boundary",
        "support.included_work",
        "support.out_of_scope_route",
    ] {
        string(&handoff, field)?;
    }
    if number(&handoff, "support.ends_at")? < accepted_at {
        return Err("service support boundary ended before acceptance".into());
    }
    if before(&support, "accepted_at", now)? < delivered_at {
        return Err("service support acceptance precedes the delivered handoff".into());
    }
    let decision = reference(&customer, "decision_evidence")?;
    let support_evidence = reference(&support, "acceptance_evidence")?;
    for r in [
        &invoice.evidence,
        &runbook,
        &manifest,
        &report,
        &decision,
        &support_evidence,
    ]
    .into_iter()
    .chain(&checks)
    .chain(&artifacts)
    {
        read(r, &mut resolver)?;
    }
    if let Some(f) = &admission.fulfillment {
        verify_fulfillment(
            f,
            account,
            &admission.offer_version,
            &invoice.id,
            now,
            |r| read(r, &mut resolver),
        )?;
    }
    let facts = Facts {
        task_digest,
        frozen_checks,
        candidate_sha256: candidate,
        runbook,
        comparison_manifest: manifest,
        comparison_report: report,
        accepted_checks: checks,
        deliverables: artifacts,
        accepted_at,
        customer_decision_maker: customer_human.into(),
        customer_decision_evidence: decision,
        support_human: support_human.into(),
        invoice_retention_reference: string(
            &agreement,
            "data.invoice_consent_retention_reference",
        )?
        .into(),
    };
    facts.validate()?;
    Ok(facts)
}

/// Verify separately priced fulfillment and retained billing/payment evidence.
pub fn verify_fulfillment<F>(
    f: &super::Fulfillment,
    account: &str,
    offer: &str,
    invoice_id: &str,
    now: u64,
    mut resolver: F,
) -> Result<(), String>
where
    F: FnMut(&Reference) -> Result<Vec<u8>, String>,
{
    let agreement = json(
        &f.agreement,
        "openagents.sales.fulfillment-agreement.v1",
        &mut resolver,
    )?;
    let accepted = json(
        &f.acceptance,
        "openagents.sales.fulfillment-acceptance.v1",
        &mut resolver,
    )?;
    if string(&agreement, "id")? != f.id
        || string(&agreement, "customer_account")? != account
        || string(&agreement, "offer_version")? != offer
        || string(&agreement, "service_invoice_id")? != invoice_id
        || string(&agreement, "responsible_human")? != f.responsible_human
        || number(&agreement, "amount_minor")? != f.amount_minor
        || string(&agreement, "currency")? != f.currency
        || number(&agreement, "currency_scale")? != f.currency_scale
        || value(&agreement, "trigger")?
            != &serde_json::to_value(f.trigger).map_err(|_| "fulfillment trigger")?
        || string(&accepted, "agreement_sha256")? != f.agreement.sha256
        || string(&accepted, "responsible_human")? != f.responsible_human
    {
        return Err(
            "fulfillment needs its separately agreed price, trigger, and acceptance".into(),
        );
    }
    before(&accepted, "accepted_at", now)?;
    read(&reference(&accepted, "acceptance_evidence")?, &mut resolver)?;
    if let Some(bill) = &f.bill {
        let bill = json(bill, "openagents.sales.fulfillment-bill.v1", &mut resolver)?;
        if string(&bill, "obligation_id")? != f.id
            || number(&bill, "amount_minor")? != f.amount_minor
            || string(&bill, "currency")? != f.currency
            || number(&bill, "currency_scale")? != f.currency_scale
        {
            return Err("fulfillment bill differs from its accepted obligation".into());
        }
        read(&reference(&bill, "evidence")?, &mut resolver)?;
    }
    if let Some(paid) = &f.payment {
        let paid = json(
            paid,
            "openagents.sales.fulfillment-payment.v1",
            &mut resolver,
        )?;
        if string(&paid, "obligation_id")? != f.id
            || number(&paid, "amount_minor")? != f.amount_minor
            || string(&paid, "currency")? != f.currency
            || number(&paid, "currency_scale")? != f.currency_scale
        {
            return Err("fulfillment payment differs from its accepted obligation".into());
        }
        read(&reference(&paid, "evidence")?, &mut resolver)?;
        string(&paid, "verified_by")?;
        before(&paid, "verified_at", now)?;
    }
    Ok(())
}
