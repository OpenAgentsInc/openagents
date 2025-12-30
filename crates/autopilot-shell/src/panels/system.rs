//! System panel for the right sidebar

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext};
use crate::dock::{DockPosition, Panel};
use super::{ClaudeUsage, SessionUsage, UsageLimit};

/// Right sidebar panel with Claude usage stats
pub struct SystemPanel {
    claude_usage: ClaudeUsage,
}

impl SystemPanel {
    pub fn new() -> Self {
        Self {
            claude_usage: ClaudeUsage::new(),
        }
    }

    /// Update the Claude usage data
    pub fn update_usage(&mut self, model: &str, context_used: u64, context_total: u64) {
        self.claude_usage.set_model(model);
        self.claude_usage.set_context(context_used, context_total);
    }

    /// Update session usage stats
    pub fn update_session(&mut self, session: SessionUsage) {
        self.claude_usage.set_session(session);
    }

    /// Update usage limits
    pub fn update_limits(&mut self, limits: Vec<UsageLimit>) {
        self.claude_usage.set_limits(limits);
    }

    /// Add tokens to session
    pub fn add_tokens(&mut self, input: u64, output: u64, cache_read: u64, cache_create: u64) {
        self.claude_usage.add_tokens(input, output, cache_read, cache_create);
    }

    /// Update session IDs for display
    pub fn update_session_ids(
        &mut self,
        autopilot_session_id: String,
        sdk_session_ids: autopilot_service::SdkSessionIds,
    ) {
        self.claude_usage
            .set_session_ids(autopilot_session_id, sdk_session_ids);
    }

    /// Get mutable access to ClaudeUsage for direct updates
    pub fn claude_usage_mut(&mut self) -> &mut ClaudeUsage {
        &mut self.claude_usage
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
        // Claude Usage at top, content-hugging.
        let padding = 8.0;
        let usage_bounds = Bounds::new(
            bounds.origin.x + padding,
            bounds.origin.y,
            bounds.size.width - padding * 2.0,
            bounds.size.height,
        );
        self.claude_usage.paint(usage_bounds, cx);
    }

    fn event(&mut self, _event: &InputEvent, _bounds: Bounds, _cx: &mut EventContext) -> EventResult {
        EventResult::Ignored
    }
}
