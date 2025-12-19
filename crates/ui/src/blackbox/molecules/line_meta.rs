use maud::{Markup, html};

use super::super::atoms::{
    CallType, call_id_badge, latency_badge, step_badge, timestamp_badge_elapsed,
};

/// Metadata badges for a log line: step + timestamp + call_id + latency.
pub struct LineMeta {
    pub step: Option<u32>,
    pub elapsed: Option<(u8, u8, u8)>,
    pub call_id: Option<(String, CallType)>,
    pub latency_ms: Option<u32>,
}

impl LineMeta {
    pub fn new() -> Self {
        Self {
            step: None,
            elapsed: None,
            call_id: None,
            latency_ms: None,
        }
    }

    pub fn step(mut self, step: u32) -> Self {
        self.step = Some(step);
        self
    }

    pub fn elapsed(mut self, hours: u8, minutes: u8, seconds: u8) -> Self {
        self.elapsed = Some((hours, minutes, seconds));
        self
    }

    pub fn call_id(mut self, id: &str, call_type: CallType) -> Self {
        self.call_id = Some((id.to_string(), call_type));
        self
    }

    pub fn latency(mut self, ms: u32) -> Self {
        self.latency_ms = Some(ms);
        self
    }

    pub fn build(self) -> Markup {
        html! {
            span class="inline-flex items-center gap-3 text-xs text-muted-foreground" {
                @if let Some(s) = self.step {
                    (step_badge(s))
                }
                @if let Some((h, m, s)) = self.elapsed {
                    (timestamp_badge_elapsed(h, m, s))
                }
                @if let Some((id, call_type)) = &self.call_id {
                    (call_id_badge(id, *call_type))
                }
                @if let Some(ms) = self.latency_ms {
                    (latency_badge(ms))
                }
            }
        }
    }
}

impl Default for LineMeta {
    fn default() -> Self {
        Self::new()
    }
}
