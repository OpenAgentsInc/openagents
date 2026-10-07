use std::sync::Arc;

use psionic_runtime::{
    StarterPluginToolBridgeConfig, execute_starter_plugin_tool_call,
    project_apple_fm_tool_definition, starter_plugin_tool_bridge_projections,
};
use thiserror::Error;

use crate::{
    AppleFmGeneratedContent, AppleFmGenerationSchema, AppleFmStructuredValueError, AppleFmTool,
    AppleFmToolCallError, AppleFmToolDefinition,
};

#[derive(Clone, Debug)]
pub struct TassadarStarterPluginAppleFmTool {
    tool_name: String,
    definition: AppleFmToolDefinition,
    config: StarterPluginToolBridgeConfig,
}

impl TassadarStarterPluginAppleFmTool {
    pub fn new(
        tool_name: impl Into<String>,
        definition: AppleFmToolDefinition,
        config: StarterPluginToolBridgeConfig,
    ) -> Self {
        Self {
            tool_name: tool_name.into(),
            definition,
            config,
        }
    }
}

impl AppleFmTool for TassadarStarterPluginAppleFmTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let envelope = execute_starter_plugin_tool_call(
            self.tool_name.as_str(),
            arguments.content,
            &self.config,
        )
        .map_err(|error| AppleFmToolCallError::new(self.tool_name.as_str(), error.to_string()))?;
        envelope
            .rendered_output()
            .map_err(|error| AppleFmToolCallError::new(self.tool_name.as_str(), error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum TassadarStarterPluginAppleFmToolError {
    #[error(transparent)]
    Schema(#[from] AppleFmStructuredValueError),
}

pub fn tassadar_starter_plugin_apple_fm_tool_definitions()
-> Result<Vec<AppleFmToolDefinition>, TassadarStarterPluginAppleFmToolError> {
    starter_plugin_tool_bridge_projections()
        .into_iter()
        .map(|projection| {
            let apple_definition = project_apple_fm_tool_definition(&projection);
            Ok(AppleFmToolDefinition::new(
                apple_definition.name,
                apple_definition.description,
                AppleFmGenerationSchema::new(apple_definition.arguments_schema)?,
            ))
        })
        .collect()
}

pub fn tassadar_starter_plugin_apple_fm_tools(
    config: StarterPluginToolBridgeConfig,
) -> Result<Vec<Arc<dyn AppleFmTool>>, TassadarStarterPluginAppleFmToolError> {
    starter_plugin_tool_bridge_projections()
        .into_iter()
        .map(|projection| {
            let apple_definition = project_apple_fm_tool_definition(&projection);
            let definition = AppleFmToolDefinition::new(
                apple_definition.name.clone(),
                apple_definition.description,
                AppleFmGenerationSchema::new(apple_definition.arguments_schema)?,
            );
            Ok(Arc::new(TassadarStarterPluginAppleFmTool::new(
                projection.tool_name,
                definition,
                config.clone(),
            )) as Arc<dyn AppleFmTool>)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        TassadarStarterPluginAppleFmTool, tassadar_starter_plugin_apple_fm_tool_definitions,
        tassadar_starter_plugin_apple_fm_tools,
    };
    use crate::{AppleFmGeneratedContent, AppleFmTool};
    use psionic_runtime::{StarterPluginInvocationStatus, StarterPluginToolBridgeConfig};

    #[test]
    fn starter_plugin_apple_fm_projection_covers_all_bridge_tools() {
        let definitions =
            tassadar_starter_plugin_apple_fm_tool_definitions().expect("apple fm definitions");

        assert_eq!(definitions.len(), 4);
        assert_eq!(definitions[0].name, "plugin_text_url_extract");
    }

    #[test]
    fn starter_plugin_apple_fm_tool_returns_receipt_bound_output() {
        let tool = TassadarStarterPluginAppleFmTool::new(
            "plugin_text_url_extract",
            tassadar_starter_plugin_apple_fm_tool_definitions()
                .expect("definitions")
                .into_iter()
                .find(|definition| definition.name == "plugin_text_url_extract")
                .expect("url extract definition"),
            StarterPluginToolBridgeConfig::default(),
        );
        let output = tool
            .call(
                AppleFmGeneratedContent::from_json_str(
                    r#"{"text":"https://snapshot.example/article"}"#,
                )
                .expect("arguments"),
            )
            .expect("tool output");
        let envelope: psionic_runtime::StarterPluginProjectedToolResultEnvelope =
            serde_json::from_str(output.as_str()).expect("projected envelope");

        assert_eq!(envelope.plugin_id, "plugin.text.url_extract");
        assert_eq!(envelope.status, StarterPluginInvocationStatus::Success);
        assert!(!envelope.plugin_receipt.receipt_id.is_empty());
    }

    #[test]
    fn starter_plugin_apple_fm_tools_construct_trait_objects() {
        let tools =
            tassadar_starter_plugin_apple_fm_tools(StarterPluginToolBridgeConfig::default())
                .expect("apple fm tools");

        assert_eq!(tools.len(), 4);
    }
}
