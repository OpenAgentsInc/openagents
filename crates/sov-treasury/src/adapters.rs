//! The existing owners the treasury drives. Money stays in them: the worker
//! ledger (`pay-ledger`) records earnings and payouts, the wallet pays and
//! receives, the X402 router sells a top-up, and the account-credit service
//! credits it from the pay host's signed callback. The treasury holds no
//! balance of its own.
use crate::Result;
use nostr::domain::RelaySigner;
use openagents_x402::{
    Facilitator, FileReplayStore, PAYMENT_REQUIRED, PAYMENT_SIGNATURE, PaymentRequired,
    ResourceInfo,
    hosted::{Paid, paid_header, verify_paid},
    router::{Bound, Lightning, Quote, Router, X402Lightning},
    server::Receiver,
    wire::decode_payment_required,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value, json};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::Write,
    path::PathBuf,
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

/// The authoritative earnings ledger.
pub trait Earnings {
    /// `(amount_msat, wallet_reference)` of a `sent` payout to `party`.
    fn sent_payout(&self, party: &str, id: &str) -> Result<Option<(u64, String)>>;
    /// The party's earnings and payouts by their original identities.
    fn statement(&mut self, party: &str) -> Result<Value>;
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
    fn statement(&mut self, party: &str) -> Result<Value> {
        let statement = self
            .earnings_statement(party, 0, 0, pay_ledger::earnings::MAX_PAGE)
            .map_err(|e| e.to_string())?;
        serde_json::to_value(&statement).map_err(|e| e.to_string())
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
    /// Reconciliation: the credit receipt already issued for a payment hash,
    /// retrying the pay host's original signed callback when it was lost.
    fn credited(&mut self, account: &str, payment_hash: &str, now: u64) -> Option<Vec<u8>>;
    /// Metered usage recorded against the account, by request ID.
    fn usage(&mut self, account: &str) -> Vec<Value>;
}

/// One credit line, one per payment hash, as the gateway journals it.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct CreditLine {
    payment: String,
    account: String,
    micros: u64,
    callback: String,
    at: u64,
}

/// The account-credit service: the gateway's `POST /v1/agents/credit`
/// contract in-process. It accepts only the pay host's signed paid header
/// for the configured resource, credits each payment hash once in a durable
/// journal, refuses revoked or unknown accounts, and meters use by holding
/// each request's worst case in USD micros. Credit units are not msat.
pub struct CreditService {
    pay_host: String,
    resource: String,
    upstream: String,
    credit_usd_micros: u64,
    journal: PathBuf,
    paid: BTreeMap<String, CreditLine>,
    balances: BTreeMap<String, u64>,
    usage: BTreeMap<String, Vec<Value>>,
    agents: BTreeSet<String>,
    revoked: BTreeSet<String>,
    /// Fault: the callback's reply is lost after the journal write.
    pub lose_reply: bool,
}

impl CreditService {
    pub fn open(
        journal: PathBuf,
        pay_host: &str,
        resource: &str,
        upstream: &str,
        credit_usd_micros: u64,
    ) -> Result<Self> {
        let mut paid = BTreeMap::new();
        let mut balances: BTreeMap<String, u64> = BTreeMap::new();
        if let Ok(text) = std::fs::read_to_string(&journal) {
            for line in text.lines().filter(|l| !l.trim().is_empty()) {
                let credit: CreditLine = serde_json::from_str(line).map_err(|e| e.to_string())?;
                *balances.entry(credit.account.clone()).or_default() += credit.micros;
                paid.insert(credit.payment.clone(), credit);
            }
        }
        Ok(Self {
            pay_host: pay_host.into(),
            resource: resource.into(),
            upstream: upstream.into(),
            credit_usd_micros,
            journal,
            paid,
            balances,
            usage: BTreeMap::new(),
            agents: BTreeSet::new(),
            revoked: BTreeSet::new(),
            lose_reply: false,
        })
    }
    pub fn register(&mut self, account: &str) {
        self.agents.insert(account.into());
    }
    pub fn revoke(&mut self, account: &str) {
        self.revoked.insert(account.into());
    }
    pub fn balance(&self, account: &str) -> u64 {
        self.balances.get(account).copied().unwrap_or(0)
    }
    fn receipt(line: &CreditLine, already: bool) -> Vec<u8> {
        serde_json::to_vec(&json!({"account":line.account,"payment_hash":line.payment,
            "credited_usd_micros":line.micros,"callback":line.callback,"already_credited":already}))
        .unwrap_or_default()
    }
    /// The signed callback. A payment hash credits once; a retry of the same
    /// callback returns the original credit.
    pub fn credit(
        &mut self,
        header: &str,
        account: &str,
        body: &[u8],
        now: u64,
    ) -> std::result::Result<Vec<u8>, String> {
        let url = format!("{}?account={account}", self.upstream);
        let paid = verify_paid(header, &self.pay_host, "POST", &url, body, now)?;
        if paid.resource != self.resource || paid.payment_hash.is_empty() {
            return Err("invalid_payment: that payment was for something else".into());
        }
        if let Some(line) = self.paid.get(&paid.payment_hash) {
            if line.account != account {
                return Err("conflict: that payment credited another account".into());
            }
            return Ok(Self::receipt(line, true));
        }
        if !self.agents.contains(account) {
            return Err("not_an_agent: no agent has that id".into());
        }
        if self.revoked.contains(account) {
            return Err("revoked: the account was revoked".into());
        }
        let line = CreditLine {
            payment: paid.payment_hash.clone(),
            account: account.into(),
            micros: self.credit_usd_micros,
            callback: paid.id,
            at: now,
        };
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.journal)
            .map_err(|e| e.to_string())?;
        file.write_all(
            format!(
                "{}\n",
                serde_json::to_string(&line).map_err(|e| e.to_string())?
            )
            .as_bytes(),
        )
        .and_then(|()| file.sync_all())
        .map_err(|e| e.to_string())?;
        *self.balances.entry(account.into()).or_default() += line.micros;
        let receipt = Self::receipt(&line, false);
        self.paid.insert(line.payment.clone(), line);
        Ok(receipt)
    }
    /// Meter one request: hold its worst case, then charge what it used.
    pub fn meter(
        &mut self,
        account: &str,
        request: &str,
        worst_usd_micros: u64,
        used_usd_micros: u64,
    ) -> Result<u64> {
        if self.revoked.contains(account) {
            return Err("revoked: the account was revoked".into());
        }
        if self
            .usage
            .get(account)
            .is_some_and(|u| u.iter().any(|r| r["request"] == request))
        {
            return Ok(self.balance(account));
        }
        let balance = self.balances.entry(account.into()).or_default();
        if *balance < worst_usd_micros || used_usd_micros > worst_usd_micros {
            return Err("limit_exceeded: the account's credit cannot cover this request".into());
        }
        *balance -= used_usd_micros;
        let left = *balance;
        self.usage
            .entry(account.into())
            .or_default()
            .push(json!({"request":request,"usd_micros":used_usd_micros}));
        Ok(left)
    }
    fn existing(&self, account: &str, payment_hash: &str) -> Option<Vec<u8>> {
        self.paid
            .get(payment_hash)
            .filter(|l| l.account == account)
            .map(|l| Self::receipt(l, true))
    }
}

/// A top-up resource served by the existing X402 router. After settlement
/// the pay host signs the paid header and calls the credit service, exactly
/// as the hosted front forwards to the gateway.
pub struct RouterTopUp {
    router: Router<FileReplayStore>,
    pay_host: RelaySigner,
    resource: String,
    credits: Arc<Mutex<CreditService>>,
    callbacks: BTreeMap<String, (String, Paid, Vec<u8>)>,
}

impl RouterTopUp {
    pub fn new(
        receiver: Arc<dyn Receiver>,
        replay: &std::path::Path,
        network: &'static str,
        pay_host: RelaySigner,
        resource: &str,
        credits: Arc<Mutex<CreditService>>,
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
            pay_host,
            resource: resource.into(),
            credits,
            callbacks: BTreeMap::new(),
        })
    }
    fn hash(url: &str, body: &[u8]) -> Result<String> {
        nostr::x402::binding_hash(
            &nostr::x402::http_binding("POST", url, body, &[]).map_err(|e| format!("{e:?}"))?,
        )
        .map_err(|e| format!("{e:?}"))
    }
    /// Deliver the pay host's signed callback for one settled payment. A
    /// retry signs a fresh header for the same payment hash; the credit
    /// service credits that hash once.
    fn callback(&mut self, payment_hash: &str, now: u64) -> Redeem {
        let Some((account, paid, body)) = self.callbacks.get(payment_hash).cloned() else {
            return Redeem::Refused("no_callback".into());
        };
        let mut credits = self.credits.lock().expect("credits");
        let upstream = format!("{}?account={account}", credits.upstream);
        let header = paid_header(&self.pay_host, &paid, "POST", &upstream, &body, now);
        match credits.credit(&header, &account, &body, now) {
            Ok(receipt) if !credits.lose_reply => Redeem::Credited(receipt),
            Ok(_) => Redeem::Unknown,
            Err(why) => Redeem::Refused(why),
        }
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
                let account = serde_json::from_slice::<Value>(body)
                    .ok()
                    .and_then(|v| v["account"].as_str().map(str::to_string))
                    .unwrap_or_default();
                let payment_hash = settled.payment_hash.unwrap_or(settled.replay_key);
                let paid = Paid {
                    resource: self.resource.clone(),
                    request_hash: hash.clone(),
                    payment_hash: payment_hash.clone(),
                    settled_at: now,
                    id: String::new(),
                };
                // Keep the settled payment's identity so a lost callback is
                // retried for the same payment hash, never a new one.
                self.callbacks
                    .insert(payment_hash.clone(), (account, paid, body.to_vec()));
                self.callback(&payment_hash, now)
            }
            Some(Err(refused)) => Redeem::Refused(refused.reason),
            None => Redeem::Refused("no_credential".into()),
        }
    }
    fn credited(&mut self, account: &str, payment_hash: &str, now: u64) -> Option<Vec<u8>> {
        if let Some(receipt) = self.credits.lock().ok()?.existing(account, payment_hash) {
            return Some(receipt);
        }
        match self.callback(payment_hash, now) {
            Redeem::Credited(receipt) => Some(receipt),
            _ => None,
        }
    }
    fn usage(&mut self, account: &str) -> Vec<Value> {
        self.credits
            .lock()
            .map(|c| c.usage.get(account).cloned().unwrap_or_default())
            .unwrap_or_default()
    }
}
