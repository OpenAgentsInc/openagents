//! The Tailnet surface: the devices on the user's tailnet, or what to do
//! when there are none.

use crate::tailnet::Tailnet;
use rust_native::style::{Color, Space, Style, TextWeight};
use rust_native::{Axis, Element, Node, TextRole};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Intent {
    SignIn,
    Refresh,
}

/// What tailnet admission found on one device.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Admit {
    Checking,
    /// Added under Computers, with its chats when it serves them.
    Connected,
    /// No OpenAgents host answered.
    NotRunning,
    /// The host refused, with its code.
    Refused(String),
}

pub enum Screen {
    Loading,
    SignIn(Url),
    Devices(Tailnet),
    Failed(String),
}

// The theme seam's inks (`openagents_chat_app::visual::inks`): white,
// gray, and the system green on the dark look, Coder Light's roles on the
// light one (#11028).
fn white() -> Color {
    openagents_chat_app::visual::inks().text
}

fn gray() -> Color {
    openagents_chat_app::visual::inks().quiet
}

fn green() -> Color {
    match openagents_chat_app::visual::scheme() {
        openagents_chat_app::visual::Scheme::Dark => Color::rgb(52, 199, 89),
        openagents_chat_app::visual::Scheme::Light => openagents_chat_app::visual::inks().done,
    }
}

pub fn root(screen: &Screen, admits: &BTreeMap<String, Admit>) -> Node<Intent> {
    match screen {
        Screen::Loading => page(vec![
            heading("tailnet-title", "Tailnet"),
            status("loading", "Checking your tailnet…", gray()),
        ]),
        Screen::SignIn(_) => page(vec![
            heading("tailnet-title", "Connect to a tailnet"),
            body("Sign in with Tailscale to see the devices on your tailnet."),
            button("sign-in", "Sign in with Tailscale", Intent::SignIn),
        ]),
        Screen::Failed(error) => page(vec![
            heading("tailnet-title", "Tailnet"),
            body(error),
            button("refresh", "Try again", Intent::Refresh),
        ]),
        Screen::Devices(tailnet) if tailnet.devices.is_empty() => page(vec![
            heading("tailnet-title", "Connect to a tailnet"),
            body(
                "No other devices are on this tailnet. Connect a device with Tailscale, then refresh.",
            ),
            button("refresh", "Refresh", Intent::Refresh),
        ]),
        Screen::Devices(tailnet) => devices(tailnet, admits),
    }
}

fn devices(tailnet: &Tailnet, admits: &BTreeMap<String, Admit>) -> Node<Intent> {
    let count = match tailnet.devices.len() {
        1 => "1 device".to_string(),
        n => format!("{n} devices"),
    };
    let summary = match &tailnet.name {
        Some(name) => format!("{count} on {name}"),
        None => count,
    };
    let rows = tailnet
        .devices
        .iter()
        .enumerate()
        .map(|(index, device)| {
            let (state, color) = match device.online {
                Some(true) => ("Online", green()),
                Some(false) => ("Offline", gray()),
                None => ("Status unknown", gray()),
            };
            let mut lines = vec![
                text(
                    &format!("device-{index}-name"),
                    &device.name,
                    TextRole::Body,
                    white(),
                    true,
                ),
                status(
                    &format!("device-{index}-detail"),
                    &format!("{} · {}", device.os, device.address),
                    gray(),
                ),
                status(&format!("device-{index}-state"), state, color),
            ];
            let admit = match admits.get(&device.address) {
                Some(Admit::Checking) => Some(("Looking for OpenAgents…", gray())),
                Some(Admit::Connected) => Some(("OpenAgents connected", green())),
                Some(Admit::Refused(code)) if code == "not_owner" => {
                    Some(("Another Tailscale user's device", gray()))
                }
                Some(Admit::Refused(_)) => Some(("OpenAgents refused this phone", gray())),
                Some(Admit::NotRunning) | None => None,
            };
            if let Some((line, color)) = admit {
                lines.push(status(&format!("device-{index}-openagents"), line, color));
            }
            stack(&format!("device-{index}"), Space::Xs, lines)
        })
        .collect();
    let mut children = vec![
        heading("tailnet-title", "Tailnet"),
        status("devices-summary", &summary, gray()),
        Node {
            key: "devices".into(),
            style: Style::default(),
            element: Element::List {
                label: "Tailnet devices".into(),
                children: rows,
            },
        },
        status(
            "tailnet-hint",
            "Computers running `coder host serve --tailnet-admission standard` connect automatically.",
            gray(),
        ),
    ];
    if let Some(this) = &tailnet.this_device {
        children.push(status(
            "this-device",
            &format!("This app appears on the tailnet as {this}."),
            gray(),
        ));
    }
    children.push(button("refresh", "Refresh", Intent::Refresh));
    page(children)
}

fn page(children: Vec<Node<Intent>>) -> Node<Intent> {
    let mut node = stack("tailnet", Space::Md, children);
    node.style.padding_top = Some(Space::Lg);
    node.style.padding_end = Some(Space::Md);
    node.style.padding_bottom = Some(Space::Md);
    node.style.padding_start = Some(Space::Md);
    node
}

fn stack(key: &str, gap: Space, children: Vec<Node<Intent>>) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            gap: Some(gap),
            ..Style::default()
        },
        element: Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    }
}

fn text(key: &str, value: &str, role: TextRole, foreground: Color, bold: bool) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(foreground),
            weight: bold.then_some(TextWeight::Bold),
            ..Style::default()
        },
        element: Element::Text {
            value: value.into(),
            role,
        },
    }
}

fn heading(key: &str, value: &str) -> Node<Intent> {
    text(key, value, TextRole::Heading, white(), true)
}

fn body(value: &str) -> Node<Intent> {
    text("message", value, TextRole::Body, white(), false)
}

fn status(key: &str, value: &str, color: Color) -> Node<Intent> {
    text(key, value, TextRole::Status, color, false)
}

fn button(key: &str, label: &str, intent: Intent) -> Node<Intent> {
    Node {
        key: key.into(),
        style: Style {
            foreground: Some(white()),
            ..Style::default()
        },
        element: Element::Button {
            shortcut: None,
            label: label.into(),
            enabled: true,
            icon: None,
            intent,
        },
    }
}
