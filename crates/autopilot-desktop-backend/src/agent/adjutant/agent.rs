//! Adjutant Agent Implementation
//!
//! A native DSPy agent that implements the unified Agent trait with first-class
//! plan mode support through dsrs signatures.

use async_trait::async_trait;
use std::path::Path;
use tokio::sync::{mpsc, Mutex};
use std::sync::Arc;
use std::collections::HashMap;

use crate::agent::{
    trait_def::Agent,
    unified::{AgentId, UnifiedEvent, UnifiedConversationItem}
};
use super::planning::{PlanModePipeline, PlanResult};
use super::signatures::PlanModeConfig;

/// Adjutant Agent - DSPy-native agent with plan mode capabilities
pub struct AdjutantAgent {
    /// Events channel for streaming responses
    events_tx: mpsc::Sender<UnifiedEvent>,
    events_rx: Arc<Mutex<Option<mpsc::Receiver<UnifiedEvent>>>>,
    
    /// Active sessions and their conversation history
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,
    
    /// Plan mode pipeline
    plan_pipeline: Option<PlanModePipeline>,
    
    /// Configuration
    config: PlanModeConfig,
}

#[derive(Debug, Clone)]
struct SessionState {
    session_id: String,
    workspace_path: String,
    conversation_items: Vec<UnifiedConversationItem>,
    current_plan: Option<PlanResult>,
}

impl AdjutantAgent {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel(100);
        
        Self {
            events_tx,
            events_rx: Arc::new(Mutex::new(Some(events_rx))),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            plan_pipeline: None,
            config: PlanModeConfig::default(),
        }
    }

    pub fn with_config(mut self, config: PlanModeConfig) -> Self {
        self.config = config;
        self
    }

    /// Send a unified event
    async fn send_event(&self, event: UnifiedEvent) {
        if let Err(_) = self.events_tx.send(event).await {
            eprintln!("Failed to send event - receiver dropped");
        }
    }

    /// Process user message and generate response
    async fn process_message(&self, session_id: &str, message: String) -> Result<(), String> {
        // Send session started event
        self.send_event(UnifiedEvent::SessionStarted {
            session_id: session_id.to_string(),
            agent_id: AgentId::Adjutant,
        }).await;

        // Check if this is a plan mode request
        if self.is_plan_mode_request(&message) {
            self.handle_plan_mode_request(session_id, &message).await?;
        } else {
            self.handle_regular_request(session_id, &message).await?;
        }

        // Send session completed event
        self.send_event(UnifiedEvent::SessionCompleted {
            session_id: session_id.to_string(),
            stop_reason: "completed".to_string(),
        }).await;

        Ok(())
    }

    /// Check if the message should trigger plan mode
    fn is_plan_mode_request(&self, message: &str) -> bool {
        let plan_keywords = [
            "plan", "implement", "add", "create", "build", "design",
            "refactor", "migrate", "upgrade", "integrate"
        ];
        
        plan_keywords.iter().any(|keyword| {
            message.to_lowercase().contains(keyword)
        })
    }

    /// Handle plan mode request using DSPy signatures
    async fn handle_plan_mode_request(&self, session_id: &str, message: &str) -> Result<(), String> {
        // Get workspace path for this session
        let workspace_path = {
            let sessions = self.sessions.lock().await;
            sessions.get(session_id)
                .map(|s| s.workspace_path.clone())
                .ok_or("Session not found".to_string())?
        };

        // Send thinking chunk
        self.send_event(UnifiedEvent::ThoughtChunk {
            session_id: session_id.to_string(),
            content: "Analyzing request and initializing plan mode pipeline...".to_string(),
            is_complete: false,
        }).await;

        // Initialize plan pipeline if not already done
        if self.plan_pipeline.is_none() {
            let pipeline = PlanModePipeline::new(
                workspace_path.clone().into(), 
                self.config.clone()
            ).with_auto_lm().await;
            
            self.send_event(UnifiedEvent::ThoughtChunk {
                session_id: session_id.to_string(),
                content: "Plan mode pipeline initialized. Starting topic decomposition...".to_string(),
                is_complete: false,
            }).await;

            // Execute plan mode
            match pipeline.execute_plan_mode(message).await {
                Ok(plan_result) => {
                    // Send final thinking chunk
                    self.send_event(UnifiedEvent::ThoughtChunk {
                        session_id: session_id.to_string(),
                        content: format!(
                            "Plan completed. Explored {} topics, examined {} files.", 
                            plan_result.topics_explored.len(), 
                            plan_result.files_examined.len()
                        ),
                        is_complete: true,
                    }).await;

                    // Send the implementation plan as message chunks
                    self.stream_plan_response(session_id, &plan_result).await;

                    // Update session state
                    let mut sessions = self.sessions.lock().await;
                    if let Some(session) = sessions.get_mut(session_id) {
                        session.current_plan = Some(plan_result);
                        
                        // Add conversation items
                        session.conversation_items.push(UnifiedConversationItem::Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            role: "user".to_string(),
                            text: message.to_string(),
                        });
                        
                        session.conversation_items.push(UnifiedConversationItem::Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            role: "assistant".to_string(),
                            text: session.current_plan.as_ref().unwrap().implementation_plan.clone(),
                        });
                    }
                }
                Err(e) => {
                    self.send_event(UnifiedEvent::ThoughtChunk {
                        session_id: session_id.to_string(),
                        content: format!("Plan mode failed: {}", e),
                        is_complete: true,
                    }).await;

                    self.send_event(UnifiedEvent::MessageChunk {
                        session_id: session_id.to_string(),
                        content: format!("I encountered an error while creating your implementation plan: {}", e),
                        is_complete: true,
                    }).await;
                }
            }
        }

        Ok(())
    }

    /// Handle regular (non-plan mode) request
    async fn handle_regular_request(&self, session_id: &str, message: &str) -> Result<(), String> {
        // For now, provide a simple response explaining plan mode capabilities
        let response = format!(
            r#"I'm Adjutant, a DSPy-native agent specialized in implementation planning.

For best results, try requests like:
• "Plan implementation for user authentication"
• "Add real-time notifications to the app"  
• "Refactor the database layer"
• "Migrate from REST to GraphQL"

Your request: "{}"

Would you like me to create an implementation plan for this? I'll analyze your codebase, decompose the task into exploration topics, and generate a comprehensive plan with specific steps and file modifications."#,
            message
        );

        self.send_event(UnifiedEvent::MessageChunk {
            session_id: session_id.to_string(),
            content: response.clone(),
            is_complete: true,
        }).await;

        // Add to conversation items
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.get_mut(session_id) {
            session.conversation_items.push(UnifiedConversationItem::Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "user".to_string(),
                text: message.to_string(),
            });
            
            session.conversation_items.push(UnifiedConversationItem::Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: "assistant".to_string(),
                text: response,
            });
        }

        Ok(())
    }

    /// Stream plan response as message chunks
    async fn stream_plan_response(&self, session_id: &str, plan_result: &PlanResult) {
        let plan_text = &plan_result.implementation_plan;
        let chunk_size = 200; // Stream in chunks for better UX

        let text_as_string = plan_text
            .chars()
            .collect::<String>();
        let chunks: Vec<&str> = text_as_string
            .as_bytes()
            .chunks(chunk_size)
            .map(|chunk| std::str::from_utf8(chunk).unwrap_or(""))
            .collect();

        for (i, chunk) in chunks.iter().enumerate() {
            let is_complete = i == chunks.len() - 1;
            
            self.send_event(UnifiedEvent::MessageChunk {
                session_id: session_id.to_string(),
                content: chunk.to_string(),
                is_complete,
            }).await;

            // Small delay to simulate streaming
            if !is_complete {
                tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
            }
        }

        // Send token usage (mock data)
        self.send_event(UnifiedEvent::TokenUsage {
            session_id: session_id.to_string(),
            input_tokens: plan_text.len() as u64 / 4, // Rough estimate
            output_tokens: plan_text.len() as u64 / 4,
            total_tokens: plan_text.len() as u64 / 2,
        }).await;
    }
}

#[async_trait]
impl Agent for AdjutantAgent {
    fn agent_id(&self) -> AgentId {
        AgentId::Adjutant
    }

    async fn connect(&self, workspace_path: &Path) -> Result<String, String> {
        let session_id = uuid::Uuid::new_v4().to_string();
        
        // Initialize session state
        let session_state = SessionState {
            session_id: session_id.clone(),
            workspace_path: workspace_path.to_string_lossy().to_string(),
            conversation_items: Vec::new(),
            current_plan: None,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), session_state);

        println!("Adjutant agent connected to workspace: {}", workspace_path.display());

        Ok(session_id)
    }

    async fn disconnect(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);
        
        println!("Adjutant agent disconnected from session: {}", session_id);
        Ok(())
    }

    async fn start_session(&self, session_id: &str, _cwd: &Path) -> Result<(), String> {
        // Verify session exists
        let sessions = self.sessions.lock().await;
        if !sessions.contains_key(session_id) {
            return Err(format!("Session {} not found", session_id));
        }

        println!("Started new Adjutant session: {}", session_id);
        Ok(())
    }

    async fn send_message(&self, session_id: &str, text: String) -> Result<(), String> {
        // For the initial implementation, process synchronously
        // In a production version, we'd use proper async patterns
        self.process_message(session_id, text).await?;
        Ok(())
    }

    fn events_receiver(&self) -> mpsc::Receiver<UnifiedEvent> {
        // Return the receiver if available, otherwise create a new channel
        let mut rx_opt = futures::executor::block_on(self.events_rx.lock());
        match rx_opt.take() {
            Some(rx) => rx,
            None => {
                // Create a new channel if the receiver was already taken
                let (_, new_rx) = mpsc::channel(1);
                new_rx
            }
        }
    }

    async fn get_conversation_items(&self, session_id: &str) -> Result<Vec<UnifiedConversationItem>, String> {
        let sessions = self.sessions.lock().await;
        match sessions.get(session_id) {
            Some(session) => Ok(session.conversation_items.clone()),
            None => Err(format!("Session {} not found", session_id)),
        }
    }
}

impl Default for AdjutantAgent {
    fn default() -> Self {
        Self::new()
    }
}
