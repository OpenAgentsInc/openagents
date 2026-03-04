use std::collections::BTreeSet;
use std::time::Duration;

use base64::Engine;
use openagents_email_agent::{
    GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailBackfillResult,
    GmailConnectorError, GmailDeltaItem, GmailDeltaOperation, GmailHistoryProvider,
    GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader, GmailMessageMetadata,
    GmailMessagePayload, GmailSendProvider, GmailSendSuccess, GmailSyncBatch, GmailSyncError,
    GmailSyncOutcome, GmailSyncState, SendExecutionOutcome, SendExecutionPolicy,
    SendExecutionState, SendFailureClass, SendProviderError, SendRequest,
    apply_gmail_incremental_sync, execute_send_with_idempotency, run_gmail_backfill,
};
use reqwest::StatusCode;
use reqwest::blocking::Client;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use crate::app_state::RenderState;
use crate::credentials::{
    GOOGLE_GMAIL_ACCESS_TOKEN, GOOGLE_GMAIL_TOKEN_EXPIRY_UNIX, GoogleGmailOAuthLifecycle,
};

const GMAIL_API_ROOT: &str = "https://gmail.googleapis.com/gmail/v1/users/me";
const GOOGLE_OAUTH_TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GMAIL_QUERY_INBOX: &str = "in:inbox";
const GMAIL_SYNC_REBOOTSTRAP_REASON: &str = "gmail sync cursor stale; rebootstrap required";
const OAUTH_REFRESH_SKEW_SECONDS: u64 = 120;
const HTTP_TIMEOUT_SECONDS: u64 = 20;
const USER_AGENT: &str = "openagents-autopilot-desktop/0.1";

pub(super) fn fetch_live_gmail_backfill(
    state: &mut RenderState,
    checkpoint: Option<&GmailBackfillCheckpoint>,
    config: &GmailBackfillConfig,
) -> Result<GmailBackfillResult, String> {
    let provider = LiveGmailMailboxProvider::new(gmail_session(state)?);
    run_gmail_backfill(&provider, checkpoint, config).map_err(|error| error.to_string())
}

pub(super) fn run_live_gmail_incremental_sync(
    state: &mut RenderState,
    sync_state: &mut GmailSyncState,
    max_results: usize,
) -> Result<GmailSyncOutcome, String> {
    let provider = LiveGmailHistoryProvider::new(gmail_session(state)?);
    apply_gmail_incremental_sync(sync_state, &provider, max_results).map_err(|error| {
        if matches!(error, GmailSyncError::Provider(_)) {
            return format!("gmail incremental sync failed: {error}");
        }
        error.to_string()
    })
}

pub(super) fn execute_live_gmail_send(
    state: &mut RenderState,
    send_state: &mut SendExecutionState,
    request: &SendRequest,
    policy: &SendExecutionPolicy,
    now_unix: u64,
) -> Result<SendExecutionOutcome, String> {
    let provider = LiveGmailSendProvider::new(gmail_session(state)?);
    execute_send_with_idempotency(send_state, &provider, request, policy, now_unix)
        .map_err(|error| error.to_string())
}

#[derive(Clone)]
struct GmailSession {
    client: Client,
    access_token: String,
}

impl GmailSession {
    fn auth_get_json<T: DeserializeOwned>(
        &self,
        endpoint: &str,
        query: &[(&str, String)],
    ) -> Result<T, String> {
        let url = format!("{GMAIL_API_ROOT}/{endpoint}");
        let response = self
            .client
            .get(url)
            .bearer_auth(&self.access_token)
            .query(query)
            .send()
            .map_err(|error| format!("gmail GET {endpoint} failed: {error}"))?;

        parse_json_response(response, format!("gmail GET {endpoint}"))
    }

    fn auth_post_json<B: Serialize, T: DeserializeOwned>(
        &self,
        endpoint: &str,
        body: &B,
    ) -> Result<T, SendProviderError> {
        let url = format!("{GMAIL_API_ROOT}/{endpoint}");
        let response = self
            .client
            .post(url)
            .bearer_auth(&self.access_token)
            .json(body)
            .send()
            .map_err(|error| SendProviderError {
                class: SendFailureClass::Transient,
                reason: format!("gmail POST {endpoint} failed: {error}"),
            })?;
        parse_send_json_response(response, format!("gmail POST {endpoint}"))
    }
}

fn gmail_session(state: &mut RenderState) -> Result<GmailSession, String> {
    let access_token = gmail_access_token(state)?;
    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("gmail http client init failed: {error}"))?;
    Ok(GmailSession {
        client,
        access_token,
    })
}

fn gmail_access_token(state: &mut RenderState) -> Result<String, String> {
    let mut lifecycle = state
        .credentials
        .load_google_gmail_oauth_lifecycle()?
        .ok_or_else(|| "Missing Gmail OAuth credentials in Credentials pane".to_string())?;

    let now_unix = now_epoch_seconds();
    if lifecycle.should_refresh_at(now_unix, OAUTH_REFRESH_SKEW_SECONDS) {
        let refreshed = refresh_google_access_token(&lifecycle)?;
        lifecycle.access_token = refreshed.access_token.clone();
        lifecycle.expires_at_unix = refreshed.expires_at_unix;
        state.credentials.set_value_for_name(
            GOOGLE_GMAIL_ACCESS_TOKEN,
            lifecycle.access_token.as_str(),
        )?;
        state.credentials.set_value_for_name(
            GOOGLE_GMAIL_TOKEN_EXPIRY_UNIX,
            lifecycle.expires_at_unix.to_string().as_str(),
        )?;
    }

    Ok(lifecycle.access_token)
}

struct RefreshedGoogleToken {
    access_token: String,
    expires_at_unix: u64,
}

fn refresh_google_access_token(
    lifecycle: &GoogleGmailOAuthLifecycle,
) -> Result<RefreshedGoogleToken, String> {
    #[derive(Deserialize)]
    struct OAuthRefreshSuccess {
        access_token: String,
        expires_in: u64,
    }

    #[derive(Deserialize)]
    struct OAuthRefreshError {
        error: Option<String>,
        error_description: Option<String>,
    }

    let client = Client::builder()
        .timeout(Duration::from_secs(HTTP_TIMEOUT_SECONDS))
        .user_agent(USER_AGENT)
        .build()
        .map_err(|error| format!("oauth refresh client init failed: {error}"))?;

    let form = [
        ("grant_type", "refresh_token"),
        ("refresh_token", lifecycle.refresh_token.as_str()),
        ("client_id", lifecycle.client_id.as_str()),
        ("client_secret", lifecycle.client_secret.as_str()),
        ("redirect_uri", lifecycle.redirect_uri.as_str()),
    ];
    let response = client
        .post(GOOGLE_OAUTH_TOKEN_ENDPOINT)
        .form(&form)
        .send()
        .map_err(|error| format!("oauth token refresh request failed: {error}"))?;

    if response.status().is_success() {
        let payload = response
            .json::<OAuthRefreshSuccess>()
            .map_err(|error| format!("oauth refresh response parse failed: {error}"))?;
        let now_unix = now_epoch_seconds();
        return Ok(RefreshedGoogleToken {
            access_token: payload.access_token,
            expires_at_unix: now_unix.saturating_add(payload.expires_in),
        });
    }

    let status = response.status();
    let body = response.text().unwrap_or_default();
    let detail = serde_json::from_str::<OAuthRefreshError>(body.as_str())
        .ok()
        .map(|payload| {
            if let Some(description) = payload.error_description {
                return description;
            }
            payload
                .error
                .unwrap_or_else(|| "oauth refresh failed".to_string())
        })
        .unwrap_or_else(|| body.trim().to_string());
    Err(format!("oauth token refresh failed ({status}): {detail}"))
}

fn parse_json_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
    context: String,
) -> Result<T, String> {
    if response.status().is_success() {
        return response
            .json::<T>()
            .map_err(|error| format!("{context} response parse failed: {error}"));
    }
    let status = response.status();
    let text = response.text().unwrap_or_default();
    Err(format!(
        "{context} failed with status {}: {}",
        status.as_u16(),
        text.trim()
    ))
}

fn parse_send_json_response<T: DeserializeOwned>(
    response: reqwest::blocking::Response,
    context: String,
) -> Result<T, SendProviderError> {
    if response.status().is_success() {
        return response.json::<T>().map_err(|error| SendProviderError {
            class: SendFailureClass::Transient,
            reason: format!("{context} response parse failed: {error}"),
        });
    }

    let status = response.status();
    let text = response.text().unwrap_or_default();
    let class = if status == StatusCode::TOO_MANY_REQUESTS
        || status == StatusCode::UNAUTHORIZED
        || status.is_server_error()
    {
        SendFailureClass::Transient
    } else {
        SendFailureClass::Permanent
    };
    Err(SendProviderError {
        class,
        reason: format!(
            "{context} failed with status {}: {}",
            status.as_u16(),
            text.trim()
        ),
    })
}

pub(super) struct LiveGmailMailboxProvider {
    session: GmailSession,
}

impl LiveGmailMailboxProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailMailboxProvider for LiveGmailMailboxProvider {
    fn list_messages(
        &self,
        page_token: Option<&str>,
        page_size: usize,
    ) -> Result<GmailBackfillPage, GmailConnectorError> {
        #[derive(Deserialize)]
        struct MessageRow {
            id: String,
        }

        #[derive(Deserialize)]
        struct ListResponse {
            #[serde(default)]
            messages: Vec<MessageRow>,
            #[serde(rename = "nextPageToken")]
            next_page_token: Option<String>,
        }

        let mut query = vec![
            ("maxResults", page_size.to_string()),
            ("q", GMAIL_QUERY_INBOX.to_string()),
        ];
        if let Some(page_token) = page_token {
            query.push(("pageToken", page_token.to_string()));
        }
        let response = self
            .session
            .auth_get_json::<ListResponse>("messages", query.as_slice())
            .map_err(GmailConnectorError::Provider)?;
        let message_ids = response
            .messages
            .into_iter()
            .map(|row| row.id)
            .collect::<Vec<_>>();
        Ok(GmailBackfillPage {
            message_ids,
            next_page_token: response.next_page_token,
        })
    }

    fn get_message(&self, message_id: &str) -> Result<GmailMessage, GmailConnectorError> {
        let endpoint = format!("messages/{message_id}");
        let response = self
            .session
            .auth_get_json::<GmailMessageResponse>(
                endpoint.as_str(),
                &[("format", "full".to_string())],
            )
            .map_err(GmailConnectorError::Provider)?;

        decode_gmail_message(response)
    }
}

#[derive(Debug, Deserialize)]
struct GmailMessageResponse {
    id: String,
    #[serde(rename = "threadId")]
    thread_id: String,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(rename = "labelIds", default)]
    label_ids: Vec<String>,
    payload: Option<GmailPayloadNode>,
}

#[derive(Debug, Deserialize)]
struct GmailPayloadNode {
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
    #[serde(default)]
    headers: Vec<GmailHeaderValue>,
    body: Option<GmailPayloadBody>,
    #[serde(default)]
    parts: Vec<GmailPayloadNode>,
}

#[derive(Debug, Deserialize)]
struct GmailPayloadBody {
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailHeaderValue {
    name: String,
    value: String,
}

fn decode_gmail_message(response: GmailMessageResponse) -> Result<GmailMessage, GmailConnectorError> {
    let payload = response
        .payload
        .ok_or_else(|| GmailConnectorError::Provider("gmail message payload missing".to_string()))?;
    let headers = flatten_headers(payload.headers.as_slice());
    let (mime_type, body) = decode_message_body(&payload);
    let internal_date_ms = response
        .internal_date
        .as_deref()
        .unwrap_or("0")
        .parse::<u64>()
        .unwrap_or(0);
    let participants = participants_from_headers(headers.as_slice());

    Ok(GmailMessage {
        id: response.id,
        thread_id: response.thread_id,
        payload: GmailMessagePayload {
            headers,
            body: GmailMessageBody {
                mime_type,
                data: body,
            },
        },
        participants,
        metadata: GmailMessageMetadata {
            internal_date_ms,
            label_ids: response.label_ids,
        },
    })
}

fn flatten_headers(headers: &[GmailHeaderValue]) -> Vec<GmailMessageHeader> {
    headers
        .iter()
        .map(|header| GmailMessageHeader {
            name: header.name.clone(),
            value: header.value.clone(),
        })
        .collect()
}

fn decode_message_body(payload: &GmailPayloadNode) -> (String, String) {
    if let Some((mime, data)) = first_part_data(payload, "text/plain") {
        return (mime, decode_gmail_body_data(data));
    }
    if let Some((mime, data)) = first_part_data(payload, "text/html") {
        return (mime, decode_gmail_body_data(data));
    }
    if let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref()) {
        return (
            payload
                .mime_type
                .clone()
                .unwrap_or_else(|| "text/plain".to_string()),
            decode_gmail_body_data(data),
        );
    }
    ("text/plain".to_string(), String::new())
}

fn first_part_data<'a>(payload: &'a GmailPayloadNode, mime_match: &str) -> Option<(String, &'a str)> {
    if payload
        .mime_type
        .as_deref()
        .is_some_and(|mime| mime.eq_ignore_ascii_case(mime_match))
        && let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref())
    {
        return Some((mime_match.to_string(), data));
    }

    for part in &payload.parts {
        if let Some(found) = first_part_data(part, mime_match) {
            return Some(found);
        }
    }
    None
}

fn decode_gmail_body_data(raw: &str) -> String {
    let decoded = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(raw)
        .or_else(|_| base64::engine::general_purpose::URL_SAFE.decode(raw));
    match decoded {
        Ok(bytes) => String::from_utf8(bytes)
            .unwrap_or_else(|_| String::from_utf8_lossy(raw.as_bytes()).to_string()),
        Err(_) => raw.to_string(),
    }
}

fn participants_from_headers(
    headers: &[GmailMessageHeader],
) -> Vec<openagents_email_agent::GmailThreadParticipant> {
    let mut participants = Vec::new();
    let mut seen = BTreeSet::<String>::new();
    for name in ["From", "To", "Cc", "Bcc"] {
        for header in headers
            .iter()
            .filter(|header| header.name.eq_ignore_ascii_case(name))
        {
            for token in header.value.split(',') {
                let (email, display_name) = parse_email_token(token);
                if email.is_empty() {
                    continue;
                }
                let normalized = email.to_ascii_lowercase();
                if !seen.insert(normalized) {
                    continue;
                }
                participants.push(openagents_email_agent::GmailThreadParticipant {
                    email,
                    display_name,
                });
            }
        }
    }
    participants
}

fn parse_email_token(raw: &str) -> (String, Option<String>) {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return (String::new(), None);
    }
    if let (Some(open), Some(close)) = (trimmed.find('<'), trimmed.find('>'))
        && open < close
    {
        let display = trimmed[..open].trim().trim_matches('"').trim().to_string();
        let email = trimmed[open + 1..close].trim().to_string();
        let display_name = if display.is_empty() {
            None
        } else {
            Some(display)
        };
        return (email, display_name);
    }
    (trimmed.trim_matches('"').to_string(), None)
}

pub(super) struct LiveGmailHistoryProvider {
    session: GmailSession,
}

impl LiveGmailHistoryProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailHistoryProvider for LiveGmailHistoryProvider {
    fn fetch_history_since(
        &self,
        since_history_id: Option<u64>,
        max_results: usize,
    ) -> Result<GmailSyncBatch, GmailSyncError> {
        if let Some(since_history_id) = since_history_id {
            match self.fetch_history_batch(since_history_id, max_results) {
                Ok(batch) => Ok(batch),
                Err(error) if is_stale_history_error(error.as_str()) => {
                    Ok(GmailSyncBatch {
                        next_history_id: since_history_id.saturating_sub(1),
                        deltas: Vec::new(),
                    })
                }
                Err(error) => Err(GmailSyncError::Provider(error)),
            }
        } else {
            self.fetch_profile_history_id()
        }
    }
}

impl LiveGmailHistoryProvider {
    fn fetch_profile_history_id(&self) -> Result<GmailSyncBatch, GmailSyncError> {
        #[derive(Deserialize)]
        struct ProfileResponse {
            #[serde(rename = "historyId")]
            history_id: Option<String>,
        }

        let profile = self
            .session
            .auth_get_json::<ProfileResponse>("profile", &[])
            .map_err(GmailSyncError::Provider)?;
        let next_history_id = profile
            .history_id
            .as_deref()
            .unwrap_or("0")
            .parse::<u64>()
            .unwrap_or(0);
        Ok(GmailSyncBatch {
            next_history_id,
            deltas: Vec::new(),
        })
    }

    fn fetch_history_batch(
        &self,
        since_history_id: u64,
        max_results: usize,
    ) -> Result<GmailSyncBatch, String> {
        let response = self
            .session
            .auth_get_json::<LiveHistoryResponse>(
                "history",
                &[
                    ("startHistoryId", since_history_id.to_string()),
                    ("maxResults", max_results.to_string()),
                ],
            )
            .map_err(|error| {
                format!("gmail history fetch failed since {since_history_id}: {error}")
            })?;

        let mut deltas = Vec::<GmailDeltaItem>::new();
        let mut seen_delta_keys = BTreeSet::<String>::new();
        let mut max_history_id = response
            .history_id
            .as_deref()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(since_history_id);

        for entry in response.history {
            let history_id = entry
                .id
                .as_deref()
                .and_then(|value| value.parse::<u64>().ok())
                .unwrap_or(since_history_id);
            max_history_id = max_history_id.max(history_id);

            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.messages_added,
                GmailDeltaOperation::Create,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.messages_deleted,
                GmailDeltaOperation::Delete,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.labels_added,
                GmailDeltaOperation::Update,
                max_results,
            );
            ingest_history_rows(
                &mut deltas,
                &mut seen_delta_keys,
                history_id,
                entry.labels_removed,
                GmailDeltaOperation::Update,
                max_results,
            );
            if deltas.len() >= max_results {
                break;
            }
        }

        Ok(GmailSyncBatch {
            next_history_id: max_history_id,
            deltas,
        })
    }
}

#[derive(Deserialize)]
struct LiveHistoryResponse {
    #[serde(rename = "historyId")]
    history_id: Option<String>,
    #[serde(default)]
    history: Vec<LiveHistoryEntry>,
}

#[derive(Deserialize)]
struct LiveHistoryEntry {
    id: Option<String>,
    #[serde(rename = "messagesAdded", default)]
    messages_added: Vec<LiveHistoryRecord>,
    #[serde(rename = "messagesDeleted", default)]
    messages_deleted: Vec<LiveHistoryRecord>,
    #[serde(rename = "labelsAdded", default)]
    labels_added: Vec<LiveHistoryRecord>,
    #[serde(rename = "labelsRemoved", default)]
    labels_removed: Vec<LiveHistoryRecord>,
}

#[derive(Deserialize)]
struct LiveHistoryRecord {
    message: Option<LiveHistoryMessage>,
}

#[derive(Deserialize)]
struct LiveHistoryMessage {
    id: Option<String>,
}

fn ingest_history_rows(
    deltas: &mut Vec<GmailDeltaItem>,
    seen_delta_keys: &mut BTreeSet<String>,
    history_id: u64,
    rows: Vec<LiveHistoryRecord>,
    operation: GmailDeltaOperation,
    max_results: usize,
) {
    for row in rows {
        let Some(message_id) = row.message.and_then(|message| message.id) else {
            continue;
        };
        if message_id.is_empty() {
            continue;
        }
        let delta_key = format!("{history_id}:{operation:?}:{message_id}");
        if !seen_delta_keys.insert(delta_key) {
            continue;
        }
        deltas.push(GmailDeltaItem {
            message_id,
            operation,
            history_id,
        });
        if deltas.len() >= max_results {
            break;
        }
    }
}

fn is_stale_history_error(error: &str) -> bool {
    let lowercase = error.to_ascii_lowercase();
    lowercase.contains("404")
        || lowercase.contains("start historyid")
        || lowercase.contains("requested entity was not found")
}

pub(super) struct LiveGmailSendProvider {
    session: GmailSession,
}

impl LiveGmailSendProvider {
    fn new(session: GmailSession) -> Self {
        Self { session }
    }
}

impl GmailSendProvider for LiveGmailSendProvider {
    fn send_message(&self, request: &SendRequest) -> Result<GmailSendSuccess, SendProviderError> {
        #[derive(Serialize)]
        struct SendRequestBody {
            raw: String,
        }

        #[derive(Deserialize)]
        struct SendResponse {
            id: Option<String>,
        }

        let rfc822 = compose_plain_text_rfc822(request);
        let raw = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(rfc822.as_bytes());
        let response = self.session.auth_post_json::<_, SendResponse>(
            "messages/send",
            &SendRequestBody { raw },
        )?;
        let provider_message_id = response.id.unwrap_or_else(|| "gmail:unknown".to_string());
        Ok(GmailSendSuccess { provider_message_id })
    }
}

fn compose_plain_text_rfc822(request: &SendRequest) -> String {
    let sanitized_subject = request.subject.replace('\r', " ").replace('\n', " ");
    let body = request.body.replace("\r\n", "\n").replace('\r', "\n");
    format!(
        "To: {}\r\nSubject: {}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\nMIME-Version: 1.0\r\nX-OpenAgents-Idempotency-Key: {}\r\n\r\n{}\r\n",
        request.recipient_email.trim(),
        sanitized_subject.trim(),
        request.idempotency_key.trim(),
        body.trim_end()
    )
}

pub(super) fn now_epoch_seconds() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

pub(super) fn stale_cursor_reason() -> &'static str {
    GMAIL_SYNC_REBOOTSTRAP_REASON
}

#[cfg(test)]
mod tests {
    use super::{compose_plain_text_rfc822, parse_email_token};
    use openagents_email_agent::SendRequest;

    #[test]
    fn parse_email_token_extracts_display_name_and_address() {
        let (email, display_name) = parse_email_token("Example Person <person@example.com>");
        assert_eq!(email, "person@example.com");
        assert_eq!(display_name.as_deref(), Some("Example Person"));
    }

    #[test]
    fn compose_rfc822_keeps_idempotency_key_header() {
        let request = SendRequest {
            draft_id: "draft-1".to_string(),
            idempotency_key: "idem-1".to_string(),
            recipient_email: "person@example.com".to_string(),
            subject: "Subject".to_string(),
            body: "Body".to_string(),
        };
        let message = compose_plain_text_rfc822(&request);
        assert!(message.contains("To: person@example.com"));
        assert!(message.contains("Subject: Subject"));
        assert!(message.contains("X-OpenAgents-Idempotency-Key: idem-1"));
        assert!(message.ends_with("Body\r\n"));
    }
}
