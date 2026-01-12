#[derive(Clone, Copy, PartialEq, Debug)]
pub(crate) enum ModelOption {
    Default,
    Mini,
    Reasoning,
}

impl ModelOption {
    pub(crate) fn all() -> [ModelOption; 3] {
        [ModelOption::Default, ModelOption::Mini, ModelOption::Reasoning]
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
            ModelOption::Default => "gpt-4o",
            ModelOption::Mini => "gpt-4o-mini",
            ModelOption::Reasoning => "o1",
        }
    }

    pub(crate) fn from_id(id: &str) -> ModelOption {
        match id {
            "gpt-4o" => ModelOption::Default,
            "gpt-4o-mini" => ModelOption::Mini,
            "o1" => ModelOption::Reasoning,
            _ => ModelOption::Default, // Default fallback
        }
    }

    pub(crate) fn description(&self) -> &'static str {
        match self {
            ModelOption::Default => "GPT-4o · General purpose",
            ModelOption::Mini => "GPT-4o Mini · Faster, lower cost",
            ModelOption::Reasoning => "O1 · Reasoning focused",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ModelPickerEntry {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) description: String,
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
