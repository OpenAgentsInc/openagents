use wgpui::{Key, Modifiers};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Interrupt,
    OpenCommandPalette,
    OpenSettings,
    OpenWallet,
    OpenDvm,
    OpenGateway,
    OpenLmRouter,
    OpenNexus,
    OpenSparkWallet,
    OpenNip90,
    OpenOanix,
    OpenDirectives,
    OpenIssues,
    OpenIssueTracker,
    OpenRlm,
    OpenRlmTrace,
    OpenPylonEarnings,
    OpenPylonJobs,
    OpenDspy,
    OpenNip28,
    ToggleLeftSidebar,
    ToggleRightSidebar,
    ToggleSidebars,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Keybinding {
    pub key: Key,
    pub modifiers: Modifiers,
    pub action: Action,
}

impl Keybinding {
    pub fn matches(&self, key: &Key, modifiers: Modifiers) -> bool {
        self.key == *key && self.modifiers == modifiers
    }
}

impl Action {
    pub fn all() -> &'static [Action] {
        &[
            Action::Interrupt,
            Action::OpenCommandPalette,
            Action::OpenSettings,
            Action::OpenWallet,
            Action::OpenDvm,
            Action::OpenGateway,
            Action::OpenLmRouter,
            Action::OpenNexus,
            Action::OpenSparkWallet,
            Action::OpenNip90,
            Action::OpenOanix,
            Action::OpenDirectives,
            Action::OpenIssues,
            Action::OpenIssueTracker,
            Action::OpenRlm,
            Action::OpenRlmTrace,
            Action::OpenPylonEarnings,
            Action::OpenPylonJobs,
            Action::OpenDspy,
            Action::OpenNip28,
            Action::ToggleLeftSidebar,
            Action::ToggleRightSidebar,
            Action::ToggleSidebars,
        ]
    }

    pub fn id(&self) -> &'static str {
        match self {
            Action::Interrupt => "interrupt",
            Action::OpenCommandPalette => "command_palette",
            Action::OpenSettings => "settings",
            Action::OpenWallet => "wallet",
            Action::OpenDvm => "dvm",
            Action::OpenGateway => "gateway",
            Action::OpenLmRouter => "lm_router",
            Action::OpenNexus => "nexus",
            Action::OpenSparkWallet => "spark_wallet",
            Action::OpenNip90 => "nip90",
            Action::OpenOanix => "oanix",
            Action::OpenDirectives => "directives",
            Action::OpenIssues => "issues",
            Action::OpenIssueTracker => "issue_tracker",
            Action::OpenRlm => "rlm",
            Action::OpenRlmTrace => "rlm_trace",
            Action::OpenPylonEarnings => "pylon_earnings",
            Action::OpenPylonJobs => "pylon_jobs",
            Action::OpenDspy => "dspy",
            Action::OpenNip28 => "nip28",
            Action::ToggleLeftSidebar => "sidebar_left",
            Action::ToggleRightSidebar => "sidebar_right",
            Action::ToggleSidebars => "sidebar_toggle",
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Action::Interrupt => "Interrupt request",
            Action::OpenCommandPalette => "Command palette",
            Action::OpenSettings => "Open settings",
            Action::OpenWallet => "Open wallet",
            Action::OpenDvm => "Open DVM providers",
            Action::OpenGateway => "Open gateway",
            Action::OpenLmRouter => "Open LM router",
            Action::OpenNexus => "Open Nexus stats",
            Action::OpenSparkWallet => "Open Spark wallet",
            Action::OpenNip90 => "Open NIP-90 jobs",
            Action::OpenOanix => "Open OANIX",
            Action::OpenDirectives => "Open workspace directives",
            Action::OpenIssues => "Open workspace issues",
            Action::OpenIssueTracker => "Open issue tracker",
            Action::OpenRlm => "Open RLM runs",
            Action::OpenRlmTrace => "Open RLM trace",
            Action::OpenPylonEarnings => "Open Pylon earnings",
            Action::OpenPylonJobs => "Open Pylon jobs",
            Action::OpenDspy => "Open DSPy",
            Action::OpenNip28 => "Open NIP-28 chat",
            Action::ToggleLeftSidebar => "Toggle left sidebar",
            Action::ToggleRightSidebar => "Toggle right sidebar",
            Action::ToggleSidebars => "Toggle sidebars",
        }
    }

    pub fn from_id(id: &str) -> Option<Action> {
        match id {
            "interrupt" => Some(Action::Interrupt),
            "command_palette" => Some(Action::OpenCommandPalette),
            "settings" => Some(Action::OpenSettings),
            "wallet" => Some(Action::OpenWallet),
            "dvm" => Some(Action::OpenDvm),
            "gateway" => Some(Action::OpenGateway),
            "lm_router" => Some(Action::OpenLmRouter),
            "nexus" => Some(Action::OpenNexus),
            "spark_wallet" => Some(Action::OpenSparkWallet),
            "nip90" => Some(Action::OpenNip90),
            "oanix" => Some(Action::OpenOanix),
            "directives" => Some(Action::OpenDirectives),
            "issues" => Some(Action::OpenIssues),
            "issue_tracker" => Some(Action::OpenIssueTracker),
            "rlm" => Some(Action::OpenRlm),
            "rlm_trace" => Some(Action::OpenRlmTrace),
            "pylon_earnings" => Some(Action::OpenPylonEarnings),
            "pylon_jobs" => Some(Action::OpenPylonJobs),
            "dspy" => Some(Action::OpenDspy),
            "nip28" => Some(Action::OpenNip28),
            "sidebar_left" => Some(Action::ToggleLeftSidebar),
            "sidebar_right" => Some(Action::ToggleRightSidebar),
            "sidebar_toggle" => Some(Action::ToggleSidebars),
            _ => None,
        }
    }
}

pub fn default_keybindings() -> Vec<Keybinding> {
    vec![
        Keybinding {
            key: Key::Character("c".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::Interrupt,
        },
        Keybinding {
            key: Key::Character("k".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenCommandPalette,
        },
        Keybinding {
            key: Key::Character(",".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::OpenSettings,
        },
        Keybinding {
            key: Key::Character("w".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenWallet,
        },
        Keybinding {
            key: Key::Character("p".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDvm,
        },
        Keybinding {
            key: Key::Character("g".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenGateway,
        },
        Keybinding {
            key: Key::Character("l".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenLmRouter,
        },
        Keybinding {
            key: Key::Character("x".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNexus,
        },
        Keybinding {
            key: Key::Character("s".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenSparkWallet,
        },
        Keybinding {
            key: Key::Character("j".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNip90,
        },
        Keybinding {
            key: Key::Character("o".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenOanix,
        },
        Keybinding {
            key: Key::Character("t".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDirectives,
        },
        Keybinding {
            key: Key::Character("a".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenIssueTracker,
        },
        Keybinding {
            key: Key::Character("r".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenRlm,
        },
        Keybinding {
            key: Key::Character("y".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenRlmTrace,
        },
        Keybinding {
            key: Key::Character("e".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenPylonEarnings,
        },
        Keybinding {
            key: Key::Character("u".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenPylonJobs,
        },
        Keybinding {
            key: Key::Character("i".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenIssues,
        },
        Keybinding {
            key: Key::Character("d".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDspy,
        },
        Keybinding {
            key: Key::Character("n".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNip28,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleRightSidebar,
        },
        Keybinding {
            key: Key::Character("\\".to_string()),
            modifiers: Modifiers {
                ctrl: true,
                ..Default::default()
            },
            action: Action::ToggleSidebars,
        },
        Keybinding {
            key: Key::Character("w".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenWallet,
        },
        Keybinding {
            key: Key::Character("p".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDvm,
        },
        Keybinding {
            key: Key::Character("g".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenGateway,
        },
        Keybinding {
            key: Key::Character("l".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenLmRouter,
        },
        Keybinding {
            key: Key::Character("x".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNexus,
        },
        Keybinding {
            key: Key::Character("s".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenSparkWallet,
        },
        Keybinding {
            key: Key::Character("j".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNip90,
        },
        Keybinding {
            key: Key::Character("o".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenOanix,
        },
        Keybinding {
            key: Key::Character("t".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDirectives,
        },
        Keybinding {
            key: Key::Character("a".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenIssueTracker,
        },
        Keybinding {
            key: Key::Character("r".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenRlm,
        },
        Keybinding {
            key: Key::Character("y".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenRlmTrace,
        },
        Keybinding {
            key: Key::Character("e".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenPylonEarnings,
        },
        Keybinding {
            key: Key::Character("u".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenPylonJobs,
        },
        Keybinding {
            key: Key::Character("i".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenIssues,
        },
        Keybinding {
            key: Key::Character("d".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenDspy,
        },
        Keybinding {
            key: Key::Character("n".to_string()),
            modifiers: Modifiers {
                meta: true,
                shift: true,
                ..Default::default()
            },
            action: Action::OpenNip28,
        },
        Keybinding {
            key: Key::Character("[".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleLeftSidebar,
        },
        Keybinding {
            key: Key::Character("]".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleRightSidebar,
        },
        Keybinding {
            key: Key::Character("\\".to_string()),
            modifiers: Modifiers {
                meta: true,
                ..Default::default()
            },
            action: Action::ToggleSidebars,
        },
    ]
}

pub fn match_action(key: &Key, modifiers: Modifiers, bindings: &[Keybinding]) -> Option<Action> {
    bindings
        .iter()
        .find(|binding| binding.matches(key, modifiers))
        .map(|binding| binding.action)
}
