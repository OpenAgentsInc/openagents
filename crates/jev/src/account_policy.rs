//! Private, non-retrying native team policy review and reads.
use super::Account;
use crate::{Error, Result};
use receipts::team_policy::{Change, Reference, Revision, SCHEMA, identifier};
use reqwest::{
    Method,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TeamPolicyView {
    pub schema: String,
    pub reference: Reference,
    pub reviewed: Option<Revision>,
    #[serde(default)]
    pub enabled: Vec<String>,
    #[serde(default)]
    pub unsupported: Vec<String>,
}
impl Account<'_> {
    async fn policy_call(
        &self,
        account: &str,
        workspace: &str,
        change: Option<&Change>,
    ) -> Result<TeamPolicyView> {
        if !identifier(account) || !identifier(workspace) {
            return Err(Error::Config(
                "Invalid native team policy selection.".into(),
            ));
        }
        if let Some(c) = change {
            c.terms.validate().map_err(|e| Error::Config(e.into()))?;
        }
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openagents-team-account",
            HeaderValue::from_str(account)
                .map_err(|_| Error::Config("Invalid policy account.".into()))?,
        );
        headers.insert(
            "x-workspace-id",
            HeaderValue::from_str(workspace)
                .map_err(|_| Error::Config("Invalid policy workspace.".into()))?,
        );
        let bytes = change
            .map(serde_json::to_vec)
            .transpose()
            .map_err(|_| Error::Config("Invalid private policy intent.".into()))?;
        let raw = self
            .client
            .request_private_headers(
                if change.is_some() {
                    Method::PUT
                } else {
                    Method::GET
                },
                &format!("/v1/workspaces/{workspace}/team-policy"),
                bytes,
                &headers,
            )
            .await?;
        let invalid = || Error::ResponseValidation {
            status: 200,
            field_path: "team-policy".into(),
            body: None,
            request_id: None,
        };
        if raw.bytes.len() > 128 * 1024 {
            return Err(invalid());
        }
        let v: TeamPolicyView = serde_json::from_slice(&raw.bytes).map_err(|_| invalid())?;
        if v.schema != SCHEMA
            || v.reference.workspace != workspace
            || !receipts::team_policy::hash(&v.reference.digest)
            || v.reviewed
                .as_ref()
                .is_some_and(|r| r.validate().is_err() || r.reference() != v.reference)
            || change.is_some_and(|c| v.reviewed.as_ref().is_none_or(|r| &r.terms != &c.terms))
        {
            return Err(invalid());
        }
        Ok(v)
    }
    pub async fn team_policy(&self, account: &str, workspace: &str) -> Result<TeamPolicyView> {
        self.policy_call(account, workspace, None).await
    }
    pub async fn review_team_policy(
        &self,
        account: &str,
        workspace: &str,
        change: &Change,
    ) -> Result<TeamPolicyView> {
        self.policy_call(account, workspace, Some(change)).await
    }
}
