use crate::error::*;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Client for tool management
pub struct ToolClient {
    base_url: String,
    http_client: Client,
}

impl ToolClient {
    pub fn new(base_url: impl Into<String>, http_client: Client) -> Self {
        Self {
            base_url: base_url.into(),
            http_client,
        }
    }

    /// Register tools for a session
    pub async fn register_tools(
        &self,
        session_id: &str,
        tools: Vec<ToolDefinition>,
    ) -> Result<RegisterToolsResponse> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        let request = RegisterToolsRequest { tools };

        let response = self
            .http_client
            .post(&url)
            .json(&request)
            .send()
            .await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<RegisterToolsResponse>().await?;
        Ok(result)
    }

    /// List tools for a session
    pub async fn list_tools(&self, session_id: &str) -> Result<ListToolsResponse> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        let response = self.http_client.get(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<ListToolsResponse>().await?;
        Ok(result)
    }

    /// Remove all tools from a session
    pub async fn remove_tools(&self, session_id: &str) -> Result<RemoveToolsResponse> {
        let url = format!("{}/v1/sessions/{}/tools", self.base_url, session_id);

        let response = self.http_client.delete(&url).send().await?;

        if response.status().as_u16() == 404 {
            return Err(FMError::InvalidResponse("Session not found".to_string()));
        }

        if !response.status().is_success() {
            let error_text = response.text().await?;
            return Err(FMError::RequestFailed(error_text));
        }

        let result = response.json::<RemoveToolsResponse>().await?;
        Ok(result)
    }
}

// Tool-related types

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: ToolParameters,
}

impl ToolDefinition {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        parameters: ToolParameters,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            parameters,
        }
    }

    /// Builder for creating tools
    pub fn builder(name: impl Into<String>) -> ToolBuilder {
        ToolBuilder::new(name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolParameters {
    #[serde(rename = "type")]
    pub type_: String,
    pub properties: HashMap<String, PropertyDefinition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required: Option<Vec<String>>,
}

impl ToolParameters {
    /// Create a new parameters schema
    pub fn new() -> Self {
        Self {
            type_: "object".to_string(),
            properties: HashMap::new(),
            required: None,
        }
    }

    /// Add a property
    pub fn add_property(mut self, name: impl Into<String>, property: PropertyDefinition) -> Self {
        self.properties.insert(name.into(), property);
        self
    }

    /// Set required properties
    pub fn required(mut self, required: Vec<String>) -> Self {
        self.required = Some(required);
        self
    }
}

impl Default for ToolParameters {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PropertyDefinition {
    #[serde(rename = "type")]
    pub type_: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none", rename = "enum")]
    pub enum_: Option<Vec<String>>,
}

impl PropertyDefinition {
    /// Create a string property
    pub fn string(description: impl Into<String>) -> Self {
        Self {
            type_: "string".to_string(),
            description: Some(description.into()),
            enum_: None,
        }
    }

    /// Create a number property
    pub fn number(description: impl Into<String>) -> Self {
        Self {
            type_: "number".to_string(),
            description: Some(description.into()),
            enum_: None,
        }
    }

    /// Create a boolean property
    pub fn boolean(description: impl Into<String>) -> Self {
        Self {
            type_: "boolean".to_string(),
            description: Some(description.into()),
            enum_: None,
        }
    }

    /// Create an enum property
    pub fn enum_string(description: impl Into<String>, values: Vec<String>) -> Self {
        Self {
            type_: "string".to_string(),
            description: Some(description.into()),
            enum_: Some(values),
        }
    }
}

// Builder for ToolDefinition

pub struct ToolBuilder {
    name: String,
    description: Option<String>,
    parameters: ToolParameters,
}

impl ToolBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            description: None,
            parameters: ToolParameters::new(),
        }
    }

    pub fn description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    pub fn add_parameter(
        mut self,
        name: impl Into<String>,
        property: PropertyDefinition,
    ) -> Self {
        self.parameters = self.parameters.add_property(name, property);
        self
    }

    pub fn required(mut self, required: Vec<String>) -> Self {
        self.parameters = self.parameters.required(required);
        self
    }

    pub fn build(self) -> ToolDefinition {
        ToolDefinition {
            name: self.name,
            description: self.description.unwrap_or_default(),
            parameters: self.parameters,
        }
    }
}

// Request/Response types

#[derive(Debug, Clone, Serialize, Deserialize)]
struct RegisterToolsRequest {
    tools: Vec<ToolDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegisterToolsResponse {
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListToolsResponse {
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub tools: Vec<ToolDefinition>,
    pub count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoveToolsResponse {
    #[serde(rename = "session_id")]
    pub session_id: String,
    pub removed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: FunctionCall,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub arguments: String,
}
