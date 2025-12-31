//! Thread API for the Codex Agent SDK.
//!
//! A Thread represents a conversation with the Codex agent. Each thread can
//! have multiple turns, with context preserved between them.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::events::{ThreadEvent, Usage};
use crate::items::{ThreadItem, ThreadItemDetails};
use crate::options::{CodexOptions, ThreadOptions, TurnOptions};
use crate::transport::{ProcessTransport, find_codex_executable};

/// User input variants.
#[derive(Debug, Clone)]
pub enum UserInput {
    /// Text input.
    Text { text: String },
    /// Local image file.
    LocalImage { path: PathBuf },
}

/// Flexible input type (string or structured).
#[derive(Debug, Clone)]
pub enum Input {
    /// Simple text input.
    Text(String),
    /// Structured input with multiple parts.
    Structured(Vec<UserInput>),
}

impl From<&str> for Input {
    fn from(s: &str) -> Self {
        Input::Text(s.to_string())
    }
}

impl From<String> for Input {
    fn from(s: String) -> Self {
        Input::Text(s)
    }
}

impl From<Vec<UserInput>> for Input {
    fn from(parts: Vec<UserInput>) -> Self {
        Input::Structured(parts)
    }
}

/// Result of a completed turn.
#[derive(Debug, Clone)]
pub struct Turn {
    /// All items completed during this turn.
    pub items: Vec<ThreadItem>,

    /// The final text response from the agent.
    pub final_response: String,

    /// Token usage for this turn.
    pub usage: Option<Usage>,
}

/// A streaming turn that yields events as they arrive.
pub struct StreamedTurn {
    transport: ProcessTransport,
    thread_id: Option<String>,
}

impl StreamedTurn {
    /// Get the thread ID (available after ThreadStarted event).
    pub fn thread_id(&self) -> Option<&str> {
        self.thread_id.as_deref()
    }
}

impl StreamedTurn {
    /// Get the next event from the stream.
    pub async fn next(&mut self) -> Option<Result<ThreadEvent>> {
        let event = self.transport.recv().await?;

        // Capture thread ID from ThreadStarted event
        if let Ok(ThreadEvent::ThreadStarted(ref started)) = event {
            self.thread_id = Some(started.thread_id.clone());
        }

        Some(event)
    }
}

/// A conversation thread with the Codex agent.
pub struct Thread {
    codex_options: CodexOptions,
    thread_options: ThreadOptions,
    id: Option<String>,
}

impl Thread {
    /// Create a new thread.
    pub(crate) fn new(
        codex_options: CodexOptions,
        thread_options: ThreadOptions,
        id: Option<String>,
    ) -> Self {
        Self {
            codex_options,
            thread_options,
            id,
        }
    }

    /// Get the thread ID (available after the first turn starts).
    pub fn id(&self) -> Option<&str> {
        self.id.as_deref()
    }

    /// Run a turn with the given input, returning the completed result.
    pub async fn run(&mut self, input: impl Into<Input>, options: TurnOptions) -> Result<Turn> {
        let mut streamed = self.run_streamed(input, options).await?;

        let mut items = Vec::new();
        let mut final_response = String::new();
        let mut usage = None;

        while let Some(event_result) = streamed.next().await {
            let event = event_result?;

            match &event {
                ThreadEvent::ThreadStarted(e) => {
                    self.id = Some(e.thread_id.clone());
                }
                ThreadEvent::ItemCompleted(e) => {
                    if let ThreadItemDetails::AgentMessage(msg) = &e.item.details {
                        final_response = msg.text.clone();
                    }
                    items.push(e.item.clone());
                }
                ThreadEvent::TurnCompleted(e) => {
                    usage = Some(e.usage.clone());
                }
                ThreadEvent::TurnFailed(e) => {
                    return Err(Error::TurnFailed(e.error.message.clone()));
                }
                ThreadEvent::Error(e) => {
                    return Err(Error::TurnFailed(e.message.clone()));
                }
                _ => {}
            }
        }

        Ok(Turn {
            items,
            final_response,
            usage,
        })
    }

    /// Run a turn with streaming events.
    pub async fn run_streamed(
        &mut self,
        input: impl Into<Input>,
        options: TurnOptions,
    ) -> Result<StreamedTurn> {
        let input = input.into();
        let (prompt, images) = normalize_input(input);

        let executable = self
            .codex_options
            .codex_path_override
            .clone()
            .map(Ok)
            .unwrap_or_else(find_codex_executable)?;

        let mut args = vec!["exec".to_string(), "--experimental-json".to_string()];

        // Add thread options
        if let Some(model) = &self.thread_options.model {
            args.push("--model".to_string());
            args.push(model.clone());
        }

        if let Some(sandbox) = &self.thread_options.sandbox_mode {
            args.push("--sandbox".to_string());
            args.push(sandbox.as_arg().to_string());
        }

        if self.thread_options.skip_git_repo_check {
            args.push("--skip-git-repo-check".to_string());
        }

        for dir in &self.thread_options.additional_directories {
            args.push("--add-dir".to_string());
            args.push(dir.display().to_string());
        }

        if let Some(effort) = &self.thread_options.model_reasoning_effort {
            args.push("--config".to_string());
            args.push(format!(
                "model_reasoning_effort=\"{}\"",
                effort.as_config_value()
            ));
        }

        if let Some(network) = self.thread_options.network_access_enabled {
            args.push("--config".to_string());
            args.push(format!(
                "sandbox_workspace_write.network_access={}",
                network
            ));
        }

        if let Some(web_search) = self.thread_options.web_search_enabled {
            args.push("--config".to_string());
            args.push(format!("features.web_search_request={}", web_search));
        }

        if let Some(approval) = &self.thread_options.approval_policy {
            args.push("--config".to_string());
            args.push(format!(
                "approval_policy=\"{}\"",
                approval.as_config_value()
            ));
        }

        // Add images
        for image in images {
            args.push("--image".to_string());
            args.push(image);
        }

        // Handle output schema
        let _schema_file = if let Some(schema) = &options.output_schema {
            let file = tempfile::NamedTempFile::new().map_err(Error::OutputSchemaFile)?;
            std::fs::write(file.path(), serde_json::to_string(schema)?)
                .map_err(Error::OutputSchemaFile)?;
            args.push("--output-schema".to_string());
            args.push(file.path().display().to_string());
            Some(file) // Keep file alive
        } else {
            None
        };

        // Handle resume
        if let Some(thread_id) = &self.id {
            args.push("resume".to_string());
            args.push(thread_id.clone());
        } else {
            // Add prompt as final argument
            args.push(prompt);
        }

        // Build environment
        let mut env = self.codex_options.env.clone().unwrap_or_default();
        if let Some(base_url) = &self.codex_options.base_url {
            env.insert("OPENAI_BASE_URL".to_string(), base_url.clone());
        }
        if let Some(api_key) = &self.codex_options.api_key {
            env.insert("CODEX_API_KEY".to_string(), api_key.clone());
        }

        let transport = ProcessTransport::spawn(
            executable,
            args,
            self.thread_options.working_directory.clone(),
            Some(env),
        )
        .await?;

        Ok(StreamedTurn {
            transport,
            thread_id: self.id.clone(),
        })
    }
}

fn normalize_input(input: Input) -> (String, Vec<String>) {
    match input {
        Input::Text(text) => (text, vec![]),
        Input::Structured(parts) => {
            let mut prompt_parts = Vec::new();
            let mut images = Vec::new();

            for part in parts {
                match part {
                    UserInput::Text { text } => prompt_parts.push(text),
                    UserInput::LocalImage { path } => {
                        images.push(path.display().to_string());
                    }
                }
            }

            (prompt_parts.join("\n\n"), images)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_from_str() {
        let input: Input = "hello".into();
        match input {
            Input::Text(s) => assert_eq!(s, "hello"),
            _ => panic!("Expected Text"),
        }
    }

    #[test]
    fn test_normalize_input_text() {
        let (prompt, images) = normalize_input(Input::Text("test".to_string()));
        assert_eq!(prompt, "test");
        assert!(images.is_empty());
    }

    #[test]
    fn test_normalize_input_structured() {
        let input = Input::Structured(vec![
            UserInput::Text {
                text: "Hello".to_string(),
            },
            UserInput::LocalImage {
                path: PathBuf::from("/tmp/img.png"),
            },
            UserInput::Text {
                text: "World".to_string(),
            },
        ]);

        let (prompt, images) = normalize_input(input);
        assert_eq!(prompt, "Hello\n\nWorld");
        assert_eq!(images, vec!["/tmp/img.png"]);
    }
}
