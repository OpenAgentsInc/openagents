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
    #[serde(default)]
    autopilot_id: Option<String>,
    #[serde(default)]
    title: Option<String>,
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
pub struct AutopilotThreadProjection {
    pub id: String,
    pub autopilot_id: String,
    pub title: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
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
                        autopilot_id: None,
                        title: None,
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

    pub async fn create_thread_for_user(
        &self,
        user_id: &str,
        org_id: &str,
        thread_id: &str,
    ) -> Result<ThreadProjection, ThreadStoreError> {
        let now = Utc::now();
        let (projection, snapshot) = {
            let mut state = self.state.write().await;

            match state.threads.get_mut(thread_id) {
                Some(existing) => {
                    if existing.user_id != user_id {
                        return Err(ThreadStoreError::Forbidden);
                    }

                    if existing.org_id != org_id {
                        existing.org_id = org_id.to_string();
                        existing.updated_at = now;
                        (ThreadProjection::from_record(existing), Some(state.clone()))
                    } else {
                        (ThreadProjection::from_record(existing), None)
                    }
                }
                None => {
                    let record = ThreadProjectionRecord {
                        thread_id: thread_id.to_string(),
                        user_id: user_id.to_string(),
                        org_id: org_id.to_string(),
                        autopilot_id: None,
                        title: None,
                        created_at: now,
                        updated_at: now,
                        message_count: 0,
                        last_message_at: None,
                    };
                    state.threads.insert(thread_id.to_string(), record.clone());
                    (ThreadProjection::from_record(&record), Some(state.clone()))
                }
            }
        };

        if let Some(snapshot) = snapshot {
            self.persist_state(&snapshot).await?;
        }

        Ok(projection)
    }

    pub async fn get_thread_for_user(
        &self,
        user_id: &str,
        thread_id: &str,
    ) -> Result<ThreadProjection, ThreadStoreError> {
        let state = self.state.read().await;
        let thread = state
            .threads
            .get(thread_id)
            .ok_or(ThreadStoreError::NotFound)?;
        if thread.user_id != user_id {
            return Err(ThreadStoreError::Forbidden);
        }
        Ok(ThreadProjection::from_record(thread))
    }

    pub async fn autopilot_id_for_thread(&self, thread_id: &str) -> Option<String> {
        let state = self.state.read().await;
        state
            .threads
            .get(thread_id)
            .and_then(|thread| thread.autopilot_id.clone())
            .and_then(|value| {
                let trimmed = value.trim().to_string();
                if trimmed.is_empty() {
                    None
                } else {
                    Some(trimmed)
                }
            })
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

    pub async fn create_autopilot_thread_for_user(
        &self,
        user_id: &str,
        org_id: &str,
        thread_id: &str,
        autopilot_id: &str,
        title: &str,
    ) -> Result<AutopilotThreadProjection, ThreadStoreError> {
        let now = Utc::now();
        let normalized_title = title.trim().to_string();
        let (projection, snapshot) = {
            let mut state = self.state.write().await;

            match state.threads.get_mut(thread_id) {
                Some(existing) => {
                    if existing.user_id != user_id {
                        return Err(ThreadStoreError::Forbidden);
                    }

                    let existing_autopilot = existing.autopilot_id.clone();
                    if let Some(existing_autopilot) = existing_autopilot {
                        if existing_autopilot != autopilot_id {
                            return Err(ThreadStoreError::Forbidden);
                        }
                    }

                    let mut did_change = false;
                    if existing.org_id != org_id {
                        existing.org_id = org_id.to_string();
                        did_change = true;
                    }
                    if existing.autopilot_id.as_deref() != Some(autopilot_id) {
                        existing.autopilot_id = Some(autopilot_id.to_string());
                        did_change = true;
                    }
                    if existing
                        .title
                        .as_deref()
                        .map(str::trim)
                        .unwrap_or("")
                        .is_empty()
                    {
                        existing.title = Some(normalized_title.clone());
                        did_change = true;
                    }
                    if did_change {
                        existing.updated_at = now;
                        (
                            AutopilotThreadProjection::from_record(existing)
                                .ok_or(ThreadStoreError::NotFound)?,
                            Some(state.clone()),
                        )
                    } else {
                        (
                            AutopilotThreadProjection::from_record(existing)
                                .ok_or(ThreadStoreError::NotFound)?,
                            None,
                        )
                    }
                }
                None => {
                    let record = ThreadProjectionRecord {
                        thread_id: thread_id.to_string(),
                        user_id: user_id.to_string(),
                        org_id: org_id.to_string(),
                        autopilot_id: Some(autopilot_id.to_string()),
                        title: Some(normalized_title),
                        created_at: now,
                        updated_at: now,
                        message_count: 0,
                        last_message_at: None,
                    };
                    state.threads.insert(thread_id.to_string(), record.clone());
                    (
                        AutopilotThreadProjection::from_record(&record)
                            .ok_or(ThreadStoreError::NotFound)?,
                        Some(state.clone()),
                    )
                }
            }
        };

        if let Some(snapshot) = snapshot {
            self.persist_state(&snapshot).await?;
        }

        Ok(projection)
    }

    pub async fn list_autopilot_threads_for_user(
        &self,
        user_id: &str,
        org_id: Option<&str>,
        autopilot_id: &str,
    ) -> Result<Vec<AutopilotThreadProjection>, ThreadStoreError> {
        let state = self.state.read().await;
        let mut threads: Vec<AutopilotThreadProjection> = state
            .threads
            .values()
            .filter(|thread| thread.user_id == user_id)
            .filter(|thread| {
                org_id
                    .map(|expected| thread.org_id == expected)
                    .unwrap_or(true)
            })
            .filter(|thread| thread.autopilot_id.as_deref() == Some(autopilot_id))
            .filter_map(AutopilotThreadProjection::from_record)
            .collect();

        threads.sort_by(|left, right| right.updated_at.cmp(&left.updated_at));
        Ok(threads)
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

impl AutopilotThreadProjection {
    fn from_record(record: &ThreadProjectionRecord) -> Option<Self> {
        let autopilot_id = record.autopilot_id.clone()?;
        let title = record
            .title
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| "New conversation".to_string());
        Some(Self {
            id: record.thread_id.clone(),
            autopilot_id,
            title,
            created_at: record.created_at,
            updated_at: record.updated_at,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::CodexThreadStore;
    use crate::config::Config;

    fn test_config(store_path: Option<PathBuf>) -> Config {
        let mut config = Config::for_tests(std::env::temp_dir());
        config.route_split_enabled = false;
        config.route_split_mode = "legacy".to_string();
        config.route_split_rust_routes = vec!["/".to_string()];
        config.route_split_cohort_percentage = 0;
        config.auth_store_path = None;
        config.codex_thread_store_path = store_path;
        config.smoke_stream_secret = None;
        config
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

    #[tokio::test]
    async fn create_and_get_thread_respects_user_boundary() {
        let store = CodexThreadStore::from_config(&test_config(None));

        let created = store
            .create_thread_for_user("usr_owner", "org:openagents", "thread-owned")
            .await
            .expect("create thread");
        assert_eq!(created.thread_id, "thread-owned");
        assert_eq!(created.message_count, 0);

        let fetched = store
            .get_thread_for_user("usr_owner", "thread-owned")
            .await
            .expect("get thread");
        assert_eq!(fetched.thread_id, "thread-owned");

        let forbidden = store
            .get_thread_for_user("usr_other", "thread-owned")
            .await
            .expect_err("user boundary should be enforced");
        assert!(matches!(
            forbidden,
            crate::codex_threads::ThreadStoreError::Forbidden
        ));
    }

    #[tokio::test]
    async fn autopilot_thread_projection_scopes_by_autopilot() {
        let store = CodexThreadStore::from_config(&test_config(None));

        let created = store
            .create_autopilot_thread_for_user(
                "usr_owner",
                "org:openagents",
                "thread-ap-1",
                "ap_1",
                "Autopilot One",
            )
            .await
            .expect("create autopilot thread");
        assert_eq!(created.id, "thread-ap-1");
        assert_eq!(created.autopilot_id, "ap_1");
        assert_eq!(created.title, "Autopilot One");

        store
            .create_autopilot_thread_for_user(
                "usr_owner",
                "org:openagents",
                "thread-ap-2",
                "ap_2",
                "Autopilot Two",
            )
            .await
            .expect("create second autopilot thread");

        let ap_1_threads = store
            .list_autopilot_threads_for_user("usr_owner", Some("org:openagents"), "ap_1")
            .await
            .expect("list autopilot threads");
        assert_eq!(ap_1_threads.len(), 1);
        assert_eq!(ap_1_threads[0].id, "thread-ap-1");

        let ap_2_threads = store
            .list_autopilot_threads_for_user("usr_owner", Some("org:openagents"), "ap_2")
            .await
            .expect("list autopilot threads");
        assert_eq!(ap_2_threads.len(), 1);
        assert_eq!(ap_2_threads[0].id, "thread-ap-2");

        let none = store
            .list_autopilot_threads_for_user("usr_owner", Some("org:openagents"), "ap_3")
            .await
            .expect("list autopilot threads");
        assert!(none.is_empty());
    }
}
