use crate::config::Config;
use crate::db::{Database, ProviderToken, UpsertMessage, UpsertThread};
use crate::error::ApiError;
use crate::types::{BackfillResponse, GmailAuthRequest};
use anyhow::{Context, Result};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use chrono::{Duration, TimeZone, Utc};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use tracing::warn;

#[derive(Debug, Clone)]
pub struct GmailClient {
    http: reqwest::Client,
    config: Config,
}

impl GmailClient {
    pub fn new(config: Config) -> Self {
        Self {
            http: reqwest::Client::new(),
            config,
        }
    }

    pub fn auth_url(
        &self,
        redirect_uri: &str,
        state: &str,
        code_challenge: Option<&str>,
    ) -> Result<String, ApiError> {
        let Some(client_id) = self.config.google_client_id.as_ref() else {
            return Err(ApiError::ServiceUnavailable(
                "GOOGLE_OAUTH_CLIENT_ID is not configured".to_string(),
            ));
        };

        let mut params = HashMap::from([
            ("client_id", client_id.to_string()),
            ("redirect_uri", redirect_uri.to_string()),
            ("response_type", "code".to_string()),
            ("access_type", "offline".to_string()),
            ("prompt", "consent".to_string()),
            ("scope", self.config.google_scopes.clone()),
            ("state", state.to_string()),
        ]);

        if let Some(code_challenge) = code_challenge {
            params.insert("code_challenge", code_challenge.to_string());
            params.insert("code_challenge_method", "S256".to_string());
        }

        let encoded = params
            .iter()
            .map(|(k, v)| format!("{}={}", urlencoding::encode(k), urlencoding::encode(v)))
            .collect::<Vec<_>>()
            .join("&");

        Ok(format!(
            "https://accounts.google.com/o/oauth2/v2/auth?{encoded}"
        ))
    }

    pub async fn exchange_code(
        &self,
        db: &Database,
        req: GmailAuthRequest,
    ) -> Result<(), ApiError> {
        let client_id = self.config.google_client_id.as_ref().ok_or_else(|| {
            ApiError::ServiceUnavailable("missing GOOGLE_OAUTH_CLIENT_ID".to_string())
        })?;
        let client_secret = self.config.google_client_secret.as_ref().ok_or_else(|| {
            ApiError::ServiceUnavailable("missing GOOGLE_OAUTH_CLIENT_SECRET".to_string())
        })?;

        let mut form = vec![
            ("code", req.code),
            ("client_id", client_id.clone()),
            ("client_secret", client_secret.clone()),
            ("redirect_uri", req.redirect_uri),
            ("grant_type", "authorization_code".to_string()),
        ];

        if let Some(verifier) = req.code_verifier {
            form.push(("code_verifier", verifier));
        }

        let resp = self
            .http
            .post("https://oauth2.googleapis.com/token")
            .form(&form)
            .send()
            .await
            .context("failed to exchange oauth code")
            .map_err(ApiError::internal)?;

        if !resp.status().is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ApiError::BadRequest(format!(
                "google oauth code exchange failed: {body}"
            )));
        }

        let token: GoogleTokenResponse = resp
            .json()
            .await
            .context("failed to decode oauth response")
            .map_err(ApiError::internal)?;

        let expires_at = token
            .expires_in
            .map(|seconds| Utc::now() + Duration::seconds(seconds));

        db.store_provider_token(
            "gmail",
            Some(&token.access_token),
            token.refresh_token.as_deref(),
            expires_at,
            token.scope.as_deref(),
            token.token_type.as_deref(),
        )
        .map_err(ApiError::internal)?;

        Ok(())
    }

    pub async fn ensure_access_token(&self, db: &Database) -> Result<String, ApiError> {
        let token = db
            .get_provider_token("gmail")
            .map_err(ApiError::internal)?
            .ok_or_else(|| ApiError::Unauthorized("gmail account is not connected".to_string()))?;

        if let Some(access_token) = token.access_token.clone() {
            let still_valid = token
                .expires_at
                .map(|expiry| expiry > Utc::now() + Duration::seconds(60))
                .unwrap_or(true);
            if still_valid {
                return Ok(access_token);
            }
        }

        self.refresh_access_token(db, token).await
    }

    async fn refresh_access_token(
        &self,
        db: &Database,
        existing: ProviderToken,
    ) -> Result<String, ApiError> {
        let client_id = self.config.google_client_id.as_ref().ok_or_else(|| {
            ApiError::ServiceUnavailable("missing GOOGLE_OAUTH_CLIENT_ID".to_string())
        })?;
        let client_secret = self.config.google_client_secret.as_ref().ok_or_else(|| {
            ApiError::ServiceUnavailable("missing GOOGLE_OAUTH_CLIENT_SECRET".to_string())
        })?;

        let refresh_token = existing.refresh_token.ok_or_else(|| {
            ApiError::Unauthorized("gmail refresh token missing; reconnect Gmail".to_string())
        })?;

        let form = vec![
            ("refresh_token", refresh_token.clone()),
            ("client_id", client_id.clone()),
            ("client_secret", client_secret.clone()),
            ("grant_type", "refresh_token".to_string()),
        ];

        let resp = self
            .http
            .post("https://oauth2.googleapis.com/token")
            .form(&form)
            .send()
            .await
            .context("failed to refresh oauth token")
            .map_err(ApiError::internal)?;

        if !resp.status().is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ApiError::Unauthorized(format!(
                "gmail token refresh failed: {body}"
            )));
        }

        let token: GoogleTokenResponse = resp
            .json()
            .await
            .context("failed decoding token refresh response")
            .map_err(ApiError::internal)?;

        let expires_at = token
            .expires_in
            .map(|seconds| Utc::now() + Duration::seconds(seconds));

        db.store_provider_token(
            "gmail",
            Some(&token.access_token),
            Some(&refresh_token),
            expires_at,
            token.scope.as_deref().or(existing.scope.as_deref()),
            token.token_type.as_deref(),
        )
        .map_err(ApiError::internal)?;

        Ok(token.access_token)
    }

    pub async fn backfill(&self, db: &Database, days: u32) -> Result<BackfillResponse, ApiError> {
        let token = self.ensure_access_token(db).await?;

        let query = format!("newer_than:{}d", days.clamp(1, 3650));
        let list_resp = self
            .http
            .get("https://gmail.googleapis.com/gmail/v1/users/me/threads")
            .query(&[("q", query.as_str()), ("maxResults", "100")])
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .send()
            .await
            .context("failed listing gmail threads")
            .map_err(ApiError::internal)?;

        if !list_resp.status().is_success() {
            let body = list_resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ApiError::BadRequest(format!(
                "gmail thread listing failed: {body}"
            )));
        }

        let list: GmailThreadListResponse = list_resp
            .json()
            .await
            .context("failed decoding thread list")
            .map_err(ApiError::internal)?;

        let mut imported_threads = 0_usize;
        let mut imported_messages = 0_usize;

        for thread_ref in list.threads.unwrap_or_default() {
            let detail_resp = self
                .http
                .get(format!(
                    "https://gmail.googleapis.com/gmail/v1/users/me/threads/{}",
                    thread_ref.id
                ))
                .query(&[("format", "full")])
                .header(AUTHORIZATION, format!("Bearer {token}"))
                .send()
                .await
                .context("failed fetching thread detail")
                .map_err(ApiError::internal)?;

            if !detail_resp.status().is_success() {
                let body = detail_resp
                    .text()
                    .await
                    .unwrap_or_else(|_| "<no body>".to_string());
                warn!("skipping thread {} due to error: {}", thread_ref.id, body);
                continue;
            }

            let detail: GmailThread = detail_resp
                .json()
                .await
                .context("failed decoding thread detail")
                .map_err(ApiError::internal)?;

            let normalized = normalize_gmail_thread(detail).map_err(ApiError::internal)?;

            db.upsert_thread(UpsertThread {
                id: normalized.id.clone(),
                gmail_thread_id: normalized.gmail_thread_id,
                subject: normalized.subject.clone(),
                snippet: normalized.snippet.clone(),
                from_address: normalized.from_address,
                last_message_at: normalized.last_message_at,
            })
            .map_err(ApiError::internal)?;

            imported_threads += 1;

            for message in normalized.messages {
                db.upsert_message(UpsertMessage {
                    id: message.id,
                    thread_id: normalized.id.clone(),
                    gmail_message_id: message.gmail_message_id,
                    sender: message.sender,
                    recipient: message.recipient,
                    subject: message.subject,
                    snippet: message.snippet,
                    body: message.body,
                    inbound: message.inbound,
                    sent_at: message.sent_at,
                })
                .map_err(ApiError::internal)?;
                imported_messages += 1;
            }
        }

        Ok(BackfillResponse {
            imported_threads,
            imported_messages,
        })
    }

    pub async fn send_reply(
        &self,
        db: &Database,
        thread_id: &str,
        to: &str,
        subject: &str,
        body: &str,
    ) -> Result<String, ApiError> {
        let access_token = self.ensure_access_token(db).await?;

        let message = format!(
            "To: {to}\r\nSubject: Re: {subject}\r\nContent-Type: text/plain; charset=\"UTF-8\"\r\n\r\n{body}\r\n"
        );
        let raw = URL_SAFE_NO_PAD.encode(message.as_bytes());

        let resp = self
            .http
            .post("https://gmail.googleapis.com/gmail/v1/users/me/messages/send")
            .header(AUTHORIZATION, format!("Bearer {access_token}"))
            .header(CONTENT_TYPE, "application/json")
            .json(&json!({ "raw": raw, "threadId": thread_id }))
            .send()
            .await
            .context("failed sending gmail message")
            .map_err(ApiError::internal)?;

        if !resp.status().is_success() {
            let body = resp
                .text()
                .await
                .unwrap_or_else(|_| "<no body>".to_string());
            return Err(ApiError::BadRequest(format!("gmail send failed: {body}")));
        }

        let out: GmailSendResponse = resp
            .json()
            .await
            .context("failed decoding gmail send response")
            .map_err(ApiError::internal)?;

        Ok(out.id)
    }
}

#[derive(Debug, Deserialize)]
struct GoogleTokenResponse {
    access_token: String,
    refresh_token: Option<String>,
    expires_in: Option<i64>,
    scope: Option<String>,
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailThreadListResponse {
    threads: Option<Vec<GmailThreadRef>>,
}

#[derive(Debug, Deserialize)]
struct GmailThreadRef {
    id: String,
}

#[derive(Debug, Deserialize)]
struct GmailThread {
    id: String,
    snippet: Option<String>,
    messages: Option<Vec<GmailMessage>>,
}

#[derive(Debug, Deserialize)]
struct GmailMessage {
    id: String,
    #[serde(rename = "threadId")]
    _thread_id: String,
    #[serde(rename = "internalDate")]
    internal_date: Option<String>,
    #[serde(default)]
    label_ids: Vec<String>,
    snippet: Option<String>,
    payload: Option<GmailPayload>,
}

#[derive(Debug, Deserialize)]
struct GmailPayload {
    headers: Option<Vec<GmailHeader>>,
    body: Option<GmailBody>,
    parts: Option<Vec<GmailPayload>>,
    #[serde(rename = "mimeType")]
    mime_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailBody {
    data: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GmailHeader {
    name: String,
    value: String,
}

#[derive(Debug)]
struct NormalizedThread {
    id: String,
    gmail_thread_id: String,
    subject: String,
    snippet: String,
    from_address: String,
    last_message_at: chrono::DateTime<Utc>,
    messages: Vec<NormalizedMessage>,
}

#[derive(Debug)]
struct NormalizedMessage {
    id: String,
    gmail_message_id: String,
    sender: String,
    recipient: String,
    subject: String,
    snippet: String,
    body: String,
    inbound: bool,
    sent_at: chrono::DateTime<Utc>,
}

fn normalize_gmail_thread(thread: GmailThread) -> Result<NormalizedThread> {
    let mut messages = thread.messages.unwrap_or_default();
    messages.sort_by_key(|msg| {
        msg.internal_date
            .as_deref()
            .and_then(|ts| ts.parse::<i64>().ok())
            .unwrap_or_default()
    });

    let mut normalized_messages = Vec::new();
    for message in messages {
        let headers = parse_headers(message.payload.as_ref());
        let sender = headers
            .get("from")
            .map(|s| extract_email_address(s))
            .unwrap_or_else(|| "unknown@example.com".to_string());
        let recipient = headers
            .get("to")
            .map(|s| extract_email_address(s))
            .unwrap_or_else(|| "unknown@example.com".to_string());
        let subject = headers
            .get("subject")
            .cloned()
            .unwrap_or_else(|| "(no subject)".to_string());

        let body = decode_payload_body(message.payload.as_ref())
            .unwrap_or_else(|| message.snippet.clone().unwrap_or_default());
        let snippet = message.snippet.clone().unwrap_or_else(|| body.clone());

        let inbound = !message.label_ids.iter().any(|label| label == "SENT");

        let sent_at = message
            .internal_date
            .as_deref()
            .and_then(|ts| ts.parse::<i64>().ok())
            .map(|ms| {
                Utc.timestamp_millis_opt(ms)
                    .single()
                    .unwrap_or_else(Utc::now)
            })
            .unwrap_or_else(Utc::now);

        normalized_messages.push(NormalizedMessage {
            id: message.id.clone(),
            gmail_message_id: message.id,
            sender,
            recipient,
            subject,
            snippet,
            body,
            inbound,
            sent_at,
        });
    }

    if normalized_messages.is_empty() {
        anyhow::bail!("thread has no messages");
    }

    let last = normalized_messages
        .last()
        .context("thread missing latest message")?;
    let first = normalized_messages
        .iter()
        .find(|m| m.inbound)
        .unwrap_or(last);

    Ok(NormalizedThread {
        id: thread.id.clone(),
        gmail_thread_id: thread.id,
        subject: first.subject.clone(),
        snippet: thread
            .snippet
            .unwrap_or_else(|| last.snippet.chars().take(180).collect()),
        from_address: first.sender.clone(),
        last_message_at: last.sent_at,
        messages: normalized_messages,
    })
}

fn parse_headers(payload: Option<&GmailPayload>) -> HashMap<String, String> {
    let mut out = HashMap::new();
    if let Some(payload) = payload
        && let Some(headers) = payload.headers.as_ref()
    {
        for header in headers {
            out.insert(header.name.to_ascii_lowercase(), header.value.clone());
        }
    }
    out
}

fn decode_payload_body(payload: Option<&GmailPayload>) -> Option<String> {
    let payload = payload?;

    if let Some(data) = payload.body.as_ref().and_then(|body| body.data.as_deref())
        && let Some(decoded) = decode_base64_url(data)
        && !decoded.trim().is_empty()
    {
        return Some(decoded);
    }

    if let Some(parts) = payload.parts.as_ref() {
        for part in parts {
            let mime = part.mime_type.as_deref().unwrap_or_default();
            if mime.starts_with("text/plain")
                && let Some(body) = decode_payload_body(Some(part))
                && !body.trim().is_empty()
            {
                return Some(body);
            }
        }

        for part in parts {
            if let Some(body) = decode_payload_body(Some(part))
                && !body.trim().is_empty()
            {
                return Some(body);
            }
        }
    }

    None
}

fn decode_base64_url(data: &str) -> Option<String> {
    let normalized = data.replace('-', "+").replace('_', "/");
    let padding = (4 - normalized.len() % 4) % 4;
    let mut padded = normalized;
    padded.push_str(&"=".repeat(padding));
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(padded)
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn extract_email_address(raw: &str) -> String {
    if let (Some(start), Some(end)) = (raw.find('<'), raw.find('>')) {
        return raw[start + 1..end].trim().to_lowercase();
    }
    raw.trim().to_lowercase()
}

#[derive(Debug, Serialize, Deserialize)]
struct GmailSendResponse {
    id: String,
}
