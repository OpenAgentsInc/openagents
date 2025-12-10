//! Tool definitions for function calling

use serde::{Deserialize, Serialize};

/// Definition of a tool that can be called by the LLM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDefinition {
    /// Tool name (must be unique)
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// JSON Schema for the tool's input parameters
    pub input_schema: serde_json::Value,
}

impl ToolDefinition {
    /// Create a new tool definition
    pub fn new(
        name: impl Into<String>,
        description: impl Into<String>,
        input_schema: serde_json::Value,
    ) -> Self {
        Self {
            name: name.into(),
            description: description.into(),
            input_schema,
        }
    }

    /// Create a tool with no parameters
    pub fn no_params(name: impl Into<String>, description: impl Into<String>) -> Self {
        Self::new(
            name,
            description,
            serde_json::json!({
                "type": "object",
                "properties": {},
                "required": []
            }),
        )
    }
}

/// Builder for creating tool input schemas
#[derive(Debug, Default)]
pub struct SchemaBuilder {
    properties: serde_json::Map<String, serde_json::Value>,
    required: Vec<String>,
}

impl SchemaBuilder {
    /// Create a new schema builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Add a string property
    pub fn string(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "string",
                "description": description
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add a string property with enum values
    pub fn string_enum(
        mut self,
        name: &str,
        description: &str,
        values: &[&str],
        required: bool,
    ) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "string",
                "description": description,
                "enum": values
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add an integer property
    pub fn integer(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "integer",
                "description": description
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add a number property
    pub fn number(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "number",
                "description": description
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add a boolean property
    pub fn boolean(mut self, name: &str, description: &str, required: bool) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "boolean",
                "description": description
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add an array property
    pub fn array(
        mut self,
        name: &str,
        description: &str,
        item_type: &str,
        required: bool,
    ) -> Self {
        self.properties.insert(
            name.to_string(),
            serde_json::json!({
                "type": "array",
                "description": description,
                "items": { "type": item_type }
            }),
        );
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Add an object property with nested schema
    pub fn object(
        mut self,
        name: &str,
        description: &str,
        nested_schema: serde_json::Value,
        required: bool,
    ) -> Self {
        let mut schema = serde_json::json!({
            "type": "object",
            "description": description
        });
        if let serde_json::Value::Object(ref mut obj) = schema {
            if let serde_json::Value::Object(nested) = nested_schema {
                obj.extend(nested);
            }
        }
        self.properties.insert(name.to_string(), schema);
        if required {
            self.required.push(name.to_string());
        }
        self
    }

    /// Build the JSON Schema
    pub fn build(self) -> serde_json::Value {
        serde_json::json!({
            "type": "object",
            "properties": self.properties,
            "required": self.required
        })
    }
}

/// Tool choice configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolChoice {
    /// Let the model decide whether to use tools
    Auto,
    /// Model should not use tools
    None,
    /// Model must use one of the provided tools
    Any,
    /// Model must use a specific tool
    Tool { name: String },
}

impl Default for ToolChoice {
    fn default() -> Self {
        ToolChoice::Auto
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_schema_builder() {
        let schema = SchemaBuilder::new()
            .string("name", "The user's name", true)
            .integer("age", "The user's age", false)
            .boolean("active", "Whether the user is active", true)
            .build();

        assert!(schema["properties"]["name"]["type"] == "string");
        assert!(schema["properties"]["age"]["type"] == "integer");
        assert!(schema["properties"]["active"]["type"] == "boolean");
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("name")));
        assert!(schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("active")));
        assert!(!schema["required"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("age")));
    }

    #[test]
    fn test_tool_definition() {
        let tool = ToolDefinition::new(
            "search",
            "Search for information",
            SchemaBuilder::new()
                .string("query", "Search query", true)
                .integer("limit", "Max results", false)
                .build(),
        );

        assert_eq!(tool.name, "search");
        assert!(tool.input_schema["properties"]["query"].is_object());
    }
}
