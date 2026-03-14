use crate::{FleetRouter, RouteSelection, RoutingError, RoutingRequest};
use psionic_models::{PromptMessage, PromptMessageRole};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeMap, sync::Arc};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolHistoryVisibility {
    #[default]
    None,
    PromptHistory,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolResultVisibility {
    #[default]
    InjectIntoModel,
    ToolOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ToolProviderInterface {
    Native,
    Mcp {
        server_label: String,
        transport: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolProviderDescriptor {
    pub provider_id: String,
    pub interface: ToolProviderInterface,
    pub history_visibility: ToolHistoryVisibility,
    pub result_visibility: ToolResultVisibility,
}

impl ToolProviderDescriptor {
    #[must_use]
    pub fn native(provider_id: impl Into<String>) -> Self {
        Self {
            provider_id: provider_id.into(),
            interface: ToolProviderInterface::Native,
            history_visibility: ToolHistoryVisibility::None,
            result_visibility: ToolResultVisibility::InjectIntoModel,
        }
    }

    #[must_use]
    pub fn mcp(
        provider_id: impl Into<String>,
        server_label: impl Into<String>,
        transport: impl Into<String>,
    ) -> Self {
        Self {
            provider_id: provider_id.into(),
            interface: ToolProviderInterface::Mcp {
                server_label: server_label.into(),
                transport: transport.into(),
            },
            history_visibility: ToolHistoryVisibility::None,
            result_visibility: ToolResultVisibility::InjectIntoModel,
        }
    }

    #[must_use]
    pub fn with_history_visibility(mut self, visibility: ToolHistoryVisibility) -> Self {
        self.history_visibility = visibility;
        self
    }

    #[must_use]
    pub fn with_result_visibility(mut self, visibility: ToolResultVisibility) -> Self {
        self.result_visibility = visibility;
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLoopToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLoopToolResult {
    pub tool_call_id: String,
    pub tool_name: String,
    pub provider: ToolProviderDescriptor,
    pub visibility: ToolResultVisibility,
    pub message: PromptMessage,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub structured: Option<Value>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolExecutionRequest {
    pub step_index: usize,
    pub route_selection: RouteSelection,
    pub tool_call: ToolLoopToolCall,
    pub prompt_history: Vec<PromptMessage>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ToolLoopTurnRequest {
    pub step_index: usize,
    pub routing_request: RoutingRequest,
    pub route_selection: RouteSelection,
    pub prompt_history: Vec<PromptMessage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLoopModelTurn {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<PromptMessage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolLoopToolCall>,
}

impl ToolLoopModelTurn {
    #[must_use]
    pub fn message(message: PromptMessage) -> Self {
        Self {
            assistant_message: Some(message),
            tool_calls: Vec::new(),
        }
    }

    #[must_use]
    pub fn tool_call(tool_call: ToolLoopToolCall) -> Self {
        Self {
            assistant_message: None,
            tool_calls: vec![tool_call],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolLoopTerminationReason {
    Completed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolLoopStepReceipt {
    pub step_index: usize,
    pub route_selection: RouteSelection,
    pub prompt_history_len_before: usize,
    pub prompt_history_len_after: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub assistant_message: Option<PromptMessage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolLoopToolCall>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_results: Vec<ToolLoopToolResult>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ToolLoopOutcome {
    pub termination_reason: ToolLoopTerminationReason,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub final_message: Option<PromptMessage>,
    pub final_prompt_history: Vec<PromptMessage>,
    pub steps: Vec<ToolLoopStepReceipt>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolLoopPolicy {
    pub max_steps: usize,
}

impl Default for ToolLoopPolicy {
    fn default() -> Self {
        Self { max_steps: 4 }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolLoopRequest {
    pub routing_request: RoutingRequest,
    pub prompt_history: Vec<PromptMessage>,
    pub policy: ToolLoopPolicy,
}

impl ToolLoopRequest {
    #[must_use]
    pub fn new(routing_request: RoutingRequest, prompt_history: Vec<PromptMessage>) -> Self {
        Self {
            routing_request,
            prompt_history,
            policy: ToolLoopPolicy::default(),
        }
    }

    #[must_use]
    pub fn with_policy(mut self, policy: ToolLoopPolicy) -> Self {
        self.policy = policy;
        self
    }
}

#[derive(Debug, Error)]
pub enum ToolLoopError {
    #[error(transparent)]
    Routing(#[from] RoutingError),
    #[error("tool `{tool_name}` is not registered in the Psionic tool gateway")]
    UnknownTool { tool_name: String },
    #[error("tool loop exceeded the bounded step limit of {max_steps}")]
    MaxStepsExceeded { max_steps: usize },
    #[error("tool result for `{tool_name}` is hidden from model continuation")]
    HiddenToolResult { tool_name: String },
    #[error("tool result for `{tool_name}` must use the `tool` role")]
    InvalidToolResultRole { tool_name: String },
    #[error("tool-loop assistant messages must use the `assistant` role")]
    InvalidAssistantMessageRole,
    #[error("{0}")]
    Execution(String),
}

pub trait ToolLoopModelRunner {
    fn run_turn(
        &mut self,
        request: ToolLoopTurnRequest,
    ) -> Result<ToolLoopModelTurn, ToolLoopError>;
}

pub trait ToolLoopToolExecutor: Send + Sync {
    fn descriptor(&self) -> &ToolProviderDescriptor;
    fn execute(&self, request: ToolExecutionRequest) -> Result<ToolLoopToolResult, ToolLoopError>;
}

#[derive(Clone, Default)]
pub struct ToolGateway {
    executors: BTreeMap<String, Arc<dyn ToolLoopToolExecutor>>,
}

impl std::fmt::Debug for ToolGateway {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ToolGateway")
            .field("tool_names", &self.executors.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl ToolGateway {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(
        &mut self,
        tool_name: impl Into<String>,
        executor: impl ToolLoopToolExecutor + 'static,
    ) {
        let _ = self.executors.insert(tool_name.into(), Arc::new(executor));
    }

    #[must_use]
    pub fn descriptor(&self, tool_name: &str) -> Option<ToolProviderDescriptor> {
        self.executors
            .get(tool_name)
            .map(|executor| executor.descriptor().clone())
    }

    fn execute(&self, request: ToolExecutionRequest) -> Result<ToolLoopToolResult, ToolLoopError> {
        let Some(executor) = self.executors.get(request.tool_call.name.as_str()) else {
            return Err(ToolLoopError::UnknownTool {
                tool_name: request.tool_call.name,
            });
        };
        executor.execute(request)
    }
}

#[derive(Clone, Debug)]
pub struct ToolLoopController<'a> {
    router: &'a FleetRouter,
    gateway: &'a ToolGateway,
}

impl<'a> ToolLoopController<'a> {
    #[must_use]
    pub fn new(router: &'a FleetRouter, gateway: &'a ToolGateway) -> Self {
        Self { router, gateway }
    }

    pub fn run<M>(
        &self,
        request: ToolLoopRequest,
        model_runner: &mut M,
    ) -> Result<ToolLoopOutcome, ToolLoopError>
    where
        M: ToolLoopModelRunner,
    {
        let mut prompt_history = request.prompt_history;
        let mut steps = Vec::new();
        let mut preferred_worker_id = None::<String>;

        for step_index in 0..request.policy.max_steps {
            let mut routing_request = request.routing_request.clone();
            if let Some(worker_id) = preferred_worker_id.as_deref() {
                routing_request = routing_request.prefer_worker(worker_id.to_string());
            }
            let route_selection = self.router.resolve(&routing_request)?;
            let prompt_history_len_before = prompt_history.len();
            let turn = model_runner.run_turn(ToolLoopTurnRequest {
                step_index,
                routing_request,
                route_selection: route_selection.clone(),
                prompt_history: prompt_history.clone(),
            })?;
            if let Some(message) = turn.assistant_message.as_ref()
                && message.role != PromptMessageRole::Assistant
            {
                return Err(ToolLoopError::InvalidAssistantMessageRole);
            }

            let mut tool_results = Vec::new();
            if let Some(message) = turn.assistant_message.clone() {
                prompt_history.push(message);
            }

            if turn.tool_calls.is_empty() {
                let final_message = turn.assistant_message.clone();
                steps.push(ToolLoopStepReceipt {
                    step_index,
                    route_selection,
                    prompt_history_len_before,
                    prompt_history_len_after: prompt_history.len(),
                    assistant_message: turn.assistant_message,
                    tool_calls: Vec::new(),
                    tool_results,
                });
                return Ok(ToolLoopOutcome {
                    termination_reason: ToolLoopTerminationReason::Completed,
                    final_message,
                    final_prompt_history: prompt_history,
                    steps,
                });
            }

            for tool_call in &turn.tool_calls {
                prompt_history.push(tool_call_prompt_message(tool_call)?);
                let descriptor = self
                    .gateway
                    .descriptor(tool_call.name.as_str())
                    .ok_or_else(|| ToolLoopError::UnknownTool {
                        tool_name: tool_call.name.clone(),
                    })?;
                let visible_prompt_history = match descriptor.history_visibility {
                    ToolHistoryVisibility::None => Vec::new(),
                    ToolHistoryVisibility::PromptHistory => prompt_history.clone(),
                };
                let tool_result = self.gateway.execute(ToolExecutionRequest {
                    step_index,
                    route_selection: route_selection.clone(),
                    tool_call: tool_call.clone(),
                    prompt_history: visible_prompt_history,
                })?;
                if tool_result.message.role != PromptMessageRole::Tool {
                    return Err(ToolLoopError::InvalidToolResultRole {
                        tool_name: tool_result.tool_name,
                    });
                }
                if !matches!(
                    tool_result.visibility,
                    ToolResultVisibility::InjectIntoModel
                ) {
                    return Err(ToolLoopError::HiddenToolResult {
                        tool_name: tool_result.tool_name,
                    });
                }
                prompt_history.push(tool_result.message.clone());
                tool_results.push(tool_result);
            }

            preferred_worker_id = Some(route_selection.worker_id.clone());
            steps.push(ToolLoopStepReceipt {
                step_index,
                route_selection,
                prompt_history_len_before,
                prompt_history_len_after: prompt_history.len(),
                assistant_message: turn.assistant_message,
                tool_calls: turn.tool_calls,
                tool_results,
            });
        }

        Err(ToolLoopError::MaxStepsExceeded {
            max_steps: request.policy.max_steps,
        })
    }
}

fn tool_call_prompt_message(tool_call: &ToolLoopToolCall) -> Result<PromptMessage, ToolLoopError> {
    let content = serde_json::json!({
        "tool_call_id": tool_call.id,
        "name": tool_call.name,
        "arguments": tool_call.arguments,
    });
    let rendered = serde_json::to_string(&content)
        .map_err(|error| ToolLoopError::Execution(error.to_string()))?;
    Ok(PromptMessage::new(PromptMessageRole::Assistant, rendered).with_channel("tool_call"))
}

#[cfg(test)]
mod tests {
    use super::{
        ToolExecutionRequest, ToolGateway, ToolHistoryVisibility, ToolLoopController,
        ToolLoopError, ToolLoopModelRunner, ToolLoopModelTurn, ToolLoopRequest, ToolLoopToolCall,
        ToolLoopToolExecutor, ToolLoopToolResult, ToolProviderDescriptor, ToolResultVisibility,
    };
    use crate::{
        FleetRouter, RoutedModelInventory, RoutedWorkerInventory, RoutingEndpoint, RoutingRequest,
    };
    use psionic_models::{PromptMessage, PromptMessageRole};
    use psionic_runtime::ExecutionCapabilityProfile;
    use serde_json::json;
    use std::sync::Mutex;

    struct ScriptedModelRunner {
        turns: Vec<ToolLoopModelTurn>,
        observed_history_lens: Vec<usize>,
    }

    impl ToolLoopModelRunner for ScriptedModelRunner {
        fn run_turn(
            &mut self,
            request: super::ToolLoopTurnRequest,
        ) -> Result<ToolLoopModelTurn, ToolLoopError> {
            self.observed_history_lens
                .push(request.prompt_history.len());
            self.turns
                .get(request.step_index)
                .cloned()
                .ok_or_else(|| ToolLoopError::Execution(String::from("missing scripted turn")))
        }
    }

    struct RecordingExecutor {
        descriptor: ToolProviderDescriptor,
        observed_history_lens: Mutex<Vec<usize>>,
    }

    impl ToolLoopToolExecutor for RecordingExecutor {
        fn descriptor(&self) -> &ToolProviderDescriptor {
            &self.descriptor
        }

        fn execute(
            &self,
            request: ToolExecutionRequest,
        ) -> Result<ToolLoopToolResult, ToolLoopError> {
            self.observed_history_lens
                .lock()
                .expect("history mutex")
                .push(request.prompt_history.len());
            Ok(ToolLoopToolResult {
                tool_call_id: request.tool_call.id,
                tool_name: request.tool_call.name,
                provider: self.descriptor.clone(),
                visibility: self.descriptor.result_visibility,
                message: PromptMessage::new(PromptMessageRole::Tool, "72f and sunny")
                    .with_author_name("weather"),
                structured: Some(json!({"forecast": "sunny", "temperature_f": 72})),
            })
        }
    }

    #[test]
    fn tool_loop_executes_router_owned_multi_step_flow() -> Result<(), Box<dyn std::error::Error>> {
        let router = FleetRouter::new(
            "tiny-tool-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-tool-llama",
                        "tiny-tool-llama",
                        "llama",
                        ExecutionCapabilityProfile::single_request_latency_optimized(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::Responses)
                    .with_tool_calling()
                    .with_response_state(),
                ),
            ],
        )?;
        let mut gateway = ToolGateway::new();
        gateway.register(
            "get_weather",
            RecordingExecutor {
                descriptor: ToolProviderDescriptor::mcp("weather-provider", "weather", "sse")
                    .with_history_visibility(ToolHistoryVisibility::PromptHistory)
                    .with_result_visibility(ToolResultVisibility::InjectIntoModel),
                observed_history_lens: Mutex::new(Vec::new()),
            },
        );
        let controller = ToolLoopController::new(&router, &gateway);
        let mut model_runner = ScriptedModelRunner {
            turns: vec![
                ToolLoopModelTurn::tool_call(ToolLoopToolCall {
                    id: String::from("tool-0"),
                    name: String::from("get_weather"),
                    arguments: json!({"city": "Paris"}),
                }),
                ToolLoopModelTurn::message(PromptMessage::new(
                    PromptMessageRole::Assistant,
                    "It is 72F and sunny in Paris.",
                )),
            ],
            observed_history_lens: Vec::new(),
        };

        let outcome = controller.run(
            ToolLoopRequest::new(
                RoutingRequest::new(RoutingEndpoint::Responses).require_tool_calling(),
                vec![PromptMessage::new(
                    PromptMessageRole::User,
                    "How is the weather?",
                )],
            ),
            &mut model_runner,
        )?;

        assert_eq!(outcome.steps.len(), 2);
        assert_eq!(
            outcome
                .final_message
                .as_ref()
                .map(|message| message.content.as_str()),
            Some("It is 72F and sunny in Paris.")
        );
        assert_eq!(outcome.steps[0].tool_calls[0].name, "get_weather");
        assert!(matches!(
            outcome.steps[0].tool_results[0].provider.interface,
            super::ToolProviderInterface::Mcp { .. }
        ));
        assert_eq!(model_runner.observed_history_lens, vec![1, 3]);
        Ok(())
    }

    #[test]
    fn tool_loop_refuses_hidden_tool_results() -> Result<(), Box<dyn std::error::Error>> {
        let router = FleetRouter::new(
            "tiny-tool-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-tool-llama",
                        "tiny-tool-llama",
                        "llama",
                        ExecutionCapabilityProfile::single_request_latency_optimized(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::Responses)
                    .with_tool_calling(),
                ),
            ],
        )?;
        let mut gateway = ToolGateway::new();
        gateway.register(
            "get_weather",
            RecordingExecutor {
                descriptor: ToolProviderDescriptor::native("weather-provider")
                    .with_result_visibility(ToolResultVisibility::ToolOnly),
                observed_history_lens: Mutex::new(Vec::new()),
            },
        );
        let controller = ToolLoopController::new(&router, &gateway);
        let mut model_runner = ScriptedModelRunner {
            turns: vec![ToolLoopModelTurn::tool_call(ToolLoopToolCall {
                id: String::from("tool-0"),
                name: String::from("get_weather"),
                arguments: json!({"city": "Paris"}),
            })],
            observed_history_lens: Vec::new(),
        };

        let error = controller
            .run(
                ToolLoopRequest::new(
                    RoutingRequest::new(RoutingEndpoint::Responses).require_tool_calling(),
                    vec![PromptMessage::new(
                        PromptMessageRole::User,
                        "How is the weather?",
                    )],
                ),
                &mut model_runner,
            )
            .expect_err("tool-only result should refuse continuation");
        assert!(matches!(error, ToolLoopError::HiddenToolResult { .. }));
        Ok(())
    }
}
