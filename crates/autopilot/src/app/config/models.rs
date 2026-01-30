#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ModelOption {
    Default,
    Mini,
    Reasoning,
}

impl ModelOption {
    pub(crate) fn all() -> [ModelOption; 3] {
        [
            ModelOption::Default,
            ModelOption::Mini,
            ModelOption::Reasoning,
        ]
    }

    pub(crate) fn name(&self) -> &'static str {
        match self {
            ModelOption::Default => "Default (recommended)",
            ModelOption::Mini => "Mini",
            ModelOption::Reasoning => "Reasoning",
        }
    }

    pub(crate) fn model_id(&self) -> &'static str {
        match self {
            ModelOption::Default => "gpt-5.2-codex",
            ModelOption::Mini => "gpt-5.1-codex-mini",
            ModelOption::Reasoning => "gpt-5.2",
        }
    }

    pub(crate) fn from_id(id: &str) -> ModelOption {
        match id {
            "gpt-5.2-codex" => ModelOption::Default,
            "gpt-5.1-codex-max" => ModelOption::Default,
            "gpt-5.1-codex-mini" => ModelOption::Mini,
            "gpt-5.2" => ModelOption::Reasoning,
            // Legacy fallbacks
            "gpt-4o" | "gpt-5" | "gpt-5.1" | "gpt-5-codex" | "gpt-5.1-codex" => {
                ModelOption::Default
            }
            "gpt-4o-mini" | "gpt-5-codex-mini" | "codex-mini-latest" => ModelOption::Mini,
            "o1" | "o3" => ModelOption::Reasoning,
            _ => ModelOption::Default,
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            ModelOption::Default => "GPT-5.2 Codex · Frontier coding model",
            ModelOption::Mini => "GPT-5.1 Codex Mini · Faster, lower cost",
            ModelOption::Reasoning => "GPT-5.2 · Latest frontier model",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ModelPickerEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
    #[allow(dead_code)]
    pub(crate) is_default: bool,
}

pub(crate) fn legacy_model_entries() -> Vec<ModelPickerEntry> {
    ModelOption::all()
        .iter()
        .map(|model| ModelPickerEntry {
            id: model.model_id().to_string(),
            name: model.name().to_string(),
            description: model.description().to_string(),
            is_default: matches!(model, ModelOption::Default),
        })
        .collect()
}

pub(crate) fn app_server_model_entries(
    models: &[crate::app::codex_app_server::ModelInfo],
) -> Vec<ModelPickerEntry> {
    if models.is_empty() {
        return legacy_model_entries();
    }
    models
        .iter()
        .map(|model| ModelPickerEntry {
            id: model.id.clone(),
            name: model.display_name.clone(),
            description: model.description.clone(),
            is_default: model.is_default,
        })
        .collect()
}
