//! Message types for LLM conversations

use serde::{Deserialize, Serialize};

/// Role of a message sender
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System message (instructions)
    System,
    /// User message
    User,
    /// Assistant (model) message
    Assistant,
    /// Tool result message
    Tool,
}

impl Role {
    pub fn as_str(&self) -> &'static str {
        match self {
            Role::System => "system",
            Role::User => "user",
            Role::Assistant => "assistant",
            Role::Tool => "tool",
        }
    }
}

/// Content of a message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Content {
    /// Simple text content
    Text(String),
    /// Multi-part content (text + images, etc.)
    Parts(Vec<ContentPart>),
}

impl Content {
    /// Create text content
    pub fn text(s: impl Into<String>) -> Self {
        Content::Text(s.into())
    }

    /// Create multi-part content
    pub fn parts(parts: Vec<ContentPart>) -> Self {
        Content::Parts(parts)
    }

    /// Get the text content (concatenated if multi-part)
    pub fn as_text(&self) -> String {
        match self {
            Content::Text(s) => s.clone(),
            Content::Parts(parts) => parts
                .iter()
                .filter_map(|p| match p {
                    ContentPart::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }
}

/// A part of multi-part content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentPart {
    /// Text content
    Text { text: String },

    /// Image content
    Image {
        source: ImageSource,
    },

    /// Tool use (in assistant response)
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },

    /// Tool result (in user message)
    ToolResult {
        tool_use_id: String,
        content: String,
        #[serde(skip_serializing_if = "Option::is_none")]
        is_error: Option<bool>,
    },
}

impl ContentPart {
    /// Create a text part
    pub fn text(s: impl Into<String>) -> Self {
        ContentPart::Text { text: s.into() }
    }

    /// Create an image part from base64 data
    pub fn image_base64(media_type: impl Into<String>, data: impl Into<String>) -> Self {
        ContentPart::Image {
            source: ImageSource::Base64 {
                media_type: media_type.into(),
                data: data.into(),
            },
        }
    }

    /// Create an image part from URL
    pub fn image_url(url: impl Into<String>) -> Self {
        ContentPart::Image {
            source: ImageSource::Url { url: url.into() },
        }
    }

    /// Create a tool use part
    pub fn tool_use(
        id: impl Into<String>,
        name: impl Into<String>,
        input: serde_json::Value,
    ) -> Self {
        ContentPart::ToolUse {
            id: id.into(),
            name: name.into(),
            input,
        }
    }

    /// Create a tool result part
    pub fn tool_result(
        tool_use_id: impl Into<String>,
        content: impl Into<String>,
        is_error: bool,
    ) -> Self {
        ContentPart::ToolResult {
            tool_use_id: tool_use_id.into(),
            content: content.into(),
            is_error: if is_error { Some(true) } else { None },
        }
    }
}

/// Source for image content
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ImageSource {
    /// Base64-encoded image data
    Base64 {
        media_type: String,
        data: String,
    },
    /// URL to image
    Url {
        url: String,
    },
}

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role of the message sender
    pub role: Role,
    /// Content of the message
    pub content: Content,
    /// Optional name (for multi-agent scenarios)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl Message {
    /// Create a new message
    pub fn new(role: Role, content: impl Into<Content>) -> Self {
        Self {
            role,
            content: content.into(),
            name: None,
        }
    }

    /// Create a system message
    pub fn system(content: impl Into<String>) -> Self {
        Self::new(Role::System, Content::text(content))
    }

    /// Create a user message
    pub fn user(content: impl Into<String>) -> Self {
        Self::new(Role::User, Content::text(content))
    }

    /// Create an assistant message
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::new(Role::Assistant, Content::text(content))
    }

    /// Create a user message with multi-part content
    pub fn user_parts(parts: Vec<ContentPart>) -> Self {
        Self::new(Role::User, Content::parts(parts))
    }

    /// Create a tool result message
    pub fn tool_result(tool_use_id: impl Into<String>, result: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Content::Parts(vec![ContentPart::tool_result(tool_use_id, result, false)]),
            name: None,
        }
    }

    /// Create a tool error result message
    pub fn tool_error(tool_use_id: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: Content::Parts(vec![ContentPart::tool_result(tool_use_id, error, true)]),
            name: None,
        }
    }

    /// Set the name
    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    /// Get the text content
    pub fn text(&self) -> String {
        self.content.as_text()
    }
}

impl From<String> for Content {
    fn from(s: String) -> Self {
        Content::Text(s)
    }
}

impl From<&str> for Content {
    fn from(s: &str) -> Self {
        Content::Text(s.to_string())
    }
}

/// Stop reason for a response
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    /// Natural end of message
    EndTurn,
    /// Hit max tokens
    MaxTokens,
    /// Stop sequence matched
    StopSequence,
    /// Tool use requested
    ToolUse,
    /// Content filtered
    ContentFilter,
    /// Unknown reason
    #[serde(other)]
    Unknown,
}

/// Response from an LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Unique response ID
    pub id: String,
    /// Model that generated the response
    pub model: String,
    /// Generated content
    pub content: Vec<ContentPart>,
    /// Reason for stopping
    pub stop_reason: Option<StopReason>,
    /// Token usage
    pub usage: Usage,
}

impl ChatResponse {
    /// Get the text content of the response
    pub fn text(&self) -> String {
        self.content
            .iter()
            .filter_map(|p| match p {
                ContentPart::Text { text } => Some(text.clone()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("")
    }

    /// Get tool use calls from the response
    pub fn tool_uses(&self) -> Vec<&ContentPart> {
        self.content
            .iter()
            .filter(|p| matches!(p, ContentPart::ToolUse { .. }))
            .collect()
    }

    /// Check if the response contains tool uses
    pub fn has_tool_use(&self) -> bool {
        self.content
            .iter()
            .any(|p| matches!(p, ContentPart::ToolUse { .. }))
    }
}

/// Token usage information
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Usage {
    /// Input tokens (prompt)
    pub input_tokens: u32,
    /// Output tokens (completion)
    pub output_tokens: u32,
    /// Cache creation tokens (if caching enabled)
    #[serde(default)]
    pub cache_creation_input_tokens: u32,
    /// Cache read tokens (if caching enabled)
    #[serde(default)]
    pub cache_read_input_tokens: u32,
}

impl Usage {
    /// Total tokens used
    pub fn total_tokens(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}
