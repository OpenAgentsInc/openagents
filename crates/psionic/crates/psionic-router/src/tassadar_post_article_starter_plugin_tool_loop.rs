use crate::{
    ToolExecutionRequest, ToolGateway, ToolHistoryVisibility, ToolLoopError, ToolLoopToolExecutor,
    ToolLoopToolResult, ToolProviderDescriptor, ToolResultVisibility,
};
use psionic_models::{PromptMessage, PromptMessageRole};
use psionic_runtime::{
    StarterPluginProjectedToolResultEnvelope, StarterPluginRouterToolDefinition,
    StarterPluginToolBridgeConfig, execute_starter_plugin_tool_call,
    project_router_tool_definition, starter_plugin_tool_bridge_projections,
};

pub const TASSADAR_STARTER_PLUGIN_TOOL_PROVIDER_ID: &str = "starter-plugin-runtime";

#[derive(Clone, Debug)]
pub struct TassadarStarterPluginToolLoopExecutor {
    tool_name: String,
    descriptor: ToolProviderDescriptor,
    config: StarterPluginToolBridgeConfig,
}

impl TassadarStarterPluginToolLoopExecutor {
    #[must_use]
    pub fn new(tool_name: impl Into<String>, config: StarterPluginToolBridgeConfig) -> Self {
        Self {
            tool_name: tool_name.into(),
            descriptor: ToolProviderDescriptor::native(TASSADAR_STARTER_PLUGIN_TOOL_PROVIDER_ID)
                .with_history_visibility(ToolHistoryVisibility::None)
                .with_result_visibility(ToolResultVisibility::InjectIntoModel),
            config,
        }
    }
}

impl ToolLoopToolExecutor for TassadarStarterPluginToolLoopExecutor {
    fn descriptor(&self) -> &ToolProviderDescriptor {
        &self.descriptor
    }

    fn execute(&self, request: ToolExecutionRequest) -> Result<ToolLoopToolResult, ToolLoopError> {
        let envelope = execute_starter_plugin_tool_call(
            self.tool_name.as_str(),
            request.tool_call.arguments,
            &self.config,
        )
        .map_err(|error| ToolLoopError::Execution(error.to_string()))?;
        projected_tool_result(
            request.tool_call.id,
            self.tool_name.as_str(),
            self.descriptor.clone(),
            envelope,
        )
    }
}

#[must_use]
pub fn tassadar_starter_plugin_router_tool_definitions() -> Vec<StarterPluginRouterToolDefinition> {
    starter_plugin_tool_bridge_projections()
        .into_iter()
        .map(|projection| project_router_tool_definition(&projection))
        .collect()
}

#[must_use]
pub fn tassadar_starter_plugin_tool_loop_gateway(
    config: StarterPluginToolBridgeConfig,
) -> ToolGateway {
    let mut gateway = ToolGateway::new();
    register_tassadar_starter_plugin_tool_loop_executors(&mut gateway, config);
    gateway
}

pub fn register_tassadar_starter_plugin_tool_loop_executors(
    gateway: &mut ToolGateway,
    config: StarterPluginToolBridgeConfig,
) {
    for projection in starter_plugin_tool_bridge_projections() {
        gateway.register(
            projection.tool_name.clone(),
            TassadarStarterPluginToolLoopExecutor::new(projection.tool_name, config.clone()),
        );
    }
}

fn projected_tool_result(
    tool_call_id: String,
    tool_name: &str,
    descriptor: ToolProviderDescriptor,
    envelope: StarterPluginProjectedToolResultEnvelope,
) -> Result<ToolLoopToolResult, ToolLoopError> {
    let structured = serde_json::to_value(&envelope)
        .map_err(|error| ToolLoopError::Execution(error.to_string()))?;
    let rendered = envelope
        .rendered_output()
        .map_err(|error| ToolLoopError::Execution(error.to_string()))?;
    Ok(ToolLoopToolResult {
        tool_call_id,
        tool_name: String::from(tool_name),
        provider: descriptor.clone(),
        visibility: descriptor.result_visibility,
        message: PromptMessage::new(PromptMessageRole::Tool, rendered).with_author_name(tool_name),
        structured: Some(structured),
    })
}

#[cfg(test)]
mod tests {
    use super::{
        TASSADAR_STARTER_PLUGIN_TOOL_PROVIDER_ID, TassadarStarterPluginToolLoopExecutor,
        tassadar_starter_plugin_router_tool_definitions, tassadar_starter_plugin_tool_loop_gateway,
    };
    use crate::{
        FleetRouter, RoutedModelInventory, RoutedWorkerInventory, RoutingEndpoint, RoutingRequest,
        ToolExecutionRequest, ToolLoopToolExecutor, ToolProviderInterface,
    };
    use psionic_runtime::{
        ExecutionCapabilityProfile, FetchTextConfig, FetchTextSnapshotResponse,
        FetchTextSnapshotResult, StarterPluginToolBridgeConfig,
    };
    use std::collections::BTreeMap;

    fn test_route_selection() -> crate::RouteSelection {
        let router = FleetRouter::new(
            "tiny-router-tool-llama",
            vec![
                RoutedWorkerInventory::new("worker-a", "cpu", "native", "psionic").with_model(
                    RoutedModelInventory::new(
                        "tiny-router-tool-llama",
                        "tiny-router-tool-llama",
                        "llama",
                        ExecutionCapabilityProfile::single_request_latency_optimized(),
                    )
                    .with_supported_endpoint(RoutingEndpoint::Responses)
                    .with_tool_calling(),
                ),
            ],
        )
        .expect("router");
        router
            .resolve(&RoutingRequest::new(RoutingEndpoint::Responses).require_tool_calling())
            .expect("route selection")
    }

    #[test]
    fn starter_plugin_tool_loop_gateway_projects_all_bridge_tools() {
        let gateway =
            tassadar_starter_plugin_tool_loop_gateway(StarterPluginToolBridgeConfig::default());
        let tool_names = tassadar_starter_plugin_router_tool_definitions()
            .into_iter()
            .map(|definition| definition.name)
            .collect::<Vec<_>>();

        assert_eq!(tool_names.len(), 4);
        for tool_name in tool_names {
            let descriptor = gateway.descriptor(tool_name.as_str()).expect("descriptor");
            assert_eq!(
                descriptor.provider_id,
                TASSADAR_STARTER_PLUGIN_TOOL_PROVIDER_ID
            );
            assert!(matches!(
                descriptor.interface,
                ToolProviderInterface::Native
            ));
        }
    }

    #[test]
    fn starter_plugin_tool_loop_executor_preserves_receipt_bound_structured_results() {
        let executor = TassadarStarterPluginToolLoopExecutor::new(
            "plugin_text_url_extract",
            StarterPluginToolBridgeConfig::default(),
        );
        let result = executor
            .execute(ToolExecutionRequest {
                step_index: 0,
                route_selection: test_route_selection(),
                tool_call: crate::ToolLoopToolCall {
                    id: String::from("tool-0"),
                    name: String::from("plugin_text_url_extract"),
                    arguments: serde_json::json!({
                        "text": "read https://snapshot.example/article and https://snapshot.example/feed.rss"
                    }),
                },
                prompt_history: Vec::new(),
            })
            .expect("tool result");

        let structured = result.structured.expect("structured envelope");
        assert_eq!(
            structured["tool_name"],
            serde_json::json!("plugin_text_url_extract")
        );
        assert_eq!(structured["status"], serde_json::json!("success"));
        assert_eq!(
            structured["plugin_receipt"]["plugin_id"],
            serde_json::json!("plugin.text.url_extract")
        );
        assert!(
            structured["plugin_receipt"]["receipt_id"]
                .as_str()
                .is_some_and(|value| !value.is_empty())
        );
        assert_eq!(
            result.message.author_name.as_deref(),
            Some("plugin_text_url_extract")
        );
    }

    #[test]
    fn starter_plugin_tool_loop_executor_preserves_typed_refusals() {
        let mut snapshots = BTreeMap::new();
        let _ = snapshots.insert(
            String::from("https://snapshot.example/binary"),
            FetchTextSnapshotResult::Success(FetchTextSnapshotResponse {
                final_url: String::from("https://snapshot.example/binary"),
                status_code: 200,
                content_type: String::from("image/png"),
                charset: None,
                body_bytes: vec![0x89, b'P', b'N', b'G'],
            }),
        );
        let executor = TassadarStarterPluginToolLoopExecutor::new(
            "plugin_http_fetch_text",
            StarterPluginToolBridgeConfig {
                fetch_text: FetchTextConfig::snapshot(snapshots),
                ..StarterPluginToolBridgeConfig::default()
            },
        );
        let result = executor
            .execute(ToolExecutionRequest {
                step_index: 1,
                route_selection: test_route_selection(),
                tool_call: crate::ToolLoopToolCall {
                    id: String::from("tool-1"),
                    name: String::from("plugin_http_fetch_text"),
                    arguments: serde_json::json!({
                        "url": "https://snapshot.example/binary"
                    }),
                },
                prompt_history: Vec::new(),
            })
            .expect("tool result");

        let structured = result.structured.expect("structured envelope");
        assert_eq!(structured["status"], serde_json::json!("refusal"));
        assert_eq!(
            structured["output_or_refusal_schema_id"],
            serde_json::json!("plugin.refusal.content_type_unsupported.v1")
        );
        assert_eq!(
            structured["plugin_receipt"]["refusal_class_id"],
            serde_json::json!("content_type_unsupported")
        );
        assert_eq!(
            result.message.author_name.as_deref(),
            Some("plugin_http_fetch_text")
        );
    }
}
