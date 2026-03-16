use std::sync::atomic::{AtomicU64, Ordering};

use schemars::{JsonSchema, schema_for};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

static NEXT_GENERATION_ID: AtomicU64 = AtomicU64::new(1);

/// Rust-native marker trait for typed Apple FM structured-generation targets.
pub trait AppleFmStructuredType: DeserializeOwned + JsonSchema {}

impl<T> AppleFmStructuredType for T where T: DeserializeOwned + JsonSchema {}

/// Opaque generation identifier for structured Apple FM content.
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct AppleFmGenerationId(pub String);

impl AppleFmGenerationId {
    /// Builds a new local generation identifier.
    #[must_use]
    pub fn new() -> Self {
        let next = NEXT_GENERATION_ID.fetch_add(1, Ordering::Relaxed);
        Self(format!("gen-{next}"))
    }
}

impl Default for AppleFmGenerationId {
    fn default() -> Self {
        Self::new()
    }
}

/// Reusable JSON-schema wrapper for Apple FM structured generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AppleFmGenerationSchema {
    schema: Value,
}

impl AppleFmGenerationSchema {
    /// Builds a validated schema wrapper from a raw JSON value.
    pub fn new(schema: Value) -> Result<Self, AppleFmStructuredValueError> {
        Self::with_title_hint(schema, None)
    }

    /// Builds a validated schema wrapper from a raw JSON value plus an
    /// optional title hint for Apple runtime normalization.
    pub fn with_title_hint(
        mut schema: Value,
        title_hint: Option<&str>,
    ) -> Result<Self, AppleFmStructuredValueError> {
        sanitize_generated_schema(&mut schema, title_hint);
        let schema = Self { schema };
        schema.validate()?;
        Ok(schema)
    }

    /// Parses a schema from serialized JSON.
    pub fn from_json_str(value: &str) -> Result<Self, AppleFmStructuredValueError> {
        let schema = serde_json::from_str(value).map_err(|error| {
            AppleFmStructuredValueError::SchemaDecode {
                error: error.to_string(),
            }
        })?;
        Self::new(schema)
    }

    /// Builds a schema from a Rust type derived with `schemars::JsonSchema`.
    pub fn from_type<T: JsonSchema>() -> Result<Self, AppleFmStructuredValueError> {
        let schema = serde_json::to_value(schema_for!(T)).map_err(|error| {
            AppleFmStructuredValueError::SchemaEncode {
                error: error.to_string(),
            }
        })?;
        Self::with_title_hint(schema, Some(T::schema_name().as_ref()))
    }

    /// Serializes the schema to a compact JSON string.
    pub fn to_json_string(&self) -> Result<String, AppleFmStructuredValueError> {
        self.validate()?;
        serde_json::to_string(&self.schema).map_err(|error| {
            AppleFmStructuredValueError::SchemaEncode {
                error: error.to_string(),
            }
        })
    }

    /// Returns the raw schema value.
    #[must_use]
    pub fn as_json_value(&self) -> &Value {
        &self.schema
    }

    /// Clones the raw schema value.
    #[must_use]
    pub fn clone_json_value(&self) -> Value {
        self.schema.clone()
    }

    /// Verifies the wrapped schema is an object-shaped JSON schema.
    pub fn validate(&self) -> Result<(), AppleFmStructuredValueError> {
        if !self.schema.is_object() {
            return Err(AppleFmStructuredValueError::InvalidSchemaRoot);
        }
        Ok(())
    }
}

/// Structured content returned by Apple FM guided generation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AppleFmGeneratedContent {
    /// Locally assigned generation identifier.
    #[serde(default)]
    pub generation_id: AppleFmGenerationId,
    /// Structured JSON payload returned by Apple FM.
    pub content: Value,
    /// Whether the content is complete.
    #[serde(default = "default_true")]
    pub is_complete: bool,
}

impl AppleFmGeneratedContent {
    /// Wraps a raw JSON value as completed generated content.
    #[must_use]
    pub fn new(content: Value) -> Self {
        Self {
            generation_id: AppleFmGenerationId::new(),
            content,
            is_complete: true,
        }
    }

    /// Parses generated content from a JSON string.
    pub fn from_json_str(value: &str) -> Result<Self, AppleFmStructuredValueError> {
        let content = serde_json::from_str(value).map_err(|error| {
            AppleFmStructuredValueError::ContentDecode {
                error: error.to_string(),
            }
        })?;
        Ok(Self::new(content))
    }

    /// Serializes generated content to a compact JSON string.
    pub fn to_json_string(&self) -> Result<String, AppleFmStructuredValueError> {
        serde_json::to_string(&self.content).map_err(|error| {
            AppleFmStructuredValueError::ContentEncode {
                error: error.to_string(),
            }
        })
    }

    /// Decodes the full generated payload into a Rust type.
    pub fn to_typed<T: DeserializeOwned>(&self) -> Result<T, AppleFmStructuredValueError> {
        serde_json::from_value(self.content.clone()).map_err(|error| {
            AppleFmStructuredValueError::TypedDecode {
                error: error.to_string(),
            }
        })
    }

    /// Decodes one named property from an object payload.
    pub fn property<T: DeserializeOwned>(
        &self,
        property_name: &str,
    ) -> Result<Option<T>, AppleFmStructuredValueError> {
        match self
            .content
            .as_object()
            .and_then(|object| object.get(property_name))
        {
            Some(value) => serde_json::from_value(value.clone())
                .map(Some)
                .map_err(|error| AppleFmStructuredValueError::TypedDecode {
                    error: error.to_string(),
                }),
            None => Ok(None),
        }
    }
}

/// Schema/content validation and conversion failures for structured Apple FM output.
#[derive(Clone, Debug, Error, PartialEq)]
pub enum AppleFmStructuredValueError {
    /// Schema JSON failed to decode.
    #[error("invalid Apple FM generation schema JSON: {error}")]
    SchemaDecode { error: String },
    /// Schema JSON failed to encode.
    #[error("failed to serialize Apple FM generation schema: {error}")]
    SchemaEncode { error: String },
    /// Content JSON failed to decode.
    #[error("invalid Apple FM generated content JSON: {error}")]
    ContentDecode { error: String },
    /// Content JSON failed to encode.
    #[error("failed to serialize Apple FM generated content: {error}")]
    ContentEncode { error: String },
    /// Typed decoding from generated content failed.
    #[error("failed to decode Apple FM generated content into the requested Rust type: {error}")]
    TypedDecode { error: String },
    /// Generation options were invalid for structured generation.
    #[error("invalid Apple FM structured generation options: {error}")]
    OptionsValidation { error: String },
    /// JSON schema roots must be objects.
    #[error("Apple FM generation schema root must be a JSON object")]
    InvalidSchemaRoot,
}

fn sanitize_generated_schema(schema: &mut Value, title_hint: Option<&str>) {
    if let Some(schema_object) = schema.as_object_mut() {
        schema_object.remove("$schema");
        if let Some(properties) = schema_object.get("properties").and_then(Value::as_object) {
            let property_order = properties
                .keys()
                .map(|key| Value::String(key.clone()))
                .collect::<Vec<_>>();
            schema_object
                .entry("additionalProperties".to_string())
                .or_insert(Value::Bool(false));
            schema_object
                .entry("x-order".to_string())
                .or_insert(Value::Array(property_order));
            if let Some(title_hint) = title_hint {
                schema_object
                    .entry("title".to_string())
                    .or_insert(Value::String(title_hint.to_string()));
            }
        }
        if let Some(properties) = schema_object
            .get_mut("properties")
            .and_then(Value::as_object_mut)
        {
            for (property_name, property_schema) in properties {
                sanitize_generated_schema(property_schema, Some(property_name.as_str()));
            }
        }
        if let Some(definitions) = schema_object
            .get_mut("$defs")
            .and_then(Value::as_object_mut)
        {
            for (definition_name, definition_schema) in definitions {
                sanitize_generated_schema(definition_schema, Some(definition_name.as_str()));
            }
        }
        if let Some(items) = schema_object.get_mut("items") {
            sanitize_generated_schema(items, None);
        }
        if let Some(any_of) = schema_object.get_mut("anyOf").and_then(Value::as_array_mut) {
            for schema in any_of {
                sanitize_generated_schema(schema, None);
            }
        }
        if let Some(one_of) = schema_object.get_mut("oneOf").and_then(Value::as_array_mut) {
            for schema in one_of {
                sanitize_generated_schema(schema, None);
            }
        }
        if let Some(all_of) = schema_object.get_mut("allOf").and_then(Value::as_array_mut) {
            for schema in all_of {
                sanitize_generated_schema(schema, None);
            }
        }
        if let Some(not_schema) = schema_object.get_mut("not") {
            sanitize_generated_schema(not_schema, None);
        }
    }
}

const fn default_true() -> bool {
    true
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use schemars::JsonSchema;
    use serde::{Deserialize, Serialize};

    use super::{AppleFmGeneratedContent, AppleFmGenerationSchema, AppleFmStructuredValueError};

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct ProductReview {
        sentiment: ReviewSentiment,
        #[schemars(range(min = 1.0, max = 5.0))]
        rating: f64,
        #[schemars(length(min = 3, max = 3))]
        keywords: Vec<String>,
        #[schemars(pattern(r"^\\w+[ ]\\w+$"))]
        reviewer_name: String,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct ProjectReport {
        summary: ProductReview,
        #[schemars(length(min = 2, max = 2))]
        checklist: Vec<String>,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    struct ReviewCatalog {
        owner: String,
        reports: Vec<ProductReview>,
    }

    #[derive(Clone, Debug, Deserialize, JsonSchema, PartialEq, Serialize)]
    enum ReviewSentiment {
        Positive,
        Negative,
        Neutral,
    }

    #[test]
    fn typed_schema_builder_emits_supported_constraint_families() {
        let schema =
            AppleFmGenerationSchema::from_type::<ProductReview>().expect("build typed schema");
        let schema_json = schema.clone_json_value();
        let properties = schema_json["properties"]
            .as_object()
            .expect("object properties");

        assert!(
            properties["sentiment"].get("$ref").is_some()
                || properties["sentiment"].get("oneOf").is_some()
                || properties["sentiment"].get("enum").is_some()
        );
        assert_eq!(properties["rating"]["minimum"], 1.0);
        assert_eq!(properties["rating"]["maximum"], 5.0);
        assert_eq!(properties["keywords"]["minItems"], 3);
        assert_eq!(properties["keywords"]["maxItems"], 3);
        assert_eq!(properties["reviewer_name"]["pattern"], r"^\\w+[ ]\\w+$");
        assert_eq!(schema_json["title"], "ProductReview");
        assert!(schema_json["x-order"].is_array());
    }

    #[test]
    fn typed_schema_builder_supports_nested_objects_and_lists() {
        let nested_schema =
            AppleFmGenerationSchema::from_type::<ProjectReport>().expect("project report schema");
        let nested_schema_json = nested_schema.clone_json_value();
        let nested_properties = nested_schema_json["properties"]
            .as_object()
            .expect("nested object properties");
        assert!(
            nested_properties["summary"].get("$ref").is_some()
                || nested_properties["summary"].get("type").is_some()
        );
        assert_eq!(nested_properties["checklist"]["type"], "array");
        assert_eq!(nested_properties["checklist"]["items"]["type"], "string");

        let list_schema =
            AppleFmGenerationSchema::from_type::<ReviewCatalog>().expect("review catalog schema");
        let list_schema_json = list_schema.clone_json_value();
        let list_properties = list_schema_json["properties"]
            .as_object()
            .expect("list object properties");
        assert_eq!(list_properties["owner"]["type"], "string");
        assert_eq!(list_properties["reports"]["type"], "array");
        assert!(
            list_properties["reports"]["items"].get("$ref").is_some()
                || list_properties["reports"]["items"].get("type").is_some()
        );
    }

    #[test]
    fn raw_schema_rejects_non_object_roots() {
        let error = AppleFmGenerationSchema::from_json_str("\"not a schema\"")
            .expect_err("non-object schema should fail");
        assert_eq!(error, AppleFmStructuredValueError::InvalidSchemaRoot);
    }

    #[test]
    fn raw_schema_normalizes_title_and_property_order_for_runtime_use() {
        let schema = AppleFmGenerationSchema::with_title_hint(
            serde_json::json!({
                "type": "object",
                "properties": {
                    "apple_lane": {"type": "string"},
                    "runtime_validation": {"type": "string"}
                },
                "required": ["apple_lane", "runtime_validation"]
            }),
            Some("AppleLaneStatus"),
        )
        .expect("normalize raw runtime schema");
        let schema_json = schema.clone_json_value();
        assert_eq!(schema_json["title"], "AppleLaneStatus");
        assert_eq!(schema_json["additionalProperties"], false);
        assert!(schema_json["x-order"].is_array());
    }

    #[test]
    fn generated_content_round_trips_to_typed_value() {
        let content = AppleFmGeneratedContent::from_json_str(
            r#"{"sentiment":"Positive","rating":4.5,"keywords":["battery","screen","speed"],"reviewer_name":"Taylor Kim"}"#,
        )
        .expect("parse generated content");

        let typed = content
            .to_typed::<ProductReview>()
            .expect("decode typed content");
        assert_eq!(
            typed,
            ProductReview {
                sentiment: ReviewSentiment::Positive,
                rating: 4.5,
                keywords: vec![
                    "battery".to_string(),
                    "screen".to_string(),
                    "speed".to_string()
                ],
                reviewer_name: "Taylor Kim".to_string(),
            }
        );
        assert_eq!(
            content
                .property::<Vec<String>>("keywords")
                .expect("decode property"),
            Some(vec![
                "battery".to_string(),
                "screen".to_string(),
                "speed".to_string()
            ])
        );
    }

    #[test]
    fn nested_generated_content_round_trips_to_typed_value() {
        let content = AppleFmGeneratedContent::from_json_str(
            r#"{
                "summary":{
                    "sentiment":"Neutral",
                    "rating":3.0,
                    "keywords":["metal","bridge","sdk"],
                    "reviewer_name":"Casey Stone"
                },
                "checklist":["export schema","decode content"]
            }"#,
        )
        .expect("parse nested generated content");

        let typed = content
            .to_typed::<ProjectReport>()
            .expect("decode nested typed content");
        assert_eq!(typed.summary.sentiment, ReviewSentiment::Neutral);
        assert_eq!(
            typed.checklist,
            vec!["export schema".to_string(), "decode content".to_string()]
        );
    }
}
