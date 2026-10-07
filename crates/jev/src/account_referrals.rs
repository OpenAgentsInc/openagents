//! Source-only introduction calls share the account transport and never retry.
use super::Account;
use crate::{Error, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReferralKind {
    Person,
    Agent,
    Author,
    Partner,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferralRecord {
    pub schema: String,
    pub id: String,
    pub version: u64,
    pub owner: String,
    pub kind: ReferralKind,
    pub label: String,
    pub source_only: bool,
    pub pending_owner: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferralIdentity {
    pub id: String,
    pub version: u64,
    pub kind: ReferralKind,
    pub source_only: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferralSource {
    pub schema: String,
    pub account: String,
    pub request: String,
    pub outcome: String,
    pub referrer: Option<ReferralIdentity>,
    pub consent_version: Option<String>,
    pub captured_at: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferralCapture {
    pub request: String,
    pub token: Option<String>,
    pub consent: bool,
    pub consent_version: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ReferralLink {
    pub referrer: String,
    pub token: String,
    pub path: String,
}
fn route(id: &str) -> Result<String> {
    if !id.strip_prefix("ref_").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    }) {
        return Err(Error::Config("Invalid stable referrer identifier.".into()));
    }
    Ok(format!("/v1/account/referrers/{id}"))
}
impl Account<'_> {
    /// Pin referral reads and writes to an already selected account. The
    /// authenticated service must refuse a changed credential mapping.
    pub fn for_referrals_account(mut self, account: &str) -> Self {
        self.referral_account = Some(account.into());
        self
    }
    async fn referral_call<T: for<'de> Deserialize<'de>>(
        &self,
        method: Method,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T> {
        #[derive(Deserialize)]
        struct Envelope<T> {
            v: String,
            referral: T,
        }
        let bytes = body
            .as_ref()
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| Error::Config("Invalid referral request.".into()))?;
        let mut headers = reqwest::header::HeaderMap::new();
        if let Some(expected) = &self.referral_account {
            if expected.is_empty() || expected.len() > 128 {
                return Err(Error::Config("Invalid expected referral account.".into()));
            }
            headers.insert(
                "x-openagents-referral-account",
                reqwest::header::HeaderValue::from_str(expected)
                    .map_err(|_| Error::Config("Invalid expected referral account.".into()))?,
            );
        }
        let raw = self
            .client
            .request_private_headers(method, path, bytes, &headers)
            .await?;
        if raw.bytes.len() <= 16 * 1024 {
            if let Ok(value) = serde_json::from_slice::<Envelope<T>>(&raw.bytes) {
                if value.v == "openagents.accounts.v1" {
                    return Ok(value.referral);
                }
            }
        }
        Err(Error::ResponseValidation {
            status: raw.status,
            field_path: "referral".into(),
            body: None,
            request_id: None,
        })
    }
    pub async fn create_referrer(&self, kind: ReferralKind, label: &str) -> Result<ReferralRecord> {
        self.referral_call(
            Method::POST,
            "/v1/account/referrers",
            Some(json!({"kind":kind,"label":label})),
        )
        .await
    }
    pub async fn referrer(&self, id: &str) -> Result<ReferralRecord> {
        self.referral_call(Method::GET, &route(id)?, None).await
    }
    pub async fn issue_referral_link(&self, id: &str) -> Result<ReferralLink> {
        let link: ReferralLink = self
            .referral_call(Method::POST, &format!("{}/link", route(id)?), None)
            .await?;
        if link.referrer != id
            || !link.token.strip_prefix("rfr_").is_some_and(|hex| {
                hex.len() == 64
                    && hex
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            })
            || link.path != format!("/join?ref={}", link.token)
        {
            return Err(Error::ResponseValidation {
                status: 200,
                field_path: "referral link".into(),
                body: None,
                request_id: None,
            });
        }
        Ok(link)
    }
    pub async fn disable_referral_links(&self, id: &str) -> Result<()> {
        let _: serde_json::Value = self
            .referral_call(Method::DELETE, &format!("{}/link", route(id)?), None)
            .await?;
        Ok(())
    }
    pub async fn capture_acquisition(&self, input: &ReferralCapture) -> Result<ReferralSource> {
        self.referral_call(
            Method::POST,
            "/v1/account/acquisition",
            Some(
                serde_json::to_value(input)
                    .map_err(|_| Error::Config("Invalid acquisition input.".into()))?,
            ),
        )
        .await
    }
    pub async fn acquisition(&self) -> Result<Option<ReferralSource>> {
        self.referral_call(Method::GET, "/v1/account/acquisition", None)
            .await
    }
    pub async fn offer_referrer_migration(
        &self,
        id: &str,
        account: &str,
    ) -> Result<ReferralRecord> {
        self.referral_call(
            Method::POST,
            &format!("{}/migration", route(id)?),
            Some(json!({"account":account})),
        )
        .await
    }
    pub async fn accept_referrer_migration(&self, id: &str) -> Result<ReferralRecord> {
        self.referral_call(
            Method::POST,
            &format!("{}/migration/accept", route(id)?),
            None,
        )
        .await
    }
}
