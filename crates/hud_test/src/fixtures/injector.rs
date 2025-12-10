//! HudInjector: Message injection for testing
//!
//! Injects HUD messages directly into GraphView entities,
//! replacing the HTTP/WebSocket injection from TypeScript tests.

use gpui::{Entity, TestAppContext};
use hud::GraphView;
use std::time::Duration;

use crate::protocol::HudMessage;

/// Injects HUD messages directly into GraphView - equivalent to TypeScript HudInjector
pub struct HudInjector<'a> {
    view: &'a Entity<GraphView>,
    cx: &'a mut TestAppContext,
}

impl<'a> HudInjector<'a> {
    /// Create a new injector for a GraphView entity
    pub fn new(view: &'a Entity<GraphView>, cx: &'a mut TestAppContext) -> Self {
        Self { view, cx }
    }

    /// Inject a single HUD message
    pub fn inject(&mut self, message: HudMessage) {
        let json = serde_json::to_value(&message).expect("HudMessage should serialize");
        self.view.update(self.cx, |view, cx| {
            view.handle_hud_message(json, cx);
        });
        self.cx.run_until_parked();
    }

    /// Inject a sequence of messages with delay between each
    pub fn inject_sequence(&mut self, messages: Vec<HudMessage>, delay_ms: u64) {
        for msg in messages {
            self.inject(msg);
            if delay_ms > 0 {
                // Advance the test clock
                self.cx.executor().advance_clock(Duration::from_millis(delay_ms));
                self.cx.run_until_parked();
            }
        }
    }

    /// Inject raw/malformed data (for error handling tests)
    pub fn inject_raw(&mut self, data: &str) {
        let data = data.to_string();
        self.view.update(self.cx, |view, cx| {
            view.handle_raw_message(&data, cx);
        });
        self.cx.run_until_parked();
    }

    /// Simulate WebSocket disconnect
    pub fn simulate_disconnect(&mut self) {
        self.view.update(self.cx, |view, cx| {
            view.handle_disconnect(cx);
        });
        self.cx.run_until_parked();
    }

    /// Simulate WebSocket reconnect
    pub fn simulate_reconnect(&mut self) {
        self.view.update(self.cx, |view, cx| {
            view.handle_reconnect(cx);
        });
        self.cx.run_until_parked();
    }

    /// Inject multiple messages rapidly (no delay)
    pub fn inject_burst(&mut self, messages: Vec<HudMessage>) {
        for msg in messages {
            let json = serde_json::to_value(&msg).expect("HudMessage should serialize");
            self.view.update(self.cx, |view, cx| {
                view.handle_hud_message(json, cx);
            });
        }
        self.cx.run_until_parked();
    }
}
