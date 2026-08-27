//! The trace ingest client: `POST /api/v1/traces`.
//!
//! The Rust half of what `packages/openagents-cli/src/trace-client.ts` does. The
//! route takes an ATIF v1 document as the whole request body, stores it against the
//! calling account, and answers 201 for a document it did not hold or 200 for one it
//! already has under the same digest.
//!
//! Three things this client will not do:
//!
//! - It does not invent a visibility. The server's vocabulary is the forge
//!   transparency ladder, and `dark` — nothing public — is the default there and
//!   here. A caller who wants more has to say which rung.
//! - It does not call an existing trace a new one. The status is the only thing that
//!   tells them apart, so it is kept rather than discarded.
//! - It does not report a stored trace by a link. The response carries a `url`
//!   pointing at `GET /api/v1/traces/:id`, and that route does not exist, so
//!   printing it would hand someone a 404 dressed as a receipt. The id and the
//!   digest are real, and they are what get reported.

use reqwest::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::tracker::{ApiError, error_fields, error_sentence, header_request_id, urlencode};

/// The transparency ladder the server stores a trace under.
///
/// `dark` is nothing public, `pulse` is metadata only, `ledger` is content and
/// metadata, `glass` is full access. The database enforces this exact set with a
/// CHECK constraint, so a name outside it is refused here with the list rather than
/// sent on to earn a 422 that does not say what the choices were.
pub const TRACE_VISIBILITIES: [&str; 4] = ["dark", "pulse", "ledger", "glass"];

/// What the server defaults to when no visibility is named: nothing public.
pub const DEFAULT_TRACE_VISIBILITY: &str = "dark";

/// The largest body the ingest route accepts, in bytes.
pub const MAXIMUM_TRACE_BYTES: u64 = 10_485_760;

/// Read a visibility name, or refuse with the set the server actually has.
pub fn read_visibility(raw: &str) -> Result<&'static str, ApiError> {
    let trimmed = raw.trim().to_lowercase();
    TRACE_VISIBILITIES
        .iter()
        .copied()
        .find(|name| *name == trimmed)
        .ok_or_else(|| {
            ApiError::Input(format!(
                "--visibility must be one of {}; got \"{}\".",
                TRACE_VISIBILITIES.join(", "),
                raw
            ))
        })
}

/// What the server said it stored. Every field is the server's, not a guess.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoredTrace {
    pub id: String,
    pub digest: String,
    pub byte_size: u64,
    pub visibility: String,
    pub inserted_at: String,
    /// 201: the server did not hold this document. 200: it already did.
    pub created: bool,
}

pub struct TraceClient {
    pub api_base: String,
    pub token: Option<String>,
    pub http: reqwest::Client,
}

impl TraceClient {
    pub fn new(api_base: &str, token: Option<String>) -> Self {
        Self {
            api_base: api_base.trim_end_matches('/').to_string(),
            token,
            http: reqwest::Client::new(),
        }
    }

    fn headers(&self) -> HeaderMap {
        let mut map = HeaderMap::new();
        map.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        map.insert(ACCEPT, HeaderValue::from_static("application/json"));
        if let Some(token) = &self.token {
            if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", token)) {
                map.insert(AUTHORIZATION, value);
            }
        }
        map
    }

    /// Send one ATIF document. `visibility` must already have passed
    /// [`read_visibility`]; `assignment_id` names the forge attempt the trajectory
    /// belongs to, when there is one.
    pub async fn upload(
        &self,
        document: &Value,
        visibility: &str,
        assignment_id: Option<&str>,
    ) -> Result<StoredTrace, ApiError> {
        let mut url = format!(
            "{}/traces?visibility={}",
            self.api_base,
            urlencode(visibility)
        );
        if let Some(assignment) = assignment_id {
            url.push_str(&format!("&assignment_id={}", urlencode(assignment)));
        }

        crate::diag::request("POST", &url);
        let response = self
            .http
            .post(&url)
            .headers(self.headers())
            .json(document)
            .send()
            .await
            .map_err(|error| {
                crate::diag::transport(&url, &error.to_string());
                ApiError::Transport {
                    operation: "upload a trace".to_string(),
                    why: error.to_string(),
                }
            })?;

        let status = response.status().as_u16();
        crate::diag::response(status, &url);
        let header_id = header_request_id(&response);
        let text = response.text().await.map_err(|error| ApiError::Transport {
            operation: "upload a trace".to_string(),
            why: error.to_string(),
        })?;

        if status != 200 && status != 201 {
            let message = error_sentence(&text, status);
            crate::diag::refused(status, &message);
            let (code, body_id) = error_fields(&text);
            return Err(ApiError::Refused {
                operation: "upload a trace".to_string(),
                status,
                message,
                code,
                request_id: header_id.or(body_id),
            });
        }

        let stored: Value = serde_json::from_str(&text).map_err(|error| ApiError::Malformed {
            operation: "upload a trace".to_string(),
            why: error.to_string(),
        })?;
        let field = |key: &str| stored.get(key).and_then(Value::as_str).map(String::from);

        // An accepted status with no id is not a stored trace. Reporting one anyway
        // is how a caller comes to believe a trace exists server-side that nothing
        // can ever be found by.
        let (Some(id), Some(digest)) = (field("id"), field("digest")) else {
            return Err(ApiError::Malformed {
                operation: "upload a trace".to_string(),
                why: "the server accepted the trace but named no id or digest".to_string(),
            });
        };

        Ok(StoredTrace {
            id,
            digest,
            byte_size: stored
                .get("byte_size")
                .and_then(Value::as_u64)
                .unwrap_or_default(),
            visibility: field("visibility").unwrap_or_else(|| visibility.to_string()),
            inserted_at: field("inserted_at").unwrap_or_default(),
            created: status == 201,
        })
    }
}
