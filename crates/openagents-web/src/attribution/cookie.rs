//! The `oa_ref` cookie: `v1.PAYLOAD.SIGNATURE`, the payload base64url
//! JSON, the signature HMAC-SHA256 over `v1.PAYLOAD` with a key derived
//! from the server's secret for this use only.

use axum::http::{HeaderMap, header};
use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{COOKIE, Link, WINDOW};

/// Why the browser may keep the click.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    /// The visitor said yes on the consent page.
    Consent,
    /// The visitor's country needs no consent; the privacy policy says it.
    Notice,
}

/// What the cookie holds: the link (a campaign code, or an affiliate's
/// token), the series, the click's id and time, and its basis.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ticket {
    #[serde(rename = "c", default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,
    #[serde(rename = "t", default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
    #[serde(rename = "s", default, skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    #[serde(rename = "i")]
    pub click: String,
    #[serde(rename = "at")]
    pub at: u64,
    #[serde(rename = "b")]
    pub basis: Basis,
}

impl Ticket {
    pub fn link(&self) -> Link {
        if self.token.is_some() {
            Link::Affiliate
        } else {
            Link::Go
        }
    }

    /// The name the scoreboard counts this click under.
    pub fn key(&self) -> String {
        match (&self.code, &self.token) {
            (Some(code), _) => code.clone(),
            (None, Some(token)) => affiliate_key(token),
            (None, None) => "unknown".into(),
        }
    }

    fn valid(&self) -> bool {
        use tenancy::accounts::referrals::{campaign_word, click_id};
        let link_ok = match (&self.code, &self.token) {
            (Some(code), None) => super::codes::find(code).is_some(),
            (None, Some(token)) => super::affiliate_token(token),
            _ => false,
        };
        link_ok && self.series.as_deref().is_none_or(campaign_word) && click_id(&self.click)
    }

    /// The source and click the account service records with a new
    /// account: a source request named after the click, the affiliate's
    /// token (when there is one) and the click itself.
    pub fn signup(
        &self,
    ) -> (
        tenancy::accounts::referrals::Capture,
        tenancy::accounts::referrals::Campaign,
    ) {
        use tenancy::accounts::referrals::{self as r, Campaign, Capture, LinkKind};
        (
            Capture {
                request: format!("signup-{}", self.click),
                token: self.token.clone(),
                consent: true,
                consent_version: Some(r::CONSENT.into()),
            },
            Campaign {
                link: match self.link() {
                    Link::Go => LinkKind::Go,
                    Link::Affiliate => LinkKind::Affiliate,
                },
                code: self.code.clone(),
                series: self.series.clone(),
                click: self.click.clone(),
                clicked_at: self.at,
                basis: match self.basis {
                    Basis::Consent => r::Basis::Consent,
                    Basis::Notice => r::Basis::Notice,
                },
            },
        )
    }
}

/// `aff-` and the first 16 hex digits of the token's SHA-256. The
/// referral system keys its links by the same hash, so a scoreboard row can
/// be matched to its affiliate without keeping the token.
pub fn affiliate_key(token: &str) -> String {
    let digest = Sha256::digest(token.as_bytes());
    let hex: String = digest.iter().take(8).map(|b| format!("{b:02x}")).collect();
    format!("aff-{hex}")
}

/// A fresh click id: `clk_` and 32 random hex digits.
pub fn click_id() -> String {
    let bytes: [u8; 16] = secp256k1::rand::random();
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!("clk_{hex}")
}

/// The cookie key, derived from the server's secret for this use only.
pub fn derive_key(salt: &[u8; 32]) -> [u8; 32] {
    let mut mac = Hmac::<Sha256>::new_from_slice(salt).expect("HMAC accepts 32 bytes");
    mac.update(b"openagents.oa_ref.v1");
    mac.finalize().into_bytes().into()
}

fn sign(key: &[u8; 32], body: &str) -> Hmac<Sha256> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts 32 bytes");
    mac.update(body.as_bytes());
    mac
}

/// The cookie's value for `ticket`.
pub fn encode(ticket: &Ticket, key: &[u8; 32]) -> String {
    let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(ticket).unwrap_or_default());
    let body = format!("v1.{payload}");
    let signature = URL_SAFE_NO_PAD.encode(sign(key, &body).finalize().into_bytes());
    format!("{body}.{signature}")
}

/// The ticket a cookie value holds, when its signature is ours, it is
/// well formed, and its click is at most [`WINDOW`] old at `now`.
pub fn decode(value: &str, key: &[u8; 32], now: u64) -> Option<Ticket> {
    if value.len() > 1024 {
        return None;
    }
    let (body, signature) = value.rsplit_once('.')?;
    let payload = body.strip_prefix("v1.")?;
    let signature = URL_SAFE_NO_PAD.decode(signature).ok()?;
    sign(key, body).verify_slice(&signature).ok()?;
    let ticket: Ticket = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(payload).ok()?).ok()?;
    (ticket.valid()
        && ticket.at <= now.saturating_add(300)
        && now.saturating_sub(ticket.at) <= WINDOW)
        .then_some(ticket)
}

/// The raw `oa_ref` value the browser sent, if any.
pub fn raw(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == COOKIE)
        .map(|(_, value)| value.to_owned())
}

/// The ticket in the request's cookie, if valid at `now`.
pub fn read(headers: &HeaderMap, key: &[u8; 32], now: u64) -> Option<Ticket> {
    decode(&raw(headers)?, key, now)
}

fn attributes(secure: bool) -> &'static str {
    if secure {
        "; Path=/; HttpOnly; SameSite=Lax; Secure"
    } else {
        "; Path=/; HttpOnly; SameSite=Lax"
    }
}

/// The `Set-Cookie` value that keeps `ticket` for [`WINDOW`].
pub fn set(ticket: &Ticket, key: &[u8; 32], secure: bool) -> String {
    format!(
        "{COOKIE}={}; Max-Age={WINDOW}{}",
        encode(ticket, key),
        attributes(secure)
    )
}

/// The `Set-Cookie` value that removes the cookie.
pub fn clear(secure: bool) -> String {
    format!("{COOKIE}=; Max-Age=0{}", attributes(secure))
}
