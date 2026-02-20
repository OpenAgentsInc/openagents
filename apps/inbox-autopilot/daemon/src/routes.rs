use crate::auth::AuthManager;
use crate::config::Config;
use crate::db::Database;
use crate::error::ApiError;
use crate::gmail::GmailClient;
use crate::pipeline::DraftPipeline;
use crate::types::{
    ApproveSendResponse, AuthStatusResponse, BackfillRequest, ChatGptAuthRequest,
    DraftListResponse, EventListResponse, ExportAuditResponse, GmailAuthRequest,
    GmailAuthUrlResponse, HealthResponse, SessionCreateRequest, SettingsResponse, SyncNowResponse,
    TemplateMineResponse, ThreadListResponse, UpdateSettingsRequest,
};
use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use futures_util::StreamExt;
use futures_util::stream::Stream;
use serde::Deserialize;
use serde_json::json;
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub config: Config,
    pub db: Database,
    pub auth: Arc<AuthManager>,
    pub gmail: GmailClient,
    pub pipeline: DraftPipeline,
    pub event_tx: broadcast::Sender<crate::types::EventRecord>,
}

impl AppState {
    pub fn emit_event(
        &self,
        thread_id: Option<&str>,
        event_type: &str,
        payload: serde_json::Value,
    ) -> Result<crate::types::EventRecord, ApiError> {
        let event = self
            .db
            .append_event(thread_id, event_type, &payload)
            .map_err(ApiError::internal)?;
        let _ = self.event_tx.send(event.clone());
        Ok(event)
    }

    pub async fn generate_drafts_for_recent_threads(&self) -> Result<(), ApiError> {
        let threads = self.db.list_threads(None, 60).map_err(ApiError::internal)?;

        for thread in threads {
            let has_draft = self
                .db
                .draft_by_thread(&thread.id)
                .map_err(ApiError::internal)?
                .is_some();
            if has_draft {
                continue;
            }

            if let Ok(generated) = self.pipeline.run_for_thread(&self.db, &thread.id).await {
                self.emit_event(
                    Some(&thread.id),
                    "classification_completed",
                    json!({
                        "category": generated.category,
                        "risk": generated.risk,
                        "policy": generated.policy,
                    }),
                )?;
                self.emit_event(
                    Some(&thread.id),
                    "draft_created",
                    json!({
                        "draft_id": generated.draft.id,
                        "model_used": generated.draft.model_used,
                    }),
                )?;
            }
        }

        Ok(())
    }
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/session", post(create_session))
        .route("/auth/gmail/url", get(gmail_auth_url))
        .route("/auth/gmail", post(gmail_auth_exchange))
        .route("/auth/gmail/status", get(gmail_auth_status))
        .route("/auth/chatgpt", post(chatgpt_auth))
        .route("/auth/chatgpt/status", get(chatgpt_auth_status))
        .route("/sync/backfill", post(sync_backfill))
        .route("/sync/now", post(sync_now))
        .route("/threads", get(list_threads))
        .route("/threads/:id", get(get_thread))
        .route(
            "/threads/:id/generate-draft",
            post(generate_draft_for_thread),
        )
        .route("/threads/:id/draft", get(get_thread_draft))
        .route("/threads/:id/approve-send", post(approve_and_send))
        .route("/threads/:id/audit", get(thread_audit))
        .route("/threads/:id/events", get(thread_events))
        .route("/threads/:id/export-audit", post(export_audit))
        .route("/drafts", get(list_drafts))
        .route("/drafts/:id/needs-human", post(mark_needs_human))
        .route("/templates/mine", get(mine_templates))
        .route("/events", get(list_events))
        .route("/events/stream", get(stream_events))
        .route("/settings", get(get_settings).put(update_settings))
        .route("/settings/delete-corpus", post(delete_corpus))
        .route("/settings/factory-reset", post(factory_reset))
        .with_state(Arc::new(state))
}

async fn health(State(state): State<Arc<AppState>>) -> Result<Json<HealthResponse>, ApiError> {
    let (gmail_connected, _) = state.db.oauth_status("gmail").map_err(ApiError::internal)?;
    let (chatgpt_connected, _) = state
        .db
        .oauth_status("chatgpt")
        .map_err(ApiError::internal)?;
    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        connected_gmail: gmail_connected,
        connected_chatgpt: chatgpt_connected,
    }))
}

async fn create_session(
    State(state): State<Arc<AppState>>,
    Json(request): Json<SessionCreateRequest>,
) -> Result<Json<crate::types::SessionCreateResponse>, ApiError> {
    let session = state.auth.create_session(request.client_name);
    Ok(Json(session))
}

#[derive(Debug, Deserialize)]
struct GmailUrlQuery {
    redirect_uri: String,
    code_challenge: Option<String>,
}

async fn gmail_auth_url(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<GmailUrlQuery>,
) -> Result<Json<GmailAuthUrlResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let oauth_state = Uuid::new_v4().to_string();
    let url = state.gmail.auth_url(
        &query.redirect_uri,
        &oauth_state,
        query.code_challenge.as_deref(),
    )?;
    Ok(Json(GmailAuthUrlResponse {
        url,
        state: oauth_state,
    }))
}

async fn gmail_auth_exchange(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<GmailAuthRequest>,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    state.gmail.exchange_code(&state.db, request).await?;

    state.emit_event(None, "auth_gmail_connected", json!({ "provider": "gmail" }))?;
    Ok((axum::http::StatusCode::NO_CONTENT, "").into_response())
}

async fn gmail_auth_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AuthStatusResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let (connected, updated_at) = state.db.oauth_status("gmail").map_err(ApiError::internal)?;
    Ok(Json(AuthStatusResponse {
        connected,
        updated_at,
    }))
}

async fn chatgpt_auth(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<ChatGptAuthRequest>,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    let key = request.api_key.trim();
    if key.is_empty() {
        return Err(ApiError::BadRequest(
            "chatgpt api key cannot be empty".to_string(),
        ));
    }

    state
        .db
        .store_provider_token("chatgpt", Some(key), None, None, None, None)
        .map_err(ApiError::internal)?;

    state.emit_event(
        None,
        "auth_chatgpt_connected",
        json!({ "provider": "chatgpt" }),
    )?;

    Ok((axum::http::StatusCode::NO_CONTENT, "").into_response())
}

async fn chatgpt_auth_status(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<AuthStatusResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let (connected, updated_at) = state
        .db
        .oauth_status("chatgpt")
        .map_err(ApiError::internal)?;
    Ok(Json(AuthStatusResponse {
        connected,
        updated_at,
    }))
}

async fn sync_backfill(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<BackfillRequest>,
) -> Result<Json<crate::types::BackfillResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let settings = state.db.settings().map_err(ApiError::internal)?;
    let days = request
        .days
        .unwrap_or(settings.backfill_days)
        .clamp(1, 3650);

    let backfill = state.gmail.backfill(&state.db, days).await?;

    state.emit_event(
        None,
        "sync_backfill_completed",
        json!({
            "days": days,
            "imported_threads": backfill.imported_threads,
            "imported_messages": backfill.imported_messages
        }),
    )?;

    state.generate_drafts_for_recent_threads().await?;

    Ok(Json(backfill))
}

async fn sync_now(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<SyncNowResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let settings = state.db.settings().map_err(ApiError::internal)?;
    let backfill = state
        .gmail
        .backfill(&state.db, settings.backfill_days)
        .await?;

    state.emit_event(
        None,
        "sync_incremental_completed",
        json!({
            "imported_threads": backfill.imported_threads,
            "imported_messages": backfill.imported_messages
        }),
    )?;

    state.generate_drafts_for_recent_threads().await?;

    Ok(Json(SyncNowResponse {
        imported_threads: backfill.imported_threads,
        imported_messages: backfill.imported_messages,
    }))
}

#[derive(Debug, Deserialize)]
struct ThreadListQuery {
    search: Option<String>,
    limit: Option<usize>,
}

async fn list_threads(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<ThreadListQuery>,
) -> Result<Json<ThreadListResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let threads = state
        .db
        .list_threads(query.search, query.limit.unwrap_or(100))
        .map_err(ApiError::internal)?;

    Ok(Json(ThreadListResponse { threads }))
}

async fn get_thread(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::types::ThreadDetailResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let detail = state
        .db
        .get_thread_detail(&id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::NotFound(format!("thread {id} not found")))?;
    Ok(Json(detail))
}

async fn generate_draft_for_thread(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::types::GenerateDraftResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let generated = state.pipeline.run_for_thread(&state.db, &id).await?;

    state.emit_event(
        Some(&id),
        "classification_completed",
        json!({
            "category": generated.category,
            "risk": generated.risk,
            "policy": generated.policy,
        }),
    )?;

    state.emit_event(
        Some(&id),
        "policy_evaluated",
        json!({
            "decision": generated.policy,
        }),
    )?;

    state.emit_event(
        Some(&id),
        "draft_created",
        json!({
            "draft_id": generated.draft.id,
            "model_used": generated.draft.model_used,
            "status": generated.draft.status,
        }),
    )?;

    Ok(Json(generated))
}

async fn get_thread_draft(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Option<crate::types::DraftRecord>>, ApiError> {
    require_auth(&state, &headers)?;
    let draft = state.db.draft_by_thread(&id).map_err(ApiError::internal)?;
    Ok(Json(draft))
}

async fn approve_and_send(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ApproveSendResponse>, ApiError> {
    require_auth(&state, &headers)?;

    let thread = state
        .db
        .get_thread(&id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::NotFound(format!("thread {id} not found")))?;

    match thread.policy {
        Some(crate::types::PolicyDecision::SendWithApproval) => {}
        Some(crate::types::PolicyDecision::Blocked) => {
            return Err(ApiError::Conflict(
                "policy blocks one-click send for this thread".to_string(),
            ));
        }
        Some(crate::types::PolicyDecision::DraftOnly) => {
            return Err(ApiError::Conflict(
                "policy requires draft-only handling; send is disabled".to_string(),
            ));
        }
        None => {
            return Err(ApiError::Conflict(
                "policy missing for thread; generate a draft before sending".to_string(),
            ));
        }
    }

    let draft = state
        .db
        .get_pending_draft_for_thread(&id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::BadRequest("no pending draft found".to_string()))?;

    let (subject, sender, _recipient) = state
        .db
        .latest_thread_subject_and_recipient(&id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::BadRequest("thread has no message metadata".to_string()))?;

    let settings = state.db.settings().map_err(ApiError::internal)?;
    if !settings.allowed_recipient_domains.is_empty() {
        let sender_domain = sender
            .split('@')
            .nth(1)
            .map(|v| v.to_lowercase())
            .ok_or_else(|| ApiError::BadRequest("sender email missing domain".to_string()))?;
        if !settings
            .allowed_recipient_domains
            .iter()
            .any(|domain| domain.to_lowercase() == sender_domain)
        {
            return Err(ApiError::Conflict(
                "recipient domain is blocked by allowlist policy".to_string(),
            ));
        }
    }

    state.emit_event(
        Some(&id),
        "approval_granted",
        json!({ "draft_id": draft.id }),
    )?;

    let gmail_message_id = state
        .gmail
        .send_reply(&state.db, &id, &sender, &subject, &draft.body)
        .await?;

    state
        .db
        .set_draft_as_sent(&draft.id, &gmail_message_id)
        .map_err(ApiError::internal)?;

    state.emit_event(
        Some(&id),
        "email_sent",
        json!({
            "draft_id": draft.id,
            "gmail_message_id": gmail_message_id,
        }),
    )?;

    Ok(Json(ApproveSendResponse {
        draft_id: draft.id,
        gmail_message_id,
    }))
}

async fn thread_audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<crate::types::AuditResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let audit = state
        .db
        .audit_for_thread(&id)
        .map_err(ApiError::internal)?
        .ok_or_else(|| ApiError::NotFound(format!("thread {id} not found")))?;
    Ok(Json(audit))
}

async fn thread_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<EventListResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let events = state
        .db
        .events(Some(&id), 500)
        .map_err(ApiError::internal)?;
    Ok(Json(EventListResponse { events }))
}

async fn export_audit(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ExportAuditResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let mut export_dir = state.config.data_dir.clone();
    export_dir.push("exports");

    let (path, count) = state
        .db
        .export_thread_events(&id, export_dir)
        .map_err(ApiError::internal)?;

    Ok(Json(ExportAuditResponse {
        path: path.display().to_string(),
        exported_events: count,
    }))
}

#[derive(Debug, Deserialize)]
struct DraftListQuery {
    status: Option<String>,
    limit: Option<usize>,
}

async fn list_drafts(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<DraftListQuery>,
) -> Result<Json<DraftListResponse>, ApiError> {
    require_auth(&state, &headers)?;

    let drafts = match query.status.as_deref() {
        Some("pending") | None => state
            .db
            .pending_drafts(query.limit.unwrap_or(200))
            .map_err(ApiError::internal)?,
        Some(other) => {
            return Err(ApiError::BadRequest(format!(
                "unsupported status filter: {other}"
            )));
        }
    };

    Ok(Json(DraftListResponse { drafts }))
}

async fn mark_needs_human(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    state
        .db
        .mark_draft_needs_human(&id)
        .map_err(ApiError::internal)?;
    state.emit_event(None, "draft_marked_needs_human", json!({ "draft_id": id }))?;
    Ok((axum::http::StatusCode::NO_CONTENT, "").into_response())
}

#[derive(Debug, Deserialize)]
struct TemplateMineQuery {
    limit: Option<usize>,
}

async fn mine_templates(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<TemplateMineQuery>,
) -> Result<Json<TemplateMineResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let suggestions = state
        .db
        .mine_template_suggestions(query.limit.unwrap_or(12))
        .map_err(ApiError::internal)?;
    Ok(Json(TemplateMineResponse { suggestions }))
}

#[derive(Debug, Deserialize)]
struct EventQuery {
    thread_id: Option<String>,
    limit: Option<usize>,
}

async fn list_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Query(query): Query<EventQuery>,
) -> Result<Json<EventListResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let events = state
        .db
        .events(query.thread_id.as_deref(), query.limit.unwrap_or(300))
        .map_err(ApiError::internal)?;
    Ok(Json(EventListResponse { events }))
}

async fn stream_events(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, ApiError> {
    require_auth(&state, &headers)?;
    let rx = state.event_tx.subscribe();

    let stream = BroadcastStream::new(rx).filter_map(|result| async move {
        match result {
            Ok(event) => {
                let payload = serde_json::to_string(&event).unwrap_or_else(|_| "{}".to_string());
                Some(Ok(Event::default()
                    .event(event.event_type)
                    .id(event.id)
                    .data(payload)))
            }
            Err(_) => None,
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::new().interval(std::time::Duration::from_secs(15))))
}

async fn get_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let settings = state.db.settings().map_err(ApiError::internal)?;
    Ok(Json(settings))
}

async fn update_settings(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<UpdateSettingsRequest>,
) -> Result<Json<SettingsResponse>, ApiError> {
    require_auth(&state, &headers)?;
    let existing = state.db.settings().map_err(ApiError::internal)?;

    let settings = SettingsResponse {
        privacy_mode: request.privacy_mode,
        backfill_days: request.backfill_days,
        allowed_recipient_domains: request.allowed_recipient_domains,
        attachment_storage_mode: request.attachment_storage_mode,
        signature: request.signature,
        template_scheduling: request.template_scheduling,
        template_report_delivery: request.template_report_delivery,
        sync_interval_seconds: request
            .sync_interval_seconds
            .unwrap_or(existing.sync_interval_seconds),
    };

    state
        .db
        .update_settings(&settings)
        .map_err(ApiError::internal)?;

    state.emit_event(
        None,
        "settings_updated",
        json!({
            "privacy_mode": settings.privacy_mode,
            "backfill_days": settings.backfill_days,
            "allowed_recipient_domains": settings.allowed_recipient_domains,
            "attachment_storage_mode": settings.attachment_storage_mode,
            "sync_interval_seconds": settings.sync_interval_seconds,
        }),
    )?;

    Ok(Json(settings))
}

async fn delete_corpus(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    state.db.delete_local_corpus().map_err(ApiError::internal)?;
    state.emit_event(None, "local_corpus_deleted", json!({}))?;
    Ok((axum::http::StatusCode::NO_CONTENT, "").into_response())
}

async fn factory_reset(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    require_auth(&state, &headers)?;
    state.db.factory_reset().map_err(ApiError::internal)?;
    state.emit_event(None, "factory_reset_completed", json!({}))?;
    Ok((axum::http::StatusCode::NO_CONTENT, "").into_response())
}

fn require_auth(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let token = headers
        .get("x-session-token")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing x-session-token header".to_string()))?;
    let nonce = headers
        .get("x-nonce")
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| ApiError::Unauthorized("missing x-nonce header".to_string()))?;

    state.auth.verify(token, nonce)?;
    Ok(())
}
