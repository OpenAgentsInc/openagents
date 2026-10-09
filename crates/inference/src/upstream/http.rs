//! The HTTP side every adapter shares: one client shape, the POST that
//! opens a stream, error statuses as [`AttemptError`]s, and the response
//! body as server-sent-event frames with a bound on silence.

use std::pin::Pin;
use std::time::Duration;

use futures_util::{Stream, StreamExt};
use serde_json::Value;

use super::{AttemptError, ErrorClass};
use crate::sse::{Frame, SseDecoder};

/// Time to connect.
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);

/// Time from the request to the response headers. The router's
/// first-token deadline is usually shorter and wins.
pub const HEADERS_TIMEOUT: Duration = Duration::from_secs(60);

/// The longest silence between two chunks of a stream. A reasoning model
/// can think quietly for a while; past this, the stream is broken.
pub const QUIET_TIMEOUT: Duration = Duration::from_secs(120);

/// How much of an upstream error message an [`AttemptError`] keeps.
pub const MESSAGE_LIMIT: usize = 400;

/// A client with the connect timeout. Per-request timeouts are set by the
/// caller, because a stream may run for minutes.
#[must_use]
pub fn client(connect: Duration) -> reqwest::Client {
    reqwest::Client::builder()
        .connect_timeout(connect)
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

/// Frames read from a response body.
pub type Frames = Pin<Box<dyn Stream<Item = Result<Frame, AttemptError>> + Send>>;

/// Sends `request` and returns the body as SSE frames, or the upstream's
/// error as an [`AttemptError`] when the status is not 2xx.
///
/// `scrub` lists words to remove from an upstream error message (a host
/// name the caller must not see); keys are never in a message because
/// none of ours is ever in a body.
///
/// # Errors
///
/// The status class, a timeout, or a connection failure.
pub async fn open_stream(
    request: reqwest::RequestBuilder,
    scrub: &[&str],
) -> Result<Frames, AttemptError> {
    let response = tokio::time::timeout(HEADERS_TIMEOUT, request.send())
        .await
        .map_err(|_| AttemptError::new(ErrorClass::Timeout, "no response headers in time"))?
        .map_err(|error| transport(&error))?;
    let status = response.status().as_u16();
    if !(200..300).contains(&status) {
        return Err(status_error(response, scrub).await);
    }
    Ok(frames(response))
}

/// The error for a failed send.
#[must_use]
pub fn transport(error: &reqwest::Error) -> AttemptError {
    if error.is_timeout() {
        AttemptError::new(ErrorClass::Timeout, "the upstream timed out")
    } else {
        AttemptError::new(ErrorClass::Connection, "could not reach the upstream")
    }
}

/// Reads an error response into an [`AttemptError`].
pub async fn status_error(response: reqwest::Response, scrub: &[&str]) -> AttemptError {
    let status = response.status().as_u16();
    let retry_after = response
        .headers()
        .get("retry-after")
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.trim().parse::<u64>().ok());
    let body = tokio::time::timeout(Duration::from_secs(10), response.text())
        .await
        .ok()
        .and_then(Result::ok)
        .unwrap_or_default();
    AttemptError {
        class: ErrorClass::of_status(status),
        status: Some(status),
        retry_after,
        message: error_message(&body, scrub),
    }
}

/// The message in an upstream error body: `error.message`, `message`, or
/// the body's start, scrubbed and truncated.
#[must_use]
pub fn error_message(body: &str, scrub: &[&str]) -> String {
    let parsed: Option<Value> = serde_json::from_str(body).ok();
    let found = parsed.as_ref().and_then(|value| {
        // Google sends errors as a one-element list.
        let value = value
            .as_array()
            .and_then(|list| list.first())
            .unwrap_or(value);
        let error = value.get("error").unwrap_or(value);
        error
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| error.as_str())
            .map(str::to_owned)
    });
    let text = found.unwrap_or_else(|| body.trim().to_owned());
    clean(&text, scrub)
}

/// Removes `scrub` words and anything shaped like a bearer key, and caps
/// the length.
#[must_use]
pub fn clean(text: &str, scrub: &[&str]) -> String {
    let mut text = text.to_owned();
    for word in scrub {
        text = text.replace(word, "upstream");
    }
    let text: String = text
        .split_whitespace()
        .map(|word| {
            if looks_like_key(word) {
                "<redacted>"
            } else {
                word
            }
        })
        .collect::<Vec<_>>()
        .join(" ");
    match text.char_indices().nth(MESSAGE_LIMIT) {
        Some((end, _)) => format!("{}…", &text[..end]),
        None => text,
    }
}

fn looks_like_key(word: &str) -> bool {
    let word = word.trim_matches(|c: char| !c.is_ascii_alphanumeric());
    let prefixed = ["sk-", "sk_", "rk_", "AIza", "ya29.", "Bearer"]
        .iter()
        .any(|prefix| word.starts_with(prefix) && word.len() > 16);
    let long_token = word.len() >= 32
        && word
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.');
    prefixed || long_token
}

/// A response body as SSE frames. Each chunk must arrive within
/// [`QUIET_TIMEOUT`] of the last.
#[must_use]
pub fn frames(response: reqwest::Response) -> Frames {
    let body = response.bytes_stream();
    let state = (
        Box::pin(body),
        SseDecoder::new(),
        std::collections::VecDeque::new(),
        false,
    );
    Box::pin(futures_util::stream::unfold(
        state,
        |(mut body, mut decoder, mut pending, mut ended)| async move {
            loop {
                if let Some(frame) = pending.pop_front() {
                    return Some((Ok(frame), (body, decoder, pending, ended)));
                }
                if ended {
                    return None;
                }
                match tokio::time::timeout(QUIET_TIMEOUT, body.next()).await {
                    Err(_) => {
                        ended = true;
                        let error = AttemptError::new(ErrorClass::Timeout, "the stream went quiet");
                        return Some((Err(error), (body, decoder, pending, ended)));
                    }
                    Ok(Some(Ok(chunk))) => pending.extend(decoder.push(&chunk)),
                    Ok(Some(Err(error))) => {
                        ended = true;
                        let error = if error.is_timeout() {
                            AttemptError::new(ErrorClass::Timeout, "the stream timed out")
                        } else {
                            AttemptError::new(ErrorClass::Connection, "the stream broke")
                        };
                        return Some((Err(error), (body, decoder, pending, ended)));
                    }
                    Ok(None) => {
                        ended = true;
                        pending.extend(decoder.finish());
                    }
                }
            }
        },
    ))
}
