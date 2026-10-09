//! Existing team routes with pinned account identity and no mutation retries.
use super::Account;
use crate::{ApiKey, Client, Error, Result};
use reqwest::{
    Method,
    header::{HeaderMap, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum TeamRole {
    Admin,
    Member,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamInvitation {
    pub id: String,
    pub workspace: String,
    pub role: TeamRole,
    pub expires_unix: u64,
}
#[derive(Debug)]
pub struct InvitationGrant {
    pub invitation: TeamInvitation,
    pub token: ApiKey,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamMember {
    pub account: String,
    pub role: String,
    pub status: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamWorkspace {
    pub workspace: TeamWorkspaceIdentity,
    pub role: String,
    pub members: Vec<TeamMember>,
    #[serde(default)]
    pub invitations: Vec<TeamInvitationSummary>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamWorkspaceIdentity {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub tenant: String,
    pub seats: Option<u32>,
    pub members_epoch: u64,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct TeamInvitationSummary {
    pub id: String,
    pub role: TeamRole,
    pub status: String,
    pub invited_by: String,
    pub expires_unix: u64,
    pub accepted_by: Option<String>,
}
/// A once-issued recovery token for one current member. The issuer
/// delivers it out of band; this client never stores it.
#[derive(Debug)]
pub struct RecoveryGrant {
    pub account: String,
    pub issued_at: u64,
    pub expires_at: u64,
    pub token: ApiKey,
}
/// One retained native access event for a workspace.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TeamAccess {
    pub at: u64,
    pub actor: String,
    pub action: String,
    #[serde(default)]
    pub workspace: Option<String>,
    #[serde(default)]
    pub session: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
}
const MAX_ACCESS: usize = 4096;
const BUDGET_SCHEMA: &str = "openagents.money.budgets.v1";
const REPORT_SCHEMA: &str = "openagents.team-report.v1";
pub struct Team<'a> {
    client: &'a Client,
    account: String,
}
fn id(value: &str) -> Result<()> {
    if value.is_empty()
        || value.len() > 128
        || matches!(value, "." | "..")
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(Error::Config("Invalid team route identifier.".into()));
    }
    Ok(())
}
fn invalid() -> Error {
    Error::ResponseValidation {
        status: 200,
        field_path: "team".into(),
        body: None,
        request_id: None,
    }
}
impl<'a> Account<'a> {
    pub fn team(&self, account: &str) -> Team<'a> {
        Team {
            client: self.client,
            account: account.into(),
        }
    }
}
impl Team<'_> {
    async fn call(&self, method: Method, path: &str, body: Option<Value>) -> Result<Value> {
        let value = self.raw(method, path, body, 64 * 1024).await?;
        if value["v"] != "openagents.accounts.v1" {
            return Err(invalid());
        }
        Ok(value)
    }
    /// One private, non-retrying call pinned to the selected account and
    /// bounded while it is read.
    async fn raw(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        maximum: usize,
    ) -> Result<Value> {
        id(&self.account)?;
        let mut headers = HeaderMap::new();
        headers.insert(
            "x-openagents-team-account",
            HeaderValue::from_str(&self.account)
                .map_err(|_| Error::Config("Invalid selected team account.".into()))?,
        );
        let bytes = body
            .map(|b| serde_json::to_vec(&b))
            .transpose()
            .map_err(|_| Error::Config("Invalid private team input.".into()))?;
        let raw = self
            .client
            .request_private_headers_bounded(method, path, bytes, &headers, Some(maximum))
            .await?;
        if raw.bytes.len() > maximum {
            return Err(invalid());
        }
        serde_json::from_slice(&raw.bytes).map_err(|_| invalid())
    }
    pub async fn members(&self, workspace: &str) -> Result<TeamWorkspace> {
        id(workspace)?;
        let value = self
            .call(Method::GET, &format!("/v1/workspaces/{workspace}"), None)
            .await?;
        let view: TeamWorkspace = serde_json::from_value(value).map_err(|_| invalid())?;
        if view.workspace.id != workspace
            || !matches!(view.workspace.kind.as_str(), "personal" | "organization")
            || view.members.len() > 4096
            || view.invitations.len() > 4096
            || !matches!(view.role.as_str(), "owner" | "admin" | "member")
            || view.members.iter().any(|m| {
                !matches!(m.role.as_str(), "owner" | "admin" | "member")
                    || !matches!(m.status.as_str(), "active" | "revoked")
            })
            || view
                .invitations
                .iter()
                .any(|i| !matches!(i.status.as_str(), "pending" | "accepted" | "revoked"))
        {
            return Err(invalid());
        }
        Ok(view)
    }
    pub async fn create(&self, name: &str, seats: u32) -> Result<Value> {
        if name.is_empty() || name.len() > 256 || name.chars().any(char::is_control) || seats == 0 {
            return Err(Error::Config("Invalid team workspace input.".into()));
        }
        let value = self
            .call(
                Method::POST,
                "/v1/workspaces",
                Some(json!({"name":name,"seats":seats})),
            )
            .await?;
        let workspace = value["workspace"].clone();
        id(workspace["id"].as_str().ok_or_else(invalid)?)?;
        if workspace["kind"] != "organization" {
            return Err(invalid());
        }
        Ok(
            json!({"id":workspace["id"],"name":workspace["name"],"kind":workspace["kind"],"tenant":workspace["tenant"],"seats":workspace["seats"]}),
        )
    }
    pub async fn invite(
        &self,
        workspace: &str,
        role: TeamRole,
        ttl_secs: u64,
    ) -> Result<InvitationGrant> {
        id(workspace)?;
        if ttl_secs == 0 || ttl_secs > 2_592_000 {
            return Err(Error::Config("Invalid invitation lifetime.".into()));
        }
        let value = self
            .call(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/invitations"),
                Some(json!({"role":role,"ttl_secs":ttl_secs})),
            )
            .await?;
        let invitation: TeamInvitation =
            serde_json::from_value(value["invitation"].clone()).map_err(|_| invalid())?;
        let token = value["token"].as_str().ok_or_else(invalid)?;
        id(&invitation.id)?;
        if invitation.workspace != workspace
            || invitation.role != role
            || !token.starts_with(&format!("inv_{}.", invitation.id))
            || token.len() > 256
            || token
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(invalid());
        }
        Ok(InvitationGrant {
            invitation,
            token: ApiKey::new(token),
        })
    }
    pub async fn accept(&self, workspace: &str, token: &ApiKey) -> Result<TeamMember> {
        self.accept_expected(workspace, None, token).await
    }
    pub async fn accept_reviewed(
        &self,
        workspace: &str,
        role: TeamRole,
        token: &ApiKey,
    ) -> Result<TeamMember> {
        self.accept_expected(workspace, Some(role), token).await
    }
    async fn accept_expected(
        &self,
        workspace: &str,
        role: Option<TeamRole>,
        token: &ApiKey,
    ) -> Result<TeamMember> {
        id(workspace)?;
        if !token.expose().starts_with("inv_")
            || token.expose().len() > 256
            || token
                .expose()
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(Error::Config("Invalid private invitation token.".into()));
        }
        let mut body = json!({"workspace":workspace,"token":token.expose()});
        if let Some(role) = role {
            body["role"] = json!(role);
        }
        let value = self
            .call(
                Method::POST,
                if role.is_some() {
                    "/v1/invitations/accept-reviewed"
                } else {
                    "/v1/invitations/accept"
                },
                Some(body),
            )
            .await?;
        let member = &value["membership"];
        if member["workspace"] != workspace
            || member["account"] != self.account
            || !matches!(member["role"].as_str(), Some("admin" | "member"))
            || role.is_some_and(|role| member["role"] != json!(role))
        {
            return Err(invalid());
        }
        let mut value = member.clone();
        value["status"] = json!("active");
        serde_json::from_value(value).map_err(|_| invalid())
    }
    pub async fn withdraw(&self, workspace: &str, invitation: &str) -> Result<Value> {
        id(workspace)?;
        id(invitation)?;
        let value = self
            .call(
                Method::DELETE,
                &format!("/v1/workspaces/{workspace}/invitations/{invitation}"),
                None,
            )
            .await?;
        if value["invitation"]["id"] != invitation || value["invitation"]["status"] != "revoked" {
            return Err(invalid());
        }
        Ok(json!({"id":invitation,"status":"revoked"}))
    }
    pub async fn role(&self, workspace: &str, account: &str, role: TeamRole) -> Result<TeamMember> {
        let member = self
            .member_change(
                workspace,
                account,
                Method::PATCH,
                Some(json!({"role":role})),
            )
            .await?;
        let expected = match role {
            TeamRole::Admin => "admin",
            TeamRole::Member => "member",
        };
        if member.role != expected || member.status != "active" {
            return Err(invalid());
        }
        Ok(member)
    }
    pub async fn remove(&self, workspace: &str, account: &str) -> Result<TeamMember> {
        let member = self
            .member_change(workspace, account, Method::DELETE, None)
            .await?;
        if member.status != "revoked" {
            return Err(invalid());
        }
        Ok(member)
    }
    async fn member_change(
        &self,
        workspace: &str,
        account: &str,
        method: Method,
        body: Option<Value>,
    ) -> Result<TeamMember> {
        id(workspace)?;
        id(account)?;
        let value = self
            .call(
                method,
                &format!("/v1/workspaces/{workspace}/members/{account}"),
                body,
            )
            .await?;
        let member: TeamMember =
            serde_json::from_value(value["membership"].clone()).map_err(|_| invalid())?;
        if member.account != account
            || !matches!(member.role.as_str(), "owner" | "admin" | "member")
            || !matches!(member.status.as_str(), "active" | "revoked")
        {
            return Err(invalid());
        }
        Ok(member)
    }
    pub async fn transfer(&self, workspace: &str, account: &str) -> Result<Value> {
        id(workspace)?;
        id(account)?;
        let value = self
            .call(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/transfer"),
                Some(json!({"account":account})),
            )
            .await?;
        if value["workspace"] != workspace || value["owner"] != account {
            return Err(invalid());
        }
        Ok(json!({"workspace":workspace,"owner":account}))
    }
    /// Issue a single-use recovery token for one current member. Only a
    /// current admin or owner succeeds; the token is returned once.
    pub async fn recovery(&self, workspace: &str, account: &str) -> Result<RecoveryGrant> {
        id(workspace)?;
        id(account)?;
        let value = self
            .call(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/recovery"),
                Some(json!({"account":account})),
            )
            .await?;
        let token = value["token"].as_str().ok_or_else(invalid)?;
        let recovery = &value["recovery"];
        if recovery["user"] != account
            || !token.starts_with("rcv_")
            || token.len() > 256
            || token
                .bytes()
                .any(|b| b.is_ascii_whitespace() || b.is_ascii_control())
        {
            return Err(invalid());
        }
        Ok(RecoveryGrant {
            account: account.into(),
            issued_at: recovery["issued_at"].as_u64().ok_or_else(invalid)?,
            expires_at: recovery["expires_at"].as_u64().ok_or_else(invalid)?,
            token: ApiKey::new(token),
        })
    }
    /// The workspace's retained access events: an admin reads every
    /// member's, a member reads their own. A longer history refuses.
    pub async fn access(&self, workspace: &str) -> Result<Vec<TeamAccess>> {
        id(workspace)?;
        let value = self
            .raw(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/access"),
                None,
                2 * 1024 * 1024,
            )
            .await?;
        if value["v"] != "openagents.accounts.v1" {
            return Err(invalid());
        }
        let events: Vec<TeamAccess> =
            serde_json::from_value(value["access"].clone()).map_err(|_| invalid())?;
        if events.len() > MAX_ACCESS
            || events
                .iter()
                .any(|e| e.workspace.as_deref() != Some(workspace))
        {
            return Err(invalid());
        }
        Ok(events)
    }
    /// The current monetary caps and holds in this member's scope.
    pub async fn budgets(&self, workspace: &str) -> Result<Value> {
        id(workspace)?;
        let value = self
            .raw(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/budgets"),
                None,
                512 * 1024,
            )
            .await?;
        self.budget_answer(workspace, value)
    }
    /// Replace the budget policy under the exact policy last read. The
    /// native owner alone may change caps; the owner checks the roster.
    pub async fn change_budgets(
        &self,
        workspace: &str,
        request: &str,
        expected: Option<&str>,
        policy: &Value,
    ) -> Result<Value> {
        id(workspace)?;
        id(request)?;
        let value = self
            .raw(
                Method::PUT,
                &format!("/v1/workspaces/{workspace}/budgets"),
                Some(json!({"request":request,"expected_policy":expected,"policy":policy})),
                512 * 1024,
            )
            .await?;
        self.budget_answer(workspace, value)
    }
    fn budget_answer(&self, workspace: &str, value: Value) -> Result<Value> {
        if value["v"] != BUDGET_SCHEMA
            || value["workspace"] != workspace
            || value["account"] != self.account.as_str()
            || !value["budget"].is_object()
        {
            return Err(invalid());
        }
        Ok(value)
    }
    /// The current authorized team work report: bounded native rows with
    /// exact receipt, policy, budget, hold, and price pins.
    pub async fn report(&self, workspace: &str) -> Result<Value> {
        id(workspace)?;
        let value = self
            .raw(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/reports"),
                None,
                8 * 1024 * 1024,
            )
            .await?;
        let maximum = value["maximum_rows"].as_u64().unwrap_or(0).min(1024) as usize;
        if value["schema"] != REPORT_SCHEMA
            || value["workspace"] != workspace
            || value["rows"]
                .as_array()
                .is_none_or(|rows| rows.len() > maximum)
        {
            return Err(invalid());
        }
        Ok(value)
    }
}
