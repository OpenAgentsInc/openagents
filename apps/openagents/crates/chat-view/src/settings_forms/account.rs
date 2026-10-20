//! Settings → Your ID (#12290): the account's ID and the keys that sign in
//! to it, sign out, and delete the account with the paid-credit question,
//! as the desktop's account menu has them (`openagents-ui`'s
//! `openagents_account`). The public profile's name and picture are set on
//! the profile form (U1, #12254) and the recovery words stay on the
//! person's own devices (U5, #12258); this page links to the first and
//! names the second.

use rust_native::Node;
use serde::{Deserialize, Serialize};

use super::{Intent, Load, Paints, button, card, danger_button, header, row, section, text};

/// What the warning says before the account is deleted
/// (`openagents_account::DELETE_WARNING`).
pub const DELETE_WARNING: &str = "This deletes your chats, projects, files, connections, computers, and sign-ins. It can't be undone. Your wallet isn't deleted: its money stays with your 12 words, so keep them safe.";
/// What it asks when paid credit is left
/// (`openagents_account::PAID_CREDIT_WARNING`).
pub const PAID_CREDIT_WARNING: &str =
    "You still have paid credit. Deleting your account gives it up.";
/// When the person owns a team with other members
/// (`openagents_account::SHARED_TEAM_NOTICE`).
pub const SHARED_TEAM_NOTICE: &str = "You own a team with other members. Make one of them its owner on the website, then delete your account.";
/// The confirmation `DELETE /v1/account` takes.
pub const DELETE_CONFIRM: &str = "delete my account";

/// A key that signs in to the account.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Key {
    pub pubkey: String,
    #[serde(default)]
    pub npub: String,
}

/// Where deleting the account is.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Deleting {
    /// The warning, before anything is sent.
    Ask,
    /// Paid credit is left: give it up?
    PaidCredit,
    Working,
    /// It didn't happen; the sentence says why.
    Notice(String),
}

/// What the page shows.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    /// This browser is signed in; signed out, the page offers sign-in only.
    pub signed_in: bool,
    /// The account service names this person as a site admin.
    pub admin: bool,
    /// The account's name, such as its email or GitHub login.
    pub label: Option<String>,
    /// The claimed `name@openagents.com` name, without the domain.
    pub name: Option<String>,
    pub keys: Load<Vec<Key>>,
    /// The key waiting for "Remove" to be confirmed, and one being removed.
    pub confirm_remove: Option<String>,
    pub removing: Option<String>,
    pub deleting: Option<Deleting>,
    pub notice: Option<String>,
}

impl State {
    /// The ID people find you by: the claimed name, or the first key.
    pub fn id(&self) -> Option<String> {
        if let Some(name) = &self.name {
            return Some(format!("{name}@openagents.com"));
        }
        self.keys.ready()?.first().map(|k| k.npub.clone())
    }

    /// Your public profile's address on the site (`/@name` or `/@npub`).
    pub fn profile_path(&self) -> Option<String> {
        if let Some(name) = &self.name {
            return Some(format!("/@{name}"));
        }
        self.keys
            .ready()?
            .first()
            .filter(|k| !k.npub.is_empty())
            .map(|k| format!("/@{}", k.npub))
    }
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Copy(String),
    /// Ask before removing a key; `None` cancels.
    ConfirmRemove(Option<String>),
    RemoveKey(String),
    /// A page of the site: the profile form or the profile.
    Open(String),
    SignOut,
    /// Sign in, then come back.
    SignIn,
    /// Show the warning; `false` puts it away.
    AskDelete(bool),
    /// Delete, giving up paid credit when `true`.
    Delete(bool),
}

/// The site's profile form (U1).
pub const PROFILE_FORM: &str = "/settings/profile";

fn act(action: Action) -> Intent {
    Intent::Account(action)
}

/// A key shortened for a row: its start and end.
pub fn short(npub: &str) -> String {
    if npub.chars().count() <= 20 {
        return npub.to_owned();
    }
    let start: String = npub.chars().take(12).collect();
    let end: String = npub
        .chars()
        .rev()
        .take(6)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect();
    format!("{start}…{end}")
}

pub(super) fn page(state: &State, paints: &Paints) -> Vec<Node<Intent>> {
    let mut out = vec![header(
        "account-title",
        "Your ID",
        state.label.as_deref(),
        paints,
    )];
    if !state.signed_in {
        out.push(card(
            "account-signed-out",
            vec![row(
                "account-sign-in",
                "Sign in",
                Some("Sign in to see your ID and the keys that sign in to your account."),
                vec![button(
                    "account-sign-in-button",
                    "Sign in",
                    act(Action::SignIn),
                )],
                paints,
            )],
            paints,
        ));
        return out;
    }
    if state.admin {
        out.push(section(
            "account-admin",
            "Admin",
            vec![button(
                "account-admin-open",
                "Open admin",
                act(Action::Open("/admin/analytics?ui=gpui".into())),
            )],
            paints,
        ));
    }
    let id = state.id();
    let mut id_controls = Vec::new();
    if let Some(id) = &id {
        id_controls.push(button(
            "account-id-copy",
            "Copy",
            act(Action::Copy(id.clone())),
        ));
    }
    if let Some(path) = state.profile_path() {
        id_controls.push(button(
            "account-id-profile",
            "View profile",
            act(Action::Open(path)),
        ));
    }
    id_controls.push(button(
        "account-id-edit",
        "Public profile",
        act(Action::Open(PROFILE_FORM.into())),
    ));
    let id_line = match (&id, &state.keys) {
        (Some(id), _) => id.clone(),
        (None, Load::Failed(error)) => error.clone(),
        (None, Load::Ready(_)) => "No key signs in to this account yet.".into(),
        (None, _) => "Loading…".into(),
    };
    out.push(card(
        "account-id",
        vec![
            row(
                "account-id-row",
                "Your ID",
                Some(&id_line),
                id_controls,
                paints,
            ),
            row(
                "account-words",
                "Recovery words",
                Some("Your 12 words stay in the OpenAgents app on your phone and computers."),
                Vec::new(),
                paints,
            ),
        ],
        paints,
    ));

    let rows: Vec<Node<Intent>> = match &state.keys {
        Load::Ready(keys) if keys.is_empty() => vec![text(
            "account-keys-none",
            "No keys yet. Sign in from the OpenAgents app to add one.",
            paints.muted,
        )],
        Load::Ready(keys) => keys
            .iter()
            .enumerate()
            .map(|(ix, key)| {
                let shown = if key.npub.is_empty() {
                    &key.pubkey
                } else {
                    &key.npub
                };
                let mut controls = vec![button(
                    &format!("account-key-{ix}-copy"),
                    "Copy",
                    act(Action::Copy(shown.clone())),
                )];
                if state.removing.as_deref() == Some(key.pubkey.as_str()) {
                    controls = vec![text(
                        &format!("account-key-{ix}-removing"),
                        "Removing…",
                        paints.muted,
                    )];
                } else if state.confirm_remove.as_deref() == Some(key.pubkey.as_str()) {
                    controls.push(danger_button(
                        &format!("account-key-{ix}-remove-yes"),
                        "Remove key",
                        act(Action::RemoveKey(key.pubkey.clone())),
                        paints,
                    ));
                    controls.push(button(
                        &format!("account-key-{ix}-remove-no"),
                        "Keep",
                        act(Action::ConfirmRemove(None)),
                    ));
                } else if keys.len() > 1 {
                    // The last key can't go: nothing would sign in.
                    controls.push(button(
                        &format!("account-key-{ix}-remove"),
                        "Remove",
                        act(Action::ConfirmRemove(Some(key.pubkey.clone()))),
                    ));
                }
                row(
                    &format!("account-key-{ix}"),
                    &short(shown),
                    (state.confirm_remove.as_deref() == Some(key.pubkey.as_str())).then_some(
                        "That device or app can't sign in with this key after it's removed.",
                    ),
                    controls,
                    paints,
                )
            })
            .collect(),
        Load::Failed(error) => vec![text("account-keys-error", error, paints.muted)],
        Load::Idle | Load::Loading => vec![text("account-keys-loading", "Loading…", paints.muted)],
    };
    out.push(section("account-keys", "Keys that sign in", rows, paints));
    if let Some(notice) = &state.notice {
        out.push(text("account-notice", notice, paints.danger));
    }

    let mut danger = vec![row(
        "account-sign-out",
        "Sign out",
        Some("Sign this browser out of OpenAgents."),
        vec![button(
            "account-sign-out-button",
            "Sign out",
            act(Action::SignOut),
        )],
        paints,
    )];
    danger.push(match &state.deleting {
        None => row(
            "account-delete",
            "Delete account",
            None,
            vec![danger_button(
                "account-delete-ask",
                "Delete account…",
                act(Action::AskDelete(true)),
                paints,
            )],
            paints,
        ),
        Some(step @ (Deleting::Ask | Deleting::PaidCredit)) => {
            let (warning, label, forfeit) = if *step == Deleting::Ask {
                (DELETE_WARNING, "Delete my account", false)
            } else {
                (PAID_CREDIT_WARNING, "Give up the credit and delete", true)
            };
            row(
                "account-delete",
                "Delete account",
                Some(warning),
                vec![
                    danger_button(
                        "account-delete-confirm",
                        label,
                        act(Action::Delete(forfeit)),
                        paints,
                    ),
                    button(
                        "account-delete-cancel",
                        "Keep my account",
                        act(Action::AskDelete(false)),
                    ),
                ],
                paints,
            )
        }
        Some(Deleting::Working) => row(
            "account-delete",
            "Delete account",
            Some("Deleting your account…"),
            Vec::new(),
            paints,
        ),
        Some(Deleting::Notice(message)) => row(
            "account-delete",
            "Delete account",
            Some(message),
            vec![button(
                "account-delete-close",
                "OK",
                act(Action::AskDelete(false)),
            )],
            paints,
        ),
    });
    out.push(section("account-session", "This browser", danger, paints));
    out
}

/// What a `DELETE /v1/account` refusal means for the page: its
/// `error.code` and `error.message`.
pub fn refused(code: &str, message: &str) -> Deleting {
    match code {
        "balance_remaining" => Deleting::PaidCredit,
        "shared_workspace" => Deleting::Notice(SHARED_TEAM_NOTICE.into()),
        _ if !message.is_empty() => Deleting::Notice(message.into()),
        _ => Deleting::Notice("We couldn't delete your account. Try again.".into()),
    }
}
