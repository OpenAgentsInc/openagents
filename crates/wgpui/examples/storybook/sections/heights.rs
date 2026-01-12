use wgpui::Bounds;

use crate::constants::{
    BG_TILE_GAP, BG_TILE_H, BG_TILE_W, DOT_ORIGINS, DOT_SHAPES, FRAME_ANIMATIONS, FRAME_DIRECTIONS,
    FRAME_STYLES, FRAME_TILE_GAP, FRAME_TILE_H, FRAME_TILE_W, FRAME_VARIANT_H, FRAME_VARIANT_W,
    GLOW_PRESETS, ILLUMINATOR_TILE_GAP, ILLUMINATOR_TILE_H, ILLUMINATOR_TILE_W,
    LIGHT_DEMO_FRAMES_INNER_H, LIGHT_DEMO_HERO_INNER_H, LINE_DIRECTIONS, PANEL_PADDING,
    TEXT_TILE_GAP, TEXT_TILE_H, TEXT_TILE_W, TOOLCALL_DEMO_INNER_H,
};
use crate::helpers::{grid_metrics, panel_height, stacked_height};

pub(crate) fn atoms_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(140.0), // Tool & Status Atoms
        panel_height(160.0), // Mode & Model Atoms
        panel_height(180.0), // Agent Status Badges
        panel_height(180.0), // Bitcoin & Payment Atoms
        panel_height(180.0), // Nostr Protocol Atoms
        panel_height(180.0), // GitAfter Atoms
        panel_height(180.0), // Marketplace Atoms
        panel_height(180.0), // Autopilot Atoms
        panel_height(160.0), // Interactive Atoms
    ];
    stacked_height(&panels)
}

pub(crate) fn arwes_frames_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let permutations = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * FRAME_DIRECTIONS.len();
    let glow_palette = FRAME_STYLES.len() * FRAME_ANIMATIONS.len() * GLOW_PRESETS.len();
    let panels = [
        panel_height(
            grid_metrics(
                available,
                permutations,
                FRAME_TILE_W,
                FRAME_TILE_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                FRAME_STYLES.len() * 2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                FRAME_STYLES.len() * 2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                glow_palette,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                16,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                2,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                4,
                FRAME_VARIANT_W,
                FRAME_VARIANT_H,
                FRAME_TILE_GAP,
            )
            .height,
        ),
    ];
    stacked_height(&panels)
}

pub(crate) fn arwes_backgrounds_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let dots_count = DOT_SHAPES.len() * DOT_ORIGINS.len() * 2;
    let moving_count = LINE_DIRECTIONS.len() * 2;
    let panels = [
        panel_height(grid_metrics(available, dots_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 4, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(grid_metrics(available, 8, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
        panel_height(
            grid_metrics(available, moving_count, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height,
        ),
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

pub(crate) fn arwes_text_effects_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 8, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP).height),
        panel_height(grid_metrics(available, 6, TEXT_TILE_W, TEXT_TILE_H, TEXT_TILE_GAP).height),
    ];
    stacked_height(&panels)
}

pub(crate) fn arwes_illuminator_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(
            grid_metrics(
                available,
                8,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            )
            .height,
        ),
        panel_height(
            grid_metrics(
                available,
                4,
                ILLUMINATOR_TILE_W,
                ILLUMINATOR_TILE_H,
                ILLUMINATOR_TILE_GAP,
            )
            .height,
        ),
    ];
    stacked_height(&panels)
}

pub(crate) fn hud_widgets_height(bounds: Bounds) -> f32 {
    let available = (bounds.size.width - PANEL_PADDING * 2.0).max(0.0);
    let panels = [
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Scanlines
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Signal meters
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Reticles
        panel_height(grid_metrics(available, 6, BG_TILE_W, BG_TILE_H, BG_TILE_GAP).height), // Resizable panes
    ];
    stacked_height(&panels)
}

pub(crate) fn light_demo_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(LIGHT_DEMO_FRAMES_INNER_H),
        panel_height(LIGHT_DEMO_HERO_INNER_H),
    ];
    stacked_height(&panels)
}

pub(crate) fn toolcall_demo_height(_bounds: Bounds) -> f32 {
    panel_height(TOOLCALL_DEMO_INNER_H)
}

pub(crate) fn system_ui_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(180.0), // Tooltip demos
        panel_height(120.0), // StatusBar demos
        panel_height(260.0), // Notifications demos
        panel_height(200.0), // ContextMenu demo
        panel_height(240.0), // CommandPalette demo
    ];
    stacked_height(&panels)
}

pub(crate) fn chat_threads_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(480.0), // Simple Conversation
        panel_height(600.0), // Multi-Tool Workflow
        panel_height(520.0), // Code Editing Session
        panel_height(440.0), // Search & Navigation
        panel_height(320.0), // Streaming Response
        panel_height(800.0), // Complex Agent Session
        panel_height(280.0), // Error Handling
    ];
    stacked_height(&panels)
}

pub(crate) fn bitcoin_wallet_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(200.0), // Payment Method Icons
        panel_height(180.0), // Payment Status Badges
        panel_height(160.0), // Network Badges
        panel_height(200.0), // Bitcoin Amounts
        panel_height(220.0), // Balance Cards
        panel_height(300.0), // Payment Rows (Transaction History)
        panel_height(320.0), // Invoice Displays
        panel_height(400.0), // Complete Wallet Dashboard
    ];
    stacked_height(&panels)
}

pub(crate) fn nostr_protocol_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Relay Status Indicators
        panel_height(280.0), // Event Kind Badges
        panel_height(200.0), // Bech32 Entities
        panel_height(300.0), // Relay Connection List
        panel_height(320.0), // Complete Relay Dashboard
    ];
    stacked_height(&panels)
}

pub(crate) fn gitafter_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Issue Status Badges
        panel_height(180.0), // PR Status Badges
        panel_height(140.0), // Bounty Badges
        panel_height(160.0), // Stack Layer Indicators
        panel_height(180.0), // Agent Status + Type Badges
        panel_height(160.0), // Trajectory Status Badges
        panel_height(360.0), // Complete GitAfter Dashboard
    ];
    stacked_height(&panels)
}

pub(crate) fn sovereign_agents_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Threshold Key Badges
        panel_height(180.0), // Agent Schedule Badges
        panel_height(160.0), // Goal Progress Badges
        panel_height(180.0), // Tick Event Badges
        panel_height(180.0), // Skill License Badges
        panel_height(400.0), // Complete Agent Dashboard Preview
    ];
    stacked_height(&panels)
}

pub(crate) fn marketplace_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(140.0), // Market Type Badges
        panel_height(180.0), // Job Status Badges
        panel_height(160.0), // Reputation Badges
        panel_height(180.0), // Trajectory Source Badges
        panel_height(180.0), // Earnings Badges
        panel_height(400.0), // Complete Marketplace Dashboard
    ];
    stacked_height(&panels)
}

pub(crate) fn autopilot_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(180.0), // Session Status Badges
        panel_height(160.0), // APM Gauges
        panel_height(180.0), // Resource Usage Bars
        panel_height(160.0), // Daemon Status Badges
        panel_height(180.0), // Parallel Agent Badges
        panel_height(400.0), // Complete Autopilot Dashboard
    ];
    stacked_height(&panels)
}

pub(crate) fn thread_components_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Thread Headers
        panel_height(180.0), // Message Editor States
        panel_height(200.0), // Thread Feedback
        panel_height(140.0), // Entry Actions
        panel_height(140.0), // Terminal Headers
        panel_height(400.0), // Complete Thread Layout
    ];
    stacked_height(&panels)
}

pub(crate) fn sessions_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(280.0), // Session Cards (2 rows of 3)
        panel_height(120.0), // Session Breadcrumbs
        panel_height(180.0), // Session Search & Filters
        panel_height(160.0), // Session Actions
        panel_height(320.0), // Complete Session List
    ];
    stacked_height(&panels)
}

pub(crate) fn permissions_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(160.0), // Permission Decisions
        panel_height(240.0), // Permission Rules
        panel_height(280.0), // Permission History
        panel_height(200.0), // Permission Bar Variants
        panel_height(140.0), // Permission Statistics
    ];
    stacked_height(&panels)
}

pub(crate) fn apm_metrics_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(200.0), // APM Gauge Variations
        panel_height(220.0), // APM Session Rows
        panel_height(280.0), // Session Comparison
        panel_height(320.0), // APM Leaderboard
        panel_height(200.0), // APM Trends Summary
    ];
    stacked_height(&panels)
}

pub(crate) fn wallet_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(260.0), // Mnemonic Display
        panel_height(180.0), // Address Cards
        panel_height(280.0), // Transaction History
        panel_height(360.0), // Send Flow
        panel_height(420.0), // Receive Flow
    ];
    stacked_height(&panels)
}

pub(crate) fn gitafter_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(240.0), // Repository Cards
        panel_height(320.0), // Issue List
        panel_height(280.0), // PR Timeline
        panel_height(200.0), // Issue Labels & Statuses
    ];
    stacked_height(&panels)
}

pub(crate) fn marketplace_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(260.0), // Compute Providers
        panel_height(280.0), // Skills Marketplace
        panel_height(280.0), // Data Marketplace
        panel_height(180.0), // Categories & Formats Reference
    ];
    stacked_height(&panels)
}

pub(crate) fn nostr_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(320.0), // Contact Cards
        panel_height(380.0), // DM Conversations
        panel_height(280.0), // Zaps & Lightning
        panel_height(420.0), // Relay Manager Organism
        panel_height(450.0), // DM Thread Organism
        panel_height(420.0), // Zap Flow Organism
        panel_height(400.0), // Event Inspector Organism
        panel_height(180.0), // Status Reference
    ];
    stacked_height(&panels)
}

pub(crate) fn sovereign_agent_flows_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(340.0), // Agent Profiles
        panel_height(400.0), // Signing Requests
        panel_height(280.0), // Agent Status Matrix
        panel_height(450.0), // Agent State Inspector Organism
        panel_height(450.0), // Threshold Key Manager Organism
        panel_height(400.0), // Schedule Configuration Organism
        panel_height(180.0), // Type & Status Reference
    ];
    stacked_height(&panels)
}

pub(crate) fn codex_events_height(_bounds: Bounds) -> f32 {
    let panels = [
        panel_height(240.0), // Thread + Turn Events
        panel_height(300.0), // Item + Error Events
        panel_height(420.0), // Streaming Deltas
        panel_height(260.0), // Tool Interactions
        panel_height(360.0), // Plan + Diff
        panel_height(300.0), // Usage
        panel_height(300.0), // Account + MCP
        panel_height(220.0), // Notices + Raw Response
    ];
    stacked_height(&panels)
}
