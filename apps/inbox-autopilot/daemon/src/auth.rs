use crate::error::ApiError;
use crate::types::SessionCreateResponse;
use chrono::{Duration, Utc};
use parking_lot::Mutex;
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
struct SessionState {
    expires_at_ts: i64,
    nonces: HashMap<String, i64>,
    #[allow(dead_code)]
    client_name: Option<String>,
}

#[derive(Debug)]
pub struct AuthManager {
    session_ttl_seconds: i64,
    sessions: Mutex<HashMap<String, SessionState>>,
}

impl AuthManager {
    pub fn new(session_ttl_seconds: i64) -> Self {
        Self {
            session_ttl_seconds,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn create_session(&self, client_name: Option<String>) -> SessionCreateResponse {
        let token = Uuid::new_v4().to_string();
        let expires_at = Utc::now() + Duration::seconds(self.session_ttl_seconds);
        let state = SessionState {
            expires_at_ts: expires_at.timestamp(),
            nonces: HashMap::new(),
            client_name,
        };

        let mut sessions = self.sessions.lock();
        sessions.insert(token.clone(), state);

        SessionCreateResponse {
            session_token: token,
            expires_at,
        }
    }

    pub fn verify(&self, token: &str, nonce: &str) -> Result<(), ApiError> {
        if token.trim().is_empty() || nonce.trim().is_empty() {
            return Err(ApiError::Unauthorized(
                "missing session token or nonce".to_string(),
            ));
        }

        let now_ts = Utc::now().timestamp();
        let mut sessions = self.sessions.lock();
        sessions.retain(|_, session| session.expires_at_ts > now_ts);

        let session = sessions.get_mut(token).ok_or_else(|| {
            ApiError::Unauthorized("invalid or expired session token".to_string())
        })?;

        let nonce_cutoff = now_ts - 600;
        session
            .nonces
            .retain(|_, created_at| *created_at >= nonce_cutoff);

        if session.nonces.contains_key(nonce) {
            return Err(ApiError::Unauthorized(
                "nonce has already been used".to_string(),
            ));
        }

        session.nonces.insert(nonce.to_string(), now_ts);

        Ok(())
    }
}
