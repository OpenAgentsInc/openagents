//! Authentication and header utilities for HTTP clients.

use std::collections::HashMap;
use std::hash::BuildHasher;

use base64::Engine;

/// Create a Basic authentication header value.
///
/// Returns a string in the format `Basic <base64-encoded-credentials>`.
pub fn make_basic_auth_header(username: &str, password: &str) -> String {
    let credentials = format!("{username}:{password}");
    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials.as_bytes());
    format!("Basic {encoded}")
}

/// Add a Basic authentication header to the given headers map.
///
/// This mutates the headers map in place, inserting the `Authorization` header.
pub fn add_basic_auth_header<S: BuildHasher>(
    headers: &mut HashMap<String, String, S>,
    username: &str,
    password: &str,
) {
    let auth_value = make_basic_auth_header(username, password);
    headers.insert("Authorization".to_string(), auth_value);
}

/// Content types for HTTP requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContentType {
    /// `application/json`
    Json,
    /// `text/plain`
    TextPlain,
}

impl ContentType {
    /// Returns the MIME type string for this content type.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::TextPlain => "text/plain",
        }
    }
}

/// Add a Content-Type header to the given headers map.
///
/// This mutates the headers map in place, inserting the `Content-Type` header.
pub fn add_content_type_header<S: BuildHasher>(
    headers: &mut HashMap<String, String, S>,
    content_type: ContentType,
) {
    headers.insert(
        "Content-Type".to_string(),
        content_type.as_str().to_string(),
    );
}
