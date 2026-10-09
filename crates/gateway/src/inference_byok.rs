//! Bring your own key on the inference API (`docs/inference/gateway.md`,
//! sections 4 and 9; #11067).
//!
//! A workspace keeps its own provider keys here,
//! sealed with [`oa_seal`] (AES-256-GCM under a keyring read from
//! `inference.byok.keyring`, a private file outside the registry, so a copy
//! of the registry holds only ciphertext). Each sealed key is bound to its
//! tenant and provider. A key is never logged, never answered back: the
//! routes show the provider and [`model_access::fingerprint`] only, as the
//! desktop's key store does.
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

/// tenant -> provider word -> key.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Saved {
    #[serde(default)]
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

fn associated(tenant: &str, provider: Provider) -> Vec<u8> {
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

    /// Seals and keeps `key` for `tenant`, replacing any key it held for
    /// `provider`. Returns the fingerprint.
    ///
    /// # Errors
    ///
    /// Sealing or writing failed.
    pub fn put(&self, tenant: &str, provider: Provider, key: &str) -> Result<String, String> {
        let sealed = self
            .keyring
            .seal(&associated(tenant, provider), key.as_bytes())
            .map_err(|error| error.to_string())?;
        let fingerprint = model_access::fingerprint(key);
        if let Some(database) = &self.database {
            let kept = Kept {
                sealed,
                fingerprint: fingerprint.clone(),
                added_at: now(),
            };
            let record = serde_json::to_value(&kept).map_err(|e| e.to_string())?;
            tenancy::db::records::put_provider_key(database, tenant, provider.word(), &record)
                .map_err(|e| e.to_string())?;
            return Ok(fingerprint);
        }
        let mut saved = self.saved.lock().map_err(|_| "The key store is busy.")?;
        saved.keys.entry(tenant.to_owned()).or_default().insert(
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

    /// Forgets `tenant`'s key for `provider`; false when there was none.
    ///
    /// # Errors
    ///
    /// Writing failed.
    pub fn delete(&self, tenant: &str, provider: Provider) -> Result<bool, String> {
        if let Some(database) = &self.database {
            return tenancy::db::records::delete_provider_key(database, tenant, provider.word())
                .map_err(|e| e.to_string());
        }
        let mut saved = self.saved.lock().map_err(|_| "The key store is busy.")?;
        let removed = saved
            .keys
            .get_mut(tenant)
            .and_then(|keys| keys.remove(provider.word()))
            .is_some();
        if removed {
            self.save(&saved)?;
        }
        Ok(removed)
    }

    /// `tenant`'s kept keys, by provider word.
    fn kept(&self, tenant: &str) -> BTreeMap<String, Kept> {
        if let Some(database) = &self.database {
            return tenancy::db::records::provider_keys(database, tenant)
                .unwrap_or_default()
                .into_iter()
                .filter_map(|(word, record)| Some((word, serde_json::from_value(record).ok()?)))
                .collect();
        }
        self.saved
            .lock()
            .ok()
            .and_then(|saved| saved.keys.get(tenant).cloned())
            .unwrap_or_default()
    }

    /// What `tenant` keeps: provider, fingerprint, when added.
    #[must_use]
    pub fn listed(&self, tenant: &str) -> Vec<(String, String, u64)> {
        self.kept(tenant)
            .into_iter()
            .map(|(word, kept)| (word, kept.fingerprint, kept.added_at))
            .collect()
    }

    /// Adapters on `tenant`'s own keys. A key that no longer opens is left
    /// out (and its fingerprint stays listed, so the owner can replace it).
    #[must_use]
    pub fn upstreams(&self, tenant: &str) -> Vec<Arc<dyn Upstream>> {
        let opened: Vec<(Provider, Zeroizing<Vec<u8>>)> = self
            .kept(tenant)
            .iter()
            .filter_map(|(word, kept)| {
                let provider = provider(word)?;
                let plain = self
                    .keyring
                    .open(&associated(tenant, provider), &kept.sealed)
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

/// The caller's own adapters, when BYOK is set up and the tenant keeps any.
#[must_use]
pub fn own(state: &ServeState, tenant: &str) -> inference::run::OwnUpstreams {
    inference::run::OwnUpstreams(
        state
            .provider_keys
            .as_ref()
            .map(|keys| keys.upstreams(tenant))
            .unwrap_or_default(),
    )
}

// ----------------------------------------------------------------- routes

/// The workspace's tenant, for a signed-in owner or admin of it.
fn admin_tenant(
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
        .map(|ws| ws.tenant.clone())
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
    let tenant = match admin_tenant(&state, &headers, &workspace) {
        Ok(tenant) => tenant,
        Err(response) => return response,
    };
    let keys = match keys(&state) {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    let listed: Vec<_> = keys
        .listed(&tenant)
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
    let tenant = match admin_tenant(&state, &headers, &workspace) {
        Ok(tenant) => tenant,
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
    if key.is_empty() || key.len() > KEY_MAX || key.chars().any(char::is_whitespace) {
        return refused(
            StatusCode::BAD_REQUEST,
            "invalid_key",
            format!("That doesn't look like a {} key.", provider.name()),
        );
    }
    match keys.put(&tenant, provider, key) {
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

async fn remove(
    State(state): State<Arc<ServeState>>,
    headers: HeaderMap,
    UrlPath((workspace, word)): UrlPath<(String, String)>,
) -> Response {
    let tenant = match admin_tenant(&state, &headers, &workspace) {
        Ok(tenant) => tenant,
        Err(response) => return response,
    };
    let keys = match keys(&state) {
        Ok(keys) => keys,
        Err(response) => return response,
    };
    let Some(provider) = provider(&word) else {
        return unknown_provider(&word);
    };
    match keys.delete(&tenant, provider) {
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
    fn direct_provider_keys_are_sealed_and_bound_to_the_tenant() {
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
    fn keys_are_sealed_per_tenant_and_never_kept_in_the_clear() {
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
        // A record moved to another tenant does not open there.
        {
            let mut saved = keys.saved.lock().unwrap();
            let moved = saved.keys.remove("acme").unwrap();
            saved.keys.insert("other".into(), moved);
        }
        assert!(keys.upstreams("other").is_empty());
        assert_eq!(
            keys.listed("other").len(),
            1,
            "still listed, to be replaced"
        );
        {
            let mut saved = keys.saved.lock().unwrap();
            let moved = saved.keys.remove("other").unwrap();
            saved.keys.insert("acme".into(), moved);
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
}
