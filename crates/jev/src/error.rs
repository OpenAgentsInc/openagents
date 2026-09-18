//! The error set, and the message a failed response carries.
//!
//! The two official SDKs raise a dozen classes between them. One enum with a
//! `kind` on the API variant gives a caller the same match without a dozen
//! types, and every variant names what a caller can act on: the status, the
//! request id, the field that failed to decode, or the timeout that expired.

use std::fmt;
use std::time::Duration;

use reqwest::header::HeaderMap;
use serde_json::Value;
use thiserror::Error;

use crate::retry::parse_retry_after;

/// The longest body that reaches an error message. Both official SDKs cut at
/// the same count and mark the cut with an ellipsis.
const MAX_BODY_IN_MESSAGE: usize = 200;

/// The response header that carries the request id.
pub(crate) const REQUEST_ID_HEADER: &str = "x-typesafe-request-id";

/// A response body, parsed as JSON when it parses and kept as text when it
/// does not. A server or a proxy does not always send a content type, so the
/// text form is the fallback rather than a failure.
#[derive(Debug, Clone, PartialEq)]
pub enum ResponseBody {
    /// A body that parsed as JSON.
    Json(Value),
    /// A body that did not parse as JSON, as it arrived.
    Text(String),
}

impl ResponseBody {
    /// The parsed JSON, or `None` when the body arrived as text.
    #[must_use]
    pub fn as_json(&self) -> Option<&Value> {
        match self {
            Self::Json(value) => Some(value),
            Self::Text(_) => None,
        }
    }

    /// The body as text, or `None` when it parsed as JSON.
    #[must_use]
    pub fn as_text(&self) -> Option<&str> {
        match self {
            Self::Json(_) => None,
            Self::Text(text) => Some(text),
        }
    }
}

impl fmt::Display for ResponseBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(value) => write!(f, "{value}"),
            Self::Text(text) => write!(f, "{text}"),
        }
    }
}

/// What a failed request failed at.
#[derive(Debug, Error)]
pub enum Error {
    /// A setting is missing or out of range. Raised before any request.
    #[error("{0}")]
    Config(String),

    /// A question set failed the checks the SDK runs before any request.
    #[error("{}", describe_question(id, message))]
    Question {
        /// The question id, or an empty string when the whole set is at fault.
        id: String,
        /// What is wrong with it.
        message: String,
    },

    /// The API answered with a status outside the 2xx range.
    ///
    /// The error is boxed because it carries the response headers and body, and
    /// a `Result` whose failure is that wide costs every call that returns one.
    #[error(transparent)]
    Api(Box<ApiError>),

    /// The request never reached the API, or its response never arrived.
    #[error("{message}")]
    Connection {
        /// What the transport reported.
        message: String,
        /// The transport error, when there is one to carry.
        #[source]
        source: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// One attempt ran past its timeout.
    #[error("the request timed out after {}ms", timeout.as_millis())]
    Timeout {
        /// The per-attempt timeout that expired.
        timeout: Duration,
    },

    /// A 2xx body was missing a field, or carried one the SDK cannot read.
    #[error("invalid response data at {field_path:?}")]
    ResponseValidation {
        /// The response status.
        status: u16,
        /// A dotted path to the field, such as `answers.tone.confidence`.
        field_path: String,
        /// The body the SDK read, when it read one. It is boxed so the enum
        /// stays narrow under every feature combination a dependent crate's
        /// `serde_json` features give `Value`.
        body: Option<Box<ResponseBody>>,
        /// The request id, when the response carried one.
        request_id: Option<String>,
    },

    /// A typed accessor asked for one answer type and found another.
    #[error("answer {id:?} is a {found} answer, not a {expected} answer")]
    AnswerType {
        /// The question id.
        id: String,
        /// The type the caller asked for.
        expected: &'static str,
        /// The type the response carried.
        found: &'static str,
    },

    /// A typed accessor named an answer the response does not carry.
    #[error("the response carries no answer named {id:?}")]
    MissingAnswer {
        /// The question id.
        id: String,
    },
}

impl Error {
    /// Build a connection error from a transport error.
    pub(crate) fn connection(source: reqwest::Error) -> Self {
        Self::Connection {
            message: format!("connection error: {source}"),
            source: Some(Box::new(source)),
        }
    }

    /// The request id, when the failure carries one.
    #[must_use]
    pub fn request_id(&self) -> Option<&str> {
        match self {
            Self::Api(error) => error.request_id.as_deref(),
            Self::ResponseValidation { request_id, .. } => request_id.as_deref(),
            _ => None,
        }
    }
}

impl From<ApiError> for Error {
    fn from(error: ApiError) -> Self {
        Self::Api(Box::new(error))
    }
}

/// How `Error::Question` reads. A complaint about the whole set names no id.
fn describe_question(id: &str, message: &str) -> String {
    if id.is_empty() {
        message.to_string()
    } else {
        format!("question {id:?}: {message}")
    }
}

/// Which class of failure a status names. The official SDKs raise one class
/// per status; a caller matches this instead.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiErrorKind {
    /// 400: the request is invalid.
    BadRequest,
    /// 401: the key is missing or not accepted.
    Authentication,
    /// 403: the key does not carry access.
    PermissionDenied,
    /// 404: the route or the resource does not exist.
    NotFound,
    /// 422: the API rejected the request body.
    UnprocessableEntity,
    /// 429: the account is over its rate limit.
    RateLimit {
        /// The wait the server asked for, when it asked for one.
        retry_after: Option<Duration>,
    },
    /// 5xx: the API failed to answer.
    InternalServer,
    /// Any other status outside the 2xx range.
    Other,
}

impl ApiErrorKind {
    /// The kind a status and its headers name.
    #[must_use]
    pub fn of(status: u16, headers: &HeaderMap) -> Self {
        match status {
            400 => Self::BadRequest,
            401 => Self::Authentication,
            403 => Self::PermissionDenied,
            404 => Self::NotFound,
            422 => Self::UnprocessableEntity,
            429 => Self::RateLimit {
                retry_after: parse_retry_after(headers),
            },
            500..=599 => Self::InternalServer,
            _ => Self::Other,
        }
    }
}

/// A response with a status outside the 2xx range.
#[derive(Debug, Clone, PartialEq)]
pub struct ApiError {
    /// The response status.
    pub status: u16,
    /// The response headers.
    pub headers: HeaderMap,
    /// The body the SDK read, when the response carried one.
    pub body: Option<ResponseBody>,
    /// The request id, when the response carried one.
    pub request_id: Option<String>,
    /// The method and the URL, without the key.
    pub endpoint: String,
    /// Which class of failure the status names.
    pub kind: ApiErrorKind,
}

impl ApiError {
    /// Build the error for one failed response.
    pub(crate) fn new(
        endpoint: String,
        status: u16,
        headers: HeaderMap,
        body: Option<ResponseBody>,
    ) -> Self {
        let request_id = headers
            .get(REQUEST_ID_HEADER)
            .and_then(|value| value.to_str().ok())
            .map(str::to_string);
        let kind = ApiErrorKind::of(status, &headers);
        Self {
            status,
            headers,
            body,
            request_id,
            endpoint,
            kind,
        }
    }

    /// The message the body carries, read the way both official SDKs read it.
    ///
    /// An extracted message is returned as it stands, however long. A body
    /// that names no message falls back to the body itself, cut at 200
    /// characters, and an empty body or a JSON `null` to a note that there was
    /// none.
    #[must_use]
    pub fn message(&self) -> String {
        match self.body.as_ref() {
            // A body of `null` reads as no body, the way the Python SDK reads
            // one.
            Some(ResponseBody::Json(Value::Null)) | None => "status code (no body)".to_string(),
            Some(body) => match extract_message(body) {
                Some(message) if !message.is_empty() => message,
                _ => truncate(&body.to_string()),
            },
        }
    }

    /// The wait the server asked for on a rate limit, when it asked for one.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        parse_retry_after(&self.headers)
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {} {}", self.endpoint, self.status, self.message())?;
        if let Some(id) = self.request_id.as_deref() {
            write!(f, " (request_id={id})")?;
        }
        Ok(())
    }
}

impl std::error::Error for ApiError {}

/// The message a body names, in the order both official SDKs read: a text
/// body, `error`, `error.message`, `message`, `detail`, `detail.message`, or a
/// `detail` list rendered as `loc: msg` entries.
///
/// An extracted message is not cut: only the raw-body fallback is, the way
/// both official SDKs cut it.
fn extract_message(body: &ResponseBody) -> Option<String> {
    let value = match body {
        ResponseBody::Text(text) if text.is_empty() => return None,
        ResponseBody::Text(text) => return Some(text.clone()),
        // A body that is one JSON string is the message, the way both official
        // SDKs read a parsed body that turns out to be a string.
        ResponseBody::Json(Value::String(text)) if text.is_empty() => return None,
        ResponseBody::Json(Value::String(text)) => return Some(text.clone()),
        ResponseBody::Json(value) => value,
    };
    let object = value.as_object()?;
    if let Some(error) = object.get("error") {
        if let Some(text) = error.as_str() {
            return Some(text.to_string());
        }
        if let Some(text) = error.get("message").and_then(Value::as_str) {
            return Some(text.to_string());
        }
    }
    if let Some(text) = object.get("message").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    let detail = object.get("detail")?;
    if let Some(text) = detail.as_str() {
        return Some(text.to_string());
    }
    if let Some(text) = detail.get("message").and_then(Value::as_str) {
        return Some(text.to_string());
    }
    let entries = detail.as_array()?;
    let rendered: Vec<String> = entries.iter().filter_map(describe_entry).collect();
    if rendered.is_empty() {
        None
    } else {
        Some(rendered.join("; "))
    }
}

/// One entry of a validation list, as `loc: msg`. The `body` element of a
/// location is the request itself and says nothing, so it is dropped.
fn describe_entry(entry: &Value) -> Option<String> {
    let message = entry.get("msg")?.as_str()?;
    let location = match entry.get("loc").and_then(Value::as_array) {
        Some(parts) => parts
            .iter()
            .filter(|part| part.as_str() != Some("body"))
            .map(render_part)
            .collect::<Vec<_>>()
            .join("."),
        None => String::new(),
    };
    if location.is_empty() {
        Some(message.to_string())
    } else {
        Some(format!("{location}: {message}"))
    }
}

/// One element of a location, as text without JSON quotes.
fn render_part(part: &Value) -> String {
    match part.as_str() {
        Some(text) => text.to_string(),
        None => part.to_string(),
    }
}

/// `text` cut to 200 characters, with an ellipsis when it was cut.
fn truncate(text: &str) -> String {
    let mut out: String = text.chars().take(MAX_BODY_IN_MESSAGE).collect();
    if text.chars().nth(MAX_BODY_IN_MESSAGE).is_some() {
        out.push('…');
    }
    out
}
