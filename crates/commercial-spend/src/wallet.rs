//! Original invoice effects and directional funding evidence never share roles.
use crate::{Controller, Error, Result, digest, now};
use openagents_wallet::{
    IssuedInvoice, LightningWallet, PaymentDirection, PaymentStatus, Proof,
    custody::{Permit, Terms},
};
use pay_ledger::{
    compute::{Receipt, TopUp},
    shared::{Binding, Intent, Invoice, Liability, Outcome},
};
use serde_json::{Value, json};
/// Construct the exact canonical intent before native buyer approval.
pub fn plugin_intent(
    id: &str,
    offer: &coder::customer::plugins::Offer,
    binding: Binding,
) -> Result<Intent> {
    if binding.conversion.source != receipts::funding_units::Unit::Millisatoshis
        || binding.conversion.numerator != binding.conversion.denominator
    {
        return Err(Error::Denied);
    }
    let invoice = nostr::x402::decode_invoice(offer.invoice()).map_err(|_| Error::Denied)?;
    let attempt = format!("plugin-purchase:{id}");
    let quote = openagents_x402::execution::quote_digest(&offer.quote);
    let intent_id = Intent::stable_id(&binding, &attempt);
    let intent = Intent {
        id: intent_id.clone(),
        binding,
        native_attempt: attempt,
        quote: quote.clone(),
        // Two separately approved purchases can share a release and price. Their
        // execution identity must retain the original buyer and purchase instead.
        execution: intent_id,
        terms: offer.request_hash.clone(),
        maximum_units: offer.quote.price_msat,
        fee_cap_msat: offer.max_fee_msat,
        invoice: Some(Invoice {
            bolt11: offer.invoice().into(),
            payment_hash: hex::encode(invoice.payment_hash()),
            request_hash: hex::encode(invoice.description_hash()),
            receiver: hex::encode(invoice.payee()),
            network: offer.payer.network.clone(),
            amount_msat: invoice.amount_msat(),
            valid_until: invoice
                .created_at()
                .saturating_add(invoice.expiry_seconds()),
        }),
        liability: Liability::ExternalInvoice {
            merchant: offer.url.clone(),
            plugin: offer.quote.plugin.clone().ok_or(Error::Denied)?,
            release: offer.quote.release.clone().ok_or(Error::Denied)?,
            author: offer.quote.author.clone().ok_or(Error::Denied)?,
            author_fee_msat: offer.quote.fee_msat.ok_or(Error::Denied)?,
        },
        admitted_at: invoice.created_at(),
    };
    if intent.binding.commercial != *offer.commercial.as_ref().ok_or(Error::Denied)?
        || intent.binding.custodian_node != offer.payer.node
    {
        return Err(Error::Denied);
    }
    intent.reserved_msat()?;
    Ok(intent)
}
impl Controller {
    fn buyer_approval(&self, intent: &Intent) -> Result<Value> {
        use std::os::unix::fs::MetadataExt;
        let grant = self.grant(&intent.binding.id)?;
        let crate::Native::Tenancy {
            buyer_root: Some(root),
            principal,
            tenant,
            member_epoch,
            members_epoch,
            ..
        } = &grant.native
        else {
            return Err(Error::Denied);
        };
        let held = self
            .buyer_directories
            .get(&grant.binding.id)
            .ok_or(Error::Denied)?
            .metadata()?;
        let check = || -> Result<()> {
            let current = std::fs::symlink_metadata(root)?;
            if !current.is_dir() || current.dev() != held.dev() || current.ino() != held.ino() {
                return Err(Error::Denied);
            }
            Ok(())
        };
        check()?;
        let id = intent
            .native_attempt
            .strip_prefix("plugin-purchase:")
            .ok_or(Error::Denied)?;
        let approved =
            coder::customer::Store::shared_plugin_approval(root, id, now().saturating_mul(1000))
                .map_err(|_| Error::Denied)?;
        let reference = approved.offer.shared.as_ref().ok_or(Error::Denied)?;
        if plugin_intent(id, &approved.offer, intent.binding.clone())? != *intent
            || reference.intent != intent.id
            || reference.digest != intent.digest()
            || reference.mode != intent.binding.mode()
            || approved.selection.context.account != intent.binding.source.account
            || approved.selection.context.credential_reference != *principal
            || approved.selection.context.tenant != *tenant
            || approved.selection.context.membership_epoch != *member_epoch
            || approved.selection.context.workspace_members_epoch != *members_epoch
            || approved.offer.payer.home != self.config.wallet_home
            || Some(&approved.selection.context.workspace)
                != intent.binding.source.workspace.as_ref()
        {
            return Err(Error::Denied);
        }
        check()?;
        Ok(
            json!({"purchase":id,"approval":approved.approval_digest,"offer":approved.offer.digest(),"intent":reference.digest}),
        )
    }
    pub(crate) fn validate_invoice(&self, intent: &Intent) -> Result<()> {
        let invoice = intent.invoice.as_ref().ok_or(Error::Denied)?;
        let parsed = nostr::x402::decode_invoice(&invoice.bolt11).map_err(|_| Error::Denied)?;
        let network = match parsed.currency() {
            "bc" => nostr::x402::MAINNET,
            "tb" => nostr::x402::TESTNET,
            _ => return Err(Error::Denied),
        };
        if hex::encode(parsed.payment_hash()) != invoice.payment_hash
            || hex::encode(parsed.description_hash()) != invoice.request_hash
            || hex::encode(parsed.payee()) != invoice.receiver
            || parsed.amount_msat() != invoice.amount_msat
            || network != invoice.network
            || parsed.created_at().saturating_add(parsed.expiry_seconds()) != invoice.valid_until
            || now() >= invoice.valid_until
        {
            return Err(Error::Denied);
        }
        Ok(())
    }
    fn proof(&self, intent: &Intent, proof: Proof) -> Result<Outcome> {
        let invoice = intent.invoice.as_ref().ok_or(Error::Denied)?;
        let preimage = hex::decode(&proof.preimage).map_err(|_| Error::Denied)?;
        if preimage.len() != 32
            || digest(&preimage) != invoice.payment_hash
            || proof.payment_hash != invoice.payment_hash
            || proof.bolt11 != invoice.bolt11
            || proof.amount_msat != invoice.amount_msat
            || proof.fee_msat > intent.fee_cap_msat
        {
            return Err(Error::Denied);
        }
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.ledger_custody.check()?;
        Ok(ledger.shared_settle(
            &intent.id,
            intent.maximum_units,
            proof.fee_msat,
            &format!("wallet:{}", proof.payment_hash),
            Some(&serde_json::to_value(&proof)?),
            now() as i64,
        )?)
    }
    pub(crate) fn dispatch_plugin(&self, intent: Intent, wait_secs: u64) -> Result<Value> {
        if !(1..=60).contains(&wait_secs) {
            return Err(Error::Denied);
        }
        let grant = self.grant(&intent.binding.id)?;
        {
            let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(grant, true)?;
            self.validate_invoice(&intent)?;
            self.buyer_approval(&intent)?;
            if intent.binding != grant.binding {
                return Err(Error::Denied);
            }
            if let Some(old) = ledger.shared_outcome(&intent.id)? {
                if old.intent != intent {
                    return Err(Error::Denied);
                }
                return Ok(serde_json::to_value(old)?);
            }
            ledger.shared_reserve(&intent)?;
            ledger.shared_handoff(&intent.id)?;
        }
        let invoice = intent.invoice.as_ref().ok_or(Error::Denied)?;
        let writer = self.writer.token()?;
        let permit = Permit::sign(
            &self.config.origin,
            &intent.binding.custodian_node,
            &intent.digest(),
            Terms::Pay {
                invoice: invoice.bolt11.clone(),
                max_fee_msat: intent.fee_cap_msat,
                wait_secs,
            },
            &writer,
        )?;
        // No canonical or Accounts guard is held across resident IPC. The resident
        // authenticates the once-only callback after its own financial queue wait.
        match self.wallet.custodial_pay(permit, &writer) {
            Ok(proof) => match self.proof(&intent, proof) {
                Ok(out) => Ok(serde_json::to_value(out)?),
                Err(_) => self.mark_unknown(&intent.id),
            },
            Err(_) => self.mark_unknown(&intent.id),
        }
    }
    fn mark_unknown(&self, id: &str) -> Result<Value> {
        Ok(serde_json::to_value(
            self.ledger
                .lock()
                .map_err(|_| Error::Denied)?
                .shared_unknown(id)?,
        )?)
    }
    pub(crate) fn reconcile_plugin(&self, id: &str, binding: &str) -> Result<Value> {
        let intent = {
            let ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(self.grant(binding)?, false)?;
            let old = ledger.shared_outcome(id)?.ok_or(Error::Denied)?;
            if !old
                .intent
                .binding
                .mode()
                .same_native(&self.grant(binding)?.binding.mode())
            {
                return Err(Error::Denied);
            }
            if old.state == "settled" {
                return Ok(serde_json::to_value(old)?);
            }
            old.intent
        };
        let invoice = intent.invoice.as_ref().ok_or(Error::Denied)?;
        let hash = openagents_wallet::parse_hash32(&invoice.payment_hash)?;
        if let Some(record) = self
            .wallet
            .lookup_from_node(&intent.binding.custodian_node, hash)?
        {
            if record.direction == PaymentDirection::Outbound
                && record.status == PaymentStatus::Succeeded
                && record.payment_hash == invoice.payment_hash
                && record.amount_msat == Some(invoice.amount_msat)
            {
                if let (Some(preimage), Some(fee), Some(bolt11)) =
                    (record.preimage, record.fee_msat, record.bolt11)
                {
                    return Ok(serde_json::to_value(self.proof(
                        &intent,
                        Proof {
                            payment_hash: invoice.payment_hash.clone(),
                            preimage,
                            fee_msat: fee,
                            amount_msat: invoice.amount_msat,
                            bolt11,
                        },
                    )?)?);
                }
            }
        }
        // Missing, pending, or failed lookup is not proof that IPC never occurred.
        self.mark_unknown(id)
    }
    pub(crate) fn custody_authorization(&self, permit: Permit) -> Result<Value> {
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.ledger_custody.check()?;
        let writer = self.writer.token()?;
        let expected = Permit::sign(
            &self.config.origin,
            &self.wallet.bound_payment_identity()?,
            &permit.intent,
            permit.terms.clone(),
            &writer,
        )?;
        if permit != expected {
            return Err(Error::Denied);
        }
        let authority = if let Some(intent) = ledger.shared_intent_by_digest(&permit.intent)? {
            let Terms::Pay {
                invoice,
                max_fee_msat,
                wait_secs,
            } = &permit.terms
            else {
                return Err(Error::Denied);
            };
            if !(1..=60).contains(wait_secs)
                || intent.invoice.as_ref().is_none_or(|i| &i.bolt11 != invoice)
                || *max_fee_msat != intent.fee_cap_msat
            {
                return Err(Error::Denied);
            }
            self.validate_invoice(&intent)?;
            let native = self.current(self.grant(&intent.binding.id)?, true)?;
            let approval = self.buyer_approval(&intent)?;
            json!({"native":native,"buyer":approval})
        } else if let Some((binding, terms)) = ledger.shared_funding_by_digest(&permit.intent)? {
            let Terms::Receive {
                amount_msat,
                request_hash,
                expiry_secs,
            } = &permit.terms
            else {
                return Err(Error::Denied);
            };
            if terms["amount_msat"] != *amount_msat
                || terms["request_hash"] != *request_hash
                || terms["expiry_secs"] != *expiry_secs
                || terms["due"]
                    .as_u64()
                    .is_none_or(|due| now().saturating_add(u64::from(*expiry_secs)) >= due)
            {
                return Err(Error::Denied);
            }
            self.current(self.grant(&binding)?, true)?
        } else if let Some(plan) = ledger.shared_return_by_digest(&permit.intent)? {
            let Terms::Receive {
                amount_msat,
                request_hash,
                expiry_secs,
            } = &permit.terms
            else {
                return Err(Error::Denied);
            };
            if *amount_msat != plan.returned_msat as u64
                || *request_hash != plan.request_hash
                || *expiry_secs == 0
                || now().saturating_add(u64::from(*expiry_secs)) >= plan.due
                || self.config.refunds.iter().find(|r| r.id == plan.review.id) != Some(&plan.review)
            {
                return Err(Error::Denied);
            }
            self.current_cleanup(self.original_grant(&plan.binding)?)?
        } else {
            return Err(Error::Denied);
        };
        ledger.shared_wallet_handoff(
            &permit.intent,
            &serde_json::to_value(&permit)?,
            &authority,
            now(),
        )?;
        // A native key file or membership can change independently of SQLite.
        // Recheck after sealing; uncertainty retains this intent and never pays.
        if let Some(intent) = ledger.shared_intent_by_digest(&permit.intent)? {
            if authority["native"] != self.current(self.grant(&intent.binding.id)?, true)?
                || authority["buyer"] != self.buyer_approval(&intent)?
            {
                return Err(Error::Denied);
            }
        } else if let Some((binding, _)) = ledger.shared_funding_by_digest(&permit.intent)? {
            if authority != self.current(self.grant(&binding)?, true)? {
                return Err(Error::Denied);
            }
        } else if let Some(plan) = ledger.shared_return_by_digest(&permit.intent)? {
            if authority != self.current_cleanup(self.original_grant(&plan.binding)?)? {
                return Err(Error::Denied);
            }
        } else {
            return Err(Error::Denied);
        }
        Ok(json!({"authorized":true,"origin":self.config.origin,"intent":permit.intent}))
    }
    pub(crate) fn funding(&self, binding: &str, purchase: &str, amount_sats: u64) -> Result<Value> {
        let g = self.grant(binding)?;
        let amount = amount_sats
            .checked_mul(1000)
            .filter(|a| *a > 0 && *a <= 1_000_000_000)
            .ok_or(Error::Denied)?;
        let id = format!(
            "shared-funding:{}",
            digest(&serde_json::to_vec(&(
                g.binding.native_identity(),
                purchase
            ))?)
        );
        let (terms, fresh) = {
            let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(g, true)?;
            if let Some((b, terms, _)) = ledger.shared_funding_record(&id)? {
                let original = ledger.shared_binding(&b)?.ok_or(Error::Denied)?;
                if !original.mode().same_native(&g.binding.mode())
                    || terms["purchase"] != purchase
                    || terms["amount_msat"] != amount
                {
                    return Err(Error::Denied);
                }
                (terms, false)
            } else {
                let created = now();
                let request_hash = digest(&serde_json::to_vec(&(
                    "shared-funding",
                    &g.binding,
                    purchase,
                    amount,
                    created,
                ))?);
                let terms = json!({"binding":g.binding,"purchase":purchase,"amount_msat":amount,"request_hash":request_hash,"created_at":created,"due":created+3600,"expiry_secs":3539,"direction":"inbound"});
                let fresh = ledger.shared_begin_funding(&id, &g.binding, &terms)?;
                (terms, fresh)
            }
        };
        if fresh {
            let writer = self.writer.token()?;
            let permit = Permit::sign(
                &self.config.origin,
                &g.binding.custodian_node,
                &digest(&serde_json::to_vec(&terms)?),
                Terms::Receive {
                    amount_msat: amount,
                    request_hash: terms["request_hash"].as_str().ok_or(Error::Denied)?.into(),
                    expiry_secs: 3539,
                },
                &writer,
            )?;
            let invoice = self.wallet.custodial_receive(permit, &writer)?;
            let parsed = Self::validate_funding_invoice(&g.binding, &terms, &invoice)?;
            let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(g, true)?;
            ledger.shared_funding_invoice(&id, &serde_json::to_value(&invoice)?)?;
            ledger.open_top_up(&TopUp {
                id: id.clone(),
                account: g.binding.pool.clone(),
                amount_msat: amount as i64,
                payment_hash: invoice.payment_hash,
                invoice: invoice.bolt11,
                created_at: parsed.created_at() as i64,
                expires_at: (parsed.created_at() + parsed.expiry_seconds()) as i64,
            })?;
        }
        self.funding_status(binding, purchase)
    }
    fn validate_funding_invoice(
        binding: &Binding,
        terms: &Value,
        issued: &IssuedInvoice,
    ) -> Result<nostr::x402::Invoice> {
        let parsed = nostr::x402::decode_invoice(&issued.bolt11).map_err(|_| Error::Denied)?;
        if terms["binding"] != serde_json::to_value(binding)?
            || terms["direction"] != "inbound"
            || terms["amount_msat"] != issued.amount_msat
            || terms["request_hash"] != issued.description_hash
            || issued.pay_to != binding.custodian_node
            || hex::encode(parsed.payee()) != issued.pay_to
            || hex::encode(parsed.payment_hash()) != issued.payment_hash
            || parsed.amount_msat() != issued.amount_msat
            || hex::encode(parsed.description_hash()) != issued.description_hash
            || parsed.created_at() < terms["created_at"].as_u64().ok_or(Error::Denied)?
            || parsed
                .created_at()
                .checked_add(parsed.expiry_seconds())
                .is_none_or(|expiry| expiry > terms["due"].as_u64().unwrap_or(0))
        {
            return Err(Error::Denied);
        }
        Ok(parsed)
    }
    pub(crate) fn funding_status(&self, binding: &str, purchase: &str) -> Result<Value> {
        let g = self.grant(binding)?;
        let id = format!(
            "shared-funding:{}",
            digest(&serde_json::to_vec(&(
                g.binding.native_identity(),
                purchase
            ))?)
        );
        let (terms, invoice) = {
            let ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
            self.current(self.grant(binding)?, false)?;
            let (b, t, i) = ledger.shared_funding_record(&id)?.ok_or(Error::Denied)?;
            let original = ledger.shared_binding(&b)?.ok_or(Error::Denied)?;
            if !original.mode().same_native(&g.binding.mode()) {
                return Err(Error::Denied);
            }
            (t, i)
        };
        let original: Binding = serde_json::from_value(terms["binding"].clone())?;
        let invoice = match invoice {
            Some(invoice) => invoice,
            None => {
                let intent = digest(&serde_json::to_vec(&terms)?);
                let writer = self.writer.token()?;
                let Some(recovered) =
                    self.wallet
                        .custodial_result(&original.custodian_node, &intent, &writer)?
                else {
                    return Ok(json!({"id":purchase,"state":"unknown","original_terms":terms}));
                };
                let permit: Permit = serde_json::from_value(recovered["permit"].clone())?;
                let expected = Permit::sign(
                    &self.config.origin,
                    &original.custodian_node,
                    &intent,
                    Terms::Receive {
                        amount_msat: terms["amount_msat"].as_u64().ok_or(Error::Denied)?,
                        request_hash: terms["request_hash"].as_str().ok_or(Error::Denied)?.into(),
                        expiry_secs: terms["expiry_secs"]
                            .as_u64()
                            .ok_or(Error::Denied)?
                            .try_into()
                            .map_err(|_| Error::Denied)?,
                    },
                    &writer,
                )?;
                if permit != expected {
                    return Err(Error::Denied);
                }
                let issued: IssuedInvoice = serde_json::from_value(recovered["result"].clone())?;
                Self::validate_funding_invoice(&original, &terms, &issued)?;
                let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
                self.current(self.grant(binding)?, false)?;
                let value = serde_json::to_value(issued)?;
                ledger.shared_funding_invoice(&id, &value)?;
                value
            }
        };
        let issued: IssuedInvoice = serde_json::from_value(invoice)?;
        Self::validate_funding_invoice(&original, &terms, &issued)?;
        let node = &original.custodian_node;
        let report = self
            .wallet
            .lookup_from_node(node, openagents_wallet::parse_hash32(&issued.payment_hash)?)?;
        let mut ledger = self.ledger.lock().map_err(|_| Error::Denied)?;
        self.current(self.grant(binding)?, false)?;
        // A crash after retaining an issued invoice but before its native projection
        // reuses that exact invoice; it never asks the custodian for another one.
        if ledger.top_up(&id)?.is_none() {
            let parsed = nostr::x402::decode_invoice(&issued.bolt11).map_err(|_| Error::Denied)?;
            ledger.open_top_up(&TopUp {
                id: id.clone(),
                account: self.grant(binding)?.binding.pool.clone(),
                amount_msat: issued.amount_msat as i64,
                payment_hash: issued.payment_hash.clone(),
                invoice: issued.bolt11.clone(),
                created_at: parsed.created_at() as i64,
                expires_at: (parsed.created_at() + parsed.expiry_seconds()) as i64,
            })?;
        }
        if let Some(record) = report {
            if record.direction == PaymentDirection::Inbound
                && record.status == PaymentStatus::Succeeded
                && record.payment_hash == issued.payment_hash
                && record.amount_msat == Some(issued.amount_msat)
                && record.bolt11.as_ref().is_none_or(|b| b == &issued.bolt11)
            {
                ledger.observe_top_up(
                    &issued.payment_hash,
                    &Receipt::Paid {
                        received_msat: issued.amount_msat as i64,
                        at: now() as i64,
                    },
                )?;
            }
        }
        let p = ledger.top_up(&id)?.ok_or(Error::Unknown)?;
        Ok(
            json!({"id":purchase,"state":p.state.as_str(),"invoice":issued,"original_terms":terms,"canonical_source":id,"observed_at":p.observed_at}),
        )
    }
}
