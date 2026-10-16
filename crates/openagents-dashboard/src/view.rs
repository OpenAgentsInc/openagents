//! Shared card and settings primitives for every dashboard surface.
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action(pub String);
pub fn text(key: impl Into<String>, value: impl Into<String>, heading: bool) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Text {
            value: value.into(),
            role: if heading {
                TextRole::Heading
            } else {
                TextRole::Body
            },
        },
    }
}
pub fn button(
    key: impl Into<String>,
    label: impl Into<String>,
    action: impl Into<String>,
) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Button {
            label: label.into(),
            enabled: true,
            icon: None,
            shortcut: None,
            intent: Action(action.into()),
        },
    }
}
pub fn field(
    key: impl Into<String>,
    label: impl Into<String>,
    value: impl Into<String>,
    multiline: bool,
) -> Node<Action> {
    let key = key.into();
    Node {
        key: key.clone(),
        style: Default::default(),
        element: Element::Field {
            label: label.into(),
            value: value.into(),
            placeholder: String::new(),
            secret: false,
            multiline,
            enabled: true,
            max_bytes: if multiline || key == "repositories" {
                4096
            } else if key == "labels" {
                2048
            } else {
                256
            },
            on_change: Action(key),
        },
    }
}
pub fn stack(
    key: impl Into<String>,
    children: Vec<Node<Action>>,
    horizontal: bool,
) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Stack {
            axis: if horizontal {
                Axis::Wrap
            } else {
                Axis::Vertical
            },
            children,
        },
    }
}
pub fn card(key: impl Into<String>, children: Vec<Node<Action>>) -> Node<Action> {
    let mut out = stack(key, children, false);
    out.style.radius = Some(16);
    out.style.padding_points = Some([16; 4]);
    out.style.gap_points = Some(10);
    out
}
pub fn document(instance: String, revision: u64, children: Vec<Node<Action>>) -> View<Action> {
    View::new_v3(instance, revision, stack("home-root", children, false))
}
