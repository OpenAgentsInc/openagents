//! The existing owners the treasury drives. Money stays in them: the worker
//! ledger (`pay-ledger`) records earnings and payouts, the wallet pays and
//! receives, and the X402 router plus the account service sell and credit a
//! top-up. The treasury holds no balance of its own.
use crate::Result;
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentRequired,
    ResourceInfo,
    router::{Bound, Lightning, Quote, Router, X402Lightning},
    server::Receiver,
    wire::decode_payment_required,
};
use serde_json::{Map, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};

/// What a wallet observed for one outgoing payment.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Pay {
    Paid { preimage: String, fee_msat: u64 },
    Failed,
    Unknown,
}

/// The agent's wallet. Credentials stay inside the implementation.
pub trait Wallet {
    /// Whether the wallet enforces a hard routing-fee ceiling.
    fn fee_limit(&self) -> bool;
    /// Whether the wallet can report a payment's outcome by hash.
    fn lookup(&self) -> bool;
    fn pay(&mut self, bolt11: &str, max_fee_msat: u64) -> Pay;
    fn status(&mut self, payment_hash: &str) -> Pay;
    /// An inbound payment the wallet has as settled, by its reference.
    fn received(&self, reference: &str) -> Option<u64>;
}

/// The authoritative earnings ledger: a payout that actually went out.
pub trait Earnings {
    /// `(amount_msat, wallet_reference)` of a `sent` payout to `party`.
    fn sent_payout(&self, party: &str, id: &str) -> Result<Option<(u64, String)>>;
}

impl Earnings for pay_ledger::Ledger {
    fn sent_payout(&self, party: &str, id: &str) -> Result<Option<(u64, String)>> {
        let payment = self.earnings_payout(party, id).map_err(|e| e.to_string())?;
        Ok(payment.and_then(|p| {
            (p.state == pay_ledger::PayoutState::Sent)
                .then_some(())
                .and(p.wallet_reference)
                .map(|reference| (p.amount_msat.max(0) as u64, reference))
        }))
    }
}

/// What redeeming a paid top-up produced.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Redeem {
    /// The account service credited the account; exact receipt bytes.
    Credited(Vec<u8>),
    Refused(String),
    /// The payment may have been consumed and credited; the reply was lost.
    Unknown,
}

/// The seller of API credit over X402 HTTP.
pub trait TopUp {
    fn challenge(
        &mut self,
        url: &str,
        body: &[u8],
        amount_msat: u64,
        now: u64,
    ) -> Result<PaymentRequired>;
    fn redeem(
        &mut self,
        url: &str,
        body: &[u8],
        signature: &str,
        amount_msat: u64,
        now: u64,
    ) -> Redeem;
    /// Reconciliation: the credit receipt already issued for a payment hash.
    fn credited(&mut self, account: &str, payment_hash: &str) -> Option<Vec<u8>>;
}

/// The account service: credits an API account once per settled payment
/// and meters use against it.
#[derive(Default)]
pub struct Accounts {
    credits: BTreeMap<String, (String, u64, Vec<u8>)>,
    balances: BTreeMap<String, u64>,
    /// Fault: the credit acknowledgment is lost after crediting.
    pub lose_acknowledgment: bool,
}

impl Accounts {
    /// Credit once per payment hash. A retry returns the original receipt.
    pub fn credit(&mut self, account: &str, payment_hash: &str, msat: u64) -> Vec<u8> {
        if let Some((_, _, receipt)) = self.credits.get(payment_hash) {
            return receipt.clone();
        }
        let receipt = serde_json::to_vec(
            &json!({"account":account,"payment_hash":payment_hash,"credited_msat":msat}),
        )
        .unwrap_or_default();
        self.credits
            .insert(payment_hash.into(), (account.into(), msat, receipt.clone()));
        *self.balances.entry(account.into()).or_default() += msat;
        receipt
    }
    pub fn balance(&self, account: &str) -> u64 {
        self.balances.get(account).copied().unwrap_or(0)
    }
    /// Metered use draws the credited balance down.
    pub fn meter(&mut self, account: &str, msat: u64) -> Result<u64> {
        let balance = self.balances.entry(account.into()).or_default();
        if *balance < msat {
            return Err("limit_exceeded: account credit is insufficient".into());
        }
        *balance -= msat;
        Ok(*balance)
    }
    fn receipt(&self, account: &str, payment_hash: &str) -> Option<Vec<u8>> {
        self.credits
            .get(payment_hash)
            .filter(|(a, _, _)| a == account)
            .map(|(_, _, r)| r.clone())
    }
}

/// A top-up resource served by the existing X402 router, crediting the
/// account named in the request body.
pub struct RouterTopUp {
    router: Router<FileReplayStore>,
    accounts: Arc<Mutex<Accounts>>,
}

impl RouterTopUp {
    pub fn new(
        receiver: Arc<dyn Receiver>,
        replay: &std::path::Path,
        network: &'static str,
        accounts: Arc<Mutex<Accounts>>,
    ) -> Result<Self> {
        let lightning = Lightning::new(
            receiver,
            Facilitator::new(
                FileReplayStore::open(replay).map_err(|e| e.to_string())?,
                60,
            ),
            network,
            300,
        );
        Ok(Self {
            router: Router::new(lightning, vec![Box::new(X402Lightning)])?,
            accounts,
        })
    }
    fn hash(url: &str, body: &[u8]) -> Result<String> {
        nostr::x402::binding_hash(
            &nostr::x402::http_binding("POST", url, body, &[]).map_err(|e| format!("{e:?}"))?,
        )
        .map_err(|e| format!("{e:?}"))
    }
}

impl TopUp for RouterTopUp {
    fn challenge(
        &mut self,
        url: &str,
        body: &[u8],
        amount_msat: u64,
        now: u64,
    ) -> Result<PaymentRequired> {
        let hash = Self::hash(url, body)?;
        let bound = Bound {
            http_method: "POST",
            url,
            body,
            request_hash: &hash,
            headers: &[],
        };
        let resource = ResourceInfo {
            url: url.into(),
            description: Some("API account credit".into()),
            mime_type: Some("application/json".into()),
            rest: Map::new(),
        };
        let challenged = self
            .router
            .challenge(
                &bound,
                Quote {
                    amount_msat,
                    usd_micros: 0,
                },
                &resource,
                None,
                None,
                now,
            )
            .map_err(|e| format!("unavailable: {e:?}"))?;
        let header = challenged
            .headers
            .iter()
            .find(|(n, _)| n.eq_ignore_ascii_case(PAYMENT_REQUIRED))
            .map(|(_, v)| v.clone())
            .ok_or("unavailable: no x402 challenge")?;
        decode_payment_required(&header).map_err(|e| format!("malformed: {e}"))
    }
    fn redeem(
        &mut self,
        url: &str,
        body: &[u8],
        signature: &str,
        amount_msat: u64,
        now: u64,
    ) -> Redeem {
        let Ok(hash) = Self::hash(url, body) else {
            return Redeem::Refused("malformed".into());
        };
        let headers = vec![(PAYMENT_SIGNATURE.to_string(), signature.to_string())];
        let bound = Bound {
            http_method: "POST",
            url,
            body,
            request_hash: &hash,
            headers: &headers,
        };
        match self.router.settle(
            &bound,
            Quote {
                amount_msat,
                usd_micros: 0,
            },
            "topup",
            now,
        ) {
            Some(Ok(settled)) => {
                let account = serde_json::from_slice::<serde_json::Value>(body)
                    .ok()
                    .and_then(|v| v["account"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let payment_hash = settled.payment_hash.unwrap_or(settled.replay_key);
                let mut accounts = self.accounts.lock().expect("accounts");
                let receipt = accounts.credit(&account, &payment_hash, amount_msat);
                if accounts.lose_acknowledgment {
                    return Redeem::Unknown;
                }
                Redeem::Credited(receipt)
            }
            Some(Err(refused)) => Redeem::Refused(refused.reason),
            None => Redeem::Refused("no_credential".into()),
        }
    }
    fn credited(&mut self, account: &str, payment_hash: &str) -> Option<Vec<u8>> {
        self.accounts.lock().ok()?.receipt(account, payment_hash)
    }
}
