//! Account credentials as shared Rust Native forms. Secrets never enter a view tree.

use rust_native::Node;
use serde::{Deserialize, Serialize};

use super::{Intent, Load, Page, Paints, button, card, header, row, text};

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Entry {
    pub status: Option<String>,
    pub scopes: Option<Scopes>,
    pub id: String,
    #[serde(deserialize_with = "name_of")]
    pub name: String,
    pub current: bool,
    pub revoked: Option<String>,
    pub created: Option<String>,
    pub created_at: Option<u64>,
    pub last_used_at: Option<u64>,
    pub expires_at: Option<u64>,
}

fn name_of<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<String, D::Error> {
    Ok(Option::<String>::deserialize(deserializer)?.unwrap_or_default())
}

#[derive(Clone, Debug, Default, PartialEq, Deserialize)]
#[serde(default)]
pub struct Scopes {
    pub actions: Option<Vec<String>>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub saved: bool,
}

pub const SCOPES: [(&str, &str); 6] = [
    ("responses", "Call models"),
    ("models:read", "List models and prices"),
    ("usage:read", "Read usage and balance"),
    ("keys", "Make and revoke keys"),
    ("billing", "Plans and top-ups"),
    ("workspace", "Members and workspace settings"),
];

#[derive(Clone, Debug, PartialEq)]
pub struct State {
    pub signed_in: bool,
    pub actions: Vec<String>,
    pub monthly_limit: String,
    pub providers: Option<Vec<Provider>>,
    pub paste_available: bool,
    pub billing_enabled: bool,
    pub entries: Load<Vec<Entry>>,
    pub busy: bool,
    pub copy_available: bool,
    pub notice: Option<String>,
    pub confirm: Option<String>,
    pub expires_in_days: Option<u64>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            signed_in: false,
            actions: SCOPES[..3].iter().map(|(s, _)| (*s).into()).collect(),
            monthly_limit: String::new(),
            providers: None,
            paste_available: false,
            billing_enabled: false,
            entries: Load::default(),
            busy: false,
            copy_available: false,
            notice: None,
            confirm: None,
            expires_in_days: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Action {
    Refresh,
    Create,
    Copy,
    Dismiss,
    Expiry(Option<u64>),
    Confirm(Option<String>),
    Revoke(String),
    Scope(String),
    Limit,
    ProviderPaste(String),
    ProviderRemove(String),
    SignIn,
    Open(Page),
    Settings,
}

fn act(action: Action) -> Intent {
    Intent::Keys(action)
}

pub fn is_page(page: Page) -> bool {
    matches!(page, Page::ApiKeys | Page::SignInKeys | Page::AgentAccounts)
}

pub fn list_path(page: Page) -> &'static str {
    match page {
        Page::ApiKeys => "/v1/keys?kind=api",
        Page::SignInKeys => "/v1/keys",
        Page::AgentAccounts => "/v1/agents/claimed",
        _ => unreachable!("not a credentials page"),
    }
}

/// Draw only the credential forms, without linking the other Settings pages.
pub fn view(page: Page, state: &State, paints: &Paints) -> Node<Intent> {
    super::column(
        &format!("settings-{}", page.slug()),
        self::page(page, state, paints),
    )
}

pub(super) fn page(page: Page, state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let agents = page == Page::AgentAccounts;
    let mut out = vec![header("keys-title", page.label(), None, paints)];
    out.push(card(
        "keys-navigation",
        vec![row(
            "keys-navigation-row",
            "Settings",
            None,
            [Page::ApiKeys, Page::SignInKeys, Page::AgentAccounts]
                .into_iter()
                .map(|page| {
                    button(
                        &format!("keys-nav-{}", page.slug()),
                        page.label(),
                        act(Action::Open(page)),
                    )
                })
                .chain([button(
                    "keys-settings",
                    "All settings",
                    act(Action::Settings),
                )])
                .collect(),
            paints,
        )],
        paints,
    ));
    if !state.signed_in {
        out.push(card(
            "keys-signed-out",
            vec![row(
                "keys-sign-in-row",
                "Sign in",
                Some("Sign in to manage your keys and agent accounts."),
                vec![button("keys-sign-in", "Sign in", act(Action::SignIn))],
                paints,
            )],
            paints,
        ));
        return out;
    }
    if let Some(notice) = &state.notice {
        out.push(text("keys-notice", notice, paints.muted));
    }
    if state.copy_available {
        out.push(card(
            "keys-created",
            vec![row(
                "keys-secret",
                "••••••••••••••••",
                Some("Copy your new key now. You can't copy it again after leaving this page."),
                vec![
                    button("keys-copy", "Copy key", act(Action::Copy)),
                    button("keys-dismiss", "Done", act(Action::Dismiss)),
                ],
                paints,
            )],
            paints,
        ));
    }
    if page == Page::ApiKeys && !state.busy {
        let scopes = SCOPES
            .iter()
            .enumerate()
            .filter(|(_, (scope, _))| *scope != "billing" || state.billing_enabled)
            .map(|(i, (scope, label))| {
                row(
                    &format!("keys-scope-row-{i}"),
                    label,
                    None,
                    vec![super::toggle(
                        &format!("keys-scope-{i}"),
                        state.actions.iter().any(|s| s == scope),
                        act(Action::Scope((*scope).into())),
                    )],
                    paints,
                )
            })
            .collect();
        out.push(card("keys-scopes", scopes, paints));
        out.push(card(
            "keys-limit",
            vec![row(
                "keys-limit-row",
                "Monthly spending limit",
                Some(&if state.monthly_limit.is_empty() {
                    "No limit".into()
                } else {
                    format!("${} each month", state.monthly_limit)
                }),
                vec![button("keys-limit-edit", "Change", act(Action::Limit))],
                paints,
            )],
            paints,
        ));
    }
    if !state.busy {
        out.push(card(
            "keys-make",
            vec![row(
                "keys-make-row",
                if agents {
                    "Claim an agent"
                } else {
                    "Make a key"
                },
                Some(if agents {
                    "Enter the claim code your agent received when it signed up."
                } else {
                    "Choose Create key and enter a name."
                }),
                vec![button(
                    "keys-create",
                    if agents { "Claim" } else { "Create key" },
                    act(Action::Create),
                )],
                paints,
            )],
            paints,
        ));
        if page == Page::SignInKeys {
            out.push(card(
                "keys-expiry",
                vec![row(
                    "keys-expiry-row",
                    "Expires",
                    Some(match state.expires_in_days {
                        Some(30) => "In 30 days",
                        Some(90) => "In 90 days",
                        _ => "Never",
                    }),
                    [
                        (None, "Never"),
                        (Some(30), "30 days"),
                        (Some(90), "90 days"),
                    ]
                    .into_iter()
                    .enumerate()
                    .map(|(i, (days, label))| {
                        button(
                            &format!("keys-expiry-{i}"),
                            label,
                            act(Action::Expiry(days)),
                        )
                    })
                    .collect(),
                    paints,
                )],
                paints,
            ));
        }
    }
    let mut rows = match &state.entries {
        Load::Ready(entries) => entries
            .iter()
            .map(|entry| {
                let id = &entry.id;
                let mut controls = Vec::new();
                if !state.busy && entry.revoked.is_none() {
                    if state.confirm.as_ref() == Some(id) {
                        controls.push(button(
                            &format!("keys-revoke-{id}"),
                            "Confirm revoke",
                            act(Action::Revoke(id.clone())),
                        ));
                        controls.push(button(
                            &format!("keys-cancel-{id}"),
                            "Cancel",
                            act(Action::Confirm(None)),
                        ));
                    } else {
                        controls.push(button(
                            &format!("keys-ask-{id}"),
                            "Revoke",
                            act(Action::Confirm(Some(id.clone()))),
                        ));
                    }
                }
                let paused = entry.status.as_deref() == Some("paused");
                let mut detail = if entry.current {
                    "This key signs in to your current session. Revoking it signs you out."
                        .to_owned()
                } else if entry.revoked.is_some() {
                    "Revoked".to_owned()
                } else if let Some(day) = &entry.created {
                    format!("Created {}", day.get(..10).unwrap_or(day))
                } else {
                    let date = |at: u64| {
                        chrono::DateTime::from_timestamp(i64::try_from(at).unwrap_or(0), 0)
                            .map(|at| at.format("%Y-%m-%d").to_string())
                            .unwrap_or_default()
                    };
                    let mut detail = entry
                        .created_at
                        .map(|at| format!("Created {}", date(at)))
                        .unwrap_or_else(|| if paused { "Paused" } else { "Active" }.into());
                    if let Some(at) = entry.expires_at {
                        detail.push_str(&format!(" · Expires {}", date(at)));
                    }
                    if let Some(at) = entry.last_used_at {
                        detail.push_str(&format!(" · Last used {}", date(at)));
                    }
                    detail
                };
                if paused && detail != "Paused" {
                    detail.push_str(" · Paused");
                }
                if let Some(actions) = entry.scopes.as_ref().and_then(|s| s.actions.as_ref()) {
                    let words: Vec<_> = SCOPES
                        .iter()
                        .filter(|(s, _)| actions.iter().any(|a| a == s))
                        .map(|(_, label)| *label)
                        .collect();
                    detail.push_str(" · ");
                    detail.push_str(&if words.is_empty() {
                        "Limited".into()
                    } else {
                        words.join(", ")
                    });
                }
                row(
                    &format!("keys-row-{id}"),
                    if entry.name.is_empty() {
                        "Unnamed"
                    } else {
                        &entry.name
                    },
                    Some(&detail),
                    controls,
                    paints,
                )
            })
            .collect::<Vec<_>>(),
        Load::Failed(_) => vec![text(
            "keys-failed",
            "We couldn't load this list. Try again.",
            paints.muted,
        )],
        _ => vec![text("keys-loading", "Loading…", paints.muted)],
    };
    if rows.is_empty() {
        rows.push(text(
            "keys-empty",
            if agents {
                "You haven't claimed any agents."
            } else {
                "No keys yet."
            },
            paints.muted,
        ));
    }
    out.push(card("keys-list", rows, paints));
    if !state.busy {
        out.push(button("keys-refresh", "Refresh", act(Action::Refresh)));
    }
    if page == Page::ApiKeys
        && let Some(providers) = &state.providers
        && (state.paste_available || providers.iter().any(|p| p.saved))
    {
        let mut rows = vec![
            text("keys-own-title", "Your own provider keys", paints.text),
            text(
                "keys-own-help",
                if state.paste_available {
                    "Copy a provider key, then choose Paste key. Your provider bills you for requests made with it."
                } else {
                    "Your saved provider keys are listed below."
                },
                paints.muted,
            ),
        ];
        for provider in providers
            .iter()
            .filter(|p| p.saved || state.paste_available)
        {
            let id = &provider.id;
            let confirmation = format!("provider:{id}");
            let mut controls = Vec::new();
            if !state.busy {
                if provider.saved && state.confirm.as_ref() == Some(&confirmation) {
                    controls.push(button(
                        &format!("keys-own-confirm-{id}"),
                        "Confirm remove",
                        act(Action::ProviderRemove(id.clone())),
                    ));
                    controls.push(button(
                        &format!("keys-own-cancel-{id}"),
                        "Cancel",
                        act(Action::Confirm(None)),
                    ));
                } else {
                    if state.paste_available {
                        controls.push(button(
                            &format!("keys-own-paste-{id}"),
                            if provider.saved {
                                "Paste replacement"
                            } else {
                                "Paste key"
                            },
                            act(Action::ProviderPaste(id.clone())),
                        ));
                    }
                    if provider.saved {
                        controls.push(button(
                            &format!("keys-own-remove-{id}"),
                            "Remove",
                            act(Action::Confirm(Some(confirmation))),
                        ));
                    }
                }
            }
            rows.push(row(
                &format!("keys-own-row-{id}"),
                &provider.name,
                Some(if provider.saved { "Saved" } else { "Not added" }),
                controls,
                paints,
            ));
        }
        out.push(card("keys-own", rows, paints));
    }
    out
}
