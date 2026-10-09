//! Native card-provider verification. Verified provider objects are evidence;
//! the billing adapter must bind them to an admitted checkout before funding.

use hmac::{Hmac, Mac};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::time::Duration;

const MAX_BODY: usize = 512 * 1024;
const MAX_HEADER: usize = 4096;
const API_ORIGIN: &str = "https://api.stripe.com";

mod config;
pub(crate) mod controller;
pub use config::Config;
pub(crate) mod browser;
mod browser_csrf;
mod checkout;
pub use checkout::CheckoutRequest;
mod adjustments;
pub use adjustments::Adjustments;
mod collection;
pub use collection::{Collection, Original};

/// A bounded, scrubbed webhook identity. No card or customer payload is retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Event {
    pub id: String,
    pub kind: String,
    pub object: String,
    pub created_at: u64,
    pub body_sha256: String,
}

fn refusal() -> String {
    "The native Stripe evidence could not be verified.".into()
}

fn identifier(value: &str, prefix: &str) -> Result<String, String> {
    // Current native dispute objects use du_; retain the earlier dp_ family.
    // Both still require the exact authenticated dispute object and reference.
    let prefix = if prefix == "dp_" && value.starts_with("du_") {
        "du_"
    } else {
        prefix
    };
    let suffix = value.strip_prefix(prefix).ok_or_else(refusal)?;
    if suffix.is_empty()
        || value.len() > 128
        || !suffix
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    {
        return Err(refusal());
    }
    Ok(value.into())
}

fn strict(bytes: &[u8]) -> Result<Value, String> {
    if bytes.len() > MAX_BODY {
        return Err(refusal());
    }
    nostr::contracts::parse_strict(bytes).map_err(|_| refusal())
}

/// Verify raw bytes before parsing. Accept any matching v1 tag during provider
/// signing-secret rotation, but reject ambiguous timestamps and stale deliveries.
pub fn verify_webhook(
    body: &[u8],
    header: &str,
    secret: &[u8],
    now: u64,
    tolerance: u64,
    api_version: &str,
    live: bool,
) -> Result<Event, String> {
    verify_signature(body, header, secret, now, tolerance)?;
    event(&strict(body)?, body, api_version, live)
}

/// Check a `Stripe-Signature` header (`t=<unix>,v1=<hex>[,v1=…]`) over the
/// raw body: any matching `v1` tag within `tolerance` seconds of `now`.
pub(crate) fn verify_signature(
    body: &[u8],
    header: &str,
    secret: &[u8],
    now: u64,
    tolerance: u64,
) -> Result<(), String> {
    if body.len() > MAX_BODY
        || header.len() > MAX_HEADER
        || secret.len() < 16
        || secret.len() > 1024
        || !(1..=300).contains(&tolerance)
    {
        return Err(refusal());
    }
    let mut timestamp = None;
    let mut tags = Vec::new();
    for part in header.split(',').map(str::trim) {
        if let Some(value) = part.strip_prefix("t=") {
            if timestamp.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(refusal());
            }
            timestamp = Some((value, value.parse::<u64>().map_err(|_| refusal())?));
        } else if let Some(value) = part.strip_prefix("v1=") {
            if value.len() != 64 || tags.len() >= 16 {
                return Err(refusal());
            }
            let tag = value
                .as_bytes()
                .chunks_exact(2)
                .map(|pair| {
                    std::str::from_utf8(pair)
                        .ok()
                        .and_then(|text| u8::from_str_radix(text, 16).ok())
                        .ok_or_else(refusal)
                })
                .collect::<Result<Vec<_>, _>>()?;
            tags.push(tag);
        }
    }
    let (raw_timestamp, timestamp) = timestamp.ok_or_else(refusal)?;
    if now.abs_diff(timestamp) > tolerance || tags.is_empty() {
        return Err(refusal());
    }
    let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(secret).map_err(|_| refusal())?;
    mac.update(raw_timestamp.as_bytes());
    mac.update(b".");
    mac.update(body);
    if !tags.iter().any(|tag| mac.clone().verify_slice(tag).is_ok()) {
        return Err(refusal());
    }
    Ok(())
}

fn event(value: &Value, body: &[u8], api_version: &str, live: bool) -> Result<Event, String> {
    if value["object"] != "event"
        || value["api_version"] != api_version
        || value["livemode"] != live
        || !value["account"].is_null()
        || !value["context"].is_null()
    {
        return Err(refusal());
    }
    let kind = value["type"].as_str().ok_or_else(refusal)?;
    let (object_type, prefix) = match kind {
        "checkout.session.completed" | "checkout.session.async_payment_succeeded" => {
            ("checkout.session", "cs_")
        }
        "payment_intent.succeeded" => ("payment_intent", "pi_"),
        "charge.refunded" | "charge.succeeded" => ("charge", "ch_"),
        "refund.created" | "refund.updated" => ("refund", "re_"),
        "charge.dispute.created"
        | "charge.dispute.updated"
        | "charge.dispute.closed"
        | "charge.dispute.funds_withdrawn"
        | "charge.dispute.funds_reinstated" => ("dispute", "dp_"),
        _ => return Err("This Stripe event is outside the admitted prepaid funding lane.".into()),
    };
    let object = &value["data"]["object"];
    if object["object"] != object_type {
        return Err(refusal());
    }
    Ok(Event {
        id: identifier(value["id"].as_str().ok_or_else(refusal)?, "evt_")?,
        kind: kind.into(),
        object: identifier(object["id"].as_str().ok_or_else(refusal)?, prefix)?,
        created_at: value["created"].as_u64().ok_or_else(refusal)?,
        body_sha256: format!("{:x}", Sha256::digest(body)),
    })
}

/// The single native API origin. Redirects, ambient proxies, response payload
/// logs, and caller-supplied URLs cannot disclose the restricted credential.
pub struct Stripe {
    client: reqwest::Client,
    key: String,
    version: String,
    origin: String,
    mode: Option<bool>,
    verified_account: Option<String>,
}

impl Stripe {
    pub fn new(key: String, version: String) -> Result<Self, String> {
        if key.is_empty()
            || key.len() > 1024
            || key.bytes().any(|b| !b.is_ascii_graphic())
            || version.is_empty()
            || version.len() > 64
            || !version
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
        {
            return Err(refusal());
        }
        let client = reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(5))
            .timeout(Duration::from_secs(15))
            .build()
            .map_err(|_| refusal())?;
        Ok(Self {
            client,
            key,
            version,
            origin: API_ORIGIN.into(),
            mode: None,
            verified_account: None,
        })
    }

    /// Bind the deployment's restricted credential to its admitted native mode.
    /// A test credential cannot normalize live evidence or fund a live lane.
    pub fn new_for_mode(key: String, version: String, live: bool) -> Result<Self, String> {
        let prefix = if live { "rk_live_" } else { "rk_test_" };
        let suffix = key.strip_prefix(prefix).ok_or_else(refusal)?;
        if suffix.len() < 16
            || !suffix
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
        {
            return Err(refusal());
        }
        let mut client = Self::new(key, version)?;
        client.mode = Some(live);
        Ok(client)
    }

    fn check_mode(&self, value: &Value) -> Result<(), String> {
        if self
            .mode
            .is_some_and(|live| value["livemode"].as_bool() != Some(live))
        {
            return Err(refusal());
        }
        Ok(())
    }

    /// Bind one immutable native merchant identity before creating anything.
    /// Creation rechecks that account through the same restricted credential.
    pub async fn bind_account(&mut self, expected: &str) -> Result<(), String> {
        if self.mode.is_none()
            || self
                .verified_account
                .as_ref()
                .is_some_and(|prior| prior != expected)
        {
            return Err(refusal());
        }
        self.account(expected).await?;
        self.verified_account = Some(expected.into());
        Ok(())
    }

    async fn admitted_account(&self) -> Result<(), String> {
        let expected = self.verified_account.as_ref().ok_or_else(refusal)?;
        self.account(expected).await
    }

    /// Retrieve only known native resources. The billing journal supplies the
    /// original reference; an event cannot select a different API or account.
    pub async fn get(&self, resource: &str, id: &str) -> Result<Value, String> {
        let prefix = match resource {
            "events" => "evt_",
            "checkout/sessions" => "cs_",
            "payment_intents" => "pi_",
            "charges" => "ch_",
            "balance_transactions" => "txn_",
            "refunds" => "re_",
            "disputes" => "dp_",
            "customers" => "cus_",
            _ => return Err(refusal()),
        };
        identifier(id, prefix)?;
        let value = self.read(&format!("/v1/{resource}/{id}")).await?;
        let object = match resource {
            "events" => "event",
            "checkout/sessions" => "checkout.session",
            "payment_intents" => "payment_intent",
            "charges" => "charge",
            "balance_transactions" => "balance_transaction",
            "refunds" => "refund",
            "disputes" => "dispute",
            "customers" => "customer",
            _ => unreachable!(),
        };
        if value["object"] != object || value["id"] != id {
            return Err(refusal());
        }
        if matches!(
            resource,
            "events"
                | "checkout/sessions"
                | "payment_intents"
                | "charges"
                | "disputes"
                | "customers"
        ) {
            self.check_mode(&value)?;
        }
        Ok(value)
    }

    /// Verify the credential's own native account before binding any checkout.
    pub async fn account(&self, expected: &str) -> Result<(), String> {
        identifier(expected, "acct_")?;
        let account = self.read("/v1/account").await?;
        if account["object"] != "account" || account["id"] != expected {
            return Err(refusal());
        }
        Ok(())
    }

    async fn read(&self, path: &str) -> Result<Value, String> {
        self.exchange(self.client.get(format!("{}{path}", self.origin)))
            .await
    }

    async fn exchange(&self, request: reqwest::RequestBuilder) -> Result<Value, String> {
        let mut response = request
            .bearer_auth(&self.key)
            .header("Stripe-Version", &self.version)
            .send()
            .await
            .map_err(|_| refusal())?;
        if !response.status().is_success()
            || response
                .content_length()
                .is_some_and(|n| n > MAX_BODY as u64)
        {
            return Err(refusal());
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| refusal())? {
            if bytes.len().saturating_add(chunk.len()) > MAX_BODY {
                return Err(refusal());
            }
            bytes.extend_from_slice(&chunk);
        }
        strict(&bytes)
    }
}

#[cfg(test)]
mod tests;
