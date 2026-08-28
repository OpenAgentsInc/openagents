use openagents_coder_contract::{CatalogModel, CatalogPricing, CatalogResponse};

use crate::config::Config;

#[derive(Clone, Debug)]
pub struct Model {
    pub id: String,
    pub provider: String,
    pub provider_model: String,
    pub context_window: u64,
    pub max_output: u64,
    pub pricing_basis: String,
    pub pricing_id: String,
    pub input_per_million: u64,
    pub output_per_million: u64,
    pub cached_input_per_million: Option<u64>,
}

impl Model {
    pub fn billable(&self) -> bool {
        self.pricing_basis == "declared"
    }
}

pub fn models(_config: &Config) -> Vec<Model> {
    vec![
        Model {
            id: "glm-5.3-flash".into(),
            provider: "vercel_gateway".into(),
            provider_model: "zai/glm-5.3-flash".into(),
            context_window: 1_000_000,
            max_output: 131_000,
            pricing_basis: "declared".into(),
            pricing_id: "declared.glm-5.3-flash.v1".into(),
            input_per_million: 150_000,
            output_per_million: 500_000,
            cached_input_per_million: Some(30_000),
        },
        Model {
            id: "gemini-3.7-flash".into(),
            provider: "vercel_gateway".into(),
            provider_model: "google/gemini-3.7-flash".into(),
            context_window: 1_048_576,
            max_output: 65_536,
            pricing_basis: "placeholder".into(),
            pricing_id: "placeholder.gemini-3.7-flash.v1".into(),
            input_per_million: 1_250_000,
            output_per_million: 10_000_000,
            cached_input_per_million: Some(100_000),
        },
        Model {
            id: "openrouter/free".into(),
            provider: "openrouter".into(),
            provider_model: "openrouter/free".into(),
            context_window: 32_768,
            max_output: 8_192,
            pricing_basis: "declared".into(),
            pricing_id: "declared.openrouter-free.v1".into(),
            input_per_million: 0,
            output_per_million: 0,
            cached_input_per_million: Some(0),
        },
    ]
}

pub fn fetch(config: &Config, id: &str) -> Option<Model> {
    models(config)
        .into_iter()
        .find(|model| model.id == id || model.provider_model == id)
}

pub fn default_id() -> &'static str {
    "glm-5.3-flash"
}

pub fn available(config: &Config, model: &Model) -> bool {
    match model.provider.as_str() {
        "vercel_gateway" => config.vercel_configured(),
        "openrouter" => config.openrouter_configured(),
        _ => false,
    }
}

pub fn project(config: &Config) -> CatalogResponse {
    let default = default_id().to_string();
    let models = models(config)
        .into_iter()
        .map(|model| {
            let avail = if available(config, &model) {
                "available"
            } else {
                "unavailable"
            };
            CatalogModel {
                id: model.id.clone(),
                provider: model.provider.clone(),
                context_window: model.context_window,
                max_output: model.max_output,
                availability: avail.into(),
                pricing_basis: model.pricing_basis.clone(),
                default: model.id == default,
                pricing: Some(CatalogPricing {
                    id: model.pricing_id,
                    basis: model.pricing_basis,
                    input_per_million_tokens: model.input_per_million,
                    output_per_million_tokens: model.output_per_million,
                    cached_input_per_million_tokens: model.cached_input_per_million,
                }),
            }
        })
        .collect();
    CatalogResponse { models, default }
}
