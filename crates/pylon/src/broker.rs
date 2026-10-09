//! Brokered pylon sales (P3 of `docs/compute/verse-compute.md`): a
//! customer pays OpenAgents, by an x402 payment or a debit from the
//! purchased compute balance; OpenAgents' broker key buys the job from a
//! pylon and signs its `3201` receipt; and the sale settles in the
//! central split ledger (`crates/pay-ledger`) under the v2 rule, with the
//! provider's share bound to that receipt.
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

use nostr::domain::Event;
use nostr::pylon::check::{Verdict, counted, parse_check};
use nostr::pylon::{Outcome, Receipt, parse_receipt};
use pay_ledger::adjustment::Adjustment;
use pay_ledger::payout::{Invoice, Lookup, Outcome as SendOutcome, Policy, Rails, Step, tick};
use pay_ledger::{Ledger, PayoutState, Rail, Recorded, SettlementInput, Split};

use crate::paid::{Grant, Network};

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
}

impl Broker {
    /// A broker over `ledger` on `network`, buying as `key`. Installs the
    /// v2 split rule.
    ///
    /// # Errors
    ///
    /// When the rule cannot be installed.
    pub fn open(mut ledger: Ledger, network: Network, key: &str) -> Result<Self, String> {
        ledger.install_pylon_rule().map_err(|e| e.to_string())?;
        Ok(Self {
            ledger,
            network,
            key: key.into(),
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
    /// whose hash is the settlement key; `received_msat` is what reached
    /// the receiver after its service-provider fee. `owner` is the pylon's
    /// verified NIP-OA owner, if any. Replaying a settled sale returns it.
    ///
    /// # Errors
    ///
    /// A receipt that does not verify, that the broker did not buy, that
    /// was not accepted, or whose payment is missing, on another network,
    /// or under another profile; and ledger refusals.
    pub fn settle(
        &mut self,
        event: &Event,
        owner: Option<&str>,
        received_msat: u64,
        at: i64,
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
        let price = i64::try_from(payment.amount_msat).map_err(|e| e.to_string())?;
        self.ledger
            .record_settlement(SettlementInput {
                key: payment.payment_hash.clone(),
                resource: pay_ledger::pylon::RESOURCE.into(),
                plugin_id: None,
                release_id: None,
                price_msat: price,
                received_msat: i64::try_from(received_msat).map_err(|e| e.to_string())?,
                rail: Rail::Lightning,
                payer_alias: None,
                settled_at: at,
                split: Split::PylonJob {
                    provider: party(&receipt.provider, owner),
                    receipt: event.id.clone(),
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
