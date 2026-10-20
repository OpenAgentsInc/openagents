//! Payments the person's browser asks for, approved and paid on the phone
//! (#12306). The website's side is `/v1/wallet/browser` (#12258): the
//! browser holds no wallet and no key; it asks, and this decides.
//!
//! The documents are the agent spend protocol's (`coder_access::spend`,
//! `docs/breez/spend-protocol.md`), with the website as the asking side:
//!
//! 1. **Grant.** [`Approvals::refresh`] reads the browser's book. When the
//!    phone has no current grant for the browser's key there, it gives one
//!    in request mode with the phase 1 bounds (10,000 sats a payment, 50,000
//!    a day, 500,000 in the grant's life): nothing pays without a tap.
//! 2. **Check.** Each unanswered request is checked against the grant and
//!    what the phone already paid under it. One that fails is refused at
//!    once with its code; the rest wait on the approval card.
//! 3. **Approve or decline.** [`Approvals::approve`] pays the invoice once
//!    through the phone's Spark wallet, with an idempotency key from the
//!    request ID, and sends the receipt with the preimage, which the browser
//!    hands to x402. [`Approvals::decline`] sends `declined_by_owner`.
//!
//! The view is a Rust Native card; the native shells only draw it and pass
//! its button presses here.

use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};

use coder_access::spend::{Grant, Purpose, RECEIPT, Receipt, Refusal, Settlement, SpendRequest};
use openagents_spark::model::{AgentPayFailure, Node};
use rust_native::{Axis, Element, Node as ViewNode, TextRole, View};
use serde_json::Value;

use super::Action;

/// A day, the phase 1 grant's period, before its end when the phone renews it.
const RENEW_BEFORE: u64 = 24 * 60 * 60;

/// The site's side, as the app's session reaches it.
pub trait BrowserApi: Send + Sync {
    fn book(&self) -> Pin<Box<dyn Future<Output = Result<Book, String>> + Send + '_>>;
    fn grant(&self, grant: Grant) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
    fn settle(
        &self,
        receipt: Receipt,
    ) -> Pin<Box<dyn Future<Output = Result<(), String>> + Send + '_>>;
}

/// What the site holds for the browser.
#[derive(Clone, Debug, Default)]
pub struct Book {
    pub grantee: String,
    pub grant: Option<Grant>,
    pub requests: Vec<(SpendRequest, Option<Receipt>)>,
}

impl Book {
    /// The book from the SDK's answer, keeping only documents that read.
    #[must_use]
    pub fn from_sdk(wallet: openagents_account::openagents::browser_wallet::BrowserWallet) -> Self {
        Self {
            grantee: wallet.grantee,
            grant: wallet.grant.and_then(|g| serde_json::from_value(g).ok()),
            requests: wallet
                .requests
                .into_iter()
                .filter_map(|entry| {
                    let request: SpendRequest = serde_json::from_value(entry.request).ok()?;
                    let receipt = entry
                        .receipt
                        .and_then(|r: Value| serde_json::from_value(r).ok());
                    Some((request, receipt))
                })
                .collect(),
        }
    }
}

#[derive(Default)]
struct State {
    grant: Option<Grant>,
    pending: Vec<SpendRequest>,
    paid_today: u64,
    notice: Option<String>,
    busy: bool,
    revision: u64,
}

/// The phone's answers to its browser.
pub struct Approvals {
    api: Arc<dyn BrowserApi>,
    node: Arc<dyn Node>,
    /// The phone's key, the grant's issuer.
    issuer: String,
    instance: String,
    state: Mutex<State>,
    now: Arc<dyn Fn() -> u64 + Send + Sync>,
}

fn sats(msat: u64) -> u64 {
    msat.div_ceil(1000)
}

fn paid_total(receipt: &Receipt) -> u64 {
    receipt.amount_msat.unwrap_or(0) + receipt.fees_msat.unwrap_or(0)
}

/// What's already been paid under `grant` within its period before `now`.
fn spent(book: &Book, grant: &Grant, now: u64) -> (u64, u64) {
    let (mut period, mut total) = (0, 0);
    for (request, receipt) in &book.requests {
        if let Some(receipt) = receipt
            && receipt.outcome == Settlement::Paid
            && request.grant == grant.grant
        {
            total += paid_total(receipt);
            if now.saturating_sub(receipt.at) < grant.period {
                period += paid_total(receipt);
            }
        }
    }
    (period, total)
}

/// Why `request` can't be paid under `grant`, if it can't.
fn check(request: &SpendRequest, grant: &Grant, spent: (u64, u64), now: u64) -> Option<Refusal> {
    if request.validate().is_err() || request.grantee != grant.grantee {
        return Some(Refusal::Malformed);
    }
    if request.grant != grant.grant || request.epoch != grant.epoch {
        return Some(Refusal::Stale);
    }
    if request.expires_at <= now {
        return Some(Refusal::Expired);
    }
    if !grant.purposes.contains(&request.purpose) {
        return Some(Refusal::PurposeNotAllowed);
    }
    if request.fee_max_msat > grant.fee_max.ceiling(request.amount_msat) {
        return Some(Refusal::FeeTooHigh);
    }
    let cost = request.amount_msat + request.fee_max_msat;
    if cost > grant.per_payment_max {
        return Some(Refusal::OverPaymentCap);
    }
    if spent.0 + cost > grant.period_max {
        return Some(Refusal::OverPeriodCap);
    }
    if spent.1 + cost > grant.total_max {
        return Some(Refusal::OverTotalCap);
    }
    None
}

fn random_id() -> String {
    secp256k1::rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

impl Approvals {
    pub fn new(
        api: Arc<dyn BrowserApi>,
        node: Arc<dyn Node>,
        issuer: String,
        now: Arc<dyn Fn() -> u64 + Send + Sync>,
    ) -> Self {
        Self {
            api,
            node,
            issuer,
            instance: format!("browser-payments-{}", uuid::Uuid::new_v4()),
            state: Mutex::new(State::default()),
            now,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Read the browser's requests, giving or renewing the grant first, and
    /// refuse at once what can't be paid.
    ///
    /// # Errors
    ///
    /// A sentence for the person, also shown on the card.
    pub async fn refresh(&self) -> Result<(), String> {
        let result = self.refresh_inner().await;
        let mut state = self.lock();
        state.notice = result.as_ref().err().cloned();
        state.revision += 1;
        result
    }

    async fn refresh_inner(&self) -> Result<(), String> {
        let now = (self.now)();
        let mut book = self.api.book().await?;
        let current = book.grant.clone().filter(|g| {
            g.issuer == self.issuer
                && g.grantee == book.grantee
                && g.expires_at > now + RENEW_BEFORE
        });
        let grant = match current {
            Some(grant) => grant,
            None => {
                let epoch = book.grant.as_ref().map_or(1, |g| g.epoch);
                let grant =
                    Grant::request_mode(random_id(), &self.issuer, &book.grantee, epoch, now);
                self.api.grant(grant.clone()).await?;
                book.grant = Some(grant.clone());
                grant
            }
        };
        let mut pending = Vec::new();
        let mut refusals = Vec::new();
        let spent_now = spent(&book, &grant, now);
        for (request, receipt) in &book.requests {
            if receipt.as_ref().is_some_and(Receipt::is_final) {
                continue;
            }
            match check(request, &grant, spent_now, now) {
                // An old grant's requests stay unanswered rather than refused:
                // the site keeps them until they expire.
                Some(Refusal::Stale) if request.expires_at <= now => {}
                Some(code) => refusals.push(Receipt::refused(
                    &request.request,
                    &request.grant,
                    code,
                    now,
                )),
                None => pending.push(request.clone()),
            }
        }
        for receipt in refusals {
            self.api.settle(receipt).await?;
        }
        let mut state = self.lock();
        state.grant = Some(grant);
        state.pending = pending;
        state.paid_today = spent_now.0;
        Ok(())
    }

    fn take(&self, id: &str) -> Result<(SpendRequest, Grant), String> {
        let mut state = self.lock();
        if state.busy {
            return Err("A payment is already going through.".into());
        }
        let grant = state.grant.clone().ok_or("Open payments again.")?;
        let request = state
            .pending
            .iter()
            .find(|r| r.request == id)
            .cloned()
            .ok_or("That request is gone.")?;
        state.busy = true;
        state.revision += 1;
        Ok((request, grant))
    }

    fn done(&self, id: &str, notice: Option<String>) {
        let mut state = self.lock();
        state.busy = false;
        state.pending.retain(|r| r.request != id);
        state.notice = notice;
        state.revision += 1;
    }

    /// Pay request `id` once and tell the browser.
    ///
    /// # Errors
    ///
    /// A sentence for the person, also shown on the card.
    pub async fn approve(&self, id: &str) -> Result<(), String> {
        let (request, grant) = self.take(id)?;
        let now = (self.now)();
        let spent_now = (self.lock().paid_today, 0);
        if let Some(code) = check(&request, &grant, (spent_now.0, 0), now) {
            let receipt = Receipt::refused(&request.request, &request.grant, code, now);
            let sent = self.api.settle(receipt).await;
            self.done(id, Some(code.describe().into()));
            return sent;
        }
        let node = self.node.clone();
        let invoice = request.payment.clone();
        let max_fee = sats(request.fee_max_msat);
        // The request ID is 32 bytes; its first 16 make the UUID the wallet
        // keys a payment by, so a retry is the same payment.
        let key = uuid::Uuid::from_slice(
            &(0..16)
                .map(|i| u8::from_str_radix(&request.request[2 * i..2 * i + 2], 16).unwrap_or(0))
                .collect::<Vec<_>>(),
        )
        .unwrap_or_default()
        .to_string();
        let paid = tokio::task::spawn_blocking(move || node.pay_invoice(&invoice, max_fee, &key))
            .await
            .map_err(|_| "The payment didn't go through.".to_string())?;
        let receipt = match paid {
            Ok(payment) => match payment.preimage {
                Some(proof) => Receipt {
                    v: RECEIPT.into(),
                    requires: vec![],
                    request: request.request.clone(),
                    grant: request.grant.clone(),
                    outcome: Settlement::Paid,
                    code: None,
                    payment_id: Some(payment.row.id),
                    amount_msat: Some(request.amount_msat),
                    fees_msat: Some(payment.row.fee_sats * 1000),
                    proof: Some(proof),
                    remaining: None,
                    at: now,
                },
                None => Receipt {
                    outcome: Settlement::Pending,
                    code: None,
                    payment_id: Some(payment.row.id),
                    ..Receipt::refused(
                        &request.request,
                        &request.grant,
                        Refusal::PaymentFailed,
                        now,
                    )
                },
            },
            Err(AgentPayFailure::InsufficientFunds) => Receipt::refused(
                &request.request,
                &request.grant,
                Refusal::InsufficientFunds,
                now,
            ),
            Err(AgentPayFailure::FeeTooHigh(_)) => {
                Receipt::refused(&request.request, &request.grant, Refusal::FeeTooHigh, now)
            }
            Err(AgentPayFailure::Unknown(_)) => Receipt {
                outcome: Settlement::Unknown,
                code: None,
                ..Receipt::refused(
                    &request.request,
                    &request.grant,
                    Refusal::PaymentFailed,
                    now,
                )
            },
            Err(AgentPayFailure::Failed(_)) => Receipt::refused(
                &request.request,
                &request.grant,
                Refusal::PaymentFailed,
                now,
            ),
        };
        let notice = match receipt.outcome {
            Settlement::Paid => None,
            Settlement::Refused => receipt.code.map(|c| c.describe().to_string()),
            _ => Some(
                "The payment may still go through. Check Activity before you try again.".into(),
            ),
        };
        let sent = self.api.settle(receipt).await;
        self.done(id, notice);
        sent
    }

    /// Decline request `id`.
    ///
    /// # Errors
    ///
    /// A sentence for the person.
    pub async fn decline(&self, id: &str) -> Result<(), String> {
        let (request, _) = self.take(id)?;
        let receipt = Receipt::refused(
            &request.request,
            &request.grant,
            Refusal::DeclinedByOwner,
            (self.now)(),
        );
        let sent = self.api.settle(receipt).await;
        self.done(id, None);
        sent
    }

    /// The approval card.
    pub fn view(&self) -> View<Action> {
        let state = self.lock();
        let mut children = vec![text(
            "heading",
            "Payments from your browser",
            TextRole::Heading,
        )];
        if let Some(notice) = &state.notice {
            children.push(text("notice", notice.clone(), TextRole::Status));
        }
        if state.pending.is_empty() {
            children.push(text(
                "empty",
                "Your browser hasn't asked to pay for anything.",
                TextRole::Body,
            ));
        }
        for request in &state.pending {
            let what = request
                .context
                .resource
                .clone()
                .or_else(|| request.context.title.clone())
                .unwrap_or_else(|| "openagents.com".into());
            let purpose = match request.purpose {
                Purpose::X402Purchase => "A purchase",
                _ => "A payment",
            };
            let mut rows = vec![
                text(
                    format!("amount-{}", request.request),
                    format!("{} sats", sats(request.amount_msat)),
                    TextRole::Heading,
                ),
                text(
                    format!("what-{}", request.request),
                    format!(
                        "{purpose} for {what}, plus up to {} sats in fees.",
                        sats(request.fee_max_msat)
                    ),
                    TextRole::Body,
                ),
            ];
            if let Some(note) = &request.context.note {
                rows.push(text(
                    format!("note-{}", request.request),
                    format!("The browser says: {note}"),
                    TextRole::Body,
                ));
            }
            rows.push(button(
                format!("approve:{}", request.request),
                "Pay",
                !state.busy,
            ));
            rows.push(button(
                format!("decline:{}", request.request),
                "Decline",
                !state.busy,
            ));
            children.push(stack(format!("request-{}", request.request), rows));
        }
        View::new_v3(
            &self.instance,
            state.revision,
            stack("browser-payments", children),
        )
    }
}

fn node(key: impl Into<String>, element: Element<Action>) -> ViewNode<Action> {
    ViewNode {
        key: key.into(),
        style: Default::default(),
        element,
    }
}

fn text(key: impl Into<String>, value: impl Into<String>, role: TextRole) -> ViewNode<Action> {
    node(
        key,
        Element::Text {
            value: value.into(),
            role,
        },
    )
}

fn button(key: impl Into<String>, label: impl Into<String>, enabled: bool) -> ViewNode<Action> {
    let key = key.into();
    node(
        key.clone(),
        Element::Button {
            label: label.into(),
            enabled,
            icon: None,
            shortcut: None,
            intent: Action(key),
        },
    )
}

fn stack(key: impl Into<String>, children: Vec<ViewNode<Action>>) -> ViewNode<Action> {
    node(
        key,
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    )
}

#[cfg(test)]
mod tests;
