//! Authenticated referral calls share the private transport and never retry.
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
        if raw.bytes.len() <= 256 * 1024 {
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
    pub async fn attribution_policy(&self) -> Result<Option<AttributionPolicy>> {
        self.referral_call(Method::GET, "/v1/account/attribution/policy", None)
            .await
    }
    pub async fn attribution_policy_version(
        &self,
        digest: &str,
    ) -> Result<Option<AttributionPolicy>> {
        if !digest.strip_prefix("sha256:").is_some_and(|h| {
            h.len() == 64
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }) {
            return Err(Error::Config("Invalid attribution policy digest.".into()));
        }
        self.referral_call(
            Method::GET,
            &format!("/v1/account/attribution/policy?digest={digest}"),
            None,
        )
        .await
    }
    pub async fn attribution(&self) -> Result<Option<AttributionView>> {
        self.referral_call(Method::GET, "/v1/account/attribution", None)
            .await
    }
    pub async fn propose_attribution(
        &self,
        input: &AttributionProposal,
    ) -> Result<AttributionDecision> {
        self.referral_call(
            Method::POST,
            "/v1/account/attribution",
            Some(
                serde_json::to_value(input)
                    .map_err(|_| Error::Config("Invalid attribution input.".into()))?,
            ),
        )
        .await
    }
    pub async fn confirm_attribution(
        &self,
        customer: &str,
        decision: &str,
    ) -> Result<AttributionConfirmation> {
        self.referral_call(
            Method::POST,
            "/v1/account/attribution/confirm",
            Some(json!({"customer":customer,"decision":decision})),
        )
        .await
    }
    pub async fn workspace_attribution(
        &self,
        workspace: &str,
    ) -> Result<Option<WorkspaceAttribution>> {
        self.referral_call(Method::GET, &workspace_route(workspace)?, None)
            .await
    }
    pub async fn adopt_workspace_attribution(
        &self,
        workspace: &str,
        decision: &str,
    ) -> Result<WorkspaceAttribution> {
        self.referral_call(
            Method::POST,
            &workspace_route(workspace)?,
            Some(json!({"decision":decision})),
        )
        .await
    }
    pub async fn referrer_successors(&self, id: &str) -> Result<Vec<ReferralSuccessor>> {
        self.referral_call(Method::GET, &format!("{}/lineage", route(id)?), None)
            .await
    }
}

fn workspace_route(workspace: &str) -> Result<String> {
    if workspace.is_empty()
        || workspace.len() > 128
        || !workspace
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(Error::Config("Invalid attribution workspace.".into()));
    }
    Ok(format!("/v1/workspaces/{workspace}/attribution"))
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionPolicy {
    pub schema: String,
    pub version: String,
    pub rule: String,
    pub terms: String,
    pub digest: String,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ReferralIntroduction {
    CapturedSource,
    EarlyAgreement,
    PreexistingCustomer,
    MissingEvidence,
    Correction,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferralEvidence {
    pub reference: String,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionProposal {
    pub request: String,
    pub policy_digest: String,
    pub introduction: ReferralIntroduction,
    pub referrer: Option<String>,
    pub evidence: Vec<ReferralEvidence>,
    pub reason: String,
    pub consent: bool,
    pub expected_decision: Option<String>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionStatus {
    Accepted,
    Review,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum AttributionReview {
    MissingEvidence,
    PreexistingCustomer,
    CompetingIntroduction,
    AwaitingConfirmation,
    SelfReferral,
    SourceOnly,
    UnknownSignup,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionBinding {
    pub id: String,
    pub customer: String,
    pub referrer: ReferralIdentity,
    pub policy_digest: String,
    pub accepted_decision: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionDecision {
    pub schema: String,
    pub customer: String,
    pub sequence: u64,
    pub prior: Option<String>,
    pub request: String,
    pub policy_digest: String,
    pub introduction: ReferralIntroduction,
    pub status: AttributionStatus,
    pub review: Option<AttributionReview>,
    pub referrer: Option<ReferralIdentity>,
    pub referrer_owner: Option<String>,
    pub source: Option<ReferralSource>,
    pub evidence: Vec<ReferralEvidence>,
    pub reason: String,
    pub actor: String,
    pub confirmed: Option<String>,
    pub at: u64,
    pub digest: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionView {
    pub schema: String,
    pub customer: String,
    pub status: AttributionStatus,
    pub binding: Option<AttributionBinding>,
    pub decisions: Vec<AttributionDecision>,
    pub commission_eligibility: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AttributionConfirmation {
    pub customer: String,
    pub decision: String,
    pub policy_digest: String,
    pub status: AttributionStatus,
    pub referrer: ReferralIdentity,
    pub commission_eligibility: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceAttribution {
    pub schema: String,
    pub workspace: String,
    pub status: AttributionStatus,
    pub binding: AttributionBinding,
    pub commission_eligibility: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ReferralSuccessor {
    pub referrer: String,
    pub from: String,
    pub to: String,
    pub version: u64,
    pub accepted_at: u64,
    pub management_only: bool,
    pub digest: String,
}
