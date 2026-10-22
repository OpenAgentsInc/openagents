//! An inbox that keeps original NIP-17 ciphertext and serves screened text.
//! The service holds the inbox key; readers authenticate with their own keys.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{OriginalUri, State},
    http::{HeaderMap, StatusCode},
    routing::get,
};
use nostr::domain::Event;
use playtest::suggestion::{self, InboxConfig, Screen};
use secp256k1::SecretKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// The only on-disk message representation. No decrypted text or screen
/// reason is written, including for messages awaiting owner review.
#[derive(Serialize, Deserialize)]
struct Stored {
    received: u64,
    wrap: Event,
}

#[derive(Serialize)]
pub struct Page {
    pub suggestions: Vec<Value>,
    pub next: Option<String>,
}

#[derive(Default, Deserialize)]
struct Paging {
    after: Option<String>,
    limit: Option<usize>,
}

pub struct Store {
    root: PathBuf,
    key: SecretKey,
}

impl Store {
    pub fn new(root: PathBuf, key: SecretKey) -> Result<Self, String> {
        std::fs::create_dir_all(&root).map_err(|e| e.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700))
                .map_err(|e| e.to_string())?;
        }
        Ok(Self { root, key })
    }

    /// Store every valid suggestion, deduplicating by the full rumor ID.
    /// Screening never decides whether a message is kept.
    pub fn receive(&self, event: &Event, received: u64) -> Result<bool, String> {
        let opened = suggestion::open(event, &self.key)?;
        let path = self.root.join(format!("{}.json", opened.rumor_id));
        use std::io::Write;
        let mut file = tempfile::NamedTempFile::new_in(&self.root).map_err(|e| e.to_string())?;
        let bytes = serde_json::to_vec(&Stored {
            received,
            wrap: event.clone(),
        })
        .map_err(|e| e.to_string())?;
        file.write_all(&bytes)
            .and_then(|()| file.as_file().sync_all())
            .map_err(|e| e.to_string())?;
        match file.persist_noclobber(&path) {
            Ok(_) => {}
            Err(error) if error.error.kind() == std::io::ErrorKind::AlreadyExists => {
                // Recover an incomplete record left by an older writer.
                // A valid existing record is the only deduplication proof.
                let valid = std::fs::read(&path)
                    .ok()
                    .and_then(|bytes| serde_json::from_slice::<Stored>(&bytes).ok())
                    .and_then(|stored| suggestion::open(&stored.wrap, &self.key).ok())
                    .is_some_and(|message| message.rumor_id == opened.rumor_id);
                if valid {
                    return Ok(false);
                }
                error.file.persist(&path).map_err(|e| e.error.to_string())?;
            }
            Err(error) => return Err(error.error.to_string()),
        }
        #[cfg(unix)]
        std::fs::File::open(&self.root)
            .and_then(|dir| dir.sync_all())
            .map_err(|e| e.to_string())?;
        Ok(true)
    }

    fn page(&self, after: Option<&str>, limit: usize, quarantine: bool) -> Result<Page, String> {
        let mut files = std::fs::read_dir(&self.root)
            .map_err(|e| e.to_string())?
            .map(|e| e.map(|e| e.path()).map_err(|e| e.to_string()))
            .collect::<Result<Vec<_>, _>>()?;
        files.retain(|path| {
            path.extension().and_then(|x| x.to_str()) == Some("json")
                && after.is_none_or(|cursor| {
                    path.file_stem()
                        .and_then(|x| x.to_str())
                        .is_some_and(|id| id > cursor)
                })
        });
        files.sort();
        let mut suggestions = Vec::new();
        let mut next = None;
        for (index, path) in files.iter().enumerate() {
            let stored: Stored =
                serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            let message = suggestion::open(&stored.wrap, &self.key)?;
            if quarantine && message.screen != Screen::Normal {
                suggestions.push(json!({"code":message.code,"sender":message.sender,"screen":message.screen,"suggestion":message.suggestion}));
            } else if !quarantine && let Some(quoted) = suggestion::for_agents(&message) {
                suggestions.push(json!({"code":message.code,"client":message.suggestion.client,
                    "category":message.suggestion.category,"place":message.suggestion.place,"content":quoted}));
            }
            if suggestions.len() >= limit.clamp(1, 100) {
                if index + 1 < files.len() {
                    next = Some(message.rumor_id);
                }
                break;
            }
        }
        Ok(Page { suggestions, next })
    }

    /// This API has no switch that reveals quarantine or original wraps.
    pub fn agent_page(&self, after: Option<&str>, limit: usize) -> Result<Page, String> {
        self.page(after, limit, false)
    }
}

/// Server configuration. Keep the file and inbox key in a service account
/// that delegated agents cannot access. Only public keys leave this process.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    pub key_file: PathBuf,
    pub data: PathBuf,
    pub owner: String,
    pub agents: Vec<String>,
    pub relays: Vec<String>,
    pub public_origin: String,
    pub bind: String,
}

pub struct Service {
    pub store: Store,
    pub config: InboxConfig,
    owner: String,
    origin: String,
}

impl Service {
    pub fn new(config: &Config) -> Result<Self, String> {
        let key = read_key(&config.key_file)?;
        let inbox = key
            .x_only_public_key(&secp256k1::Secp256k1::new())
            .0
            .to_string();
        let mut public = InboxConfig {
            inbox,
            relays: config.relays.clone(),
            agents: config.agents.clone(),
        };
        public.check()?;
        let owner = config
            .owner
            .parse::<secp256k1::XOnlyPublicKey>()
            .map_err(|_| "Invalid owner public key")?
            .to_string();
        public.agents = public
            .agents
            .iter()
            .map(|key| {
                key.parse::<secp256k1::XOnlyPublicKey>()
                    .map(|key| key.to_string())
                    .map_err(|_| "Invalid delegate public key")
            })
            .collect::<Result<Vec<_>, _>>()?;
        if owner == public.inbox {
            return Err("The owner review key must differ from the inbox key.".into());
        }
        if public
            .agents
            .iter()
            .any(|key| key == &owner || key == &public.inbox)
        {
            return Err(
                "Delegated readers must have keys separate from the owner and inbox.".into(),
            );
        }
        if !config.public_origin.starts_with("https://")
            && !config.public_origin.starts_with("http://127.0.0.1:")
        {
            return Err("The inbox needs an HTTPS origin.".into());
        }
        Ok(Self {
            store: Store::new(config.data.clone(), key)?,
            config: public,
            owner,
            origin: config.public_origin.trim_end_matches('/').to_string(),
        })
    }

    fn authorize(&self, headers: &HeaderMap, uri: &str, owner: bool) -> Result<(), StatusCode> {
        let header = headers
            .get("authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or(StatusCode::UNAUTHORIZED)?;
        let proof = nostr::domain::parse_http_authorization_hash(
            header,
            "GET",
            &format!("{}{uri}", self.origin),
            None,
            now(),
        )
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
        if proof.pubkey == self.owner || (!owner && self.config.agents.contains(&proof.pubkey)) {
            Ok(())
        } else {
            Err(StatusCode::FORBIDDEN)
        }
    }
}

pub fn read_key(path: &Path) -> Result<SecretKey, String> {
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    text.trim()
        .parse()
        .map_err(|_| "The inbox key couldn't be read.".into())
}

pub fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub fn router(service: Arc<Service>) -> Router {
    Router::new()
        .route("/config.json", get(configuration))
        .route("/v1/suggestions", get(agent_view))
        .route("/v1/quarantine", get(owner_view))
        .layer(axum::middleware::from_fn(
            |request, next: axum::middleware::Next| async move {
                let mut response = next.run(request).await;
                response.headers_mut().insert(
                    "cache-control",
                    axum::http::HeaderValue::from_static("no-store"),
                );
                response
            },
        ))
        .with_state(service)
}

async fn configuration(State(service): State<Arc<Service>>) -> Json<InboxConfig> {
    Json(service.config.clone())
}

async fn agent_view(
    State(service): State<Arc<Service>>,
    OriginalUri(uri): OriginalUri,
    axum::extract::Query(paging): axum::extract::Query<Paging>,
    headers: HeaderMap,
) -> Result<Json<Page>, StatusCode> {
    service.authorize(&headers, &uri.to_string(), false)?;
    let page = tokio::task::spawn_blocking(move || {
        service
            .store
            .page(paging.after.as_deref(), paging.limit.unwrap_or(50), false)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(page))
}

async fn owner_view(
    State(service): State<Arc<Service>>,
    OriginalUri(uri): OriginalUri,
    axum::extract::Query(paging): axum::extract::Query<Paging>,
    headers: HeaderMap,
) -> Result<Json<Page>, StatusCode> {
    service.authorize(&headers, &uri.to_string(), true)?;
    let page = tokio::task::spawn_blocking(move || {
        service
            .store
            .page(paging.after.as_deref(), paging.limit.unwrap_or(50), true)
    })
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(page))
}

#[cfg(test)]
mod tests;
