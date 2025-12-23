//! Agent selector component for choosing between Claude Code and Codex

use maud::{html, Markup};
use serde::{Deserialize, Serialize};

/// Available AI agents
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Agent {
    Claude,
    Codex,
}

impl Agent {
    /// Get human-readable display name
    pub fn display_name(&self) -> &'static str {
        match self {
            Agent::Claude => "Claude Code",
            Agent::Codex => "Codex",
        }
    }

    /// Get identifier string
    pub fn id(&self) -> &'static str {
        match self {
            Agent::Claude => "claude",
            Agent::Codex => "codex",
        }
    }

    /// Get icon/emoji
    pub fn icon(&self) -> &'static str {
        match self {
            Agent::Claude => "🤖",
            Agent::Codex => "⚡",
        }
    }

    /// Get description
    pub fn description(&self) -> &'static str {
        match self {
            Agent::Claude => "Anthropic's Claude AI assistant with advanced reasoning",
            Agent::Codex => "OpenAI's code-specialized AI assistant",
        }
    }

    /// Get supported models
    pub fn supported_models(&self) -> Vec<&'static str> {
        match self {
            Agent::Claude => vec![
                "claude-sonnet-4-5-20250929",
                "claude-opus-4-5-20251101",
                "claude-haiku-4-20250514",
            ],
            Agent::Codex => vec![
                "gpt-4o",
                "gpt-4o-mini",
                "o1",
                "o1-mini",
            ],
        }
    }

    /// Check if agent is available (executable found)
    pub fn is_available(&self) -> bool {
        match self {
            Agent::Claude => which::which("claude").is_ok(),
            Agent::Codex => which::which("codex").is_ok(),
        }
    }
}

impl Default for Agent {
    fn default() -> Self {
        Agent::Claude
    }
}

impl std::str::FromStr for Agent {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "claude" => Ok(Agent::Claude),
            "codex" => Ok(Agent::Codex),
            _ => Err(format!("Unknown agent: {}", s)),
        }
    }
}

impl std::fmt::Display for Agent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.id())
    }
}

/// Agent selector dropdown component
pub fn agent_selector(current_agent: Agent) -> Markup {
    let agents = [Agent::Claude, Agent::Codex];

    html! {
        div class="agent-selector" {
            style {
                r#"
                .agent-selector {
                    background: #2a2a2a;
                    border: 1px solid #3a3a3a;
                    padding: 1rem;
                    margin-bottom: 1rem;
                }
                .agent-selector h3 {
                    color: #4a9eff;
                    margin-bottom: 0.75rem;
                    font-size: 1rem;
                }
                .agent-options {
                    display: grid;
                    grid-template-columns: 1fr 1fr;
                    gap: 0.75rem;
                }
                .agent-option {
                    background: #1a1a1a;
                    border: 2px solid #3a3a3a;
                    padding: 1rem;
                    cursor: pointer;
                    transition: all 0.2s;
                }
                .agent-option:hover {
                    border-color: #4a9eff;
                    background: #252525;
                }
                .agent-option.selected {
                    border-color: #4a9eff;
                    background: #1a3a5a;
                }
                .agent-option.unavailable {
                    opacity: 0.5;
                    cursor: not-allowed;
                }
                .agent-option.unavailable:hover {
                    border-color: #3a3a3a;
                    background: #1a1a1a;
                }
                .agent-header {
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                    margin-bottom: 0.5rem;
                }
                .agent-icon {
                    font-size: 1.5rem;
                }
                .agent-name {
                    color: #e0e0e0;
                    font-weight: 600;
                    font-size: 1rem;
                }
                .agent-status {
                    margin-left: auto;
                    font-size: 0.75rem;
                    padding: 0.2rem 0.5rem;
                }
                .agent-status.available {
                    background: #2d5016;
                    color: #7dff7d;
                }
                .agent-status.unavailable {
                    background: #5d1616;
                    color: #ff7d7d;
                }
                .agent-description {
                    color: #a0a0a0;
                    font-size: 0.85rem;
                    margin-bottom: 0.5rem;
                }
                .agent-models {
                    display: flex;
                    flex-wrap: wrap;
                    gap: 0.25rem;
                }
                .model-tag {
                    background: #3a3a3a;
                    color: #a0a0a0;
                    font-size: 0.7rem;
                    padding: 0.15rem 0.4rem;
                    font-family: monospace;
                }
                @media (max-width: 768px) {
                    .agent-options {
                        grid-template-columns: 1fr;
                    }
                }
                "#
            }

            h3 { "Select AI Agent" }
            div class="agent-options" {
                @for agent in &agents {
                    @let is_selected = *agent == current_agent;
                    @let is_available = agent.is_available();
                    @let classes = format!(
                        "agent-option{}{}",
                        if is_selected { " selected" } else { "" },
                        if !is_available { " unavailable" } else { "" }
                    );

                    div
                        class=(classes)
                        onclick=(format!(
                            "{}",
                            if is_available {
                                format!("selectAgent('{}')", agent.id())
                            } else {
                                "".to_string()
                            }
                        ))
                        {
                        div class="agent-header" {
                            span class="agent-icon" { (agent.icon()) }
                            span class="agent-name" { (agent.display_name()) }
                            span class=(format!(
                                "agent-status {}",
                                if is_available { "available" } else { "unavailable" }
                            )) {
                                @if is_available {
                                    "✓ Available"
                                } @else {
                                    "✗ Not Installed"
                                }
                            }
                        }
                        div class="agent-description" {
                            (agent.description())
                        }
                        div class="agent-models" {
                            @for model in agent.supported_models().iter().take(2) {
                                span class="model-tag" { (*model) }
                            }
                            @if agent.supported_models().len() > 2 {
                                span class="model-tag" {
                                    (format!("+{} more", agent.supported_models().len() - 2))
                                }
                            }
                        }
                    }
                }
            }

            script {
                r#"
                async function selectAgent(agentId) {
                    try {
                        const response = await fetch('/api/agent/select', {
                            method: 'POST',
                            headers: {
                                'Content-Type': 'application/json',
                            },
                            body: JSON.stringify({ agent: agentId }),
                        });

                        if (response.ok) {
                            // Reload page to update UI
                            window.location.reload();
                        } else {
                            const error = await response.json();
                            console.error('Failed to select agent:', error);
                            alert('Failed to select agent: ' + (error.error || 'Unknown error'));
                        }
                    } catch (error) {
                        console.error('Failed to select agent:', error);
                        alert('Failed to select agent: ' + error.message);
                    }
                }
                "#
            }
        }
    }
}

/// Agent preferences stored in settings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentPreferences {
    /// Currently selected agent
    pub agent: Agent,

    /// Model to use for the agent (None = agent default)
    pub model: Option<String>,

    /// Last updated timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<String>,
}

impl Default for AgentPreferences {
    fn default() -> Self {
        Self {
            agent: Agent::default(),
            model: None,
            updated_at: None,
        }
    }
}

impl AgentPreferences {
    /// Create new preferences with current timestamp
    pub fn new(agent: Agent, model: Option<String>) -> Self {
        Self {
            agent,
            model,
            updated_at: Some(chrono::Utc::now().to_rfc3339()),
        }
    }

    /// Load from file or return default
    pub fn load() -> Self {
        let config_path = dirs::config_dir()
            .map(|p| p.join("openagents").join("agent-preferences.json"))
            .unwrap_or_else(|| std::path::PathBuf::from("agent-preferences.json"));

        if let Ok(contents) = std::fs::read_to_string(&config_path) {
            serde_json::from_str(&contents).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    /// Save to file
    pub fn save(&self) -> anyhow::Result<()> {
        let config_dir = dirs::config_dir()
            .map(|p| p.join("openagents"))
            .unwrap_or_else(|| std::path::PathBuf::from("."));

        std::fs::create_dir_all(&config_dir)?;

        let config_path = config_dir.join("agent-preferences.json");
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(config_path, contents)?;

        Ok(())
    }
}
