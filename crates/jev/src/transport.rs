//! Header assembly, log redaction, and the lenient body read.
//!
//! The headers a caller supplies go on first, so the ones the API reads to
//! identify the request and the key cannot be replaced by accident. That is the
//! order both official SDKs use.

use std::sync::LazyLock;

use reqwest::header::{
    ACCEPT, AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue, USER_AGENT,
};
use serde_json::Value;

use crate::Result;
use crate::config::ApiKey;
use crate::error::{Error, ResponseBody};
use crate::retry::RETRY_COUNT_HEADER;

/// The name this crate reports as. The official SDKs send `typesafe-sdk`, and
/// this crate is not one of them.
pub(crate) const SDK_NAME: &str = "jev-rust";

/// The header that names the SDK.
pub(crate) const SDK_HEADER: &str = "x-typesafe-sdk";

/// The header that names the runtime the SDK runs on.
pub(crate) const RUNTIME_HEADER: &str = "x-typesafe-runtime";

/// The content type every request and response uses.
const JSON: &str = "application/json";

/// How this crate reports itself, as `jev-rust/<crate version>`.
static AGENT: LazyLock<String> = LazyLock::new(|| format!("{SDK_NAME}/{}", crate::VERSION));

/// The runtime, as `rust/<version> (<os>; <arch>)`. The version is the crate's
/// minimum supported Rust version, because reading the compiler's own version
/// needs a build script and this crate has none.
static RUNTIME: LazyLock<String> = LazyLock::new(|| {
    format!(
        "rust/{} ({}; {})",
        env!("CARGO_PKG_RUST_VERSION"),
        std::env::consts::OS,
        std::env::consts::ARCH
    )
});

/// The headers one attempt sends.
///
/// `defaults` and `per_call` go on first, `per_call` last, and the headers
/// below replace whatever they set: a caller cannot send another key, ask for
/// another content type, or claim another SDK. The retry count is dropped here
/// and added by the attempt that needs it.
pub(crate) fn headers(
    defaults: &HeaderMap,
    per_call: &HeaderMap,
    key: &ApiKey,
    has_body: bool,
) -> Result<HeaderMap> {
    let mut merged = defaults.clone();
    for (name, value) in per_call {
        merged.insert(name.clone(), value.clone());
    }
    merged.remove(RETRY_COUNT_HEADER);
    let bearer = HeaderValue::from_str(&format!("Bearer {}", key.expose()))
        .map_err(|_| Error::Config("the API key holds a character a header cannot carry".into()))?;
    merged.insert(AUTHORIZATION, bearer);
    merged.insert(ACCEPT, HeaderValue::from_static(JSON));
    merged.insert(USER_AGENT, value(&AGENT)?);
    merged.insert(name(SDK_HEADER)?, value(&AGENT)?);
    merged.insert(name(RUNTIME_HEADER)?, value(&RUNTIME)?);
    if has_body {
        merged.insert(CONTENT_TYPE, HeaderValue::from_static(JSON));
    }
    Ok(merged)
}

/// One header name, which every caller here supplies as lowercase ASCII.
fn name(text: &str) -> Result<HeaderName> {
    HeaderName::from_bytes(text.as_bytes())
        .map_err(|_| Error::Config(format!("{text} is not a header name")))
}

/// One header value from text this crate built.
fn value(text: &str) -> Result<HeaderValue> {
    HeaderValue::from_str(text).map_err(|_| Error::Config(format!("{text} is not a header value")))
}

/// Headers as one line for a log, with every credential masked.
///
/// A credential keeps its scheme and the last four characters, so two keys read
/// apart in a log without either one reaching it. Both official SDKs mask the
/// same five headers.
pub(crate) fn redact(headers: &HeaderMap) -> String {
    const KEYS: &[&str] = &["authorization", "proxy-authorization", "x-api-key"];
    const OPAQUE: &[&str] = &["cookie", "set-cookie"];
    let mut rendered: Vec<String> = Vec::with_capacity(headers.len());
    for (name, value) in headers {
        let name = name.as_str();
        let text = value.to_str().unwrap_or("<not text>");
        let shown = if KEYS.contains(&name) {
            mask(text)
        } else if OPAQUE.contains(&name) {
            "***".to_string()
        } else {
            text.to_string()
        };
        rendered.push(format!("{name}: {shown}"));
    }
    rendered.join(", ")
}

/// One credential, as its scheme, three stars, and its last four characters.
fn mask(value: &str) -> String {
    let (scheme, secret) = match value.split_once(char::is_whitespace) {
        Some((scheme, secret)) => (format!("{scheme} "), secret.trim()),
        None => (String::new(), value),
    };
    let tail: String = if secret.chars().count() > 8 {
        secret.chars().skip(secret.chars().count() - 4).collect()
    } else {
        String::new()
    };
    format!("{scheme}***{tail}")
}

/// One response body, as JSON when it parses and as text when it does not. An
/// empty body is `None`.
///
/// A server or a proxy does not always send a content type, so the parse is
/// tried either way, as both official SDKs try it.
pub(crate) fn parse_body(bytes: &[u8]) -> Option<ResponseBody> {
    if bytes.is_empty() {
        return None;
    }
    match serde_json::from_slice::<Value>(bytes) {
        Ok(value) => Some(ResponseBody::Json(value)),
        Err(_) => Some(ResponseBody::Text(
            String::from_utf8_lossy(bytes).into_owned(),
        )),
    }
}
