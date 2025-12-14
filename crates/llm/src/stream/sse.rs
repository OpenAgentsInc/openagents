//! Server-Sent Events (SSE) stream parsing.
//!
//! This module provides utilities for parsing SSE streams from LLM providers.

use bytes::Bytes;
use eventsource_stream::{Event, EventStream, Eventsource};
use futures::{Stream, StreamExt};
use pin_project_lite::pin_project;
use std::pin::Pin;
use std::task::{Context, Poll};

pin_project! {
    /// A wrapper around an SSE event stream.
    pub struct SseStream<S> {
        #[pin]
        inner: EventStream<S>,
    }
}

impl<S> SseStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    /// Create a new SSE stream from a bytes stream.
    pub fn new(stream: S) -> Self {
        Self {
            inner: stream.eventsource(),
        }
    }
}

impl<S> Stream for SseStream<S>
where
    S: Stream<Item = Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<SseEvent, SseError>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let mut this = self.project();

        match this.inner.poll_next_unpin(cx) {
            Poll::Ready(Some(Ok(event))) => Poll::Ready(Some(Ok(SseEvent::from(event)))),
            Poll::Ready(Some(Err(e))) => Poll::Ready(Some(Err(SseError::Parse(e.to_string())))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// A parsed SSE event.
#[derive(Debug, Clone)]
pub struct SseEvent {
    /// Event type (e.g., "message", "content_block_delta").
    pub event: String,
    /// Event data (usually JSON).
    pub data: String,
    /// Event ID (optional).
    pub id: Option<String>,
}

impl From<Event> for SseEvent {
    fn from(event: Event) -> Self {
        Self {
            event: event.event,
            data: event.data,
            id: Some(event.id),
        }
    }
}

impl SseEvent {
    /// Check if this is a done event.
    pub fn is_done(&self) -> bool {
        self.data == "[DONE]" || self.event == "done" || self.event == "message_stop"
    }

    /// Parse the data as JSON.
    pub fn parse_json<T: serde::de::DeserializeOwned>(&self) -> Result<T, SseError> {
        serde_json::from_str(&self.data).map_err(|e| SseError::Json(e.to_string()))
    }
}

/// Errors that can occur during SSE parsing.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SseError {
    /// Failed to parse SSE stream.
    #[error("SSE parse error: {0}")]
    Parse(String),

    /// Failed to parse JSON data.
    #[error("JSON parse error: {0}")]
    Json(String),

    /// Connection error.
    #[error("Connection error: {0}")]
    Connection(String),
}
