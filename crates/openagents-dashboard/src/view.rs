//! Shared card and settings primitives for every dashboard surface.
use rust_native::{Axis, Element, Node, TextRole, View};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action(pub String);

fn display(value: String, limit: usize) -> String {
    let mut out = String::new();
    for c in value
        .chars()
        .filter(|c| !c.is_control() || matches!(c, '\n' | '\t'))
    {
        if out.len() + c.len_utf8() > limit.saturating_sub(3) {
            out.push('…');
            break;
        }
        out.push(c);
    }
    out
}
pub fn text(key: impl Into<String>, value: impl Into<String>, heading: bool) -> Node<Action> {
    Node {
        key: key.into(),
        style: Default::default(),
        element: Element::Text {
            value: display(value.into(), 2048),
            role: if heading {
                TextRole::Heading
            } else {
                TextRole::Body
            },
        },
    }
}
pub fn metric(key: impl Into<String>, value: impl Into<String>) -> Node<Action> {
    let mut node = text(key, value, true);
    node.style.text_size = Some(40);
    node.style.weight = Some(rust_native::style::TextWeight::Medium);
    node
}
pub fn button(
    key: impl Into<String>,
    label: impl Into<String>,
    action: impl Into<String>,
) -> Node<Action> {
    Node {
        key: key.into(),
        style: rust_native::style::Style {
            text_size: Some(15),
            min_height: Some(44),
            button_padding: Some([0, 8]),
            ..Default::default()
        },
        element: Element::Button {
            label: display(label.into(), 512),
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
    out.style.radius = Some(20);
    out.style.padding_points = Some([20; 4]);
    out.style.gap_points = Some(10);
    out
}
pub fn document(instance: String, revision: u64, children: Vec<Node<Action>>) -> View<Action> {
    View::new_v3(instance, revision, stack("home-root", children, false))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn long_and_malformed_source_titles_keep_the_dashboard_readable() {
        let mut cards = vec![];
        for widget in 0..32 {
            let mut rows = vec![text(
                format!("title-{widget}"),
                "A title\0with a bad character",
                true,
            )];
            for row in 0..12 {
                rows.push(button(
                    format!("row-{widget}-{row}"),
                    "🦀".repeat(10_000),
                    "open",
                ));
            }
            cards.push(card(format!("widget-{widget}"), rows));
        }
        let frame = document("test".into(), 1, cards).validate().unwrap();
        let json = serde_json::to_string(frame.view()).unwrap();
        assert!(!json.contains("\\u0000"));
        assert!(json.contains('…'));
    }
}
