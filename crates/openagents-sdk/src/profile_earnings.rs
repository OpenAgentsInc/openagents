//! Private earnings previews and explicit publication of individual receipts.
use crate::{Client, Error, Session};
use nostr::{domain::Event, earnings::Receipt};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
/// The private credit identity selected for publication; never include it in public copy.
pub enum EarningsSource {
    Ledger { sequence: i64, role: String },
    Tip { payment_hash: String },
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// A private preview of one verified credit and its publication choice.
pub struct Earning {
    pub source: EarningsSource,
    pub receipt: Receipt,
    pub published: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Private earnings for an account's verified Nostr identity.
pub struct ProfileEarnings {
    pub rows: Vec<Earning>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Selected public receipts and the service issuers trusted for this response.
pub struct PublishedEarnings {
    pub events: Vec<Event>,
    pub issuers: BTreeSet<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// One service-signed earnings receipt.
pub struct PublishedReceipt {
    pub event: Event,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
/// Confirmation that a credit is hidden from the public profile.
pub struct HiddenReceipt {
    pub hidden: bool,
}
fn key(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
impl EarningsSource {
    fn check(&self) -> Result<(), Error> {
        let valid = match self {
            Self::Tip { payment_hash } => key(payment_hash),
            Self::Ledger { sequence, role } => {
                *sequence > 0
                    && !role.is_empty()
                    && role.len() <= 64
                    && role.bytes().all(|b| b.is_ascii_lowercase() || b == b'_')
            }
        };
        if valid {
            Ok(())
        } else {
            Err(Error::InvalidReference)
        }
    }
}
impl Client {
    /// Read private Pylon, plugin, and confirmed tip credits for your profile.
    pub async fn profile_earnings(
        &self,
        session: &Session,
        party: &str,
    ) -> Result<ProfileEarnings, Error> {
        self.check_session(session)?;
        if !key(party) {
            return Err(Error::InvalidReference);
        }
        self.request(
            Method::GET,
            &["device", "earnings", party],
            &[],
            None,
            Some(session.token()),
        )
        .await
    }
    /// Choose a verified credit to show publicly; the service resolves its exact amount.
    pub async fn publish_earning(
        &self,
        session: &Session,
        party: &str,
        source: &EarningsSource,
    ) -> Result<PublishedReceipt, Error> {
        self.check_session(session)?;
        if !key(party) {
            return Err(Error::InvalidReference);
        }
        source.check()?;
        self.request(
            Method::POST,
            &["device", "earnings", party, "receipts"],
            &[],
            Some(json!({"source":source})),
            Some(session.token()),
        )
        .await
    }
    /// Hide a selected credit and its earlier receipt links from your public profile.
    pub async fn hide_earning(
        &self,
        session: &Session,
        party: &str,
        source: &str,
    ) -> Result<HiddenReceipt, Error> {
        self.check_session(session)?;
        if !key(party) || !key(source) {
            return Err(Error::InvalidReference);
        }
        self.request(
            Method::DELETE,
            &["device", "earnings", party, "receipts", source],
            &[],
            None,
            Some(session.token()),
        )
        .await
    }
    /// Read a profile's selected public receipts without sending account credentials.
    pub async fn published_earnings(&self, party: &str) -> Result<PublishedEarnings, Error> {
        if !key(party) {
            return Err(Error::InvalidReference);
        }
        self.request(Method::GET, &["earnings", "public", party], &[], None, None)
            .await
    }
}
