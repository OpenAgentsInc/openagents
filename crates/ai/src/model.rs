use crate::provider::Provider;
use serde::{Deserialize, Serialize};
use std::fmt;
use thiserror::Error;

/// Unified model enum for all allowed AI providers
/// Only contains compile-time approved models
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Model {
    // Claude 4.5/4.1 models
    #[serde(alias = "claude-haiku-4-5-20251001")]
    ClaudeHaiku45,
    #[serde(alias = "claude-sonnet-4-5-20250929")]
    ClaudeSonnet45,
    #[serde(alias = "claude-opus-4-5-20251101")]
    ClaudeOpus45,
    #[serde(alias = "claude-opus-4-1-20250805")]
    ClaudeOpus41,

    // Grok models
    #[serde(alias = "grok-2-vision-latest")]
    Grok2Vision,
    #[serde(alias = "grok-3-latest")]
    Grok3,
    #[serde(alias = "grok-3-mini-latest")]
    Grok3Mini,
    #[serde(alias = "grok-3-fast-latest")]
    Grok3Fast,
    #[serde(alias = "grok-3-mini-fast-latest")]
    Grok3MiniFast,
    #[serde(alias = "grok-4-latest")]
    Grok4,
    #[serde(alias = "grok-4-fast-reasoning-latest")]
    Grok4FastReasoning,
    #[serde(alias = "grok-4-fast-non-reasoning-latest")]
    Grok4FastNonReasoning,
    #[serde(alias = "grok-4-1-fast-reasoning-latest")]
    Grok41FastReasoning,
    #[serde(alias = "grok-4-1-fast-non-reasoning-latest")]
    Grok41FastNonReasoning,
    #[serde(alias = "grok-code-fast-1-0825")]
    GrokCodeFast1,

    // OpenAI models
    #[serde(alias = "gpt-4o")]
    Gpt4o,
    #[serde(alias = "gpt-4o-mini")]
    Gpt4oMini,
    #[serde(alias = "o1")]
    O1,
    #[serde(alias = "o3")]
    O3,
    #[serde(alias = "o3-mini")]
    O3Mini,
    #[serde(alias = "o4-mini")]
    O4Mini,
    #[serde(alias = "gpt-4-turbo")]
    Gpt4Turbo,
    #[serde(alias = "gpt-4.1")]
    Gpt41,
    #[serde(alias = "gpt-4.1-mini")]
    Gpt41Mini,
    #[serde(alias = "gpt-4.1-nano")]
    Gpt41Nano,
    #[serde(alias = "gpt-5")]
    Gpt5,
    #[serde(alias = "gpt-5-mini")]
    Gpt5Mini,
    #[serde(alias = "gpt-5-nano")]
    Gpt5Nano,
    #[serde(alias = "gpt-5.1")]
    Gpt51,
}

impl Default for Model {
    fn default() -> Self {
        Self::ClaudeHaiku45
    }
}

#[derive(Debug, Error)]
pub enum ModelError {
    #[error(
        "Model is deprecated. Use 'claude-haiku-4-5-20251001' (Haiku 4.5) or 'claude-sonnet-4-5-20250929' (Sonnet 4.5): {0}"
    )]
    DeprecatedModel(String),
    #[error("Unknown model: {0}")]
    UnknownModel(String),
    #[error("Model '{0}' is not a Bedrock model")]
    NotBedrockModel(String),
}

impl Model {
    /// Get the API format ID (e.g., "claude-haiku-4-5-20251001")
    pub const fn id(&self) -> &'static str {
        match self {
            // Claude
            Model::ClaudeHaiku45 => "claude-haiku-4-5-20251001",
            Model::ClaudeSonnet45 => "claude-sonnet-4-5-20250929",
            Model::ClaudeOpus45 => "claude-opus-4-5-20251101",
            Model::ClaudeOpus41 => "claude-opus-4-1-20250805",

            // Grok
            Model::Grok2Vision => "grok-2-vision-latest",
            Model::Grok3 => "grok-3-latest",
            Model::Grok3Mini => "grok-3-mini-latest",
            Model::Grok3Fast => "grok-3-fast-latest",
            Model::Grok3MiniFast => "grok-3-mini-fast-latest",
            Model::Grok4 => "grok-4-latest",
            Model::Grok4FastReasoning => "grok-4-fast-reasoning-latest",
            Model::Grok4FastNonReasoning => "grok-4-fast-non-reasoning-latest",
            Model::Grok41FastReasoning => "grok-4-1-fast-reasoning-latest",
            Model::Grok41FastNonReasoning => "grok-4-1-fast-non-reasoning-latest",
            Model::GrokCodeFast1 => "grok-code-fast-1-0825",

            // OpenAI
            Model::Gpt4o => "gpt-4o",
            Model::Gpt4oMini => "gpt-4o-mini",
            Model::O1 => "o1",
            Model::O3 => "o3",
            Model::O3Mini => "o3-mini",
            Model::O4Mini => "o4-mini",
            Model::Gpt4Turbo => "gpt-4-turbo",
            Model::Gpt41 => "gpt-4.1",
            Model::Gpt41Mini => "gpt-4.1-mini",
            Model::Gpt41Nano => "gpt-4.1-nano",
            Model::Gpt5 => "gpt-5",
            Model::Gpt5Mini => "gpt-5-mini",
            Model::Gpt5Nano => "gpt-5-nano",
            Model::Gpt51 => "gpt-5.1",
        }
    }

    /// Get the display name for the model
    pub const fn display_name(&self) -> &'static str {
        match self {
            // Claude
            Model::ClaudeHaiku45 => "Claude Haiku 4.5",
            Model::ClaudeSonnet45 => "Claude Sonnet 4.5",
            Model::ClaudeOpus45 => "Claude Opus 4.5",
            Model::ClaudeOpus41 => "Claude Opus 4.1",

            // Grok
            Model::Grok2Vision => "Grok 2 Vision",
            Model::Grok3 => "Grok 3",
            Model::Grok3Mini => "Grok 3 Mini",
            Model::Grok3Fast => "Grok 3 Fast",
            Model::Grok3MiniFast => "Grok 3 Mini Fast",
            Model::Grok4 => "Grok 4",
            Model::Grok4FastReasoning => "Grok 4 Fast Reasoning",
            Model::Grok4FastNonReasoning => "Grok 4 Fast Non-Reasoning",
            Model::Grok41FastReasoning => "Grok 4.1 Fast Reasoning",
            Model::Grok41FastNonReasoning => "Grok 4.1 Fast Non-Reasoning",
            Model::GrokCodeFast1 => "Grok Code Fast 1",

            // OpenAI
            Model::Gpt4o => "GPT-4o",
            Model::Gpt4oMini => "GPT-4o Mini",
            Model::O1 => "o1",
            Model::O3 => "o3",
            Model::O3Mini => "o3-mini",
            Model::O4Mini => "o4-mini",
            Model::Gpt4Turbo => "GPT-4 Turbo",
            Model::Gpt41 => "GPT-4.1",
            Model::Gpt41Mini => "GPT-4.1 Mini",
            Model::Gpt41Nano => "GPT-4.1 Nano",
            Model::Gpt5 => "GPT-5",
            Model::Gpt5Mini => "GPT-5 Mini",
            Model::Gpt5Nano => "GPT-5 Nano",
            Model::Gpt51 => "GPT-5.1",
        }
    }

    /// Get the provider for this model
    pub const fn provider(&self) -> Provider {
        match self {
            // Claude
            Model::ClaudeHaiku45
            | Model::ClaudeSonnet45
            | Model::ClaudeOpus45
            | Model::ClaudeOpus41 => Provider::Anthropic,

            // Grok
            Model::Grok2Vision
            | Model::Grok3
            | Model::Grok3Mini
            | Model::Grok3Fast
            | Model::Grok3MiniFast
            | Model::Grok4
            | Model::Grok4FastReasoning
            | Model::Grok4FastNonReasoning
            | Model::Grok41FastReasoning
            | Model::Grok41FastNonReasoning
            | Model::GrokCodeFast1 => Provider::Grok,

            // OpenAI
            Model::Gpt4o
            | Model::Gpt4oMini
            | Model::O1
            | Model::O3
            | Model::O3Mini
            | Model::O4Mini
            | Model::Gpt4Turbo
            | Model::Gpt41
            | Model::Gpt41Mini
            | Model::Gpt41Nano
            | Model::Gpt5
            | Model::Gpt5Mini
            | Model::Gpt5Nano
            | Model::Gpt51 => Provider::OpenAI,
        }
    }

    /// Parse a model from its API ID
    /// Returns an error if the model ID is not in the allowed list
    pub fn from_id(id: &str) -> Result<Self, ModelError> {
        // Check for deprecated models first with helpful error messages
        if id == "claude-sonnet-4-20250514"
            || id == "claude-sonnet-4-20250514-v1:0"
            || id == "anthropic.claude-sonnet-4-20250514-v1:0"
        {
            return Err(ModelError::DeprecatedModel(id.to_string()));
        }

        if id == "claude-3-5-sonnet-20241022"
            || id == "claude-3-5-sonnet-20241022-v2:0"
            || id == "anthropic.claude-3-5-sonnet-20241022-v2:0"
        {
            return Err(ModelError::DeprecatedModel(id.to_string()));
        }

        if id == "claude-3-5-haiku-20241022"
            || id == "claude-3-5-haiku-20241022-v1:0"
            || id == "anthropic.claude-3-5-haiku-20241022-v1:0"
        {
            return Err(ModelError::DeprecatedModel(id.to_string()));
        }

        // Try to parse the model
        match id.to_lowercase().as_str() {
            // Claude
            "claude-haiku-4-5-20251001"
            | "claude-haiku-4-5-20251001-v1:0"
            | "anthropic.claude-haiku-4-5-20251001-v1:0" => Ok(Model::ClaudeHaiku45),
            "claude-sonnet-4-5-20250929"
            | "claude-sonnet-4-5-20250929-v1:0"
            | "anthropic.claude-sonnet-4-5-20250929-v1:0" => Ok(Model::ClaudeSonnet45),
            "claude-opus-4-5-20251101"
            | "claude-opus-4-5-20251101-v1:0"
            | "anthropic.claude-opus-4-5-20251101-v1:0" => Ok(Model::ClaudeOpus45),
            "claude-opus-4-1-20250805"
            | "claude-opus-4-1-20250805-v1:0"
            | "anthropic.claude-opus-4-1-20250805-v1:0" => Ok(Model::ClaudeOpus41),

            // Grok
            "grok-2-vision-latest" => Ok(Model::Grok2Vision),
            "grok-3-latest" => Ok(Model::Grok3),
            "grok-3-mini-latest" => Ok(Model::Grok3Mini),
            "grok-3-fast-latest" => Ok(Model::Grok3Fast),
            "grok-3-mini-fast-latest" => Ok(Model::Grok3MiniFast),
            "grok-4-latest" | "grok-4" => Ok(Model::Grok4),
            "grok-4-fast-reasoning-latest" | "grok-4-fast-reasoning" => {
                Ok(Model::Grok4FastReasoning)
            }
            "grok-4-fast-non-reasoning-latest" | "grok-4-fast-non-reasoning" => {
                Ok(Model::Grok4FastNonReasoning)
            }
            "grok-4-1-fast-reasoning-latest" | "grok-4-1-fast-reasoning" | "grok-4-1-fast" => {
                Ok(Model::Grok41FastReasoning)
            }
            "grok-4-1-fast-non-reasoning-latest" | "grok-4-1-fast-non-reasoning" => {
                Ok(Model::Grok41FastNonReasoning)
            }
            "grok-code-fast-1-0825" | "grok-code-fast-1" => Ok(Model::GrokCodeFast1),

            // OpenAI
            "gpt-4o" => Ok(Model::Gpt4o),
            "gpt-4o-mini" => Ok(Model::Gpt4oMini),
            "o1" => Ok(Model::O1),
            "o3" => Ok(Model::O3),
            "o3-mini" => Ok(Model::O3Mini),
            "o4-mini" => Ok(Model::O4Mini),
            "gpt-4-turbo" | "gpt-4-turbo-2024-04-09" => Ok(Model::Gpt4Turbo),
            "gpt-4.1" => Ok(Model::Gpt41),
            "gpt-4.1-mini" => Ok(Model::Gpt41Mini),
            "gpt-4.1-nano" => Ok(Model::Gpt41Nano),
            "gpt-5" => Ok(Model::Gpt5),
            "gpt-5-mini" => Ok(Model::Gpt5Mini),
            "gpt-5-nano" => Ok(Model::Gpt5Nano),
            "gpt-5.1" => Ok(Model::Gpt51),

            _ => Err(ModelError::UnknownModel(id.to_string())),
        }
    }

    /// Get the Bedrock ARN format for this model (if supported)
    pub fn bedrock_arn(&self) -> Result<String, ModelError> {
        let arn = match self {
            // Claude
            Model::ClaudeHaiku45 => "anthropic.claude-haiku-4-5-20251001-v1:0",
            Model::ClaudeSonnet45 => "anthropic.claude-sonnet-4-5-20250929-v1:0",
            Model::ClaudeOpus45 => "anthropic.claude-opus-4-5-20251101-v1:0",
            Model::ClaudeOpus41 => "anthropic.claude-opus-4-1-20250805-v1:0",

            // Grok and OpenAI don't have Bedrock ARNs
            _ => return Err(ModelError::NotBedrockModel(self.display_name().to_string())),
        };
        Ok(arn.to_string())
    }

    /// Get all allowed Claude models
    pub fn all_claude() -> &'static [Model] {
        &[
            Model::ClaudeHaiku45,
            Model::ClaudeSonnet45,
            Model::ClaudeOpus45,
            Model::ClaudeOpus41,
        ]
    }

    /// Get all allowed Grok models
    pub fn all_grok() -> &'static [Model] {
        &[
            Model::Grok2Vision,
            Model::Grok3,
            Model::Grok3Mini,
            Model::Grok3Fast,
            Model::Grok3MiniFast,
            Model::Grok4,
            Model::Grok4FastReasoning,
            Model::Grok4FastNonReasoning,
            Model::Grok41FastReasoning,
            Model::Grok41FastNonReasoning,
            Model::GrokCodeFast1,
        ]
    }

    /// Get all allowed OpenAI models
    pub fn all_openai() -> &'static [Model] {
        &[
            Model::Gpt4o,
            Model::Gpt4oMini,
            Model::O1,
            Model::O3,
            Model::O3Mini,
            Model::O4Mini,
            Model::Gpt4Turbo,
            Model::Gpt41,
            Model::Gpt41Mini,
            Model::Gpt41Nano,
            Model::Gpt5,
            Model::Gpt5Mini,
            Model::Gpt5Nano,
            Model::Gpt51,
        ]
    }
}

impl fmt::Display for Model {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display_name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_model() {
        assert_eq!(Model::default(), Model::ClaudeHaiku45);
    }

    #[test]
    fn test_model_ids() {
        assert_eq!(Model::ClaudeHaiku45.id(), "claude-haiku-4-5-20251001");
        assert_eq!(Model::ClaudeSonnet45.id(), "claude-sonnet-4-5-20250929");
        assert_eq!(
            Model::Grok41FastReasoning.id(),
            "grok-4-1-fast-reasoning-latest"
        );
        assert_eq!(Model::Gpt4o.id(), "gpt-4o");
    }

    #[test]
    fn test_from_id() {
        assert_eq!(
            Model::from_id("claude-haiku-4-5-20251001").unwrap(),
            Model::ClaudeHaiku45
        );
        assert_eq!(
            Model::from_id("claude-sonnet-4-5-20250929").unwrap(),
            Model::ClaudeSonnet45
        );
        assert_eq!(
            Model::from_id("grok-4-1-fast-reasoning-latest").unwrap(),
            Model::Grok41FastReasoning
        );
        assert_eq!(Model::from_id("gpt-4o").unwrap(), Model::Gpt4o);
    }

    #[test]
    fn test_deprecated_models_error() {
        assert!(Model::from_id("claude-sonnet-4-20250514").is_err());
        assert!(Model::from_id("claude-3-5-sonnet-20241022").is_err());
        assert!(Model::from_id("claude-3-5-haiku-20241022").is_err());
    }

    #[test]
    fn test_bedrock_arn() {
        let arn = Model::ClaudeHaiku45.bedrock_arn().unwrap();
        assert!(arn.contains("claude-haiku"));
        assert!(arn.starts_with("anthropic."));
    }

    #[test]
    fn test_providers() {
        assert_eq!(Model::ClaudeHaiku45.provider(), Provider::Anthropic);
        assert_eq!(Model::Grok3.provider(), Provider::Grok);
        assert_eq!(Model::Gpt4o.provider(), Provider::OpenAI);
    }

    #[test]
    fn test_all_claude() {
        assert_eq!(Model::all_claude().len(), 4);
    }

    #[test]
    fn test_all_grok() {
        assert_eq!(Model::all_grok().len(), 11);
    }

    #[test]
    fn test_all_openai() {
        assert_eq!(Model::all_openai().len(), 14);
    }
}
