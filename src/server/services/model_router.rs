use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use super::deepseek::{
    ChatMessage, DeepSeekService, Tool, ToolCallResponse, ToolChoice,
};
use super::gemini::{service::GeminiService, StreamUpdate};

#[derive(Debug, Deserialize)]
pub struct RoutingDecision {
    pub needs_tool: bool,
    pub reasoning: String,
    pub suggested_tool: Option<String>,
}

pub struct ModelRouter {
    tool_model: Arc<DeepSeekService>,
    chat_model: Arc<DeepSeekService>,
    gemini: Option<Arc<GeminiService>>,
    available_tools: Vec<Tool>,
}

impl ModelRouter {
    pub fn new(
        tool_model: Arc<DeepSeekService>,
        chat_model: Arc<DeepSeekService>,
        available_tools: Vec<Tool>,
    ) -> Self {
        // Initialize Gemini service if possible
        let gemini = GeminiService::new().ok().map(Arc::new);
        
        Self {
            tool_model,
            chat_model,
            gemini,
            available_tools,
        }
    }

    pub async fn route_message(
        &self,
        message: String,
    ) -> Result<(RoutingDecision, Option<Vec<ToolCallResponse>>)> {
        // Create system prompt for routing
        let system_message = ChatMessage {
            role: "system".to_string(),
            content: r#"You are a routing assistant that determines whether a user message requires tool usage.
DO NOT USE ANY TOOLS DIRECTLY. Instead, analyze the user's message and respond with a JSON object containing:
1. "needs_tool": boolean - whether any tools are needed
2. "reasoning": string - brief explanation of your decision (use "requesting a calculation" for math queries)
3. "suggested_tool": string | null - name of suggested tool if applicable

Available tools:
- read_github_issue: Read GitHub issues by number
- calculate: Perform mathematical calculations
- analyze_files: Analyze repository files for changes (handled by Gemini)

IMPORTANT: Your response must be a valid JSON object and nothing else.

Example responses:
{
    "needs_tool": true,
    "reasoning": "User is requesting to view a GitHub issue",
    "suggested_tool": "read_github_issue"
}

{
    "needs_tool": false,
    "reasoning": "General chat message that doesn't require tools",
    "suggested_tool": null
}

Remember: Only respond with a JSON object, do not use any tools, and do not add any additional text."#.to_string(),
            tool_call_id: None,
            tool_calls: None,
        };

        // Create user message
        let user_message = ChatMessage {
            role: "user".to_string(),
            content: message.clone(),
            tool_call_id: None,
            tool_calls: None,
        };

        // Get routing decision
        let (response, _, _) = self
            .tool_model
            .chat_with_tools_messages(
                vec![system_message, user_message],
                self.available_tools.clone(),
                None, // Don't allow tool usage during routing
                false,
            )
            .await?;

        info!("Routing decision: {}", response);

        // Try to parse routing decision
        let decision = match serde_json::from_str::<RoutingDecision>(&response) {
            Ok(d) => d,
            Err(_) => {
                // If parsing fails, treat it as a non-tool message
                RoutingDecision {
                    needs_tool: false,
                    reasoning: "General chat message".to_string(),
                    suggested_tool: None,
                }
            }
        };

        // Special handling for file analysis
        if decision.needs_tool {
            if let Some(suggested_tool_name) = &decision.suggested_tool {
                if suggested_tool_name == "analyze_files" {
                    // File analysis will be handled by the chat handler directly
                    return Ok((decision, None));
                }
                
                // For other tools, use the normal flow
                let tool = self
                    .available_tools
                    .iter()
                    .find(|t| t.function.name == *suggested_tool_name);

                if let Some(tool) = tool {
                    // Try to use the tool
                    let (_, _, tool_calls) = self
                        .tool_model
                        .chat_with_tools(
                            message,
                            vec![tool.clone()],
                            Some(ToolChoice::Auto("auto".to_string())),
                            false,
                        )
                        .await?;

                    return Ok((decision, tool_calls));
                }
            }
        }

        Ok((decision, None))
    }

    pub async fn execute_tool_call(
        &self,
        message: String,
        tool: Tool,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        // Create a system message to guide tool usage
        let system_message = ChatMessage {
            role: "system".to_string(),
            content: format!(
                r#"You are a helpful assistant with access to the {tool_name} tool. 
Description: {tool_desc}

IMPORTANT:
1. ALWAYS provide a clear response explaining what you're doing
2. Use the tool when appropriate
3. Format the tool arguments carefully
4. Explain the results after tool usage

Example response format:
"I'll help you with that using the {tool_name} tool. [Use tool]
Here's what I found: [Explain results]""#,
                tool_name = tool.function.name,
                tool_desc = tool
                    .function
                    .description
                    .as_deref()
                    .unwrap_or("no description")
            ),
            tool_call_id: None,
            tool_calls: None,
        };

        let user_message = ChatMessage {
            role: "user".to_string(),
            content: message.clone(),
            tool_call_id: None,
            tool_calls: None,
        };

        self.tool_model
            .chat_with_tools_messages(
                vec![system_message, user_message],
                vec![tool],
                Some(ToolChoice::Auto("auto".to_string())),
                false,
            )
            .await
    }

    pub async fn chat(
        &self,
        message: String,
        use_reasoning: bool,
    ) -> Result<(String, Option<String>)> {
        // Use basic chat without any tools or messages
        self.chat_model.chat(message, use_reasoning).await
    }

    pub async fn chat_stream(&self, message: String) -> mpsc::Receiver<StreamUpdate> {
        // Try to use Gemini for streaming if available
        if let Some(gemini) = &self.gemini {
            let (tx, rx) = mpsc::channel(100);
            let mut stream = gemini.analyze_files_stream(&message, &vec![], "").await;
            
            tokio::spawn(async move {
                while let Some(update) = stream.recv().await {
                    let _ = tx.send(update).await;
                }
            });
            
            rx
        } else {
            // Convert DeepSeek stream to Gemini StreamUpdate format
            let (tx, rx) = mpsc::channel(100);
            let mut stream = self.chat_model.chat_stream(message, true).await;
            
            tokio::spawn(async move {
                while let Some(update) = stream.recv().await {
                    match update {
                        super::deepseek::StreamUpdate::Content(content) => {
                            let _ = tx.send(StreamUpdate::Content(content)).await;
                        }
                        super::deepseek::StreamUpdate::Done => {
                            let _ = tx.send(StreamUpdate::Done).await;
                            break;
                        }
                        _ => {}
                    }
                }
            });
            
            rx
        }
    }

    pub async fn handle_tool_response(
        &self,
        messages: Vec<ChatMessage>,
        tool_message: ChatMessage,
    ) -> Result<(String, Option<String>, Option<Vec<ToolCallResponse>>)> {
        let mut all_messages = messages;
        all_messages.push(tool_message);

        self.tool_model
            .chat_with_tools_messages(all_messages, self.available_tools.clone(), None, false)
            .await
    }
}