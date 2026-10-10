//! Bring your own key on the inference API (`docs/inference/gateway.md`,
//! sections 4 and 9; #11067).
//!
//! A workspace keeps its own provider keys here,
//! sealed with [`oa_seal`] (AES-256-GCM under a keyring read from
//! `inference.byok.keyring`, a private file outside the registry, so a copy
//! of the registry holds only ciphertext). Each sealed key is bound to its
//! workspace and provider. A key is never logged, never answered back: the
//! routes show the provider and [`model_access::fingerprint`] only, as the
//! desktop's key store does.
//!
//! Keys are kept by workspace, never by registry tenant (#11186): every
//! personal workspace made by sign-up shares one tenant, so a key kept by
//! tenant was one key for everyone. A request's own keys are those of the
//! workspace its API key acts in ([`crate::inference_public::key_scope`]).
//! Keys saved by tenant before that are moved at start
//! ([`Keys::adopt`]): into the tenant's only workspace, or into the
//! workspace an operator named (`tenant-db assign-provider-key`); a key on a
//! shared tenant that nobody named is never used.
//!
//! A request with `openagents.pay: "mine"` is offered only adapters on
//! these keys ([`inference::run::Caller::own`]) and never falls back to
//! ours; without a key it is `400` naming `openagents.pay`. There is no fee
//! in P1: the request is metered like any other, its attempts recorded
//! under the account `caller-key`, and `GET /v1/usage/{id}` says the
//! caller paid. The free tier and the balance hold do not apply.
//!
//! Routes, for a signed-in workspace owner or admin (never an API key):
//! `GET /v1/workspaces/{ws}/provider-keys`, and `PUT` (`{"key": "..."}`)
//! or `DELETE /v1/workspaces/{ws}/provider-keys/{provider}`, where
//! `provider` is `openrouter`, `vercel`, `anthropic`, `openai`, or `google`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::Json;
use axum::extract::{Path as UrlPath, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{MethodRouter, get, put};
use inference::upstream::secret::Secret;
use inference::upstream::{Account, CostBasis, Upstream};
use oa_seal::{Keyring, Sealed};
use serde::{Deserialize, Serialize};
use serde_json::json;
use zeroize::Zeroizing;

use crate::accounts::{accounts_store, member, member_account, principal, refused};
use crate::serve::ServeState;

/// Providers supported by the API key store, separate from desktop keys.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Provider {
    OpenRouter,
    Vercel,
    Anthropic,
    OpenAi,
    Google,
}

impl Provider {
    fn word(self) -> &'static str {
        match self {
            Self::OpenRouter => "openrouter",
            Self::Vercel => "vercel",
            Self::Anthropic => "anthropic",
            Self::OpenAi => "openai",
            Self::Google => "google",
        }
    }
    fn name(self) -> &'static str {
        match self {
            Self::OpenRouter => "OpenRouter",
            Self::Vercel => "Vercel AI Gateway",
            Self::Anthropic => "Anthropic",
            Self::OpenAi => "OpenAI",
            Self::Google => "Google",
        }
    }
}

pub const KEYS: &str = "/v1/workspaces/{workspace}/provider-keys";
pub const KEY: &str = "/v1/workspaces/{workspace}/provider-keys/{provider}";

/// The sealed keys' file, beside the registry.
pub const STORE: &str = "inference-provider-keys.json";

/// The longest key accepted.
const KEY_MAX: usize = 512;

/// The BYOK settings in the gateway config.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The `oa-seal` keyring document (a private file, outside the
    /// registry directory).
    pub keyring: PathBuf,
}

pub fn routes() -> Vec<(&'static str, MethodRouter<Arc<ServeState>>)> {
    vec![(KEYS, get(list)), (KEY, put(store).delete(remove))]
}

/// One kept key: sealed, with what may be shown.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Kept {
    sealed: Sealed,
    fingerprint: String,
    /// Unix seconds.
    added_at: u64,
}

/// The file: workspace -> provider word -> key.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    workspaces: BTreeMap<String, BTreeMap<String, Kept>>,
    /// Keys saved by registry tenant before #11186: tenant -> provider
    /// word -> key. Never read for a call or a listing; [`Keys::adopt`]
    /// moves each into its workspace.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    keys: BTreeMap<String, BTreeMap<String, Kept>>,
}

/// The sealed key store and the keyring that opens it.
pub struct Keys {
    path: PathBuf,
    keyring: Keyring,
    saved: std::sync::Mutex<Saved>,
    /// The account database, when the registry is kept there (#11154):
    /// every read and write goes to it, so any number of gateways agree.
    database: Option<tenancy::db::Database>,
}

impl std::fmt::Debug for Keys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Keys")
            .field("path", &self.path)
            .finish_non_exhaustive()
    }
}

/// The providers whose keys the gateway can route to: those with an
/// adapter here.
fn provider(word: &str) -> Option<Provider> {
    match word.trim().to_ascii_lowercase().as_str() {
        "openrouter" | "open-router" => Some(Provider::OpenRouter),
        "vercel" | "ai-gateway" | "gateway" => Some(Provider::Vercel),
        "anthropic" => Some(Provider::Anthropic),
        "openai" => Some(Provider::OpenAi),
        "google" => Some(Provider::Google),
        _ => None,
    }
}

/// What a key is sealed to: its workspace and provider.
fn associated(workspace: &str, provider: Provider) -> Vec<u8> {
    format!(
        "openagents.inference.byok.v2\0workspace\0{workspace}\0{}",
        provider.word()
    )
    .into_bytes()
}

/// What a key saved before #11186 was sealed to: its registry tenant.
fn associated_by_tenant(tenant: &str, provider: Provider) -> Vec<u8> {
    format!(
        "openagents.inference.byok.v1\0{tenant}\0{}",
        provider.word()
    )
    .into_bytes()
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or_default()
}

impl Keys {
    /// The store in `dir` under the keyring at `config.keyring`.
    ///
    /// # Errors
    ///
    /// The keyring or the store cannot be read.
    pub fn open(dir: &Path, config: &Config) -> Result<Self, String> {
        let keyring = Keyring::load(&config.keyring)?;
        Self::with_keyring(dir, keyring)
    }

    /// The store in `dir` under `keyring`.
    ///
    /// # Errors
    ///
    /// The store cannot be read.
    pub fn with_keyring(dir: &Path, keyring: Keyring) -> Result<Self, String> {
        let path = dir.join(STORE);
        if let Some(database) = tenancy::db::bound(dir) {
            return Ok(Self {
                path,
                keyring,
                saved: std::sync::Mutex::new(Saved::default()),
                database: Some(database),
            });
        }
        let saved = match std::fs::read(&path) {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("{}: {error}", path.display()))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Saved::default(),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        Ok(Self {
            path,
            keyring,
            saved: std::sync::Mutex::new(saved),
            database: None,
        })
    }

    fn save(&self, saved: &Saved) -> Result<(), String> {
        let bytes = serde_json::to_vec_pretty(saved).map_err(|e| e.to_string())?;
        let temporary = self.path.with_extension("json.tmp");
        {
            use std::io::Write;
            let mut options = std::fs::OpenOptions::new();
            options.write(true).create(true).truncate(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&temporary).map_err(|e| e.to_string())?;
            file.write_all(&bytes).map_err(|e| e.to_string())?;
            file.sync_all().map_err(|e| e.to_string())?;
        }
        std::fs::rename(&temporary, &self.path).map_err(|e| e.to_string())
    }

    /// Seals and keeps `key` for `workspace`, replacing any key it held
    /// for `provider`. Returns the fingerprint.
    ///
    /// # Errors
    ///
    /// Sealing or writing failed.
    pub fn put(&self, workspace: &str, provider: Provider, key: &str) -> Result<String, String> {
        let sealed = self
            .keyring
            .seal(&associated(workspace, provider), key.as_bytes())
            .map_err(|error| error.to_string())?;
        let fingerprint = model_access::fingerprint(key);
        if let Some(database) = &self.database {
            let kept = Kept {
                sealed,
                fingerprint: fingerprint.clone(),
                added_at: now(),
            };
            let record = serde_json::to_value(&kept).map_err(|e| e.to_string())?;
            tenancy::db::records::put_provider_key(database, workspace, provider.word(), &record)
                .map_err(|e| e.to_string())?;
            return Ok(fingerprint);
        }
        let mut saved = self.saved.lock().map_err(|_| "The key store is busy.")?;
        saved
            .workspaces
            .entry(workspace.to_owned())
            .or_default()
            .insert(
                provider.word().to_owned(),
                Kept {
                    sealed,
                    fingerprint: fingerprint.clone(),
                    added_at: now(),
                },
            );
        self.save(&saved)?;
        Ok(fingerprint)
    }

    /// Forgets `workspace`'s key for `provider`; false when there was
    /// none.
    ///
    /// # Errors
    ///
    /// Writing failed.
    pub fn delete(&self, workspace: &str, provider: Provider) -> Result<bool, String> {
        if let Some(database) = &self.database {
            return tenancy::db::records::delete_provider_key(database, workspace, provider.word())
                .map_err(|e| e.to_string());
        }
        let mut saved = self.saved.lock().map_err(|_| "The key store is busy.")?;
        let removed = saved
            .workspaces
            .get_mut(workspace)
            .and_then(|keys| keys.remove(provider.word()))
            .is_some();
        if removed {
            self.save(&saved)?;
        }
        Ok(removed)
    }

    /// `workspace`'s kept keys, by provider word.
    fn kept(&self, workspace: &str) -> BTreeMap<String, Kept> {
        if let Some(database) = &self.database {
            return tenancy::db::records::provider_keys(database, workspace)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(word, record)| Some((word, serde_json::from_value(record).ok()?)))
                .collect();
        }
        self.saved
            .lock()
            .ok()
            .and_then(|saved| saved.workspaces.get(workspace).cloned())
            .unwrap_or_default()
    }

    /// What `workspace` keeps: provider, fingerprint, when added.
    #[must_use]
    pub fn listed(&self, workspace: &str) -> Vec<(String, String, u64)> {
        self.kept(workspace)
            .into_iter()
            .map(|(word, kept)| (word, kept.fingerprint, kept.added_at))
            .collect()
    }

    /// Adapters on `workspace`'s own keys. A key that no longer opens is
    /// left out (and its fingerprint stays listed, so the owner can
    /// replace it).
    #[must_use]
    pub fn upstreams(&self, workspace: &str) -> Vec<Arc<dyn Upstream>> {
        let opened: Vec<(Provider, Zeroizing<Vec<u8>>)> = self
            .kept(workspace)
            .iter()
            .filter_map(|(word, kept)| {
                let provider = provider(word)?;
                let plain = self
                    .keyring
                    .open(&associated(workspace, provider), &kept.sealed)
                    .ok()?;
                Some((provider, plain))
            })
            .collect();
        opened
            .into_iter()
            .filter_map(|(provider, plain)| {
                let secret = Secret::new(std::str::from_utf8(&plain).ok()?)?;
                Some(adapter(provider, secret))
            })
            .collect()
    }
}

impl Keys {
    /// Moves the keys saved by registry tenant before #11186 into their
    /// workspaces: a key whose tenant holds exactly one workspace goes to
    /// it, and a key on a tenant several workspaces share goes to the
    /// workspace an operator named (`tenant-db assign-provider-key`),
    /// when it is on that tenant. Each is opened under its tenant and
    /// sealed again under its workspace. A key nobody named stays where
    /// it is, used by no one, as does a key that no longer opens.
    /// Returns (moved, left).
    ///
    /// # Errors
    ///
    /// The store cannot be read or written.
    pub fn adopt(&self, accounts: &tenancy::accounts::Store) -> Result<(usize, usize), String> {
        let home = |tenant: &str, named: Option<&str>| -> Option<String> {
            if let Some(named) = named {
                return accounts
                    .workspaces
                    .get(named)
                    .filter(|ws| ws.tenant == tenant)
                    .map(|ws| ws.id.clone());
            }
            let mut on_tenant = accounts
                .workspaces
                .values()
                .filter(|ws| ws.tenant == tenant);
            match (on_tenant.next(), on_tenant.next()) {
                (Some(only), None) => Some(only.id.clone()),
                _ => None,
            }
        };
        let (mut moved, mut left) = (0, 0);
        if let Some(database) = &self.database {
            let rows =
                tenancy::db::records::tenant_provider_keys(database).map_err(|e| e.to_string())?;
            for row in rows {
                let Some(workspace) = home(&row.tenant, row.workspace.as_deref()) else {
                    left += 1;
                    continue;
                };
                let Some(record) =
                    self.resealed(&row.tenant, &row.provider, &workspace, row.record)
                else {
                    left += 1;
                    continue;
                };
                tenancy::db::records::adopt_tenant_provider_key(
                    database,
                    &row.tenant,
                    &row.provider,
                    &workspace,
                    &record,
                )
                .map_err(|e| e.to_string())?;
                moved += 1;
            }
            return Ok((moved, left));
        }
        let mut saved = self.saved.lock().map_err(|_| "The key store is busy.")?;
        let tenants = std::mem::take(&mut saved.keys);
        for (tenant, providers) in tenants {
            for (word, kept) in providers {
                let target = home(&tenant, None).and_then(|workspace| {
                    let record = serde_json::to_value(&kept).ok()?;
                    let record = self.resealed(&tenant, &word, &workspace, record)?;
                    Some((workspace, serde_json::from_value::<Kept>(record).ok()?))
                });
                match target {
                    Some((workspace, resealed)) => {
                        saved
                            .workspaces
                            .entry(workspace)
                            .or_default()
                            .entry(word)
                            .or_insert(resealed);
                        moved += 1;
                    }
                    None => {
                        saved
                            .keys
                            .entry(tenant.clone())
                            .or_default()
                            .insert(word, kept);
                        left += 1;
                    }
                }
            }
        }
        if moved > 0 {
            self.save(&saved)?;
        }
        Ok((moved, left))
    }

    /// A tenant-sealed key record sealed again for `workspace`, or `None`
    /// when it no longer opens.
    fn resealed(
        &self,
        tenant: &str,
        word: &str,
        workspace: &str,
        record: serde_json::Value,
    ) -> Option<serde_json::Value> {
        let provider = provider(word)?;
        let mut kept: Kept = serde_json::from_value(record).ok()?;
        let plain = self
            .keyring
            .open(&associated_by_tenant(tenant, provider), &kept.sealed)
            .ok()?;
        kept.sealed = self
            .keyring
            .seal(&associated(workspace, provider), &plain)
            .ok()?;
        serde_json::to_value(&kept).ok()
    }
}

/// An adapter on the caller's key, billed to no account of ours.
fn adapter(provider: Provider, key: Secret) -> Arc<dyn Upstream> {
    use inference::upstream::{
        anthropic::Anthropic, gemini::Gemini, openai, openrouter, responses::ResponsesUpstream,
        vercel,
    };
    let mut config = match provider {
        Provider::Vercel => vercel::config(Some(key)),
        Provider::OpenRouter => openrouter::config(Some(key)),
        Provider::OpenAi => openai::config(Some(key)),
        Provider::Anthropic => return Arc::new(Anthropic::new(key)),
        Provider::Google => return Arc::new(Gemini::new(key)),
    };
    config.account = Account {
        id: inference::run::CALLER_KEY.to_owned(),
        basis: CostBasis::PayAsYouGo,
    };
    Arc::new(ResponsesUpstream::new(config))
}

/// The caller's own adapters: the keys of the workspace its API key acts
/// in, when BYOK is set up and that workspace keeps any.
#[must_use]
pub fn own(state: &ServeState, workspace: &str) -> inference::run::OwnUpstreams {
    inference::run::OwnUpstreams(
        state
            .provider_keys
            .as_ref()
            .map(|keys| keys.upstreams(workspace))
            .unwrap_or_default(),
    )
}

// ----------------------------------------------------------------- routes

/// The workspace, for a signed-in owner or admin of it.
fn admin_workspace(
    state: &ServeState,
    headers: &HeaderMap,
    workspace: &str,
) -> Result<String, Response> {
    let principal = principal(state, headers)?;
    if principal.account().is_none() || principal_is_key(headers) {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "session_required",
            "Sign in to manage provider keys. An API key can't change them.",
        ));
    }
    let account = member_account(&principal)?.to_string();
    let membership = member(state, &account, workspace)?;
    if membership.role < tenancy::Role::Admin {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "Only the workspace's owner or an admin can manage its provider keys.",
        ));
    }
    let accounts = accounts_store(state)?;
    let store = accounts.store().map_err(|trouble| {
        refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "accounts_unavailable",
            trouble.to_string(),
        )
    })?;
    store
        .workspaces
        .get(workspace)
        .map(|ws| ws.id.clone())
        .ok_or_else(|| {
            refused(
                StatusCode::NOT_FOUND,
                "unknown_workspace",
                "This workspace no longer exists.",
            )
        })
}

fn principal_is_key(headers: &HeaderMap) -> bool {
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.starts_with("Bearer oak_"))
}

fn keys(state: &ServeState) -> Result<&Keys, Response> {
    state.provider_keys.as_deref().ok_or_else(|| {
        refused(
            StatusCode::NOT_FOUND,
            "byok_unavailable",
            "This server doesn't keep provider keys.",
        )
    })
}

fn unknown_provider(word: &str) -> Response {
    let shown: String = word.chars().take(40).collect();
    refused(
        StatusCode::NOT_FOUND,
        "unknown_provider",
        format!(
            "`{shown}` isn't a provider whose key the API can use. Use openrouter, vercel, anthropic, openai, or google."
        ),
    )
}

async fn list(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath(workspace): UrlPath<String>,
) -> Response {
    let workspace = match admin_workspace(&state, &headers, &workspace) {
        Ok(workspace) => workspace,
        Err(response) => return response,
    };
    let keys = match keys(&state) {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    let listed: Vec<_> = keys
        .listed(&workspace)
        .into_iter()
        .map(|(word, fingerprint, added_at)| {
            json!({"provider": word, "fingerprint": fingerprint, "added_at": added_at})
        })
        .collect();
    Json(json!({"keys": listed})).into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PutKey {
    key: String,
}

async fn store(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath((workspace, word)): UrlPath<(String, String)>,
    body: axum::body::Bytes,
) -> Response {
    let workspace = match admin_workspace(&state, &headers, &workspace) {
        Ok(workspace) => workspace,
        Err(response) => return response,
    };
    let keys = match keys(&state) {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    let Some(provider) = provider(&word) else {
        return unknown_provider(&word);
    };
    // Never echo the body: it holds the key.
    let Ok(input) = serde_json::from_slice::<PutKey>(&body) else {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Send {\"key\": \"...\"} with the provider's API key.",
        );
    };
    let input = Zeroizing::new(input.key);
    let key = input.trim();
    if let Err((code, message)) = admit_key(key) {
        return refused(StatusCode::BAD_REQUEST, code, message);
    }
    if key.is_empty() || key.len() > KEY_MAX || key.chars().any(char::is_whitespace) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_key",
            format!("That doesn't look like a {} key.", provider.name()),
        );
    }
    match keys.put(&workspace, provider, key) {
        Ok(fingerprint) => (
            StatusCode::OK,
            Json(json!({"provider": provider.word(), "fingerprint": fingerprint})),
        )
            .into_response(),
        Err(_) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "byok_unavailable",
            "The key couldn't be saved right now. Try again in a minute.",
        ),
    }
}

/// Why a Claude subscription token is refused as a provider key.
pub const SUBSCRIPTION_REFUSAL: &str = "That's a Claude subscription token (from claude setup-token). Anthropic allows those only in Claude Code, so the API can't call models with it. Use an Anthropic API key (sk-ant-api...) here; save the token on openagents.com under Settings, Claude credential, for Claude Code runs.";

/// Refuse a key the API may not call models with: a Claude subscription
/// (OAuth) token, for every provider. Anthropic permits those only inside
/// Claude Code; the gateway calls the Messages API directly.
fn admit_key(key: &str) -> Result<(), (&'static str, &'static str)> {
    if key.starts_with("sk-ant-oat") || key.starts_with("sk-ant-ort") {
        return Err(("subscription_token", SUBSCRIPTION_REFUSAL));
    }
    Ok(())
}

async fn remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath((workspace, word)): UrlPath<(String, String)>,
) -> Response {
    let workspace = match admin_workspace(&state, &headers, &workspace) {
        Ok(workspace) => workspace,
        Err(response) => return response,
    };
    let keys = match keys(&state) {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    let Some(provider) = provider(&word) else {
        return unknown_provider(&word);
    };
    match keys.delete(&workspace, provider) {
        Ok(true) => Json(json!({"provider": provider.word(), "deleted": true})).into_response(),
        Ok(false) => refused(
            StatusCode::NOT_FOUND,
            "no_key",
            format!("This workspace keeps no {} key.", provider.name()),
        ),
        Err(_) => refused(
            StatusCode::SERVICE_UNAVAILABLE,
            "byok_unavailable",
            "The key couldn't be removed right now. Try again in a minute.",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_claude_subscription_token_is_never_a_provider_key() {
        // Assembled at run time so no token-shaped literal sits here.
        let token = format!("sk-ant-oat01-{}", "g5".repeat(40));
        let (code, message) = admit_key(&token).unwrap_err();
        assert_eq!(code, "subscription_token");
        assert!(message.contains("only in Claude Code"));
        assert!(!message.contains(&token));
        assert!(admit_key(&format!("sk-ant-ort01-{}", "g5".repeat(40))).is_err());
        assert!(admit_key("sk-ant-api03-stub").is_ok());
        assert!(admit_key("sk-or-v1-stub").is_ok());
    }

    #[test]
    fn direct_provider_keys_are_sealed_and_bound_to_the_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let (keyring, _) = Keyring::scratch("direct").unwrap();
        let keys = Keys::with_keyring(dir.path(), keyring).unwrap();
        for word in ["anthropic", "openai", "google"] {
            let provider = provider(word).unwrap();
            keys.put("acme", provider, "stub-credential").unwrap();
        }
        let adapters = keys.upstreams("acme");
        assert_eq!(adapters.len(), 3);
        assert!(keys.upstreams("other").is_empty());
        for adapter in adapters {
            assert_eq!(adapter.account().id, inference::run::CALLER_KEY);
            assert!(adapter.configured());
            assert!(!adapter.models().is_empty());
            assert!(
                !adapter
                    .privacy()
                    .allows(&inference::openagents::Privacy::Strict)
            );
        }
        assert!(
            !std::fs::read_to_string(dir.path().join(STORE))
                .unwrap()
                .contains("stub-credential")
        );
    }

    #[test]
    fn keys_are_sealed_per_workspace_and_never_kept_in_the_clear() {
        let dir = tempfile::tempdir().unwrap();
        let (keyring, _) = Keyring::scratch("k1").unwrap();
        let keys = Keys::with_keyring(dir.path(), keyring).unwrap();
        let fingerprint = keys
            .put("acme", Provider::OpenRouter, "sk-or-v1-secret")
            .unwrap();
        assert_eq!(fingerprint, model_access::fingerprint("sk-or-v1-secret"));
        let file = std::fs::read_to_string(dir.path().join(STORE)).unwrap();
        assert!(!file.contains("sk-or-v1-secret"));
        assert_eq!(keys.upstreams("acme").len(), 1);
        assert!(keys.upstreams("other").is_empty());
        assert_eq!(
            keys.upstreams("acme")[0].account().id,
            inference::run::CALLER_KEY
        );
        // A record moved to another workspace does not open there.
        {
            let mut saved = keys.saved.lock().unwrap();
            let moved = saved.workspaces.remove("acme").unwrap();
            saved.workspaces.insert("other".into(), moved);
        }
        assert!(keys.upstreams("other").is_empty());
        assert_eq!(
            keys.listed("other").len(),
            1,
            "still listed, to be replaced"
        );
        {
            let mut saved = keys.saved.lock().unwrap();
            let moved = saved.workspaces.remove("other").unwrap();
            saved.workspaces.insert("acme".into(), moved);
        }
        assert!(keys.delete("acme", Provider::OpenRouter).unwrap());
        assert!(!keys.delete("acme", Provider::OpenRouter).unwrap());
        assert!(provider("typesafe").is_none());
        assert_eq!(provider("vercel"), Some(Provider::Vercel));
    }

    /// The same store kept in the account database (#11154): sealed,
    /// shared by two gateways at once. Runs with
    /// `TENANCY_TEST_DATABASE_URL` set; passes without it.
    #[test]
    fn keys_in_the_account_database_are_sealed_and_shared() {
        let Some(url) = std::env::var("TENANCY_TEST_DATABASE_URL")
            .ok()
            .filter(|u| !u.is_empty())
        else {
            eprintln!("skipped: TENANCY_TEST_DATABASE_URL is not set");
            return;
        };
        let database = tenancy::db::Database::scratch(&url).unwrap();
        let (dir, other) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        tenancy::db::attach(dir.path(), database.clone());
        tenancy::db::attach(other.path(), database.clone());
        let (keyring, document) = Keyring::scratch("k1").unwrap();
        let keys = Keys::with_keyring(dir.path(), keyring).unwrap();
        let theirs =
            Keys::with_keyring(other.path(), Keyring::parse(document.as_bytes()).unwrap()).unwrap();
        keys.put("acme", Provider::OpenRouter, "sk-or-v1-secret")
            .unwrap();
        assert!(!dir.path().join(STORE).exists());
        let rows = database
            .query("SELECT record::text FROM identity.provider_keys", &[])
            .unwrap();
        assert!(!rows[0].get::<_, String>(0).contains("sk-or-v1-secret"));
        // The other gateway sees it at once, and opens it.
        assert_eq!(theirs.listed("acme").len(), 1);
        assert_eq!(theirs.upstreams("acme").len(), 1);
        assert!(theirs.upstreams("other").is_empty());
        assert!(theirs.delete("acme", Provider::OpenRouter).unwrap());
        assert!(keys.listed("acme").is_empty());
        assert!(!keys.delete("acme", Provider::OpenRouter).unwrap());
    }

    /// Accounts with one workspace on the tenant `solo` and two on the
    /// shared tenant `signup`: (store, solo's, Ada's, Bo's).
    fn three_workspaces(dir: &Path) -> (tenancy::accounts::Store, String, String, String) {
        let accounts = tenancy::Accounts::install(dir).unwrap();
        let mut made = Vec::new();
        for (label, tenant) in [("Solo", "solo"), ("Ada", "signup"), ("Bo", "signup")] {
            let account = accounts.create_account(label, &[]).unwrap();
            let ws = accounts
                .create_workspace(
                    &account.id,
                    label,
                    tenancy::WorkspaceKind::Personal,
                    tenant,
                    None,
                )
                .unwrap();
            made.push(ws.id);
        }
        let store = accounts.store().unwrap();
        let bo = made.pop().unwrap();
        let ada = made.pop().unwrap();
        let solo = made.pop().unwrap();
        (store, solo, ada, bo)
    }

    /// A key as the store sealed it before #11186: by tenant.
    fn sealed_by_tenant(keyring: &Keyring, tenant: &str, key: &str) -> Kept {
        Kept {
            sealed: keyring
                .seal(
                    &associated_by_tenant(tenant, Provider::OpenRouter),
                    key.as_bytes(),
                )
                .unwrap(),
            fingerprint: model_access::fingerprint(key),
            added_at: 1,
        }
    }

    /// #11186: keys saved by tenant move to the tenant's only workspace;
    /// a key on the shared sign-up tenant goes to no one's workspace.
    #[test]
    fn tenant_kept_keys_move_to_their_only_workspace_and_never_to_a_shared_one() {
        let dir = tempfile::tempdir().unwrap();
        let (store, solo, ada, bo) = three_workspaces(dir.path());
        let (keyring, document) = Keyring::scratch("k1").unwrap();
        let mut legacy = Saved::default();
        for (tenant, key) in [("solo", "sk-or-v1-solo"), ("signup", "sk-or-v1-someone")] {
            legacy
                .keys
                .entry(tenant.into())
                .or_default()
                .insert("openrouter".into(), sealed_by_tenant(&keyring, tenant, key));
        }
        std::fs::write(dir.path().join(STORE), serde_json::to_vec(&legacy).unwrap()).unwrap();
        let keys = Keys::with_keyring(dir.path(), keyring).unwrap();
        // Before the move, nobody's call uses either key.
        for workspace in [&solo, &ada, &bo] {
            assert!(keys.upstreams(workspace).is_empty());
            assert!(keys.listed(workspace).is_empty());
        }
        assert_eq!(keys.adopt(&store).unwrap(), (1, 1));
        assert_eq!(keys.upstreams(&solo).len(), 1);
        assert!(keys.upstreams(&ada).is_empty());
        assert!(keys.upstreams(&bo).is_empty());
        // Kept across a restart, and moving again changes nothing.
        let reopened =
            Keys::with_keyring(dir.path(), Keyring::parse(document.as_bytes()).unwrap()).unwrap();
        assert_eq!(reopened.upstreams(&solo).len(), 1);
        assert_eq!(reopened.adopt(&store).unwrap(), (0, 1));
        let file = std::fs::read_to_string(dir.path().join(STORE)).unwrap();
        assert!(!file.contains("sk-or-v1"));
    }

    /// The same in the account database, with the shared tenant's key
    /// named for Ada's workspace by an operator. Runs with
    /// `TENANCY_TEST_DATABASE_URL` set; passes without it.
    #[test]
    fn a_named_tenant_key_moves_to_that_workspace_in_the_database() {
        let Some(url) = std::env::var("TENANCY_TEST_DATABASE_URL")
            .ok()
            .filter(|u| !u.is_empty())
        else {
            eprintln!("skipped: TENANCY_TEST_DATABASE_URL is not set");
            return;
        };
        let database = tenancy::db::Database::scratch(&url).unwrap();
        let dir = tempfile::tempdir().unwrap();
        tenancy::db::attach(dir.path(), database.clone());
        let (store, solo, ada, bo) = three_workspaces(dir.path());
        let (keyring, _) = Keyring::scratch("k1").unwrap();
        for (tenant, key) in [("solo", "sk-or-v1-solo"), ("signup", "sk-or-v1-ada")] {
            let record = serde_json::to_value(sealed_by_tenant(&keyring, tenant, key)).unwrap();
            database
                .execute(
                    "INSERT INTO identity.provider_keys_by_tenant (tenant, provider, record) VALUES ($1, 'openrouter', $2)",
                    &[&tenant, &record],
                )
                .unwrap();
        }
        let keys = Keys::with_keyring(dir.path(), keyring).unwrap();
        assert_eq!(keys.adopt(&store).unwrap(), (1, 1));
        assert!(keys.upstreams(&ada).is_empty());
        tenancy::db::records::assign_tenant_provider_key(&database, "signup", "openrouter", &ada)
            .unwrap();
        assert_eq!(keys.adopt(&store).unwrap(), (1, 0));
        assert_eq!(keys.upstreams(&solo).len(), 1);
        assert_eq!(keys.upstreams(&ada).len(), 1);
        assert!(keys.upstreams(&bo).is_empty());
        assert!(
            tenancy::db::records::tenant_provider_keys(&database)
                .unwrap()
                .is_empty()
        );
    }
}
