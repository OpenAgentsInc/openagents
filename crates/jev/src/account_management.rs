//! Existing gateway credential routes. Mutations never retry automatically;
//! callers retain uncertain outcomes instead of repeating credential effects.

use super::Account;
use crate::{ApiKey, Error, RawResponse, Result};
use reqwest::Method;
use serde::Deserialize;

/// A once-issued session. Debug output masks its credential.
#[derive(Debug)]
pub struct SessionGrant {
    pub session: GatewaySession,
    pub token: ApiKey,
}

/// Gateway session timestamps are Unix seconds, rather than SDK display text.
#[derive(Debug, Clone, Deserialize)]
pub struct GatewaySession {
    pub id: String,
    pub kind: String,
    pub account: Option<String>,
    pub created_at: u64,
    pub expires_at: u64,
    pub state: Option<String>,
}

/// Public credential identity, separate from its secret and workspace rights.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyIdentity {
    pub id: String,
    pub tenant: String,
}

/// A recovery or rotation result. The credential belongs in private storage.
#[derive(Debug)]
pub struct KeyGrant {
    pub account: Option<String>,
    pub key: KeyIdentity,
    pub token: ApiKey,
}

/// Current workspace identity and membership revision from the gateway.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceIdentity {
    pub id: String,
    pub tenant: String,
    pub members_epoch: u64,
}

/// An authenticated workspace read; the role is not device or host authority.
#[derive(Debug, Clone, Deserialize)]
pub struct WorkspaceView {
    pub workspace: WorkspaceIdentity,
    pub role: String,
}

fn identifier(value: &str) -> Result<()> {
    if value.is_empty()
        || matches!(value, "." | "..")
        || value.len() > 128
        || !value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err(Error::Config("Invalid account route identifier.".into()));
    }
    Ok(())
}

fn decode<T: for<'de> Deserialize<'de>>(raw: &RawResponse) -> Result<T> {
    if raw.bytes.len() <= 16 * 1024
        && let Ok(value) = serde_json::from_slice(&raw.bytes)
    {
        return Ok(value);
    }
    Err(Error::ResponseValidation {
        status: raw.status,
        field_path: "credential document".into(),
        body: None,
        request_id: None,
    })
}

impl Account<'_> {
    /// Sign in using this client's existing account API key. A session token
    /// or an unbound key receives the gateway's explicit refusal.
    pub async fn sign_in(&self) -> Result<SessionGrant> {
        #[derive(Deserialize)]
        struct Wire {
            session: GatewaySession,
            token: String,
        }
        let raw = self
            .client
            .request_private(Method::POST, "/v1/sessions", None)
            .await?;
        let wire: Wire = decode(&raw)?;
        Ok(SessionGrant {
            session: wire.session,
            token: ApiKey::new(wire.token),
        })
    }

    /// Read exactly the selected workspace; no automatic workspace selection.
    pub async fn workspace(&self, workspace: &str) -> Result<WorkspaceView> {
        identifier(workspace)?;
        let raw = self
            .client
            .request_read(
                Method::GET,
                &format!("/v1/workspaces/{workspace}"),
                None,
                &reqwest::header::HeaderMap::new(),
                None,
                None,
            )
            .await?;
        decode(&raw)
    }

    /// Consume a recovery token once. A transport failure is uncertain and
    /// requires reconciliation; this method performs no automatic retry.
    pub async fn recover(&self, token: &ApiKey) -> Result<KeyGrant> {
        let body = serde_json::to_vec(&serde_json::json!({"token":token.expose()}))
            .map_err(|_| Error::Config("Recovery request encoding failed.".into()))?;
        self.key_mutation("/v1/recovery/redeem", Some(body)).await
    }

    /// Rotate an existing workspace key without changing historical identity.
    pub async fn rotate_key(&self, workspace: &str, key: &str) -> Result<KeyGrant> {
        identifier(workspace)?;
        identifier(key)?;
        self.key_mutation(
            &format!("/v1/workspaces/{workspace}/keys/{key}/rotate"),
            None,
        )
        .await
    }

    async fn key_mutation(&self, path: &str, body: Option<Vec<u8>>) -> Result<KeyGrant> {
        #[derive(Deserialize)]
        struct Wire {
            account: Option<String>,
            key: KeyIdentity,
            key_token: String,
        }
        let raw = self
            .client
            .request_private(Method::POST, path, body)
            .await?;
        let wire: Wire = decode(&raw)?;
        Ok(KeyGrant {
            account: wire.account,
            key: wire.key,
            token: ApiKey::new(wire.key_token),
        })
    }

    /// Revoke the named key through current authenticated workspace rights.
    pub async fn revoke_key(&self, workspace: &str, key: &str) -> Result<()> {
        identifier(workspace)?;
        identifier(key)?;
        self.client
            .request_private(
                Method::DELETE,
                &format!("/v1/workspaces/{workspace}/keys/{key}"),
                None,
            )
            .await?;
        Ok(())
    }

    /// End the current session once; uncertainty never causes a replay.
    pub async fn sign_out(&self) -> Result<()> {
        self.client
            .request_private(Method::DELETE, "/v1/session", None)
            .await?;
        Ok(())
    }
}
