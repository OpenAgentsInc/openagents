use crate::model_family::ModelFamily;

/// Metadata about a model, particularly OpenAI models.
/// We may want to consider including details like the pricing for
/// input tokens, output tokens, etc., though users will need to be able to
/// override this in config.toml, as this information can get out of date.
/// Though this would help present more accurate pricing information in the UI.
#[derive(Debug)]
pub(crate) struct ModelInfo {
    /// Size of the context window in tokens.
    pub(crate) context_window: u64,

    /// Maximum number of output tokens that can be generated for the model.
    pub(crate) max_output_tokens: u64,

    /// Token threshold where we should automatically compact conversation history.
    pub(crate) auto_compact_token_limit: Option<i64>,
}

impl ModelInfo {
    const fn new(context_window: u64, max_output_tokens: u64) -> Self {
        Self {
            context_window,
            max_output_tokens,
            auto_compact_token_limit: None,
        }
    }
}

pub(crate) fn get_model_info(model_family: &ModelFamily) -> Option<ModelInfo> {
    let slug = model_family.slug.as_str();
    match slug {
        // OSS models have a 128k shared token pool.
        // Arbitrarily splitting it: 3/4 input context, 1/4 output.
        // https://openai.com/index/gpt-oss-model-card/
        "gpt-oss-20b" => Some(ModelInfo::new(96_000, 32_000)),
        "gpt-oss-120b" => Some(ModelInfo::new(96_000, 32_000)),
        // https://platform.openai.com/docs/models/o3
        "o3" => Some(ModelInfo::new(200_000, 100_000)),

        // https://platform.openai.com/docs/models/o4-mini
        "o4-mini" => Some(ModelInfo::new(200_000, 100_000)),

        // https://platform.openai.com/docs/models/codex-mini-latest
        "codex-mini-latest" => Some(ModelInfo::new(200_000, 100_000)),

        // As of Jun 25, 2025, gpt-4.1 defaults to gpt-4.1-2025-04-14.
        // https://platform.openai.com/docs/models/gpt-4.1
        "gpt-4.1" | "gpt-4.1-2025-04-14" => Some(ModelInfo::new(1_047_576, 32_768)),

        // As of Jun 25, 2025, gpt-4o defaults to gpt-4o-2024-08-06.
        // https://platform.openai.com/docs/models/gpt-4o
        "gpt-4o" | "gpt-4o-2024-08-06" => Some(ModelInfo::new(128_000, 16_384)),

        // https://platform.openai.com/docs/models/gpt-4o?snapshot=gpt-4o-2024-05-13
        "gpt-4o-2024-05-13" => Some(ModelInfo::new(128_000, 4_096)),

        // https://platform.openai.com/docs/models/gpt-4o?snapshot=gpt-4o-2024-11-20
        "gpt-4o-2024-11-20" => Some(ModelInfo::new(128_000, 16_384)),

        // https://platform.openai.com/docs/models/gpt-3.5-turbo
        "gpt-3.5-turbo" => Some(ModelInfo::new(16_385, 4_096)),

        _ if slug.starts_with("gpt-5") => Some(ModelInfo::new(272_000, 128_000)),

        _ if slug.starts_with("codex-") => Some(ModelInfo::new(272_000, 128_000)),

        _ => None,
    }
}
