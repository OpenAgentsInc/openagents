//! The JSON calls Gmail and Calendar make with one access token, and why
//! one didn't work, in words for the person.

use std::fmt;

use serde_json::Value;

/// The most bytes one Gmail or Calendar answer may be.
const MAX_ANSWER_BYTES: usize = 8 * 1024 * 1024;

/// Why a Gmail or Calendar call didn't work.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiError {
    /// The id names nothing this connection can see.
    NotFound,
    /// Google refused the token (expired or access removed).
    Unauthorized,
    /// The connection lacks the access.
    Forbidden,
    /// Bad arguments.
    Invalid(String),
    /// Google couldn't be reached or answered with an error.
    Unreachable,
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => f.write_str("That wasn't found."),
            Self::Unauthorized => {
                f.write_str("Google no longer accepts this connection. Connect Google again.")
            }
            Self::Forbidden => f.write_str("Google didn't allow that with this connection."),
            Self::Invalid(why) => f.write_str(why),
            Self::Unreachable => f.write_str("Google couldn't be reached. Try again."),
        }
    }
}

/// One access token's calls.
#[derive(Clone)]
pub struct Rest {
    http: reqwest::Client,
    token: String,
}

impl fmt::Debug for Rest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Rest(token redacted)")
    }
}

impl Rest {
    #[must_use]
    pub fn new(http: reqwest::Client, token: String) -> Self {
        Self { http, token }
    }

    async fn send(&self, request: reqwest::RequestBuilder) -> Result<Value, ApiError> {
        let mut response = request
            .bearer_auth(&self.token)
            .send()
            .await
            .map_err(|_| ApiError::Unreachable)?;
        match response.status().as_u16() {
            200..=299 => {}
            400 => {
                return Err(ApiError::Invalid(
                    "Google didn't accept that request.".into(),
                ));
            }
            401 => return Err(ApiError::Unauthorized),
            403 => return Err(ApiError::Forbidden),
            404 | 410 => return Err(ApiError::NotFound),
            _ => return Err(ApiError::Unreachable),
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|_| ApiError::Unreachable)? {
            bytes.extend_from_slice(&chunk);
            if bytes.len() > MAX_ANSWER_BYTES {
                return Err(ApiError::Unreachable);
            }
        }
        if bytes.iter().all(u8::is_ascii_whitespace) {
            return Ok(Value::Null);
        }
        serde_json::from_slice(&bytes).map_err(|_| ApiError::Unreachable)
    }

    pub(crate) async fn get(&self, url: &str, query: &[(&str, &str)]) -> Result<Value, ApiError> {
        self.send(self.http.get(url).query(query)).await
    }

    pub(crate) async fn post(
        &self,
        url: &str,
        query: &[(&str, &str)],
        body: &Value,
    ) -> Result<Value, ApiError> {
        self.send(self.http.post(url).query(query).json(body)).await
    }

    pub(crate) async fn delete(&self, url: &str) -> Result<(), ApiError> {
        self.send(self.http.delete(url)).await.map(|_| ())
    }
}

/// Something an answer used, to link: a message, a thread, an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cite {
    pub title: String,
    pub link: String,
}

/// Cut `text` to `limit` characters; whether it was cut.
pub(crate) fn capped(text: &str, limit: usize) -> (String, bool) {
    match text.char_indices().nth(limit) {
        Some((at, _)) => (text[..at].to_owned(), true),
        None => (text.to_owned(), false),
    }
}

/// Whether `address` is one plain email address (no name, no line break).
#[must_use]
pub fn valid_address(address: &str) -> bool {
    let mut parts = address.split('@');
    let (Some(local), Some(domain), None) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    address.len() <= 254
        && !local.is_empty()
        && domain.contains('.')
        && !domain.starts_with('.')
        && !domain.ends_with('.')
        && !address.chars().any(|c| {
            c.is_whitespace() || c.is_control() || matches!(c, '<' | '>' | ',' | ';' | '"')
        })
}

/// Whether `id` looks like a Gmail or Calendar id.
#[must_use]
pub fn valid_id(id: &str) -> bool {
    (1..=256).contains(&id.len())
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b'@'))
}
