use super::*;

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
        .map_err(map_auth_error)?;

    let is_pat_session = session.session.session_id.starts_with("pat:");

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
            requested_topics: payload.topics,
            requested_ttl_seconds: payload.ttl_seconds,
        })
        .map_err(map_sync_error)?;

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
        .map_err(map_auth_error)?;

    if !decision.allowed {
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
