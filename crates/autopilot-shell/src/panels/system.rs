//! System panel for the right sidebar

use super::{SessionUsage, UsageLimit, UsagePanel};
use crate::dock::{DockPosition, Panel};
use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext};

/// Right sidebar panel with agent usage stats
pub struct SystemPanel {
    usage_panel: UsagePanel,
}

impl SystemPanel {
    pub fn new() -> Self {
        Self {
            usage_panel: UsagePanel::new(),
        }
    }

    /// Update the agent usage data
    pub fn update_usage(&mut self, model: &str, context_used: u64, context_total: u64) {
        self.usage_panel.set_model(model);
        self.usage_panel.set_context(context_used, context_total);
    }

    /// Update session usage stats
    pub fn update_session(&mut self, session: SessionUsage) {
        self.usage_panel.set_session(session);
    }

    /// Update usage limits
    pub fn update_limits(&mut self, limits: Vec<UsageLimit>) {
        self.usage_panel.set_limits(limits);
    }

    /// Add tokens to session
    #[allow(dead_code)]
    pub fn add_tokens(&mut self, input: u64, output: u64, cache_read: u64, cache_create: u64) {
        self.usage_panel
            .add_tokens(input, output, cache_read, cache_create);
    }

    /// Update session IDs for display
    pub fn update_session_ids(
        &mut self,
        autopilot_session_id: String,
        sdk_session_ids: autopilot_service::SdkSessionIds,
    ) {
        self.usage_panel
            .set_session_ids(autopilot_session_id, sdk_session_ids);
    }

    /// Get mutable access to UsagePanel for direct updates
    #[allow(dead_code)]
    pub fn usage_panel_mut(&mut self) -> &mut UsagePanel {
        &mut self.usage_panel
    }
}

impl Default for SystemPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Panel for SystemPanel {
    fn panel_id(&self) -> &'static str {
        "system"
    }

    fn title(&self) -> &str {
        "System"
    }

    fn preferred_position(&self) -> DockPosition {
        DockPosition::Right
    }

    fn paint(&mut self, bounds: Bounds, cx: &mut PaintContext) {
        // No frame here - parent already draws one. Dense layout.
        let padding = 8.0;
        let usage_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y,
            bounds.size.width - padding * 2.0,
            bounds.size.height,
        );
        self.usage_panel.paint(usage_bounds, cx);
    }

    fn event(
        &mut self,
        _event: &InputEvent,
        _bounds: Bounds,
        _cx: &mut EventContext,
    ) -> EventResult {
        EventResult::Ignored
    }
}
