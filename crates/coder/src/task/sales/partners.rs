//! Accepted partner assignments share the private pipeline. Records grant no
//! outbound, execution, invoice, commission, or payment authority.

use super::{Access, DataBoundary, Lead, NextAction, PermissionState, Result, Role, Store};
use receipts::service_sale::{self as service, Fulfillment, Reference};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::path::Path;

pub const SCHEMA: &str = "openagents.sales.partner-assignment.v1";
pub const MAX_ASSIGNMENTS: usize = 16;
const MAX_EVENTS: usize = 16;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Commission {
    /// An explicitly accepted agreement reference, never an earned commission.
    pub agreement: Reference,
    pub attribution_id: String,
    pub referrer_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Terms {
    Discovery {
        brief: Reference,
        permitted_use: String,
    },
    Fulfillment {
        brief: Reference,
        scope: Reference,
        offer_version: String,
        service_sale: String,
        invoice_id: String,
        obligation: Fulfillment,
    },
}
impl Terms {
    fn kind(&self) -> &'static str {
        match self {
            Self::Discovery { .. } => "discovery",
            Self::Fulfillment { .. } => "fulfillment",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Proposal {
    pub id: String,
    pub recipient_human: String,
    pub expires_at: u64,
    pub next: NextAction,
    pub terms: Terms,
    pub consent: Reference,
    pub provenance: Reference,
    pub approval: Reference,
    pub commission: Option<Commission>,
}

/// Pinned in the existing fulfillment agreement as `partner_scope`.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Scope {
    pub schema: String,
    pub deliverable: Reference,
    pub protected_checks: Vec<Reference>,
    pub revision_limit: u16,
    pub rework_limit: u16,
    pub support_human: String,
    pub support_boundary: Reference,
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Proposed,
    Accepted,
    Delivered,
    Completed,
    Refused,
    TimedOut,
    Cancelled,
}
impl Status {
    fn terminal(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Refused | Self::TimedOut | Self::Cancelled
        )
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub actor: String,
    pub at: u64,
    pub outcome: String,
    pub evidence: Reference,
    pub handoff: Option<Handoff>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Handoff {
    pub target: String,
    pub proposed_at: u64,
    pub expires_at: u64,
    pub evidence: Reference,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Assignment {
    pub schema: String,
    pub proposal: Proposal,
    pub proposal_sha256: String,
    pub pipeline_lead: String,
    pub account: String,
    pub owner_human: String,
    pub data: DataBoundary,
    pub permission_reference: String,
    pub proposed_at: u64,
    pub status: Status,
    pub next: Option<NextAction>,
    pub events: Vec<Event>,
    pub handoff: Option<Handoff>,
    pub delivery_sale: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Action {
    Accept {
        proposal_sha256: String,
        evidence: Reference,
    },
    Refuse {
        proposal_sha256: String,
        evidence: Reference,
    },
    Cancel {
        evidence: Reference,
    },
    Next {
        next: NextAction,
        evidence: Reference,
    },
    Deliver {
        evidence: Reference,
    },
    ProposeHandoff {
        target: String,
        expires_at: u64,
        evidence: Reference,
    },
    AcceptHandoff {
        evidence: Reference,
    },
    RejectHandoff {
        evidence: Reference,
    },
}
impl Action {
    fn evidence(&self) -> &Reference {
        match self {
            Self::Accept { evidence, .. }
            | Self::Refuse { evidence, .. }
            | Self::Cancel { evidence }
            | Self::Next { evidence, .. }
            | Self::Deliver { evidence }
            | Self::ProposeHandoff { evidence, .. }
            | Self::AcceptHandoff { evidence }
            | Self::RejectHandoff { evidence } => evidence,
        }
    }
}

fn reference(reader: &mut super::service::Reader<'_>, value: &Reference) -> Result<()> {
    reader.read(value).map(|_| ())
}
fn document(
    reader: &mut super::service::Reader<'_>,
    value: &Reference,
    schema: &str,
) -> Result<Value> {
    let bytes = reader.read(value)?;
    if bytes.len() > 64 * 1024 {
        return Err("Partner contract exceeds its document bound.".into());
    }
    let doc: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid partner contract JSON.")?;
    if doc["schema"] != schema {
        return Err("Unsupported partner contract schema.".into());
    }
    Ok(doc)
}
fn field(doc: &Value, name: &str) -> Result<Reference> {
    serde_json::from_value(doc[name].clone())
        .map_err(|_| "Missing partner source reference.".into())
}
fn fingerprint(
    p: &Proposal,
    lead: &str,
    account: &str,
    data: &DataBoundary,
    permission: &str,
) -> Result<String> {
    let mut proposal = serde_json::to_value(p).map_err(|_| "Invalid partner proposal.")?;
    proposal.as_object_mut().unwrap().remove("approval");
    let bytes = serde_json::to_vec(&json!({"proposal":proposal,"pipeline_lead":lead,
        "account":account,"data":data,"permission_reference":permission}))
    .map_err(|_| "Invalid partner proposal.")?;
    Ok(super::digest(&bytes))
}
fn validate(p: &Proposal, proposed_at: u64, retain_until: u64) -> Result<()> {
    super::id(&p.id)?;
    super::id(&p.recipient_human)?;
    if p.expires_at <= proposed_at || p.expires_at > retain_until || p.next.due_at > p.expires_at {
        return Err("Partner proposal needs a bounded expiry and next action.".into());
    }
    super::text(&p.next.description, 2048)?;
    for r in [&p.consent, &p.provenance, &p.approval] {
        r.validate()?;
    }
    match &p.terms {
        Terms::Discovery {
            brief,
            permitted_use,
        } => {
            brief.validate()?;
            super::text(permitted_use, 2048)?;
        }
        Terms::Fulfillment {
            brief,
            scope,
            offer_version,
            service_sale,
            invoice_id,
            obligation,
        } => {
            for r in [brief, scope, &obligation.agreement, &obligation.acceptance] {
                r.validate()?;
            }
            for value in [offer_version, service_sale, invoice_id] {
                super::text(value, 256)?;
            }
            if obligation.responsible_human != p.recipient_human
                || obligation.amount_minor == 0
                || obligation.bill.is_some()
                || obligation.payment.is_some()
            {
                return Err("Partner fulfillment must pin one accepted, unbilled obligation and its exact human.".into());
            }
            receipts::service_sale::validate_fulfillment_amount(
                &obligation.currency,
                obligation.currency_scale,
                obligation.amount_minor,
            )?;
        }
    }
    if let Some(c) = &p.commission {
        c.agreement.validate()?;
        super::id(&c.attribution_id)?;
        super::id(&c.referrer_id)?;
    }
    Ok(())
}
impl Assignment {
    pub(super) fn validate(&self) -> Result<()> {
        validate(&self.proposal, self.proposed_at, self.data.retain_until)?;
        if self.schema != SCHEMA
            || self.events.len() > MAX_EVENTS
            || self.data.recipients.len() > 16
            || self.proposal_sha256
                != fingerprint(
                    &self.proposal,
                    &self.pipeline_lead,
                    &self.account,
                    &self.data,
                    &self.permission_reference,
                )?
            || !self
                .data
                .recipients
                .contains(&format!("human:{}", self.owner_human))
            || !self
                .data
                .recipients
                .contains(&format!("human:{}", self.proposal.recipient_human))
        {
            return Err("Invalid retained partner assignment.".into());
        }
        for event in &self.events {
            super::id(&event.actor)?;
            event.evidence.validate()?;
            if event.at < self.proposed_at || event.at >= self.proposal.expires_at {
                return Err("Partner event is outside its admitted period.".into());
            }
        }
        let accepted = self
            .events
            .iter()
            .any(|e| e.outcome == "accepted" && e.actor == self.proposal.recipient_human);
        if matches!(
            self.status,
            Status::Accepted | Status::Delivered | Status::Completed
        ) && !accepted
            || matches!(self.status, Status::Delivered | Status::Completed)
                && matches!(self.proposal.terms, Terms::Fulfillment { .. })
                && self.delivery_sale.is_none()
            || self.status == Status::Completed
                && !self.events.iter().any(|e| e.outcome == "handoff_accepted")
        {
            return Err(
                "Partner state is missing its explicit acceptance or delivery history.".into(),
            );
        }
        Ok(())
    }
    pub(super) fn retire(&mut self, now: u64, unavailable: bool) -> bool {
        if self.status.terminal() {
            return false;
        }
        if unavailable || now >= self.proposal.expires_at {
            self.status = if unavailable {
                Status::Cancelled
            } else {
                Status::TimedOut
            };
            self.next = None;
            self.handoff = None;
            return true;
        }
        if self.handoff.as_ref().is_some_and(|h| now >= h.expires_at) {
            self.handoff = None;
            return true;
        }
        false
    }
}
impl Store {
    pub fn partner_digest(
        &mut self,
        access: &Access,
        lead: &str,
        proposal: &Proposal,
    ) -> Result<Value> {
        self.refresh()?;
        self.admin(access)?;
        let lead = self.state.leads.get(lead).ok_or("Lead is unavailable.")?;
        self.readable(access, lead)?;
        validate(proposal, (self.clock)(), lead.details.data.retain_until)?;
        if proposal.expires_at > lead.details.permission.expires_at {
            return Err("Partner expiry exceeds recorded permission.".into());
        }
        Ok(
            json!({"proposal_sha256":fingerprint(proposal,&lead.id,&lead.details.account,
            &lead.details.data,&lead.details.permission.reference)?,"authority_granted":false}),
        )
    }
    pub(super) fn propose_partner(
        &self,
        access: &Access,
        lead: &Lead,
        p: &Proposal,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Assignment> {
        self.admin(access)?;
        self.readable(access, lead)?;
        validate(p, now, lead.details.data.retain_until)?;
        if lead.partner_assignments.len() >= MAX_ASSIGNMENTS
            || lead.partner_assignments.contains_key(&p.id)
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || p.expires_at > lead.details.permission.expires_at
            || !Self::recipient(&lead.details, &p.recipient_human)
            || !self
                .state
                .principals
                .get(&p.recipient_human)
                .is_some_and(|human| human.active && human.role != Role::Reader)
        {
            return Err("Partner proposal needs current permission, an admitted human, and a distinct bounded assignment.".into());
        }
        let digest = fingerprint(
            p,
            &lead.id,
            &lead.details.account,
            &lead.details.data,
            &lead.details.permission.reference,
        )?;
        let mut reader = super::service::Reader::new(root)?;
        reference(&mut reader, &p.consent)?;
        reference(&mut reader, &p.provenance)?;
        let approval = document(
            &mut reader,
            &p.approval,
            "openagents.sales.partner-approval.v1",
        )?;
        if approval["pipeline_lead"] != lead.id
            || approval["assignment"] != p.id
            || approval["proposal_sha256"] != digest
            || approval["approved_by"] != access.principal()
            || approval["allow_private_assignment"] != true
            || !approval["approved_at"]
                .as_u64()
                .is_some_and(|at| at <= now && at >= lead.details.permission.recorded_at)
        {
            return Err(
                "Partner approval must name this exact proposal, lead, and current owner.".into(),
            );
        }
        match &p.terms {
            Terms::Discovery { brief, .. } => reference(&mut reader, brief)?,
            Terms::Fulfillment {
                brief,
                scope,
                offer_version,
                invoice_id,
                obligation,
                ..
            } => {
                reference(&mut reader, brief)?;
                service::verify_fulfillment(
                    obligation,
                    &lead.details.account,
                    offer_version,
                    invoice_id,
                    now,
                    |r| reader.read(r),
                )?;
                let agreement = document(
                    &mut reader,
                    &obligation.agreement,
                    "openagents.sales.fulfillment-agreement.v1",
                )?;
                if field(&agreement, "partner_scope")? != *scope {
                    return Err("Fulfillment agreement must pin this exact partner scope.".into());
                }
                let scope = self.partner_scope(&mut reader, scope, lead)?;
                if self.state.leads.values().flat_map(|lead| lead.partner_assignments.values()).any(|assignment| {
                    !assignment.status.terminal() && matches!(&assignment.proposal.terms, Terms::Fulfillment { obligation: prior, .. } if prior.id == obligation.id)
                }) { return Err("A fulfillment obligation already has an active partner assignment.".into()); }
                if !Self::recipient(&lead.details, &scope.support_human) {
                    return Err("Partner support is outside admitted recipients.".into());
                }
            }
        }
        if let Some(c) = &p.commission {
            let agreement = document(
                &mut reader,
                &c.agreement,
                "openagents.sales.partner-commission-reference.v1",
            )?;
            if agreement["uses_commission"] != true
                || agreement["attribution_id"] != c.attribution_id
                || agreement["referrer_id"] != c.referrer_id
                || agreement["accepted"] != true
            {
                return Err("Commission references require explicitly accepted terms and independent attribution.".into());
            }
        }
        Ok(Assignment {
            schema: SCHEMA.into(),
            proposal: p.clone(),
            proposal_sha256: digest,
            pipeline_lead: lead.id.clone(),
            account: lead.details.account.clone(),
            owner_human: access.principal().into(),
            data: lead.details.data.clone(),
            permission_reference: lead.details.permission.reference.clone(),
            proposed_at: now,
            status: Status::Proposed,
            next: Some(p.next.clone()),
            events: vec![],
            handoff: None,
            delivery_sale: None,
        })
    }
    fn partner_scope(
        &self,
        reader: &mut super::service::Reader<'_>,
        r: &Reference,
        lead: &Lead,
    ) -> Result<Scope> {
        let value = document(reader, r, "openagents.sales.partner-fulfillment-scope.v1")?;
        let scope: Scope =
            serde_json::from_value(value).map_err(|_| "Invalid partner fulfillment scope.")?;
        super::id(&scope.support_human)?;
        if scope.revision_limit > 16
            || scope.rework_limit > 16
            || scope.protected_checks.is_empty()
            || scope.protected_checks.len() > 8
            || !Self::recipient(&lead.details, &scope.support_human)
        {
            return Err(
                "Partner scope needs protected checks, bounded rework, and admitted support."
                    .into(),
            );
        }
        for r in [&scope.deliverable, &scope.support_boundary]
            .into_iter()
            .chain(&scope.protected_checks)
        {
            reference(reader, r)?;
        }
        Ok(scope)
    }
    pub(super) fn advance_partner(
        &self,
        access: &Access,
        lead: &Lead,
        id: &str,
        action: &Action,
        root: Option<&Path>,
        now: u64,
    ) -> Result<Assignment> {
        let role = self.check(access)?;
        let prior = lead
            .partner_assignments
            .get(id)
            .ok_or("Partner assignment is unavailable.")?;
        prior.validate()?;
        let actor = access.principal();
        if prior.status.terminal()
            || now >= prior.proposal.expires_at
            || now >= prior.data.retain_until
            || lead.details.permission.state != PermissionState::Granted
            || lead.details.permission.expires_at <= now
            || !Self::recipient(&lead.details, actor)
            || !prior.data.recipients.contains(&format!("human:{actor}"))
            || prior.events.len() >= MAX_EVENTS
            || prior.events.len() >= MAX_EVENTS - 1
                && !matches!(action, Action::Cancel { .. } | Action::Refuse { .. })
        {
            return Err(
                "Partner action exceeds current admission or its bounded active period.".into(),
            );
        }
        let coordinator = role == Role::Owner && actor == prior.owner_human;
        let recipient = actor == prior.proposal.recipient_human;
        let mut reader = super::service::Reader::new(root)?;
        reference(&mut reader, action.evidence())?;
        let mut next = prior.clone();
        let outcome;
        match action {
            Action::Accept {
                proposal_sha256, ..
            }
            | Action::Refuse {
                proposal_sha256, ..
            } => {
                if !recipient
                    || prior.status != Status::Proposed
                    || proposal_sha256 != &prior.proposal_sha256
                {
                    return Err(
                        "Only the proposed recipient may decide the exact pending proposal.".into(),
                    );
                }
                if matches!(action, Action::Accept { .. }) {
                    next.status = Status::Accepted;
                    outcome = "accepted";
                } else {
                    next.status = Status::Refused;
                    next.next = None;
                    outcome = "refused";
                }
            }
            Action::Cancel { .. } => {
                if !coordinator && !recipient {
                    return Err("Only the assignment owner or recipient may cancel.".into());
                }
                next.status = Status::Cancelled;
                next.next = None;
                next.handoff = None;
                outcome = "cancelled";
            }
            Action::Next { next: action, .. } => {
                if !coordinator && !(recipient && prior.status != Status::Proposed) {
                    return Err("Partner next action requires accepted responsibility.".into());
                }
                super::text(&action.description, 2048)?;
                if action.due_at > prior.proposal.expires_at {
                    return Err("Next action exceeds partner expiry.".into());
                }
                next.next = Some(action.clone());
                outcome = "next_action";
            }
            Action::Deliver { evidence } => {
                if !recipient || prior.status != Status::Accepted {
                    return Err(
                        "Only an accepted fulfillment recipient may record delivery.".into(),
                    );
                }
                let Terms::Fulfillment {
                    scope,
                    service_sale,
                    offer_version,
                    invoice_id,
                    obligation,
                    ..
                } = &prior.proposal.terms
                else {
                    return Err(
                        "A discovery assignment cannot delegate or bill fulfillment.".into(),
                    );
                };
                let scope = self.partner_scope(&mut reader, scope, lead)?;
                let sale = lead
                    .service_sales
                    .get(service_sale)
                    .ok_or("Canonical accepted service sale is unavailable.")?;
                sale.validate()?;
                let facts = service::verify_sources(
                    &sale.admission,
                    &sale.pipeline_lead,
                    &sale.account,
                    sale.pipeline_revision_at_admission,
                    sale.admitted_at,
                    |r| reader.read(r),
                )?;
                if facts != sale.facts {
                    return Err("Canonical service source acceptance changed.".into());
                }
                let mut admitted = sale
                    .admission
                    .fulfillment
                    .clone()
                    .ok_or("Canonical fulfillment obligation is unavailable.")?;
                let accepted_at = prior
                    .events
                    .iter()
                    .find(|event| event.outcome == "accepted")
                    .ok_or("Partner acceptance is unavailable.")?
                    .at;
                admitted.bill = None;
                admitted.payment = None;
                let mut expected_checks = scope.protected_checks.clone();
                expected_checks.sort_by(|a, b| a.path.cmp(&b.path).then(a.sha256.cmp(&b.sha256)));
                let mut checks = sale.facts.frozen_checks.clone();
                checks.sort_by(|a, b| a.path.cmp(&b.path).then(a.sha256.cmp(&b.sha256)));
                if &admitted != obligation
                    || sale.account != prior.account
                    || sale.admission.offer_version != *offer_version
                    || sale.admission.invoice.id != *invoice_id
                    || sale.facts.support_human != scope.support_human
                    || sale.facts.accepted_at < accepted_at
                    || !sale.facts.deliverables.contains(&scope.deliverable)
                    || checks != expected_checks
                    || sale.retain_until <= now
                    || !sale.admitted_recipients.contains(&format!("human:{actor}"))
                {
                    return Err("Delivery must retain the same canonical service terms, protected checks, and support owner.".into());
                }
                let doc = document(
                    &mut reader,
                    evidence,
                    "openagents.sales.partner-delivery.v1",
                )?;
                if doc["assignment"] != prior.proposal.id
                    || doc["proposal_sha256"] != prior.proposal_sha256
                    || doc["service_sale"] != *service_sale
                    || doc["candidate_sha256"] != sale.facts.candidate_sha256
                    || !doc["revision_count"]
                        .as_u64()
                        .is_some_and(|n| n <= scope.revision_limit as u64)
                    || !doc["rework_count"]
                        .as_u64()
                        .is_some_and(|n| n <= scope.rework_limit as u64)
                {
                    return Err(
                        "Partner delivery differs from its exact scope or rework bound.".into(),
                    );
                }
                for r in sale
                    .facts
                    .deliverables
                    .iter()
                    .chain(&sale.facts.accepted_checks)
                {
                    reference(&mut reader, r)?;
                }
                next.delivery_sale = Some(service_sale.clone());
                next.status = Status::Delivered;
                outcome = "delivered";
            }
            Action::ProposeHandoff {
                target, expires_at, ..
            } => {
                if !recipient
                    || !matches!(prior.status, Status::Accepted | Status::Delivered)
                    || prior.handoff.is_some()
                    || *expires_at <= now
                    || *expires_at > prior.proposal.expires_at
                {
                    return Err("Partner handoff needs accepted responsibility and a bounded pending target.".into());
                }
                let expected = match &prior.proposal.terms {
                    Terms::Discovery { .. } => prior.owner_human.clone(),
                    Terms::Fulfillment { scope, .. } => {
                        if prior.status != Status::Delivered {
                            return Err(
                                "Fulfillment support handoff requires accepted canonical delivery."
                                    .into(),
                            );
                        }
                        self.partner_scope(&mut reader, scope, lead)?.support_human
                    }
                };
                if target != &expected
                    || !Self::recipient(&lead.details, target)
                    || !prior.data.recipients.contains(&format!("human:{target}"))
                    || !self
                        .state
                        .principals
                        .get(target)
                        .is_some_and(|human| human.active && human.role != Role::Reader)
                {
                    return Err("Partner handoff target differs from its admitted owner or support contract.".into());
                }
                next.handoff = Some(Handoff {
                    target: target.clone(),
                    proposed_at: now,
                    expires_at: *expires_at,
                    evidence: action.evidence().clone(),
                });
                outcome = "handoff_proposed";
            }
            Action::AcceptHandoff { .. } | Action::RejectHandoff { .. } => {
                if !prior
                    .handoff
                    .as_ref()
                    .is_some_and(|h| h.target == actor && now < h.expires_at)
                {
                    return Err("Only the current handoff target may accept or refuse it.".into());
                }
                next.handoff = None;
                if matches!(action, Action::AcceptHandoff { .. }) {
                    next.status = Status::Completed;
                    next.next = None;
                    outcome = "handoff_accepted";
                } else {
                    outcome = "handoff_refused";
                }
            }
        }
        let handoff = if matches!(action, Action::ProposeHandoff { .. }) {
            next.handoff.clone()
        } else if matches!(
            action,
            Action::AcceptHandoff { .. } | Action::RejectHandoff { .. }
        ) {
            prior.handoff.clone()
        } else {
            None
        };
        next.events.push(Event {
            actor: actor.into(),
            at: now,
            outcome: outcome.into(),
            evidence: action.evidence().clone(),
            handoff,
        });
        next.validate()?;
        Ok(next)
    }
    pub fn partner_show(&mut self, access: &Access, lead: &str, assignment: &str) -> Result<Value> {
        self.refresh()?;
        self.check(access)?;
        let lead = self.state.leads.get(lead).ok_or("Lead is unavailable.")?;
        let assignment = lead
            .partner_assignments
            .get(assignment)
            .ok_or("Partner assignment is unavailable.")?;
        let actor = access.principal();
        if !Self::recipient(&lead.details, actor)
            || !assignment
                .data
                .recipients
                .contains(&format!("human:{actor}"))
            || (self.clock)() >= assignment.data.retain_until
        {
            return Err("Partner read exceeds admitted recipients or retention.".into());
        }
        let accepted = assignment
            .events
            .iter()
            .any(|e| e.actor == actor && e.outcome == "accepted");
        let owns = actor == assignment.owner_human;
        let recipient = actor == assignment.proposal.recipient_human;
        let support = assignment
            .handoff
            .as_ref()
            .is_some_and(|h| h.target == actor);
        let accepted_support = assignment
            .events
            .iter()
            .any(|e| e.actor == actor && e.outcome == "handoff_accepted");
        if owns || recipient && accepted || accepted_support {
            let canonical_fulfillment = assignment
                .delivery_sale
                .as_ref()
                .and_then(|id| lead.service_sales.get(id))
                .filter(|sale| {
                    sale.retain_until > (self.clock)()
                        && sale.admitted_recipients.contains(&format!("human:{actor}"))
                })
                .map(|sale| sale.effective_fulfillment())
                .transpose()?
                .flatten();
            Ok(
                json!({"assignment":assignment,"canonical_fulfillment":canonical_fulfillment,
                    "authority_granted":false,"commission_eligibility_verified":false,"live_payment_qualified":false}),
            )
        } else if recipient || support {
            Ok(
                json!({"invitation":{"id":assignment.proposal.id,"kind":assignment.proposal.terms.kind(),
                "recipient_human":actor,"status":assignment.status,"proposal_sha256":assignment.proposal_sha256,
                "expires_at":assignment.proposal.expires_at,"pipeline_revision":lead.revision},"authority_granted":false}),
            )
        } else {
            Err("Partner record access refused.".into())
        }
    }
    pub(super) fn partner_visible(&self, access: &Access, assignment: &Assignment) -> bool {
        assignment.owner_human == access.principal()
            || assignment.proposal.recipient_human == access.principal()
                && assignment
                    .events
                    .iter()
                    .any(|e| e.actor == access.principal() && e.outcome == "accepted")
            || assignment
                .events
                .iter()
                .any(|e| e.actor == access.principal() && e.outcome == "handoff_accepted")
    }
    pub fn partner_export(
        &mut self,
        access: &Access,
        lead: &str,
        assignment: &str,
        path: &Path,
    ) -> Result<String> {
        let value = self.partner_show(access, lead, assignment)?;
        self.external_file(path)?;
        let bytes = serde_json::to_vec_pretty(&value)
            .map_err(|_| "Partner export serialization failed.")?;
        let mut file =
            super::super::private_open(path, true, true).map_err(|_| "Partner export refused.")?;
        use std::io::Write;
        file.write_all(&bytes)
            .and_then(|()| file.sync_all())
            .map_err(|_| "Partner export write failed.")?;
        super::super::sync_directory(
            path.parent()
                .filter(|p| !p.as_os_str().is_empty())
                .unwrap_or(Path::new(".")),
        )
        .map_err(|_| "Partner export sync failed.")?;
        Ok(super::digest(&bytes))
    }
}

#[cfg(test)]
mod tests;
