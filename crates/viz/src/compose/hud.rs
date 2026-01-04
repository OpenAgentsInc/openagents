//! HUD layout system
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  ZONE A: Status bar (venue, cost, latency)                  │
//! ├───────────────┬─────────────────────────┬───────────────────┤
//! │   ZONE B      │        ZONE C           │      ZONE D       │
//! │   I/O Load    │   Pipeline / Tokens     │   Heat / Topo     │
//! │   (Flow)      │   (Pulse + Fill)        │   (Heat + Topo)   │
//! ├───────────────┴─────────────────────────┴───────────────────┤
//! │  ZONE E: Timeline scrubber + event log                      │
//! └─────────────────────────────────────────────────────────────┘
//! ```

use wgpui::Bounds;

use crate::grammar::sub_bounds;

/// HUD zone identifier
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HudZone {
    /// Top status bar
    StatusBar,
    /// Left panel (I/O, flow)
    IoPanel,
    /// Center panel (pipeline, tokens)
    MainPanel,
    /// Right panel (heat, topology)
    DetailPanel,
    /// Bottom timeline
    Timeline,
}

/// HUD layout configuration
pub struct HudLayout {
    pub status_bar_height: f32,
    pub timeline_height: f32,
    pub io_panel_width: f32,
    pub detail_panel_width: f32,
    pub padding: f32,
}

impl Default for HudLayout {
    fn default() -> Self {
        Self {
            status_bar_height: 32.0,
            timeline_height: 48.0,
            io_panel_width: 0.2,  // 20% of width
            detail_panel_width: 0.25, // 25% of width
            padding: 8.0,
        }
    }
}

impl HudLayout {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_status_bar_height(mut self, height: f32) -> Self {
        self.status_bar_height = height;
        self
    }

    pub fn with_timeline_height(mut self, height: f32) -> Self {
        self.timeline_height = height;
        self
    }

    /// Compute bounds for a specific zone
    pub fn zone_bounds(&self, zone: HudZone, total: Bounds) -> Bounds {
        let status_h_pct = self.status_bar_height / total.size.height;
        let timeline_h_pct = self.timeline_height / total.size.height;
        let middle_h_pct = 1.0 - status_h_pct - timeline_h_pct;

        match zone {
            HudZone::StatusBar => {
                sub_bounds(total, 0.0, 0.0, 1.0, status_h_pct)
            }
            HudZone::IoPanel => {
                sub_bounds(
                    total,
                    0.0,
                    status_h_pct,
                    self.io_panel_width,
                    middle_h_pct,
                )
            }
            HudZone::MainPanel => {
                let main_width = 1.0 - self.io_panel_width - self.detail_panel_width;
                sub_bounds(
                    total,
                    self.io_panel_width,
                    status_h_pct,
                    main_width,
                    middle_h_pct,
                )
            }
            HudZone::DetailPanel => {
                sub_bounds(
                    total,
                    1.0 - self.detail_panel_width,
                    status_h_pct,
                    self.detail_panel_width,
                    middle_h_pct,
                )
            }
            HudZone::Timeline => {
                sub_bounds(total, 0.0, 1.0 - timeline_h_pct, 1.0, timeline_h_pct)
            }
        }
    }

    /// Get all zones as (zone, bounds) pairs
    pub fn all_zones(&self, total: Bounds) -> Vec<(HudZone, Bounds)> {
        vec![
            (HudZone::StatusBar, self.zone_bounds(HudZone::StatusBar, total)),
            (HudZone::IoPanel, self.zone_bounds(HudZone::IoPanel, total)),
            (HudZone::MainPanel, self.zone_bounds(HudZone::MainPanel, total)),
            (HudZone::DetailPanel, self.zone_bounds(HudZone::DetailPanel, total)),
            (HudZone::Timeline, self.zone_bounds(HudZone::Timeline, total)),
        ]
    }
}
