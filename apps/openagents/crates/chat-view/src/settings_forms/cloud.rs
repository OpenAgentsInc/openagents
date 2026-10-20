//! Claude credentials and account data, shared by every Settings shell.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Provider(String),
    Paste,
    Consent(bool),
    Save,
    Remove,
    Export,
    AskDelete(bool),
    Delete,
    Open(String),
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub available: bool,
    pub loaded: bool,
    pub subscription_allowed: bool,
    pub saved: Option<String>,
    pub provider: String,
    /// The secret stays in the shell and never enters the view tree.
    pub pasted: bool,
    pub consent: bool,
    pub busy: bool,
    pub confirm_delete: bool,
    pub notice: Option<String>,
}

fn act(action: Action) -> Intent {
    Intent::Cloud(action)
}

/// A standalone page, drawn identically by any shell.
pub fn view(claude_page: bool, state: &State, signed_in: bool, paints: &Paints) -> Node<Intent> {
    if claude_page && state.loaded && !signed_in {
        return column(
            "settings-claude",
            vec![
                header("claude-title", "Claude", None, paints),
                row(
                    "claude-sign-in",
                    "Sign in to manage Claude",
                    None,
                    vec![button(
                        "claude-sign-in-button",
                        "Sign in",
                        Intent::Account(account::Action::SignIn),
                    )],
                    paints,
                ),
            ],
        );
    }
    column(
        if claude_page {
            "settings-claude"
        } else {
            "settings-data"
        },
        if claude_page {
            claude(state, paints)
        } else {
            data(state, signed_in, paints)
        },
    )
}

pub(super) fn claude(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header("claude-title", "Claude", None, paints)];
    if !state.loaded {
        out.push(text("claude-loading", "Loading…", paints.muted));
        return out;
    }
    if !state.available {
        out.push(text(
            "claude-unavailable",
            state
                .notice
                .as_deref()
                .unwrap_or("This server doesn't keep Claude keys."),
            paints.muted,
        ));
        return out;
    }
    let mut rows = vec![row(
        "claude-status",
        "Saved credential",
        Some(state.saved.as_deref().unwrap_or("Nothing saved.")),
        if state.saved.is_some() && !state.busy {
            vec![button("claude-remove", "Remove", act(Action::Remove))]
        } else {
            vec![]
        },
        paints,
    )];
    rows.push(row(
        "claude-login",
        "Log in on your computer",
        Some("Run claude, then /login in a terminal, or use Log in to Claude on a Cloud computer."),
        vec![],
        paints,
    ));
    let mut providers = vec![
        ("anthropic_api_key", "Anthropic API key"),
        ("bedrock_credential", "Amazon Bedrock"),
        ("vertex_credential", "Google Vertex AI"),
        ("foundry_credential", "Microsoft Foundry"),
    ];
    if state.subscription_allowed {
        providers.push(("claude_subscription_token", "Claude subscription"));
    }
    let selected = providers
        .iter()
        .position(|(id, _)| *id == state.provider)
        .unwrap_or(0);
    let choices: Vec<_> = providers
        .iter()
        .map(|(id, label)| (*label, act(Action::Provider((*id).into()))))
        .collect();
    if !state.busy {
        rows.push(wide_row(
            "claude-provider",
            "Provider",
            Some("Usage bills to your own account."),
            segmented("claude-provider", &choices, selected),
            paints,
        ));
        if state.provider == "claude_subscription_token" {
            rows.push(row("claude-subscription", "Connect your Claude subscription", Some("1. Run claude setup-token in a terminal. 2. Sign in on Anthropic's page and approve access. 3. Copy the token, then paste it here."), vec![], paints));
        }
        rows.push(row(
            "claude-key",
            "Credential",
            Some(if state.pasted {
                "Ready to save. The value is hidden."
            } else {
                "Copy your key, token, or JSON credential, then paste it here."
            }),
            vec![button(
                "claude-paste",
                "Paste credential",
                act(Action::Paste),
            )],
            paints,
        ));
        rows.push(row("claude-consent", "Save this key for my account", Some("Used only for your own tasks. Never included in exports or logs. Remove it any time."), vec![toggle("claude-consent-toggle", state.consent, act(Action::Consent(!state.consent)))], paints));
        if state.pasted && state.consent {
            rows.push(button("claude-save", "Save", act(Action::Save)));
        }
    } else {
        rows.push(text("claude-working", "Working…", paints.muted));
    }
    out.push(card("claude-card", rows, paints));
    if let Some(notice) = &state.notice {
        out.push(text("claude-notice", notice, paints.muted));
    }
    out
}

pub(super) fn data(state: &State, signed_in: bool, paints: &Paints) -> Vec<Node<Intent>> {
    if !state.loaded && !signed_in {
        return vec![
            header("data-title", "Data", None, paints),
            text("data-loading", "Loading…", paints.muted),
        ];
    }
    if !signed_in {
        return vec![
            header("data-title", "Data", None, paints),
            row(
                "data-sign-in",
                "Sign in to manage your data",
                None,
                vec![button(
                    "data-sign-in-button",
                    "Sign in",
                    Intent::Account(account::Action::SignIn),
                )],
                paints,
            ),
        ];
    }
    let mut rows = vec![row(
        "data-export",
        "Export everything",
        Some("Download your chats, projects, and account settings as JSON."),
        if state.busy {
            vec![]
        } else {
            vec![button(
                "data-export-start",
                "Download export",
                act(Action::Export),
            )]
        },
        paints,
    )];
    if state.busy {
        rows.push(text("data-working", "Working…", paints.muted));
    } else if state.confirm_delete {
        rows.push(row("data-confirm", "Delete all chats on your account?", Some("This includes archived chats and chats in every project. This can't be undone. Chats still being answered stay until they finish."), vec![
            danger_button("data-delete-confirm", "Delete all chats", act(Action::Delete), paints),
            button("data-delete-cancel", "Cancel", act(Action::AskDelete(false))),
        ], paints));
    } else {
        rows.push(row(
            "data-delete",
            "Delete all chats",
            Some("Remove your saved chats for good."),
            vec![danger_button(
                "data-delete-ask",
                "Delete all chats…",
                act(Action::AskDelete(true)),
                paints,
            )],
            paints,
        ));
    }
    let mut out = vec![
        header("data-title", "Data", None, paints),
        card("data-card", rows, paints),
    ];
    if let Some(notice) = &state.notice {
        out.push(text("data-notice", notice, paints.muted));
    }
    out
}
