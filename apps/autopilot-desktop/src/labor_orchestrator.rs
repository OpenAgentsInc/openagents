use std::path::PathBuf;

use codex_client::{AskForApproval, SandboxPolicy, TurnStartParams, UserInput};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodexRunTrigger {
    PersonalAgent,
    AutonomousGoal {
        goal_id: String,
        goal_title: String,
    },
    LaborMarket {
        work_unit_id: String,
        contract_id: Option<String>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum CodexRunClassification {
    PersonalAgent,
    AutonomousGoal {
        goal_id: String,
        goal_title: String,
    },
    LaborMarket {
        work_unit_id: String,
        contract_id: Option<String>,
    },
}

impl CodexRunClassification {
    pub(crate) fn from_trigger(trigger: CodexRunTrigger) -> Self {
        match trigger {
            CodexRunTrigger::PersonalAgent => Self::PersonalAgent,
            CodexRunTrigger::AutonomousGoal {
                goal_id,
                goal_title,
            } => Self::AutonomousGoal {
                goal_id,
                goal_title,
            },
            CodexRunTrigger::LaborMarket {
                work_unit_id,
                contract_id,
            } => Self::LaborMarket {
                work_unit_id,
                contract_id,
            },
        }
    }

    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::PersonalAgent => "personal_agent",
            Self::AutonomousGoal { .. } => "autonomous_goal",
            Self::LaborMarket { .. } => "labor_market",
        }
    }

    pub(crate) fn is_economically_meaningful(&self) -> bool {
        !matches!(self, Self::PersonalAgent)
    }

    pub(crate) fn is_labor_market_bound(&self) -> bool {
        matches!(self, Self::LaborMarket { .. })
    }

    pub(crate) fn timeline_descriptor(&self) -> String {
        let mut descriptor = format!(
            "class={} economic={} labor_bound={}",
            self.label(),
            self.is_economically_meaningful(),
            self.is_labor_market_bound()
        );
        match self {
            Self::PersonalAgent => {}
            Self::AutonomousGoal {
                goal_id,
                goal_title,
            } => {
                descriptor.push_str(&format!(" goal_id={goal_id} goal_title={goal_title:?}"));
            }
            Self::LaborMarket {
                work_unit_id,
                contract_id,
            } => {
                descriptor.push_str(&format!(
                    " work_unit_id={work_unit_id} contract_id={contract_id:?}"
                ));
            }
        }
        descriptor
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CodexTurnExecutionRequest {
    pub trigger: CodexRunTrigger,
    pub thread_id: String,
    pub input: Vec<UserInput>,
    pub cwd: Option<PathBuf>,
    pub approval_policy: Option<AskForApproval>,
    pub sandbox_policy: Option<SandboxPolicy>,
    pub model: Option<String>,
}

#[derive(Debug)]
pub(crate) struct CodexTurnExecutionPlan {
    pub classification: CodexRunClassification,
    pub command: crate::codex_lane::CodexLaneCommand,
}

pub(crate) fn orchestrate_codex_turn(request: CodexTurnExecutionRequest) -> CodexTurnExecutionPlan {
    let classification = CodexRunClassification::from_trigger(request.trigger);
    let command = crate::codex_lane::CodexLaneCommand::TurnStart(TurnStartParams {
        thread_id: request.thread_id,
        input: request.input,
        cwd: request.cwd,
        approval_policy: request.approval_policy,
        sandbox_policy: request.sandbox_policy,
        model: request.model,
        effort: None,
        summary: None,
        personality: None,
        output_schema: None,
        collaboration_mode: None,
    });

    CodexTurnExecutionPlan {
        classification,
        command,
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use codex_client::{AskForApproval, SandboxPolicy, UserInput};

    use super::{
        CodexRunClassification, CodexRunTrigger, CodexTurnExecutionRequest, orchestrate_codex_turn,
    };

    #[test]
    fn personal_agent_runs_remain_local_only() {
        let classification = CodexRunClassification::from_trigger(CodexRunTrigger::PersonalAgent);

        assert_eq!(classification.label(), "personal_agent");
        assert!(!classification.is_economically_meaningful());
        assert!(!classification.is_labor_market_bound());
    }

    #[test]
    fn autonomous_goal_runs_are_economic_but_not_yet_labor_market_bound() {
        let classification =
            CodexRunClassification::from_trigger(CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-earn".to_string(),
                goal_title: "Earn bitcoin".to_string(),
            });

        assert_eq!(classification.label(), "autonomous_goal");
        assert!(classification.is_economically_meaningful());
        assert!(!classification.is_labor_market_bound());
    }

    #[test]
    fn labor_market_runs_are_explicitly_labor_bound() {
        let classification = CodexRunClassification::from_trigger(CodexRunTrigger::LaborMarket {
            work_unit_id: "wu-1".to_string(),
            contract_id: Some("contract-1".to_string()),
        });

        assert_eq!(classification.label(), "labor_market");
        assert!(classification.is_economically_meaningful());
        assert!(classification.is_labor_market_bound());
    }

    #[test]
    fn orchestration_preserves_turn_start_params() {
        let request = CodexTurnExecutionRequest {
            trigger: CodexRunTrigger::AutonomousGoal {
                goal_id: "goal-42".to_string(),
                goal_title: "Close accounting loop".to_string(),
            },
            thread_id: "thread-123".to_string(),
            input: vec![UserInput::Text {
                text: "close the books".to_string(),
                text_elements: Vec::new(),
            }],
            cwd: Some(PathBuf::from("/tmp/work")),
            approval_policy: Some(AskForApproval::Never),
            sandbox_policy: Some(SandboxPolicy::DangerFullAccess),
            model: Some("gpt-5.2-codex".to_string()),
        };

        let plan = orchestrate_codex_turn(request);

        assert_eq!(
            plan.classification,
            CodexRunClassification::AutonomousGoal {
                goal_id: "goal-42".to_string(),
                goal_title: "Close accounting loop".to_string(),
            }
        );
        let crate::codex_lane::CodexLaneCommand::TurnStart(params) = plan.command else {
            panic!("expected TurnStart command");
        };
        assert_eq!(params.thread_id, "thread-123");
        assert_eq!(params.model.as_deref(), Some("gpt-5.2-codex"));
        assert_eq!(params.cwd, Some(PathBuf::from("/tmp/work")));
        assert!(matches!(
            params.approval_policy,
            Some(AskForApproval::Never)
        ));
        assert_eq!(
            params.sandbox_policy,
            Some(codex_client::SandboxPolicy::DangerFullAccess)
        );
        assert_eq!(params.input.len(), 1);
    }
}
