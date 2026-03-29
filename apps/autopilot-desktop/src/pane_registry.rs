use crate::app_state::PaneKind;
use crate::spark_pane::{
    CREATE_INVOICE_PANE_HEIGHT, CREATE_INVOICE_PANE_WIDTH, PAY_INVOICE_PANE_HEIGHT,
    PAY_INVOICE_PANE_WIDTH, SPARK_PANE_HEIGHT, SPARK_PANE_WIDTH,
};

pub const HOTBAR_SLOT_PROVIDER_CONTROL: u8 = 1;
pub const HOTBAR_SLOT_NOSTR_IDENTITY: u8 = 2;
pub const HOTBAR_SLOT_SPARK_WALLET: u8 = 3;
pub const HOTBAR_SLOT_EARNINGS_JOBS: u8 = 4;
pub const HOTBAR_SLOT_LOG_STREAM: u8 = 5;
pub const HOTBAR_SLOT_COMMAND_PALETTE: u8 = 6;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneSearchTier {
    Release,
    Experimental,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PaneSearchFilter {
    Release,
    Experimental,
    All,
}

impl PaneSearchFilter {
    pub const fn cycle(self) -> Self {
        match self {
            Self::Release => Self::Experimental,
            Self::Experimental => Self::All,
            Self::All => Self::Release,
        }
    }

    pub const fn button_label(self) -> &'static str {
        match self {
            Self::Release => "REL",
            Self::Experimental => "EXP",
            Self::All => "ALL",
        }
    }

    pub const fn includes(self, tier: PaneSearchTier) -> bool {
        match self {
            Self::All => true,
            Self::Release => matches!(tier, PaneSearchTier::Release),
            Self::Experimental => matches!(tier, PaneSearchTier::Experimental),
        }
    }
}

pub const fn pane_search_tier(kind: PaneKind) -> PaneSearchTier {
    match kind {
        PaneKind::ProjectOps
        | PaneKind::CodexAccount
        | PaneKind::CodexModels
        | PaneKind::CodexConfig
        | PaneKind::CodexMcp
        | PaneKind::CodexApps
        | PaneKind::CodexLabs
        | PaneKind::CodexDiagnostics
        | PaneKind::PsionicViz
        | PaneKind::PsionicRemoteTraining
        | PaneKind::XtrainExplorer
        | PaneKind::ContributorBeta
        | PaneKind::AttnResLab
        | PaneKind::TassadarLab
        | PaneKind::RivePreview
        | PaneKind::Presentation
        | PaneKind::FrameDebugger
        | PaneKind::AppleFmWorkbench
        | PaneKind::AppleAdapterTraining
        | PaneKind::NetworkRequests
        | PaneKind::StarterJobs
        | PaneKind::ReciprocalLoop
        | PaneKind::ActivityFeed
        | PaneKind::AlertsRecovery
        | PaneKind::Settings
        | PaneKind::Credentials
        | PaneKind::AgentProfileState
        | PaneKind::AgentScheduleTick
        | PaneKind::TrajectoryAudit
        | PaneKind::CastControl
        | PaneKind::SkillRegistry
        | PaneKind::SkillTrustRevocation
        | PaneKind::CreditDesk
        | PaneKind::CreditSettlementLedger
        | PaneKind::Calculator
        | PaneKind::CadDemo
        | PaneKind::BuyerRaceMatrix
        | PaneKind::SellerEarningsTimeline
        | PaneKind::SettlementLadder
        | PaneKind::KeyLedger
        | PaneKind::SettlementAtlas
        | PaneKind::RelayChoreography => PaneSearchTier::Experimental,
        _ => PaneSearchTier::Release,
    }
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

pub fn pane_kind_enabled(kind: PaneKind) -> bool {
    match kind {
        PaneKind::LocalInference => !cfg!(target_os = "macos"),
        PaneKind::ProjectOps => crate::project_ops::project_ops_enabled_from_env(),
        _ => true,
    }
}

pub fn enabled_pane_specs() -> impl Iterator<Item = &'static PaneSpec> {
    pane_specs()
        .iter()
        .filter(|spec| pane_kind_enabled(spec.kind))
}

pub fn pane_spec_by_command_id(command_id: &str) -> Option<&'static PaneSpec> {
    enabled_pane_specs().find(|spec| spec.command.is_some_and(|command| command.id == command_id))
}

pub fn pane_kind_for_hotbar_slot(slot: u8) -> Option<PaneKind> {
    enabled_pane_specs().find_map(|spec| {
        spec.hotbar
            .filter(|hotbar| hotbar.slot == slot)
            .map(|_| spec.kind)
    })
}

pub fn pane_spec_for_hotbar_slot(slot: u8) -> Option<&'static PaneSpec> {
    enabled_pane_specs().find(|spec| spec.hotbar.is_some_and(|hotbar| hotbar.slot == slot))
}

pub fn startup_pane_kinds() -> Vec<PaneKind> {
    enabled_pane_specs()
        .filter(|spec| spec.startup)
        .map(|spec| spec.kind)
        .collect()
}

const PANE_SPECS: [PaneSpec; 67] = [
    PaneSpec {
        kind: PaneKind::Empty,
        title: "Pane",
        default_width: 420.0,
        default_height: 280.0,
        singleton: false,
        startup: false,
        command: None,
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AutopilotChat,
        title: "CHAT",
        default_width: 760.0,
        default_height: 540.0,
        singleton: true,
        startup: true,
        command: Some(PaneCommandSpec {
            id: "pane.codex",
            label: "CHAT",
            description: "Open a simple local Autopilot conversation pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ProjectOps,
        title: "Project Ops",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.project_ops",
            label: "Project Ops",
            description: "Open the native PM shell behind the project_ops feature gate",
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
        title: "MISSION CONTROL",
        default_width: 1040.0,
        default_height: 620.0,
        singleton: true,
        startup: true,
        command: Some(PaneCommandSpec {
            id: "pane.mission_control",
            label: "Mission Control",
            description: "Open the earn-first control panel for wallet, jobs, and provider state",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ProviderControl,
        title: "Provider Control",
        default_width: 760.0,
        default_height: 600.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.provider_control",
            label: "Provider Control",
            description: "Open provider status, runtime controls, and launch inventory toggles",
            keybinding: Some("1"),
        }),
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_PROVIDER_CONTROL,
            icon: ">",
            tooltip: "Provider control",
            shortcut: Some("1"),
        }),
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
        kind: PaneKind::TailnetStatus,
        title: "Tailnet Status",
        default_width: 700.0,
        default_height: 520.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.tailnet_status",
            label: "Tailnet Status",
            description: "Open the live auto-discovered Tailnet device roster",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::VoicePlayground,
        title: "Voice Playground",
        default_width: 1040.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.voice_playground",
            label: "Voice Playground",
            description: "Open the Google Cloud speech workbench for voice transcription verification",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::LocalInference,
        title: "GPT-OSS Workbench",
        default_width: 960.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.local_inference",
            label: "GPT-OSS Workbench",
            description: "Open the GPT-OSS local inference workbench and runtime controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::PsionicViz,
        title: "Psionic Mesh",
        default_width: 980.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.psionic_viz",
            label: "Psionic Mesh",
            description: "Open a derived GPT-OSS decode field built from Psionic runtime metrics",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::PsionicRemoteTraining,
        title: "Training Runs",
        default_width: 1260.0,
        default_height: 820.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.psionic_remote_training",
            label: "Training Runs",
            description: "Open the shared PGOLF, HOMEGOLF, Psion, and bounded XTRAIN training dashboard",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::XtrainExplorer,
        title: "XTRAIN Explorer",
        default_width: 1220.0,
        default_height: 800.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.xtrain_explorer",
            label: "XTRAIN Explorer",
            description: "Open the decentralized XTRAIN participant, window, checkpoint, and evidence explorer",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::ContributorBeta,
        title: "Contributor Beta",
        default_width: 1180.0,
        default_height: 780.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.contributor_beta",
            label: "Contributor Beta",
            description: "Open the bounded external contributor beta for compiled-agent benchmarks, receipts, and governed worker roles",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AttnResLab,
        title: "AttnRes Lab",
        default_width: 1120.0,
        default_height: 720.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.attnres_lab",
            label: "AttnRes Lab",
            description: "Open the replay-first AttnRes desktop lab pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::TassadarLab,
        title: "Tassadar Lab",
        default_width: 1120.0,
        default_height: 720.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.tassadar_lab",
            label: "Tassadar Lab",
            description: "Open the replay and live Tassadar desktop lab pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::RivePreview,
        title: "Rive Preview",
        default_width: 1080.0,
        default_height: 700.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.rive_preview",
            label: "Rive Preview",
            description: "Open the packaged HUD asset preview pane driven by the shared native Rive path",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::Presentation,
        title: "Presentation",
        default_width: 960.0,
        default_height: 540.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.presentation",
            label: "Presentation",
            description: "Open a slide-style HUD presentation surface backed by the packaged Rive HUD asset",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::FrameDebugger,
        title: "Frame Debugger",
        default_width: 1120.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.frame_debugger",
            label: "Frame Debugger",
            description: "Open live FPS, redraw-pressure, and renderer timing diagnostics",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AppleFmWorkbench,
        title: "Apple FM Workbench",
        default_width: 1180.0,
        default_height: 760.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.apple_fm_workbench",
            label: "Apple FM Workbench",
            description: "Open the Apple Foundation Models API workbench and bridge controls",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::AppleAdapterTraining,
        title: "Apple Adapter Training",
        default_width: 1240.0,
        default_height: 820.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.apple_adapter_training",
            label: "Apple Adapter Training",
            description: "Open the Apple adapter training operator pane and run-monitoring shell",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::EarningsScoreboard,
        title: "Earnings & Jobs",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.earnings_scoreboard",
            label: "Earnings & Jobs",
            description: "Open provider earnings, active-job, inbox, and recent history summaries",
            keybinding: Some("4"),
        }),
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_EARNINGS_JOBS,
            icon: "E",
            tooltip: "Earnings and jobs",
            shortcut: Some("4"),
        }),
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
        kind: PaneKind::ReciprocalLoop,
        title: "Reciprocal Loop",
        default_width: 860.0,
        default_height: 440.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.reciprocal_loop",
            label: "Reciprocal Loop",
            description: "Open two-key 10-sat ping-pong loop controls and metrics",
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
        kind: PaneKind::Credentials,
        title: "Credentials",
        default_width: 960.0,
        default_height: 520.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.credentials",
            label: "Credentials",
            description: "Open secure API key/env var manager with scoped runtime injection",
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
        kind: PaneKind::LogStream,
        title: "Log Stream",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.log_stream",
            label: "Log Stream",
            description: "Open replay-safe runtime logs with independent scroll and copy-all",
            keybinding: Some("5"),
        }),
        hotbar: Some(PaneHotbarSpec {
            slot: HOTBAR_SLOT_LOG_STREAM,
            icon: "L",
            tooltip: "Log stream",
            shortcut: Some("5"),
        }),
    },
    PaneSpec {
        kind: PaneKind::BuyModePayments,
        title: "Buy Mode",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.buy_mode_payments",
            label: "Buy Mode",
            description: "Open the buyer smoke-test loop, targeting state, and payment history surface",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::Nip90SentPayments,
        title: "NIP-90 Sent Payments",
        default_width: 1120.0,
        default_height: 640.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.nip90_sent_payments",
            label: "NIP-90 Sent Payments",
            description: "Open definitive buyer sent-payment totals and sats across the current relay scope",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::DataSeller,
        title: "Data Seller",
        default_width: 1160.0,
        default_height: 680.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.data_seller",
            label: "Data Seller",
            description: "Open the conversational seller shell for drafting, previewing, and publishing data listings",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::DataBuyer,
        title: "Data Buyer",
        default_width: 980.0,
        default_height: 560.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.data_buyer",
            label: "Data Buyer",
            description: "Open the narrow targeted-request surface for selecting an asset and issuing a buyer-side NIP-90 data-access request",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::DataMarket,
        title: "Data Market",
        default_width: 1120.0,
        default_height: 640.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.data_market",
            label: "Data Market",
            description: "Open the read-only relay-backed data-market catalog and lifecycle pane",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::BuyerRaceMatrix,
        title: "Buyer Race Matrix",
        default_width: 1080.0,
        default_height: 600.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.buyer_race_matrix",
            label: "Buyer Race Matrix",
            description: "Open live NIP-90 provider competition lanes with replay-ready role splits",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SellerEarningsTimeline,
        title: "Seller Earnings Timeline",
        default_width: 1120.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.seller_earnings_timeline",
            label: "Seller Earnings Timeline",
            description: "Open wallet-confirmed provider payout pulses with degraded settlement rows kept distinct",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SettlementLadder,
        title: "Settlement Ladder",
        default_width: 1120.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.settlement_ladder",
            label: "Settlement Ladder",
            description: "Open a per-request proof ladder from request observation through settlement",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::KeyLedger,
        title: "Key Ledger",
        default_width: 1160.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.key_ledger",
            label: "Key Ledger",
            description: "Open actor-by-actor NIP-90 payment and settlement activity tables",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SettlementAtlas,
        title: "Settlement Atlas",
        default_width: 1180.0,
        default_height: 660.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.settlement_atlas",
            label: "Settlement Atlas",
            description: "Open the buyer-to-provider NIP-90 payment constellation",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::SparkReplay,
        title: "Spark Replay",
        default_width: 1180.0,
        default_height: 660.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.spark_replay",
            label: "Spark Replay",
            description: "Open the scrubbable NIP-90 request-to-settlement replay field",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::RelayChoreography,
        title: "Relay Choreography",
        default_width: 1180.0,
        default_height: 660.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.relay_choreography",
            label: "Relay Choreography",
            description: "Open live relay health against persisted NIP-90 relay-hop evidence",
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
            shortcut: Some("2"),
        }),
    },
    PaneSpec {
        kind: PaneKind::SparkWallet,
        title: "WALLET",
        default_width: SPARK_PANE_WIDTH,
        default_height: SPARK_PANE_HEIGHT,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.wallet",
            label: "Wallet",
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
            description: "Open dedicated pane for creating BOLT11 Lightning invoices",
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
        kind: PaneKind::CastControl,
        title: "CAST Control",
        default_width: 940.0,
        default_height: 420.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.cast_control",
            label: "CAST Control",
            description: "Open Charms CAST operation controls, status, and receipt visibility",
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
        kind: PaneKind::Calculator,
        title: "Calculator",
        default_width: 440.0,
        default_height: 260.0,
        singleton: false,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.calculator",
            label: "Calculator",
            description: "Open a calculator pane for quick expression evaluation",
            keybinding: None,
        }),
        hotbar: None,
    },
    PaneSpec {
        kind: PaneKind::CadDemo,
        title: "CAD Demo",
        default_width: 1020.0,
        default_height: 620.0,
        singleton: true,
        startup: false,
        command: Some(PaneCommandSpec {
            id: "pane.cad_demo",
            label: "CAD Demo",
            description: "Open the CAD demo pane",
            keybinding: None,
        }),
        hotbar: None,
    },
];

#[cfg(test)]
mod tests {
    use super::{
        enabled_pane_specs, pane_kind_enabled, pane_kind_for_hotbar_slot, pane_spec,
        pane_spec_by_command_id, pane_specs, startup_pane_kinds,
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
                assert_eq!(
                    pane_spec_by_command_id(command.id).is_some(),
                    pane_kind_enabled(spec.kind),
                    "command id {} visibility should match pane feature gate",
                    command.id
                );
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
    fn project_ops_command_is_hidden_when_feature_gate_is_disabled() {
        assert!(
            !pane_kind_enabled(PaneKind::ProjectOps),
            "project ops feature gate should be off by default in tests"
        );
        assert!(pane_spec_by_command_id("pane.project_ops").is_none());
        assert!(
            !enabled_pane_specs().any(|spec| spec.kind == PaneKind::ProjectOps),
            "disabled project ops pane should not appear in enabled pane iteration"
        );
    }

    #[test]
    fn local_inference_visibility_matches_platform_contract() {
        if cfg!(target_os = "macos") {
            assert!(
                !pane_kind_enabled(PaneKind::LocalInference),
                "macOS should use Apple FM instead of exposing the GPT-OSS local inference pane"
            );
            assert!(pane_spec_by_command_id("pane.local_inference").is_none());
        } else {
            assert!(
                pane_kind_enabled(PaneKind::LocalInference),
                "non-macOS builds should keep the GPT-OSS local inference pane enabled"
            );
            assert!(pane_spec_by_command_id("pane.local_inference").is_some());
        }
    }

    #[test]
    fn psionic_mesh_command_is_registered_on_all_platforms() {
        let spec =
            pane_spec_by_command_id("pane.psionic_viz").expect("psionic mesh pane should exist");
        assert_eq!(spec.kind, PaneKind::PsionicViz);
        assert!(spec.singleton, "psionic mesh pane must be singleton");
        assert!(
            !spec.startup,
            "psionic mesh pane should stay opt-in instead of opening at startup"
        );
    }

    #[test]
    fn remote_training_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.psionic_remote_training")
            .expect("remote training command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::PsionicRemoteTraining);
        assert!(spec.singleton, "remote training pane must be singleton");
        assert!(
            !spec.startup,
            "remote training pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn xtrain_explorer_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.xtrain_explorer")
            .expect("xtrain explorer command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::XtrainExplorer);
        assert!(spec.singleton, "xtrain explorer pane must be singleton");
        assert!(
            !spec.startup,
            "xtrain explorer pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn contributor_beta_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.contributor_beta")
            .expect("contributor beta command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::ContributorBeta);
        assert!(spec.singleton, "contributor beta pane must be singleton");
        assert!(
            !spec.startup,
            "contributor beta pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn attnres_lab_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.attnres_lab")
            .expect("attnres lab command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::AttnResLab);
        assert!(spec.singleton, "attnres lab pane must be singleton");
        assert!(
            !spec.startup,
            "attnres lab pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn tassadar_lab_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.tassadar_lab")
            .expect("tassadar lab command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::TassadarLab);
        assert!(spec.singleton, "tassadar lab pane must be singleton");
        assert!(
            !spec.startup,
            "tassadar lab pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn rive_preview_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.rive_preview")
            .expect("rive preview command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::RivePreview);
        assert!(spec.singleton, "rive preview pane must be singleton");
        assert!(
            !spec.startup,
            "rive preview pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn presentation_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.presentation")
            .expect("presentation command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::Presentation);
        assert!(spec.singleton, "presentation pane must be singleton");
        assert!(
            !spec.startup,
            "presentation pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn frame_debugger_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.frame_debugger")
            .expect("frame debugger command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::FrameDebugger);
        assert!(spec.singleton, "frame debugger pane must be singleton");
        assert!(
            !spec.startup,
            "frame debugger pane should remain opt-in instead of opening at startup"
        );
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

    #[test]
    fn cad_demo_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.cad_demo")
            .expect("cad demo command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::CadDemo);
        assert!(spec.singleton, "cad demo pane must be singleton");
        assert!(
            !spec.startup,
            "cad demo pane should not auto-open during startup"
        );
    }

    #[test]
    fn mission_control_command_maps_to_singleton_startup_pane() {
        let mission_control_spec = pane_spec_by_command_id("pane.mission_control")
            .expect("mission control command should resolve to a pane spec");
        assert_eq!(mission_control_spec.kind, PaneKind::GoOnline);
        assert!(
            mission_control_spec.singleton,
            "mission control pane must be singleton"
        );
        assert!(
            mission_control_spec.startup,
            "mission control pane should auto-open during startup"
        );
    }

    #[test]
    fn provider_control_command_maps_to_singleton_non_startup_pane() {
        let provider_spec = pane_spec_by_command_id("pane.provider_control")
            .expect("provider control command should resolve to a pane spec");
        assert_eq!(provider_spec.kind, PaneKind::ProviderControl);
        assert!(
            provider_spec.singleton,
            "provider control pane must be singleton"
        );
        assert!(
            !provider_spec.startup,
            "provider control pane should stay opt-in instead of auto-opening at startup"
        );
    }

    #[test]
    fn apple_adapter_training_command_maps_to_singleton_non_startup_pane() {
        let spec = pane_spec_by_command_id("pane.apple_adapter_training")
            .expect("apple adapter training command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::AppleAdapterTraining);
        assert!(
            spec.singleton,
            "apple adapter training pane must be singleton"
        );
        assert!(
            !spec.startup,
            "apple adapter training pane should remain opt-in instead of opening at startup"
        );
    }

    #[test]
    fn calculator_command_maps_to_non_singleton_pane() {
        let spec = pane_spec_by_command_id("pane.calculator")
            .expect("calculator command should resolve to a pane spec");
        assert_eq!(spec.kind, PaneKind::Calculator);
        assert!(
            !spec.singleton,
            "calculator pane should allow multiple instances"
        );
        assert!(
            !spec.startup,
            "calculator pane should not auto-open during startup"
        );
    }
}
