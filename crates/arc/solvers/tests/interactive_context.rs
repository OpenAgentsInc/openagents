use std::path::PathBuf;

use arc_client::{ArcEnvironmentInfo, LocalArcEnvironment};
use arc_core::{ArcAction, ArcScorePolicyId, ArcTaskId};
use arc_solvers::{
    ArcInteractiveAgent, ArcInteractiveAgentError, ArcInteractiveContextRetentionPolicy,
    ArcInteractiveGameStep, ArcInteractivePromptPolicy, ArcInteractivePromptSection,
    ArcInteractivePromptSectionView, ArcInteractiveRunner, ArcInteractiveRunnerConfig,
    ArcInteractiveRunnerError, ArcInteractiveSessionContext,
};
use serde_json::json;

fn demo_package_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("engine")
        .join("fixtures")
        .join("demo_game.json")
}

fn demo_environment_info() -> ArcEnvironmentInfo {
    ArcEnvironmentInfo {
        game_id: ArcTaskId::new("arc-engine-demo").expect("task id should validate"),
        title: Some("Demo ARC".to_owned()),
        tags: vec!["interactive-context".to_owned()],
        private_tags: Vec::new(),
        level_tags: Vec::new(),
        baseline_actions: vec![7, 5],
        class_name: Some("DemoArcGame".to_owned()),
        local_package_path: None,
    }
}

fn local_environment(scorecard_id: &str) -> LocalArcEnvironment {
    LocalArcEnvironment::load_from_path(demo_environment_info(), demo_package_path(), scorecard_id)
        .expect("local environment should initialize")
}

fn winning_demo_sequence() -> Vec<ArcAction> {
    vec![
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::action6(22, 22).expect("coords should validate"),
        ArcAction::Action4,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action4,
        ArcAction::Action2,
        ArcAction::Action5,
    ]
}

fn runner_config(checkpoint_id: &str, max_agent_actions: u32) -> ArcInteractiveRunnerConfig {
    let mut config = ArcInteractiveRunnerConfig::new(
        checkpoint_id,
        ArcScorePolicyId::ArcAgi3MethodologyV1,
        max_agent_actions,
    )
    .expect("runner config should validate");
    config.metadata.tags = vec!["interactive-context".to_owned()];
    config
}

#[derive(Default)]
struct ContextProbeAgent {
    name: String,
    actions: Vec<ArcAction>,
    cursor: usize,
    seen: Vec<ArcInteractiveSessionContext>,
}

impl ContextProbeAgent {
    fn new(name: &str, actions: Vec<ArcAction>) -> Self {
        Self {
            name: name.to_owned(),
            actions,
            cursor: 0,
            seen: Vec::new(),
        }
    }
}

impl ArcInteractiveAgent for ContextProbeAgent {
    fn agent_name(&self) -> &str {
        &self.name
    }

    fn step(
        &mut self,
        context: &ArcInteractiveSessionContext,
    ) -> Result<ArcInteractiveGameStep, ArcInteractiveAgentError> {
        self.seen.push(context.clone());
        let action = self
            .actions
            .get(self.cursor)
            .cloned()
            .ok_or_else(|| ArcInteractiveAgentError::message("probe sequence exhausted"))?;
        self.cursor = self.cursor.saturating_add(1);
        Ok(ArcInteractiveGameStep::new(action).with_reasoning(json!({
            "cursor": self.cursor,
            "agent": self.name,
        })))
    }

    fn checkpoint_state(&self) -> Result<Option<serde_json::Value>, ArcInteractiveAgentError> {
        Ok(Some(json!({ "cursor": self.cursor })))
    }

    fn restore_checkpoint_state(
        &mut self,
        state: Option<&serde_json::Value>,
    ) -> Result<(), ArcInteractiveAgentError> {
        let cursor = state
            .and_then(|value| value.get("cursor"))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(0);
        self.cursor = usize::try_from(cursor).unwrap_or(usize::MAX);
        Ok(())
    }
}

#[test]
fn interactive_runner_exposes_bounded_context_memory_and_prompt_plan() {
    let environment = local_environment("interactive-context-window");
    let mut config = runner_config("interactive-context-window", 3);
    config.context_retention = ArcInteractiveContextRetentionPolicy::new("window-v1", 2, 2)
        .expect("retention policy should validate");
    config.prompt_policy = ArcInteractivePromptPolicy::new(
        "prompt-window-v1",
        vec![
            ArcInteractivePromptSection::CurrentObservation,
            ArcInteractivePromptSection::RetainedHistory,
            ArcInteractivePromptSection::SessionMemory,
        ],
    )
    .expect("prompt policy should validate");

    let mut runner = ArcInteractiveRunner::new(environment, config);
    let mut agent = ContextProbeAgent::new("context-probe", winning_demo_sequence()[..3].to_vec());
    let artifacts = runner
        .run_episode(&mut agent)
        .expect("bounded context run should complete");

    assert_eq!(agent.seen.len(), 3);
    assert_eq!(agent.seen[0].history.len(), 1);
    assert_eq!(agent.seen[0].memory.entries.len(), 0);
    assert_eq!(agent.seen[1].history.len(), 2);
    assert_eq!(agent.seen[1].memory.entries.len(), 1);
    assert_eq!(agent.seen[2].history.len(), 2);
    assert_eq!(agent.seen[2].omitted_history_steps, 1);
    assert_eq!(agent.seen[2].memory.entries.len(), 2);
    assert!(matches!(
        agent.seen[2].prompt_plan.sections.as_slice(),
        [
            ArcInteractivePromptSectionView::CurrentObservation { .. },
            ArcInteractivePromptSectionView::RetainedHistory { .. },
            ArcInteractivePromptSectionView::SessionMemory { .. }
        ]
    ));
    assert_eq!(
        agent.seen[1].memory.entries[0].reasoning,
        Some(json!({ "cursor": 1, "agent": "context-probe" }))
    );

    let context_state = artifacts
        .checkpoint_handoff
        .context_state
        .expect("checkpoint handoff should include retained context");
    assert_eq!(context_state.retention_policy.policy_id, "window-v1");
    assert_eq!(context_state.prompt_policy.policy_id, "prompt-window-v1");
    assert_eq!(context_state.history.len(), 2);
    assert_eq!(context_state.omitted_history_steps, 2);
    assert_eq!(context_state.memory.entries.len(), 2);
    assert_eq!(context_state.memory.omitted_entries, 1);
    assert_eq!(context_state.memory.entries[0].step_index, 2);
}

#[test]
fn interactive_runner_resume_restores_context_and_prompt_resume_summary() {
    let environment = local_environment("interactive-context-resume");
    let retention_policy = ArcInteractiveContextRetentionPolicy::new("resume-v1", 3, 3)
        .expect("retention policy should validate");
    let prompt_policy = ArcInteractivePromptPolicy::new(
        "resume-prompt-v1",
        vec![
            ArcInteractivePromptSection::SessionMemory,
            ArcInteractivePromptSection::ResumeSummary,
        ],
    )
    .expect("prompt policy should validate");

    let mut prefix_config = runner_config("interactive-context-resume-prefix", 2);
    prefix_config.context_retention = retention_policy.clone();
    prefix_config.prompt_policy = prompt_policy.clone();
    let mut prefix_runner = ArcInteractiveRunner::new(environment, prefix_config);
    let mut prefix_agent = ContextProbeAgent::new("context-probe", winning_demo_sequence());
    let prefix = prefix_runner
        .run_episode(&mut prefix_agent)
        .expect("prefix run should complete");

    let environment = prefix_runner.into_environment();
    let mut resume_config = runner_config("interactive-context-resume", 4);
    resume_config.context_retention = retention_policy;
    resume_config.prompt_policy = prompt_policy;
    resume_config.resume_state = Some(prefix.checkpoint_handoff.clone());
    let mut resume_runner = ArcInteractiveRunner::new(environment, resume_config);
    let mut resume_agent = ContextProbeAgent::new("context-probe", winning_demo_sequence());
    let resumed = resume_runner
        .run_episode(&mut resume_agent)
        .expect("resumed run should complete");

    assert!(!resume_agent.seen.is_empty());
    let first_context = &resume_agent.seen[0];
    assert_eq!(
        first_context.step_index,
        prefix.checkpoint_handoff.next_step_index
    );
    assert_eq!(
        first_context.actions_taken,
        prefix.checkpoint_handoff.actions_taken
    );
    assert_eq!(first_context.history.len(), 3);
    assert_eq!(first_context.memory.entries.len(), 2);
    assert!(matches!(
        first_context.prompt_plan.sections.as_slice(),
        [
            ArcInteractivePromptSectionView::SessionMemory { .. },
            ArcInteractivePromptSectionView::ResumeSummary { .. }
        ]
    ));
    assert_eq!(
        first_context.memory.entries[1].reasoning,
        Some(json!({ "cursor": 2, "agent": "context-probe" }))
    );

    let resumed_context = resumed
        .checkpoint_handoff
        .context_state
        .expect("resumed handoff should preserve context");
    assert_eq!(resumed_context.history.len(), 3);
    assert!(resumed_context.memory.entries.len() <= 3);
}

#[test]
fn interactive_runner_refuses_resume_when_context_policy_drifts() {
    let environment = local_environment("interactive-context-drift");
    let mut prefix_config = runner_config("interactive-context-drift-prefix", 2);
    prefix_config.context_retention = ArcInteractiveContextRetentionPolicy::new("drift-v1", 3, 3)
        .expect("retention policy should validate");
    prefix_config.prompt_policy = ArcInteractivePromptPolicy::new(
        "drift-prompt-v1",
        vec![ArcInteractivePromptSection::SessionMemory],
    )
    .expect("prompt policy should validate");
    let mut prefix_runner = ArcInteractiveRunner::new(environment, prefix_config);
    let mut prefix_agent = ContextProbeAgent::new("context-probe", winning_demo_sequence());
    let prefix = prefix_runner
        .run_episode(&mut prefix_agent)
        .expect("prefix run should complete");

    let environment = prefix_runner.into_environment();
    let mut resume_config = runner_config("interactive-context-drift", 4);
    resume_config.context_retention = ArcInteractiveContextRetentionPolicy::new("drift-v1", 3, 3)
        .expect("retention policy should validate");
    resume_config.prompt_policy = ArcInteractivePromptPolicy::new(
        "drift-prompt-v2",
        vec![
            ArcInteractivePromptSection::SessionMemory,
            ArcInteractivePromptSection::ResumeSummary,
        ],
    )
    .expect("prompt policy should validate");
    resume_config.resume_state = Some(prefix.checkpoint_handoff);
    let mut resume_runner = ArcInteractiveRunner::new(environment, resume_config);
    let mut resume_agent = ContextProbeAgent::new("context-probe", winning_demo_sequence());
    let error = resume_runner
        .run_episode(&mut resume_agent)
        .expect_err("policy drift should refuse resume");
    match error {
        ArcInteractiveRunnerError::IncompatibleResumeContext { detail } => {
            assert!(detail.contains("prompt policy"));
        }
        other => panic!("unexpected resume error: {other:?}"),
    }
}
