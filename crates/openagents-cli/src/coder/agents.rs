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
            Self::ReadOnly => matches!(name, "read" | "bash" | "skill" | "grep" | "glob"),
            Self::ReadWrite => matches!(
                name,
                "read" | "write" | "edit" | "bash" | "shell" | "skill" | "grep" | "glob"
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
    pub system_prompt: &'static str,
}

const REPORT_BACK_PROMPT: &str = "You are an agent for Coder. Complete the task fully with the \
tools you have. Work independently from the self-contained prompt. Respond with a concise report \
that covers what you did and the findings the parent needs.";

pub const BUILTIN_AGENTS: &[AgentDefinition] = &[
    AgentDefinition {
        id: "coder-mini",
        label: "Coder Mini",
        pool: ToolPool::ReadOnly,
        system_prompt: REPORT_BACK_PROMPT,
    },
    AgentDefinition {
        id: "explore",
        label: "Explore",
        pool: ToolPool::ReadOnly,
        system_prompt: REPORT_BACK_PROMPT,
    },
    AgentDefinition {
        id: "coder",
        label: "Coder",
        pool: ToolPool::ReadWrite,
        system_prompt: REPORT_BACK_PROMPT,
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
    fn the_builtin_registry_has_stable_ids_and_default_pools() {
        assert_eq!(
            BUILTIN_AGENTS
                .iter()
                .map(|agent| (agent.id, agent.pool))
                .collect::<Vec<_>>(),
            vec![
                ("coder-mini", ToolPool::ReadOnly),
                ("explore", ToolPool::ReadOnly),
                ("coder", ToolPool::ReadWrite),
            ]
        );
    }
}
