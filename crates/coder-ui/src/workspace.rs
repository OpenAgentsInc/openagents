//! Account and workspace presentation over Rust Native. Values are supplied
//! by an authenticated application; these views grant no domain authority.

use rust_native::style::{Color, Space, Style};
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum WorkspaceIntent {
    Credential,
    Refresh,
    SignOut,
}

/// The application supplies its palette rather than changing the generic
/// framework or the source-equivalent Coder terminal profile.
#[derive(Clone, Copy)]
pub struct Palette {
    pub text: Color,
    pub heading: Color,
    pub secondary: Color,
    pub border: Color,
}

pub struct Session<'a> {
    pub account: &'a str,
    pub account_id: &'a str,
    pub workspace: Option<&'a str>,
    pub workspace_id: Option<&'a str>,
    pub role: Option<&'a str>,
    pub members_epoch: Option<u64>,
    pub expires_at: u64,
}

fn text(
    key: &str,
    value: impl Into<String>,
    role: TextRole,
    color: Color,
) -> Node<WorkspaceIntent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(color),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

/// A secret control starts empty and keeps its edited value in the platform
/// control until the application submits it to the selected account service.
#[must_use]
pub fn sign_in(palette: Palette) -> View<WorkspaceIntent> {
    View::new_v3(
        "cloud-sign-in",
        1,
        Node {
            key: "credential".into(),
            style: Style {
                foreground: Some(palette.text),
                border: Some(palette.border),
                ..Style::default()
            },
            element: Element::Field {
                label: "Account API key".into(),
                value: String::new(),
                placeholder: "Your existing account key".into(),
                secret: true,
                multiline: false,
                enabled: true,
                max_bytes: 512,
                on_change: WorkspaceIntent::Credential,
            },
        },
    )
}

#[must_use]
pub fn session(session: &Session<'_>, palette: Palette) -> View<WorkspaceIntent> {
    let mut children = vec![
        text(
            "account-label",
            session.account,
            TextRole::Heading,
            palette.heading,
        ),
        text(
            "account-id",
            format!("Account: {}", session.account_id),
            TextRole::Body,
            palette.secondary,
        ),
    ];
    if let Some(workspace) = session.workspace {
        children.push(text(
            "workspace-label",
            workspace,
            TextRole::Heading,
            palette.heading,
        ));
        children.push(text(
            "workspace-standing",
            format!(
                "Workspace: {} · Role: {}",
                session.workspace_id.unwrap_or("Unavailable"),
                session.role.unwrap_or("Unavailable"),
            ),
            TextRole::Body,
            palette.text,
        ));
    } else {
        children.push(text(
            "workspace-empty",
            "Choose a workspace to see its connections.",
            TextRole::Status,
            palette.text,
        ));
    }
    children.push(text(
        "session-expiry",
        format!("Signed in until {} (Unix time).", session.expires_at),
        TextRole::Body,
        palette.secondary,
    ));
    View::new_v3(
        "cloud-session",
        1,
        Node {
            key: "session-summary".into(),
            style: Style {
                gap: Some(Space::Sm),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        },
    )
}

#[must_use]
pub fn connection(key: &str, title: &str, reason: &str, palette: Palette) -> View<WorkspaceIntent> {
    connection_state(key, title, "Unavailable", reason, palette)
}

/// Display only a connection state established by the application adapter.
pub fn connection_state(
    key: &str,
    title: &str,
    state: &str,
    reason: &str,
    palette: Palette,
) -> View<WorkspaceIntent> {
    View::new_v3(
        format!("cloud-{key}"),
        1,
        Node {
            key: key.into(),
            style: Style {
                gap: Some(Space::Xs),
                ..Style::default()
            },
            element: Element::Stack {
                axis: Axis::Vertical,
                children: vec![
                    text(
                        &format!("{key}-title"),
                        format!("{title} · {state}"),
                        TextRole::Heading,
                        palette.heading,
                    ),
                    text(
                        &format!("{key}-reason"),
                        reason,
                        TextRole::Status,
                        palette.secondary,
                    ),
                ],
            },
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_secret_control_never_contains_a_retained_key() {
        let color = Color::rgb(255, 255, 255);
        let view = sign_in(Palette {
            text: color,
            heading: color,
            secondary: color,
            border: color,
        })
        .validate()
        .unwrap();
        assert!(
            matches!(&view.view().root.element, Element::Field {secret:true, value, ..} if value.is_empty())
        );
    }
}
