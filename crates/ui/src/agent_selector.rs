//! AgentSelector component.
//!
//! Dropdown selector for choosing between AI coding agents (Claude, Codex).
//! Shows availability status and allows switching the active agent.

use maud::{Markup, PreEscaped, html};

/// Available agent types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentType {
    #[default]
    Claude,
    Codex,
    GptOss,
}

impl AgentType {
    /// Get the display name for this agent
    pub fn display_name(&self) -> &'static str {
        match self {
            AgentType::Claude => "Claude",
            AgentType::Codex => "Codex",
            AgentType::GptOss => "GPT-OSS",
        }
    }

    /// Get the API identifier for this agent
    pub fn id(&self) -> &'static str {
        match self {
            AgentType::Claude => "claude",
            AgentType::Codex => "codex",
            AgentType::GptOss => "gpt-oss",
        }
    }

    /// Parse from string
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "claude" => Some(AgentType::Claude),
            "codex" => Some(AgentType::Codex),
            "gpt-oss" | "gptoss" => Some(AgentType::GptOss),
            _ => None,
        }
    }
}

/// Information about an available agent
#[derive(Debug, Clone)]
pub struct AgentInfo {
    pub agent_type: AgentType,
    pub available: bool,
}

impl AgentInfo {
    pub fn new(agent_type: AgentType, available: bool) -> Self {
        Self {
            agent_type,
            available,
        }
    }
}

/// Agent selector dropdown component
pub struct AgentSelector {
    selected: AgentType,
    agents: Vec<AgentInfo>,
}

impl AgentSelector {
    /// Create a new agent selector with default agents
    pub fn new(selected: AgentType) -> Self {
        Self {
            selected,
            agents: vec![
                AgentInfo::new(AgentType::Claude, true),
                AgentInfo::new(AgentType::Codex, false),
                AgentInfo::new(AgentType::GptOss, false),
            ],
        }
    }

    /// Set the available agents
    pub fn agents(mut self, agents: Vec<AgentInfo>) -> Self {
        self.agents = agents;
        self
    }

    /// Build the component HTML
    pub fn build(self) -> Markup {
        let selected_name = self.selected.display_name().to_uppercase();
        let selected_available = self
            .agents
            .iter()
            .find(|a| a.agent_type == self.selected)
            .map(|a| a.available)
            .unwrap_or(false);

        let status_color = if selected_available {
            "#00A645"
        } else {
            "#FF0000"
        };

        html! {
            div
                id="agent-selector"
                style="position: relative;"
            {
                // Main button (collapsed state)
                button
                    type="button"
                    onclick="document.getElementById('agent-dropdown').classList.toggle('hidden')"
                    style="
                        display: inline-flex;
                        align-items: center;
                        gap: 0.5rem;
                        cursor: pointer;
                        font-family: 'Vera Mono', ui-monospace, monospace;
                        font-size: 0.75rem;
                        background: #111;
                        border: 1px solid #333;
                        padding: 0.4rem 0.6rem;
                        color: inherit;
                    "
                {
                    // Status dot
                    span style={
                        "width: 6px; height: 6px; display: inline-block; background: " (status_color) ";"
                    } {}

                    // Label
                    span style="color: #52525b;" { "AGENT:" }

                    // Selected agent name
                    span style="color: #fafafa; font-weight: 600;" {
                        (selected_name)
                    }

                    // Chevron down
                    span style="color: #52525b; font-size: 0.6rem;" { "▾" }
                }

                // Dropdown menu (hidden by default)
                div
                    id="agent-dropdown"
                    class="hidden"
                    style="
                        position: absolute;
                        top: 100%;
                        left: 0;
                        margin-top: 2px;
                        background: #111;
                        border: 1px solid #333;
                        min-width: 140px;
                        z-index: 100;
                    "
                {
                    @for agent in &self.agents {
                        @let is_selected = agent.agent_type == self.selected;
                        @let dot_color = if agent.available { "#00A645" } else { "#FF0000" };
                        @let agent_id = agent.agent_type.id();

                        button
                            type="button"
                            hx-post="/api/agents/select"
                            hx-vals=(format!(r#"{{"agent": "{}"}}"#, agent_id))
                            hx-target="#agent-selector"
                            hx-swap="outerHTML"
                            disabled[!agent.available]
                            onclick="document.getElementById('agent-dropdown').classList.add('hidden')"
                            style={
                                "display: flex; align-items: center; gap: 0.5rem; width: 100%; "
                                "padding: 0.4rem 0.6rem; border: none; background: none; "
                                "font-family: 'Vera Mono', ui-monospace, monospace; font-size: 0.7rem; "
                                "cursor: " (if agent.available { "pointer" } else { "not-allowed" }) "; "
                                "text-align: left; "
                                @if is_selected { "background: #222; " }
                            }
                        {
                            // Status dot
                            span style={
                                "width: 6px; height: 6px; display: inline-block; background: " (dot_color) ";"
                            } {}

                            // Agent name
                            span style={
                                "color: " (if agent.available { "#fafafa" } else { "#666" }) ";"
                            } {
                                (agent.agent_type.display_name())
                            }

                            // Checkmark for selected
                            @if is_selected {
                                span style="color: #00A645; margin-left: auto;" { "✓" }
                            }
                        }
                    }
                }

                // Close dropdown when clicking outside
                (PreEscaped(r#"<script>
document.addEventListener('click', function(e) {
    const selector = document.getElementById('agent-selector');
    const dropdown = document.getElementById('agent-dropdown');
    if (selector && dropdown && !selector.contains(e.target)) {
        dropdown.classList.add('hidden');
    }
});
</script>"#))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_agent_type_display() {
        assert_eq!(AgentType::Claude.display_name(), "Claude");
        assert_eq!(AgentType::Codex.display_name(), "Codex");
        assert_eq!(AgentType::GptOss.display_name(), "GPT-OSS");
    }

    #[test]
    fn test_agent_type_id() {
        assert_eq!(AgentType::Claude.id(), "claude");
        assert_eq!(AgentType::Codex.id(), "codex");
        assert_eq!(AgentType::GptOss.id(), "gpt-oss");
    }

    #[test]
    fn test_agent_type_from_str() {
        assert_eq!(AgentType::from_str("claude"), Some(AgentType::Claude));
        assert_eq!(AgentType::from_str("CLAUDE"), Some(AgentType::Claude));
        assert_eq!(AgentType::from_str("codex"), Some(AgentType::Codex));
        assert_eq!(AgentType::from_str("gpt-oss"), Some(AgentType::GptOss));
        assert_eq!(AgentType::from_str("gptoss"), Some(AgentType::GptOss));
        assert_eq!(AgentType::from_str("unknown"), None);
    }

    #[test]
    fn test_selector_build() {
        let selector = AgentSelector::new(AgentType::Claude);
        let html = selector.build().into_string();

        assert!(html.contains("agent-selector"));
        assert!(html.contains("CLAUDE"));
        assert!(html.contains("agent-dropdown"));
    }

    #[test]
    fn test_selector_with_custom_agents() {
        let agents = vec![
            AgentInfo::new(AgentType::Claude, true),
            AgentInfo::new(AgentType::Codex, true),
        ];
        let selector = AgentSelector::new(AgentType::Codex).agents(agents);
        let html = selector.build().into_string();

        assert!(html.contains("CODEX"));
    }
}
