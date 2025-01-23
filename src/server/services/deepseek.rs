use anyhow::Result;
use futures::StreamExt;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::info;

#[derive(Debug, Clone)]
pub struct DeepSeekService {
    client: Client,
    api_key: String,
    base_url: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ChatMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallResponse>>,
}

#[derive(Debug, Serialize, Clone)]
pub struct AssistantMessage {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCallResponse>>,
}

impl From<AssistantMessage> for ChatMessage {
    fn from(msg: AssistantMessage) -> Self {
        ChatMessage {
            role: msg.role,
            content: msg.content,
            tool_call_id: msg.tool_call_id,
            tool_calls: msg.tool_calls,
        }
    }
}

#[derive(Debug, Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    temperature: f32,
    max_tokens: Option<i32>,
    tools: Option<Vec<Tool>>,        // Add this
    tool_choice: Option<ToolChoice>, // Add this
}

#[derive(Debug, Deserialize)]
struct ChatChoice {
    message: ChatResponseMessage,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ChatResponseMessage {
    content: String,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallResponse>>,
    role: String,
}

#[derive(Debug, Deserialize)]
struct ChatResponse {
    choices: Vec<ChatChoice>,
}

// Streaming response types
#[derive(Debug, Deserialize)]
struct StreamChoice {
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct StreamDelta {
    content: Option<String>,
    reasoning_content: Option<String>,
    tool_calls: Option<Vec<ToolCallResponse>>,
    role: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamResponse {
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Clone)]
pub enum StreamUpdate {
    Content(String),
    Reasoning(String),
    ToolCalls(Vec<ToolCallResponse>),
    Done,
}

// Tool/Function calling types
#[derive(Debug, Serialize, Clone)]
pub struct Tool {
    #[serde(rename = "type")]
    tool_type: String,
    function: FunctionDefinition,
}

#[derive(Debug, Serialize, Clone)]
pub struct FunctionDefinition {
    name: String,
    description: Option<String>,
    parameters: serde_json::Value, // Using Value for flexible JSON Schema
}

#[derive(Debug, Serialize)]
#[serde(untagged)]
pub enum ToolChoice {
    Auto(String), // "auto", "none", or "required"
    Function {
        #[serde(rename = "type")]
        tool_type: String,
        function: FunctionCall,
    },
}

#[derive(Debug, Serialize)]
pub struct FunctionCall {
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ToolCallResponse {
    pub id: String,
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: FunctionCallResponse,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FunctionCallResponse {
    pub name: String,
    pub arguments: String,
}

impl DeepSeekService {
    pub fn new(api_key: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url: "https://api.deepseek.com".to_string(),
        }
    }

    pub fn with_base_url(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub fn create_tool(
        name: String,
        description: Option<String>,
        parameters: serde_json::Value,
    ) -> Tool {
        Tool {
            tool_type: "function".to_string(),
            function: FunctionDefinition {
                name,
                description,
                parameters,
            },
        }
    }

    pub async fn chat(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>)> {
        self.chat_internal(prompt, use_reasoner, false).await
    }

    pub async fn chat_with_tools(
        &self,
        prompt: String,
        tools: Vec<Tool>,
        tool_choice: Option<ToolChoice>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream: false,
            temperature: 0.7,
            max_tokens: None,
            tools: Some(tools),
            tool_choice,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        // Add status code check
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                text
            ));
        }

        // Get response text for debugging
        let text = response.text().await?;
        info!("API Response: {}", text);

        if text.is_empty() {
            return Err(anyhow::anyhow!("Empty response from API"));
        }

        // Parse the response
        let chat_response: ChatResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Failed to parse response: {}\nResponse text: {}", e, text)
        })?;

        if let Some(choice) = chat_response.choices.first() {
            Ok((
                choice.message.content.clone(),
                choice.message.reasoning_content.clone(),
                choice.message.tool_calls.clone(),
            ))
        } else {
            Err(anyhow::anyhow!("No response from model"))
        }
    }

    pub async fn chat_with_tool_response(
        &self,
        messages: Vec<ChatMessage>,
        tool_response: ChatMessage,
        tools: Vec<Tool>,
        use_reasoner: bool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        // Find the assistant message with tool calls
        let has_assistant_with_tools = messages
            .iter()
            .any(|msg| msg.role == "assistant" && msg.tool_calls.is_some());

        if !has_assistant_with_tools {
            return Err(anyhow::anyhow!(
                "No assistant message with tool calls found"
            ));
        }

        // Make sure tool response has tool_call_id
        if tool_response.tool_call_id.is_none() {
            return Err(anyhow::anyhow!("Tool response must have tool_call_id"));
        }

        // Create a new sequence of messages with the tool response
        let mut all_messages = messages;
        all_messages.push(tool_response);

        // Debug print the messages
        info!("Sending messages to API:");
        for (i, msg) in all_messages.iter().enumerate() {
            info!("Message {}: role={}, content={}", i, msg.role, msg.content);
            if let Some(tool_calls) = &msg.tool_calls {
                info!("  Tool calls: {:?}", tool_calls);
            }
            if let Some(tool_call_id) = &msg.tool_call_id {
                info!("  Tool call ID: {}", tool_call_id);
            }
        }

        let request = ChatRequest {
            model: model.to_string(),
            messages: all_messages,
            stream: false,
            temperature: 0.7,
            max_tokens: None,
            tools: Some(tools),
            tool_choice: Some(ToolChoice::Auto("none".to_string())),
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        // Add status code check
        if !response.status().is_success() {
            let status = response.status();
            let text = response.text().await?;
            return Err(anyhow::anyhow!(
                "API request failed with status {}: {}",
                status,
                text
            ));
        }

        // Get response text for debugging
        let text = response.text().await?;
        info!("API Response: {}", text);

        if text.is_empty() {
            return Err(anyhow::anyhow!("Empty response from API"));
        }

        // Parse the response
        let chat_response: ChatResponse = serde_json::from_str(&text).map_err(|e| {
            anyhow::anyhow!("Failed to parse response: {}\nResponse text: {}", e, text)
        })?;

        if let Some(choice) = chat_response.choices.first() {
            info!("Response content: {}", choice.message.content);
            Ok((
                choice.message.content.clone(),
                choice.message.reasoning_content.clone(),
                choice.message.tool_calls.clone(),
            ))
        } else {
            Err(anyhow::anyhow!("No response from model"))
        }
    }

    pub async fn chat_stream(
        &self,
        prompt: String,
        use_reasoner: bool,
    ) -> mpsc::Receiver<StreamUpdate> {
        let (tx, rx) = mpsc::channel(100);
        let client = self.client.clone();
        let api_key = self.api_key.clone();
        let base_url = self.base_url.clone();

        tokio::spawn(async move {
            let model = if use_reasoner {
                "deepseek-reasoner"
            } else {
                "deepseek-chat"
            };

            let messages = vec![ChatMessage {
                role: "user".to_string(),
                content: prompt,
                tool_call_id: None,
                tool_calls: None,
            }];

            let request = ChatRequest {
                model: model.to_string(),
                messages,
                stream: true,
                temperature: 0.7,
                max_tokens: None,
                tools: None,
                tool_choice: None,
            };

            let url = format!("{}/chat/completions", base_url);
            let response = client
                .post(&url)
                .header("Content-Type", "application/json")
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&request)
                .send()
                .await;

            match response {
                Ok(response) => {
                    let mut stream = response.bytes_stream();
                    let mut buffer = String::new();

                    while let Some(chunk) = stream.next().await {
                        match chunk {
                            Ok(chunk) => {
                                let chunk_str = String::from_utf8_lossy(&chunk);
                                buffer.push_str(&chunk_str);

                                // Process complete SSE messages
                                while let Some(pos) = buffer.find('\n') {
                                    // Extract the line and update buffer without borrowing issues
                                    let line = buffer[..pos].trim().to_string();
                                    buffer = buffer[pos + 1..].to_string();

                                    if let Some(data) = line.strip_prefix("data: ") {
                                        if data == "[DONE]" {
                                            let _ = tx.send(StreamUpdate::Done).await;
                                            break;
                                        }

                                        if let Ok(response) =
                                            serde_json::from_str::<StreamResponse>(data)
                                        {
                                            if let Some(choice) = response.choices.first() {
                                                if let Some(ref content) = choice.delta.content {
                                                    let _ = tx
                                                        .send(StreamUpdate::Content(
                                                            content.to_string(),
                                                        ))
                                                        .await;
                                                }
                                                if let Some(ref reasoning) =
                                                    choice.delta.reasoning_content
                                                {
                                                    let _ = tx
                                                        .send(StreamUpdate::Reasoning(
                                                            reasoning.to_string(),
                                                        ))
                                                        .await;
                                                }
                                                if let Some(tool_calls) = &choice.delta.tool_calls {
                                                    let _ = tx
                                                        .send(StreamUpdate::ToolCalls(
                                                            tool_calls.clone(),
                                                        ))
                                                        .await;
                                                }
                                                if choice.finish_reason.is_some() {
                                                    let _ = tx.send(StreamUpdate::Done).await;
                                                    break;
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                            Err(e) => {
                                info!("Stream error: {}", e);
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    info!("Request error: {}", e);
                }
            }
        });

        rx
    }

    async fn chat_internal(
        &self,
        prompt: String,
        use_reasoner: bool,
        stream: bool,
    ) -> Result<(String, Option<String>)> {
        info!("Making chat request to DeepSeek API");

        let model = if use_reasoner {
            "deepseek-reasoner"
        } else {
            "deepseek-chat"
        };

        let messages = vec![ChatMessage {
            role: "user".to_string(),
            content: prompt,
            tool_call_id: None,
            tool_calls: None,
        }];

        let request = ChatRequest {
            model: model.to_string(),
            messages,
            stream,
            temperature: 0.7,
            max_tokens: None,
            tools: None,
            tool_choice: None,
        };

        let url = format!("{}/chat/completions", self.base_url);
        let response = self
            .client
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key))
            .json(&request)
            .send()
            .await?;

        let chat_response: ChatResponse = response.json().await?;

        if let Some(choice) = chat_response.choices.first() {
            Ok((
                choice.message.content.clone(),
                choice.message.reasoning_content.clone(),
            ))
        } else {
            Err(anyhow::anyhow!("No response from model"))
        }
    }
}
