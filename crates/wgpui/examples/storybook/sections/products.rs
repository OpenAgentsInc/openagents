use wgpui::components::atoms::{
    AgentScheduleBadge, AgentStatus, AgentStatusBadge, AgentType, AmountDirection, ApmGauge,
    Bech32Entity, Bech32Type, BitcoinAmount, BitcoinNetwork, BitcoinUnit, BountyBadge,
    BountyStatus, ContributionStatus, DaemonStatus, DaemonStatusBadge, EarningsBadge, EarningsType,
    EventKind, EventKindBadge, GoalPriority, GoalProgressBadge, GoalStatus, IssueStatus,
    IssueStatusBadge, JobStatus, JobStatusBadge, LicenseStatus, MarketType, MarketTypeBadge, Model,
    NetworkBadge, ParallelAgentBadge, ParallelAgentStatus, PaymentMethod, PaymentMethodIcon,
    PaymentStatus, PaymentStatusBadge, PrStatus, PrStatusBadge, RelayStatus, RelayStatusBadge,
    RelayStatusDot, ReputationBadge, ResourceType, ResourceUsageBar, SessionStatus,
    SessionStatusBadge, SkillLicenseBadge, SkillType, StackLayerBadge, StackLayerStatus,
    ThresholdKeyBadge, TickEventBadge, TickOutcome, ToolStatus, ToolType, TrajectorySource,
    TrajectorySourceBadge, TrajectoryStatus, TrajectoryStatusBadge, TriggerType, TrustTier,
};
use wgpui::components::molecules::{
    BalanceCard, DiffType, InvoiceDisplay, InvoiceInfo, InvoiceType, PaymentDirection, PaymentInfo,
    PaymentRow, RelayInfo, RelayRow, WalletBalance,
};
use wgpui::components::organisms::{
    AssistantMessage, DiffLine, DiffLineKind, DiffToolCall, SearchMatch, SearchToolCall,
    TerminalToolCall, ToolCallCard, UserMessage,
};
use wgpui::{Bounds, Component, PaintContext, Point, Quad, theme};

use crate::constants::SECTION_GAP;
use crate::helpers::{draw_panel, panel_height, panel_stack};
use crate::state::Storybook;

mod autopilot;
mod bitcoin_wallet;
mod chat_threads;
mod gitafter;
mod marketplace;
mod nostr_protocol;
mod sovereign_agents;
