//! Existing gateway credential routes. Mutations never retry automatically;
//! callers retain uncertain outcomes instead of repeating credential effects.

use super::Account;
use crate::{ApiKey, Error, RawResponse, Result};
use reqwest::Method;
use serde::{Deserialize, Serialize};

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

/// One API key as the workspace's key list shows it: never its secret.
#[derive(Debug, Clone, Deserialize)]
pub struct KeyRecord {
    pub id: String,
    #[serde(default)]
    pub name: Option<String>,
    /// `active`, `paused`, or `revoked`.
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub created: Option<String>,
}

/// One of a workspace's own provider keys, as the gateway lists it.
#[derive(Debug, Clone, Deserialize)]
pub struct ProviderKeyRecord {
    pub provider: String,
    pub fingerprint: String,
    #[serde(default)]
    pub added_at: u64,
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

/// A bounded activity page used to locate an original purchase attempt.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PurchaseActivity {
    pub workspace: String,
    pub items: Vec<PurchaseActivityItem>,
    pub cursor: Option<String>,
}
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PurchaseActivityItem {
    pub digest: String,
    pub request: String,
    pub attempt: u32,
}
/// Current money-ledger position for the original execution receipt.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PurchaseCost {
    pub reserved: u64,
    pub retail: Option<u64>,
    pub phase: String,
    pub price_version: String,
}
/// A verified receipt and its current settlement claim from the same origin.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct PurchaseReceipt {
    pub receipt: receipts::execution::ExecutionReceipt,
    pub cost: Option<PurchaseCost>,
}
fn receipt_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|hex| {
        hex.len() == 64
            && hex
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
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

/// The `url` an answer carries: an https address only.
fn hosted_url(raw: &RawResponse) -> Result<String> {
    #[derive(Deserialize)]
    struct Wire {
        url: String,
    }
    let wire: Wire = decode(raw)?;
    if !wire.url.starts_with("https://") || wire.url.len() > 8192 {
        return Err(Error::ResponseValidation {
            status: raw.status,
            field_path: "url".into(),
            body: None,
            request_id: None,
        });
    }
    Ok(wire.url)
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
            .request_private_bounded(Method::POST, "/v1/sessions", None, 64 * 1024)
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
            .request_private_bounded(
                Method::GET,
                &format!("/v1/workspaces/{workspace}"),
                None,
                64 * 1024,
            )
            .await?;
        decode(&raw)
    }

    /// Read current customer, payer, resource, and bounded price references.
    /// Absence of the selected monetary/account lane is an explicit refusal.
    pub async fn purchase_context(
        &self,
        workspace: &str,
        door: &str,
    ) -> Result<receipts::purchase::Context> {
        identifier(workspace)?;
        identifier(door)?;
        let raw = self
            .client
            .request_private(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/purchase-context/{door}"),
                None,
            )
            .await?;
        let context: receipts::purchase::Context = decode(&raw)?;
        context
            .validate()
            .map_err(|_| Error::Config("Invalid customer purchase context.".into()))?;
        if context.workspace != workspace || context.door != door {
            return Err(Error::Config("Customer purchase selection changed.".into()));
        }
        Ok(context)
    }

    /// Authenticate an original native Plugin source for read-only recovery.
    pub async fn plugin_reader(
        &self,
        source: &receipts::purchase::CommercialSource,
    ) -> Result<receipts::purchase::PluginReadIdentity> {
        use receipts::purchase::CommercialProduct;
        if source.product != CommercialProduct::Plugin {
            return Err(Error::Config(
                "Plugin recovery requires its original native source.".into(),
            ));
        }
        identifier(&source.account)?;
        let workspace = source.workspace.as_deref().ok_or_else(|| {
            Error::Config("Plugin recovery requires its native workspace.".into())
        })?;
        identifier(workspace)?;
        let raw = self
            .client
            .request_private(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/plugin-reader"),
                None,
            )
            .await?;
        if raw.bytes.len() > 8192 {
            return Err(Error::Config(
                "Native Plugin reader document is too large.".into(),
            ));
        }
        let reader: receipts::purchase::PluginReadIdentity = decode(&raw)?;
        if reader.validate().is_err() || &reader.source != source {
            return Err(Error::Config(
                "Native Plugin recovery source changed.".into(),
            ));
        }
        Ok(reader)
    }

    /// Read only the operator-reviewed mapping for this exact native selection.
    pub async fn commercial_selection(
        &self,
        account: &str,
        workspace: &str,
        product: receipts::purchase::CommercialProduct,
    ) -> Result<Option<receipts::purchase::CommercialRef>> {
        identifier(account)?;
        identifier(workspace)?;
        let name = match product {
            receipts::purchase::CommercialProduct::Gateway => "gateway",
            receipts::purchase::CommercialProduct::Plugin => "plugin",
            receipts::purchase::CommercialProduct::Retail => {
                return Err(Error::Config(
                    "Retail attribution requires its native product adapter.".into(),
                ));
            }
        };
        let raw = self
            .client
            .request_private(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/commercial/{name}"),
                None,
            )
            .await?;
        if raw.bytes.len() > 8192 {
            return Err(Error::Config(
                "Commercial projection exceeds its bound.".into(),
            ));
        }
        let reference: Option<receipts::purchase::CommercialRef> = decode(&raw)?;
        if reference.as_ref().is_some_and(|r| {
            r.validate().is_err() || !r.matches_native(product, account, Some(workspace))
        }) {
            return Err(Error::Config(
                "Commercial projection changes the selected native identity.".into(),
            ));
        }
        Ok(reference)
    }

    /// Read at most ten activity references under fresh workspace membership.
    pub async fn purchase_activity(
        &self,
        workspace: &str,
        original_key: &str,
        cursor: Option<&str>,
    ) -> Result<PurchaseActivity> {
        identifier(workspace)?;
        if original_key.is_empty()
            || original_key.len() > 128
            || !original_key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
        {
            return Err(Error::Config(
                "Invalid original purchase credential reference.".into(),
            ));
        }
        if cursor
            .is_some_and(|value| value.len() > 256 || value.bytes().any(|b| b.is_ascii_control()))
        {
            return Err(Error::Config("Invalid purchase activity cursor.".into()));
        }
        let mut query = reqwest::Url::parse("https://fixture.invalid/").expect("static URL");
        {
            let mut pairs = query.query_pairs_mut();
            pairs
                .append_pair("limit", "10")
                .append_pair("key", original_key);
            if let Some(cursor) = cursor {
                pairs.append_pair("cursor", cursor);
            }
        }
        let raw = self
            .client
            .request_private(
                Method::GET,
                &format!(
                    "/v1/workspaces/{workspace}/usage/activity?{}",
                    query.query().unwrap()
                ),
                None,
            )
            .await?;
        let page: PurchaseActivity = decode(&raw)?;
        if page.workspace != workspace
            || page.items.len() > 10
            || page
                .items
                .iter()
                .any(|item| !receipt_digest(&item.digest) || item.attempt == 0)
            || page.cursor.as_ref().is_some_and(|cursor| {
                cursor.len() > 256 || cursor.bytes().any(|b| b.is_ascii_control())
            })
        {
            return Err(Error::Config("Invalid purchase activity page.".into()));
        }
        Ok(page)
    }
    /// Read the exact original receipt. This never invokes or retries a purchase.
    pub async fn purchase_receipt(&self, workspace: &str, digest: &str) -> Result<PurchaseReceipt> {
        identifier(workspace)?;
        if !receipt_digest(digest) {
            return Err(Error::Config("Invalid purchase receipt digest.".into()));
        }
        let raw = self
            .client
            .request_private(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/usage/receipts/{digest}"),
                None,
            )
            .await?;
        let proof: PurchaseReceipt = decode(&raw)?;
        if proof.receipt.verify().is_err()
            || proof.receipt.digest != digest
            || proof.receipt.workspace.as_deref() != Some(workspace)
            || proof.cost.as_ref().is_some_and(|cost| {
                !matches!(
                    cost.phase.as_str(),
                    "held" | "unknown" | "settled" | "released"
                )
            })
        {
            return Err(Error::Config(
                "Purchase receipt is unverifiable or belongs to another workspace.".into(),
            ));
        }
        Ok(proof)
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

    /// The workspace's API keys the caller may see: ids, names, and states,
    /// never a secret.
    pub async fn keys(&self, workspace: &str) -> Result<Vec<KeyRecord>> {
        #[derive(Deserialize)]
        struct Wire {
            keys: Vec<KeyRecord>,
        }
        identifier(workspace)?;
        let raw = self
            .client
            .request_private_bounded(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/keys"),
                None,
                1024 * 1024,
            )
            .await?;
        serde_json::from_slice::<Wire>(&raw.bytes)
            .map(|wire| wire.keys)
            .map_err(|_| Error::ResponseValidation {
                status: raw.status,
                field_path: "keys".into(),
                body: None,
                request_id: None,
            })
    }

    /// Issue a new API key on the workspace, bound to the caller's account.
    /// The secret is in the grant only.
    pub async fn issue_key(&self, workspace: &str, name: &str) -> Result<KeyGrant> {
        identifier(workspace)?;
        let body = serde_json::to_vec(&serde_json::json!({"name": name}))
            .map_err(|_| Error::Config("Key request encoding failed.".into()))?;
        self.key_mutation(&format!("/v1/workspaces/{workspace}/keys"), Some(body))
            .await
    }

    /// Replace the limits a key's owner set for the model API (spending
    /// cap, price cap, models, rate, expiry), as the gateway's
    /// `.../keys/{key}/limits` document.
    pub async fn set_key_limits(
        &self,
        workspace: &str,
        key: &str,
        limits: &serde_json::Value,
    ) -> Result<()> {
        identifier(workspace)?;
        identifier(key)?;
        let body = serde_json::to_vec(limits)
            .map_err(|_| Error::Config("Limits encoding failed.".into()))?;
        self.client
            .request_private(
                Method::PUT,
                &format!("/v1/workspaces/{workspace}/keys/{key}/limits"),
                Some(body),
            )
            .await?;
        Ok(())
    }

    /// The workspace's own provider keys for the model API (bring your own
    /// key): provider and fingerprint only, never the key.
    pub async fn provider_keys(&self, workspace: &str) -> Result<Vec<ProviderKeyRecord>> {
        #[derive(Deserialize)]
        struct Wire {
            keys: Vec<ProviderKeyRecord>,
        }
        identifier(workspace)?;
        let raw = self
            .client
            .request_private_bounded(
                Method::GET,
                &format!("/v1/workspaces/{workspace}/provider-keys"),
                None,
                64 * 1024,
            )
            .await?;
        decode::<Wire>(&raw).map(|wire| wire.keys)
    }

    /// Seal and keep the workspace's own key for `provider` (`openrouter`
    /// or `vercel`), replacing any it held.
    pub async fn set_provider_key(
        &self,
        workspace: &str,
        provider: &str,
        key: &ApiKey,
    ) -> Result<()> {
        identifier(workspace)?;
        identifier(provider)?;
        let body = serde_json::to_vec(&serde_json::json!({"key": key.expose()}))
            .map_err(|_| Error::Config("Key request encoding failed.".into()))?;
        self.client
            .request_private(
                Method::PUT,
                &format!("/v1/workspaces/{workspace}/provider-keys/{provider}"),
                Some(body),
            )
            .await?;
        Ok(())
    }

    /// Forget the workspace's own key for `provider`.
    pub async fn remove_provider_key(&self, workspace: &str, provider: &str) -> Result<()> {
        identifier(workspace)?;
        identifier(provider)?;
        self.client
            .request_private(
                Method::DELETE,
                &format!("/v1/workspaces/{workspace}/provider-keys/{provider}"),
                None,
            )
            .await?;
        Ok(())
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

    /// Open the payment page for `plan` on `workspace` (the owner only):
    /// the hosted checkout address to send the browser to. The plan starts
    /// only when the payment provider confirms it to the gateway.
    pub async fn plan_checkout(&self, workspace: &str, plan: &str) -> Result<String> {
        identifier(workspace)?;
        identifier(plan)?;
        let body = serde_json::to_vec(&serde_json::json!({ "plan": plan }))
            .map_err(|_| Error::Config("Invalid plan.".into()))?;
        let raw = self
            .client
            .request_private_bounded(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/billing/checkout"),
                Some(body),
                64 * 1024,
            )
            .await?;
        hosted_url(&raw)
    }

    /// Open the billing page for `workspace`'s subscription, where the owner
    /// changes their card or cancels.
    pub async fn billing_portal(&self, workspace: &str) -> Result<String> {
        identifier(workspace)?;
        let raw = self
            .client
            .request_private_bounded(
                Method::POST,
                &format!("/v1/workspaces/{workspace}/billing/portal"),
                Some(b"{}".to_vec()),
                64 * 1024,
            )
            .await?;
        hosted_url(&raw)
    }

    /// End the current session once; uncertainty never causes a replay.
    pub async fn sign_out(&self) -> Result<()> {
        self.client
            .request_private_bounded(Method::DELETE, "/v1/session", None, 64 * 1024)
            .await?;
        Ok(())
    }
}
