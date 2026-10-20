//! A Lab document (#12333): the same Rust Native card the chat and the site
//! draw (`lab_ui::card`), as a page. The card's player (a video, an image,
//! a 3D model, or a playable) is a surface the host mounts over the page
//! in its own frame, so this view leaves it out and says where it goes
//! ([`State::player`]).

use lab_kit::{Card, View};
use rust_native::{Element, Node};
use serde::{Deserialize, Serialize};

use super::{Intent, Load, Paints, button, card, column, text};

/// The open document.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct State {
    pub space: String,
    pub card: Load<Card>,
}

impl State {
    /// Whether the card has a player for the host to mount.
    pub fn player(&self) -> bool {
        self.card.ready().is_some_and(|card| {
            lab_ui::card(card, &self.space, "lab-page", 1)
                .is_ok_and(|projection| !projection.media.is_empty())
        })
    }
}

/// What a control on the page asks for.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// Another document or version.
    Open {
        space: String,
        kind: String,
        document: String,
        version: u64,
    },
    /// Download a report's file from this address.
    Download(String),
    /// Leave for a page of the site, such as the video editor.
    Visit(String),
}

fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                char::from(b).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

/// A report's file, by the site's artifact address.
pub fn artifact(space: &str, id: &str) -> String {
    format!(
        "/v1/labs/artifacts/{}/content?space={}",
        encode(id),
        encode(space)
    )
}

/// `lab_ui`'s card, with its intents as this page's and its player left
/// to the host.
fn shared(node: &Node<lab_ui::Intent>) -> Option<Node<Intent>> {
    let element = match &node.element {
        Element::Stack { axis, children } => Element::Stack {
            axis: *axis,
            children: children.iter().filter_map(shared).collect(),
        },
        Element::Text { value, role } => Element::Text {
            value: value.clone(),
            role: *role,
        },
        Element::Button {
            label,
            enabled,
            icon,
            shortcut,
            intent:
                lab_ui::Intent::Open {
                    space,
                    kind,
                    document,
                    version,
                },
        } => Element::Button {
            label: label.clone(),
            enabled: *enabled,
            icon: icon.clone(),
            shortcut: shortcut.clone(),
            intent: Intent::Lab(Action::Open {
                space: space.clone(),
                kind: kind.clone(),
                document: document.clone(),
                version: *version,
            }),
        },
        _ => return None,
    };
    Some(Node {
        key: node.key.clone(),
        style: node.style.clone(),
        element,
    })
}

/// The view.
pub fn view(state: &State, paints: &Paints) -> Node<Intent> {
    let body = match &state.card {
        Load::Ready(found) => match lab_ui::card(found, &state.space, "lab-page", 1) {
            Ok(projection) => {
                let mut rows: Vec<Node<Intent>> =
                    shared(&projection.view.view().root).into_iter().collect();
                if let (View::Report, Some(primary)) = (&found.view, found.primary.as_deref()) {
                    rows.push(button(
                        "lab-download",
                        if found.detail.get("replay").is_some() {
                            "Download the replay"
                        } else {
                            "Download the result"
                        },
                        Intent::Lab(Action::Download(artifact(&state.space, primary))),
                    ));
                }
                // The video editor stays the site's own page for now.
                if found.kind == "video" {
                    rows.push(button(
                        "lab-edit-video",
                        "Edit the video",
                        Intent::Lab(Action::Visit(format!(
                            "/labs/video/{}?ui=html&v={}&space={}",
                            encode(&found.document),
                            found.version,
                            encode(&state.space)
                        ))),
                    ));
                }
                card("lab-card", rows, paints)
            }
            Err(_) => card(
                "lab-card",
                vec![text(
                    "lab-line",
                    "This document can't be displayed.",
                    paints.muted,
                )],
                paints,
            ),
        },
        Load::Failed(error) => card(
            "lab-card",
            vec![text("lab-line", error, paints.muted)],
            paints,
        ),
        _ => card(
            "lab-card",
            vec![text("lab-line", "Loading…", paints.muted)],
            paints,
        ),
    };
    column("lab", vec![body])
}
