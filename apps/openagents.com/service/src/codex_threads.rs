use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::config::Config;

#[derive(Clone)]
pub struct CodexThreadStore {
    state: Arc<RwLock<ThreadStoreState>>,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
struct ThreadStoreState {
    threads: HashMap<String, ThreadProjectionRecord>,
    messages_by_thread: HashMap<String, Vec<ThreadMessageRecord>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThreadProjectionRecord {
    thread_id: String,
    user_id: String,
    org_id: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
    message_count: u32,
    last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ThreadMessageRecord {
    message_id: String,
    thread_id: String,
    user_id: String,
    role: String,
    text: String,
    created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadProjection {
    pub thread_id: String,
    pub user_id: String,
    pub org_id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub message_count: u32,
    pub last_message_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ThreadMessageProjection {
    pub message_id: String,
    pub thread_id: String,
    pub role: String,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppendThreadMessageResult {
    pub thread: ThreadProjection,
    pub message: ThreadMessageProjection,
}

#[derive(Debug, thiserror::Error)]
pub enum ThreadStoreError {
    #[error("thread not found")]
    NotFound,
    #[error("requested thread is not owned by current user")]
    Forbidden,
    #[error("{message}")]
    Persistence { message: String },
}

impl CodexThreadStore {
    pub fn from_config(config: &Config) -> Self {
        let path = config.codex_thread_store_path.clone();
        let state = Self::load_state(path.as_ref());

        Self {
            state: Arc::new(RwLock::new(state)),
            path,
        }
    }

    pub async fn append_user_message(
        &self,
        user_id: &str,
        org_id: &str,
        thread_id: &str,
        text: String,
    ) -> Result<AppendThreadMessageResult, ThreadStoreError> {
        let now = Utc::now();
        let (result, snapshot) = {
            let mut state = self.state.write().await;

            if let Some(existing) = state.threads.get(thread_id) {
                if existing.user_id != user_id {
                    return Err(ThreadStoreError::Forbidden);
                }
            }

            let thread_projection = {
                let thread = state
                    .threads
                    .entry(thread_id.to_string())
                    .or_insert_with(|| ThreadProjectionRecord {
                        thread_id: thread_id.to_string(),
                        user_id: user_id.to_string(),
                        org_id: org_id.to_string(),
                        created_at: now,
                        updated_at: now,
                        message_count: 0,
                        last_message_at: None,
                    });

                thread.org_id = org_id.to_string();
                thread.updated_at = now;
                thread.message_count = thread.message_count.saturating_add(1);
                thread.last_message_at = Some(now);
                ThreadProjection::from_record(thread)
            };

            let message = ThreadMessageRecord {
                message_id: format!("msg_{}", Uuid::new_v4().simple()),
                thread_id: thread_id.to_string(),
                user_id: user_id.to_string(),
                role: "user".to_string(),
                text,
                created_at: now,
            };

            state
                .messages_by_thread
                .entry(thread_id.to_string())
                .or_default()
                .push(message.clone());

            let result = AppendThreadMessageResult {
                thread: thread_projection,
                message: ThreadMessageProjection::from_record(&message),
            };

            (result, state.clone())
        };

        self.persist_state(&snapshot).await?;
        Ok(result)
    }

    pub async fn list_threads_for_user(
        &self,
        user_id: &str,
        org_id: Option<&str>,
    ) -> Result<Vec<ThreadProjection>, ThreadStoreError> {
        let state = self.state.read().await;

        let mut threads: Vec<ThreadProjection> = state
            .threads
            .values()
            .filter(|thread| thread.user_id == user_id)
            .filter(|thread| {
                org_id
                    .map(|expected| thread.org_id == expected)
                    .unwrap_or(true)
            })
            .map(ThreadProjection::from_record)
            .collect();

        threads.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(threads)
    }

    pub async fn list_thread_messages_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<Vec<ThreadMessageProjection>, ThreadStoreError> {
        let state = self.state.read().await;
        let thread = state
            .threads
            .get(thread_id)
            .ok_or(ThreadStoreError::NotFound)?;
        if thread.user_id != user_id {
            return Err(ThreadStoreError::Forbidden);
        }

        let mut messages: Vec<ThreadMessageProjection> = state
            .messages_by_thread
            .get(thread_id)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|record| ThreadMessageProjection::from_record(&record))
            .collect();
        messages.sort_by(|left, right| left.created_at.cmp(&right.created_at));
        Ok(messages)
    }

    fn load_state(path: Option<&PathBuf>) -> ThreadStoreState {
        let Some(path) = path else {
            return ThreadStoreState::default();
        };

        let raw = match std::fs::read_to_string(path) {
            Ok(value) => value,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return ThreadStoreState::default();
            }
            Err(error) => {
                tracing::warn!(
                    target: "openagents.codex_threads",
                    path = %path.display(),
                    error = %error,
                    "failed to read codex thread store; booting with empty thread state",
                );
                return ThreadStoreState::default();
            }
        };

        match serde_json::from_str::<ThreadStoreState>(&raw) {
            Ok(state) => state,
            Err(error) => {
                tracing::warn!(
                    target: "openagents.codex_threads",
                    path = %path.display(),
                    error = %error,
                    "failed to parse codex thread store; booting with empty thread state",
                );
                ThreadStoreState::default()
            }
        }
    }

    async fn persist_state(&self, snapshot: &ThreadStoreState) -> Result<(), ThreadStoreError> {
        let Some(path) = self.path.as_ref() else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.map_err(|error| {
                ThreadStoreError::Persistence {
                    message: format!("failed to prepare codex thread store directory: {error}"),
                }
            })?;
        }

        let payload =
            serde_json::to_vec(snapshot).map_err(|error| ThreadStoreError::Persistence {
                message: format!("failed to encode codex thread store payload: {error}"),
            })?;
        let temp_path = path.with_extension(format!("{}.tmp", Uuid::new_v4().simple()));

        tokio::fs::write(&temp_path, payload)
            .await
            .map_err(|error| ThreadStoreError::Persistence {
                message: format!("failed to write codex thread store payload: {error}"),
            })?;

        tokio::fs::rename(&temp_path, path).await.map_err(|error| {
            ThreadStoreError::Persistence {
                message: format!("failed to finalize codex thread store payload: {error}"),
            }
        })?;

        Ok(())
    }
}

impl ThreadProjection {
    fn from_record(record: &ThreadProjectionRecord) -> Self {
        Self {
            thread_id: record.thread_id.clone(),
            user_id: record.user_id.clone(),
            org_id: record.org_id.clone(),
            created_at: record.created_at,
            updated_at: record.updated_at,
            message_count: record.message_count,
            last_message_at: record.last_message_at,
        }
    }
}

impl ThreadMessageProjection {
    fn from_record(record: &ThreadMessageRecord) -> Self {
        Self {
            message_id: record.message_id.clone(),
            thread_id: record.thread_id.clone(),
            role: record.role.clone(),
            text: record.text.clone(),
            created_at: record.created_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::path::PathBuf;

    use super::CodexThreadStore;
    use crate::config::Config;

    fn test_config(store_path: Option<PathBuf>) -> Config {
        Config {
            bind_addr: SocketAddr::from(([127, 0, 0, 1], 0)),
            log_filter: "debug".to_string(),
            static_dir: std::env::temp_dir(),
            auth_provider_mode: "mock".to_string(),
            workos_client_id: None,
            workos_api_key: None,
            workos_api_base_url: "https://api.workos.com".to_string(),
            mock_magic_code: "123456".to_string(),
            auth_local_test_login_enabled: false,
            auth_local_test_login_allowed_emails: vec![],
            auth_local_test_login_signing_key: None,
            auth_api_signup_enabled: false,
            auth_api_signup_allowed_domains: vec![],
            auth_api_signup_default_token_name: "api-bootstrap".to_string(),
            admin_emails: vec![],
            khala_token_enabled: true,
            khala_token_signing_key: Some("khala-test-signing-key".to_string()),
            khala_token_issuer: "https://openagents.test".to_string(),
            khala_token_audience: "openagents-khala-test".to_string(),
            khala_token_subject_prefix: "user".to_string(),
            khala_token_key_id: "khala-auth-test-v1".to_string(),
            khala_token_claims_version: "oa_khala_claims_v1".to_string(),
            khala_token_ttl_seconds: 300,
            khala_token_min_ttl_seconds: 60,
            khala_token_max_ttl_seconds: 900,
            auth_store_path: None,
            auth_challenge_ttl_seconds: 600,
            auth_access_ttl_seconds: 3600,
            auth_refresh_ttl_seconds: 86400,
            sync_token_enabled: true,
            sync_token_signing_key: Some("sync-test-signing-key".to_string()),
            sync_token_issuer: "https://openagents.test".to_string(),
            sync_token_audience: "openagents-sync-test".to_string(),
            sync_token_key_id: "sync-auth-test-v1".to_string(),
            sync_token_claims_version: "oa_sync_claims_v1".to_string(),
            sync_token_ttl_seconds: 300,
            sync_token_min_ttl_seconds: 60,
            sync_token_max_ttl_seconds: 900,
            sync_token_allowed_scopes: vec!["runtime.codex_worker_events".to_string()],
            sync_token_default_scopes: vec!["runtime.codex_worker_events".to_string()],
            route_split_enabled: false,
            route_split_mode: "legacy".to_string(),
            route_split_rust_routes: vec!["/".to_string()],
            route_split_cohort_percentage: 0,
            route_split_salt: "route-split-test-salt".to_string(),
            route_split_force_legacy: false,
            route_split_legacy_base_url: Some("https://legacy.openagents.test".to_string()),
            runtime_sync_revoke_base_url: None,
            runtime_sync_revoke_path: "/internal/v1/sync/sessions/revoke".to_string(),
            runtime_signature_secret: None,
            runtime_signature_ttl_seconds: 60,
            runtime_internal_shared_secret: None,
            runtime_internal_key_id: "runtime-internal-v1".to_string(),
            runtime_internal_signature_ttl_seconds: 60,
            codex_thread_store_path: store_path,
            domain_store_path: None,
            maintenance_mode_enabled: false,
            maintenance_bypass_token: None,
            maintenance_bypass_cookie_name: "oa_maintenance_bypass".to_string(),
            maintenance_bypass_cookie_ttl_seconds: 900,
            maintenance_allowed_paths: vec!["/healthz".to_string(), "/readyz".to_string()],
            compat_control_enforced: false,
            compat_control_protocol_version: "openagents.control.v1".to_string(),
            compat_control_min_client_build_id: "00000000T000000Z".to_string(),
            compat_control_max_client_build_id: None,
            compat_control_min_schema_version: 1,
            compat_control_max_schema_version: 1,
        }
    }

    #[tokio::test]
    async fn append_and_list_messages_round_trip() {
        let store = CodexThreadStore::from_config(&test_config(None));

        let appended = store
            .append_user_message(
                "usr_1",
                "org:openagents",
                "thread-1",
                "hello from codex".to_string(),
            )
            .await
            .expect("append message");

        assert_eq!(appended.thread.thread_id, "thread-1");
        assert_eq!(appended.thread.message_count, 1);

        let threads = store
            .list_threads_for_user("usr_1", Some("org:openagents"))
            .await
            .expect("list threads");
        assert_eq!(threads.len(), 1);
        assert_eq!(threads[0].thread_id, "thread-1");

        let messages = store
            .list_thread_messages_for_user("usr_1", "thread-1")
            .await
            .expect("list messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "hello from codex");
    }

    #[tokio::test]
    async fn store_persists_to_disk_when_configured() {
        let temp = tempfile::tempdir().expect("tempdir");
        let path = temp.path().join("codex-thread-store.json");
        let config = test_config(Some(path.clone()));

        let store = CodexThreadStore::from_config(&config);
        store
            .append_user_message(
                "usr_2",
                "org:openagents",
                "thread-persisted",
                "persist me".to_string(),
            )
            .await
            .expect("append persisted message");

        let restored = CodexThreadStore::from_config(&config);
        let messages = restored
            .list_thread_messages_for_user("usr_2", "thread-persisted")
            .await
            .expect("read persisted messages");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].text, "persist me");
        assert!(path.is_file());
    }
}
