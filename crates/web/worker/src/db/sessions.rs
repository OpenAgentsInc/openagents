//! KV-based session management

use serde::{Deserialize, Serialize};
use worker::*;

const SESSION_TTL_SECONDS: u64 = 30 * 24 * 60 * 60; // 30 days
pub const SESSION_COOKIE_NAME: &str = "oa_session";

/// Session data stored in KV
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    pub user_id: String,
    pub github_username: String,
    pub github_oauth_state: Option<String>,
    pub created_at: String,
    pub last_active_at: String,
}

impl Session {
    /// Create a new session and store in KV
    pub async fn create(
        kv: &kv::KvStore,
        user_id: &str,
        github_username: &str,
    ) -> Result<String> {
        let token = generate_secure_token();
        let now = chrono::Utc::now().to_rfc3339();

        let session = Session {
            user_id: user_id.to_string(),
            github_username: github_username.to_string(),
            github_oauth_state: None,
            created_at: now.clone(),
            last_active_at: now,
        };

        let json = serde_json::to_string(&session)
            .map_err(|e| Error::RustError(format!("JSON serialize error: {}", e)))?;

        kv.put(&format!("session:{}", token), json)?
            .expiration_ttl(SESSION_TTL_SECONDS)
            .execute()
            .await?;

        Ok(token)
    }

    /// Get session from KV by token
    pub async fn get(kv: &kv::KvStore, token: &str) -> Result<Option<Session>> {
        let key = format!("session:{}", token);
        match kv.get(&key).text().await? {
            Some(json) => {
                let session: Session = serde_json::from_str(&json)
                    .map_err(|e| Error::RustError(format!("JSON parse error: {}", e)))?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    /// Update session's last_active_at and refresh TTL
    pub async fn touch(&self, kv: &kv::KvStore, token: &str) -> Result<()> {
        let mut updated = self.clone();
        updated.last_active_at = chrono::Utc::now().to_rfc3339();

        let json = serde_json::to_string(&updated)
            .map_err(|e| Error::RustError(format!("JSON serialize error: {}", e)))?;

        kv.put(&format!("session:{}", token), json)?
            .expiration_ttl(SESSION_TTL_SECONDS)
            .execute()
            .await?;

        Ok(())
    }

    /// Store OAuth state in session
    pub async fn set_oauth_state(kv: &kv::KvStore, token: &str, state: &str) -> Result<()> {
        let key = format!("session:{}", token);
        if let Some(json) = kv.get(&key).text().await? {
            let mut session: Session = serde_json::from_str(&json)
                .map_err(|e| Error::RustError(format!("JSON parse error: {}", e)))?;

            session.github_oauth_state = Some(state.to_string());

            let json = serde_json::to_string(&session)
                .map_err(|e| Error::RustError(format!("JSON serialize error: {}", e)))?;

            kv.put(&key, json)?
                .expiration_ttl(SESSION_TTL_SECONDS)
                .execute()
                .await?;
        }
        Ok(())
    }

    /// Delete session from KV
    pub async fn delete(kv: &kv::KvStore, token: &str) -> Result<()> {
        kv.delete(&format!("session:{}", token))
            .await
            .map_err(|e| Error::RustError(format!("KV delete error: {:?}", e)))
    }

    /// Delete all sessions for a user (logout everywhere)
    pub async fn delete_all_for_user(kv: &kv::KvStore, user_id: &str) -> Result<()> {
        // KV doesn't support listing by prefix efficiently in workers
        // We'd need to track sessions in D1 for this to work properly
        // For now, this is a no-op - individual session deletion works
        let _ = (kv, user_id);
        Ok(())
    }
}

/// Generate a cryptographically secure session token
fn generate_secure_token() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 32] = rng.r#gen();
    base64::Engine::encode(&base64::engine::general_purpose::URL_SAFE_NO_PAD, bytes)
}

/// Build a session cookie string
pub fn session_cookie(token: &str, secure: bool) -> String {
    format!(
        "{}={}; HttpOnly; SameSite=Lax; Path=/; Max-Age={}{}",
        SESSION_COOKIE_NAME,
        token,
        SESSION_TTL_SECONDS,
        if secure { "; Secure" } else { "" }
    )
}

/// Build a cookie that clears the session
pub fn clear_session_cookie() -> String {
    format!(
        "{}=; HttpOnly; SameSite=Lax; Path=/; Max-Age=0",
        SESSION_COOKIE_NAME
    )
}

/// Extract session token from cookie header
pub fn extract_session_token(cookie_header: &str) -> Option<String> {
    for cookie in cookie_header.split(';') {
        let cookie = cookie.trim();
        if let Some(value) = cookie.strip_prefix(&format!("{}=", SESSION_COOKIE_NAME)) {
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}
