use crate::app_state::PaneKind;
use crate::spark_pane::{
    CREATE_INVOICE_PANE_HEIGHT, CREATE_INVOICE_PANE_WIDTH, PAY_INVOICE_PANE_HEIGHT,
    PAY_INVOICE_PANE_WIDTH, SPARK_PANE_HEIGHT, SPARK_PANE_WIDTH,
};

pub const HOTBAR_SLOT_NEW_CHAT: u8 = 1;
pub const HOTBAR_SLOT_NOSTR_IDENTITY: u8 = 2;
pub const HOTBAR_SLOT_SPARK_WALLET: u8 = 3;
pub const HOTBAR_SLOT_COMMAND_PALETTE: u8 = 4;

pub const HOTBAR_COMMAND_PALETTE_ICON: &str = "K";
pub const HOTBAR_COMMAND_PALETTE_TOOLTIP: &str = "Command palette";
pub const HOTBAR_COMMAND_PALETTE_SHORTCUT: &str = "K";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneCommandSpec {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub keybinding: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PaneHotbarSpec {
    pub slot: u8,
    pub icon: &'static str,
    pub tooltip: &'static str,
    pub shortcut: Option<&'static str>,
}

#[derive(Clone, Copy, Debug)]
pub struct PaneSpec {
    pub kind: PaneKind,
    pub title: &'static str,
    pub default_width: f32,
    pub default_height: f32,
    pub singleton: bool,
    pub startup: bool,
    pub command: Option<PaneCommandSpec>,
    pub hotbar: Option<PaneHotbarSpec>,
}

pub fn pane_specs() -> &'static [PaneSpec] {
    &PANE_SPECS
}

pub fn pane_spec(kind: PaneKind) -> &'static PaneSpec {
    pane_specs()
        .iter()
        .find(|spec| spec.kind == kind)
        .unwrap_or(&PANE_SPECS[0])
}

pub fn pane_spec_by_command_id(command_id: &str) -> Option<&'static PaneSpec> {
    pane_specs()
        .iter()
        .find(|spec| spec.command.is_some_and(|command| command.id == command_id))
}

pub fn pane_kind_for_hotbar_slot(slot: u8) -> Option<PaneKind> {
    pane_specs().iter().find_map(|spec| {
        spec.hotbar
            .filter(|hotbar| hotbar.slot == slot)
            .map(|_| spec.kind)
    })
}

pub fn pane_spec_for_hotbar_slot(slot: u8) -> Option<&'static PaneSpec> {
    pane_specs()
        .iter()
        .find(|spec| spec.hotbar.is_some_and(|hotbar| hotbar.slot == slot))
}

pub fn startup_pane_kinds() -> Vec<PaneKind> {
    pane_specs()
        .iter()
        .filter(|spec| spec.startup)
        .map(|spec| spec.kind)
        .collect()
}

const PANE_SPECS: [PaneSpec; 37] = [
    PaneSpec {
        kind: PaneKind::Empty,
        title: "Pane",
        default_width: 420.0,
        default_height: 280.0,
        singleton: false,
        startup: false,
        command: None,
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_NEW_CHAT,
            icon: "+",
            tooltip: "New pane",
            shortcut: None,
        }),
    },
    PaneSpec {
        kind: PaneKind::AutopilotChat,
        title: "Codex",
        default_width: 940.0,
        default_height: 540.0,
        singleton: true,
        startup: true,
        command: Some(PaneCommandSpec {
            id: "pane.codex",
            label: "Codex",
            description: "Open Codex chat threads and model controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexAccount,
        title: "Codex Account",
        default_width: 720.0,
        default_height: 360.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_account",
            label: "Codex Account",
            description: "Open Codex account auth and rate-limit controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexModels,
        title: "Codex Models",
        default_width: 860.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_models",
            label: "Codex Models",
            description: "Open Codex model catalog and reroute view",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexConfig,
        title: "Codex Config",
        default_width: 860.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_config",
            label: "Codex Config",
            description: "Open Codex config, requirements, and external import tools",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexMcp,
        title: "Codex MCP",
        default_width: 900.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_mcp",
            label: "Codex MCP",
            description: "Open MCP server status, OAuth login, and reload controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexApps,
        title: "Codex Apps",
        default_width: 920.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_apps",
            label: "Codex Apps",
            description: "Open Codex app connector catalog and update visibility",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexRemoteSkills,
        title: "Codex Remote Skills",
        default_width: 920.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_remote_skills",
            label: "Codex Remote Skills",
            description: "Open remote skill discovery/export while keeping local skills primary",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexLabs,
        title: "Codex Labs",
        default_width: 960.0,
        default_height: 520.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_labs",
            label: "Codex Labs",
            description: "Open review, utility exec, and gated experimental Codex controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CodexDiagnostics,
        title: "Codex Diagnostics",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.codex_diagnostics",
            label: "Codex Diagnostics",
            description: "Open Codex protocol diagnostics, counters, and wire-log controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::GoOnline,
        title: "Go Online",
        default_width: 560.0,
        default_height: 300.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.go_online",
            label: "Go Online",
            description: "Open SA runner toggle with SKL trust gate and AC lane status",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ProviderStatus,
        title: "Provider Status",
        default_width: 700.0,
        default_height: 360.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.provider_status",
            label: "Provider Status",
            description: "Open runtime health and heartbeat visibility pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::EarningsScoreboard,
        title: "Earnings Scoreboard",
        default_width: 640.0,
        default_height: 320.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.earnings_scoreboard",
            label: "Earnings Scoreboard",
            description: "Open sats/day, lifetime, jobs/day and last-result metrics pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::RelayConnections,
        title: "Relay Connections",
        default_width: 900.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.relay_connections",
            label: "Relay Connections",
            description: "Open relay connectivity and retry controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SyncHealth,
        title: "Sync Health",
        default_width: 760.0,
        default_height: 360.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.sync_health",
            label: "Sync Health",
            description: "Open spacetime subscription and stale-cursor diagnostics pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::NetworkRequests,
        title: "Network Requests",
        default_width: 900.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.network_requests",
            label: "Network Requests",
            description: "Open buyer composer with skill scope and credit envelope references",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::StarterJobs,
        title: "Starter Jobs",
        default_width: 860.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.starter_jobs",
            label: "Starter Jobs",
            description: "Open starter-demand queue and completion payouts pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ActivityFeed,
        title: "Activity Feed",
        default_width: 940.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.activity_feed",
            label: "Activity Feed",
            description: "Open unified stream including SA/SKL/AC runtime classes",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AlertsRecovery,
        title: "Alerts and Recovery",
        default_width: 900.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.alerts_recovery",
            label: "Alerts and Recovery",
            description: "Open incident queue including trust and credit lane recovery",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::Settings,
        title: "Settings",
        default_width: 860.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.settings",
            label: "Settings",
            description: "Open network, wallet, and provider defaults with validation",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::JobInbox,
        title: "Job Inbox",
        default_width: 860.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.job_inbox",
            label: "Job Inbox",
            description: "Open incoming requests with SA/SKL/AC linkage metadata",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ActiveJob,
        title: "Active Job",
        default_width: 860.0,
        default_height: 440.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.active_job",
            label: "Active Job",
            description: "Open in-flight job lifecycle with SA tick, trajectory, and AC links",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::JobHistory,
        title: "Job History",
        default_width: 900.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.job_history",
            label: "Job History",
            description: "Open immutable receipts with SA/SKL/AC proof links",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::NostrIdentity,
        title: "Nostr Keys (NIP-06)",
        default_width: 760.0,
        default_height: 380.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.identity_keys",
            label: "Identity Keys",
            description: "Open Nostr keys plus agent/skill derivation previews",
            keybinding: Some("2"),
        }),
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_NOSTR_IDENTITY,
            icon: "N",
            tooltip: "Nostr keys",
            shortcut: None,
        }),
    },
    PaneSpec {
        kind: PaneKind::SparkWallet,
        title: "Spark Lightning Wallet",
        default_width: SPARK_PANE_WIDTH,
        default_height: SPARK_PANE_HEIGHT,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.wallet",
            label: "Spark Wallet",
            description: "Show Spark wallet controls",
            keybinding: Some("3"),
        }),
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_SPARK_WALLET,
            icon: "S",
            tooltip: "Spark wallet",
            shortcut: None,
        }),
    },
    PaneSpec {
        kind: PaneKind::SparkCreateInvoice,
        title: "Create Lightning Invoice",
        default_width: CREATE_INVOICE_PANE_WIDTH,
        default_height: CREATE_INVOICE_PANE_HEIGHT,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.create_invoice",
            label: "Create Lightning Invoice",
            description: "Open dedicated pane for creating Lightning invoices",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SparkPayInvoice,
        title: "Pay Lightning Invoice",
        default_width: PAY_INVOICE_PANE_WIDTH,
        default_height: PAY_INVOICE_PANE_HEIGHT,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.pay_invoice",
            label: "Pay Lightning Invoice",
            description: "Open dedicated pane for paying Lightning invoices",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AgentProfileState,
        title: "Agent Profile and State",
        default_width: 900.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.agent_profile_state",
            label: "Agent Profile and State",
            description: "Open SA profile/state/goals publishing and audit pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AgentScheduleTick,
        title: "Agent Schedule and Tick",
        default_width: 900.0,
        default_height: 400.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.agent_schedule_tick",
            label: "Agent Schedule and Tick",
            description: "Open SA schedule controls and manual tick lifecycle pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::TrajectoryAudit,
        title: "Trajectory Audit",
        default_width: 900.0,
        default_height: 400.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.trajectory_audit",
            label: "Trajectory Audit",
            description: "Open SA trajectory session inspection and verification pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SkillRegistry,
        title: "Agent Skill Registry",
        default_width: 900.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.skill_registry",
            label: "Agent Skill Registry",
            description: "Open SKL discovery and manifest installation pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SkillTrustRevocation,
        title: "Skill Trust and Revocation",
        default_width: 940.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.skill_trust_revocation",
            label: "Skill Trust and Revocation",
            description: "Open SKL trust tier, kill-switch, and revocation controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CreditDesk,
        title: "Credit Desk",
        default_width: 940.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.credit_desk",
            label: "Credit Desk",
            description: "Open AC intent/offer/envelope/spend workflow pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CreditSettlementLedger,
        title: "Credit Settlement Ledger",
        default_width: 940.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.credit_settlement_ledger",
            label: "Credit Settlement Ledger",
            description: "Open AC settlement/default audit pane with reputation labeling",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AgentNetworkSimulation,
        title: "Sovereign Agent Simulation",
        default_width: 980.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.agent_network_simulation",
            label: "Sovereign Agent Simulation",
            description: "Open multi-agent simulation using NIP-28 chat with SA/SKL/AC flow",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::TreasuryExchangeSimulation,
        title: "Treasury Exchange Simulation",
        default_width: 980.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.treasury_exchange_simulation",
            label: "Treasury Exchange Simulation",
            description: "Open NIP-69/60/61/87/89/47 market and liquidity simulation",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::RelaySecuritySimulation,
        title: "Relay Security Simulation",
        default_width: 980.0,
        default_height: 460.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.relay_security_simulation",
            label: "Relay Security Simulation",
            description: "Open NIP-11/42/65/46/17/59/98/77 secure relay simulation",
            keybinding: None,
        }),
        hotbar: None,
    },
];

#[cfg(test)]
mod tests {
    use super::{
        pane_kind_for_hotbar_slot, pane_spec, pane_spec_by_command_id, pane_specs,
        startup_pane_kinds,
    };
    use crate::app_state::PaneKind;
    use std::collections::BTreeSet;

    #[test]
    fn pane_registry_has_unique_command_ids_and_hotbar_slots() {
        let mut command_ids = BTreeSet::new();
        let mut hotbar_slots = BTreeSet::new();
        for spec in pane_specs() {
            if let Some(command) = spec.command {
                assert!(
                    command_ids.insert(command.id),
                    "duplicate command id {}",
                    command.id
                );
                assert!(pane_spec_by_command_id(command.id).is_some());
            }
            if let Some(hotbar) = spec.hotbar {
                assert!(
                    hotbar_slots.insert(hotbar.slot),
                    "duplicate hotbar slot {}",
                    hotbar.slot
                );
            }
        }
    }

    #[test]
    fn hotbar_slots_resolve_back_to_registry_kind() {
        for spec in pane_specs() {
            let Some(hotbar) = spec.hotbar else {
                continue;
            };
            assert_eq!(
                pane_kind_for_hotbar_slot(hotbar.slot),
                Some(spec.kind),
                "hotbar slot {} should resolve to {:?}",
                hotbar.slot,
                spec.kind
            );
        }
    }

    #[test]
    fn singleton_contract_covers_startup_and_hotbar_panes() {
        for kind in startup_pane_kinds() {
            assert!(
                pane_spec(kind).singleton,
                "startup pane {:?} must be singleton",
                kind
            );
        }

        for spec in pane_specs() {
            if spec.kind == PaneKind::Empty {
                continue;
            }
            if spec.hotbar.is_some() {
                assert!(
                    spec.singleton,
                    "hotbar pane {:?} must be singleton",
                    spec.kind
                );
            }
        }
    }

    #[test]
    fn codex_feature_family_commands_are_registered() {
        let required = [
            "pane.codex",
            "pane.codex_account",
            "pane.codex_models",
            "pane.codex_config",
            "pane.codex_mcp",
            "pane.codex_apps",
            "pane.codex_remote_skills",
            "pane.codex_labs",
            "pane.codex_diagnostics",
        ];

        for command_id in required {
            assert!(
                pane_spec_by_command_id(command_id).is_some(),
                "missing codex pane command registration for {command_id}"
            );
        }
    }
}
