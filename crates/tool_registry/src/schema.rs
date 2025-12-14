//! JSON Schema utilities for tool input validation.

use schemars::JsonSchema;
use serde::de::DeserializeOwned;

/// Generate a JSON Schema for a type.
pub fn generate_schema<T: JsonSchema>() -> serde_json::Value {
    let schema = schemars::schema_for!(T);
    serde_json::to_value(schema).unwrap_or_else(|_| serde_json::json!({}))
}

/// Validate JSON input against a type's schema.
pub fn validate_input<T: DeserializeOwned>(input: &serde_json::Value) -> Result<T, String> {
    serde_json::from_value(input.clone())
        .map_err(|e| format!("Invalid input: {}", e))
}

/// Convert a tool's input schema to Anthropic's tool format.
pub fn to_anthropic_tool_schema(
    name: &str,
    description: &str,
    input_schema: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "name": name,
        "description": description,
        "input_schema": input_schema
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Debug, Deserialize, JsonSchema)]
    struct TestInput {
        name: String,
        #[serde(default)]
        count: Option<i32>,
    }

    #[test]
    fn test_generate_schema() {
        let schema = generate_schema::<TestInput>();
        assert!(schema.is_object());
    }

    #[test]
    fn test_validate_input() {
        let valid = serde_json::json!({ "name": "test" });
        let result: Result<TestInput, _> = validate_input(&valid);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().name, "test");

        let invalid = serde_json::json!({ "count": 5 });
        let result: Result<TestInput, _> = validate_input(&invalid);
        assert!(result.is_err());
    }

    #[test]
    fn test_anthropic_format() {
        let schema = serde_json::json!({
            "type": "object",
            "properties": {
                "file_path": { "type": "string" }
            }
        });

        let tool = to_anthropic_tool_schema("read", "Read a file", schema);
        assert_eq!(tool["name"], "read");
        assert_eq!(tool["description"], "Read a file");
    }
}
