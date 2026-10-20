//! A fixture approval in the demo's working chat: the decision panel the
//! phone shows (`decision-panel`), drawn through the desktop's Rust Native
//! GPUI adapter (`openagents_chat_view::rust_native_gpui`, #12218).

use decision_panel::{ALLOWED, Flow, Paints, Panel, Prompt, Risk, Step};
use gpui::Hsla;
use openagents_chat_view::theme::Theme;
use rust_native::style::Color;

/// The demo chat whose last turn waits on this approval.
pub const CHAT: &str = "chat-veil";

pub struct Approval {
    flow: Flow,
    answer: Option<String>,
}

impl Approval {
    pub fn new() -> Self {
        Self {
            flow: Flow::approval_step(
                "Run the checks before I open the pull request?",
                Prompt {
                    tool: "shell".into(),
                    command: "cargo test -p openagents-ui".into(),
                    cwd: "/Users/you/code/openagents".into(),
                    reason: "The veil change touches every transcript row.".into(),
                    risk: Risk::Low,
                    always: Some("cargo test in this project".into()),
                },
            ),
            answer: None,
        }
    }

    pub fn panel(&self, theme: &Theme) -> Panel {
        self.flow
            .view("demo", self.answer.is_none(), &paints(theme))
    }

    /// What the person chose, once they chose.
    pub fn answered(&self) -> Option<&'static str> {
        self.answer.as_deref().map(|answer| {
            if answer == ALLOWED {
                "Allowed once"
            } else if answer.starts_with("Approved") {
                "Always allowed"
            } else {
                "Denied"
            }
        })
    }

    /// Run the panel control a press named.
    pub fn press(&mut self, key: &str, theme: &Theme) {
        let panel = self.panel(theme);
        let Some(control) = panel
            .controls
            .iter()
            .find_map(|(k, control)| (k == key).then_some(*control))
        else {
            return;
        };
        if let Step::Done(answer) = self.flow.control(control) {
            self.answer = Some(answer);
        }
    }
}

/// The panel's colors from the desktop theme.
fn paints(theme: &Theme) -> Paints {
    Paints {
        card: native(theme.surface_card),
        risk_low: native(theme.success.opacity(0.22)),
        risk_medium: native(theme.warning.opacity(0.22)),
        risk_high: native(theme.danger.opacity(0.22)),
    }
}

fn native(color: Hsla) -> Color {
    let rgba = gpui::Rgba::from(color);
    let channel = |value: f32| (value.clamp(0., 1.) * 255.).round() as u8;
    Color {
        red: channel(rgba.r),
        green: channel(rgba.g),
        blue: channel(rgba.b),
        alpha: channel(rgba.a),
    }
}
