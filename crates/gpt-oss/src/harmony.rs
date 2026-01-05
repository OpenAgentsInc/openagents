use std::env;
use std::path::PathBuf;

use openai_harmony::chat::{
    Content, Conversation, DeveloperContent, Message, ReasoningEffort, Role, SystemContent,
    TextContent, ToolDescription,
};
use openai_harmony::{HarmonyEncoding, HarmonyEncodingName, load_harmony_encoding};
use tracing::{info, warn};

use crate::error::{GptOssError, Result};
use crate::types::GptOssReasoningEffort;

#[derive(Debug, Clone)]
pub struct HarmonyTurn {
    pub role: Role,
    pub content: String,
    pub recipient: Option<String>,
    pub name: Option<String>,
    pub channel: Option<String>,
    pub content_type: Option<String>,
}

impl HarmonyTurn {
    pub fn new(role: Role, content: impl Into<String>) -> Self {
        Self {
            role,
            content: content.into(),
            recipient: None,
            name: None,
            channel: None,
            content_type: None,
        }
    }

    pub fn with_recipient(mut self, recipient: impl Into<String>) -> Self {
        self.recipient = Some(recipient.into());
        self
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_channel(mut self, channel: impl Into<String>) -> Self {
        self.channel = Some(channel.into());
        self
    }

    pub fn with_content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }
}

#[derive(Debug, Clone)]
pub struct HarmonyToolSpec {
    pub name: String,
    pub description: String,
    pub parameters: Option<serde_json::Value>,
}

impl HarmonyToolSpec {
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: Option<serde_json::Value>,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }
}

pub struct HarmonyRenderer {
    encoding: HarmonyEncoding,
}

#[derive(Debug, Clone, Default)]
pub struct HarmonyPromptConfig {
    pub reasoning_effort: Option<GptOssReasoningEffort>,
}

impl HarmonyPromptConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_reasoning_effort(mut self, effort: GptOssReasoningEffort) -> Self {
        self.reasoning_effort = Some(effort);
        self
    }
}

impl HarmonyRenderer {
    pub fn gpt_oss() -> Result<Self> {
        ensure_tiktoken_cache_dir();
        let encoding = load_harmony_encoding(HarmonyEncodingName::HarmonyGptOss)
            .map_err(|err| GptOssError::HarmonyError(err.to_string()))?;
        Ok(Self { encoding })
    }

    pub fn render_prompt(
        &self,
        turns: &[HarmonyTurn],
        tools: &[HarmonyToolSpec],
    ) -> Result<String> {
        self.render_prompt_with_config(turns, tools, None)
    }

    pub fn render_prompt_with_config(
        &self,
        turns: &[HarmonyTurn],
        tools: &[HarmonyToolSpec],
        config: Option<&HarmonyPromptConfig>,
    ) -> Result<String> {
        let mut messages = Vec::new();

        let mut system = SystemContent::new();
        if let Some(cfg) = config
            && let Some(effort) = cfg.reasoning_effort.clone()
        {
            system = system.with_reasoning_effort(map_reasoning_effort(effort));
        }

        messages.push(Message::from_role_and_content(Role::System, system));

        if !tools.is_empty() {
            let tool_defs = tools
                .iter()
                .map(|tool| {
                    ToolDescription::new(
                        tool.name.clone(),
                        tool.description.clone(),
                        tool.parameters.clone(),
                    )
                })
                .collect::<Vec<_>>();
            let developer = DeveloperContent::new().with_function_tools(tool_defs);
            messages.push(Message::from_role_and_content(Role::Developer, developer));
        }

        for turn in turns {
            let mut message =
                Message::from_role_and_content(turn.role.clone(), turn.content.clone());
            if let Some(name) = &turn.name {
                message.author.name = Some(name.clone());
            }
            if let Some(recipient) = &turn.recipient {
                message.recipient = Some(recipient.clone());
            }
            if let Some(channel) = &turn.channel {
                message.channel = Some(channel.clone());
            }
            if let Some(content_type) = &turn.content_type {
                message.content_type = Some(content_type.clone());
            }
            messages.push(message);
        }

        let convo = Conversation::from_messages(messages);
        let tokens = self
            .encoding
            .render_conversation_for_completion(&convo, Role::Assistant, None)
            .map_err(|err| GptOssError::HarmonyError(err.to_string()))?;

        let prompt = self
            .encoding
            .tokenizer()
            .decode_utf8(&tokens)
            .map_err(|err| GptOssError::HarmonyError(err.to_string()))?;

        Ok(prompt)
    }

    pub fn parse_completion(&self, completion: &str, role: Option<Role>) -> Result<Vec<Message>> {
        let tokens = self
            .encoding
            .tokenizer()
            .encode_with_special_tokens(completion);
        self.encoding
            .parse_messages_from_completion_tokens(tokens, role)
            .map_err(|err| GptOssError::HarmonyError(err.to_string()))
    }

    pub fn extract_assistant_text(&self, completion: &str) -> Result<String> {
        if let Ok(text) = self.extract_assistant_text_with_role(completion, Some(Role::Assistant))
        {
            return Ok(text);
        }

        self.extract_assistant_text_with_role(completion, None)
    }

    pub fn encoding(&self) -> &HarmonyEncoding {
        &self.encoding
    }
}

fn ensure_tiktoken_cache_dir() {
    if env::var_os("TIKTOKEN_RS_CACHE_DIR").is_some() {
        return;
    }

    let home = match env::var("HOME") {
        Ok(home) => PathBuf::from(home),
        Err(_) => return,
    };
    let cache_dir = home.join(".cache/tiktoken-rs");
    if let Err(err) = std::fs::create_dir_all(&cache_dir) {
        warn!(
            cache_dir = %cache_dir.display(),
            error = %err,
            "Failed to create tiktoken cache dir"
        );
        return;
    }
    // Safe here: we set the default cache dir during initialization before any worker threads.
    unsafe {
        env::set_var("TIKTOKEN_RS_CACHE_DIR", &cache_dir);
    }
    info!(
        cache_dir = %cache_dir.display(),
        "Defaulted TIKTOKEN_RS_CACHE_DIR"
    );
}

impl HarmonyRenderer {
    fn extract_assistant_text_with_role(
        &self,
        completion: &str,
        role: Option<Role>,
    ) -> Result<String> {
        let messages = self.parse_completion(completion, role)?;
        if let Some(text) = extract_assistant_text_from_messages(&messages) {
            return Ok(text);
        }

        Err(GptOssError::HarmonyError(
            "No assistant content found in completion".to_string(),
        ))
    }
}

fn extract_assistant_text_from_messages(messages: &[Message]) -> Option<String> {
    for message in messages.iter().rev() {
        if message.author.role == Role::Assistant && message.recipient.is_none() {
            let mut text = String::new();
            for content in &message.content {
                if let Content::Text(TextContent { text: chunk }) = content {
                    text.push_str(chunk);
                }
            }
            if !text.is_empty() {
                return Some(text);
            }
        }
    }
    None
}

pub use openai_harmony::chat::{
    Author as HarmonyAuthor, Content as HarmonyContent, Message as HarmonyMessage,
    Role as HarmonyRole, TextContent as HarmonyTextContent,
};

fn map_reasoning_effort(effort: GptOssReasoningEffort) -> ReasoningEffort {
    match effort {
        GptOssReasoningEffort::Low => ReasoningEffort::Low,
        GptOssReasoningEffort::Medium => ReasoningEffort::Medium,
        GptOssReasoningEffort::High => ReasoningEffort::High,
    }
}
