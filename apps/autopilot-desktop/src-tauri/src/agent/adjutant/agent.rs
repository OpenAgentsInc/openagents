//! Adjutant Agent Implementation
//!
//! A native DSPy agent that implements the unified Agent trait with first-class
//! plan mode support through dsrs signatures.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

use super::PlanModeConfig;
use super::config::PlanModeOptimizationConfig;
use super::plan_mode_optimizer::load_latest_instruction;
use super::plan_mode_signatures::PlanModeSignatureKind;
use super::planning::{PlanModePipeline, PlanResult};
use crate::agent::{
    trait_def::Agent,
    ui::UiTreeState,
    unified::{AgentId, UnifiedConversationItem, UnifiedEvent},
};
use crate::contracts::ipc::UiEvent;
use dsrs::core::MetaSignature;
use dsrs::signature_registry::signature_info;
use dsrs::signatures::{
    ParallelExplorationSignature, PlanSynthesisSignature, ResultValidationSignature,
    TopicDecompositionSignature,
};
use serde_json::{Value, json};

/// Adjutant Agent - DSPy-native agent with plan mode capabilities
pub struct AdjutantAgent {
    /// Events channel for streaming responses
    events_tx: mpsc::Sender<UnifiedEvent>,
    events_rx: Arc<Mutex<Option<mpsc::Receiver<UnifiedEvent>>>>,

    /// UI events channel for signature-driven UI updates
    ui_events_tx: mpsc::Sender<UiEvent>,
    ui_events_rx: Arc<Mutex<Option<mpsc::Receiver<UiEvent>>>>,

    /// Active sessions and their conversation history
    sessions: Arc<Mutex<HashMap<String, SessionState>>>,

    /// Configuration
    config: PlanModeConfig,
}

#[derive(Debug, Clone)]
struct SessionState {
    #[expect(dead_code)]
    session_id: String,
    workspace_path: String,
    conversation_items: Vec<UnifiedConversationItem>,
    current_plan: Option<PlanResult>,
    ui_tree: Option<UiTreeState>,
}

impl AdjutantAgent {
    pub fn new() -> Self {
        let (events_tx, events_rx) = mpsc::channel(100);
        let (ui_events_tx, ui_events_rx) = mpsc::channel(100);

        Self {
            events_tx,
            events_rx: Arc::new(Mutex::new(Some(events_rx))),
            ui_events_tx,
            ui_events_rx: Arc::new(Mutex::new(Some(ui_events_rx))),
            sessions: Arc::new(Mutex::new(HashMap::new())),
            config: PlanModeConfig::default(),
        }
    }

    #[expect(dead_code)]
    pub fn with_config(mut self, config: PlanModeConfig) -> Self {
        self.config = config;
        self
    }

    /// Send a unified event
    async fn send_event(&self, event: UnifiedEvent) {
        if let Err(_) = self.events_tx.send(event).await {
            tracing::warn!("Failed to send event - receiver dropped");
        }
    }

    /// Send a UI event
    async fn send_ui_event(&self, event: UiEvent) {
        if let Err(_) = self.ui_events_tx.send(event).await {
            tracing::warn!("Failed to send UI event - receiver dropped");
        }
    }

    async fn update_ui_data(&self, session_id: &str, path: &str, value: Value) {
        self.send_ui_event(UiEvent::UiDataUpdate {
            session_id: session_id.to_string(),
            path: path.to_string(),
            value,
        })
        .await;
    }

    /// Get UI events receiver (one-time).
    pub async fn ui_events_receiver(&self) -> mpsc::Receiver<UiEvent> {
        let mut rx_opt = self.ui_events_rx.lock().await;
        match rx_opt.take() {
            Some(rx) => rx,
            None => {
                let (_, new_rx) = mpsc::channel(1);
                new_rx
            }
        }
    }

    async fn initialize_ui_tree(&self, session_id: &str) -> Result<(), String> {
        let ui_event = {
            let mut sessions = self.sessions.lock().await;
            let session = sessions
                .get_mut(session_id)
                .ok_or_else(|| format!("Session {} not found", session_id))?;

            if session.ui_tree.is_some() {
                return Ok(());
            }

            let mut tree = UiTreeState::new("Autopilot", "Ready");

        let steps = [
            (
                "topic",
                signature_info_with_optimization(
                    TopicDecompositionSignature::new(),
                    PlanModeSignatureKind::TopicDecomposition,
                    &self.config.optimization,
                ),
            ),
            (
                "exploration",
                signature_info_with_optimization(
                    ParallelExplorationSignature::new(),
                    PlanModeSignatureKind::ParallelExploration,
                    &self.config.optimization,
                ),
            ),
            (
                "synthesis",
                signature_info_with_optimization(
                    PlanSynthesisSignature::new(),
                    PlanModeSignatureKind::PlanSynthesis,
                    &self.config.optimization,
                ),
            ),
            (
                "validation",
                signature_info_with_optimization(
                    ResultValidationSignature::new(),
                    PlanModeSignatureKind::ResultValidation,
                    &self.config.optimization,
                ),
            ),
        ];

            let mut new_elements: Vec<(String, Value)> = Vec::new();
            let mut child_keys: Vec<String> = Vec::new();

            for (step_id, info) in steps.iter() {
                let panel_key = format!("panel-{}", step_id);
                let detail_key = format!("detail-{}", step_id);
                let status_key = format!("status-{}", step_id);
                let inputs_label_key = format!("inputs-label-{}", step_id);
                let inputs_key = format!("inputs-{}", step_id);
                let outputs_label_key = format!("outputs-label-{}", step_id);
                let outputs_key = format!("outputs-{}", step_id);

                let display_name = short_signature_name(&info.name);
                let instruction = first_line(&info.instruction);
                let inputs = serde_json::to_string_pretty(&info.input_fields)
                    .unwrap_or_else(|_| "{}".to_string());
                let outputs = serde_json::to_string_pretty(&info.output_fields)
                    .unwrap_or_else(|_| "{}".to_string());

                new_elements.push((
                    panel_key.clone(),
                    json!({
                        "key": panel_key,
                        "type": "panel",
                        "props": {
                            "title": display_name,
                            "subtitle": "Signature"
                        },
                        "children": [
                            detail_key,
                            status_key,
                            inputs_label_key,
                            inputs_key,
                            outputs_label_key,
                            outputs_key
                        ],
                    }),
                ));

                new_elements.push((
                    detail_key.clone(),
                    json!({
                        "key": detail_key,
                        "type": "text",
                        "props": {
                            "text": instruction,
                            "tone": "muted",
                            "size": "sm"
                        }
                    }),
                ));

                new_elements.push((
                    status_key.clone(),
                    json!({
                        "key": status_key,
                        "type": "text",
                        "props": {
                            "text": "Status: pending",
                            "tone": "default",
                            "size": "sm"
                        }
                    }),
                ));

                new_elements.push((
                    inputs_label_key.clone(),
                    json!({
                        "key": inputs_label_key,
                        "type": "text",
                        "props": {
                            "text": "Inputs",
                            "tone": "muted",
                            "size": "xs"
                        }
                    }),
                ));

                new_elements.push((
                    inputs_key.clone(),
                    json!({
                        "key": inputs_key,
                        "type": "code_block",
                        "props": {
                            "code": inputs,
                            "language": "json"
                        }
                    }),
                ));

                new_elements.push((
                    outputs_label_key.clone(),
                    json!({
                        "key": outputs_label_key,
                        "type": "text",
                        "props": {
                            "text": "Outputs",
                            "tone": "muted",
                            "size": "xs"
                        }
                    }),
                ));

                new_elements.push((
                    outputs_key.clone(),
                    json!({
                        "key": outputs_key,
                        "type": "code_block",
                        "props": {
                            "code": outputs,
                            "language": "json"
                        }
                    }),
                ));

                child_keys.push(panel_key);
            }

            for (key, element) in new_elements {
                tree.elements.insert(key, element);
            }

            if let Some(stack) = tree.elements.get_mut(&tree.stack_key) {
                if let Some(children) = stack
                    .get_mut("children")
                    .and_then(|value| value.as_array_mut())
                {
                    children.clear();
                    children.extend(child_keys.into_iter().map(Value::String));
                }
            }

            let tree_value = tree.to_value();
            session.ui_tree = Some(tree);

            UiEvent::UiTreeReset {
                session_id: session_id.to_string(),
                tree: tree_value,
            }
        };

        self.send_ui_event(ui_event).await;

        Ok(())
    }

    async fn mark_ui_step(
        &self,
        session_id: &str,
        step_id: &str,
        status: &str,
        output: Option<&str>,
    ) {
        let mut events = Vec::new();

        {
            let mut sessions = self.sessions.lock().await;
            let session = match sessions.get_mut(session_id) {
                Some(session) => session,
                None => return,
            };

            let tree = match session.ui_tree.as_mut() {
                Some(tree) => tree,
                None => return,
            };

            let status_key = format!("status-{}", step_id);
            let panel_key = format!("panel-{}", step_id);

            if let Some(patch) = tree.set_element_prop(
                &status_key,
                "/props/text",
                json!(format!("Status: {}", status)),
            ) {
                events.push(UiEvent::UiPatch {
                    session_id: session_id.to_string(),
                    patch,
                });
            }

            if let Some(output_text) = output {
                let output_key = format!("output-{}", step_id);
                if !tree.elements.contains_key(&output_key) {
                    let element = json!({
                        "key": output_key,
                        "type": "code_block",
                        "props": {
                            "code": output_text,
                            "language": "text"
                        }
                    });
                    if let Some(patch) = tree.add_element(element) {
                        events.push(UiEvent::UiPatch {
                            session_id: session_id.to_string(),
                            patch,
                        });
                    }
                    if let Some(patch) = tree.append_child(&panel_key, output_key) {
                        events.push(UiEvent::UiPatch {
                            session_id: session_id.to_string(),
                            patch,
                        });
                    }
                } else if let Some(patch) =
                    tree.set_element_prop(&output_key, "/props/code", json!(output_text))
                {
                    events.push(UiEvent::UiPatch {
                        session_id: session_id.to_string(),
                        patch,
                    });
                }
            }
        }

        for event in events {
            self.send_ui_event(event).await;
        }
    }

    /// Process user message and generate response
    async fn process_message(&self, session_id: &str, message: String) -> Result<(), String> {
        // Send session started event
        self.send_event(UnifiedEvent::SessionStarted {
            session_id: session_id.to_string(),
            agent_id: AgentId::Adjutant,
        })
        .await;

        self.initialize_ui_tree(session_id).await?;
        self.update_ui_data(session_id, "/status/phase", json!("Running"))
            .await;

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
        })
        .await;

        Ok(())
    }

    /// Check if the message should trigger plan mode
    fn is_plan_mode_request(&self, message: &str) -> bool {
        let plan_keywords = [
            "plan",
            "implement",
            "add",
            "create",
            "build",
            "design",
            "refactor",
            "migrate",
            "upgrade",
            "integrate",
        ];

        plan_keywords
            .iter()
            .any(|keyword| message.to_lowercase().contains(keyword))
    }

    /// Handle plan mode request using DSPy signatures
    async fn handle_plan_mode_request(
        &self,
        session_id: &str,
        message: &str,
    ) -> Result<(), String> {
        // Get workspace path for this session
        let workspace_path = {
            let sessions = self.sessions.lock().await;
            sessions
                .get(session_id)
                .map(|s| s.workspace_path.clone())
                .ok_or("Session not found".to_string())?
        };

        // Send thinking chunk
        self.send_event(UnifiedEvent::ThoughtChunk {
            session_id: session_id.to_string(),
            content: "Analyzing request and initializing plan mode pipeline...".to_string(),
            is_complete: false,
        })
        .await;

        self.mark_ui_step(session_id, "topic", "running", None)
            .await;
        self.mark_ui_step(session_id, "exploration", "running", None)
            .await;

        let pipeline =
            PlanModePipeline::new(workspace_path.clone().into(), self.config.clone())
                .with_auto_lm()
                .await;

        self.send_event(UnifiedEvent::ThoughtChunk {
            session_id: session_id.to_string(),
            content: "Plan mode pipeline initialized. Starting topic decomposition..."
                .to_string(),
            is_complete: false,
        })
        .await;

        let message_text = message.to_string();
        let handle = tokio::runtime::Handle::current();
        // Run the pipeline on a blocking thread to keep this future Send.
        let plan_result = tokio::task::spawn_blocking(move || {
            handle.block_on(pipeline.execute_plan_mode(&message_text))
        })
        .await
        .map_err(|err| format!("Plan mode pipeline task failed: {}", err))?;

        match plan_result {
            Ok(plan_result) => {
                let topics_output = serde_json::to_string_pretty(&plan_result.topics_explored)
                    .unwrap_or_else(|_| "[]".to_string());
                let exploration_output =
                    serde_json::to_string_pretty(&plan_result.files_examined)
                        .unwrap_or_else(|_| "[]".to_string());

                self.mark_ui_step(session_id, "topic", "completed", Some(&topics_output))
                    .await;
                self.mark_ui_step(
                    session_id,
                    "exploration",
                    "completed",
                    Some(&exploration_output),
                )
                .await;
                self.mark_ui_step(
                    session_id,
                    "synthesis",
                    "completed",
                    Some(&plan_result.implementation_plan),
                )
                .await;
                self.mark_ui_step(
                    session_id,
                    "validation",
                    if self.config.enable_validation {
                        "completed"
                    } else {
                        "skipped"
                    },
                    None,
                )
                .await;

                // Send final thinking chunk
                self.send_event(UnifiedEvent::ThoughtChunk {
                    session_id: session_id.to_string(),
                    content: format!(
                        "Plan completed. Explored {} topics, examined {} files.",
                        plan_result.topics_explored.len(),
                        plan_result.files_examined.len()
                    ),
                    is_complete: true,
                })
                .await;

                // Send the implementation plan as message chunks
                self.stream_plan_response(session_id, &plan_result).await;
                self.update_ui_data(session_id, "/status/phase", json!("Complete"))
                    .await;

                // Update session state
                let mut sessions = self.sessions.lock().await;
                if let Some(session) = sessions.get_mut(session_id) {
                    session.current_plan = Some(plan_result);

                    // Add conversation items
                    session
                        .conversation_items
                        .push(UnifiedConversationItem::Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            role: "user".to_string(),
                            text: message.to_string(),
                        });

                    session
                        .conversation_items
                        .push(UnifiedConversationItem::Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            role: "assistant".to_string(),
                            text: session
                                .current_plan
                                .as_ref()
                                .unwrap()
                                .implementation_plan
                                .clone(),
                        });
                }
            }
            Err(e) => {
                self.mark_ui_step(
                    session_id,
                    "synthesis",
                    "error",
                    Some(&format!("Plan mode failed: {}", e)),
                )
                .await;
                self.update_ui_data(session_id, "/status/phase", json!("Error"))
                    .await;

                self.send_event(UnifiedEvent::ThoughtChunk {
                    session_id: session_id.to_string(),
                    content: format!("Plan mode failed: {}", e),
                    is_complete: true,
                })
                .await;

                self.send_event(UnifiedEvent::MessageChunk {
                    session_id: session_id.to_string(),
                    content: format!(
                        "I encountered an error while creating your implementation plan: {}",
                        e
                    ),
                    is_complete: true,
                })
                .await;
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
        })
        .await;

        self.mark_ui_step(
            session_id,
            "synthesis",
            "info",
            Some("Awaiting a plan-mode request to start signature execution."),
        )
        .await;

        // Add to conversation items
        {
            let mut sessions = self.sessions.lock().await;
            if let Some(session) = sessions.get_mut(session_id) {
                session
                    .conversation_items
                    .push(UnifiedConversationItem::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "user".to_string(),
                        text: message.to_string(),
                    });

                session
                    .conversation_items
                    .push(UnifiedConversationItem::Message {
                        id: uuid::Uuid::new_v4().to_string(),
                        role: "assistant".to_string(),
                        text: response,
                    });
            }
        }

        self.update_ui_data(session_id, "/status/phase", json!("Awaiting Plan"))
            .await;

        Ok(())
    }

    /// Stream plan response as message chunks
    async fn stream_plan_response(&self, session_id: &str, plan_result: &PlanResult) {
        let plan_text = &plan_result.implementation_plan;
        let chunk_size = 200; // Stream in chunks for better UX

        let text_as_string = plan_text.chars().collect::<String>();
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
            })
            .await;

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
        })
        .await;
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
            ui_tree: None,
        };

        let mut sessions = self.sessions.lock().await;
        sessions.insert(session_id.clone(), session_state);

        tracing::info!(
            workspace = %workspace_path.display(),
            "adjutant agent connected to workspace"
        );

        Ok(session_id)
    }

    async fn disconnect(&self, session_id: &str) -> Result<(), String> {
        let mut sessions = self.sessions.lock().await;
        sessions.remove(session_id);

        tracing::info!(
            session_id = %session_id,
            "adjutant agent disconnected from session"
        );
        Ok(())
    }

    async fn start_session(&self, session_id: &str, _cwd: &Path) -> Result<(), String> {
        // Verify session exists
        let sessions = self.sessions.lock().await;
        if !sessions.contains_key(session_id) {
            return Err(format!("Session {} not found", session_id));
        }

        tracing::info!(
            session_id = %session_id,
            "started new adjutant session"
        );
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
        let mut rx_opt = self.events_rx.blocking_lock();
        match rx_opt.take() {
            Some(rx) => rx,
            None => {
                // Create a new channel if the receiver was already taken
                let (_, new_rx) = mpsc::channel(1);
                new_rx
            }
        }
    }

    async fn get_conversation_items(
        &self,
        session_id: &str,
    ) -> Result<Vec<UnifiedConversationItem>, String> {
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

fn short_signature_name(name: &str) -> String {
    name.split("::").last().unwrap_or(name).to_string()
}

fn first_line(text: &str) -> String {
    text.lines().next().unwrap_or(text).trim().to_string()
}

fn signature_info_with_optimization<S: MetaSignature>(
    mut signature: S,
    kind: PlanModeSignatureKind,
    config: &PlanModeOptimizationConfig,
) -> dsrs::signature_registry::DsrsSignatureInfo {
    if config.apply_optimized_instructions {
        if let Some(instruction) = load_latest_instruction(kind) {
            if let Err(err) = signature.update_instruction(instruction) {
                tracing::warn!(
                    kind = %kind.name(),
                    error = %err,
                    "Failed to apply optimized instruction"
                );
            }
        }
    }

    signature_info(signature)
}
