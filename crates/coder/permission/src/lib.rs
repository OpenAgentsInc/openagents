//! Permission system with async ask/respond for Coder.
//!
//! This module provides:
//! - Async permission requests that block until user responds
//! - Pattern-based "always allow" for similar operations
//! - Session-scoped approval tracking
//! - Wildcard pattern matching

use chrono::{DateTime, Utc};
use coder_domain::{PermissionId, SessionId};
use globset::{Glob, GlobMatcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, mpsc, oneshot};
use tracing::{debug, info, warn};

/// Error types for permission operations.
#[derive(Debug, Error)]
pub enum PermissionError {
    /// Permission was rejected by the user.
    #[error("Permission rejected: {reason}")]
    Rejected {
        session_id: SessionId,
        permission_id: PermissionId,
        reason: String,
    },

    /// Permission request timed out.
    #[error("Permission request timed out")]
    Timeout,

    /// Internal channel error.
    #[error("Internal error: {0}")]
    Internal(String),
}

/// A pending permission request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    /// Unique identifier for this request.
    pub id: PermissionId,
    /// Session this request belongs to.
    pub session_id: SessionId,
    /// Type of permission (e.g., "bash", "file_write", "file_read").
    pub permission_type: String,
    /// Human-readable title for the permission dialog.
    pub title: String,
    /// Detailed description of what will be done.
    pub description: String,
    /// Patterns that this permission covers (for "Always Allow").
    pub patterns: Vec<String>,
    /// Additional metadata for the permission decision.
    pub metadata: serde_json::Value,
    /// When this request was created.
    pub created_at: DateTime<Utc>,
}

/// User response to a permission request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Response {
    /// Allow this specific action once.
    Once,
    /// Allow this action and similar patterns for the session.
    Always,
    /// Reject this action.
    Reject,
}

/// Events emitted by the permission system.
#[derive(Debug, Clone)]
pub enum PermissionEvent {
    /// A new permission request is pending.
    RequestPending(PermissionRequest),
    /// A permission request was responded to.
    RequestResponded {
        permission_id: PermissionId,
        session_id: SessionId,
        response: Response,
    },
}

/// Internal state for a pending request.
struct PendingRequest {
    request: PermissionRequest,
    response_tx: oneshot::Sender<Response>,
}

/// Session-specific permission state.
#[derive(Default)]
struct SessionState {
    /// Patterns that have been approved with "Always".
    approved_patterns: HashMap<String, Vec<GlobMatcher>>,
    /// Pending requests for this session.
    pending: HashMap<PermissionId, PendingRequest>,
}

/// Permission audit entry for visibility and replay.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionAuditEntry {
    pub timestamp: DateTime<Utc>,
    pub session_id: SessionId,
    pub permission_id: PermissionId,
    pub permission_type: String,
    pub patterns: Vec<String>,
    pub title: String,
    pub description: String,
    pub response: Option<Response>,
}

/// The permission manager handles permission requests and responses.
pub struct PermissionManager {
    /// Session states.
    sessions: Arc<RwLock<HashMap<SessionId, SessionState>>>,
    /// Global deny rules by permission type.
    global_denies: Arc<RwLock<HashMap<String, Vec<GlobMatcher>>>>,
    /// Audit entries keyed by session for later inspection.
    audit_log: Arc<RwLock<HashMap<SessionId, Vec<PermissionAuditEntry>>>>,
    /// Channel to send permission events.
    event_tx: mpsc::UnboundedSender<PermissionEvent>,
    /// Receiver for permission events (can be cloned to allow multiple listeners).
    event_rx: Arc<RwLock<Option<mpsc::UnboundedReceiver<PermissionEvent>>>>,
}

impl PermissionManager {
    /// Create a new permission manager.
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            global_denies: Arc::new(RwLock::new(HashMap::new())),
            audit_log: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
        }
    }

    /// Take the event receiver (can only be called once).
    pub async fn take_event_receiver(&self) -> Option<mpsc::UnboundedReceiver<PermissionEvent>> {
        self.event_rx.write().await.take()
    }

    /// Request permission asynchronously.
    ///
    /// This will:
    /// 1. Check if the patterns are already approved (returns immediately if so)
    /// 2. Create a pending request and emit an event
    /// 3. Block until the user responds
    pub async fn ask(&self, request: PermissionRequest) -> Result<(), PermissionError> {
        let session_id = request.session_id;
        let permission_id = request.id;

        // Check global denies first
        if self
            .is_denied_globally(&request.permission_type, &request.patterns)
            .await
        {
            warn!(
                permission_type = %request.permission_type,
                "Permission blocked by global deny rule"
            );
            return Err(PermissionError::Rejected {
                session_id,
                permission_id,
                reason: "Action blocked by global policy".to_string(),
            });
        }

        // Check if already approved
        {
            let sessions = self.sessions.read().await;
            if let Some(session) = sessions.get(&session_id) {
                if self.is_covered(&request.permission_type, &request.patterns, session) {
                    debug!(
                        permission_type = %request.permission_type,
                        "Permission already approved by pattern"
                    );
                    return Ok(());
                }
            }
        }

        // Create oneshot channel for response
        let (response_tx, response_rx) = oneshot::channel();

        // Add to pending
        {
            let mut sessions = self.sessions.write().await;
            let session = sessions.entry(session_id).or_default();
            session.pending.insert(
                permission_id,
                PendingRequest {
                    request: request.clone(),
                    response_tx,
                },
            );
        }

        // Emit event
        info!(
            permission_id = %permission_id,
            permission_type = %request.permission_type,
            title = %request.title,
            "Permission request pending"
        );
        self.push_audit(&request, None).await;
        let _ = self.event_tx.send(PermissionEvent::RequestPending(request));

        // Wait for response
        match response_rx.await {
            Ok(Response::Once) | Ok(Response::Always) => Ok(()),
            Ok(Response::Reject) => Err(PermissionError::Rejected {
                session_id,
                permission_id,
                reason: "User rejected the permission request".to_string(),
            }),
            Err(_) => Err(PermissionError::Internal(
                "Permission request was cancelled".to_string(),
            )),
        }
    }

    /// Respond to a pending permission request.
    pub async fn respond(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: Response,
    ) -> Result<(), PermissionError> {
        self.respond_with_patterns(session_id, permission_id, response, None)
            .await
    }

    /// Respond to a pending permission request with optional override patterns for "Always".
    pub async fn respond_with_patterns(
        &self,
        session_id: SessionId,
        permission_id: PermissionId,
        response: Response,
        patterns: Option<Vec<String>>,
    ) -> Result<(), PermissionError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.get_mut(&session_id).ok_or_else(|| {
            PermissionError::Internal(format!("Session not found: {}", session_id))
        })?;

        let pending = session.pending.remove(&permission_id).ok_or_else(|| {
            PermissionError::Internal(format!("Permission request not found: {}", permission_id))
        })?;

        info!(
            permission_id = %permission_id,
            response = ?response,
            "Permission response received"
        );

        // If "Always", add patterns to approved
        if response == Response::Always {
            let patterns_to_approve = patterns.unwrap_or_else(|| pending.request.patterns.clone());
            self.approve_patterns(
                session,
                &pending.request.permission_type,
                &patterns_to_approve,
            );

            // Check other pending requests that might now be covered
            let to_auto_approve: Vec<PermissionId> = session
                .pending
                .iter()
                .filter(|(_, p)| {
                    self.is_covered(&p.request.permission_type, &p.request.patterns, session)
                })
                .map(|(id, _)| *id)
                .collect();

            // Auto-approve covered requests
            for id in to_auto_approve {
                if let Some(p) = session.pending.remove(&id) {
                    debug!(
                        permission_id = %id,
                        "Auto-approving permission covered by pattern"
                    );
                    let _ = p.response_tx.send(Response::Always);
                    let _ = self.event_tx.send(PermissionEvent::RequestResponded {
                        permission_id: id,
                        session_id,
                        response: Response::Always,
                    });
                }
            }
        }

        // Send response
        let _ = pending.response_tx.send(response);

        // Emit event
        let _ = self.event_tx.send(PermissionEvent::RequestResponded {
            permission_id,
            session_id,
            response,
        });
        self.push_audit(&pending.request, Some(response)).await;

        Ok(())
    }

    /// Get all pending requests for a session.
    pub async fn pending_requests(&self, session_id: SessionId) -> Vec<PermissionRequest> {
        let sessions = self.sessions.read().await;
        sessions
            .get(&session_id)
            .map(|s| s.pending.values().map(|p| p.request.clone()).collect())
            .unwrap_or_default()
    }

    /// Clear all pending requests for a session (e.g., on session end).
    pub async fn clear_session(&self, session_id: SessionId) {
        let mut sessions = self.sessions.write().await;
        if let Some(session) = sessions.remove(&session_id) {
            for (id, pending) in session.pending {
                warn!(
                    permission_id = %id,
                    "Rejecting pending permission due to session clear"
                );
                let _ = pending.response_tx.send(Response::Reject);
            }
        }
    }

    /// Check if patterns are covered by approved patterns.
    fn is_covered(
        &self,
        permission_type: &str,
        patterns: &[String],
        session: &SessionState,
    ) -> bool {
        let approved = match session.approved_patterns.get(permission_type) {
            Some(matchers) => matchers,
            None => return false,
        };

        if patterns.is_empty() {
            // No patterns means just check if the type is approved
            return !approved.is_empty();
        }

        // Check if all patterns are covered
        patterns
            .iter()
            .all(|pattern| approved.iter().any(|matcher| matcher.is_match(pattern)))
    }

    /// Add patterns to approved list.
    fn approve_patterns(
        &self,
        session: &mut SessionState,
        permission_type: &str,
        patterns: &[String],
    ) {
        let matchers = session
            .approved_patterns
            .entry(permission_type.to_string())
            .or_default();

        for pattern in patterns {
            // Try to compile as glob, fall back to exact match
            let glob_pattern = if pattern.contains('*') || pattern.contains('?') {
                pattern.clone()
            } else {
                // Exact match - create a pattern that matches exactly
                pattern.clone()
            };

            match Glob::new(&glob_pattern) {
                Ok(glob) => {
                    matchers.push(glob.compile_matcher());
                    debug!(pattern = %pattern, "Added approved pattern");
                }
                Err(e) => {
                    warn!(pattern = %pattern, error = %e, "Failed to compile glob pattern");
                    // Fall back to literal match by escaping special chars
                    let escaped = escape_glob_chars(pattern);
                    if let Ok(exact) = Glob::new(&escaped) {
                        matchers.push(exact.compile_matcher());
                    }
                }
            }
        }
    }

    /// Set global deny rules (replaces existing).
    pub async fn set_global_denies(&self, rules: HashMap<String, Vec<String>>) {
        let mut compiled: HashMap<String, Vec<GlobMatcher>> = HashMap::new();
        for (ptype, patterns) in rules {
            let mut matchers = Vec::new();
            for pattern in patterns {
                if let Ok(glob) = Glob::new(&pattern) {
                    matchers.push(glob.compile_matcher());
                }
            }
            compiled.insert(ptype, matchers);
        }
        *self.global_denies.write().await = compiled;
    }

    async fn is_denied_globally(&self, permission_type: &str, patterns: &[String]) -> bool {
        let denies = self.global_denies.read().await;
        let Some(matchers) = denies.get(permission_type) else {
            return false;
        };

        if patterns.is_empty() {
            return false;
        }

        patterns
            .iter()
            .any(|pattern| matchers.iter().any(|m| m.is_match(pattern)))
    }

    /// Retrieve audit entries for a session.
    pub async fn audit_entries(&self, session_id: SessionId) -> Vec<PermissionAuditEntry> {
        self.audit_log
            .read()
            .await
            .get(&session_id)
            .cloned()
            .unwrap_or_default()
    }

    async fn push_audit(&self, request: &PermissionRequest, response: Option<Response>) {
        let mut log = self.audit_log.write().await;
        let entry = PermissionAuditEntry {
            timestamp: Utc::now(),
            session_id: request.session_id,
            permission_id: request.id,
            permission_type: request.permission_type.clone(),
            patterns: request.patterns.clone(),
            title: request.title.clone(),
            description: request.description.clone(),
            response,
        };
        log.entry(request.session_id).or_default().push(entry);
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Escape glob special characters in a string.
fn escape_glob_chars(s: &str) -> String {
    let mut result = String::with_capacity(s.len() * 2);
    for c in s.chars() {
        match c {
            '*' | '?' | '[' | ']' | '{' | '}' | '\\' => {
                result.push('[');
                result.push(c);
                result.push(']');
            }
            _ => result.push(c),
        }
    }
    result
}

/// Builder for creating permission requests.
#[derive(Debug, Clone)]
pub struct PermissionRequestBuilder {
    session_id: SessionId,
    permission_type: String,
    title: String,
    description: String,
    patterns: Vec<String>,
    metadata: serde_json::Value,
}

impl PermissionRequestBuilder {
    /// Create a new builder.
    pub fn new(session_id: SessionId, permission_type: impl Into<String>) -> Self {
        Self {
            session_id,
            permission_type: permission_type.into(),
            title: String::new(),
            description: String::new(),
            patterns: Vec::new(),
            metadata: serde_json::Value::Null,
        }
    }

    /// Set the title.
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = title.into();
        self
    }

    /// Set the description.
    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = description.into();
        self
    }

    /// Add patterns for "Always Allow".
    pub fn patterns(mut self, patterns: Vec<String>) -> Self {
        self.patterns = patterns;
        self
    }

    /// Add a single pattern.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.patterns.push(pattern.into());
        self
    }

    /// Set metadata.
    pub fn metadata(mut self, metadata: serde_json::Value) -> Self {
        self.metadata = metadata;
        self
    }

    /// Build the permission request.
    pub fn build(self) -> PermissionRequest {
        PermissionRequest {
            id: PermissionId::new(),
            session_id: self.session_id,
            permission_type: self.permission_type,
            title: self.title,
            description: self.description,
            patterns: self.patterns,
            metadata: self.metadata,
            created_at: Utc::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_permission_ask_respond() {
        let manager = PermissionManager::new();
        let mut event_rx = manager.take_event_receiver().await.unwrap();

        let session_id = SessionId::new();
        let request = PermissionRequestBuilder::new(session_id, "bash")
            .title("Execute command")
            .description("Run: ls -la")
            .pattern("/bin/ls")
            .build();

        let permission_id = request.id;

        // Spawn task to respond
        let manager_clone = Arc::new(manager);
        let manager_for_respond = manager_clone.clone();
        tokio::spawn(async move {
            // Wait for the event
            if let Some(PermissionEvent::RequestPending(_)) = event_rx.recv().await {
                // Respond with Once
                manager_for_respond
                    .respond(session_id, permission_id, Response::Once)
                    .await
                    .unwrap();
            }
        });

        // Ask for permission (should complete after response)
        let result = manager_clone.ask(request).await;
        assert!(result.is_ok());

        let audit = manager_clone.audit_entries(session_id).await;
        assert_eq!(audit.len(), 2); // request + response
        assert!(audit.iter().any(|e| e.response.is_none()));
        assert!(audit.iter().any(|e| e.response == Some(Response::Once)));
    }

    #[tokio::test]
    async fn test_permission_always_covers_similar() {
        let manager = PermissionManager::new();
        let session_id = SessionId::new();

        // First request with "Always"
        let request1 = PermissionRequestBuilder::new(session_id, "file_read")
            .title("Read file")
            .pattern("/home/user/*")
            .build();

        let permission_id = request1.id;

        let manager = Arc::new(manager);
        let manager_clone = manager.clone();

        // Respond with Always in a separate task
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            manager_clone
                .respond(session_id, permission_id, Response::Always)
                .await
                .unwrap();
        });

        manager.ask(request1).await.unwrap();

        // Second request with same pattern should be auto-approved
        let request2 = PermissionRequestBuilder::new(session_id, "file_read")
            .title("Read another file")
            .pattern("/home/user/test.txt")
            .build();

        // This should return immediately (no pending)
        let result = manager.ask(request2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_permission_rejection() {
        let manager = PermissionManager::new();
        let mut event_rx = manager.take_event_receiver().await.unwrap();

        let session_id = SessionId::new();
        let request = PermissionRequestBuilder::new(session_id, "bash")
            .title("Execute dangerous command")
            .build();

        let permission_id = request.id;

        let manager = Arc::new(manager);
        let manager_clone = manager.clone();

        // Spawn task to reject
        tokio::spawn(async move {
            if let Some(PermissionEvent::RequestPending(_)) = event_rx.recv().await {
                manager_clone
                    .respond(session_id, permission_id, Response::Reject)
                    .await
                    .unwrap();
            }
        });

        let result = manager.ask(request).await;
        assert!(matches!(result, Err(PermissionError::Rejected { .. })));
    }

    #[tokio::test]
    async fn test_permission_always_with_glob_patterns() {
        let manager = PermissionManager::new();
        let session_id = SessionId::new();

        // First request for a specific command
        let request1 = PermissionRequestBuilder::new(session_id, "bash")
            .title("Run git status")
            .pattern("git status")
            .build();

        let permission_id = request1.id;
        let manager = Arc::new(manager);
        let manager_clone = manager.clone();

        // Respond with Always and a broader glob pattern
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            manager_clone
                .respond_with_patterns(
                    session_id,
                    permission_id,
                    Response::Always,
                    Some(vec!["git *".to_string()]),
                )
                .await
                .unwrap();
        });

        manager.ask(request1).await.unwrap();

        // This git command should now be auto-approved by the glob
        let request2 = PermissionRequestBuilder::new(session_id, "bash")
            .title("Run git commit")
            .pattern("git commit -m \"test\"")
            .build();

        let result = manager.ask(request2).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_global_deny_blocks_request() {
        let manager = PermissionManager::new();
        let session_id = SessionId::new();

        manager
            .set_global_denies(HashMap::from([(
                "bash".to_string(),
                vec!["rm -rf /".to_string()],
            )]))
            .await;

        let request = PermissionRequestBuilder::new(session_id, "bash")
            .title("Dangerous command")
            .pattern("rm -rf /")
            .build();

        let result = manager.ask(request).await;
        assert!(
            matches!(result, Err(PermissionError::Rejected { reason, .. }) if reason.contains("global policy"))
        );
    }
}
