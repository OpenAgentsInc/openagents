//! Host-confirmed choices for new-session menus. Cached and bundled catalogs are not offers.
use super::*;

fn offered(
    harnesses: Vec<HarnessInfo>,
    requested: Option<&str>,
) -> Result<(String, Vec<HarnessInfo>)> {
    if harnesses.len() > 64 {
        return Err(ClientError::HostError(
            "This computer returned too many providers.".into(),
        ));
    }
    let harnesses: Vec<_> = harnesses
        .into_iter()
        .filter(|h| h.offered() && h.id != "mock")
        .collect();
    let selected = match requested {
        Some(id) => harnesses.iter().find(|h| h.id == id),
        None => harnesses.first(),
    }
    .ok_or_else(|| {
        ClientError::HostUnavailable("No provider is available on this computer.".into())
    })?
    .id
    .clone();
    let mut ids = std::collections::BTreeSet::new();
    if harnesses.iter().any(|h| {
        h.id.is_empty()
            || h.id.len() > 128
            || h.label.len() > 256
            || h.reasoning_levels.len() > 32
            || !ids.insert(h.id.clone())
    }) {
        return Err(ClientError::HostError(
            "This computer returned an invalid provider list.".into(),
        ));
    }
    Ok((selected, harnesses))
}
fn models(list: Vec<ModelInfo>) -> Result<Vec<ModelInfo>> {
    if list.is_empty() || list.len() > 512 {
        return Err(ClientError::HostUnavailable(
            "No models are available on this computer.".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    if list.iter().any(|m| {
        m.id.is_empty()
            || m.id.len() > 256
            || m.label.len() > 256
            || m.description.as_ref().is_some_and(|text| text.len() > 4096)
            || m.reasoning_levels.len() > 32
            || m.reasoning_levels
                .iter()
                .any(|level| level.is_empty() || level.len() > 64)
            || m.reasoning_levels
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != m.reasoning_levels.len()
            || m.options.len() > 32
            || m.options.iter().any(|option| {
                option.id.is_empty()
                    || option.id.len() > 128
                    || option.label.len() > 256
                    || option.choices.is_empty()
                    || option.choices.len() > 32
                    || option.choices.iter().any(|choice| {
                        choice.id.is_empty() || choice.id.len() > 128 || choice.label.len() > 256
                    })
                    || !option
                        .choices
                        .iter()
                        .any(|choice| choice.id == option.default_choice)
            })
            || m.options
                .iter()
                .map(|option| &option.id)
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != m.options.len()
            || !ids.insert(m.id.clone())
    }) {
        return Err(ClientError::HostError(
            "This computer returned an invalid model list.".into(),
        ));
    }
    Ok(list)
}
impl Client {
    /// Read only the selected computer. Failure never substitutes another computer or cached list.
    pub async fn live_session_catalog(
        &self,
        device: &str,
        harness: Option<&str>,
    ) -> Result<(String, Vec<HarnessInfo>, Vec<ModelInfo>)> {
        match self.inner.backend() {
            Backend::Live(live) => {
                let value = tokio::time::timeout(
                    Duration::from_secs(20),
                    live.relay.call(
                        device,
                        openagents_rpc::methods::LIST_HARNESSES,
                        serde_json::json!({}),
                    ),
                )
                .await
                .map_err(|_| {
                    ClientError::HostUnavailable("Couldn't reach this computer. Try again.".into())
                })??;
                let list: Vec<HarnessInfo> = serde_json::from_value(value).map_err(|_| {
                    ClientError::HostError("Couldn't read this computer's providers.".into())
                })?;
                let (selected, harnesses) = offered(list, harness)?;
                let value = tokio::time::timeout(
                    Duration::from_secs(20),
                    live.relay.call(
                        device,
                        openagents_rpc::methods::LIST_MODELS,
                        serde_json::json!({"harness":selected}),
                    ),
                )
                .await
                .map_err(|_| {
                    ClientError::HostUnavailable("Couldn't reach this computer. Try again.".into())
                })??;
                let list: Vec<ModelInfo> = serde_json::from_value(value).map_err(|_| {
                    ClientError::HostError("Couldn't read this computer's models.".into())
                })?;
                Ok((selected, harnesses, models(list)?))
            }
            Backend::Demo(demo) => {
                let (selected, harnesses) = offered(demo.list_harnesses(device).await, harness)?;
                let list = models(demo.list_models(&selected).await)?;
                Ok((selected, harnesses, list))
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn provider(id: &str, enabled: bool) -> HarnessInfo {
        HarnessInfo {
            id: id.into(),
            label: id.into(),
            supports_steering: None,
            steering_mode: None,
            reasoning_levels: vec![],
            installed: true,
            enabled: Some(enabled),
        }
    }
    fn model(id: &str) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            label: id.into(),
            description: None,
            reasoning_levels: vec!["low".into(), "high".into()],
            options: vec![],
        }
    }
    #[test]
    fn requested_provider_must_be_offered_by_that_host() {
        assert!(offered(vec![provider("codex", true)], Some("claude-code")).is_err());
        assert!(offered(vec![provider("codex", false)], None).is_err());
        let (id, list) =
            offered(vec![provider("codex", true), provider("mock", true)], None).unwrap();
        assert_eq!(id, "codex");
        assert_eq!(list.len(), 1);
    }
    #[test]
    fn empty_or_duplicate_live_models_never_get_a_bundled_substitute() {
        assert!(models(vec![]).is_err());
        assert!(
            models(vec![
                model("synthetic-host-model"),
                model("synthetic-host-model")
            ])
            .is_err()
        );
        assert_eq!(
            models(vec![model("synthetic-host-model")]).unwrap()[0].id,
            "synthetic-host-model"
        );
    }
    #[test]
    fn duplicate_and_unbounded_catalogs_are_refused() {
        assert!(offered(vec![provider("codex", true), provider("codex", true)], None).is_err());
        assert!(models((0..513).map(|n| model(&format!("model-{n}"))).collect()).is_err());
    }

    #[test]
    fn malformed_effort_and_model_options_are_refused() {
        let mut duplicate_effort = model("fixture");
        duplicate_effort.reasoning_levels = vec!["low".into(), "low".into()];
        assert!(models(vec![duplicate_effort.clone()]).is_err());

        duplicate_effort.reasoning_levels = vec!["".into()];
        assert!(models(vec![duplicate_effort.clone()]).is_err());

        duplicate_effort.reasoning_levels.clear();
        duplicate_effort.options.push(crate::catalog::ModelOption {
            id: "mode".into(),
            label: "Mode".into(),
            choices: vec![crate::catalog::ModelOptionChoice {
                id: "safe".into(),
                label: "Safe".into(),
            }],
            default_choice: "missing".into(),
        });
        assert!(models(vec![duplicate_effort]).is_err());
    }
}
