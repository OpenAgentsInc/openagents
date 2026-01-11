//! Usage display panel (generic agent usage)

use wgpui::{Bounds, Component, EventContext, EventResult, InputEvent, PaintContext};

/// Session usage statistics
#[derive(Clone, Debug, Default)]
pub struct SessionUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_creation_tokens: u64,
    pub total_cost_usd: f64,
    pub duration_ms: u64,
    pub duration_api_ms: u64,
    pub num_turns: u64,
}

/// Usage limit info
#[derive(Clone, Debug)]
pub struct UsageLimit {
    pub name: String,
    pub used: u64,
    pub limit: u64,
    pub percent_used: f64,
    pub resets_at: String,
}

/// Agent usage display panel
pub struct UsagePanel {
    model: String,
    context_used: u64,
    context_total: u64,
    session: SessionUsage,
    limits: Vec<UsageLimit>,
    autopilot_session_id: Option<String>,
    sdk_session_ids: Option<autopilot_service::SdkSessionIds>,
}

impl UsagePanel {
    pub fn new() -> Self {
        Self {
            model: "Agent".to_string(),
            context_used: 0,
            context_total: 0,
            session: SessionUsage::default(),
            limits: Vec::new(),
            autopilot_session_id: None,
            sdk_session_ids: None,
        }
    }

    pub fn set_model(&mut self, model: &str) {
        self.model = model.to_string();
    }

    pub fn set_context(&mut self, used: u64, total: u64) {
        self.context_used = used;
        self.context_total = total;
    }

    pub fn set_session(&mut self, session: SessionUsage) {
        self.session = session;
    }

    pub fn set_limits(&mut self, limits: Vec<UsageLimit>) {
        self.limits = limits;
    }

    pub fn add_tokens(&mut self, input: u64, output: u64, cache_read: u64, cache_create: u64) {
        self.session.input_tokens += input;
        self.session.output_tokens += output;
        self.session.cache_read_tokens += cache_read;
        self.session.cache_creation_tokens += cache_create;
    }

    pub fn set_session_ids(
        &mut self,
        autopilot_id: String,
        sdk_ids: autopilot_service::SdkSessionIds,
    ) {
        self.autopilot_session_id = Some(autopilot_id);
        self.sdk_session_ids = Some(sdk_ids);
    }
}

impl Default for UsagePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl Component for UsagePanel {
    fn paint(&mut self, _bounds: Bounds, _cx: &mut PaintContext) {
        // Minimal implementation - show model info
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
