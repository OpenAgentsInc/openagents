//! Closed LAB transitions over authenticated, retained declarations.
use crate::*;
use nostr::market_contracts::labor::{AcceptancePolicy, LaborTerms};
use nostr::market_contracts::{OrderRef, Terms, parse_order_ref};
use nostr::private_artifact::OpenEnvelope;
use std::collections::BTreeSet;

pub const LINK: &str = "openagents.labor-execution.v1";
pub const SUBMISSION: &str = "openagents.labor-submission.v1";
pub const DELIVERY: &str = "openagents.labor-delivery.v1";
pub const VERIFICATION: &str = "openagents.labor-verification.v1";
pub const REVIEW: &str = "openagents.labor-review.v1";
pub const DISPUTE: &str = "openagents.labor-dispute.v1";
pub const ACCEPTANCE: &str = "openagents.labor-acceptance.v1";

#[derive(Clone, Debug, Default)]
pub struct Records {
    pub values: BTreeMap<String, Value>,
    pub link: Option<Value>,
    pub submission: Option<Value>,
    pub delivery: Option<Value>,
    pub verification: Option<Value>,
    pub review: Option<Value>,
    pub acceptance: Option<Value>,
    pub disputes: Vec<Value>,
    pub conflict: bool,
}

pub struct Contracts<'a> {
    pub order: &'a OrderRef,
    pub market: &'a Terms,
    pub labor: &'a LaborTerms,
    pub policy: &'a AcceptancePolicy,
    pub blobs: &'a Blobs,
}

impl Records {
    pub fn ingest(
        &mut self,
        opened: &OpenEnvelope,
        contracts: &Contracts<'_>,
        now: u64,
    ) -> Result<&'static str> {
        let bytes = opened
            .inline_bytes()
            .ok_or("labor record bytes unavailable")?;
        let body = nostr::contracts::parse_strict(bytes).map_err(|e| e.to_string())?;
        let digest = opened.artifact().digest.clone();
        if self.values.contains_key(&digest) {
            return Ok("duplicate");
        }
        if self.conflict {
            return Err("labor evidence conflicts; automatic transitions stopped".into());
        }
        if parse_order_ref(&body["order"]).map_err(|e| e.to_string())? != *contracts.order
            || body["issuer"] != opened.signer()
        {
            return Err("labor record has a different order or authenticated issuer".into());
        }
        let version = body["v"].as_str().ok_or("labor record schema")?;
        if opened.artifact().schema.as_deref() != Some(version) {
            return Err("labor envelope schema differs from body".into());
        }
        let r = artifact_value(opened.artifact());
        match version {
            LINK => {
                exact(
                    &body,
                    LINK,
                    &[
                        "issuer",
                        "order",
                        "request",
                        "attempt",
                        "run",
                        "execute",
                        "execute_body",
                        "rework",
                        "context",
                        "previous",
                    ],
                )?;
                if opened.signer() != contracts.market.buyer
                    || body["attempt"] != 1
                    || !body["previous"].is_null()
                    || !body["rework"].is_null()
                    || body["context"] != artifact_value(&contracts.labor.execution.context)
                {
                    return Err("labor linkage is not the admitted initial buyer attempt".into());
                }
                if self.link.is_some() {
                    self.conflict = true;
                    return Err("competing labor execution linkage".into());
                }
                contracts.blobs.get(&body["execute_body"])?;
                self.link = Some(r);
            }
            SUBMISSION => {
                exact(
                    &body,
                    SUBMISSION,
                    &[
                        "issuer",
                        "order",
                        "number",
                        "previous",
                        "rework",
                        "executions",
                        "deliverables",
                        "run_evidence",
                        "limitations",
                    ],
                )?;
                if opened.signer() != contracts.market.worker
                    || body["number"] != 0
                    || !body["previous"].is_null()
                    || !body["rework"].is_null()
                    || body["executions"]
                        != json!([self.link.as_ref().ok_or("delivery precedes linkage")?])
                {
                    return Err("submission does not bind the admitted worker execution".into());
                }
                if self.submission.is_some() {
                    self.conflict = true;
                    return Err("competing labor submission".into());
                }
                let items = body["deliverables"].as_array().ok_or("deliverables")?;
                if items.len() != contracts.labor.deliverables.len() {
                    return Err("incomplete deliverable set".into());
                }
                let mut seen = BTreeSet::new();
                for item in items {
                    let m = item.as_object().ok_or("deliverable shape")?;
                    if m.len() != 2 || !m.contains_key("id") || !m.contains_key("content") {
                        return Err("deliverable fields".into());
                    }
                    let id = item["id"].as_str().ok_or("deliverable ID")?;
                    if !seen.insert(id) {
                        return Err("duplicate deliverable".into());
                    }
                    let spec = contracts
                        .labor
                        .deliverables
                        .iter()
                        .find(|d| d.id == id)
                        .ok_or("unknown deliverable")?;
                    let reference = parse_artifact(&item["content"]).map_err(|e| e.to_string())?;
                    if reference.size > spec.max_bytes {
                        return Err("deliverable byte limit exceeded".into());
                    }
                    let artifact = contracts.blobs.resolve(&reference)?;
                    let schema = contracts.blobs.resolve(&spec.schema.0)?;
                    let schemas = BTreeMap::from([(
                        spec.schema.0.digest.clone(),
                        jcs(schema).map_err(|e| e.to_string())?,
                    )]);
                    let closure =
                        nostr::contracts::prepare_closure(&schemas).map_err(|e| e.to_string())?;
                    nostr::contracts::validate_instance(&closure, &spec.schema.0.digest, artifact)
                        .map_err(|e| e.to_string())?;
                }
                references(&body["run_evidence"], contracts.blobs, true)?;
                self.check_run_evidence(&body["run_evidence"], contracts)?;
                contracts.blobs.get(&body["limitations"])?;
                self.submission = Some(r);
            }
            DELIVERY => {
                exact(
                    &body,
                    DELIVERY,
                    &["issuer", "order", "submission", "received_at", "available"],
                )?;
                if ![
                    contracts.market.buyer.as_str(),
                    contracts.labor.resolver.as_str(),
                ]
                .contains(&opened.signer())
                    || Some(&body["submission"]) != self.submission.as_ref()
                    || body["received_at"].as_u64().is_none_or(|t| t > now)
                    || !body["available"].is_boolean()
                {
                    return Err(
                        "delivery receipt is not attributable to the admitted recipient".into(),
                    );
                }
                if self.delivery.is_some() {
                    return Err("delivery already recorded".into());
                }
                self.delivery = Some(r);
            }
            VERIFICATION => {
                exact(
                    &body,
                    VERIFICATION,
                    &[
                        "issuer",
                        "order",
                        "submission",
                        "policy",
                        "checker_receipts",
                        "criteria",
                        "verdict",
                        "limitations",
                    ],
                )?;
                if opened.signer() != contracts.labor.reviewer
                    || Some(&body["submission"]) != self.submission.as_ref()
                    || body["policy"] != artifact_value(&contracts.labor.acceptance_policy)
                {
                    return Err(
                        "verification has a different reviewer, submission, or frozen policy"
                            .into(),
                    );
                }
                references(&body["checker_receipts"], contracts.blobs, true)?;
                let receipt_refs = body["checker_receipts"]
                    .as_array()
                    .ok_or("checker receipts")?;
                if receipt_refs.len() != 1 {
                    return Err("this host requires one frozen checker receipt".into());
                }
                let receipt = contracts.blobs.get(&receipt_refs[0])?;
                exact(
                    receipt,
                    "openagents.free-labor.checker.v1",
                    &[
                        "submission",
                        "checker",
                        "lock",
                        "input",
                        "criteria",
                        "verdict",
                        "elapsed_ms",
                        "cost_usd",
                        "evidence",
                        "limitations",
                    ],
                )?;
                if receipt["submission"] != body["submission"]
                    || nostr::contracts::parse_definition(&receipt["checker"])
                        .map_err(|e| e.to_string())?
                        != contracts.policy.checker
                    || parse_artifact(&receipt["lock"]).map_err(|e| e.to_string())?
                        != contracts.policy.lock
                    || receipt["input"] != artifact_value(&contracts.labor.execution.input)
                    || receipt["criteria"] != body["criteria"]
                    || receipt["verdict"] != body["verdict"]
                    || receipt["elapsed_ms"].as_u64().is_none()
                    || !(receipt["cost_usd"].is_null()
                        || receipt["cost_usd"]
                            .as_f64()
                            .is_some_and(|cost| cost >= 0.0 && cost.is_finite()))
                {
                    return Err("checker receipt does not bind the frozen policy, input, and exact submission".into());
                }
                references(&receipt["evidence"], contracts.blobs, true)?;
                contracts.blobs.get(&receipt["limitations"])?;
                contracts.blobs.get(&body["limitations"])?;
                let criteria = body["criteria"].as_array().ok_or("verification criteria")?;
                let mut ids = BTreeSet::new();
                let mut worst = 0;
                for criterion in criteria {
                    if criterion.as_object().is_none_or(|m| m.len() != 3) {
                        return Err("criterion shape".into());
                    }
                    let id = criterion["id"].as_str().ok_or("criterion ID")?;
                    if !contracts.policy.criteria.iter().any(|s| s == id) || !ids.insert(id) {
                        return Err("criterion set differs from frozen policy".into());
                    }
                    worst = worst.max(verdict(&criterion["verdict"])?);
                    references(&criterion["evidence"], contracts.blobs, false)?;
                }
                if ids.len() != contracts.policy.criteria.len()
                    || verdict(&body["verdict"])? != worst
                {
                    return Err(
                        "verification omits criteria or changes all-pass aggregation".into(),
                    );
                }
                if self.verification.is_some() {
                    self.conflict = true;
                    return Err("competing labor verification".into());
                }
                self.verification = Some(r);
            }
            REVIEW => {
                exact(
                    &body,
                    REVIEW,
                    &[
                        "issuer",
                        "order",
                        "submission",
                        "verification",
                        "decision",
                        "criteria",
                        "reason",
                    ],
                )?;
                if opened.signer() != contracts.market.buyer
                    || Some(&body["submission"]) != self.submission.as_ref()
                    || Some(&body["verification"]) != self.verification.as_ref()
                    || now > contracts.market.review_due_at
                {
                    return Err("review is unbound or late".into());
                }
                contracts.blobs.get(&body["reason"])?;
                let criteria = body["criteria"].as_array().ok_or("review criteria")?;
                let decision = body["decision"].as_str().ok_or("review decision")?;
                if decision == "accept" {
                    if !criteria.is_empty() || !self.acceptable(contracts)? {
                        return Err(
                            "buyer acceptance requires timely complete delivery and passed checks"
                                .into(),
                        );
                    }
                } else if decision == "reject" {
                    if criteria.is_empty()
                        || criteria.iter().any(|id| {
                            id.as_str()
                                .is_none_or(|id| !contracts.policy.criteria.iter().any(|c| c == id))
                        })
                    {
                        return Err("rejection must name frozen criteria".into());
                    }
                } else if decision == "request_rework" {
                    return Err("rework is outside this order's zero-rework bound".into());
                } else {
                    return Err("unknown buyer decision".into());
                }
                if self.review.is_some() {
                    self.conflict = true;
                    return Err("competing buyer reviews".into());
                }
                self.review = Some(r);
            }
            DISPUTE => {
                exact(
                    &body,
                    DISPUTE,
                    &["issuer", "order", "subject", "cause", "evidence"],
                )?;
                if ![
                    contracts.market.buyer.as_str(),
                    contracts.market.provider.as_str(),
                ]
                .contains(&opened.signer())
                    || now > contracts.labor.dispute_due_at
                {
                    return Err("dispute issuer or deadline".into());
                }
                self.resolve(&body["subject"])?;
                if ![
                    "non_delivery",
                    "verification",
                    "review",
                    "cancellation",
                    "buyer_unavailable",
                    "conflict",
                ]
                .contains(&body["cause"].as_str().unwrap_or(""))
                {
                    return Err("unsupported dispute cause".into());
                }
                references(&body["evidence"], contracts.blobs, true)?;
                self.disputes.push(r);
            }
            ACCEPTANCE => {
                exact(
                    &body,
                    ACCEPTANCE,
                    &[
                        "issuer",
                        "order",
                        "submission",
                        "verification",
                        "outcome",
                        "basis",
                        "review",
                        "resolution",
                        "amount_due_msat",
                        "supersedes",
                        "evidence",
                    ],
                )?;
                if !self.disputes.is_empty() {
                    return Err(
                        "disputed order requires separately supported resolver reconciliation"
                            .into(),
                    );
                }
                if opened.signer() != contracts.market.buyer
                    || body["basis"] != "buyer_acceptance"
                    || body["outcome"] != "accepted"
                    || body["amount_due_msat"] != 0
                    || !body["resolution"].is_null()
                    || body["supersedes"] != json!([])
                    || Some(&body["submission"]) != self.submission.as_ref()
                    || Some(&body["verification"]) != self.verification.as_ref()
                    || Some(&body["review"]) != self.review.as_ref()
                    || self.resolve(&body["review"])?["decision"] != "accept"
                    || !self.acceptable(contracts)?
                {
                    return Err("invalid free buyer acceptance".into());
                }
                references(&body["evidence"], contracts.blobs, true)?;
                if !body["evidence"]
                    .as_array()
                    .ok_or("acceptance evidence")?
                    .contains(
                        self.delivery
                            .as_ref()
                            .ok_or("delivery receipt unavailable")?,
                    )
                {
                    return Err("acceptance omits delivery evidence".into());
                }
                if self.acceptance.is_some() {
                    self.conflict = true;
                    return Err("competing final acceptance".into());
                }
                self.acceptance = Some(r);
            }
            _ => return Err("this free labor host does not support this LAB transition".into()),
        }
        self.values.insert(digest, body);
        Ok("applied")
    }
    fn check_run_evidence(&self, references: &Value, contracts: &Contracts<'_>) -> Result<()> {
        let link = self.resolve(self.link.as_ref().ok_or("execution linkage unavailable")?)?;
        let mut journal = nostr::run::Journal::default();
        for reference in references.as_array().ok_or("run evidence list")? {
            let record = nostr::run::parse_envelope(contracts.blobs.get(reference)?)
                .map_err(|e| e.to_string())?;
            if record.run != link["run"].as_str().ok_or("linkage run")?
                || record.controller != contracts.market.worker
            {
                return Err("RUN evidence names another execution or worker".into());
            }
            if nostr::run::ingest(&mut journal, record, &contracts.market.worker)
                .map_err(|e| e.to_string())?
                != nostr::run::Ingest::Applied
            {
                return Err("RUN evidence is incomplete, duplicated, or conflicted".into());
            }
        }
        let root = journal.applied.first().ok_or("RUN root unavailable")?;
        let result = journal.applied.last().ok_or("RUN result unavailable")?;
        if root.data.get("request") != Some(&link["request"])
            || result.record_type != "resolved"
            || result.data.get("outcome") != Some(&json!("completed"))
            || result.data.get("dispatched") != Some(&json!(true))
            || !journal.applied.iter().any(|r| r.record_type == "admitted")
            || !journal
                .applied
                .iter()
                .any(|r| r.record_type == "dispatched")
        {
            return Err("RUN evidence does not establish the admitted completed attempt".into());
        }
        Ok(())
    }
    pub fn resolve(&self, reference: &Value) -> Result<&Value> {
        let r = parse_artifact(reference).map_err(|e| e.to_string())?;
        let v = self
            .values
            .get(&r.digest)
            .ok_or("required authenticated labor predecessor unavailable")?;
        check_artifact_bytes(&r, &jcs(v).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        Ok(v)
    }
    fn acceptable(&self, contracts: &Contracts<'_>) -> Result<bool> {
        let delivery = self.resolve(
            self.delivery
                .as_ref()
                .ok_or("delivery receipt unavailable")?,
        )?;
        let verification = self.resolve(
            self.verification
                .as_ref()
                .ok_or("verification unavailable")?,
        )?;
        Ok(delivery["available"] == true
            && delivery["received_at"]
                .as_u64()
                .is_some_and(|t| t <= contracts.market.delivery_due_at)
            && verification["verdict"] == "passed")
    }
}
fn references(value: &Value, blobs: &Blobs, nonempty: bool) -> Result<()> {
    let refs = value.as_array().ok_or("artifact reference list")?;
    if refs.len() > 64 || nonempty && refs.is_empty() {
        return Err("evidence reference list is empty or too large".into());
    }
    for r in refs {
        blobs.get(r)?;
    }
    Ok(())
}
fn verdict(value: &Value) -> Result<u8> {
    match value.as_str() {
        Some("passed") => Ok(0),
        Some("not_run") => Ok(1),
        Some("unverifiable") => Ok(2),
        Some("failed") => Ok(3),
        _ => Err("unsupported verification verdict".into()),
    }
}
