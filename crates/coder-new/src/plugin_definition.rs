//! Host-supported plugin definitions and bounded composer rail registrations.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::models::OPENROUTER_PLUGIN;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModelProviderBinding {
    OpenRouter,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RailSlot {
    ComposerTopRight,
    ComposerBottomRight,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailBinding {
    SelectedModel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolBinding {
    Microcoder,
    Jev,
    OpenAgentsCli,
    Github,
    AcpSubagent,
    Solver,
    /// The agent channel (#11346): the shared channel, intents and overlap
    /// notices, negotiation records and system leases.
    Comms,
    /// The solver swarm (#11346): N issues handed to N solvers at once.
    SolverSwarm,
    BrainstormSearch,
    BrainstormRank,
    BoatDelegate,
    BoatJob,
    GceDelegate,
    GceJob,
}

/// A registration is scoped by its plugin ID and its own ID.
#[derive(Clone, Copy, Debug)]
pub struct RailContribution {
    pub id: &'static str,
    pub slot: RailSlot,
    pub binding: RailBinding,
    pub priority: u16,
    pub max_cells: u16,
}

/// A definition names supported host bindings; it does not supply executable code.
#[derive(Clone, Copy, Debug)]
pub struct PluginDefinition {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub default_enabled: bool,
    pub tools: &'static [ToolBinding],
    pub model_provider: Option<ModelProviderBinding>,
    pub composer_rails: &'static [RailContribution],
}

pub const DEFINITIONS: &[PluginDefinition] = &[
    PluginDefinition {
        id: OPENROUTER_PLUGIN,
        name: "OpenRouter BYOK",
        description: "Use OpenRouter models with your own API key.",
        default_enabled: false,
        tools: &[],
        model_provider: Some(ModelProviderBinding::OpenRouter),
        composer_rails: &[RailContribution {
            id: "selected-model",
            slot: RailSlot::ComposerTopRight,
            binding: RailBinding::SelectedModel,
            priority: 100,
            max_cells: 64,
        }],
    },
    PluginDefinition {
        id: "microcoder",
        name: "Coder loop",
        description: "Coder's own coding loop, on a model you're logged in to here or your OpenRouter key.",
        default_enabled: true,
        tools: &[ToolBinding::Microcoder],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "jev",
        name: "Jev",
        description: "Quick judgment calls while Coder works, like whether a step is done. Uses your TypeSafe key or a compatible service.",
        default_enabled: true,
        tools: &[ToolBinding::Jev],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "openagents-cli",
        name: "OpenAgents CLI",
        description: "Lets Coder use the openagents command that comes with it, in this folder.",
        default_enabled: true,
        tools: &[ToolBinding::OpenAgentsCli, ToolBinding::Github],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "acp-subagents",
        name: "ACP Subagents",
        description: "Hands tasks to other coding agents installed on this computer, like Claude Code, Codex, or Cursor. Turned off, OpenCode isn't used either.",
        default_enabled: true,
        tools: &[ToolBinding::AcpSubagent],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::brainstorm::PLUGIN,
        name: "Brainstorm",
        description: "Looks up public profiles and how much influence they have, when you ask with /brainstorm.",
        default_enabled: false,
        tools: &[ToolBinding::BrainstormSearch, ToolBinding::BrainstormRank],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "boat-cloud",
        name: "Boat Cloud",
        description: "Delegate through Boat integrated agents or headless Coder, with saved workspaces and usage.",
        default_enabled: false,
        tools: &[ToolBinding::BoatDelegate, ToolBinding::BoatJob],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "gce-cloud",
        name: "GCE Cloud",
        description: "Run headless Coder in the granted GCE pool, with reconnectable jobs and estimated usage.",
        default_enabled: false,
        tools: &[ToolBinding::GceDelegate, ToolBinding::GceJob],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::solver_plugin::PLUGIN,
        name: "Solver",
        description: "Hand an issue to Solver, which plans, codes, checks, and opens the pull request.",
        default_enabled: true,
        tools: &[ToolBinding::Solver],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::claude::PLUGIN,
        name: "Claude Code",
        description: "Claude Code inside Coder on this computer's Claude login: its whole toolset, your settings and skills, and Coder's delegation tools. When it is on, new chats run on it.",
        default_enabled: true,
        tools: &[],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::codex::PLUGIN,
        name: "Codex",
        description: "Codex inside Coder on this computer's ChatGPT login: its own tools and models, and Coder's delegation tools. Coder can pick it for a chat, or you can ask for it.",
        default_enabled: true,
        tools: &[],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::code_graph_plugin::PLUGIN,
        name: "Code Graph",
        description: crate::code_graph_plugin::DESCRIPTION,
        default_enabled: true,
        tools: &[],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::token_savers::PLUGIN,
        name: "Token savers",
        description: crate::token_savers::DESCRIPTION,
        default_enabled: true,
        tools: &[],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::second_look::PLUGIN,
        name: "Second look",
        description: crate::second_look::DESCRIPTION,
        default_enabled: false,
        tools: &[],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::comms::PLUGIN,
        name: "Agent channel",
        description: "Let the subagents a chat delegates to talk to each other: what each means to change, overlaps, agreements and shared leases.",
        default_enabled: true,
        tools: &[ToolBinding::Comms],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: crate::swarm::PLUGIN,
        name: "Solver swarm",
        description: "Hand several issues to several Solvers at once; they coordinate over the agent channel.",
        default_enabled: true,
        tools: &[ToolBinding::SolverSwarm],
        model_provider: None,
        composer_rails: &[],
    },
];

/// The OpenCode plugin (#11507). It is listed only on a computer with
/// OpenCode installed ([`crate::plugins::Plugins::definitions`]), so
/// nothing offers OpenCode where it can't run.
pub const OPENCODE: PluginDefinition = PluginDefinition {
    id: crate::opencode_chat::PLUGIN,
    name: "OpenCode",
    description: "OpenCode inside Coder on this computer: its free model or the models you set up in OpenCode, with Coder checking every step before it runs. When Claude Code is off, logged out, or out of usage, new chats run on it, except one that starts with a quick question, which Coder answers itself and keeps. To start every new chat on OpenCode, run openagents coder models set opencode.",
    default_enabled: true,
    tools: &[],
    model_provider: None,
    composer_rails: &[],
};

/// The built-in plugins this computer lists: [`DEFINITIONS`], then
/// [`OPENCODE`] where OpenCode is installed, in the Plugins screen's order.
pub fn listed() -> impl Iterator<Item = &'static PluginDefinition> {
    DEFINITIONS
        .iter()
        .chain(crate::opencode_chat::installed().then_some(&OPENCODE))
}

/// The built-in provider uses the same rail registration contract as provider plugins.
pub const FALLBACK_PROVIDER: PluginDefinition = PluginDefinition {
    id: "openagents-gateway",
    name: "OpenAgents AI Gateway",
    description: "The no-setup provider after local model logins.",
    default_enabled: true,
    tools: &[],
    model_provider: None,
    composer_rails: &[RailContribution {
        id: "active-model",
        slot: RailSlot::ComposerTopRight,
        binding: RailBinding::SelectedModel,
        priority: 10,
        max_cells: 64,
    }],
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedRail {
    pub slot: RailSlot,
    pub text: String,
}

/// Choose one eligible registration per slot, with stable identity tie breaking.
///
/// The host returns no value for a disabled plugin or an unsupported binding.
pub(crate) fn resolve_composer_rails<'a>(
    definitions: &[PluginDefinition],
    mut value: impl FnMut(&PluginDefinition, RailBinding) -> Option<&'a str>,
) -> Vec<ResolvedRail> {
    let mut candidates = Vec::new();
    for definition in definitions {
        for contribution in definition.composer_rails {
            if contribution.max_cells == 0 {
                continue;
            }
            let Some(value) = value(definition, contribution.binding) else {
                continue;
            };
            let text = bounded_text(value, contribution.max_cells);
            if !text.is_empty() {
                candidates.push((definition, contribution, text));
            }
        }
    }
    candidates.sort_by(|(left_plugin, left, _), (right_plugin, right, _)| {
        left.slot
            .cmp(&right.slot)
            .then_with(|| right.priority.cmp(&left.priority))
            .then_with(|| left_plugin.id.cmp(right_plugin.id))
            .then_with(|| left.id.cmp(right.id))
    });
    let mut rails: Vec<ResolvedRail> = Vec::new();
    for (_, contribution, text) in candidates {
        if rails
            .last()
            .is_some_and(|rail| rail.slot == contribution.slot)
        {
            continue;
        }
        rails.push(ResolvedRail {
            slot: contribution.slot,
            text,
        });
    }
    rails
}

fn bounded_text(value: &str, max_cells: u16) -> String {
    let text: String = value
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let limit = usize::from(max_cells);
    if text.width() <= limit {
        return text;
    }
    let budget = limit.saturating_sub(1);
    let mut clipped = String::new();
    let mut cells = 0;
    for grapheme in text.graphemes(true) {
        let width = grapheme.width();
        if cells + width > budget {
            break;
        }
        clipped.push_str(grapheme);
        cells += width;
    }
    clipped.push('…');
    clipped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slot_winners_use_priority_then_scoped_identity() {
        let definitions = [
            PluginDefinition {
                id: "z-provider",
                name: "Z provider",
                description: "",
                default_enabled: false,
                tools: &[],
                model_provider: None,
                composer_rails: &[RailContribution {
                    id: "model",
                    slot: RailSlot::ComposerTopRight,
                    binding: RailBinding::SelectedModel,
                    priority: 200,
                    max_cells: 64,
                }],
            },
            PluginDefinition {
                id: "a-provider",
                name: "A provider",
                description: "",
                default_enabled: false,
                tools: &[],
                model_provider: None,
                composer_rails: &[
                    RailContribution {
                        id: "z-model",
                        slot: RailSlot::ComposerTopRight,
                        binding: RailBinding::SelectedModel,
                        priority: 200,
                        max_cells: 8,
                    },
                    RailContribution {
                        id: "a-model",
                        slot: RailSlot::ComposerTopRight,
                        binding: RailBinding::SelectedModel,
                        priority: 200,
                        max_cells: 5,
                    },
                    RailContribution {
                        id: "secondary",
                        slot: RailSlot::ComposerBottomRight,
                        binding: RailBinding::SelectedModel,
                        priority: 1,
                        max_cells: 64,
                    },
                ],
            },
            PluginDefinition {
                id: "0-provider",
                name: "Lower priority provider",
                description: "",
                default_enabled: false,
                tools: &[],
                model_provider: None,
                composer_rails: &[RailContribution {
                    id: "model",
                    slot: RailSlot::ComposerTopRight,
                    binding: RailBinding::SelectedModel,
                    priority: 100,
                    max_cells: 64,
                }],
            },
        ];
        let rails = resolve_composer_rails(&definitions, |definition, _| Some(definition.id));
        assert_eq!(
            rails,
            vec![
                ResolvedRail {
                    slot: RailSlot::ComposerTopRight,
                    text: "a-pr…".into()
                },
                ResolvedRail {
                    slot: RailSlot::ComposerBottomRight,
                    text: "a-provider".into()
                },
            ]
        );
        let rails = resolve_composer_rails(&definitions, |definition, _| {
            (definition.id != "a-provider").then_some(definition.id)
        });
        assert_eq!(
            rails,
            vec![ResolvedRail {
                slot: RailSlot::ComposerTopRight,
                text: "z-provider".into(),
            }]
        );
        let mut reversed = definitions;
        reversed.reverse();
        assert_eq!(
            resolve_composer_rails(&reversed, |definition, _| Some(definition.id)),
            resolve_composer_rails(&definitions, |definition, _| Some(definition.id)),
        );
    }

    #[test]
    fn rail_text_drops_controls_and_keeps_graphemes_within_cell_limit() {
        assert_eq!(
            bounded_text("openrouter/\nfree\t\u{1b}", 64),
            "openrouter/free"
        );
        assert_eq!(bounded_text("中e\u{301}abc", 4), "中e\u{301}…");
        assert_eq!(bounded_text("中abc", 2), "…");
        assert_eq!(bounded_text("abc", 1), "…");
    }
}
