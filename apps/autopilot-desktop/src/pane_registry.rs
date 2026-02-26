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
    match kind {
        PaneKind::Empty => &PANE_SPECS[0],
        PaneKind::AutopilotChat => &PANE_SPECS[1],
        PaneKind::GoOnline => &PANE_SPECS[2],
        PaneKind::ProviderStatus => &PANE_SPECS[3],
        PaneKind::EarningsScoreboard => &PANE_SPECS[4],
        PaneKind::RelayConnections => &PANE_SPECS[5],
        PaneKind::SyncHealth => &PANE_SPECS[6],
        PaneKind::NetworkRequests => &PANE_SPECS[7],
        PaneKind::StarterJobs => &PANE_SPECS[8],
        PaneKind::ActivityFeed => &PANE_SPECS[9],
        PaneKind::AlertsRecovery => &PANE_SPECS[10],
        PaneKind::Settings => &PANE_SPECS[11],
        PaneKind::JobInbox => &PANE_SPECS[12],
        PaneKind::ActiveJob => &PANE_SPECS[13],
        PaneKind::JobHistory => &PANE_SPECS[14],
        PaneKind::NostrIdentity => &PANE_SPECS[15],
        PaneKind::SparkWallet => &PANE_SPECS[16],
        PaneKind::SparkCreateInvoice => &PANE_SPECS[17],
        PaneKind::SparkPayInvoice => &PANE_SPECS[18],
    }
}

pub fn pane_spec_by_command_id(command_id: &str) -> Option<&'static PaneSpec> {
    pane_specs().iter().find(|spec| {
        spec.command
            .is_some_and(|command| command.id == command_id)
    })
}

pub fn pane_kind_for_hotbar_slot(slot: u8) -> Option<PaneKind> {
    pane_specs()
        .iter()
        .find_map(|spec| spec.hotbar.filter(|hotbar| hotbar.slot == slot).map(|_| spec.kind))
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

const PANE_SPECS: [PaneSpec; 19] = [
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
        title: "Autopilot Chat",
        default_width: 940.0,
        default_height: 540.0,
        singleton: true,
        startup: true,
        command: Some(PaneCommandSpec {
            id: "pane.autopilot_chat",
            label: "Autopilot Chat",
            description: "Open chat thread and composer for Autopilot",
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
        startup: true,
        command: Some(PaneCommandSpec {
            id: "pane.go_online",
            label: "Go Online",
            description: "Open provider mode toggle and lifecycle controls",
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
            description: "Open buyer-side request composer for network submission",
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
            description: "Open unified stream for chat/job/wallet/network/sync events",
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
            description: "Open incident alerts, remediation steps, and recovery actions",
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
            description: "Open incoming NIP-90 request intake pane",
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
            description: "Open in-flight job lifecycle timeline pane",
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
            description: "Open deterministic completed/failed job receipts pane",
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
            description: "Open Nostr keys (NIP-06) pane",
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
        startup: true,
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
];

#[cfg(test)]
mod tests {
    use super::{pane_specs, pane_spec_by_command_id};
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
}
