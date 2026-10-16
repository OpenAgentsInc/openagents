//! One participant's view of an order after execution: every record it has
//! verified, checked against the accepted terms and the records before it,
//! and the final outcome and obligation they establish.
use crate::records::*;
use crate::terms::Terms;
use crate::{Result, digest_of};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub digest: String,
    pub record: Record,
    /// When this participant received it; signed times are only claims.
    pub observed_at: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Applied {
    New(String),
    Duplicate(String),
}

/// What the final records establish.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Final {
    /// No final acceptance record yet.
    Open,
    Accepted {
        acceptance: String,
        amount_msat: u64,
    },
    Rejected {
        acceptance: String,
    },
    /// Unresolved never proves zero debt.
    Unresolved {
        acceptance: String,
    },
    /// Competing final records: automatic settlement stops.
    Conflict(Vec<String>),
}

/// The obligation's collection state. Nothing here moves money.
#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Collection {
    /// No accepted obligation.
    None,
    /// An accepted obligation whose collection is not yet known.
    Unknown {
        acceptance: String,
        amount_msat: u64,
    },
    /// The central receive adapter reported funding, recorded in the ledger.
    Funded {
        acceptance: String,
        payment_hash: String,
    },
    /// Funded, but a later resolver decision superseded the acceptance; a
    /// refund needs its own agreement and transfer.
    RefundOwed { payment_hash: String },
}

#[derive(Clone, Debug)]
pub struct Book {
    pub terms: Terms,
    entries: Vec<Entry>,
    index: BTreeMap<String, usize>,
    funded: Option<(String, String)>,
}

fn fail<T>(why: &str) -> Result<T> {
    Err(why.to_string())
}

impl Book {
    /// # Errors
    ///
    /// When the terms are not admitted at `now`.
    pub fn new(terms: Terms, now: u64) -> Result<Self> {
        terms.admit(now)?;
        Ok(Self {
            terms,
            entries: Vec::new(),
            index: BTreeMap::new(),
            funded: None,
        })
    }

    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    fn get(&self, digest: &str) -> Option<&Record> {
        self.index.get(digest).map(|i| &self.entries[*i].record)
    }

    fn submissions(&self) -> impl Iterator<Item = (&String, &Submission)> {
        self.entries.iter().filter_map(|e| match &e.record {
            Record::Submission(s) => Some((&e.digest, s)),
            _ => None,
        })
    }

    fn submission(&self, digest: &str) -> Result<&Submission> {
        match self.get(digest) {
            Some(Record::Submission(s)) => Ok(s),
            _ => fail("the record names a submission this participant does not hold"),
        }
    }

    fn verification(&self, digest: &str) -> Result<&Verification> {
        match self.get(digest) {
            Some(Record::Verification(v)) => Ok(v),
            _ => fail("the record names a verification this participant does not hold"),
        }
    }

    /// A timely, available delivery receipt for `submission` from the buyer
    /// or the resolver.
    fn timely_delivery(&self, submission: &str) -> Option<(&String, &Delivery)> {
        let number = self.submission(submission).ok()?.number;
        self.entries.iter().find_map(|e| match &e.record {
            Record::Delivery(d)
                if d.submission == submission
                    && d.available
                    && d.received_at <= self.terms.due(number) =>
            {
                Some((&e.digest, d))
            }
            _ => None,
        })
    }

    fn acceptances(&self) -> impl Iterator<Item = (&String, &Acceptance)> {
        self.entries.iter().filter_map(|e| match &e.record {
            Record::Acceptance(a) => Some((&e.digest, a)),
            _ => None,
        })
    }

    /// Verify and retain one record signed by `signer`.
    ///
    /// # Errors
    ///
    /// When the signer is not the role the record needs, or the record
    /// breaks the terms or the records before it. Nothing is retained then.
    pub fn apply(&mut self, record: Record, signer: &str, observed_at: u64) -> Result<Applied> {
        let digest = digest_of(&record)?;
        if self.index.contains_key(&digest) {
            return Ok(Applied::Duplicate(digest));
        }
        if record.issuer() != signer {
            return fail("the record's issuer is not its signer");
        }
        if record.order() != self.terms.order {
            return fail("the record is for another order");
        }
        if observed_at > self.terms.retain_until {
            return fail("the order is past its retention horizon");
        }
        match &record {
            Record::Submission(s) => self.check_submission(s)?,
            Record::Delivery(d) => self.check_delivery(d, observed_at)?,
            Record::Verification(v) => self.check_verification(v)?,
            Record::Review(r) => self.check_review(r, observed_at)?,
            Record::Dispute(d) => self.check_dispute(d)?,
            Record::Cancellation(c) => self.check_cancellation(c)?,
            Record::Resolution(r) => self.check_resolution(r, observed_at)?,
            Record::Acceptance(a) => self.check_acceptance(a)?,
        }
        self.index.insert(digest.clone(), self.entries.len());
        self.entries.push(Entry {
            digest: digest.clone(),
            record,
            observed_at,
        });
        Ok(Applied::New(digest))
    }

    fn check_submission(&self, s: &Submission) -> Result<()> {
        let t = &self.terms;
        if s.issuer != t.worker {
            return fail("only the worker submits");
        }
        if matches!(self.final_state(), Final::Accepted { .. }) {
            return fail("the order is already accepted; no submission can change it");
        }
        if let Some((_, same)) = self.submissions().find(|(_, x)| x.number == s.number) {
            if same != s {
                return fail(
                    "another submission already has this number: substitution is a conflict",
                );
            }
        }
        let count = u32::try_from(self.submissions().count()).unwrap_or(u32::MAX);
        if s.number != count {
            return fail("submission numbers must be contiguous");
        }
        if s.number > u32::from(t.max_reworks) {
            return fail("more submissions than the terms allow");
        }
        if s.executions.is_empty() || s.run_evidence.is_empty() {
            return fail("a submission names its executions and run evidence");
        }
        let ids: BTreeSet<&String> = s.deliverables.iter().map(|d| &d.id).collect();
        if ids.len() != s.deliverables.len() || ids.len() != t.deliverables.len() {
            return fail("every deliverable appears exactly once");
        }
        for want in &t.deliverables {
            let got = s
                .deliverables
                .iter()
                .find(|d| d.id == want.id)
                .ok_or("a required deliverable is missing")?;
            if got.size == 0 || got.size > want.max_bytes {
                return fail("a deliverable is empty or over its byte ceiling");
            }
        }
        if s.number == 0 {
            if s.previous.is_some() || s.rework.is_some() {
                return fail("the first submission has no previous or rework");
            }
            return Ok(());
        }
        let previous = s
            .previous
            .as_deref()
            .ok_or("a replacement names the previous submission")?;
        let prior = self.submission(previous)?;
        if prior.number + 1 != s.number {
            return fail("a replacement must follow the submission before it");
        }
        match s.rework.as_deref().and_then(|r| self.get(r)) {
            Some(Record::Review(r))
                if r.decision == ReviewDecision::RequestRework && r.submission == previous =>
            {
                Ok(())
            }
            _ => fail("a replacement needs the buyer's rework request for the previous submission"),
        }
    }

    fn check_delivery(&self, d: &Delivery, observed_at: u64) -> Result<()> {
        if d.issuer != self.terms.buyer && d.issuer != self.terms.resolver {
            return fail("only the buyer or the resolver records receipt of a delivery");
        }
        self.submission(&d.submission)?;
        if d.received_at > observed_at {
            return fail("a delivery receipt cannot claim a time after it was observed");
        }
        // Conflicting availability claims for the same submission are kept
        // apart for the resolver, not merged.
        if self.entries.iter().any(|e| matches!(&e.record, Record::Delivery(x) if x.submission == d.submission && x.issuer == d.issuer && x.available != d.available)) {
            return fail("a conflicting delivery receipt from the same issuer");
        }
        Ok(())
    }

    fn check_verification(&self, v: &Verification) -> Result<()> {
        if v.issuer != self.terms.reviewer {
            return fail("only the pinned reviewer verifies");
        }
        self.submission(&v.submission)?;
        if v.checker_receipts.is_empty() {
            return fail("a verification names its checker receipts");
        }
        let ids: Vec<&String> = v.criteria.iter().map(|c| &c.id).collect();
        let unique: BTreeSet<&String> = ids.iter().copied().collect();
        if unique.len() != ids.len() || unique != self.terms.criteria.iter().collect() {
            return fail("a verification returns every policy criterion exactly once");
        }
        let has = |x: Verdict| v.criteria.iter().any(|c| c.verdict == x);
        let overall = if has(Verdict::Failed) {
            Verdict::Failed
        } else if has(Verdict::Unverifiable) {
            Verdict::Unverifiable
        } else if has(Verdict::NotRun) {
            Verdict::NotRun
        } else {
            Verdict::Passed
        };
        if overall != v.verdict {
            return fail("the overall verdict does not follow from the criteria");
        }
        if v.verdict == Verdict::Passed && v.criteria.iter().any(|c| c.evidence.is_empty()) {
            return fail("a passed criterion needs evidence");
        }
        Ok(())
    }

    fn contested(&self, criteria: &[String]) -> Result<()> {
        let unique: BTreeSet<&String> = criteria.iter().collect();
        if criteria.is_empty()
            || unique.len() != criteria.len()
            || criteria.iter().any(|c| !self.terms.criteria.contains(c))
        {
            return fail("a rework or rejection contests existing policy criteria only");
        }
        Ok(())
    }

    fn check_review(&self, r: &Review, observed_at: u64) -> Result<()> {
        let t = &self.terms;
        if r.issuer != t.buyer {
            return fail("only the buyer reviews");
        }
        let submission = self.submission(&r.submission)?;
        let verification = self.verification(&r.verification)?;
        if verification.submission != r.submission {
            return fail("the review pairs a verification of another submission");
        }
        if observed_at > t.review_due_at {
            return fail("the review is after the review deadline");
        }
        match r.decision {
            ReviewDecision::Accept => {
                if verification.verdict != Verdict::Passed || !r.criteria.is_empty() {
                    return fail("an accept needs passed verification and contests nothing");
                }
                if self.timely_delivery(&r.submission).is_none() {
                    return fail(
                        "an accept needs an available delivery receipt within the deadline",
                    );
                }
                Ok(())
            }
            ReviewDecision::RequestRework => {
                self.contested(&r.criteria)?;
                if submission.number >= u32::from(t.max_reworks) {
                    return fail("no rework remains under the terms");
                }
                if t.rework_due_at.is_none_or(|due| observed_at >= due) {
                    return fail("too late to request rework");
                }
                Ok(())
            }
            ReviewDecision::Reject => self.contested(&r.criteria),
        }
    }

    fn check_dispute(&self, d: &Dispute) -> Result<()> {
        if d.issuer != self.terms.buyer && d.issuer != self.terms.provider {
            return fail("only the buyer or the provider disputes");
        }
        if d.evidence.is_empty() {
            return fail("a dispute names its evidence");
        }
        // Late disputes stay visible; the resolver weighs their timing.
        if d.cause != Cause::NonDelivery
            && d.cause != Cause::BuyerUnavailable
            && self.get(&d.subject).is_none()
        {
            return fail("the dispute's subject is not held");
        }
        Ok(())
    }

    fn check_cancellation(&self, c: &Cancellation) -> Result<()> {
        if c.issuer != self.terms.buyer && c.issuer != self.terms.provider {
            return fail("only the buyer or the provider requests cancellation");
        }
        Ok(())
    }

    /// The latest submission whose frozen criteria passed and whose
    /// delivery was timely: a qualifying delivered submission.
    fn qualifying(&self) -> Option<(&String, &String)> {
        self.submissions().find_map(|(d, _)| {
            self.timely_delivery(d)?;
            let verification = self.entries.iter().find_map(|e| match &e.record {
                Record::Verification(v) if &v.submission == d && v.verdict == Verdict::Passed => {
                    Some(&e.digest)
                }
                _ => None,
            })?;
            Some((d, verification))
        })
    }

    fn check_resolution(&self, r: &Resolution, observed_at: u64) -> Result<()> {
        let t = &self.terms;
        if r.issuer != t.resolver {
            return fail("only the pinned resolver resolves");
        }
        if observed_at > t.resolution_due_at && r.decision != Decision::Unresolved {
            return fail("a decision after the resolution deadline cannot settle the order");
        }
        if r.evidence.is_empty() {
            return fail("a resolution names its evidence");
        }
        for d in &r.disputes {
            if !matches!(self.get(d), Some(Record::Dispute(_))) {
                return fail("the resolution names a dispute that is not held");
            }
        }
        // Every known dispute is assessed.
        let known: BTreeSet<&String> = self
            .entries
            .iter()
            .filter(|e| matches!(e.record, Record::Dispute(_)))
            .map(|e| &e.digest)
            .collect();
        if known.iter().any(|d| !r.disputes.contains(d)) {
            return fail("the resolution leaves a known dispute unassessed");
        }
        match r.decision {
            Decision::Unresolved => Ok(()),
            Decision::Accept => {
                let (Some(s), Some(v)) = (&r.submission, &r.verification) else {
                    return fail("an accept names the submission and its verification");
                };
                let verification = self.verification(v)?;
                if &verification.submission != s || verification.verdict != Verdict::Passed {
                    return fail(
                        "the resolver can accept only a passed verification of that submission",
                    );
                }
                if self.timely_delivery(s).is_none() {
                    return fail("the resolver can accept only a timely, available delivery");
                }
                if !matches!(r.basis, Basis::Delivered | Basis::Criteria) {
                    return fail("an accept rests on the delivered work");
                }
                Ok(())
            }
            Decision::Reject => match r.basis {
                Basis::Criteria => {
                    let v = self.verification(
                        r.verification
                            .as_deref()
                            .ok_or("a criteria rejection names a verification")?,
                    )?;
                    if v.verdict != Verdict::Failed {
                        return fail(
                            "a criteria rejection needs a failed verification, not an unknown one",
                        );
                    }
                    Ok(())
                }
                Basis::NonDelivery => {
                    if observed_at <= t.delivery_due_at
                        || self
                            .submissions()
                            .any(|(d, _)| self.timely_delivery(d).is_some())
                    {
                        return fail(
                            "non-delivery needs the deadline passed with no timely delivery",
                        );
                    }
                    Ok(())
                }
                Basis::Cancellation => {
                    if !self
                        .entries
                        .iter()
                        .any(|e| matches!(e.record, Record::Cancellation(_)))
                        || self.qualifying().is_some()
                    {
                        return fail(
                            "a cancellation rejection needs a cancellation and no qualifying delivery before it",
                        );
                    }
                    Ok(())
                }
                Basis::Delivered | Basis::InsufficientEvidence => {
                    fail("a rejection needs positive supporting evidence")
                }
            },
        }
    }

    fn check_acceptance(&self, a: &Acceptance) -> Result<()> {
        let t = &self.terms;
        if a.evidence.is_empty() {
            return fail("an acceptance names its evidence");
        }
        match a.basis {
            AcceptanceBasis::BuyerAcceptance => {
                if a.issuer != t.buyer
                    || a.resolution.is_some()
                    || !a.supersedes.is_empty()
                    || a.outcome != Outcome::Accepted
                {
                    return fail(
                        "a buyer acceptance is the buyer's, accepted, with no resolution or supersession",
                    );
                }
                let review = match a.review.as_deref().and_then(|r| self.get(r)) {
                    Some(Record::Review(r)) if r.decision == ReviewDecision::Accept => r,
                    _ => return fail("a buyer acceptance needs the buyer's accept review"),
                };
                if a.submission.as_ref() != Some(&review.submission)
                    || a.verification.as_ref() != Some(&review.verification)
                {
                    return fail("the acceptance does not match its review");
                }
                if a.amount_due_msat != Some(t.price_msat) {
                    return fail("a buyer acceptance owes exactly the agreed price");
                }
                if self.acceptances().any(|_| true) {
                    return fail(
                        "a final acceptance already exists; only the resolver can supersede it",
                    );
                }
            }
            AcceptanceBasis::ResolverDecision => {
                if a.issuer != t.resolver {
                    return fail("only the resolver issues a resolver decision");
                }
                let resolution = match a.resolution.as_deref().and_then(|r| self.get(r)) {
                    Some(Record::Resolution(r)) => r,
                    _ => return fail("a resolver decision names its resolution"),
                };
                let (outcome, amount) = match resolution.decision {
                    Decision::Accept => (Outcome::Accepted, Some(t.price_msat)),
                    Decision::Reject => (Outcome::Rejected, Some(0)),
                    Decision::Unresolved => (Outcome::Unresolved, None),
                };
                if a.outcome != outcome || a.amount_due_msat != amount {
                    return fail("the outcome or amount does not follow from the resolution");
                }
                if outcome == Outcome::Accepted
                    && (a.submission != resolution.submission
                        || a.verification != resolution.verification)
                {
                    return fail("the acceptance does not match the resolution");
                }
                for prior in &a.supersedes {
                    if !matches!(self.get(prior), Some(Record::Acceptance(_)))
                        || !resolution.evidence.contains(prior)
                    {
                        return fail(
                            "a superseded acceptance must be held and assessed by the resolution",
                        );
                    }
                }
                // An uncontested accepted delivery cannot be superseded.
                for (digest, prior) in self.acceptances() {
                    if a.supersedes.contains(digest)
                        && prior.outcome == Outcome::Accepted
                        && resolution.disputes.is_empty()
                    {
                        return fail("an uncontested acceptance cannot be superseded");
                    }
                }
            }
            AcceptanceBasis::MutualCancellation => {
                if a.issuer != t.buyer && a.issuer != t.provider {
                    return fail("a mutual cancellation is issued by the buyer or the provider");
                }
                if a.outcome != Outcome::Rejected
                    || a.amount_due_msat != Some(0)
                    || a.review.is_some()
                    || a.resolution.is_some()
                    || !a.supersedes.is_empty()
                {
                    return fail(
                        "a mutual cancellation is rejected, owes zero, and names no review or resolution",
                    );
                }
                let by = |who: &str| {
                    self.entries.iter().any(|e| {
                        matches!(&e.record, Record::Cancellation(c) if c.issuer == who)
                            && a.evidence.contains(&e.digest)
                    })
                };
                if !by(&t.buyer) || !by(&t.provider) {
                    return fail(
                        "a mutual cancellation needs both parties' cancellations as evidence",
                    );
                }
                if self.acceptances().any(|_| true) {
                    return fail("a final acceptance already exists");
                }
                if self.qualifying().is_some() {
                    return fail(
                        "a qualifying delivery preceded cancellation: the buyer cannot cancel to avoid assessing it",
                    );
                }
            }
        }
        Ok(())
    }

    /// What the final records establish now.
    #[must_use]
    pub fn final_state(&self) -> Final {
        let superseded: BTreeSet<&String> = self
            .acceptances()
            .flat_map(|(_, a)| a.supersedes.iter())
            .collect();
        let heads: Vec<(&String, &Acceptance)> = self
            .acceptances()
            .filter(|(d, _)| !superseded.contains(d))
            .collect();
        match heads.as_slice() {
            [] => Final::Open,
            [(d, a)] => match a.outcome {
                Outcome::Accepted => Final::Accepted {
                    acceptance: (*d).clone(),
                    amount_msat: a.amount_due_msat.unwrap_or(0),
                },
                Outcome::Rejected => Final::Rejected {
                    acceptance: (*d).clone(),
                },
                Outcome::Unresolved => Final::Unresolved {
                    acceptance: (*d).clone(),
                },
            },
            many => Final::Conflict(many.iter().map(|(d, _)| (*d).clone()).collect()),
        }
    }

    /// The obligation's collection state. A passing check or an accepted
    /// review alone creates nothing; only a final accepted record does.
    #[must_use]
    pub fn collection(&self) -> Collection {
        match (self.final_state(), &self.funded) {
            (Final::Accepted { acceptance, .. }, Some((funded_for, hash)))
                if acceptance == *funded_for =>
            {
                Collection::Funded {
                    acceptance,
                    payment_hash: hash.clone(),
                }
            }
            (_, Some((_, hash))) => Collection::RefundOwed {
                payment_hash: hash.clone(),
            },
            (
                Final::Accepted {
                    acceptance,
                    amount_msat,
                },
                None,
            ) if amount_msat > 0 => Collection::Unknown {
                acceptance,
                amount_msat,
            },
            _ => Collection::None,
        }
    }

    /// Record funding reported by the admitted central receive adapter in
    /// the existing ledger, against the final accepted obligation. Exact
    /// retries are idempotent.
    ///
    /// # Errors
    ///
    /// When there is no accepted obligation, the order is in conflict, or
    /// the ledger refuses the funding.
    pub fn fund(
        &mut self,
        ledger: &mut pay_ledger::Ledger,
        pins: &BTreeMap<&str, String>,
        receipt: &pay_ledger::markets::worker::FundingReceipt,
    ) -> Result<pay_ledger::Recorded> {
        let Final::Accepted {
            acceptance,
            amount_msat,
        } = self.final_state()
        else {
            return fail("only a final accepted order has an obligation to fund");
        };
        if amount_msat == 0 {
            return fail("a free order has nothing to fund");
        }
        let a = match self.get(&acceptance) {
            Some(Record::Acceptance(a)) => a.clone(),
            _ => return fail("the acceptance is not held"),
        };
        let submission = a
            .submission
            .clone()
            .ok_or("an accepted order names its submission")?;
        let (delivery, _) = self
            .timely_delivery(&submission)
            .ok_or("the accepted submission has no timely delivery")?;
        let terms = self.terms.worker_terms(pins)?;
        let plain = |d: &str| d.trim_start_matches("sha256:").to_string();
        let earned = pay_ledger::markets::worker::Earned {
            obligation: terms.obligation(),
            order: terms.order.clone(),
            labor_terms: terms.labor_terms.clone(),
            delivery: plain(delivery),
            verification: plain(
                a.verification
                    .as_deref()
                    .ok_or("an accepted order names its verification")?,
            ),
            acceptance: plain(&acceptance),
            buyer: terms.buyer.clone(),
            provider: terms.provider.clone(),
            accepted_msat: terms.price_msat,
        };
        let recorded = ledger
            .record_worker_earned(&terms, &earned, receipt)
            .map_err(|e| e.to_string())?;
        self.funded = Some((acceptance, receipt.payment_hash.clone()));
        Ok(recorded)
    }

    /// Restore a funding observation from a journal.
    pub(crate) fn restore_funding(&mut self, acceptance: String, payment_hash: String) {
        self.funded = Some((acceptance, payment_hash));
    }

    pub(crate) fn funding(&self) -> Option<&(String, String)> {
        self.funded.as_ref()
    }
}
