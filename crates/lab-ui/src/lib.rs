//! Shared Lab card composition. Adapters draw media from registered resources;
//! application controllers authorize navigation and artifact reads.

use lab_kit::{Card, View as CardView};
use rust_native::{Axis, Element, Node, TextRole, ValidatedView, View, ViewError, style::Style};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case", deny_unknown_fields)]
pub enum Intent {
    Open {
        space: String,
        kind: String,
        document: String,
        version: u64,
    },
}

#[derive(Clone, Debug)]
pub struct Media {
    pub space: String,
    pub card: Card,
}

pub struct Projection {
    pub view: ValidatedView<Intent>,
    pub media: BTreeMap<String, Media>,
}

fn node(key: String, element: Element<Intent>) -> Node<Intent> {
    Node {
        key,
        style: Style::default(),
        element,
    }
}

/// Build a card for any surface. Its opaque media resources contain no URLs.
pub fn card(
    card: &Card,
    space: &str,
    instance: &str,
    revision: u64,
) -> Result<Projection, ViewError> {
    let identity = lab_kit::digest(&serde_json::json!([
        space,
        card.kind,
        card.document,
        card.version
    ]));
    let prefix = format!("lab-{}", &identity[..24]);
    let text = |suffix: &str, value: String, role| {
        node(format!("{prefix}-{suffix}"), Element::Text { value, role })
    };
    let state = card.state();
    let mut children = vec![text(
        "title",
        format!("{} · v{}", card.title, card.version),
        TextRole::Heading,
    )];
    if !card.summary.is_empty() {
        children.push(text("summary", card.summary.clone(), TextRole::Body));
    }
    let mut media = BTreeMap::new();
    if let Some(refusal) = state.refusal {
        children.push(text("refusal", refusal, TextRole::Status));
    } else if matches!(
        state.view,
        CardView::Video
            | CardView::Image
            | CardView::Compare
            | CardView::Model3d
            | CardView::Playable
    ) && (card.primary.is_some()
        || card.poster.is_some()
        || card
            .compare
            .as_ref()
            .is_some_and(|c| c.a_primary.is_some() || c.b_primary.is_some()))
    {
        let resource = format!("{prefix}-media");
        media.insert(
            resource.clone(),
            Media {
                space: space.into(),
                card: card.clone(),
            },
        );
        children.push(node(
            resource.clone(),
            Element::Surface {
                resource,
                label: card.title.clone(),
            },
        ));
    }
    if let Some(status) = state.status {
        children.push(text("status", status, TextRole::Status));
    }
    if card.private_local {
        children.push(text(
            "private",
            "Only on your computer".into(),
            TextRole::Status,
        ));
    }
    for (at, note) in card.notes.iter().take(100).enumerate() {
        let value = match note_at(&note.locator) {
            Some(place) => format!("{place} · {}", note.text),
            None => note.text.clone(),
        };
        children.push(text(&format!("note-{at}"), value, TextRole::Body));
    }
    if card.page.is_some() {
        children.push(node(
            format!("{prefix}-open"),
            Element::Button {
                label: "Open document".into(),
                enabled: true,
                icon: None,
                shortcut: None,
                intent: Intent::Open {
                    space: space.into(),
                    kind: card.kind.clone(),
                    document: card.document.clone(),
                    version: card.version,
                },
            },
        ));
    }
    let root = node(
        prefix,
        Element::Stack {
            axis: Axis::Vertical,
            children,
        },
    );
    Ok(Projection {
        view: View::new(instance, revision, root).validate()?,
        media,
    })
}

/// Where a note is anchored, in words: "1:26" for a time.
pub fn note_at(locator: &lab_kit::Locator) -> Option<String> {
    match locator {
        lab_kit::Locator::Time { seconds } if seconds.is_finite() && *seconds >= 0.0 => {
            Some(clock(*seconds))
        }
        lab_kit::Locator::Frame { frame } => Some(format!("Frame {frame}")),
        lab_kit::Locator::Step { step } => Some(format!("Step {step}")),
        _ => None,
    }
}

/// A playing time as `m:ss`, or `h:mm:ss` past an hour.
pub fn clock(seconds: f64) -> String {
    let total = if seconds.is_finite() {
        seconds.max(0.0).floor() as u64
    } else {
        0
    };
    let (h, m, s) = (total / 3600, total / 60 % 60, total % 60);
    if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m}:{s:02}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rust_native::Activation;
    use serde_json::json;

    #[test]
    fn notes_at_a_time_show_the_time() {
        assert_eq!(clock(86.4), "1:26");
        assert_eq!(clock(3723.0), "1:02:03");
        assert_eq!(clock(f64::NAN), "0:00");
        let value: Card = serde_json::from_value(json!({"kind":"video","document":"film","version":2,
            "title":"Film","view":"video","notes":[{"locator":{"at":"time","seconds":86.0},"author":"a","text":"Too bright"},
            {"locator":{"at":"none"},"author":"a","text":"Good cut"}]})).unwrap();
        let drawn = card(&value, "me", "test", 1).unwrap();
        let json = String::from_utf8(drawn.view.to_json().unwrap()).unwrap();
        assert!(json.contains("1:26 · Too bright"), "{json}");
        assert!(json.contains("\"Good cut\""), "{json}");
    }

    #[test]
    fn unknown_views_and_private_cards_share_the_same_native_controls() {
        let mut value: Card = serde_json::from_value(json!({"kind":"future","document":"example","version":3,
            "title":"Example","view":"new_view","primary":"file","private_local":true,"page":"/example"})).unwrap();
        let report = card(&value, "project:shared", "test", 7).unwrap();
        assert!(report.media.is_empty());
        let Element::Stack { children, .. } = &report.view.view().root.element else {
            panic!()
        };
        let private = children
            .iter()
            .find(|n| n.key.ends_with("-private"))
            .unwrap();
        assert!(
            matches!(&private.element, Element::Text { value, .. } if value == "Only on your computer")
        );
        let open = children.last().unwrap();
        let activation = Activation {
            instance: "test".into(),
            revision: 7,
            node: open.key.clone(),
        };
        assert_eq!(
            report.view.activate(&activation).unwrap(),
            &Intent::Open {
                space: "project:shared".into(),
                kind: "future".into(),
                document: "example".into(),
                version: 3,
            }
        );
        value.view = CardView::Model3d;
        let model = card(&value, "me", "test", 8).unwrap();
        assert_eq!(model.media.len(), 1);
        assert!(model.view.activate(&activation).is_err());
        assert!(
            !String::from_utf8(model.view.to_json().unwrap())
                .unwrap()
                .contains("/v1/")
        );
    }
}
