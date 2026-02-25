use super::*;
use std::collections::BTreeSet;

pub(super) async fn sync_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(payload): Json<SyncTokenRequestPayload>,
) -> Result<impl IntoResponse, (StatusCode, Json<ApiErrorResponse>)> {
    let request_id = request_id(&headers);
    let access_token = access_token_from_headers(&headers)
        .ok_or_else(|| unauthorized_error("Unauthenticated."))?;

    let session = state
        .auth
        .session_or_pat_from_access_token(&access_token)
        .await
        .map_err(|error| {
            emit_auth_failure_event(&state, &request_id, "sync.token.issue.auth_failed", &error);
            map_auth_error(error)
        })?;

    let is_pat_session = session.session.session_id.starts_with("pat:");
    let requested_streams = merge_stream_requests(payload.streams, payload.topics);

    let device_id = payload
        .device_id
        .and_then(non_empty)
        .unwrap_or_else(|| session.session.device_id.clone());

    if !is_pat_session && device_id != session.session.device_id {
        return Err(forbidden_error(
            "Requested device does not match active authenticated session device.",
        ));
    }

    let issued = state
        .sync_token_issuer
        .issue(SyncTokenIssueRequest {
            user_id: session.user.id.clone(),
            org_id: session.session.active_org_id.clone(),
            session_id: session.session.session_id.clone(),
            device_id,
            requested_scopes: payload.scopes,
            requested_streams,
            requested_topics: Vec::new(),
            requested_ttl_seconds: payload.ttl_seconds,
        })
        .map_err(|error| {
            state.observability.audit(
                AuditEvent::new("sync.token.issue.failed", request_id.clone())
                    .with_outcome("failure")
                    .with_attribute("reason", error.to_string()),
            );
            state
                .observability
                .increment_counter("sync.token.issue.failed", &request_id);
            map_sync_error(error)
        })?;

    let decision = state
        .auth
        .evaluate_policy_by_access_token(
            &access_token,
            PolicyCheckRequest {
                org_id: Some(issued.org_id.clone()),
                required_scopes: issued.scopes.clone(),
                requested_topics: issued
                    .granted_topics
                    .iter()
                    .map(|grant| grant.topic.clone())
                    .collect(),
            },
        )
        .await
        .map_err(|error| {
            emit_auth_failure_event(
                &state,
                &request_id,
                "sync.token.issue.policy_eval_failed",
                &error,
            );
            map_auth_error(error)
        })?;

    if !decision.allowed {
        state.observability.audit(
            AuditEvent::new("sync.token.issue.policy_denied", request_id.clone())
                .with_outcome("failure")
                .with_user_id(session.user.id.clone())
                .with_session_id(session.session.session_id.clone())
                .with_org_id(session.session.active_org_id.clone())
                .with_device_id(session.session.device_id.clone())
                .with_attribute("scope_count", issued.scopes.len().to_string())
                .with_attribute("topic_count", issued.granted_topics.len().to_string()),
        );
        state
            .observability
            .increment_counter("sync.token.issue.policy_denied", &request_id);
        return Err(forbidden_error(
            "Requested sync scopes/topics are not allowed for current org policy.",
        ));
    }

    state.observability.audit(
        AuditEvent::new("sync.token.issued", request_id.clone())
            .with_user_id(session.user.id.clone())
            .with_session_id(session.session.session_id.clone())
            .with_org_id(session.session.active_org_id.clone())
            .with_device_id(session.session.device_id.clone())
            .with_attribute("scope_count", issued.scopes.len().to_string())
            .with_attribute("topic_count", issued.granted_topics.len().to_string())
            .with_attribute("expires_in", issued.expires_in.to_string()),
    );
    state
        .observability
        .increment_counter("sync.token.issued", &request_id);

    Ok(ok_data(issued))
}

fn merge_stream_requests(streams: Vec<String>, topics: Vec<String>) -> Vec<String> {
    let mut merged = BTreeSet::new();
    for value in streams.into_iter().chain(topics) {
        let normalized = value.trim().to_string();
        if !normalized.is_empty() {
            merged.insert(normalized);
        }
    }
    merged.into_iter().collect()
}
