//! Host-supported plugin definitions and bounded composer rail registrations.

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use super::models::OPENROUTER_PLUGIN;

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
    AcpSubagent,
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
        name: "Microcoder",
        description: "The bundled coding loop, with local model logins and bounded commands.",
        default_enabled: true,
        tools: &[ToolBinding::Microcoder],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "jev",
        name: "Jev",
        description: "Typed decisions through the Jev SDK. Connect TypeSafe or a compatible gateway.",
        default_enabled: true,
        tools: &[ToolBinding::Jev],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "openagents-cli",
        name: "OpenAgents CLI",
        description: "Discover and call the bundled openagents command in this working directory.",
        default_enabled: true,
        tools: &[ToolBinding::OpenAgentsCli],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: "acp-subagents",
        name: "ACP Subagents",
        description: "Define named ACP agents and delegate tasks to their registered executables.",
        default_enabled: true,
        tools: &[ToolBinding::AcpSubagent],
        model_provider: None,
        composer_rails: &[],
    },
    PluginDefinition {
        id: super::brainstorm::PLUGIN,
        name: "Brainstorm",
        description: "Explicit public profile and reputation lookups through the Brainstorm house perspective.",
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
];

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
pub fn resolve_composer_rails<'a>(
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
