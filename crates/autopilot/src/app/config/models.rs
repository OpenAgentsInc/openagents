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
