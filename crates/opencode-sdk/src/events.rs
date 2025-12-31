use crate::error::{Error, Result};
use crate::types::*;
use futures::Stream;
use reqwest_eventsource::{Event as SseEvent, EventSource};
use serde::{Deserialize, Serialize};
use std::pin::Pin;
use std::task::{Context, Poll};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Event {
    #[serde(rename = "session.created")]
    SessionCreated { info: Session },

    #[serde(rename = "session.updated")]
    SessionUpdated { info: Session },

    #[serde(rename = "session.deleted")]
    SessionDeleted { info: Session },

    #[serde(rename = "session.status")]
    SessionStatus {
        #[serde(rename = "sessionID")]
        session_id: String,
        status: SessionStatus,
    },

    #[serde(rename = "session.idle")]
    SessionIdle {
        #[serde(rename = "sessionID")]
        session_id: String,
    },

    #[serde(rename = "session.error")]
    SessionError {
        #[serde(rename = "sessionID")]
        session_id: Option<String>,
        error: Option<String>,
    },

    #[serde(rename = "message.updated")]
    MessageUpdated { info: Message },

    #[serde(rename = "message.removed")]
    MessageRemoved {
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
    },

    #[serde(rename = "message.part.updated")]
    MessagePartUpdated {
        part: serde_json::Value,
        delta: Option<String>,
    },

    #[serde(rename = "message.part.removed")]
    MessagePartRemoved {
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "messageID")]
        message_id: String,
        #[serde(rename = "partID")]
        part_id: String,
    },

    #[serde(rename = "permission.updated")]
    PermissionUpdated { permission: Permission },

    #[serde(rename = "permission.replied")]
    PermissionReplied {
        #[serde(rename = "sessionID")]
        session_id: String,
        #[serde(rename = "permissionID")]
        permission_id: String,
        response: String,
    },

    #[serde(rename = "file.edited")]
    FileEdited { file: serde_json::Value },

    #[serde(rename = "todo.updated")]
    TodoUpdated {
        #[serde(rename = "sessionID")]
        session_id: String,
        todos: Vec<Todo>,
    },

    #[serde(rename = "server.connected")]
    ServerConnected,

    #[serde(rename = "server.instance.disposed")]
    ServerDisposed { directory: String },

    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionStatus {
    Idle,
    Busy,
    Compacting,
}

pub struct EventStream {
    source: EventSource,
}

impl EventStream {
    pub async fn connect(url: &str) -> Result<Self> {
        let source = EventSource::get(url);
        Ok(Self { source })
    }

    pub async fn next_event(&mut self) -> Option<Result<Event>> {
        use futures::StreamExt;

        loop {
            match self.source.next().await? {
                Ok(SseEvent::Open) => continue,
                Ok(SseEvent::Message(msg)) => {
                    let event = serde_json::from_str(&msg.data).map_err(|e| Error::EventStream {
                        message: format!("Failed to parse event: {}", e),
                    });
                    return Some(event);
                }
                Err(e) => {
                    return Some(Err(Error::EventStream {
                        message: e.to_string(),
                    }));
                }
            }
        }
    }
}

impl Stream for EventStream {
    type Item = Result<Event>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        loop {
            match Pin::new(&mut self.source).poll_next(cx) {
                Poll::Pending => return Poll::Pending,
                Poll::Ready(None) => return Poll::Ready(None),
                Poll::Ready(Some(Ok(SseEvent::Open))) => continue,
                Poll::Ready(Some(Ok(SseEvent::Message(msg)))) => {
                    let event = serde_json::from_str(&msg.data).map_err(|e| Error::EventStream {
                        message: format!("Failed to parse event: {}", e),
                    });
                    return Poll::Ready(Some(event));
                }
                Poll::Ready(Some(Err(e))) => {
                    return Poll::Ready(Some(Err(Error::EventStream {
                        message: e.to_string(),
                    })));
                }
            }
        }
    }
}
