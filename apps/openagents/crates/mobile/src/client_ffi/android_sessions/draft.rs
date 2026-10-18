//! New-session choices stay scoped to one account, computer, and successful catalog request.
use super::*;
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Menu {
    Home,
    Computers,
    Projects,
    Harnesses,
    Models,
    Efforts,
    Branches,
    Folders,
}
pub(super) struct Draft {
    pub generation: u64,
    pub token: u64,
    pub device: Option<String>,
    pub project: Option<ProjectView>,
    pub branch: Option<RepoRef>,
    pub harness: Option<String>,
    pub models: Vec<ModelInfo>,
    pub harnesses: Vec<HarnessInfo>,
    pub model: Option<String>,
    pub effort: Option<String>,
}
impl Draft {
    pub fn new(generation: u64) -> Self {
        Self {
            generation,
            token: 0,
            device: None,
            project: None,
            branch: None,
            harness: None,
            models: vec![],
            harnesses: vec![],
            model: None,
            effort: None,
        }
    }
    pub fn computer(&mut self, device: String) {
        self.token += 1;
        self.device = Some(device);
        self.project = None;
        self.branch = None;
        self.harness = None;
        self.models.clear();
        self.harnesses.clear();
        self.model = None;
        self.effort = None;
    }
    pub fn catalog(
        &mut self,
        generation: u64,
        token: u64,
        device: &str,
        harness: String,
        harnesses: Vec<HarnessInfo>,
        models: Vec<ModelInfo>,
    ) -> Result<(), CoreError> {
        if self.generation != generation
            || self.token != token
            || self.device.as_deref() != Some(device)
        {
            return Err(CoreError::Closed);
        }
        if !harnesses.iter().any(|h| h.id == harness && h.offered) || models.is_empty() {
            return Err(failed("No models are available on this computer."));
        }
        self.harness = Some(harness);
        self.harnesses = harnesses;
        self.models = models;
        if !self
            .models
            .iter()
            .any(|m| Some(&m.id) == self.model.as_ref())
        {
            self.model = None;
            self.effort = None;
        }
        if self.effort.as_ref().is_some_and(|e| {
            !self
                .selected()
                .is_some_and(|m| m.reasoning_levels.contains(e))
        }) {
            self.effort = None;
        }
        Ok(())
    }
    pub fn selected(&self) -> Option<&ModelInfo> {
        self.models
            .iter()
            .find(|m| Some(&m.id) == self.model.as_ref())
    }
    pub fn config(&self) -> Result<ChatConfig, CoreError> {
        let model = self
            .selected()
            .ok_or_else(|| failed("Choose a model from this computer."))?;
        if self
            .effort
            .as_ref()
            .is_some_and(|e| !model.reasoning_levels.contains(e))
        {
            return Err(failed("Choose an effort from this model."));
        }
        Ok(ChatConfig {
            harness: self.harness.clone().ok_or(CoreError::Closed)?,
            model: Some(model.id.clone()),
            reasoning: self.effort.clone(),
            model_options: Default::default(),
            sandbox: SandboxLevel::ReadOnly,
        })
    }
    pub fn session(&self) -> Result<NewSession, CoreError> {
        let device = self
            .device
            .as_ref()
            .ok_or_else(|| failed("Choose a computer."))?;
        let target = if let Some(project) = &self.project {
            if project.device_id != *device {
                return Err(CoreError::Closed);
            }
            SessionTarget::Project {
                space_id: project.id.clone(),
            }
        } else {
            SessionTarget::Projectless {
                device_id: device.clone(),
            }
        };
        Ok(NewSession {
            target,
            config: Some(self.config()?),
            branch: self.branch.as_ref().map(|b| b.name.clone()),
            cwd: self.branch.as_ref().and_then(|b| b.worktree_path.clone()),
            title: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn harness() -> HarnessInfo {
        HarnessInfo {
            id: "codex".into(),
            label: "Fixture provider".into(),
            supports_steering: None,
            steering_mode: None,
            reasoning_levels: vec![],
            installed: true,
            enabled: Some(true),
            offered: true,
            mid_turn_steering: None,
        }
    }
    fn model(id: &str, levels: &[&str]) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            label: id.into(),
            description: None,
            reasoning_levels: levels.iter().map(|s| s.to_string()).collect(),
            options: vec![],
            default_reasoning: None,
        }
    }
    #[test]
    fn delayed_catalog_from_previous_computer_or_account_is_rejected() {
        let mut d = Draft::new(7);
        d.computer("a".into());
        let old = d.token;
        d.computer("b".into());
        assert!(
            d.catalog(
                7,
                old,
                "a",
                "codex".into(),
                vec![harness()],
                vec![model("a-only", &[])]
            )
            .is_err()
        );
        assert!(
            d.catalog(
                8,
                d.token,
                "b",
                "codex".into(),
                vec![harness()],
                vec![model("b-only", &[])]
            )
            .is_err()
        );
        assert!(d.models.is_empty());
    }
    #[test]
    fn only_live_offered_models_and_efforts_can_create_a_session() {
        let mut d = Draft::new(1);
        d.computer("host".into());
        assert!(d.session().is_err());
        d.catalog(
            1,
            d.token,
            "host",
            "codex".into(),
            vec![harness()],
            vec![model("host-only", &["low", "high"])],
        )
        .unwrap();
        d.model = Some("invented".into());
        assert!(d.session().is_err());
        d.model = Some("host-only".into());
        d.effort = Some("invented".into());
        assert!(d.session().is_err());
        d.effort = Some("high".into());
        let n = d.session().unwrap();
        assert!(matches!(n.target,SessionTarget::Projectless{device_id} if device_id=="host"));
        assert_eq!(n.config.unwrap().reasoning.as_deref(), Some("high"));
    }
    #[test]
    fn host_switch_discards_branch_project_and_incompatible_model_state() {
        let mut d = Draft::new(1);
        d.computer("a".into());
        d.model = Some("a-only".into());
        d.effort = Some("high".into());
        d.branch = Some(RepoRef {
            name: "fixture".into(),
            current: false,
            worktree_path: Some("/synthetic/tree".into()),
        });
        d.computer("b".into());
        assert!(
            d.branch.is_none() && d.project.is_none() && d.model.is_none() && d.effort.is_none()
        );
    }
    #[test]
    fn missing_catalog_cannot_make_a_default_model_choice() {
        let mut d = Draft::new(1);
        d.computer("host".into());
        assert!(
            d.catalog(1, d.token, "host", "codex".into(), vec![harness()], vec![])
                .is_err()
        );
        assert!(d.model.is_none());
        assert!(d.config().is_err());
    }
}
