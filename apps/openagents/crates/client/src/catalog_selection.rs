//! Draft choices from a successfully discovered host catalog.
use crate::catalog::{HarnessInfo, ModelInfo};
use openagents_proto::ChatConfig;

pub(crate) fn harness<'a>(config: &ChatConfig, offered: &'a [HarnessInfo]) -> Option<&'a str> {
    let current = serde_json::to_value(config.harness).ok()?;
    offered
        .iter()
        .filter(|h| h.offered() && h.id != "mock")
        .find(|h| Some(h.id.as_str()) == current.as_str())
        .or_else(|| offered.iter().find(|h| h.offered() && h.id != "mock"))
        .map(|h| h.id.as_str())
}

pub(crate) fn config(
    config: &ChatConfig,
    harness: &str,
    models: &[ModelInfo],
) -> Option<ChatConfig> {
    let selected = serde_json::from_value(serde_json::Value::String(harness.into())).ok()?;
    if selected == config.harness
        && config
            .model
            .as_ref()
            .is_none_or(|id| models.iter().any(|m| &m.id == id))
    {
        return Some(config.clone());
    }
    let model = models.first()?;
    let mut next = config.clone();
    next.harness = selected;
    next.model = Some(model.id.clone());
    next.reasoning = None;
    next.model_options.clear();
    Some(next)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::catalog::{fallback_harnesses, fallback_models};

    fn draft() -> ChatConfig {
        serde_json::from_value(serde_json::json!({
            "harness": "claude-code", "model": "claude-opus-5-5", "reasoning": "high",
            "modelOptions": {"contextWindow": "1m"}, "sandbox": "read-only"
        }))
        .unwrap()
    }

    #[test]
    fn supported_draft_and_host_default_keep_their_settings() {
        let mut draft = draft();
        let models = fallback_models("claude-code");
        assert_eq!(config(&draft, "claude-code", &models), Some(draft.clone()));
        draft.model = None;
        assert_eq!(config(&draft, "claude-code", &models), Some(draft));
    }

    #[test]
    fn host_switch_uses_offered_harness_and_discards_incompatible_options() {
        let draft = draft();
        let offered: Vec<_> = fallback_harnesses()
            .into_iter()
            .filter(|h| h.id == "opencode")
            .collect();
        let id = harness(&draft, &offered).unwrap();
        assert_eq!(id, "opencode");
        let models = fallback_models(id);
        let next = config(&draft, id, &models).unwrap();
        assert_eq!(next.model.as_deref(), Some(models[0].id.as_str()));
        assert!(next.reasoning.is_none());
        assert!(next.model_options.is_empty());
        assert_eq!(next.sandbox, draft.sandbox);
    }

    #[test]
    fn unsupported_model_changes_but_empty_catalog_does_not_guess() {
        let draft = draft();
        assert!(config(&draft, "opencode", &[]).is_none());
        assert!(harness(&draft, &[]).is_none());
        let mut models = fallback_models("claude-code");
        models.retain(|m| Some(&m.id) != draft.model.as_ref());
        let next = config(&draft, "claude-code", &models).unwrap();
        assert_eq!(next.model.as_deref(), Some(models[0].id.as_str()));
        assert!(next.reasoning.is_none());
    }
}
