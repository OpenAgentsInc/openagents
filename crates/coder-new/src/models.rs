//! Plugin-qualified model choices and staged generation settings.

use crossterm::event::{KeyCode, KeyEvent};
use serde::{Deserialize, Serialize};

use crate::Draft;

pub const OPENROUTER_PLUGIN: &str = "openrouter-byok";
pub const DEFAULT_MODEL: &str = "openrouter/free";
pub const SHORTLIST: [&str; 7] = [
    DEFAULT_MODEL,
    "openai/gpt-6-luna",
    "openai/gpt-6.1-sol",
    "anthropic/claude-fable-5.1",
    "google/gemini-3.5-flash",
    "deepseek/deepseek-v4.1-flash",
    "x-ai/grok-4.7",
];

/// Whether `model` takes images in a user message (#11173): the shortlist's
/// vision models and the model families that all accept images. The free
/// router and other models get a plain note with the file's path instead.
#[must_use]
pub fn accepts_images(model: &str) -> bool {
    const FAMILIES: [&str; 6] = [
        "openai/gpt-6",
        "openai/gpt-5",
        "openai/gpt-4o",
        "anthropic/claude-",
        "google/gemini-",
        "x-ai/grok-4",
    ];
    let model = model.trim();
    FAMILIES.iter().any(|family| model.starts_with(family))
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GenerationOptions {
    pub reasoning: Option<String>,
    pub max_tokens: Option<u32>,
}

impl GenerationOptions {
    pub fn slug(&self, model: &str) -> String {
        let mut slug = model.to_owned();
        if let Some(reasoning) = &self.reasoning {
            slug.push(':');
            slug.push_str(reasoning);
        }
        if let Some(tokens) = self.max_tokens {
            slug.push_str(&format!(":max-tokens={tokens}"));
        }
        slug
    }

    pub fn valid(&self) -> bool {
        self.reasoning.as_deref().is_none_or(|effort| {
            matches!(
                effort,
                "none" | "minimal" | "low" | "medium" | "high" | "xhigh" | "max"
            )
        }) && self
            .max_tokens
            .is_none_or(|tokens| (1..=32_768).contains(&tokens))
    }
}

/// Identity includes the providing plugin so different plugins can offer the same model.
#[derive(Clone, Debug)]
pub struct Model {
    pub plugin: String,
    pub provider: String,
    pub id: String,
    pub name: String,
    pub description: String,
    pub context_length: Option<u32>,
    pub max_output_tokens: Option<u32>,
    pub efforts: Vec<String>,
    pub default_effort: Option<String>,
    pub supports_output_limit: bool,
}

/// A small known catalog is available while public model metadata refreshes.
pub fn openrouter_catalog() -> Vec<Model> {
    let rows: [(&str, &str, &[&str], Option<&str>); 7] = [
        ("Free router", "Automatic free text model", &[], None),
        (
            "GPT-6 Luna",
            "Fast OpenAI text model",
            &["none", "low", "medium", "high", "xhigh", "max"],
            Some("medium"),
        ),
        (
            "GPT-6.1 Sol",
            "OpenAI text and reasoning",
            &["low", "medium", "high", "xhigh", "max"],
            Some("medium"),
        ),
        (
            "Claude Fable 5.1",
            "Anthropic text and reasoning",
            &["low", "medium", "high", "xhigh", "max"],
            Some("high"),
        ),
        (
            "Gemini 3.5 Flash",
            "Fast Google text model",
            &["minimal", "low", "medium", "high"],
            Some("medium"),
        ),
        (
            "DeepSeek V4.1 Flash",
            "DeepSeek text and reasoning",
            &["low", "high", "max"],
            Some("high"),
        ),
        (
            "Grok 4.7",
            "xAI text and reasoning",
            &["low", "medium", "high", "xhigh"],
            Some("high"),
        ),
    ];
    SHORTLIST
        .iter()
        .zip(rows)
        .map(|(id, (name, description, efforts, default))| Model {
            plugin: OPENROUTER_PLUGIN.into(),
            provider: "OpenRouter BYOK".into(),
            id: (*id).into(),
            name: name.into(),
            description: description.into(),
            context_length: None,
            max_output_tokens: None,
            efforts: efforts.iter().map(|effort| (*effort).into()).collect(),
            default_effort: default.map(str::to_owned),
            supports_output_limit: true,
        })
        .collect()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Stage {
    #[default]
    Models,
    Reasoning,
    Output,
}

pub struct Picker {
    pub models: Vec<Model>,
    pub query: Draft,
    pub selected: usize,
    pub stage: Stage,
    pub pending: Option<Model>,
    pub options: GenerationOptions,
    pub active_plugin: String,
    pub active_model: String,
    pub active_options: GenerationOptions,
    pub loading: bool,
    pub refresh_requested: bool,
    pub error: Option<String>,
}

pub enum Action {
    Continue,
    Close,
    Save(Model, GenerationOptions),
}

impl Picker {
    pub fn new(
        models: Vec<Model>,
        plugin: &str,
        model: &str,
        options: GenerationOptions,
        live: bool,
    ) -> Self {
        let selected = models
            .iter()
            .position(|item| item.plugin == plugin && item.id == model)
            .unwrap_or(0);
        Self {
            models,
            query: Draft::default(),
            selected,
            stage: Stage::Models,
            pending: None,
            options: options.clone(),
            active_plugin: plugin.into(),
            active_model: model.into(),
            active_options: options,
            loading: live,
            refresh_requested: live,
            error: None,
        }
    }

    pub fn matching(&self) -> Vec<&Model> {
        let query = self.query.text.to_lowercase();
        self.models
            .iter()
            .filter(|model| {
                format!(
                    "{} {} {} {}",
                    model.name, model.id, model.provider, model.description
                )
                .to_lowercase()
                .contains(&query)
            })
            .collect()
    }

    pub fn reasoning_choices(&self) -> Vec<Option<String>> {
        std::iter::once(None)
            .chain(
                self.pending
                    .iter()
                    .flat_map(|model| model.efforts.iter().cloned().map(Some)),
            )
            .collect()
    }

    pub fn output_choices(&self) -> Vec<Option<u32>> {
        let maximum = self
            .pending
            .as_ref()
            .and_then(|model| model.max_output_tokens)
            .unwrap_or(32_768)
            .min(32_768);
        std::iter::once(None)
            .chain(
                [2_048, 4_096, 8_192, 16_384, 32_768]
                    .into_iter()
                    .filter(|value| *value <= maximum)
                    .map(Some),
            )
            .collect()
    }

    pub fn paste(&mut self, text: &str) {
        if self.stage == Stage::Models {
            self.query.insert(&text.replace(['\r', '\n'], ""));
            self.selected = 0;
        }
    }

    pub fn refresh(&mut self, models: Vec<Model>, error: Option<String>) {
        let identity = self
            .matching()
            .get(self.selected)
            .map(|model| (model.plugin.clone(), model.id.clone()));
        self.models = models;
        self.loading = false;
        self.error = error;
        if let Some(pending) = &self.pending {
            let fresh = self
                .models
                .iter()
                .find(|model| model.plugin == pending.plugin && model.id == pending.id)
                .cloned();
            let changed = fresh.as_ref().is_none_or(|model| {
                model.efforts != pending.efforts
                    || model.supports_output_limit != pending.supports_output_limit
                    || model.max_output_tokens.unwrap_or(32_768).min(32_768)
                        != pending.max_output_tokens.unwrap_or(32_768).min(32_768)
            });
            if changed {
                self.back_to_models();
                self.error = Some("Model details changed. Select the model again.".into());
                return;
            }
            self.pending = fresh;
        }
        if self.stage == Stage::Models {
            self.selected = identity
                .and_then(|(plugin, id)| {
                    self.matching()
                        .iter()
                        .position(|model| model.plugin == plugin && model.id == id)
                })
                .unwrap_or(0);
        }
    }

    pub fn handle(&mut self, key: KeyEvent) -> Action {
        let count = match self.stage {
            Stage::Models => self.matching().len(),
            Stage::Reasoning => self.reasoning_choices().len(),
            Stage::Output => self.output_choices().len(),
        };
        match key.code {
            KeyCode::Down => self.selected = (self.selected + 1).min(count.saturating_sub(1)),
            KeyCode::Up => self.selected = self.selected.saturating_sub(1),
            KeyCode::Esc => match self.stage {
                Stage::Models => return Action::Close,
                Stage::Reasoning => self.back_to_models(),
                Stage::Output => {
                    if self
                        .pending
                        .as_ref()
                        .is_some_and(|model| !model.efforts.is_empty())
                    {
                        self.stage = Stage::Reasoning;
                        self.selected = self
                            .reasoning_choices()
                            .iter()
                            .position(|effort| *effort == self.options.reasoning)
                            .unwrap_or(0);
                    } else {
                        self.back_to_models();
                    }
                }
            },
            KeyCode::Enter if count > 0 => match self.stage {
                Stage::Models => {
                    let model = self.matching()[self.selected.min(count - 1)].clone();
                    let current =
                        model.plugin == self.active_plugin && model.id == self.active_model;
                    self.options = if current {
                        self.active_options.clone()
                    } else {
                        GenerationOptions::default()
                    };
                    if !model.efforts.is_empty() {
                        if !current {
                            self.options.reasoning = model.default_effort.clone();
                        }
                        self.pending = Some(model);
                        self.stage = Stage::Reasoning;
                        self.selected = self
                            .reasoning_choices()
                            .iter()
                            .position(|effort| *effort == self.options.reasoning)
                            .unwrap_or(0);
                    } else {
                        self.options.reasoning = None;
                        self.pending = Some(model);
                        return self.output_or_save();
                    }
                }
                Stage::Reasoning => {
                    self.options.reasoning =
                        self.reasoning_choices()[self.selected.min(count - 1)].clone();
                    return self.output_or_save();
                }
                Stage::Output => {
                    self.options.max_tokens = self.output_choices()[self.selected.min(count - 1)];
                    return Action::Save(self.pending.clone().unwrap(), self.options.clone());
                }
            },
            _ if self.stage == Stage::Models => {
                self.query.edit(key);
                self.selected = 0;
            }
            _ => {}
        }
        Action::Continue
    }

    fn output_or_save(&mut self) -> Action {
        if self
            .pending
            .as_ref()
            .is_some_and(|model| model.supports_output_limit)
        {
            self.stage = Stage::Output;
            self.selected = self
                .output_choices()
                .iter()
                .position(|limit| *limit == self.options.max_tokens)
                .unwrap_or(0);
            Action::Continue
        } else {
            self.options.max_tokens = None;
            Action::Save(self.pending.clone().unwrap(), self.options.clone())
        }
    }

    fn back_to_models(&mut self) {
        let pending = self.pending.take();
        self.stage = Stage::Models;
        self.selected = pending
            .and_then(|selected| {
                self.matching()
                    .iter()
                    .position(|model| model.plugin == selected.plugin && model.id == selected.id)
            })
            .unwrap_or(0);
    }
}
