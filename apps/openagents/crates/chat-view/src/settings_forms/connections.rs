//! Connections, vault entry, and terminal sign-in forms shared by GPUI shells.

use rust_native::Node;
use serde::{Deserialize, Serialize};

use super::{Intent, Load, Page, Paints, button, enabled_button, header, row, section, text};

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Features {
    pub connections: bool,
    pub vault: bool,
    pub terminal: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Google {
    pub status: String,
    pub email: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
pub struct Waiting {
    pub code: String,
    pub app: String,
    pub computer: String,
    pub csrf: String,
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Terminal {
    pub since: u64,
    pub install: String,
    pub login: String,
    pub waiting: Vec<Waiting>,
    pub computer: Option<String>,
    pub choice: Option<String>,
    pub csrf: String,
    pub chats: usize,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub features: Features,
    pub google: Load<Google>,
    pub terminal: Load<Terminal>,
    pub confirm_remove: bool,
    pub busy: bool,
    pub notice: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Reload,
    Connect,
    ConfirmRemove(bool),
    Remove,
    OpenVault,
    Copy(String),
    Decide {
        code: String,
        csrf: String,
        approve: bool,
    },
    Sync {
        computer: String,
        csrf: String,
        choice: String,
    },
}

pub fn visible(page: Page, features: &Features) -> bool {
    match page {
        Page::Connections => features.connections,
        Page::Vault => features.vault,
        Page::Terminal => features.terminal,
        _ => true,
    }
}

fn act(action: Action) -> Intent {
    Intent::Connections(action)
}

/// The page's shared Rust Native view, mounted by the web page host.
pub fn view(page: Page, state: &State, paints: &Paints) -> Node<Intent> {
    super::column(
        &format!("settings-{}", page.slug()),
        self::page(page, state, paints),
    )
}

pub(super) fn page(page: Page, state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header("connect-heading", page.label(), None, paints)];
    if !visible(page, &state.features) {
        return out;
    }
    if let Some(notice) = &state.notice {
        out.push(text("connect-notice", notice, paints.muted));
    }
    match page {
        Page::Connections => match &state.google {
            Load::Ready(google) => {
                let connected = matches!(google.status.as_str(), "connected" | "reconnect");
                let mut controls = vec![];
                if google.status != "unavailable" {
                    controls.push(enabled_button(
                        "google-connect",
                        if connected {
                            "Connect again"
                        } else {
                            "Connect Google"
                        },
                        !state.busy,
                        act(Action::Connect),
                    ));
                }
                if connected {
                    controls.push(enabled_button(
                        "google-remove",
                        "Remove",
                        !state.busy,
                        act(Action::ConfirmRemove(true)),
                    ));
                }
                let detail = match google.status.as_str() {
                    "connected" => google.email.as_deref().unwrap_or("Connected"),
                    "reconnect" => "Connect Google again to keep reading your files.",
                    "unavailable" => "Google can't connect right now. Try again later.",
                    _ => "Connect Google to read your Drive files in chat.",
                };
                out.push(section(
                    "google",
                    "Google Drive",
                    vec![row(
                        "google-account",
                        "Google",
                        Some(detail),
                        controls,
                        paints,
                    )],
                    paints,
                ));
                if state.confirm_remove {
                    out.push(row(
                        "google-confirm",
                        "Remove Google?",
                        Some("Your projects keep their sources. Connect again to use them."),
                        vec![
                            enabled_button(
                                "google-remove-confirm",
                                "Remove Google",
                                !state.busy,
                                act(Action::Remove),
                            ),
                            button(
                                "google-remove-cancel",
                                "Cancel",
                                act(Action::ConfirmRemove(false)),
                            ),
                        ],
                        paints,
                    ));
                }
            }
            Load::Failed(_) => out.push(button("google-retry", "Try again", act(Action::Reload))),
            _ => out.push(text("google-loading", "Loading connections…", paints.muted)),
        },
        Page::Vault => out.push(section(
            "vault",
            "Vault (only you)",
            vec![row(
                "vault-open",
                "Your vault",
                Some("Open your locked files on this device."),
                vec![button(
                    "vault-open-button",
                    "Open vault",
                    act(Action::OpenVault),
                )],
                paints,
            )],
            paints,
        )),
        Page::Terminal => match &state.terminal {
            Load::Ready(terminal) => {
                for (key, title, command) in [
                    ("install", "1. Install Coder", &terminal.install),
                    ("login", "2. Log in from your terminal", &terminal.login),
                ] {
                    out.push(row(
                        &format!("terminal-{key}"),
                        title,
                        Some(command),
                        vec![button(
                            &format!("terminal-copy-{key}"),
                            "Copy",
                            act(Action::Copy(command.clone())),
                        )],
                        paints,
                    ));
                }
                if let Some(computer) = &terminal.computer {
                    out.push(text(
                        "terminal-computer",
                        &format!("Coder on {computer} is connected."),
                        paints.text,
                    ));
                    let detail = match terminal.choice.as_deref() {
                        Some("all") => format!(
                            "Syncing all chats. {} chats in your sidebar.",
                            terminal.chats
                        ),
                        Some("local") => "Chats stay on this computer.".into(),
                        _ => "Choose where your chats live.".into(),
                    };
                    out.push(row(
                        "terminal-sync",
                        "3. Choose where your chats live",
                        Some(&detail),
                        [
                            ("all", "Sync all my chats"),
                            ("local", "Keep chats on this computer"),
                        ]
                        .into_iter()
                        .map(|(choice, label)| {
                            enabled_button(
                                &format!("terminal-sync-{choice}"),
                                label,
                                !state.busy,
                                act(Action::Sync {
                                    computer: computer.clone(),
                                    csrf: terminal.csrf.clone(),
                                    choice: choice.into(),
                                }),
                            )
                        })
                        .collect(),
                        paints,
                    ));
                } else if terminal.waiting.is_empty() {
                    out.push(text(
                        "terminal-waiting",
                        "Waiting for your terminal. This page updates by itself.",
                        paints.muted,
                    ));
                }
                for (i, request) in terminal.waiting.iter().enumerate() {
                    out.push(row(
                        &format!("terminal-request-{i}"),
                        &format!("{} on {}", request.app, request.computer),
                        Some(&format!(
                            "Your terminal shows {}. Approve only if the code matches.",
                            request.code
                        )),
                        [(true, "Approve"), (false, "Deny")]
                            .into_iter()
                            .map(|(approve, label)| {
                                enabled_button(
                                    &format!("terminal-{i}-{approve}"),
                                    label,
                                    !state.busy,
                                    act(Action::Decide {
                                        code: request.code.clone(),
                                        csrf: request.csrf.clone(),
                                        approve,
                                    }),
                                )
                            })
                            .collect(),
                        paints,
                    ));
                }
            }
            Load::Failed(_) => out.push(button("terminal-retry", "Try again", act(Action::Reload))),
            _ => out.push(text(
                "terminal-loading",
                "Loading terminal sign-in…",
                paints.muted,
            )),
        },
        _ => {}
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn hidden_features_have_no_controls() {
        let state = State::default();
        for page in [Page::Connections, Page::Vault, Page::Terminal] {
            assert!(!visible(page, &state.features));
            assert_eq!(self::page(page, &state, &Paints::default()).len(), 1);
        }
    }
    #[test]
    fn approval_shows_the_computer_and_code_and_scoped_actions() {
        let state = State {
            features: Features {
                terminal: true,
                ..Default::default()
            },
            terminal: Load::Ready(Terminal {
                waiting: vec![Waiting {
                    code: "BCDF-GHJK".into(),
                    app: "Coder".into(),
                    computer: "Scratch".into(),
                    csrf: "ticket".into(),
                }],
                ..Default::default()
            }),
            ..Default::default()
        };
        let nodes = page(Page::Terminal, &state, &Paints::default());
        let json = serde_json::to_string(&nodes).unwrap();
        assert!(json.contains("BCDF-GHJK") && json.contains("Scratch"));
        assert!(json.contains("Approve") && json.contains("Deny"));
        assert!(json.contains("ticket"));
    }
}
