use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum CommandInputError {
    #[error("thread id is required")]
    EmptyThreadId,
    #[error("Message text is required.")]
    EmptyMessageText,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SendThreadMessageCommand {
    pub thread_id: String,
    pub text: String,
}

impl SendThreadMessageCommand {
    pub fn try_new(thread_id: &str, text: &str) -> Result<Self, CommandInputError> {
        Ok(Self {
            thread_id: normalize_thread_id(thread_id)?,
            text: normalize_thread_message_text(text)?,
        })
    }
}

#[async_trait]
pub trait CommandTransport {
    type Error;

    async fn send_thread_message(
        &self,
        command: SendThreadMessageCommand,
    ) -> Result<(), Self::Error>;
}

pub fn normalize_thread_id(raw: &str) -> Result<String, CommandInputError> {
    let normalized = raw.trim().to_string();
    if normalized.is_empty() {
        return Err(CommandInputError::EmptyThreadId);
    }
    Ok(normalized)
}

pub fn normalize_thread_message_text(raw: &str) -> Result<String, CommandInputError> {
    let normalized = raw.trim().to_string();
    if normalized.is_empty() {
        return Err(CommandInputError::EmptyMessageText);
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_thread_message_text_rejects_empty_payload() {
        let error = normalize_thread_message_text("   ").expect_err("expected validation error");
        assert_eq!(error, CommandInputError::EmptyMessageText);
    }

    #[test]
    fn normalize_thread_message_text_trims_input() {
        let normalized =
            normalize_thread_message_text("  what codebase are you in  ").expect("valid input");
        assert_eq!(normalized, "what codebase are you in");
    }

    #[test]
    fn send_thread_message_command_normalizes_values() {
        let command =
            SendThreadMessageCommand::try_new("  thread-1 ", "  hi  ").expect("valid command");
        assert_eq!(command.thread_id, "thread-1");
        assert_eq!(command.text, "hi");
    }
}
