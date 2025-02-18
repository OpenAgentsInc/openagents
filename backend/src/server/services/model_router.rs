use anyhow::Result;
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::info;

use super::deepseek::{
    ChatMessage, DeepSeekService, StreamUpdate, Tool, ToolCallResponse, ToolChoice,
};

#[derive(Debug, Deserialize)]
pub struct RoutingDecision {
    pub needs_tool: bool,
    pub reasoning: String,
    pub suggested_tool: Option<String>,
}

pub struct ModelRouter {
    tool_model: Arc<DeepSeekService>,
    chat_model: Arc<DeepSeekService>,
    available_tools: Vec<Tool>,
}

impl ModelRouter {
    pub fn new(
        tool_model: Arc<DeepSeekService>,
        chat_model: Arc<DeepSeekService>,
        available_tools: Vec<Tool>,
    ) -> Self {
        Self {
            tool_model,
            chat_model,
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

        // If tools are needed, try to execute the suggested tool
        if decision.needs_tool {
            if let Some(suggested_tool_name) = &decision.suggested_tool {
                // Find the suggested tool in available tools
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
        self.chat_model.chat_stream(message, true).await
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
