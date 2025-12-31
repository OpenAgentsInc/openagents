//! Session API for multi-turn conversations.
//!
//! This module provides unstable V2 session APIs matching the Node.js SDK.
//! These APIs allow creating persistent sessions for multi-turn conversations.
//!
//! # Warning
//! These APIs are marked unstable and may change in future releases.

use crate::error::{Error, Result};
use crate::options::QueryOptions;
use crate::permissions::AllowAllPermissions;
use crate::protocol::{SdkMessage, SdkResultMessage, SdkSystemMessage};
use crate::query::Query;
use futures::{Stream, StreamExt};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

/// A session for multi-turn conversations.
///
/// Sessions provide a higher-level API than raw queries, supporting
/// sequential message sending and receiving in a conversation.
///
/// # Warning
/// This API is unstable and may change in future releases.
pub struct Session {
    /// The underlying query.
    query: Query,
    /// Session ID (populated after first message).
    session_id: Option<String>,
}

impl Session {
    /// Create a new session from a query.
    pub(crate) fn new(query: Query) -> Self {
        Self {
            query,
            session_id: None,
        }
    }

    /// Get the session ID.
    ///
    /// Returns `None` until the first message is received from the CLI.
    pub fn session_id(&self) -> Option<&str> {
        self.session_id
            .as_deref()
            .or_else(|| self.query.session_id())
    }

    /// Send a user message to the session.
    ///
    /// # Arguments
    /// * `message` - The message content to send
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{unstable_v2_create_session, QueryOptions};
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let mut session = unstable_v2_create_session(QueryOptions::new()).await?;
    /// session.send("Hello, Claude!").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&mut self, message: impl Into<String>) -> Result<()> {
        self.query.send_message(message).await
    }

    /// Receive messages from the session as a stream.
    ///
    /// Returns an async iterator over messages from Claude.
    ///
    /// # Example
    /// ```rust,no_run
    /// # use claude_agent_sdk::{unstable_v2_create_session, QueryOptions, SdkMessage};
    /// # use futures::StreamExt;
    /// # async fn example() -> Result<(), claude_agent_sdk::Error> {
    /// let mut session = unstable_v2_create_session(QueryOptions::new()).await?;
    /// session.send("What is 2+2?").await?;
    ///
    /// while let Some(msg) = session.receive().next().await {
    ///     match msg? {
    ///         SdkMessage::Assistant(a) => println!("Claude: {:?}", a.message),
    ///         SdkMessage::Result(r) => println!("Done: {:?}", r),
    ///         _ => {}
    ///     }
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn receive(&mut self) -> impl Stream<Item = Result<SdkMessage>> + '_ {
        SessionReceiver { session: self }
    }

    /// Check if the session has completed.
    pub fn is_completed(&self) -> bool {
        self.query.is_completed()
    }

    /// Interrupt the current operation.
    pub async fn interrupt(&self) -> Result<()> {
        self.query.interrupt().await
    }

    /// Close the session.
    ///
    /// This will interrupt any ongoing operations and clean up resources.
    pub async fn close(self) -> Result<()> {
        // Interrupt to stop any ongoing operations
        let _ = self.query.interrupt().await;
        Ok(())
    }
}

/// Stream wrapper for receiving session messages.
struct SessionReceiver<'a> {
    session: &'a mut Session,
}

impl<'a> Stream for SessionReceiver<'a> {
    type Item = Result<SdkMessage>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let query = &mut self.session.query;

        match Pin::new(query).poll_next(cx) {
            Poll::Ready(Some(result)) => {
                // Extract session ID from init message
                if let Ok(ref msg) = result
                    && let SdkMessage::System(SdkSystemMessage::Init(init)) = msg
                {
                    self.session.session_id = Some(init.session_id.clone());
                }
                Poll::Ready(Some(result))
            }
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

/// Create a new session for multi-turn conversations.
///
/// # Warning
/// This API is unstable and may change in future releases.
///
/// # Arguments
/// * `options` - Configuration options for the session
///
/// # Example
/// ```rust,no_run
/// use claude_agent_sdk::{unstable_v2_create_session, QueryOptions, SdkMessage};
/// use futures::StreamExt;
///
/// # async fn example() -> Result<(), claude_agent_sdk::Error> {
/// let mut session = unstable_v2_create_session(QueryOptions::new()).await?;
///
/// // Send a message and receive responses
/// session.send("Hello!").await?;
/// while let Some(msg) = session.receive().next().await {
///     println!("{:?}", msg?);
/// }
/// # Ok(())
/// # }
/// ```
pub async fn unstable_v2_create_session(options: QueryOptions) -> Result<Session> {
    // Create a query with empty prompt - we'll send messages via send()
    let query = Query::new("", options, Some(Arc::new(AllowAllPermissions))).await?;
    Ok(Session::new(query))
}

/// Resume an existing session by ID.
///
/// # Warning
/// This API is unstable and may change in future releases.
///
/// # Arguments
/// * `session_id` - The ID of the session to resume
/// * `options` - Configuration options for the session
///
/// # Example
/// ```rust,no_run
/// use claude_agent_sdk::{unstable_v2_resume_session, QueryOptions};
///
/// # async fn example() -> Result<(), claude_agent_sdk::Error> {
/// let session = unstable_v2_resume_session(
///     "session-123".to_string(),
///     QueryOptions::new()
/// ).await?;
/// # Ok(())
/// # }
/// ```
pub async fn unstable_v2_resume_session(
    session_id: String,
    mut options: QueryOptions,
) -> Result<Session> {
    options.resume = Some(session_id);
    unstable_v2_create_session(options).await
}

/// Execute a one-shot prompt and return the result.
///
/// This is a convenience function that creates a session, sends a message,
/// and waits for the result.
///
/// # Warning
/// This API is unstable and may change in future releases.
///
/// # Arguments
/// * `message` - The prompt to send
/// * `options` - Configuration options
///
/// # Example
/// ```rust,no_run
/// use claude_agent_sdk::{unstable_v2_prompt, QueryOptions};
///
/// # async fn example() -> Result<(), claude_agent_sdk::Error> {
/// let result = unstable_v2_prompt("What is 2+2?", QueryOptions::new()).await?;
/// println!("Result: {:?}", result);
/// # Ok(())
/// # }
/// ```
pub async fn unstable_v2_prompt(
    message: impl Into<String>,
    options: QueryOptions,
) -> Result<SdkResultMessage> {
    let query = Query::new(message, options, Some(Arc::new(AllowAllPermissions))).await?;
    let mut session = Session::new(query);

    // Consume stream until we get a result
    while let Some(msg) = session.receive().next().await {
        match msg? {
            SdkMessage::Result(result) => return Ok(result),
            _ => continue,
        }
    }

    Err(Error::InvalidMessage(
        "Session ended without result".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_creation() {
        // Just verify types compile
        fn _test_types() {
            let _: fn(QueryOptions) -> _ = unstable_v2_create_session;
            let _: fn(String, QueryOptions) -> _ = unstable_v2_resume_session;
        }
    }
}
