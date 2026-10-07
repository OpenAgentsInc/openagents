//! Original expense returns use protected review and the original custodian.
use crate::{Controller, Error, Result, digest, now};
use openagents_wallet::{
    IssuedInvoice, LightningWallet, PaymentDirection, PaymentStatus,
    custody::{Permit, Terms},
};
use pay_ledger::shared::{Liability, RefundPlan, RefundReview};
use serde_json::{Value, json};
impl Controller {
    fn reviewed_return(&self, binding: &str, id: &str, new: bool) -> Result<RefundReview> {
        self.policy.bytes(256 * 1024)?;
        let review = self
            .config
            .refunds
            .iter()
            .find(|r| r.id == id)
            .ok_or(Error::Denied)?
            .clone();
        let ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.current(self.grant(binding)?, false)?;
        let original = ledger
            .shared_outcome(&review.intent)?
            .ok_or(Error::Denied)?;
        if original.intent.digest() != review.intent_digest
            || !original
                .intent
                .binding
                .mode()
                .same_native(&self.grant(binding)?.binding.mode())
            || original.state != "settled"
        {
            return Err(Error::Denied);
        }
        if new && (now() < review.reviewed_at || now() >= review.valid_until) {
            return Err(Error::Denied);
        }
        Ok(review)
    }
    pub(crate) fn refund(&self, binding: &str, id: &str) -> Result<Value> {
        let review = self.reviewed_return(binding, id, true)?;
        let (plan, fresh) = {
            let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current_cleanup(self.grant(binding)?)?;
            if let Some(old) = ledger.shared_refund(id)? {
                return Ok(json!({"state":"returned","receipt":old}));
            }
            let fresh = ledger.shared_return_plan(id)?.is_none();
            let plan = ledger.shared_prepare_return(&review, now())?;
            let original = ledger
                .shared_outcome(&review.intent)?
                .ok_or(Error::Denied)?;
            if matches!(original.intent.liability, Liability::NativeService { .. }) {
                let returned = ledger.shared_return_expense(&review, None, now() as i64)?;
                return Ok(json!({"state":"returned","receipt":returned}));
            }
            if plan.returned_msat <= 0 || plan.original.expense.is_none() {
                return Err(Error::Denied);
            }
            (plan, fresh)
        };
        if fresh {
            let expiry = plan
                .due
                .checked_sub(now().saturating_add(61))
                .filter(|v| *v > 0)
                .ok_or(Error::Unknown)?;
            let writer = self.writer.token()?;
            let permit = Permit::sign(
                &self.config.origin,
                &plan.binding.custodian_node,
                &plan.digest(),
                Terms::Receive {
                    amount_msat: plan.returned_msat as u64,
                    request_hash: plan.request_hash.clone(),
                    expiry_secs: expiry.try_into().map_err(|_| Error::Denied)?,
                },
                &writer,
            )?;
            // Preparation reserves the original refundable units before IPC. A lost
            // issuance reply stays unknown and never manufactures another invoice.
            let issued = self.wallet.custodial_receive(permit, &writer)?;
            Self::validate_return_invoice(&plan, &issued)?;
            let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(self.grant(binding)?, false)?;
            ledger.shared_seal_return_invoice(id, &serde_json::to_value(&issued)?)?;
        }
        self.refund_status(binding, id)
    }
    fn validate_return_invoice(plan: &RefundPlan, issued: &IssuedInvoice) -> Result<()> {
        let invoice = nostr::x402::decode_invoice(&issued.bolt11).map_err(|_| Error::Denied)?;
        if issued.amount_msat != plan.returned_msat as u64
            || issued.description_hash != plan.request_hash
            || issued.pay_to != plan.binding.custodian_node
            || hex::encode(invoice.payee()) != issued.pay_to
            || hex::encode(invoice.payment_hash()) != issued.payment_hash
            || invoice.amount_msat() != issued.amount_msat
            || hex::encode(invoice.description_hash()) != issued.description_hash
            || invoice
                .created_at()
                .saturating_add(invoice.expiry_seconds())
                > plan.due
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    pub(crate) fn refund_status(&self, binding: &str, id: &str) -> Result<Value> {
        let review = self.reviewed_return(binding, id, false)?;
        let (plan, issued) = {
            let ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(self.grant(binding)?, false)?;
            if let Some(old) = ledger.shared_refund(id)? {
                return Ok(json!({"state":"returned","receipt":old}));
            }
            let plan = ledger.shared_return_plan(id)?.ok_or(Error::Denied)?;
            let issued = ledger.shared_return_invoice(id)?;
            (plan, issued)
        };
        let issued = match issued {
            Some(i) => i,
            None => {
                let recovered = self.wallet.custodial_result(
                    &plan.binding.custodian_node,
                    &plan.digest(),
                    &self.writer.token()?,
                )?;
                let Some(recovered) = recovered else {
                    return Ok(json!({"state":"unknown","original":plan}));
                };
                let permit: Permit = serde_json::from_value(recovered["permit"].clone())?;
                let Terms::Receive {
                    amount_msat,
                    request_hash,
                    expiry_secs,
                } = permit.terms
                else {
                    return Err(Error::Denied);
                };
                if permit.origin != self.config.origin
                    || permit.node != plan.binding.custodian_node
                    || permit.intent != plan.digest()
                    || amount_msat != plan.returned_msat as u64
                    || request_hash != plan.request_hash
                    || expiry_secs == 0
                {
                    return Err(Error::Denied);
                }
                let invoice: IssuedInvoice = serde_json::from_value(recovered["result"].clone())?;
                Self::validate_return_invoice(&plan, &invoice)?;
                let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
                self.current(self.grant(binding)?, false)?;
                ledger.shared_seal_return_invoice(id, &serde_json::to_value(&invoice)?)?;
                serde_json::to_value(invoice)?
            }
        };
        let issued: IssuedInvoice = serde_json::from_value(issued)?;
        Self::validate_return_invoice(&plan, &issued)?;
        let record = self.wallet.lookup_from_node(
            &plan.binding.custodian_node,
            openagents_wallet::parse_hash32(&issued.payment_hash)?,
        )?;
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.current(self.grant(binding)?, false)?;
        if let Some(r) = record {
            if r.direction == PaymentDirection::Inbound
                && r.status == PaymentStatus::Succeeded
                && r.payment_hash == issued.payment_hash
                && r.amount_msat == Some(issued.amount_msat)
                && r.bolt11.as_ref().is_none_or(|i| i == &issued.bolt11)
            {
                let receipt = ledger.shared_return_expense(
                    &review,
                    Some(&issued.payment_hash),
                    now() as i64,
                )?;
                return Ok(json!({"state":"returned","receipt":receipt}));
            }
        }
        Ok(json!({"state":"issued-held","invoice":issued,"original":plan}))
    }
}

// The caller selects a protected review ID and cannot substitute its terms.
#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FundingReversalReview {
    pub id: String,
    pub funding: String,
    pub funding_digest: String,
    pub amount_msat: i64,
    pub evidence: String,
    pub reviewed_at: u64,
    pub valid_until: u64,
}
impl Controller {
    pub(crate) fn reverse_funding(&self, binding: &str, review_id: &str) -> Result<Value> {
        self.policy.bytes(256 * 1024)?;
        let review = self
            .config
            .funding_reversals
            .iter()
            .find(|r| r.id == review_id)
            .ok_or(Error::Denied)?;
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.current_cleanup(self.grant(binding)?)?;
        if now() < review.reviewed_at || now() >= review.valid_until {
            return Err(Error::Denied);
        }
        let (original_id, terms, invoice) = ledger
            .shared_funding_record(&review.funding)?
            .ok_or(Error::Denied)?;
        let original = ledger.shared_binding(&original_id)?.ok_or(Error::Denied)?;
        if invoice.is_none()
            || !original
                .mode()
                .same_native(&self.grant(binding)?.binding.mode())
            || digest(&serde_json::to_vec(&terms)?) != review.funding_digest
        {
            return Err(Error::Denied);
        }
        let receipt = ledger.shared_reverse_funding(
            &review.id,
            &review.funding,
            review.amount_msat,
            &review.evidence,
        )?;
        // Evidence is a protected operator source-loss review, not an incoming
        // payment, another top-up, or permission to release an uncertain charge.
        Ok(serde_json::to_value(receipt)?)
    }
}
