use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::intent::CommandIntent;
use crate::state::AppState;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuthRequirement {
    None,
    AccessToken,
    RefreshToken,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HttpCommandRequest {
    pub method: HttpMethod,
    pub path: String,
    pub body: Option<Value>,
    pub auth: AuthRequirement,
    pub headers: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandErrorKind {
    MissingCredential,
    Unauthorized,
    Forbidden,
    Validation,
    ServiceUnavailable,
    RateLimited,
    Network,
    Decode,
    Unsupported,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandError {
    pub kind: CommandErrorKind,
    pub message: String,
    pub retryable: bool,
}

impl CommandError {
    #[must_use]
    pub fn missing_credential(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::MissingCredential,
            message: message.into(),
            retryable: false,
        }
    }

    #[must_use]
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self {
            kind: CommandErrorKind::Unsupported,
            message: message.into(),
            retryable: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CommandLatencyMetric {
    pub intent: String,
    pub latency_ms: u64,
    pub success: bool,
    pub error_kind: Option<CommandErrorKind>,
}

#[must_use]
pub fn intent_name(intent: &CommandIntent) -> &'static str {
    match intent {
        CommandIntent::Bootstrap => "bootstrap",
        CommandIntent::StartAuthChallenge { .. } => "start_auth_challenge",
        CommandIntent::VerifyAuthCode { .. } => "verify_auth_code",
        CommandIntent::RestoreSession => "restore_session",
        CommandIntent::RefreshSession => "refresh_session",
        CommandIntent::LogoutSession => "logout_session",
        CommandIntent::RequestSyncToken { .. } => "request_sync_token",
        CommandIntent::ConnectStream { .. } => "connect_stream",
        CommandIntent::DisconnectStream => "disconnect_stream",
        CommandIntent::SendThreadMessage { .. } => "send_thread_message",
        CommandIntent::Navigate { .. } => "navigate",
    }
}

#[must_use]
pub fn command_latency_metric(
    intent: &CommandIntent,
    latency_ms: u64,
    outcome: Result<(), &CommandError>,
) -> CommandLatencyMetric {
    match outcome {
        Ok(()) => CommandLatencyMetric {
            intent: intent_name(intent).to_string(),
            latency_ms,
            success: true,
            error_kind: None,
        },
        Err(error) => CommandLatencyMetric {
            intent: intent_name(intent).to_string(),
            latency_ms,
            success: false,
            error_kind: Some(error.kind.clone()),
        },
    }
}

pub fn map_intent_to_http(
    intent: &CommandIntent,
    state: &AppState,
) -> Result<HttpCommandRequest, CommandError> {
    match intent {
        CommandIntent::StartAuthChallenge { email } => Ok(HttpCommandRequest {
            method: HttpMethod::Post,
            path: "/api/auth/email".to_string(),
            body: Some(json!({ "email": email })),
            auth: AuthRequirement::None,
            headers: Vec::new(),
        }),
        CommandIntent::VerifyAuthCode { code } => {
            let mut payload = json!({ "code": code });
            if let Some(challenge_id) = state.auth.challenge_id.as_ref() {
                payload["challenge_id"] = Value::String(challenge_id.clone());
            }

            Ok(HttpCommandRequest {
                method: HttpMethod::Post,
                path: "/api/auth/verify".to_string(),
                body: Some(payload),
                auth: AuthRequirement::None,
                headers: vec![("x-client".to_string(), "openagents-web-shell".to_string())],
            })
        }
        CommandIntent::RestoreSession => {
            if state.auth.access_token.is_none() {
                return Err(CommandError::missing_credential(
                    "Access token is required to restore session.",
                ));
            }
            Ok(HttpCommandRequest {
                method: HttpMethod::Get,
                path: "/api/auth/session".to_string(),
                body: None,
                auth: AuthRequirement::AccessToken,
                headers: Vec::new(),
            })
        }
        CommandIntent::RefreshSession => {
            let Some(refresh_token) = state.auth.refresh_token.as_ref() else {
                return Err(CommandError::missing_credential(
                    "Refresh token is required to refresh session.",
                ));
            };
            Ok(HttpCommandRequest {
                method: HttpMethod::Post,
                path: "/api/auth/refresh".to_string(),
                body: Some(json!({
                    "refresh_token": refresh_token,
                    "rotate_refresh_token": true,
                })),
                auth: AuthRequirement::None,
                headers: Vec::new(),
            })
        }
        CommandIntent::LogoutSession => {
            if state.auth.access_token.is_none() {
                return Err(CommandError::missing_credential(
                    "Access token is required to logout.",
                ));
            }
            Ok(HttpCommandRequest {
                method: HttpMethod::Post,
                path: "/api/auth/logout".to_string(),
                body: None,
                auth: AuthRequirement::AccessToken,
                headers: Vec::new(),
            })
        }
        CommandIntent::RequestSyncToken { scopes } => {
            if state.auth.access_token.is_none() {
                return Err(CommandError::missing_credential(
                    "Access token is required to request sync token.",
                ));
            }
            Ok(HttpCommandRequest {
                method: HttpMethod::Post,
                path: "/api/sync/token".to_string(),
                body: Some(json!({
                    "scopes": scopes,
                })),
                auth: AuthRequirement::AccessToken,
                headers: Vec::new(),
            })
        }
        CommandIntent::SendThreadMessage { thread_id, text } => {
            if state.auth.access_token.is_none() {
                return Err(CommandError::missing_credential(
                    "Access token is required to send thread message.",
                ));
            }
            Ok(HttpCommandRequest {
                method: HttpMethod::Post,
                path: format!("/api/runtime/threads/{thread_id}/messages"),
                body: Some(json!({
                    "text": text,
                })),
                auth: AuthRequirement::AccessToken,
                headers: Vec::new(),
            })
        }
        CommandIntent::Bootstrap
        | CommandIntent::ConnectStream { .. }
        | CommandIntent::DisconnectStream
        | CommandIntent::Navigate { .. } => Err(CommandError::unsupported(format!(
            "Intent '{}' does not map to HTTP command adapter.",
            intent_name(intent)
        ))),
    }
}

#[must_use]
pub fn classify_http_error(
    status: u16,
    code: Option<&str>,
    message: impl Into<String>,
) -> CommandError {
    let code_normalized = code.map(|value| value.trim().to_lowercase());
    let kind = if status == 0 {
        CommandErrorKind::Network
    } else if status == 401 {
        CommandErrorKind::Unauthorized
    } else if status == 403 {
        CommandErrorKind::Forbidden
    } else if status == 422 {
        CommandErrorKind::Validation
    } else if status == 429 {
        CommandErrorKind::RateLimited
    } else if status == 503 {
        CommandErrorKind::ServiceUnavailable
    } else if (500..=599).contains(&status) {
        CommandErrorKind::ServiceUnavailable
    } else if code_normalized.as_deref() == Some("decode_failed") {
        CommandErrorKind::Decode
    } else {
        CommandErrorKind::Unknown
    };

    let retryable = matches!(
        kind,
        CommandErrorKind::Network
            | CommandErrorKind::RateLimited
            | CommandErrorKind::ServiceUnavailable
    );

    CommandError {
        kind,
        message: message.into(),
        retryable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppAction, AppState, CommandIntent, apply_action};

    #[test]
    fn map_start_auth_challenge_to_http_request() {
        let state = AppState::default();
        let request = map_intent_to_http(
            &CommandIntent::StartAuthChallenge {
                email: "dev@example.com".to_string(),
            },
            &state,
        )
        .expect("intent should map");

        assert_eq!(request.path, "/api/auth/email");
        assert_eq!(request.method, HttpMethod::Post);
        assert_eq!(request.auth, AuthRequirement::None);
    }

    #[test]
    fn map_restore_session_requires_access_token() {
        let state = AppState::default();
        let error = map_intent_to_http(&CommandIntent::RestoreSession, &state)
            .expect_err("missing token should fail");
        assert_eq!(error.kind, CommandErrorKind::MissingCredential);
    }

    #[test]
    fn map_verify_code_includes_challenge_id_when_present() {
        let mut state = AppState::default();
        let _ = apply_action(
            &mut state,
            AppAction::AuthChallengeAccepted {
                email: "dev@example.com".to_string(),
                challenge_id: "challenge_123".to_string(),
            },
        );

        let request = map_intent_to_http(
            &CommandIntent::VerifyAuthCode {
                code: "123456".to_string(),
            },
            &state,
        )
        .expect("verify intent should map");

        let body = request.body.expect("body should exist");
        assert_eq!(
            body["challenge_id"],
            Value::String("challenge_123".to_string())
        );
    }

    #[test]
    fn classify_http_error_produces_retry_policy() {
        let network = classify_http_error(0, Some("network_error"), "network down");
        assert_eq!(network.kind, CommandErrorKind::Network);
        assert!(network.retryable);

        let validation = classify_http_error(422, Some("invalid_request"), "bad email");
        assert_eq!(validation.kind, CommandErrorKind::Validation);
        assert!(!validation.retryable);

        let unavailable = classify_http_error(503, Some("service_unavailable"), "unavailable");
        assert_eq!(unavailable.kind, CommandErrorKind::ServiceUnavailable);
        assert!(unavailable.retryable);
    }
}
