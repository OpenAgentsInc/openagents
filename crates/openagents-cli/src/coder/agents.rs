//! Built-in agents available through the `delegate` tool.
//!
//! An agent definition is intentionally small: it selects a tool pool and
//! adds the report-back instruction for one fresh Coder runtime session. ACP
//! agents remain discovered at runtime and share the same `agent` parameter,
//! but their tools and system prompts belong to their own programs.

/// Which tools an in-process delegated agent can see.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolPool {
    ReadOnly,
    ReadWrite,
    All,
}

impl ToolPool {
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim() {
            "read-only" => Some(Self::ReadOnly),
            "read-write" => Some(Self::ReadWrite),
            "all" => Some(Self::All),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::ReadOnly => "read-only",
            Self::ReadWrite => "read-write",
            Self::All => "all",
        }
    }

    /// Whether this pool declares `name` to the delegated model.
    pub fn allows(self, name: &str) -> bool {
        match self {
            Self::ReadOnly => matches!(
                name,
                "read"
                    | "bash"
                    | "skill"
                    | "grep"
                    | "glob"
                    | "swarm_list"
                    | "swarm_inbox"
                    | "git_facts"
                    | "code_search"
                    | "repo_tree"
                    | "test_report"
                    | "session_search"
            ),
            Self::ReadWrite => matches!(
                name,
                "read"
                    | "write"
                    | "edit"
                    | "bash"
                    | "skill"
                    | "grep"
                    | "glob"
                    | "swarm_list"
                    | "swarm_send"
                    | "swarm_inbox"
                    | "git_facts"
                    | "code_search"
                    | "repo_tree"
                    | "test_report"
                    | "session_search"
            ),
            Self::All => true,
        }
    }

    pub fn prompt(self) -> &'static str {
        match self {
            Self::ReadOnly => {
                "Your tool pool is read-only. You can inspect files and run commands that the \
                 read-only shell policy admits. Do not claim that you changed files."
            }
            Self::ReadWrite => {
                "Your tool pool is read-write. You can inspect, edit, and verify the workspace."
            }
            Self::All => {
                "Your tool pool contains the parent session's tools except delegation. You cannot \
                 start another agent."
            }
        }
    }
}

/// One built-in delegated agent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AgentDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub pool: ToolPool,
    /// Catalog id this agent opens on when the call names no `model`.
    ///
    /// `None` inherits the parent session lane. A named id is pinned
    /// (`Lane::Named`) and refused if the live catalog does not serve it.
    pub default_model: Option<&'static str>,
    pub system_prompt: &'static str,
}

impl AgentDefinition {
    /// Lane for one call: explicit `model` argument, then this agent's
    /// default, then the parent session lane.
    pub fn lane(&self, model_arg: Option<&str>, parent_lane: &str) -> crate::runtime::Lane {
        if let Some(model) = model_arg.map(str::trim).filter(|value| !value.is_empty()) {
            return crate::runtime::Lane::from_str(model);
        }
        if let Some(default) = self.default_model {
            return crate::runtime::Lane::from_str(default);
        }
        crate::runtime::Lane::from_str(parent_lane)
    }
}

const MINI_PROMPT: &str = "You are an agent for Coder. Complete the task fully with the \
tools you have. Work independently from the self-contained prompt. Respond with a concise report \
that covers what you did and the findings the parent needs.";

const EXPLORE_PROMPT: &str = "You are Explore, a read-only investigator for Coder. Inspect the \
workspace and report findings. Do not edit, write, or otherwise change files. Do not load project \
skills or memory. Work independently from the self-contained prompt. Respond with a concise report \
the parent can act on.";

const PLAN_PROMPT: &str = "You are Plan, a read-only planning agent for Coder. Research the \
workspace only as needed to produce a plan. Do not edit, write, or otherwise change files. The \
report is the plan: what to do, in order, so the parent can accept it or hand it to `coder`.";

const CODER_PROMPT: &str = "You are Coder, a read-write agent for Coder. Complete the task fully \
with the tools you have. Work independently from the self-contained prompt. Respond with a concise \
report that covers what you did and the findings the parent needs.";

pub const BUILTIN_AGENTS: &[AgentDefinition] = &[
    AgentDefinition {
        id: "coder-mini",
        label: "Coder Mini",
        pool: ToolPool::ReadOnly,
        default_model: None,
        system_prompt: MINI_PROMPT,
    },
    AgentDefinition {
        id: "explore",
        label: "Explore",
        pool: ToolPool::ReadOnly,
        default_model: Some("gemini-3.7-flash"),
        system_prompt: EXPLORE_PROMPT,
    },
    AgentDefinition {
        id: "plan",
        label: "Plan",
        pool: ToolPool::ReadOnly,
        default_model: Some("glm-5.3-flash"),
        system_prompt: PLAN_PROMPT,
    },
    AgentDefinition {
        id: "coder",
        label: "Coder",
        pool: ToolPool::ReadWrite,
        default_model: None,
        system_prompt: CODER_PROMPT,
    },
];

pub fn find(id: &str) -> Option<&'static AgentDefinition> {
    BUILTIN_AGENTS.iter().find(|agent| agent.id == id)
}

pub fn system_prompt(agent: &AgentDefinition, pool: ToolPool, tool_names: &[String]) -> String {
    let tools = if tool_names.is_empty() {
        "You have no tools in this session.".to_string()
    } else {
        format!("Your tools are: {}.", tool_names.join(", "))
    };
    format!("{}\n\n{}\n\n{}", agent.system_prompt, pool.prompt(), tools)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_builtin_registry_has_stable_ids_pools_and_default_models() {
        assert_eq!(
            BUILTIN_AGENTS
                .iter()
                .map(|agent| (agent.id, agent.pool, agent.default_model))
                .collect::<Vec<_>>(),
            vec![
                ("coder-mini", ToolPool::ReadOnly, None),
                ("explore", ToolPool::ReadOnly, Some("gemini-3.7-flash")),
                ("plan", ToolPool::ReadOnly, Some("glm-5.3-flash")),
                ("coder", ToolPool::ReadWrite, None),
            ]
        );
        assert_ne!(
            find("explore").unwrap().system_prompt,
            find("coder-mini").unwrap().system_prompt
        );
        assert_ne!(
            find("plan").unwrap().system_prompt,
            find("explore").unwrap().system_prompt
        );
    }

    #[test]
    fn an_explicit_model_wins_over_the_agent_default_and_the_parent_lane() {
        let explore = find("explore").unwrap();
        assert_eq!(explore.lane(None, "flash").name(), "gemini-3.7-flash");
        assert_eq!(
            explore.lane(Some("glm-5.3-flash"), "flash").name(),
            "glm-5.3-flash"
        );
        let mini = find("coder-mini").unwrap();
        assert_eq!(mini.lane(None, "flash").name(), "flash");
        assert_eq!(
            mini.lane(Some("gemini-3.7-flash"), "flash").name(),
            "gemini-3.7-flash"
        );
        let plan = find("plan").unwrap();
        assert_eq!(plan.lane(None, "free").name(), "glm-5.3-flash");
    }

    #[test]
    fn defaults_match_the_shared_contract() {
        for agent in BUILTIN_AGENTS {
            let contract = openagents_coder_contract::BUILTIN_AGENTS
                .iter()
                .find(|row| row.id == agent.id)
                .unwrap();
            assert_eq!(agent.default_model, contract.default_model);
        }
    }
}
