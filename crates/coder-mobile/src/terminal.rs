//! The terminal screen a linked host's **Terminal** control opens.
//!
//! This is the entry point for the NIP-TERM terminal screen
//! ([#9733](https://github.com/OpenAgentsInc/openagents/issues/9733)). The
//! Computers surface resolves **Terminal** on a host only after the shared
//! authority check passes (the host is online and this device holds the
//! `terminal` right), then [`crate::App`] calls [`Terminal::open`] with the
//! host key and label. The screen is a Rust Native view with its own
//! instance and revisions. The world computer's HUD draws it on its
//! **Terminal** page and routes taps back through
//! `Request::TerminalActivate`; a native keyboard answers its input
//! requests the same way the Computers surface's are answered.
//!
//! Until the NIP-TERM client lands, the screen names the host and offers
//! only **Back**. Replace [`Terminal::view`] and [`Terminal::activate`] with
//! the terminal emulator; keep the entry point and the close path.
use rust_native::style::{Color, Style, TextWeight};
use rust_native::{Activation, Axis, Element, Node, TextRole, ValidatedView, View};
use serde::{Deserialize, Serialize};

/// What a terminal control asks for. Resolved only from the current view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(crate) enum Intent {
    /// Leave the terminal and return to the host's Computers screen.
    Close,
}

/// What an accepted activation did. The terminal screen adds its own
/// outcomes, such as an updated screen, beside `Closed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Outcome {
    /// Return to the Computers surface.
    Closed,
}

/// One terminal screen for one host.
pub(crate) struct Terminal {
    host: String,
    label: String,
    instance: String,
    revision: u64,
    current: Option<ValidatedView<Intent>>,
}

impl Terminal {
    /// Open the terminal screen for `host`. The caller has already checked
    /// the host's `terminal` right; the host checks it again on
    /// `terminal.open`.
    pub(crate) fn open(host: String, label: String) -> Result<Self, String> {
        let mut terminal = Self {
            host,
            label,
            instance: format!("terminal:{}", coder_connect::protocol::random_id()),
            revision: 0,
            current: None,
        };
        terminal.rebuild()?;
        Ok(terminal)
    }

    /// The current view, as JSON for the native host and the world HUD.
    pub(crate) fn view(&self) -> Option<serde_json::Value> {
        self.current
            .as_ref()
            .and_then(|view| serde_json::to_value(view.view()).ok())
    }

    /// Resolve a tap against the current view.
    pub(crate) fn activate(&mut self, event: &Activation) -> Result<Outcome, String> {
        let intent = self
            .current
            .as_ref()
            .ok_or("The terminal screen is unavailable.")?
            .activate(event)
            .map_err(|_| "The terminal screen changed. Try again.".to_owned())?
            .clone();
        match intent {
            Intent::Close => Ok(Outcome::Closed),
        }
    }

    fn rebuild(&mut self) -> Result<(), String> {
        self.revision += 1;
        let amber = |intensity: coder_ui::theme::Intensity| {
            let rgb = intensity.color();
            Color::rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
        };
        let text = |key: &str, value: String, role: TextRole| Node {
            key: key.into(),
            style: Style {
                foreground: Some(amber(if role == TextRole::Heading {
                    coder_ui::theme::Intensity::Full
                } else {
                    coder_ui::theme::Intensity::Half
                })),
                weight: (role == TextRole::Heading).then_some(TextWeight::Bold),
                ..Style::default()
            },
            element: Element::Text { value, role },
        };
        let root = Node {
            key: "terminal-screen".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    text(
                        "terminal-title",
                        format!("Terminal on {}", self.label),
                        TextRole::Heading,
                    ),
                    text(
                        "terminal-host",
                        format!(
                            "Computer key {}.",
                            self.host.get(..16).unwrap_or(&self.host)
                        ),
                        TextRole::Status,
                    ),
                    text(
                        "terminal-status",
                        "This build can't show terminals yet. Order work from the host's screen instead.".into(),
                        TextRole::Status,
                    ),
                    Node {
                        key: "terminal-close".into(),
                        style: Style {
                            foreground: Some(amber(coder_ui::theme::Intensity::Full)),
                            ..Style::default()
                        },
                        element: Element::Button {
                            label: "Back".into(),
                            enabled: true,
                            intent: Intent::Close,
                        },
                    },
                ],
            },
        };
        self.current = Some(
            View::new(self.instance.clone(), self.revision, root)
                .validate()
                .map_err(|error| error.to_string())?,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_entry_point_names_the_host_and_closes() {
        let mut terminal = Terminal::open("ab".repeat(32), "Studio".into()).unwrap();
        let view = terminal.view().unwrap();
        assert!(view.to_string().contains("Terminal on Studio"));
        assert!(view.to_string().contains(&"ab".repeat(8)));
        let event = Activation {
            instance: view["instance"].as_str().unwrap().into(),
            revision: view["revision"].as_u64().unwrap(),
            node: "terminal-close".into(),
        };
        assert_eq!(terminal.activate(&event), Ok(Outcome::Closed));
        let stale = Activation {
            revision: 99,
            ..event
        };
        assert!(terminal.activate(&stale).is_err());
    }
}
