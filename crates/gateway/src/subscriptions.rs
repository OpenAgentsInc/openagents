//! Stripe subscriptions for the plan catalog (#11072): a Checkout Session in
//! subscription mode, the billing portal, the webhook events that pay,
//! renew, fail, and end a subscription, and the hand-off of paid months and
//! extra-hour charges to saved environments
//! (`docs/cloud/retail-environment-contract.md`).
//!
//! The billing book (`tenancy::billing`) stays the authority on what a
//! workspace subscribes to; this module only speaks Stripe on its behalf:
//!
//! - `POST /v1/workspaces/{id}/billing/checkout` opens a Stripe Checkout
//!   Session for the plan's configured price and answers its URL. Nothing is
//!   granted by the browser coming back; only a signed event moves the book.
//! - `POST /v1/billing/webhook` takes `Stripe-Signature` events:
//!   `checkout.session.completed` starts the subscription,
//!   `invoice.paid` renews it (and records the paid month),
//!   `invoice.payment_failed` makes it past due, and
//!   `customer.subscription.deleted` ends it. `charge.refunded` (a full
//!   refund), `charge.dispute.created` and `charge.dispute.closed` are the
//!   refund and dispute rules of #11074 (see [`read`]). Anything else is
//!   acknowledged and ignored.
//! - A plan with an `environments` allowance writes each paid month into the
//!   environment meter (`environment_meter`, the retail journal) with
//!   `record_period`, so hours reset on renewal, and names the workspace
//!   whose credits pay extra hours. An ended subscription cuts its month
//!   short with `end_period`. A failed renewal records nothing: the month
//!   that was paid runs to its end, and the next one starts only when Stripe
//!   collects it.
//! - [`post_debits`] hands the meter's queued extra-hour charges to the
//!   workspace's money ledger once each (a reserve and settle keyed on the
//!   charge), on every webhook and once a minute.
//!
//! The secret key and webhook secrets are read from environment variables
//! the config names; they are never logged or echoed. Test mode (`live:
//! false`) refuses a live key and live events, and a live deployment refuses
//! test ones.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use retail_cloud::environment::Notice;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tenancy::billing::{BillingBook, Event, Invoice, InvoiceState, Plan, SubscriptionState};
use tenancy::money::{Ledger, Mutation, Operation, Phase, Price, Rate, Resource};

use crate::serve::ServeState;

const API_ORIGIN: &str = "https://api.stripe.com";
const MAX_BODY: usize = 512 * 1024;
/// How often queued extra-hour charges are offered to the ledger.
const POST_EVERY: Duration = Duration::from_secs(60);

/// The deployment's Stripe subscription settings: names of secrets, never
/// the secrets themselves.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// Live mode. Off means Stripe test mode: only `sk_test_`/`rk_test_`
    /// keys and test events.
    pub live: bool,
    /// The environment variable holding the secret (or restricted) key.
    pub secret_key_env: String,
    /// The environment variables holding webhook signing secrets (one, or
    /// two while rotating).
    pub webhook_secret_envs: Vec<String>,
    /// The Stripe price each paid plan sells: plan id to `price_…`.
    pub prices: BTreeMap<String, String>,
    /// Where Checkout sends the browser after paying.
    pub success_url: String,
    /// Where Checkout's back link and the billing portal return.
    pub return_url: String,
    /// The environment meter (the retail journal) that paid months and
    /// extra-hour charges go through. Absent: plans with environments sell,
    /// but no hours are recorded.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment_meter: Option<PathBuf>,
    /// A loopback stand-in for Stripe's API, in test mode only.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub api_origin: Option<String>,
}

fn env_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().next().is_some_and(|b| b.is_ascii_uppercase())
        && value
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
}

fn https(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|u| {
        u.scheme() == "https"
            && u.host_str().is_some()
            && u.username().is_empty()
            && u.password().is_none()
    })
}

fn reference(value: &str, prefix: &str) -> bool {
    value.strip_prefix(prefix).is_some_and(|rest| {
        !rest.is_empty()
            && value.len() <= 255
            && rest.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
    })
}

impl Config {
    /// Why the settings can't sell `plans`, if they can't.
    ///
    /// # Errors
    ///
    /// A plain sentence naming the problem.
    pub fn check(&self, plans: &[Plan]) -> Result<(), String> {
        if !env_name(&self.secret_key_env)
            || self.webhook_secret_envs.is_empty()
            || self.webhook_secret_envs.len() > 2
            || self.webhook_secret_envs.iter().any(|v| !env_name(v))
            || self.webhook_secret_envs.contains(&self.secret_key_env)
        {
            return Err("billing.stripe names its key and webhook secrets by environment variable (upper-case names, one or two webhook secrets)".into());
        }
        if !https(&self.success_url) || !https(&self.return_url) {
            return Err("billing.stripe success_url and return_url must be https addresses".into());
        }
        if let Some(origin) = &self.api_origin {
            let local = reqwest::Url::parse(origin).is_ok_and(|u| {
                matches!(u.host_str(), Some("127.0.0.1" | "localhost")) && u.path() == "/"
            });
            if self.live || !local {
                return Err(
                    "billing.stripe api_origin is a loopback stand-in for test mode only".into(),
                );
            }
        }
        for plan in plans.iter().filter(|p| !p.price.is_free()) {
            match self.prices.get(&plan.id) {
                Some(price) if reference(price, "price_") => {}
                _ => {
                    return Err(format!(
                        "billing.stripe needs a Stripe price (`price_…`) for plan `{}`",
                        plan.id
                    ));
                }
            }
        }
        if let Some(unknown) = self
            .prices
            .keys()
            .find(|id| !plans.iter().any(|p| &p.id == *id))
        {
            return Err(format!(
                "billing.stripe prices plan `{unknown}`, which isn't in the catalog"
            ));
        }
        Ok(())
    }

    fn client(&self) -> Result<Client, String> {
        let key = std::env::var(&self.secret_key_env)
            .map_err(|_| "Stripe isn't set up on this server.".to_string())?;
        let mode = if self.live { "live_" } else { "test_" };
        if !(key.starts_with(&format!("sk_{mode}")) || key.starts_with(&format!("rk_{mode}")))
            || key.len() > 1024
            || key.bytes().any(|b| !b.is_ascii_graphic())
        {
            return Err("Stripe's key doesn't match this server's mode.".into());
        }
        let http = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| "Stripe can't be reached right now.".to_string())?;
        Ok(Client {
            http,
            key,
            origin: self
                .api_origin
                .clone()
                .unwrap_or_else(|| API_ORIGIN.into())
                .trim_end_matches('/')
                .to_string(),
            live: self.live,
        })
    }

    /// Verify a `Stripe-Signature` against each configured secret and parse
    /// the event, refusing the other mode's events.
    ///
    /// # Errors
    ///
    /// The signature, mode, or shape is wrong.
    pub fn verify(&self, body: &[u8], header: &str, now: u64) -> Result<Value, String> {
        let refused = || "The event's signature couldn't be verified.".to_string();
        let ok = self.webhook_secret_envs.iter().any(|name| {
            std::env::var(name).is_ok_and(|secret| {
                crate::card_funding::verify_signature(body, header, secret.as_bytes(), now, 300)
                    .is_ok()
            })
        });
        if !ok || body.len() > MAX_BODY {
            return Err(refused());
        }
        let value: Value = serde_json::from_slice(body).map_err(|_| refused())?;
        if value["object"] != "event"
            || value["livemode"] != self.live
            || !value["id"].as_str().is_some_and(|id| reference(id, "evt_"))
            || !value["type"].is_string()
        {
            return Err(refused());
        }
        Ok(value)
    }
}

/// Stripe's API, over the one configured origin: no proxies, no redirects,
/// no logged bodies.
struct Client {
    http: reqwest::Client,
    key: String,
    origin: String,
    live: bool,
}

/// What the subscription Checkout Session carries back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    pub id: String,
    pub url: String,
}

/// Who a checkout is for.
pub struct Buyer<'a> {
    pub checkout: &'a str,
    pub workspace: &'a str,
    pub account: &'a str,
    pub plan: &'a str,
    /// The workspace's Stripe customer from an earlier subscription.
    pub customer: Option<&'a str>,
    /// When Stripe stops taking payment on the session: before the book's
    /// own checkout expires, so a late payment never meets a closed
    /// checkout.
    pub expires_at: u64,
}

impl Client {
    async fn post(
        &self,
        path: &str,
        fields: &[(String, String)],
        idempotency: &str,
    ) -> Result<Value, String> {
        let unreachable = || "Stripe didn't answer. Try again in a minute.".to_string();
        let mut response = self
            .http
            .post(format!("{}{path}", self.origin))
            .bearer_auth(&self.key)
            .header("Idempotency-Key", idempotency)
            .form(fields)
            .send()
            .await
            .map_err(|_| unreachable())?;
        if !response.status().is_success() {
            return Err(unreachable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err(unreachable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| unreachable())?;
        if value["livemode"] != self.live {
            return Err(unreachable());
        }
        Ok(value)
    }

    async fn get(&self, path: &str) -> Result<Value, String> {
        let unreachable = || "Stripe didn't answer. Try again in a minute.".to_string();
        let mut response = self
            .http
            .get(format!("{}{path}", self.origin))
            .bearer_auth(&self.key)
            .send()
            .await
            .map_err(|_| unreachable())?;
        if !response.status().is_success() {
            return Err(unreachable());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| unreachable())? {
            if bytes.len() + chunk.len() > MAX_BODY {
                return Err(unreachable());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| unreachable())?;
        if value["livemode"] != self.live {
            return Err(unreachable());
        }
        Ok(value)
    }

    async fn checkout(
        &self,
        config: &Config,
        price: &str,
        buyer: &Buyer<'_>,
    ) -> Result<Session, String> {
        let mut fields: Vec<(String, String)> = vec![
            ("mode".into(), "subscription".into()),
            ("line_items[0][price]".into(), price.into()),
            ("line_items[0][quantity]".into(), "1".into()),
            ("client_reference_id".into(), buyer.checkout.into()),
            ("success_url".into(), config.success_url.clone()),
            ("cancel_url".into(), config.return_url.clone()),
            ("expires_at".into(), buyer.expires_at.to_string()),
        ];
        for prefix in ["metadata", "subscription_data[metadata]"] {
            for (name, value) in [
                ("oa_checkout", buyer.checkout),
                ("oa_workspace", buyer.workspace),
                ("oa_account", buyer.account),
                ("oa_plan", buyer.plan),
            ] {
                fields.push((format!("{prefix}[{name}]"), value.into()));
            }
        }
        if let Some(customer) = buyer.customer.filter(|c| reference(c, "cus_")) {
            fields.push(("customer".into(), customer.into()));
        }
        let value = self
            .post("/v1/checkout/sessions", &fields, buyer.checkout)
            .await?;
        let failed = || "Stripe answered with a checkout we can't use.".to_string();
        let id = value["id"].as_str().ok_or_else(failed)?;
        let prefix = if self.live { "cs_live_" } else { "cs_test_" };
        let url = value["url"].as_str().ok_or_else(failed)?;
        let hosted = reqwest::Url::parse(url).map_err(|_| failed())?;
        if !reference(id, prefix)
            || value["object"] != "checkout.session"
            || value["mode"] != "subscription"
            || value["client_reference_id"] != buyer.checkout
            || hosted.scheme() != "https"
            || hosted.host_str() != Some("checkout.stripe.com")
        {
            return Err(failed());
        }
        Ok(Session {
            id: id.into(),
            url: url.into(),
        })
    }

    async fn portal(
        &self,
        customer: &str,
        return_url: &str,
        idempotency: &str,
    ) -> Result<String, String> {
        let failed = || "Stripe answered with a billing page we can't use.".to_string();
        if !reference(customer, "cus_") {
            return Err(failed());
        }
        let value = self
            .post(
                "/v1/billing_portal/sessions",
                &[
                    ("customer".into(), customer.into()),
                    ("return_url".into(), return_url.into()),
                ],
                idempotency,
            )
            .await?;
        let url = value["url"].as_str().ok_or_else(failed)?;
        let hosted = reqwest::Url::parse(url).map_err(|_| failed())?;
        if value["object"] != "billing_portal.session"
            || hosted.scheme() != "https"
            || hosted.host_str() != Some("billing.stripe.com")
        {
            return Err(failed());
        }
        Ok(url.into())
    }
}

/// Open the Stripe Checkout Session for a subscription.
///
/// # Errors
///
/// The plan has no price here, Stripe isn't set up, or Stripe refused.
pub async fn open_checkout(config: &Config, buyer: &Buyer<'_>) -> Result<Session, String> {
    let price = config
        .prices
        .get(buyer.plan)
        .ok_or_else(|| "This plan isn't sold here.".to_string())?;
    config.client()?.checkout(config, price, buyer).await
}

/// Read one charge from Stripe: a dispute names only its charge, so this
/// is how it finds the customer and invoice it belongs to.
///
/// # Errors
///
/// Stripe isn't set up, refused, or answered with something else.
pub async fn fetch_charge(config: &Config, charge: &str) -> Result<Value, String> {
    if !reference(charge, "ch_") && !reference(charge, "py_") {
        return Err("Stripe named a charge we can't read.".into());
    }
    let value = config
        .client()?
        .get(&format!("/v1/charges/{charge}"))
        .await?;
    if value["object"] != "charge" || value["id"] != charge {
        return Err("Stripe answered with a charge we can't use.".into());
    }
    Ok(value)
}

/// A billing portal link for the workspace's Stripe customer, where the
/// person changes their card or cancels.
///
/// # Errors
///
/// Stripe isn't set up, or Stripe refused.
pub async fn open_portal(
    config: &Config,
    customer: &str,
    idempotency: &str,
) -> Result<String, String> {
    config
        .client()?
        .portal(customer, &config.return_url, idempotency)
        .await
}

// ---------------------------------------------------------------------------
// Events.

/// What a verified Stripe event means here.
#[derive(Debug, Clone)]
pub enum Meaning {
    /// A billing-book event to journal and apply.
    Book(Box<Event>),
    /// Nothing this book holds yet; Stripe should send it again later
    /// (an invoice that arrived before its checkout completed).
    Later(String),
    /// Acknowledged; nothing to do.
    Ignored(String),
}

/// What an event owes the environment meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Meter {
    /// A paid month: record it and name the workspace that pays extras.
    Paid {
        account: String,
        workspace: String,
        plan: String,
        start: i64,
        end: i64,
    },
    /// The subscription ended at `at`.
    Ended { account: String, at: i64 },
    /// A refund or lost dispute ended the workspace's paid month at `at`
    /// and Settings says so.
    Cut {
        workspace: String,
        at: i64,
        notice: Notice,
    },
    /// Settings says (or, with `None`, stops saying) what a dispute means.
    Notice {
        workspace: String,
        notice: Option<Notice>,
    },
}

fn text(value: &Value) -> Option<String> {
    value.as_str().filter(|s| !s.is_empty()).map(str::to_string)
}

/// A subscription reference: a string, or an expanded object's id.
fn id_of(value: &Value) -> Option<String> {
    text(value).or_else(|| text(&value["id"]))
}

/// The subscription an invoice belongs to and its metadata, from either
/// API shape (`subscription` / `subscription_details` before 2025-03,
/// `parent.subscription_details` after).
fn invoice_subscription(invoice: &Value) -> (Option<String>, Value) {
    let parent = &invoice["parent"]["subscription_details"];
    let subscription = id_of(&invoice["subscription"]).or_else(|| id_of(&parent["subscription"]));
    let metadata = [
        &invoice["subscription_details"]["metadata"],
        &parent["metadata"],
    ]
    .into_iter()
    .find(|m| m.is_object())
    .cloned()
    .or_else(|| {
        invoice["lines"]["data"]
            .as_array()
            .and_then(|lines| {
                lines
                    .iter()
                    .find(|l| l["metadata"]["oa_account"].is_string())
            })
            .map(|l| l["metadata"].clone())
    })
    .unwrap_or(Value::Null);
    (subscription, metadata)
}

/// The service period an invoice pays for: its subscription line's.
fn invoice_period(invoice: &Value) -> Option<(i64, i64)> {
    let lines = invoice["lines"]["data"].as_array()?;
    lines
        .iter()
        .filter_map(|l| Some((l["period"]["start"].as_i64()?, l["period"]["end"].as_i64()?)))
        .filter(|(s, e)| e > s)
        .max_by_key(|(_, e)| *e)
}

fn blank(kind: &str, id: &str) -> Event {
    Event {
        provider: "stripe".into(),
        id: id.into(),
        kind: kind.into(),
        checkout: None,
        subscription: None,
        invoice: None,
        period: 0,
        amount: 0,
        currency: None,
        at_period_end: false,
        provider_ref: None,
        provider_subscription: None,
        customer: None,
        charges: Vec::new(),
        received: 0,
        applied: false,
        outcome: String::new(),
    }
}

/// Read a verified Stripe event against the book: what it means to the
/// book, and what it owes the environment meter.
#[must_use]
pub fn read(
    event: &Value,
    book: &BillingBook,
    plans: &[Plan],
    now: u64,
) -> (Meaning, Option<Meter>) {
    let id = event["id"].as_str().unwrap_or_default();
    let object = &event["data"]["object"];
    let kind = event["type"].as_str().unwrap_or_default();
    match kind {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            if object["mode"] != "subscription" {
                return (Meaning::Ignored("not a subscription checkout".into()), None);
            }
            if !matches!(
                object["payment_status"].as_str(),
                Some("paid" | "no_payment_required")
            ) {
                return (Meaning::Ignored("not paid yet".into()), None);
            }
            let Some(checkout) = text(&object["metadata"]["oa_checkout"])
                .or_else(|| text(&object["client_reference_id"]))
            else {
                return (Meaning::Ignored("not one of our checkouts".into()), None);
            };
            let mut e = blank("checkout-completed", id);
            e.checkout = Some(checkout);
            e.provider_ref = text(&object["id"]);
            e.provider_subscription = id_of(&object["subscription"]);
            e.customer = id_of(&object["customer"]);
            (Meaning::Book(Box::new(e)), None)
        }
        "invoice.paid" | "invoice.payment_failed" => {
            let (subscription, metadata) = invoice_subscription(object);
            let Some(subscription) = subscription else {
                return (Meaning::Ignored("not a subscription invoice".into()), None);
            };
            let paid = kind == "invoice.paid";
            let meter = match (
                paid,
                text(&metadata["oa_account"]),
                text(&metadata["oa_workspace"]),
                text(&metadata["oa_plan"]),
                invoice_period(object),
            ) {
                (true, Some(account), Some(workspace), Some(plan), Some((start, end)))
                    if plans
                        .iter()
                        .any(|p| p.id == plan && p.environments.is_some()) =>
                {
                    Some(Meter::Paid {
                        account,
                        workspace,
                        plan,
                        start,
                        end,
                    })
                }
                _ => None,
            };
            let Some(sub) = book.subscription_by_ref(&subscription) else {
                return (
                    Meaning::Later(format!(
                        "subscription {subscription} isn't started here yet"
                    )),
                    meter,
                );
            };
            let invoice = text(&object["id"]).unwrap_or_default();
            let first = object["billing_reason"] == "subscription_create";
            let settled = book
                .invoices
                .get(&invoice)
                .is_some_and(|i| i.state == tenancy::billing::InvoiceState::Paid);
            if paid && !first && !settled && sub.dispute.is_some() {
                // New paid months wait for the dispute to close. Nothing
                // is journaled or recorded, so Stripe's retry (or a resend
                // from its Dashboard) lands once the dispute is closed.
                return (
                    Meaning::Later(
                        "a dispute is open on this subscription; the new month starts once it closes"
                            .into(),
                    ),
                    None,
                );
            }
            let mut e = blank(
                if paid {
                    "invoice-paid"
                } else {
                    "invoice-failed"
                },
                id,
            );
            e.subscription = Some(sub.id.clone());
            e.invoice = Some(invoice.clone());
            e.provider_ref = Some(invoice);
            e.period = if first || settled {
                sub.period
            } else {
                sub.period.saturating_add(1)
            };
            let cents = object[if paid { "amount_paid" } else { "amount_due" }]
                .as_u64()
                .unwrap_or(0);
            e.amount = cents.saturating_mul(10_000);
            e.currency = text(&object["currency"]).map(|c| c.to_ascii_uppercase());
            if paid {
                e.charges = payment_refs(object);
            }
            (Meaning::Book(Box::new(e)), meter)
        }
        "customer.subscription.deleted" => {
            let Some(subscription) = text(&object["id"]) else {
                return (Meaning::Ignored("no subscription".into()), None);
            };
            let at = object["ended_at"]
                .as_i64()
                .or_else(|| object["canceled_at"].as_i64())
                .unwrap_or(now as i64);
            let meter =
                text(&object["metadata"]["oa_account"]).map(|account| Meter::Ended { account, at });
            let Some(sub) = book.subscription_by_ref(&subscription) else {
                return (
                    Meaning::Ignored("not a subscription this book holds".into()),
                    meter,
                );
            };
            let mut e = blank("subscription-cancelled", id);
            e.subscription = Some(sub.id.clone());
            e.provider_ref = Some(subscription);
            (Meaning::Book(Box::new(e)), meter)
        }
        "charge.refunded" | "charge.dispute.created" | "charge.dispute.closed" => {
            payment_event(event, book, now)
        }
        other => (Meaning::Ignored(format!("`{other}` isn't used here")), None),
    }
}

/// The charge and payment-intent references an invoice carries, in either
/// API shape (`charge` / `payment_intent` before 2025-03, `payments`
/// after), so a later refund or dispute finds the invoice.
fn payment_refs(invoice: &Value) -> Vec<String> {
    let mut refs: Vec<String> = [&invoice["charge"], &invoice["payment_intent"]]
        .into_iter()
        .filter_map(id_of)
        .collect();
    if let Some(payments) = invoice["payments"]["data"].as_array() {
        for p in payments {
            refs.extend(
                [&p["payment"]["charge"], &p["payment"]["payment_intent"]]
                    .into_iter()
                    .filter_map(id_of),
            );
        }
    }
    refs.retain(|r| r.len() <= 255);
    refs.sort();
    refs.dedup();
    refs.truncate(8);
    refs
}

/// The charge a refund or dispute event is about: the charge itself, or
/// the one fetched for a dispute (`oa_charge`), with the references a
/// dispute names directly.
fn charge_of(event: &Value) -> (Value, Vec<String>) {
    let object = &event["data"]["object"];
    if object["object"] == "dispute" {
        let mut refs: Vec<String> = [&object["charge"], &object["payment_intent"]]
            .into_iter()
            .filter_map(id_of)
            .collect();
        let fetched = object["oa_charge"].clone();
        refs.extend(id_of(&fetched["payment_intent"]));
        return (fetched, refs);
    }
    let refs = [&object["id"], &object["payment_intent"]]
        .into_iter()
        .filter_map(id_of)
        .collect();
    (object.clone(), refs)
}

/// The invoice a charge paid: by the invoice it names, by the payment
/// references recorded when it was paid, or (for a charge that names its
/// customer) the subscription's paid invoice for that amount.
fn invoice_of_charge<'b>(book: &'b BillingBook, event: &Value) -> Option<&'b Invoice> {
    let (charge, refs) = charge_of(event);
    if let Some(invoice) = id_of(&charge["invoice"]).and_then(|id| book.invoices.get(&id)) {
        return Some(invoice);
    }
    if let Some(invoice) = book
        .invoices
        .values()
        .find(|i| i.charges.iter().any(|c| refs.contains(c)))
    {
        return Some(invoice);
    }
    let customer = id_of(&charge["customer"])?;
    let cents = charge["amount"].as_u64()?;
    let subscription = book
        .subscriptions
        .values()
        .find(|s| s.customer.as_deref() == Some(customer.as_str()))?;
    let paid_for_amount = |open_only: bool| {
        book.invoices
            .values()
            .filter(|i| {
                i.subscription.as_deref() == Some(subscription.id.as_str())
                    && i.amount == cents.saturating_mul(10_000)
                    && (!open_only
                        || matches!(i.state, InvoiceState::Paid | InvoiceState::Disputed))
            })
            .max_by_key(|i| i.period)
    };
    // A paid invoice first; a closed one only so a repeated event is
    // recognised as the repeat it is.
    paid_for_amount(true).or_else(|| paid_for_amount(false))
}

/// The charge to read from Stripe before a dispute event can be placed:
/// the dispute names only its charge, and the book doesn't know it yet.
#[must_use]
pub fn charge_to_fetch(event: &Value, book: &BillingBook) -> Option<String> {
    let kind = event["type"].as_str()?;
    if !matches!(kind, "charge.dispute.created" | "charge.dispute.closed") {
        return None;
    }
    if invoice_of_charge(book, event).is_some() {
        return None;
    }
    id_of(&event["data"]["object"]["charge"])
}

/// Whether the book has already applied this event: a duplicate must not
/// touch the meter again.
fn applied(book: &BillingBook, id: &str) -> bool {
    book.events
        .get(&format!("stripe:{id}"))
        .is_some_and(|e| e.applied)
}

/// A refund or dispute on a subscription payment (#11074).
///
/// - `charge.refunded`, fully refunded: the invoice closes, the allowance
///   it granted is taken back (never more than is unspent), and when it
///   paid the month the subscription stands in, that month ends now. A
///   partial refund changes nothing: the month stands.
/// - `charge.dispute.created`: the subscription is at risk and no new paid
///   month starts until the dispute closes. The paid month runs on and
///   nothing is taken back yet.
/// - `charge.dispute.closed`, won (or a warning closed): the payment
///   stands and new months may start. Lost: like a full refund, once.
fn payment_event(event: &Value, book: &BillingBook, now: u64) -> (Meaning, Option<Meter>) {
    let id = event["id"].as_str().unwrap_or_default();
    let kind = event["type"].as_str().unwrap_or_default();
    let object = &event["data"]["object"];
    let at = event["created"].as_i64().unwrap_or(now as i64);
    let (book_kind, dispute) = match kind {
        "charge.refunded" => {
            let cents = object["amount"].as_u64().unwrap_or(0);
            let full = object["refunded"] == true
                || (cents > 0 && object["amount_refunded"].as_u64().unwrap_or(0) >= cents);
            if !full {
                return (
                    Meaning::Ignored("a partial refund; the paid month stands".into()),
                    None,
                );
            }
            ("charge-refunded", None)
        }
        "charge.dispute.created" => ("dispute-opened", text(&object["id"])),
        _ => match object["status"].as_str() {
            Some("won" | "warning_closed") => ("dispute-won", text(&object["id"])),
            Some("lost") => ("dispute-lost", text(&object["id"])),
            _ => {
                return (
                    Meaning::Ignored("a dispute that isn't settled yet".into()),
                    None,
                );
            }
        },
    };
    if kind != "charge.refunded" && dispute.is_none() {
        return (Meaning::Ignored("a dispute without an id".into()), None);
    }
    let Some(invoice) = invoice_of_charge(book, event) else {
        return (
            Meaning::Ignored("not a charge on a subscription invoice".into()),
            None,
        );
    };
    let Some(sub) = invoice
        .subscription
        .as_ref()
        .and_then(|id| book.subscriptions.get(id))
    else {
        return (
            Meaning::Ignored("not a charge on a subscription invoice".into()),
            None,
        );
    };
    let current = invoice.period == sub.period
        && matches!(
            sub.state,
            SubscriptionState::Active | SubscriptionState::PastDue
        );
    let workspace = invoice.workspace.clone();
    let owed = if applied(book, id) {
        None
    } else {
        match book_kind {
            "charge-refunded" if current && invoice.state != InvoiceState::Refunded => {
                Some(Meter::Cut {
                    workspace,
                    at,
                    notice: Notice::Refunded,
                })
            }
            "dispute-lost" if current && invoice.state != InvoiceState::DisputeLost => {
                Some(Meter::Cut {
                    workspace,
                    at,
                    notice: Notice::DisputeLost,
                })
            }
            "dispute-opened" if invoice.state == InvoiceState::Paid => Some(Meter::Notice {
                workspace,
                notice: Some(Notice::Dispute),
            }),
            "dispute-won" if invoice.state == InvoiceState::Disputed => Some(Meter::Notice {
                workspace,
                notice: None,
            }),
            _ => None,
        }
    };
    let mut e = blank(book_kind, id);
    e.subscription = Some(sub.id.clone());
    e.invoice = Some(invoice.id.clone());
    // A dispute event's reference is the dispute; a refund's, the charge.
    e.provider_ref = dispute.or_else(|| text(&object["id"]));
    (Meaning::Book(Box::new(e)), owed)
}

/// Apply what an event owes the environment meter.
///
/// # Errors
///
/// The meter can't be opened or written.
pub fn meter(path: &std::path::Path, owed: &Meter) -> Result<(), String> {
    use retail_cloud::environment;
    let mut journal = retail_cloud::journal::Journal::open(path)
        .map_err(|e| format!("environment meter: {e}"))?;
    match owed {
        Meter::Paid {
            account,
            workspace,
            start,
            end,
            ..
        } => {
            environment::record_period(
                &mut journal,
                &environment::Period {
                    account: account.clone(),
                    plan: environment::plan().version,
                    start: *start,
                    end: *end,
                },
            )
            .map_err(|e| format!("environment meter: {e}"))?;
            environment::set_credits_account(&mut journal, account, workspace)
                .map_err(|e| format!("environment meter: {e}"))?;
            // A month paid after a refund or a dispute is a fresh start.
            environment::set_notice(&mut journal, account, None)
                .map_err(|e| format!("environment meter: {e}"))
        }
        Meter::Cut {
            workspace,
            at,
            notice,
        } => {
            let account = environment::account_paid_by(&journal, workspace)
                .map_err(|e| format!("environment meter: {e}"))?;
            let Some(account) = account else {
                return Ok(());
            };
            environment::end_period(&mut journal, &account, *at)
                .map_err(|e| format!("environment meter: {e}"))?;
            environment::set_notice(&mut journal, &account, Some(*notice))
                .map_err(|e| format!("environment meter: {e}"))
        }
        Meter::Notice { workspace, notice } => {
            let account = environment::account_paid_by(&journal, workspace)
                .map_err(|e| format!("environment meter: {e}"))?;
            let Some(account) = account else {
                return Ok(());
            };
            environment::set_notice(&mut journal, &account, *notice)
                .map_err(|e| format!("environment meter: {e}"))
        }
        Meter::Ended { account, at } => environment::end_period(&mut journal, account, *at)
            .map(|_| ())
            .map_err(|e| format!("environment meter: {e}")),
    }
}

// ---------------------------------------------------------------------------
// Extra-hour charges into the credits ledger.

/// The money ledger as the environment meter's [`Credits`]: each charge is
/// one reserve and one settle on the paying workspace, keyed on the charge,
/// so offering it again changes nothing.
///
/// [`Credits`]: retail_cloud::environment::Credits
pub struct LedgerCredits<'a> {
    pub ledger: &'a mut Ledger,
    /// Account to the workspace whose credits pay its extra hours.
    pub payers: BTreeMap<String, String>,
    pub currency: String,
}

impl retail_cloud::environment::Credits for LedgerCredits<'_> {
    fn debit(&mut self, d: &retail_cloud::environment::Debit) -> Result<(), String> {
        let workspace = self
            .payers
            .get(&d.account)
            .ok_or("no workspace pays this account's extra hours")?
            .clone();
        if d.usd_micros == 0 {
            return Ok(());
        }
        if let Some(hold) = self.ledger.hold(&workspace, &d.key) {
            if hold.phase == Phase::Settled {
                return Ok(());
            }
        }
        // The charge was already capped when it was counted; the price is
        // that charge spread over the extra milliseconds it paid for.
        let ms = d.extra_seconds.saturating_mul(1000).max(1);
        let usage: tenancy::money::Usage = [(Resource::ComputeMilliseconds, ms)].into();
        let price = Price {
            version: format!("{}:extra-hours", d.key),
            currency: self.currency.clone(),
            model: retail_cloud::environment::COMPUTER_CLASS.into(),
            capacity: "extra-hours".into(),
            policy: "environment-extra-hours-v1".into(),
            rates: [(
                Resource::ComputeMilliseconds,
                Rate {
                    millionths: d.usd_micros,
                    per_units: ms,
                },
            )]
            .into(),
        };
        let audit = format!("environment extra hours {}", d.key);
        self.ledger.apply(Mutation {
            workspace: workspace.clone(),
            source: format!("{}:reserve", d.key),
            audit: audit.clone(),
            operation: Operation::Reserve {
                attempt: d.key.clone(),
                request_digest: d.key.clone(),
                price,
                maximum_usage: usage.clone(),
            },
        })?;
        self.ledger.apply(Mutation {
            workspace,
            source: format!("{}:settle", d.key),
            audit,
            operation: Operation::Settle {
                attempt: d.key.clone(),
                usage,
                receipt: d.key.clone(),
                provider_cost: None,
                hosting_cost: None,
            },
        })?;
        Ok(())
    }
}

/// Offer every waiting extra-hour charge in `meter_path` to `ledger` once.
/// A charge the ledger can't take yet (no credits) stays waiting.
///
/// # Errors
///
/// The meter can't be read or written.
pub fn post_debits(
    meter_path: &std::path::Path,
    ledger: &mut Ledger,
    currency: &str,
    now: i64,
) -> Result<Vec<String>, String> {
    use retail_cloud::environment;
    let mut journal = retail_cloud::journal::Journal::open(meter_path)
        .map_err(|e| format!("environment meter: {e}"))?;
    let waiting = environment::debits(&journal).map_err(|e| e.to_string())?;
    let mut payers = BTreeMap::new();
    for d in waiting.iter().filter(|d| d.posted_at.is_none()) {
        if let Some(ws) =
            environment::credits_account(&journal, &d.account).map_err(|e| e.to_string())?
        {
            payers.insert(d.account.clone(), ws);
        }
    }
    let mut credits = LedgerCredits {
        ledger,
        payers,
        currency: currency.into(),
    };
    environment::post_debits(&mut journal, &mut credits, now).map_err(|e| e.to_string())
}

/// The currency the environment plan charges in.
fn environment_currency(plans: &[Plan]) -> Option<String> {
    plans
        .iter()
        .find(|p| p.environments.is_some())
        .map(|p| p.price.currency.clone())
}

/// Post waiting extra-hour charges now, when this deployment meters
/// environments. Returns the charges posted.
pub(crate) async fn post_now(state: &ServeState) -> Vec<String> {
    let Some(billing) = &state.config.billing else {
        return vec![];
    };
    let (Some(stripe), Some(currency)) = (&billing.stripe, environment_currency(&billing.plans))
    else {
        return vec![];
    };
    let Some(path) = &stripe.environment_meter else {
        return vec![];
    };
    let Some(mut ledger) = state.money_lock().await else {
        return vec![];
    };
    let now = crate::accounts::unix_now() as i64;
    post_debits(path, &mut ledger, &currency, now).unwrap_or_default()
}

/// Keep posting extra-hour charges once a minute while the server runs.
pub(crate) fn resume(state: &Arc<ServeState>) {
    let metered = state
        .config
        .billing
        .as_ref()
        .and_then(|b| b.stripe.as_ref())
        .is_some_and(|s| s.environment_meter.is_some());
    if !metered || tokio::runtime::Handle::try_current().is_err() {
        return;
    }
    let state = state.clone();
    tokio::spawn(async move {
        loop {
            post_now(&state).await;
            tokio::time::sleep(POST_EVERY).await;
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    use retail_cloud::authority::Source;
    use retail_cloud::environment::{
        self as env, Ending, EnvironmentRequest, ExtraHours, Gate, Period, PlanStatus, Usage,
    };
    use retail_cloud::journal::Journal;

    const NOW: i64 = 1_791_200_000;
    const HOUR: u64 = 3600;

    fn run(j: &mut Journal, id: &str, seconds: u64) {
        let mut p = env::plan();
        p.status = PlanStatus::Published;
        let g = Gate {
            contract_reviewed: true,
            plan: Some(p.digest()),
            qualification: Some("fixture".into()),
        };
        let request = EnvironmentRequest {
            source: Source {
                repository: "https://github.com/OpenAgentsInc/example".into(),
                commit: "c".repeat(40),
            },
            objective: "Build it.".into(),
            profile: "rust-library".into(),
            checks: vec!["cargo test".into()],
            max_seconds: None,
        };
        let o = env::offer(j, &p, &g, "acct", id, &request, NOW).unwrap();
        env::confirm(j, &p, &g, "acct", id, &o.digest, true, NOW).unwrap();
        env::end(
            j,
            &p,
            id,
            Ending::Ended,
            Some(Usage {
                machine_seconds: seconds,
                image_gb: None,
            }),
            None,
            NOW + 1,
        )
        .unwrap();
        env::settle(j, id, NOW + 2).unwrap();
    }

    fn credit(ledger: &mut Ledger, source: &str, amount: u64) {
        ledger
            .apply(Mutation {
                workspace: "ws".into(),
                source: source.into(),
                audit: "fixture credit".into(),
                operation: Operation::Credit {
                    amount,
                    credit_kind: tenancy::money::CreditKind::TopUp,
                },
            })
            .unwrap();
    }

    #[test]
    fn extra_hour_charges_land_once_in_the_credits_ledger_up_to_the_cap() {
        let dir = tempfile::tempdir().unwrap();
        let meter_path = dir.path().join("meter.sqlite");
        let mut j = Journal::open(&meter_path).unwrap();
        env::record_period(
            &mut j,
            &Period {
                account: "acct".into(),
                plan: env::plan().version,
                start: NOW - 86_400,
                end: NOW + 29 * 86_400,
            },
        )
        .unwrap();
        env::set_extra_hours(
            &mut j,
            "acct",
            ExtraHours {
                enabled: true,
                cap_usd_micros: 1_000_000,
            },
        )
        .unwrap();
        // 101 hours: one extra hour, $0.18. Then 10 more: capped at $0.82.
        run(&mut j, "p1", 101 * HOUR);
        run(&mut j, "p2", 10 * HOUR);
        drop(j);
        let mut ledger = Ledger::open(&dir.path().join("money.jsonl")).unwrap();
        ledger
            .apply(Mutation {
                workspace: "ws".into(),
                source: "create".into(),
                audit: "fixture account".into(),
                operation: Operation::Create {
                    currency: "USD".into(),
                    spend_limit: u64::MAX,
                    topups_allowed: true,
                },
            })
            .unwrap();
        credit(&mut ledger, "c1", 500_000);
        // No workspace named yet: nothing leaves.
        assert!(
            post_debits(&meter_path, &mut ledger, "USD", NOW)
                .unwrap()
                .is_empty()
        );
        let mut j = Journal::open(&meter_path).unwrap();
        env::set_credits_account(&mut j, "acct", "ws").unwrap();
        drop(j);
        // $0.50 pays the first charge; the second waits for credits.
        assert_eq!(
            post_debits(&meter_path, &mut ledger, "USD", NOW).unwrap(),
            vec!["env:p1".to_string()]
        );
        assert_eq!(ledger.balance("ws").unwrap().settled, 180_000);
        credit(&mut ledger, "c2", 1_000_000);
        assert_eq!(
            post_debits(&meter_path, &mut ledger, "USD", NOW).unwrap(),
            vec!["env:p2".to_string()]
        );
        assert!(
            post_debits(&meter_path, &mut ledger, "USD", NOW)
                .unwrap()
                .is_empty()
        );
        let balance = ledger.balance("ws").unwrap();
        assert_eq!(balance.settled, 1_000_000, "never past the $1 cap");
        assert_eq!(balance.available, 500_000);
        // Offering a posted charge again (a lost posted mark) is one charge.
        let j = Journal::open(&meter_path).unwrap();
        let again = env::debits(&j).unwrap();
        let mut credits = LedgerCredits {
            ledger: &mut ledger,
            payers: [("acct".to_string(), "ws".to_string())].into(),
            currency: "USD".into(),
        };
        for d in &again {
            env::Credits::debit(&mut credits, d).unwrap();
        }
        assert_eq!(ledger.balance("ws").unwrap().settled, 1_000_000);
    }

    #[test]
    fn invoices_read_in_both_api_shapes() {
        let old = serde_json::json!({
            "subscription": "sub_1",
            "subscription_details": {"metadata": {"oa_account": "a"}},
            "lines": {"data": [{"period": {"start": 10, "end": 20}}]}
        });
        let (sub, meta) = invoice_subscription(&old);
        assert_eq!(sub.as_deref(), Some("sub_1"));
        assert_eq!(meta["oa_account"], "a");
        assert_eq!(invoice_period(&old), Some((10, 20)));
        let new = serde_json::json!({
            "parent": {"subscription_details": {"subscription": "sub_2", "metadata": {"oa_account": "b"}}},
            "lines": {"data": [{"period": {"start": 30, "end": 40}}]}
        });
        let (sub, meta) = invoice_subscription(&new);
        assert_eq!(sub.as_deref(), Some("sub_2"));
        assert_eq!(meta["oa_account"], "b");
    }
}
