//! Agent spending, phase 1, on the phone: an agent on one of the owner's
//! computers asks, and the owner approves each payment here
//! (`docs/breez/spend-protocol.md`, [#9863]).
//!
//! While the app is open, the phone reads each connected computer's spend
//! requests with NIP-HOST `spend.list`, which also hands the computer the
//! phone's `request`-mode spend grant for it. A request the grant or the
//! ledger refuses is answered with its refusal code at once; refusing moves
//! nothing. Every other request waits on the approval sheet, which shows
//! what the phone decoded from the invoice itself: the payee, the amount,
//! and the wallet's fee quote, beside the computer, task, purpose, and what
//! the grant has left. Only the owner's **Approve** pays, once, with an
//! idempotency key derived from the request ID; **Deny** refuses it as
//! `declined_by_owner`. The receipt goes back with `spend.settle` until the
//! computer records it.
//!
//! The ledger (`coder_access::spend::Ledger`) reserves the amount plus the
//! fee ceiling before the wallet is called and keeps pending payments
//! reserved. It and the grants are kept in the app's encrypted store, so a
//! restart never resets a period.
//!
//! [#9863]: https://github.com/OpenAgentsInc/openagents/issues/9863

use crate::wallet::{AgentPayFailure, InvoicePayment, Node};
use coder_computers::cache::Cache;
use coder_computers::live::Terminals;
use coder_host::access::protocol::{Operation, Outcome};
use coder_host::access::spend::{
    Admitted, Earlier, Entry, Grant, Ledger, Mode, RECEIPT, Receipt, Refusal, Remaining,
    Settlement, SpendRequest, State, defaults, hex,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// How often the phone reads a computer's requests while the app is open.
const POLL_EVERY: Duration = Duration::from_secs(10);
/// How long a computer that could not be read is left alone.
const BACKOFF: Duration = Duration::from_secs(30);
/// Above this amount (1,000 sats) Approve asks for Face ID or the passcode.
pub const AUTHENTICATE_ABOVE_MSAT: u64 = 1_000_000;
/// The most ledger entries the phone keeps; the oldest settled go first.
const LEDGER_KEEP: usize = 300;
/// How many ledger entries the Wallet tab lists.
const HISTORY: usize = 20;
/// A grant is renewed this long before it expires.
const RENEW_BEFORE: u64 = 24 * 60 * 60;
/// How many replaced grants' requests the phone still accepts.
const LINEAGE: usize = 8;

/// What agent spending needs from the wallet: a fee quote and one payment
/// of a BOLT11 invoice. [`NodePayer`] is the running Spark wallet's.
pub trait Payer: Send + Sync {
    /// The fee, in sats, to pay `invoice` now.
    fn invoice_fee(&self, invoice: &str) -> Result<u64, String>;
    /// Pay `invoice` for at most `max_fee_sats`, once per `idempotency_key`.
    fn pay_invoice(
        &self,
        invoice: &str,
        max_fee_sats: u64,
        idempotency_key: &str,
    ) -> Result<InvoicePayment, AgentPayFailure>;
}

/// The Wallet tab's running wallet as the payer.
pub struct NodePayer(pub Arc<dyn Node>);

impl Payer for NodePayer {
    fn invoice_fee(&self, invoice: &str) -> Result<u64, String> {
        self.0.invoice_fee(invoice)
    }
    fn pay_invoice(
        &self,
        invoice: &str,
        max_fee_sats: u64,
        idempotency_key: &str,
    ) -> Result<InvoicePayment, AgentPayFailure> {
        self.0.pay_invoice(invoice, max_fee_sats, idempotency_key)
    }
}

/// Why a receipt did not reach a computer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SettleError {
    /// The computer answered and will not record it: it holds another final
    /// answer, or the request is not one it knows. Sending again won't help.
    Refused(String),
    /// The computer could not be reached; send again later.
    Unreachable(String),
}

/// How the phone reaches a computer's spend requests.
pub trait Transport: Send + Sync {
    /// `spend.list`: hand `host` the phone's grant and read its requests.
    fn list(&self, host: &str, grant: &Grant) -> Result<Vec<Entry>, String>;
    /// `spend.settle`: record a receipt; returns the one the host recorded.
    fn settle(&self, host: &str, receipt: &Receipt) -> Result<Receipt, SettleError>;
}

/// The live transport: the Computers service's current link to each host.
pub struct Live {
    terminals: Terminals,
    handle: tokio::runtime::Handle,
}

impl Live {
    pub fn new(terminals: Terminals, handle: tokio::runtime::Handle) -> Self {
        Self { terminals, handle }
    }

    fn call(&self, host: &str, op: Operation) -> Result<Outcome, coder_host::Error> {
        let link = (self.terminals.links(host))().map_err(coder_host::Error::Access)?;
        self.handle.block_on(link.call(op))
    }
}

impl Transport for Live {
    fn list(&self, host: &str, grant: &Grant) -> Result<Vec<Entry>, String> {
        match self.call(
            host,
            Operation::ListSpends {
                grant: Box::new(grant.clone()),
            },
        ) {
            Ok(Outcome::Spends { spends }) => Ok(spends),
            Ok(_) => Err("the computer did not list its requests".into()),
            Err(error) => Err(error.to_string()),
        }
    }

    fn settle(&self, host: &str, receipt: &Receipt) -> Result<Receipt, SettleError> {
        use coder_host::access::Code;
        match self.call(
            host,
            Operation::SettleSpend {
                receipt: Box::new(receipt.clone()),
            },
        ) {
            Ok(Outcome::Settled { receipt }) => Ok(*receipt),
            Ok(_) => Err(SettleError::Unreachable(
                "the computer did not record the receipt".into(),
            )),
            Err(coder_host::Error::Access(error))
                if matches!(
                    error.code,
                    Code::Conflict | Code::Forbidden | Code::Malformed
                ) =>
            {
                Err(SettleError::Refused(error.to_string()))
            }
            Err(error) => Err(SettleError::Unreachable(error.to_string())),
        }
    }
}

/// The phone's grant for one computer.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct HostGrant {
    grant: Grant,
    /// The owner stopped this computer's requests. No grant is handed out
    /// until the owner allows it again.
    blocked: bool,
    /// The computer has not been told of the block yet.
    #[serde(default)]
    unannounced: bool,
    label: String,
    /// Grants at this epoch the current one replaced, newest last: requests
    /// made under them are still checked against the current grant.
    #[serde(default)]
    accepted: Vec<String>,
    /// Of those, the ones replaced by a setting change, whose payments count
    /// against the current grant's total. A renewal starts a new total.
    #[serde(default)]
    counted: Vec<String>,
}

impl HostGrant {
    fn earlier(&self) -> Earlier<'_> {
        Earlier {
            accepted: &self.accepted,
            counted: &self.counted,
        }
    }

    /// Replace the grant with `next`, remembering the old one's requests,
    /// and, for a setting change, its payments.
    fn replace(&mut self, next: Grant, carry_total: bool) {
        let old = std::mem::replace(&mut self.grant, next);
        if old.epoch != self.grant.epoch {
            self.accepted.clear();
            self.counted.clear();
            return;
        }
        self.accepted.push(old.grant.clone());
        if carry_total {
            self.counted.push(old.grant);
        } else {
            self.counted.clear();
        }
        let excess = self.accepted.len().saturating_sub(LINEAGE);
        self.accepted.drain(..excess);
        let excess = self.counted.len().saturating_sub(LINEAGE);
        self.counted.drain(..excess);
    }
}

/// What the phone keeps across launches.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
struct Saved {
    grants: BTreeMap<String, HostGrant>,
    ledger: Ledger,
    /// The final receipt of each entry the host has not recorded yet.
    #[serde(default)]
    unsent: BTreeMap<String, (String, Receipt)>,
}

/// A request on the approval sheet.
#[derive(Clone, Debug)]
struct Waiting {
    host: String,
    request: SpendRequest,
    admitted: Admitted,
    /// The wallet's fee quote in sats, once read.
    fee: Option<Result<u64, String>>,
}

struct Shared {
    saved: Saved,
    waiting: BTreeMap<String, Waiting>,
    /// The request being paid.
    busy: Option<String>,
    notice: Option<String>,
    polling: bool,
    last_poll: Option<Instant>,
    backoff: BTreeMap<String, Instant>,
    /// Pending payments to ask the wallet about on this pass.
    recheck: Vec<(String, SpendRequest)>,
    /// Requests a standing grant pays without a tap, on this pass.
    automatic: Vec<(String, SpendRequest)>,
    /// How amounts are shown; the app's choice.
    format: crate::amounts::Format,
}

/// Agent spending on the phone.
pub struct Spending {
    device: String,
    store: Option<Arc<Cache>>,
    shared: Arc<Mutex<Shared>>,
    clock: fn() -> u64,
}

/// The approval sheet.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Sheet {
    pub request: String,
    pub host: String,
    pub computer: String,
    pub task: Option<String>,
    pub title: Option<String>,
    pub purpose: &'static str,
    /// The payee as decoded from the invoice: its node key, shortened.
    pub payee: String,
    pub payee_new: bool,
    /// The invoice's own description.
    pub description: Option<String>,
    /// The agent's note: its words, not the payee.
    pub note: Option<String>,
    pub resource: Option<String>,
    pub amount: String,
    pub amount_msat: u64,
    pub fee: String,
    pub fee_ceiling: String,
    pub remaining: String,
    pub expires_at: u64,
    /// Approve asks for Face ID or the passcode first.
    pub authenticate: bool,
    /// The fee quote is in and fits: Approve can pay.
    pub ready: bool,
    /// The owner may approve and trust this payee: later payments to it
    /// from this computer, within the automatic ceilings, need no tap.
    pub can_trust: bool,
}

/// One computer that may ask.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ComputerRow {
    pub host: String,
    pub computer: String,
    pub blocked: bool,
    pub remaining: String,
    /// What this computer's agents are paid without a tap, when anything.
    pub automatic: Option<String>,
    /// Payees paid without a tap, with their daily ceilings.
    pub trusted: Vec<TrustedRow>,
}

/// A payee a standing grant pays without a tap.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct TrustedRow {
    /// The node key, for `spend_untrust`.
    pub payee: String,
    /// The key shortened, for the screen.
    pub label: String,
    /// Its ceiling in the window.
    pub limit: String,
}

/// One ledger entry, tagged with the computer and task that asked.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct HistoryRow {
    pub request: String,
    pub computer: String,
    pub title: Option<String>,
    pub purpose: &'static str,
    pub amount: String,
    pub fee: Option<String>,
    /// `paid`, `pending`, `paying`, or `refused`.
    pub state: &'static str,
    /// Paid without the owner's tap, under a standing grant.
    pub auto: bool,
    pub detail: Option<String>,
    pub at: u64,
}

/// What the app packet carries.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct View {
    pub sheet: Option<Sheet>,
    /// Requests waiting behind the sheet.
    pub waiting: usize,
    pub busy: bool,
    pub notice: Option<String>,
    pub computers: Vec<ComputerRow>,
    pub history: Vec<HistoryRow>,
}

/// An amount in msat, in the app's format when it is whole base units.
pub fn amount(msat: u64, format: crate::amounts::Format) -> String {
    format.show_msat(msat)
}

fn remaining_text(remaining: Remaining, format: crate::amounts::Format) -> String {
    format!(
        "{} left today, {} in all",
        amount(remaining.period_msat, format),
        amount(remaining.total_msat, format)
    )
}

fn short(key: &str) -> String {
    if key.len() > 16 {
        format!("{}…{}", &key[..8], &key[key.len() - 8..])
    } else {
        key.to_owned()
    }
}

/// The wallet's idempotency key for a request: a UUID made from its ID, so
/// a retry pays the same payment.
fn idempotency_key(request: &str) -> String {
    let mut bytes = [0_u8; 16];
    for (index, slot) in bytes.iter_mut().enumerate() {
        *slot = u8::from_str_radix(request.get(index * 2..index * 2 + 2).unwrap_or("00"), 16)
            .unwrap_or_default();
    }
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}

fn random_id() -> String {
    use secp256k1::rand::RngCore;
    let mut bytes = [0_u8; 32];
    secp256k1::rand::rng().fill_bytes(&mut bytes);
    hex(&bytes)
}

fn lock(shared: &Mutex<Shared>) -> MutexGuard<'_, Shared> {
    shared.lock().unwrap_or_else(|poison| poison.into_inner())
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs())
}

impl Spending {
    pub fn new(device: String, store: Option<Cache>) -> Self {
        let saved = store
            .as_ref()
            .and_then(|cache| cache.read::<Saved>("spend").ok().flatten())
            .unwrap_or_default();
        Self {
            device,
            store: store.map(Arc::new),
            shared: Arc::new(Mutex::new(Shared {
                saved,
                waiting: BTreeMap::new(),
                busy: None,
                notice: None,
                polling: false,
                last_poll: None,
                backoff: BTreeMap::new(),
                recheck: vec![],
                automatic: vec![],
                format: crate::amounts::Format::default(),
            })),
            clock: now,
        }
    }

    #[cfg(test)]
    pub(crate) fn at(mut self, clock: fn() -> u64) -> Self {
        self.clock = clock;
        self
    }

    fn lock(&self) -> MutexGuard<'_, Shared> {
        lock(&self.shared)
    }

    /// Read the computers' requests on the next pass, without waiting for
    /// [`POLL_EVERY`]: the app came to the foreground, perhaps from a wake.
    pub fn soon(&self) {
        let mut shared = self.lock();
        shared.last_poll = None;
        shared.backoff.clear();
    }

    /// Show amounts in `format` from now on.
    pub fn set_format(&self, format: crate::amounts::Format) {
        self.lock().format = format;
    }

    /// Read the connected computers' requests in the background, at most
    /// every [`POLL_EVERY`]. `hosts` are the computers this phone may
    /// operate, with their labels.
    pub fn poll(
        &self,
        hosts: Vec<(String, String)>,
        transport: Arc<dyn Transport>,
        node: Option<Arc<dyn Payer>>,
    ) {
        {
            let mut shared = self.lock();
            if shared.polling || shared.last_poll.is_some_and(|at| at.elapsed() < POLL_EVERY) {
                return;
            }
            shared.polling = true;
            shared.last_poll = Some(Instant::now());
        }
        let worker = self.worker();
        std::thread::spawn(move || {
            worker.poll_now(&hosts, transport.as_ref(), node.as_deref());
            lock(&worker.shared).polling = false;
        });
    }

    fn worker(&self) -> Spending {
        Spending {
            device: self.device.clone(),
            store: self.store.clone(),
            shared: self.shared.clone(),
            clock: self.clock,
        }
    }

    fn save(&self, shared: &mut Shared) {
        shared.saved.ledger.trim(LEDGER_KEEP);
        if let Some(store) = &self.store {
            let _ = store.write("spend", &shared.saved);
        }
    }

    /// One pass over `hosts`: hand each its grant, read its requests, answer
    /// what the checks refuse, and put the rest on the sheet. Blocking.
    pub(crate) fn poll_now(
        &self,
        hosts: &[(String, String)],
        transport: &dyn Transport,
        node: Option<&dyn Payer>,
    ) {
        let now = (self.clock)();
        self.deliver(transport);
        for (host, label) in hosts {
            let grant = {
                let mut shared = self.lock();
                if shared
                    .backoff
                    .get(host)
                    .is_some_and(|until| Instant::now() < *until)
                {
                    continue;
                }
                self.grant_for(&mut shared, host, label, now)
            };
            let Some(grant) = grant else {
                continue;
            };
            let listed = match transport.list(host, &grant) {
                Ok(listed) => listed,
                Err(_) => {
                    self.lock()
                        .backoff
                        .insert(host.clone(), Instant::now() + BACKOFF);
                    continue;
                }
            };
            let mut shared = self.lock();
            shared.backoff.remove(host);
            if let Some(held) = shared.saved.grants.get_mut(host)
                && held.blocked
            {
                held.unannounced = false;
                self.save(&mut shared);
                continue;
            }
            self.take(&mut shared, host, listed, now);
        }
        self.quote(node);
        self.pay_automatic(node);
        self.recheck(node);
        self.deliver(transport);
    }

    /// Pay what the standing grants admit without a tap, once each: check
    /// again under the lock, quote the fee, reserve as automatic, and pay.
    /// A quote above the request's fee ceiling, or a check that no longer
    /// admits it, sends the request to the approval sheet instead.
    fn pay_automatic(&self, node: Option<&dyn Payer>) {
        let queued = std::mem::take(&mut self.lock().automatic);
        let Some(node) = node else {
            // The wallet is still starting; the next pass finds them again.
            return;
        };
        for (host, request) in queued {
            let fee = node.invoice_fee(&request.payment);
            let now = (self.clock)();
            let mut shared = self.lock();
            if shared.busy.is_some() {
                continue;
            }
            let Some(held) = shared
                .saved
                .grants
                .get(&host)
                .filter(|held| !held.blocked)
                .cloned()
            else {
                continue;
            };
            let admitted =
                match shared
                    .saved
                    .ledger
                    .check_in(Some(&held.grant), held.earlier(), &request, now)
                {
                    Ok(admitted) => admitted,
                    Err(code) => {
                        refuse(&mut shared, &host, &request, code, now);
                        self.save(&mut shared);
                        continue;
                    }
                };
            let fits = matches!(fee, Ok(fee) if fee.saturating_mul(1000) <= request.fee_max_msat);
            if !fits
                || !shared
                    .saved
                    .ledger
                    .automatic(&held.grant, &request, &admitted, now)
            {
                shared.waiting.insert(
                    request.request.clone(),
                    Waiting {
                        host,
                        request,
                        admitted,
                        fee: Some(fee),
                    },
                );
                continue;
            }
            if shared
                .saved
                .ledger
                .reserve_in(Some(&held.grant), held.earlier(), &request, now, true)
                .is_err()
            {
                continue;
            }
            shared.busy = Some(request.request.clone());
            self.save(&mut shared);
            drop(shared);
            self.pay(&host, &request, node);
        }
    }

    /// Ask the wallet again about payments it reported pending. The same
    /// idempotency key returns the same payment, never a second one.
    fn recheck(&self, node: Option<&dyn Payer>) {
        let pending = std::mem::take(&mut self.lock().recheck);
        let Some(node) = node else {
            return;
        };
        for (host, request) in pending {
            {
                let mut shared = self.lock();
                if shared.busy.is_some() {
                    continue;
                }
                shared.busy = Some(request.request.clone());
            }
            self.pay(&host, &request, node);
        }
    }

    /// The grant to hand `host`: its current one, a new or renewed one, or
    /// for a blocked computer that was not told yet, the next epoch's with
    /// no life left, which revokes. `None` when there is nothing to send.
    fn grant_for(&self, shared: &mut Shared, host: &str, label: &str, now: u64) -> Option<Grant> {
        let device = self.device.clone();
        let held = shared.saved.grants.get_mut(host);
        let grant = match held {
            Some(held) if held.blocked => {
                if !held.unannounced {
                    return None;
                }
                let mut revoking = held.grant.clone();
                revoking.grant = random_id();
                // Expired a minute ago, so a computer whose clock runs a
                // little behind still reads it as over.
                revoking.issued_at = now.saturating_sub(120);
                revoking.expires_at = now.saturating_sub(60);
                return Some(revoking);
            }
            Some(held) if held.grant.expires_at > now + RENEW_BEFORE => {
                held.label = label.to_owned();
                return Some(held.grant.clone());
            }
            // Renewed with the same settings; what waits under the old one
            // is still answered, and the total starts again.
            Some(held) => {
                let mut next = held.grant.clone();
                next.grant = random_id();
                next.issued_at = now;
                next.expires_at = now + defaults::LIFETIME;
                held.replace(next.clone(), false);
                held.label = label.to_owned();
                self.save(shared);
                return Some(next);
            }
            None => Grant::request_mode(random_id(), &device, host, 0, now),
        };
        shared.saved.grants.insert(
            host.to_owned(),
            HostGrant {
                grant: grant.clone(),
                blocked: false,
                unannounced: false,
                label: label.to_owned(),
                accepted: vec![],
                counted: vec![],
            },
        );
        self.save(shared);
        Some(grant)
    }

    /// Sort what `host` listed: refuse, resend, or wait for the owner.
    fn take(&self, shared: &mut Shared, host: &str, listed: Vec<Entry>, now: u64) {
        let held = shared
            .saved
            .grants
            .get(host)
            .filter(|held| !held.blocked)
            .cloned();
        let grant = held.as_ref().map(|held| held.grant.clone());
        let earlier = held.as_ref().map(HostGrant::earlier).unwrap_or_default();
        let mut listed_ids = vec![];
        let mut changed = false;
        for entry in listed {
            let request = entry.request;
            if request.grantee != host {
                continue;
            }
            listed_ids.push(request.request.clone());
            if let Some(known) = shared.saved.ledger.entries.get(&request.request) {
                // Pending in the wallet: ask it again on this pass.
                if known.state == State::Pending
                    && shared.busy.as_deref() != Some(request.request.as_str())
                {
                    shared.recheck.push((host.to_owned(), request.clone()));
                }
                // Final and not recorded by the host: send it again.
                if matches!(known.state, State::Paid | State::Refused)
                    && !shared.saved.unsent.contains_key(&request.request)
                    && entry.receipt.as_ref().is_none_or(|r| !r.is_final())
                    && let Some(receipt) = receipt_for(&shared.saved.ledger, &request.request)
                {
                    shared
                        .saved
                        .unsent
                        .insert(request.request.clone(), (host.to_owned(), receipt));
                    changed = true;
                }
                continue;
            }
            if shared.waiting.contains_key(&request.request)
                || shared
                    .automatic
                    .iter()
                    .any(|(_, queued)| queued.request == request.request)
            {
                continue;
            }
            match shared
                .saved
                .ledger
                .check_in(grant.as_ref(), earlier, &request, now)
            {
                Ok(admitted)
                    if grant.as_ref().is_some_and(|grant| {
                        shared
                            .saved
                            .ledger
                            .automatic(grant, &request, &admitted, now)
                    }) =>
                {
                    shared.automatic.push((host.to_owned(), request));
                }
                Ok(admitted) => {
                    shared.waiting.insert(
                        request.request.clone(),
                        Waiting {
                            host: host.to_owned(),
                            request,
                            admitted,
                            fee: None,
                        },
                    );
                }
                Err(code) => {
                    refuse(shared, host, &request, code, now);
                    changed = true;
                }
            }
        }
        // What the host no longer lists was answered or expired there.
        let busy = shared.busy.clone();
        shared.waiting.retain(|id, waiting| {
            waiting.host != host || listed_ids.contains(id) || busy.as_deref() == Some(id)
        });
        // What expired on the sheet is refused.
        let expired: Vec<Waiting> = shared
            .waiting
            .values()
            .filter(|w| now >= w.request.expires_at && busy.as_deref() != Some(&w.request.request))
            .cloned()
            .collect();
        for waiting in expired {
            shared.waiting.remove(&waiting.request.request);
            refuse(
                shared,
                &waiting.host,
                &waiting.request,
                Refusal::Expired,
                now,
            );
            changed = true;
        }
        if changed {
            self.save(shared);
        }
    }

    /// Read the wallet's fee quote for each request on the sheet.
    fn quote(&self, node: Option<&dyn Payer>) {
        let Some(node) = node else {
            return;
        };
        let unquoted: Vec<(String, String)> = self
            .lock()
            .waiting
            .values()
            .filter(|w| w.fee.is_none())
            .map(|w| (w.request.request.clone(), w.request.payment.clone()))
            .collect();
        for (id, payment) in unquoted {
            let fee = node.invoice_fee(&payment);
            if let Some(waiting) = self.lock().waiting.get_mut(&id) {
                waiting.fee = Some(fee);
            }
        }
    }

    /// Send every final receipt the hosts have not recorded.
    fn deliver(&self, transport: &dyn Transport) {
        let unsent: Vec<(String, String, Receipt)> = self
            .lock()
            .saved
            .unsent
            .iter()
            .map(|(id, (host, receipt))| (id.clone(), host.clone(), receipt.clone()))
            .collect();
        for (id, host, receipt) in unsent {
            match transport.settle(&host, &receipt) {
                // Recorded, or the host holds another final answer: either
                // way nothing more can be said.
                Ok(_) => {
                    let mut shared = self.lock();
                    shared.saved.unsent.remove(&id);
                    if let Some(entry) = shared.saved.ledger.entries.get_mut(&id) {
                        entry.delivered = true;
                    }
                    self.save(&mut shared);
                }
                Err(SettleError::Refused(_)) => {
                    let mut shared = self.lock();
                    shared.saved.unsent.remove(&id);
                    self.save(&mut shared);
                }
                Err(SettleError::Unreachable(_)) => {}
            }
        }
    }

    /// The owner tapped Approve on the sheet for `request`. Pays in the
    /// background with `node`; the receipt goes back on the next pass, or
    /// at once through `transport`.
    pub fn approve(
        &self,
        request: &str,
        node: Option<Arc<dyn Payer>>,
        transport: Option<Arc<dyn Transport>>,
    ) {
        let now = (self.clock)();
        let Some(node) = node else {
            self.lock().notice =
                Some("The wallet is still starting. Try again in a moment.".into());
            return;
        };
        let waiting = {
            let mut shared = self.lock();
            if shared.busy.is_some() {
                return;
            }
            let Some(waiting) = shared.waiting.get(request).cloned() else {
                return;
            };
            let held = shared
                .saved
                .grants
                .get(&waiting.host)
                .filter(|held| !held.blocked)
                .cloned();
            // Check and reserve again, now: the ledger may have moved since
            // the sheet opened.
            match shared.saved.ledger.reserve_in(
                held.as_ref().map(|held| &held.grant),
                held.as_ref().map(HostGrant::earlier).unwrap_or_default(),
                &waiting.request,
                now,
                false,
            ) {
                Ok(_) => {}
                Err(code) => {
                    shared.waiting.remove(request);
                    refuse(&mut shared, &waiting.host, &waiting.request, code, now);
                    shared.notice = Some(code.describe().into());
                    self.save(&mut shared);
                    return;
                }
            }
            shared.busy = Some(request.to_owned());
            self.save(&mut shared);
            waiting
        };
        let worker = self.worker();
        std::thread::spawn(move || {
            worker.pay(&waiting.host, &waiting.request, node.as_ref());
            if let Some(transport) = transport {
                worker.deliver(transport.as_ref());
            }
        });
    }

    /// Pay a reserved request and record the outcome. Blocking.
    pub(crate) fn pay(&self, host: &str, request: &SpendRequest, node: &dyn Payer) {
        let paid = node.pay_invoice(
            &request.payment,
            request.fee_max_msat / 1000,
            &idempotency_key(&request.request),
        );
        let now = (self.clock)();
        let mut shared = self.lock();
        let label = shared
            .saved
            .grants
            .get(host)
            .map_or_else(|| "a computer".into(), |held| held.label.clone());
        match paid {
            Ok(payment) if payment.row.status == "failed" => {
                // The wallet's own evidence releases even a pending payment.
                shared.saved.ledger.fail(&request.request, now);
                refuse(&mut shared, host, request, Refusal::PaymentFailed, now);
                shared.notice = Some(Refusal::PaymentFailed.describe().into());
            }
            Ok(payment) => {
                let settled = payment.row.status == "completed";
                let fee_msat = payment.row.fee_sats.saturating_mul(1000);
                shared.saved.ledger.settle(
                    &request.request,
                    settled,
                    Some(payment.row.id.clone()),
                    Some(fee_msat),
                    now,
                );
                let remaining = grant_remaining(&shared, host, now);
                let outcome = match (settled, &payment.preimage) {
                    (true, Some(_)) => Settlement::Paid,
                    (true, None) => Settlement::Unknown,
                    (false, _) => Settlement::Pending,
                };
                let receipt = Receipt {
                    v: RECEIPT.into(),
                    requires: vec![],
                    request: request.request.clone(),
                    grant: request.grant.clone(),
                    outcome,
                    code: None,
                    payment_id: Some(payment.row.id.clone()),
                    amount_msat: Some(request.amount_msat),
                    fees_msat: Some(fee_msat),
                    proof: payment.preimage.filter(|_| outcome == Settlement::Paid),
                    remaining,
                    at: now,
                };
                shared
                    .saved
                    .unsent
                    .insert(request.request.clone(), (host.to_owned(), receipt));
                let automatically = shared
                    .saved
                    .ledger
                    .entries
                    .get(&request.request)
                    .is_some_and(|entry| entry.auto);
                shared.notice = Some(match outcome {
                    Settlement::Paid if automatically => format!(
                        "Paid {} automatically for {label}.",
                        amount(request.amount_msat, shared.format)
                    ),
                    Settlement::Paid => {
                        format!(
                            "Paid {} for {label}.",
                            amount(request.amount_msat, shared.format)
                        )
                    }
                    Settlement::Pending => format!("The payment for {label} is on its way."),
                    _ => format!("Paid for {label}, but the wallet has no proof yet."),
                });
            }
            Err(AgentPayFailure::Unknown(_)) => {
                // The send may have gone through: keep the reservation and
                // ask the wallet again with the same key on the next pass,
                // which returns that payment rather than paying twice.
                shared
                    .saved
                    .ledger
                    .settle(&request.request, false, None, None, now);
                shared.notice = Some(format!(
                    "The payment for {label} may have gone through. The wallet will check again."
                ));
            }
            Err(failure) => {
                let code = match failure {
                    AgentPayFailure::FeeTooHigh(_) => Refusal::FeeTooHigh,
                    AgentPayFailure::InsufficientFunds => Refusal::InsufficientFunds,
                    AgentPayFailure::Failed(_) | AgentPayFailure::Unknown(_) => {
                        Refusal::PaymentFailed
                    }
                };
                refuse(&mut shared, host, request, code, now);
                shared.notice = Some(code.describe().into());
            }
        }
        shared.waiting.remove(&request.request);
        shared.busy = None;
        self.save(&mut shared);
    }

    /// The owner tapped Deny.
    pub fn deny(&self, request: &str) {
        let now = (self.clock)();
        let mut shared = self.lock();
        if shared.busy.as_deref() == Some(request) {
            return;
        }
        if let Some(waiting) = shared.waiting.remove(request) {
            refuse(
                &mut shared,
                &waiting.host,
                &waiting.request,
                Refusal::DeclinedByOwner,
                now,
            );
            shared.notice = Some("Declined. Nothing was paid.".into());
            self.save(&mut shared);
        }
    }

    /// Stop `host`'s requests: advance its epoch, refuse what waits, and
    /// hand it no grant until the owner allows it again.
    pub fn block(&self, host: &str) {
        let now = (self.clock)();
        let mut shared = self.lock();
        let Some(held) = shared.saved.grants.get_mut(host) else {
            return;
        };
        held.blocked = true;
        held.unannounced = true;
        held.grant.epoch += 1;
        let waiting: Vec<Waiting> = shared
            .waiting
            .values()
            .filter(|w| w.host == host && shared.busy.as_deref() != Some(&w.request.request))
            .cloned()
            .collect();
        for waiting in waiting {
            shared.waiting.remove(&waiting.request.request);
            refuse(&mut shared, host, &waiting.request, Refusal::Revoked, now);
        }
        shared.notice = Some("This computer can no longer ask for payments.".into());
        self.save(&mut shared);
    }

    /// Let a blocked computer ask again, under a new grant at its epoch.
    pub fn allow(&self, host: &str) {
        let now = (self.clock)();
        let mut shared = self.lock();
        let device = self.device.clone();
        let Some(held) = shared.saved.grants.get_mut(host) else {
            return;
        };
        if !held.blocked {
            return;
        }
        held.grant = Grant::request_mode(random_id(), &device, host, held.grant.epoch, now);
        held.accepted.clear();
        held.counted.clear();
        held.blocked = false;
        held.unannounced = false;
        shared.last_poll = None;
        self.save(&mut shared);
    }

    /// Change `host`'s grant with `change` under a new ID at the same epoch.
    /// The old grant's waiting requests are still answered and its payments
    /// still count. The computer receives the new grant on the next pass.
    fn change(&self, host: &str, change: impl FnOnce(&mut Grant)) -> bool {
        let now = (self.clock)();
        let mut shared = self.lock();
        let Some(held) = shared
            .saved
            .grants
            .get_mut(host)
            .filter(|held| !held.blocked)
        else {
            return false;
        };
        let mut next = held.grant.clone();
        change(&mut next);
        next.grant = random_id();
        next.issued_at = now.min(next.expires_at.saturating_sub(1));
        if next.validate().is_err() {
            return false;
        }
        held.replace(next, true);
        shared.last_poll = None;
        self.save(&mut shared);
        true
    }

    /// Trust `payee` (a node key) for `host`: payments to it, within the
    /// automatic ceilings, need no tap from now on. Makes the grant
    /// `standing` if it was not.
    pub fn trust(&self, host: &str, payee: &str) {
        let payee = payee.to_owned();
        let changed = self.change(host, |grant| {
            if grant.mode == Mode::Request {
                *grant = Grant::standing(
                    grant.grant.clone(),
                    &grant.issuer,
                    &grant.grantee,
                    grant.epoch,
                    grant.issued_at,
                    Default::default(),
                )
                .with_expiry(grant.expires_at);
            }
            if let Some(auto) = grant.auto.as_mut() {
                auto.payees.insert(payee, defaults::AUTO_PAYEE_MAX);
            }
        });
        if changed {
            self.lock().notice =
                Some("Payments to this payee within your limits won't ask again.".into());
        }
    }

    /// Stop paying `payee` without a tap. When none is left, the computer's
    /// grant goes back to asking for every payment.
    pub fn untrust(&self, host: &str, payee: &str) {
        self.change(host, |grant| {
            if let Some(auto) = grant.auto.as_mut() {
                auto.payees.remove(payee);
                if auto.payees.is_empty() {
                    grant.mode = Mode::Request;
                    grant.auto = None;
                }
            }
        });
    }

    /// Stop every automatic payment for `host`: it asks for each payment.
    pub fn manual(&self, host: &str) {
        let changed = self.change(host, |grant| {
            grant.mode = Mode::Request;
            grant.auto = None;
        });
        if changed {
            self.lock().notice = Some("Every payment from this computer asks you first.".into());
        }
    }

    /// The owner tapped "Approve and trust" on the sheet: pay this request
    /// and trust its payee for later ones.
    pub fn approve_and_trust(
        &self,
        request: &str,
        node: Option<Arc<dyn Payer>>,
        transport: Option<Arc<dyn Transport>>,
    ) {
        let target = self
            .lock()
            .waiting
            .get(request)
            .map(|waiting| (waiting.host.clone(), waiting.admitted.payee.clone()));
        self.approve(request, node, transport);
        if let Some((host, payee)) = target {
            self.trust(&host, &payee);
        }
    }

    /// Whether the sheet or a payment waits, so the host asks for packets.
    pub fn live(&self) -> bool {
        let shared = self.lock();
        !shared.waiting.is_empty() || shared.busy.is_some() || shared.polling
    }

    pub fn view(&self) -> View {
        let now = (self.clock)();
        let shared = self.lock();
        let format = shared.format;
        let label = |host: &str| {
            shared
                .saved
                .grants
                .get(host)
                .map_or_else(|| short(host), |held| held.label.clone())
        };
        let mut waiting: Vec<&Waiting> = shared.waiting.values().collect();
        waiting.sort_by_key(|w| (w.request.issued_at, w.request.request.clone()));
        let sheet = waiting.first().map(|w| {
            let invoice = &w.admitted.invoice;
            let ceiling = w.request.fee_max_msat;
            let (fee, ready) = match &w.fee {
                None => ("Reading the fee…".to_owned(), false),
                Some(Ok(fee)) if fee.saturating_mul(1000) > ceiling => (
                    format!(
                        "{} (above the {} ceiling)",
                        format.show(*fee),
                        amount(ceiling, format)
                    ),
                    false,
                ),
                Some(Ok(fee)) => (format.show(*fee), true),
                Some(Err(message)) => (message.clone(), false),
            };
            Sheet {
                request: w.request.request.clone(),
                host: w.host.clone(),
                computer: label(&w.host),
                task: w.request.context.task.clone(),
                title: w.request.context.title.clone(),
                purpose: w.request.purpose.label(),
                payee: short(&w.admitted.payee),
                payee_new: w.admitted.new_payee,
                description: invoice.description.clone(),
                note: w.request.context.note.clone(),
                resource: w.request.context.resource.clone(),
                amount: amount(w.request.amount_msat, format),
                amount_msat: w.request.amount_msat,
                fee,
                fee_ceiling: amount(ceiling, format),
                remaining: remaining_text(w.admitted.remaining, format),
                expires_at: w.request.expires_at,
                authenticate: w.request.amount_msat > AUTHENTICATE_ABOVE_MSAT,
                ready: ready && shared.busy.is_none(),
                can_trust: w.request.amount_msat.saturating_add(w.request.fee_max_msat)
                    <= defaults::AUTO_PER_PAYMENT_MAX
                    && shared.saved.grants.get(&w.host).is_some_and(|held| {
                        held.grant
                            .auto
                            .as_ref()
                            .is_none_or(|auto| !auto.payees.contains_key(&w.admitted.payee))
                    }),
            }
        });
        let computers = shared
            .saved
            .grants
            .iter()
            .map(|(host, held)| ComputerRow {
                host: host.clone(),
                computer: held.label.clone(),
                blocked: held.blocked,
                remaining: if held.blocked {
                    "Can't ask for payments".into()
                } else {
                    remaining_text(
                        shared
                            .saved
                            .ledger
                            .remaining_in(&held.grant, held.earlier(), now),
                        format,
                    )
                },
                automatic: (!held.blocked)
                    .then(|| shared.saved.ledger.automatic_remaining(&held.grant, now))
                    .flatten()
                    .map(|left| {
                        format!(
                            "Pays trusted payees without asking, up to {} each: {} left today",
                            amount(
                                held.grant.auto.as_ref().map_or(0, |a| a.per_payment_max),
                                format
                            ),
                            amount(left, format)
                        )
                    }),
                trusted: held
                    .grant
                    .auto
                    .iter()
                    .flat_map(|auto| auto.payees.iter())
                    .map(|(payee, limit)| TrustedRow {
                        payee: payee.clone(),
                        label: short(payee),
                        limit: format!("{} a day", amount(*limit, format)),
                    })
                    .collect(),
            })
            .collect();
        let mut entries: Vec<_> = shared.saved.ledger.entries.values().collect();
        entries.sort_by_key(|e| std::cmp::Reverse((e.at, e.request.clone())));
        let history = entries
            .into_iter()
            .take(HISTORY)
            .map(|e| HistoryRow {
                request: e.request.clone(),
                computer: label(&e.host),
                title: e.title.clone(),
                purpose: e.purpose.label(),
                amount: amount(e.amount_msat, format),
                fee: e.fee_msat.map(|fee| amount(fee, format)),
                state: match e.state {
                    State::Paid => "paid",
                    State::Pending => "pending",
                    State::Reserved => "paying",
                    State::Refused => "refused",
                },
                auto: e.auto,
                detail: e.code.map(|code| code.describe().to_owned()),
                at: e.at,
            })
            .collect();
        View {
            sheet,
            waiting: shared.waiting.len().saturating_sub(1),
            busy: shared.busy.is_some(),
            notice: shared.notice.clone(),
            computers,
            history,
        }
    }

    /// Clear the last outcome's notice.
    pub fn dismiss(&self) {
        self.lock().notice = None;
    }
}

fn grant_remaining(shared: &Shared, host: &str, now: u64) -> Option<Remaining> {
    shared
        .saved
        .grants
        .get(host)
        .map(|held| shared.saved.ledger.remaining(&held.grant, now))
}

/// The final receipt a ledger entry states, if it is final.
fn receipt_for(ledger: &Ledger, request: &str) -> Option<Receipt> {
    let entry = ledger.entries.get(request)?;
    match entry.state {
        State::Refused => Some(Receipt::refused(
            request,
            &entry.grant,
            entry.code.unwrap_or(Refusal::Malformed),
            entry.settled_at.unwrap_or(entry.at),
        )),
        // A paid entry's proof was sent with its first receipt; without it
        // the host learns only that it is unknown.
        _ => None,
    }
}

/// Record a refusal in the ledger and queue its receipt for `host`.
fn refuse(shared: &mut Shared, host: &str, request: &SpendRequest, code: Refusal, now: u64) {
    shared.saved.ledger.refuse(request, code, now);
    shared.saved.unsent.insert(
        request.request.clone(),
        (
            host.to_owned(),
            Receipt::refused(&request.request, &request.grant, code, now),
        ),
    );
}

#[cfg(test)]
mod tests;
