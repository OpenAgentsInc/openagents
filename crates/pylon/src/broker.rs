//! Brokered pylon sales (P3 of `docs/compute/verse-compute.md`): a
//! customer pays OpenAgents, by an x402 payment or a debit from the
//! purchased compute balance; OpenAgents' broker key buys the job from a
//! pylon and signs its `3201` receipt; and the sale settles in the
//! central split ledger (`crates/pay-ledger`) under the v2 rule, with the
//! provider's share bound to that receipt.
//!
//! The customer's x402 payment goes through the embedded x402 facilitator
//! before the broker buys anything: [`Broker::quote`] issues the `http:1`
//! terms, and [`Broker::admit`] verifies the proof and inserts its replay
//! key exactly once. [`Broker::settle`] then records only a payment the
//! facilitator consumed, for the amount it consumed, so a receipt alone
//! never creates a sale. When the job used a priced plugin, its author's
//! per-call fee comes first in the split.
//!
//! Providers are paid by balance sweeps, never per job: [`Broker::sweep`]
//! runs the ordinary payout worker (`pay_ledger::payout::tick`) under
//! `Policy::pylon_sweeps` (1,000 sats owed, or the oldest share ten
//! minutes old). A trusted checker's `check-fail` on a job's receipt
//! forfeits that job's unpaid share ([`Broker::forfeit`]).
//!
//! One broker book per network: a test-network book never takes a mainnet
//! receipt, so test sats never mix with real ones. On `bitcoin` a sweep
//! needs the owner's standing grant, and every payout stays under its
//! per-payment and daily ceilings.

use std::collections::BTreeSet;
use std::sync::Arc;

use nostr::domain::Event;
use nostr::pylon::check::{Verdict, counted, parse_check};
use nostr::pylon::{Outcome, Receipt, parse_receipt};
use nostr::x402::{PaymentRequirements, binding_hash, http_binding};
use openagents_x402::facilitator::{Admission, Facilitator};
use openagents_x402::{PaymentPayload, ReplayStore};
use pay_ledger::adjustment::Adjustment;
use pay_ledger::payout::{Invoice, Lookup, Outcome as SendOutcome, Policy, Rails, Step, tick};
use pay_ledger::{Ledger, PayoutState, PluginFee, Rail, Recorded, SettlementInput, Split};
use serde_json::{Map, json};

use crate::paid::{Grant, Network, Receiver};

/// The replay store the broker's facilitator shares with every other
/// process that settles OpenAgents' x402 payments.
pub type Replay = Arc<dyn ReplayStore + Send + Sync>;

/// The ledger party a pylon's provider share goes to: the pylon's NIP-OA
/// owner when its beacon carries one, else the pylon key.
#[must_use]
pub fn party(provider: &str, owner: Option<&str>) -> String {
    owner.unwrap_or(provider).to_string()
}

/// The broker's book on one network.
pub struct Broker {
    ledger: Ledger,
    network: Network,
    /// The broker's key: the buyer of every receipt it settles.
    key: String,
    facilitator: Facilitator<Replay>,
}

impl Broker {
    /// A broker over `ledger` on `network`, buying as `key`, settling
    /// customers' x402 payments against `replay`. Installs the v2 split
    /// rule.
    ///
    /// # Errors
    ///
    /// When the rule cannot be installed.
    pub fn open(
        mut ledger: Ledger,
        network: Network,
        key: &str,
        replay: Replay,
    ) -> Result<Self, String> {
        ledger.install_pylon_rule().map_err(|e| e.to_string())?;
        Ok(Self {
            ledger,
            network,
            key: key.into(),
            facilitator: Facilitator::new(replay, nostr::x402::DEFAULT_CLOCK_SKEW),
        })
    }

    /// The x402 `http:1` terms for one brokered job sold at `url` for
    /// `amount_msat`, with `body` as the request: an invoice from
    /// OpenAgents' `receiver`, bound to that request.
    ///
    /// # Errors
    ///
    /// A book on a network x402 does not name, a malformed request, or a
    /// receiver that cannot issue the invoice.
    pub fn quote(
        &self,
        receiver: &dyn Receiver,
        url: &str,
        body: &[u8],
        amount_msat: u64,
        expiry_secs: u32,
    ) -> Result<PaymentRequirements, String> {
        let network = self
            .network
            .x402()
            .ok_or("x402 names bitcoin and testnet only")?;
        let hash =
            binding_hash(&http_binding("POST", url, body, &[]).map_err(|e| format!("{e:?}"))?)
                .map_err(|e| format!("{e:?}"))?;
        let mut raw = [0u8; 32];
        for (i, byte) in raw.iter_mut().enumerate() {
            *byte = u8::from_str_radix(&hash[2 * i..2 * i + 2], 16).map_err(|e| e.to_string())?;
        }
        let invoice = receiver.invoice(amount_msat, raw, expiry_secs)?;
        let mut extra = Map::new();
        extra.insert("assetTransferMethod".into(), json!("bolt11"));
        extra.insert("paymentFlow".into(), json!("upfront"));
        extra.insert("requestHash".into(), json!(hash));
        extra.insert("requestBindingProfile".into(), json!("http:1"));
        extra.insert("requestBindingParams".into(), json!({"headers": []}));
        extra.insert("invoice".into(), json!(invoice));
        Ok(PaymentRequirements {
            scheme: "exact".into(),
            network: network.into(),
            amount: amount_msat.to_string(),
            asset: "BTC".into(),
            pay_to: receiver.pay_to(),
            max_timeout_seconds: u64::from(expiry_secs),
            extra,
        })
    }

    /// Settle a customer's x402 payment for one brokered job through the
    /// facilitator: verify the proof against `requirements` (the terms
    /// [`Broker::quote`] issued) and insert its replay key exactly once,
    /// naming `sale`, the broker's own reference. Only an admitted payment
    /// buys a job; a replayed proof is `duplicate_settlement`.
    ///
    /// # Errors
    ///
    /// Terms on another network, and the facilitator's refusal reason.
    pub fn admit(
        &self,
        requirements: &PaymentRequirements,
        payload: &PaymentPayload,
        sale: &str,
        now: u64,
    ) -> Result<Admission, String> {
        if Some(requirements.network.as_str()) != self.network.x402() {
            return Err(format!(
                "terms on {} in the {} book",
                requirements.network,
                self.network.as_str()
            ));
        }
        self.facilitator
            .settle(requirements, payload, sale, now)
            .map_err(|refused| {
                refused
                    .error_reason
                    .unwrap_or_else(|| "settlement_failed".into())
            })
    }

    #[must_use]
    pub fn ledger(&self) -> &Ledger {
        &self.ledger
    }

    /// Record a provider's payout destination, which every sweep pays to.
    ///
    /// # Errors
    ///
    /// A malformed destination or a storage failure.
    pub fn register_payee(&mut self, payee: pay_ledger::Payee) -> Result<(), String> {
        self.ledger.register_payee(payee).map_err(|e| e.to_string())
    }

    #[must_use]
    pub fn network(&self) -> Network {
        self.network
    }

    /// The receipt, verified, if it is an accepted job the broker bought.
    fn job(&self, event: &Event, owner: Option<&str>) -> Result<Receipt, String> {
        let receipt = parse_receipt(event, owner)?;
        if receipt.buyer != self.key {
            return Err("the broker did not buy this job".into());
        }
        if receipt.outcome != Outcome::Accepted {
            return Err("only an accepted job is sold".into());
        }
        Ok(receipt)
    }

    /// Settle the x402 sale that paid for the job in `event`: the receipt
    /// carries the customer's payment (`x402-exact`, this book's network),
    /// whose hash is the settlement key, and the facilitator must already
    /// have consumed that payment ([`Broker::admit`]) for the same amount;
    /// `received_msat` is what reached the receiver after its
    /// service-provider fee. `owner` is the pylon's verified NIP-OA owner,
    /// if any; `plugin` is a priced plugin the job used. Replaying a
    /// settled sale returns it.
    ///
    /// # Errors
    ///
    /// A receipt that does not verify, that the broker did not buy, that
    /// was not accepted, or whose payment is missing, on another network,
    /// under another profile, or never settled through the facilitator;
    /// and ledger refusals.
    pub fn settle(
        &mut self,
        event: &Event,
        owner: Option<&str>,
        received_msat: u64,
        at: i64,
        plugin: Option<PluginFee>,
    ) -> Result<Recorded, String> {
        let receipt = self.job(event, owner)?;
        let payment = receipt
            .payment
            .as_ref()
            .ok_or("a brokered x402 sale's receipt names its payment")?;
        if payment.profile != "x402-exact" {
            return Err("a brokered sale is paid by x402".into());
        }
        if payment.network != self.network.as_str() {
            return Err(format!(
                "a {} receipt in the {} book",
                payment.network,
                self.network.as_str()
            ));
        }
        if received_msat > payment.amount_msat {
            return Err("received more than the price".into());
        }
        let network = self
            .network
            .x402()
            .ok_or("x402 names bitcoin and testnet only")?;
        let consumed = self
            .facilitator
            .store()
            .get(&format!("{network}:{}", payment.payment_hash))
            .map_err(|e| e.to_string())?
            .ok_or("the facilitator never settled this payment")?;
        if consumed.amount_msat != payment.amount_msat {
            return Err("the receipt's amount is not what the facilitator settled".into());
        }
        let price = i64::try_from(payment.amount_msat).map_err(|e| e.to_string())?;
        self.ledger
            .record_settlement(SettlementInput {
                key: payment.payment_hash.clone(),
                resource: pay_ledger::pylon::RESOURCE.into(),
                plugin_id: plugin.as_ref().map(|p| p.plugin_id.clone()),
                release_id: None,
                price_msat: price,
                received_msat: i64::try_from(received_msat).map_err(|e| e.to_string())?,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: at,
                split: Split::PylonJob {
                    provider: party(&receipt.provider, owner),
                    receipt: event.id.clone(),
                    plugin,
                },
            })
            .map_err(|e| e.to_string())
    }

    /// Settle a confirmed agent `order` whose compute ran as the job in
    /// `event` (P4): the buyer paid the order's fixed price (`terms`) to
    /// `receiver`, OpenAgents', by `bolt11`, an invoice bound to the order
    /// ([`crate::market::instruction`]), and `preimage` proves it. The
    /// seller's fee is the price less `compute_msat`, the broker's price
    /// for one job; it comes first, the provider's share is of the
    /// compute, and OpenAgents keeps the rest. The receipt names no
    /// payment: the order pays for it. Replaying a settled order returns
    /// it.
    ///
    /// # Errors
    ///
    /// A receipt the broker did not buy, that failed, or that names a
    /// payment; terms that are not fixed-price Lightning on this book's
    /// network or that name other parties; an invoice for another amount,
    /// order, or payee; a preimage that does not match; a payment the
    /// receiver never received; and ledger refusals.
    #[allow(clippy::too_many_arguments)]
    pub fn settle_order(
        &mut self,
        receiver: &dyn Receiver,
        event: &Event,
        owner: Option<&str>,
        order: &nostr::market_contracts::OrderRef,
        terms: &nostr::market_contracts::Terms,
        bolt11: &str,
        preimage: &str,
        compute_msat: u64,
        at: i64,
    ) -> Result<Recorded, String> {
        let receipt = self.job(event, owner)?;
        if receipt.payment.is_some() {
            return Err("an order's job carries no payment of its own".into());
        }
        if terms.payment_profile != nostr::market_contracts::LIGHTNING_PROFILE
            || terms.network.as_deref() != Some(self.network.as_str())
        {
            return Err(format!(
                "an order is paid by fixed-price Lightning on {}",
                self.network.as_str()
            ));
        }
        if terms.buyer != order.buyer || terms.provider != order.provider {
            return Err("the terms name other parties than the order".into());
        }
        let invoice = nostr::x402::decode_invoice(bolt11).map_err(|e| format!("{e:?}"))?;
        let hrp = match self.network {
            Network::Bitcoin => "bc",
            _ => "tb",
        };
        if invoice.currency() != hrp {
            return Err("the invoice is on another network".into());
        }
        if invoice.amount_msat() != terms.price_msat {
            return Err("the invoice is not for the order's price".into());
        }
        if invoice.description_hash() != crate::market::binding(order)? {
            return Err("the invoice is bound to another order".into());
        }
        let payee: String = invoice.payee().iter().map(|b| format!("{b:02x}")).collect();
        if payee != receiver.pay_to() {
            return Err("the invoice is not OpenAgents' receiver's".into());
        }
        let hash: String = invoice
            .payment_hash()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        if !crate::paid::preimage_matches(preimage, &hash) {
            return Err("the preimage does not match the invoice".into());
        }
        let received = receiver
            .received_msat(invoice.payment_hash())?
            .ok_or("the receiver never received this payment")?;
        if received > terms.price_msat {
            return Err("received more than the price".into());
        }
        if compute_msat > terms.price_msat {
            return Err("the compute costs more than the order's price".into());
        }
        let price = i64::try_from(terms.price_msat).map_err(|e| e.to_string())?;
        self.ledger
            .record_settlement(SettlementInput {
                key: hash,
                resource: pay_ledger::agent_order::RESOURCE.into(),
                plugin_id: None,
                release_id: None,
                price_msat: price,
                received_msat: i64::try_from(received).map_err(|e| e.to_string())?,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: at,
                split: Split::AgentOrder {
                    seller: terms.provider.clone(),
                    fee_msat: price - i64::try_from(compute_msat).map_err(|e| e.to_string())?,
                    provider: party(&receipt.provider, owner),
                    receipt: event.id.clone(),
                    order: order.order_id.clone(),
                },
            })
            .map_err(|e| e.to_string())
    }

    /// Settle a compute balance hold that paid for the job in `event` at
    /// `charge_msat`. The receipt names no payment: a balance debit has no
    /// preimage.
    ///
    /// # Errors
    ///
    /// As [`Broker::settle`], a receipt that names a payment, and the
    /// ledger's hold refusals.
    pub fn settle_debit(
        &mut self,
        event: &Event,
        owner: Option<&str>,
        hold: &str,
        charge_msat: i64,
        at: i64,
        plugin: Option<PluginFee>,
    ) -> Result<Recorded, String> {
        let receipt = self.job(event, owner)?;
        if receipt.payment.is_some() {
            return Err("a balance debit's receipt names no payment".into());
        }
        let (_, recorded) = self
            .ledger
            .settle_pylon_hold(
                hold,
                charge_msat,
                at,
                &party(&receipt.provider, owner),
                &event.id,
                plugin,
            )
            .map_err(|e| e.to_string())?;
        recorded.ok_or_else(|| "a zero charge settles nothing".into())
    }

    /// Forfeit the unpaid share of every sold job a trusted checker in
    /// `checkers` failed: `labels` are check labels and `receipts` the
    /// receipt events they name. Returns the adjustments, one per failed
    /// job (a replay returns the same ones).
    ///
    /// # Errors
    ///
    /// A ledger failure.
    pub fn forfeit(
        &mut self,
        labels: &[Event],
        receipts: &[Event],
        checkers: &BTreeSet<String>,
        at: i64,
    ) -> Result<Vec<Adjustment>, String> {
        let receipts = receipts
            .iter()
            .filter_map(|e| parse_receipt(e, None).ok().map(|r| (e.id.clone(), r)))
            .collect();
        let parsed: Vec<_> = labels.iter().filter_map(|e| parse_check(e).ok()).collect();
        let mut out = Vec::new();
        for check in counted(&parsed, &receipts, checkers) {
            if check.verdict != Verdict::Fail
                || self
                    .ledger
                    .pylon_job(&check.receipt)
                    .map_err(|e| e.to_string())?
                    .is_none()
            {
                continue;
            }
            out.push(
                self.ledger
                    .forfeit_pylon_job(&check.receipt, &check.id, at)
                    .map_err(|e| e.to_string())?,
            );
        }
        Ok(out)
    }

    /// One sweep at `now`: pay every provider whose balance is due under
    /// `Policy::pylon_sweeps` through `rails`. `resolve` finds a party's
    /// destination; `new_id` names a payout. On `bitcoin` it needs the
    /// owner's `grant`, and each payout stays under its ceilings; a test
    /// network ignores the grant.
    ///
    /// # Errors
    ///
    /// A mainnet sweep without a grant, and ledger failures.
    pub fn sweep(
        &mut self,
        rails: &dyn Rails,
        now: i64,
        grant: Option<Grant>,
        resolve: &mut pay_ledger::payout::Resolve<'_>,
        new_id: &mut dyn FnMut() -> String,
    ) -> Result<Vec<Step>, String> {
        let policy = Policy::pylon_sweeps();
        if self.network.is_test() {
            return tick(&mut self.ledger, rails, &policy, now, resolve, new_id)
                .map_err(|e| e.to_string());
        }
        let grant = grant.ok_or("mainnet provider payouts need the owner's standing grant")?;
        let spent = self.spent_since(now - 86_400).map_err(|e| e.to_string())?;
        let capped = Ceilings {
            inner: rails,
            grant,
            spent: std::cell::Cell::new(spent),
        };
        tick(&mut self.ledger, &capped, &policy, now, resolve, new_id).map_err(|e| e.to_string())
    }

    /// What payouts that may have gone out since `since` drained, msat.
    fn spent_since(&self, since: i64) -> pay_ledger::Result<u64> {
        let open = self.ledger.payouts(Some(&[
            PayoutState::Sending,
            PayoutState::Unknown,
            PayoutState::Sent,
        ]))?;
        Ok(open
            .iter()
            .filter(|p| p.created_at >= since)
            .map(|p| u64::try_from(p.sent_msat.unwrap_or(p.amount_msat)).unwrap_or(0))
            .sum())
    }
}

/// The owner's ceilings in front of the payout rails: an invoice over the
/// per-payment ceiling or past the day's is refused before anything is
/// journaled or sent, so the payout fails and its shares stay owed.
struct Ceilings<'a> {
    inner: &'a dyn Rails,
    grant: Grant,
    /// Drained in the last day, including payouts this sweep planned.
    spent: std::cell::Cell<u64>,
}

impl Ceilings<'_> {
    fn allows(&self, amount_msat: u64) -> Result<(), String> {
        if amount_msat > self.grant.per_payment_msat {
            return Err("over the owner's per-payment ceiling".into());
        }
        if self.spent.get().saturating_add(amount_msat) > self.grant.daily_msat {
            return Err("over the owner's daily ceiling".into());
        }
        Ok(())
    }

    /// Count a payout this sweep is about to send.
    fn take(&self, amount_msat: u64) -> Result<(), String> {
        self.allows(amount_msat)?;
        self.spent.set(self.spent.get().saturating_add(amount_msat));
        Ok(())
    }
}

impl Rails for Ceilings<'_> {
    fn lightning_invoice(&self, address: &str, amount_msat: i64) -> Result<Invoice, String> {
        self.take(u64::try_from(amount_msat).map_err(|e| e.to_string())?)?;
        self.inner.lightning_invoice(address, amount_msat)
    }
    fn pay_lightning(&self, invoice: &Invoice, max_fee_msat: i64) -> SendOutcome {
        // The invoice was counted when it was requested.
        if u64::try_from(invoice.amount_msat).map_or(true, |a| a > self.grant.per_payment_msat) {
            return SendOutcome::Failed("over the owner's per-payment ceiling".into());
        }
        self.inner.pay_lightning(invoice, max_fee_msat)
    }
    fn lookup_lightning(&self, payment_hash: &str) -> Result<Lookup, String> {
        self.inner.lookup_lightning(payment_hash)
    }
    fn fund_spark(&self, amount_sats: u64) -> Result<(), String> {
        self.take(amount_sats.saturating_mul(1000))?;
        self.inner.fund_spark(amount_sats)
    }
    fn pay_spark(&self, address: &str, amount_sats: u64, key: &str) -> SendOutcome {
        // The amount was counted when the Spark wallet was funded.
        if amount_sats.saturating_mul(1000) > self.grant.per_payment_msat {
            return SendOutcome::Failed("over the owner's per-payment ceiling".into());
        }
        self.inner.pay_spark(address, amount_sats, key)
    }
    fn lookup_spark(&self, key: &str) -> Result<Lookup, String> {
        self.inner.lookup_spark(key)
    }
}
