//! Shared home choices. Controllers supply only computers the caller can use.
use crate::{Home, Links, PartRef};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View, ViewError, style::Style};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Destination {
    pub home: Home,
    pub label: String,
    pub available: bool,
}

fn node(key: &str, element: Element<Home>) -> Node<Home> {
    Node {
        key: key.into(),
        style: Style::default(),
        element,
    }
}

pub fn parts(
    instance: &str,
    revision: u64,
    links: &Links,
) -> Result<ValidatedView<PartRef>, ViewError> {
    let mut children = Vec::new();
    for (key, label, part) in [
        (
            "earlier-part",
            "Open the earlier part",
            links.continues.as_ref(),
        ),
        (
            "latest-part",
            match links.continued_by.as_ref().map(|p| &p.home) {
                Some(Home::Cloud) => "Next part on OpenAgents",
                _ => "Next part on a computer",
            },
            links.continued_by.as_ref(),
        ),
    ] {
        if let Some(part) = part.filter(|p| p.valid()) {
            children.push(Node {
                key: key.into(),
                style: Style::default(),
                element: Element::Button {
                    label: label.into(),
                    enabled: true,
                    icon: None,
                    shortcut: None,
                    intent: part.clone(),
                },
            });
        }
    }
    View::new(
        instance,
        revision,
        Node {
            key: "chat-parts".into(),
            style: Style::default(),
            element: Element::Stack {
                axis: Axis::Horizontal,
                children,
            },
        },
    )
    .validate()
}

pub fn choices(
    instance: &str,
    revision: u64,
    current: &Destination,
    destinations: &[Destination],
    blocked: Option<&str>,
) -> Result<ValidatedView<Home>, ViewError> {
    let mut children = vec![node(
        "home-title",
        Element::Text {
            value: format!("Runs on {}", current.label),
            role: TextRole::Heading,
        },
    )];
    if let Some(reason) = blocked {
        children.push(node(
            "home-status",
            Element::Text {
                value: reason.into(),
                role: TextRole::Status,
            },
        ));
    }
    let mut seen = vec![current.home.clone()];
    for destination in destinations.iter().take(32) {
        if seen.contains(&destination.home) {
            continue;
        }
        if let Home::Computer(device) = &destination.home {
            if Home::computer(device).is_none() {
                continue;
            }
        }
        seen.push(destination.home.clone());
        children.push(node(
            &format!("home-choice-{}", children.len()),
            Element::Button {
                label: destination.label.chars().take(64).collect(),
                enabled: blocked.is_none() && destination.available,
                icon: None,
                shortcut: None,
                intent: destination.home.clone(),
            },
        ));
    }
    View::new(
        instance,
        revision,
        node(
            "home-choices",
            Element::Stack {
                axis: Axis::Vertical,
                children,
            },
        ),
    )
    .validate()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;

    #[test]
    fn a_busy_chat_cannot_move_and_old_activations_cannot_choose_a_home() {
        let current = Destination {
            home: Home::Cloud,
            label: "OpenAgents".into(),
            available: true,
        };
        let target = Destination {
            home: Home::Computer("fixture-mac".into()),
            label: "This Mac".into(),
            available: true,
        };
        let activate = Activation {
            instance: "fixture".into(),
            revision: 1,
            node: "home-choice-1".into(),
        };
        let ready = choices(
            "fixture",
            1,
            &current,
            &[target.clone(), target.clone()],
            None,
        )
        .unwrap();
        assert_eq!(ready.activate(&activate).unwrap(), &target.home);
        assert!(
            choices("fixture", 2, &current, &[target.clone()], None)
                .unwrap()
                .activate(&activate)
                .is_err()
        );
        let busy = choices(
            "fixture",
            1,
            &current,
            &[target],
            Some("Stop the current answer first."),
        )
        .unwrap();
        let activate = Activation {
            node: "home-choice-2".into(),
            ..activate
        };
        assert!(busy.activate(&activate).is_err());
    }
}
