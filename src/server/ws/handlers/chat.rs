use super::MessageHandler;
use crate::server::services::deepseek::{DeepSeekService, StreamUpdate};
use crate::server::tools::{Tool, ToolExecutorFactory};
use crate::server::ws::{transport::WebSocketState, types::ChatMessage};
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashSet;
use std::error::Error;
use std::sync::Arc;
use tokio::sync::RwLock;
use tracing::{error, info};

pub struct ChatHandler {
    ws_state: Arc<WebSocketState>,
    deepseek_service: Arc<DeepSeekService>,
    tool_factory: Arc<ToolExecutorFactory>,
    enabled_tools: Arc<RwLock<HashSet<String>>>,
}

impl ChatHandler {
    pub fn new(
        ws_state: Arc<WebSocketState>,
        deepseek_service: Arc<DeepSeekService>,
        tool_factory: Arc<ToolExecutorFactory>,
    ) -> Self {
        Self {
            ws_state,
            deepseek_service,
            tool_factory,
            enabled_tools: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    pub async fn enable_tool(&self, tool_name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tools = self.enabled_tools.write().await;
        tools.insert(tool_name.to_string());
        Ok(())
    }

    pub async fn disable_tool(&self, tool_name: &str) -> Result<(), Box<dyn Error + Send + Sync>> {
        let mut tools = self.enabled_tools.write().await;
        tools.remove(tool_name);
        Ok(())
    }

    async fn get_enabled_tools(&self) -> Vec<Tool> {
        let enabled = self.enabled_tools.read().await;
        self.tool_factory
            .get_all_tools()
            .into_iter()
            .filter(|tool| enabled.contains(&tool.function.name))
            .collect()
    }

    async fn process_message(
        &self,
        content: String,
        conn_id: &str,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Processing message: {}", content);

        // Get enabled tools
        let tools = self.get_enabled_tools().await;

        // Get streaming response from DeepSeek
        let mut stream = self.deepseek_service.chat_stream_with_tools(content, tools).await;

        // Send "typing" indicator
        let typing_json = json!({
            "type": "chat",
            "content": "...",
            "sender": "ai",
            "status": "typing"
        });
        self.ws_state
            .send_to(conn_id, &typing_json.to_string())
            .await?;

        // Accumulate the full response while sending stream updates
        let mut full_response = String::new();
        while let Some(update) = stream.recv().await {
            match update {
                StreamUpdate::Content(content) => {
                    full_response.push_str(&content);

                    // Send partial response
                    let response_json = json!({
                        "type": "chat",
                        "content": &content,
                        "sender": "ai",
                        "status": "streaming"
                    });
                    self.ws_state
                        .send_to(conn_id, &response_json.to_string())
                        .await?;
                }
                StreamUpdate::Reasoning(reasoning) => {
                    // Send reasoning update
                    let reasoning_json = json!({
                        "type": "chat",
                        "content": &reasoning,
                        "sender": "ai",
                        "status": "thinking"
                    });
                    self.ws_state
                        .send_to(conn_id, &reasoning_json.to_string())
                        .await?;
                }
                StreamUpdate::ToolCall { name, arguments } => {
                    // Send tool call indicator
                    let tool_json = json!({
                        "type": "chat",
                        "content": format!("Using tool: {}", name),
                        "sender": "system",
                        "status": "tool"
                    });
                    self.ws_state
                        .send_to(conn_id, &tool_json.to_string())
                        .await?;

                    // Execute tool
                    match self.tool_factory.execute_tool(&name, arguments).await {
                        Ok(result) => {
                            // Send tool result
                            let result_json = json!({
                                "type": "chat",
                                "content": result,
                                "sender": "tool",
                                "status": "complete"
                            });
                            self.ws_state
                                .send_to(conn_id, &result_json.to_string())
                                .await?;
                        }
                        Err(e) => {
                            // Send tool error
                            let error_json = json!({
                                "type": "chat",
                                "content": format!("Tool error: {}", e),
                                "sender": "system",
                                "status": "error"
                            });
                            self.ws_state
                                .send_to(conn_id, &error_json.to_string())
                                .await?;
                        }
                    }
                }
                StreamUpdate::Done => {
                    // Send final complete message
                    let response_json = json!({
                        "type": "chat",
                        "content": full_response,
                        "sender": "ai",
                        "status": "complete"
                    });
                    self.ws_state
                        .send_to(conn_id, &response_json.to_string())
                        .await?;
                    break;
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl MessageHandler for ChatHandler {
    type Message = ChatMessage;

    async fn handle_message(
        &self,
        msg: Self::Message,
        conn_id: String,
    ) -> Result<(), Box<dyn Error + Send + Sync>> {
        info!("Handling chat message: {:?}", msg);
        match msg {
            ChatMessage::UserMessage { content } => {
                match self.process_message(content, &conn_id).await {
                    Ok(_) => Ok(()),
                    Err(e) => {
                        error!("Error processing message: {}", e);
                        let error_json = json!({
                            "type": "chat",
                            "content": format!("Error: {}", e),
                            "sender": "system",
                            "status": "error"
                        });
                        self.ws_state
                            .send_to(&conn_id, &error_json.to_string())
                            .await?;
                        Ok(())
                    }
                }
            }
            _ => {
                error!("Unhandled message type: {:?}", msg);
                Ok(())
            }
        }
    }

    async fn broadcast(&self, _msg: Self::Message) -> Result<(), Box<dyn Error + Send + Sync>> {
        // Broadcasting not implemented for chat
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mockall::mock;
    use mockall::predicate::*;

    mock! {
        DeepSeekService {
            fn chat_stream_with_tools(&self, prompt: String, tools: Vec<Tool>) -> Result<StreamUpdate>;
        }
    }

    mock! {
        ToolExecutorFactory {
            fn get_all_tools(&self) -> Vec<Tool>;
            fn execute_tool(&self, name: &str, args: Value) -> Result<String>;
        }
    }

    mock! {
        WebSocketState {
            fn send_to(&self, conn_id: &str, msg: &str) -> Result<()>;
        }
    }

    #[tokio::test]
    async fn test_enable_disable_tool() {
        let mock_ws = Arc::new(MockWebSocketState::new());
        let mock_deepseek = Arc::new(MockDeepSeekService::new());
        let mock_factory = Arc::new(MockToolExecutorFactory::new());
        
        let handler = ChatHandler::new(mock_ws, mock_deepseek, mock_factory);

        // Enable tool
        handler.enable_tool("test_tool").await.unwrap();
        let enabled = handler.enabled_tools.read().await;
        assert!(enabled.contains("test_tool"));

        // Disable tool
        handler.disable_tool("test_tool").await.unwrap();
        let enabled = handler.enabled_tools.read().await;
        assert!(!enabled.contains("test_tool"));
    }

    #[tokio::test]
    async fn test_process_message_with_tool() {
        let mut mock_ws = MockWebSocketState::new();
        let mut mock_deepseek = MockDeepSeekService::new();
        let mut mock_factory = MockToolExecutorFactory::new();

        // Setup mock responses
        mock_deepseek
            .expect_chat_stream_with_tools()
            .with(eq("test message"), always())
            .returning(|_, _| Ok(vec![
                StreamUpdate::ToolCall {
                    name: "test_tool".to_string(),
                    arguments: json!({"arg": "value"}),
                },
                StreamUpdate::Content("test response".to_string()),
                StreamUpdate::Done,
            ]));

        mock_factory
            .expect_execute_tool()
            .with(eq("test_tool"), eq(json!({"arg": "value"})))
            .returning(|_, _| Ok("tool result".to_string()));

        mock_ws
            .expect_send_to()
            .returning(|_, _| Ok(()));

        let handler = ChatHandler::new(
            Arc::new(mock_ws),
            Arc::new(mock_deepseek),
            Arc::new(mock_factory),
        );

        // Enable the tool
        handler.enable_tool("test_tool").await.unwrap();

        // Process message
        let result = handler.process_message("test message".to_string(), "test_conn").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_message() {
        let mut mock_ws = MockWebSocketState::new();
        let mut mock_deepseek = MockDeepSeekService::new();
        let mock_factory = MockToolExecutorFactory::new();

        mock_deepseek
            .expect_chat_stream_with_tools()
            .with(eq("test message"), always())
            .returning(|_, _| Ok(vec![
                StreamUpdate::Content("test response".to_string()),
                StreamUpdate::Done,
            ]));

        mock_ws
            .expect_send_to()
            .returning(|_, _| Ok(()));

        let handler = ChatHandler::new(
            Arc::new(mock_ws),
            Arc::new(mock_deepseek),
            Arc::new(mock_factory),
        );

        let msg = ChatMessage::UserMessage {
            content: "test message".to_string(),
        };

        let result = handler.handle_message(msg, "test_conn".to_string()).await;
        assert!(result.is_ok());
    }
}