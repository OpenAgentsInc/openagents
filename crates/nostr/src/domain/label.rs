//! NIP-32 labels.
//!
//! Kind `1985` attaches `l` labels to one or more `e`, `p`, `a`, `r`, or
//! `t` targets. When an `L` namespace is present, every `l` mark must name
//! one of those namespaces. With no `L` tag and no mark, the namespace is
//! `ugc`. A namespace that starts with `#` associates the label value with
//! that tag on the targets.
//!
//! On any other kind, `l` and `L` describe the event itself. Kind `1985`
//! is a regular event: a correction is a new event, not a replacement head.
//! The relay does not rewrite the labeled target.

use std::str::FromStr;

use super::hex::decode_lower_hex;
use super::{DomainError, Event, ReplacementAddress, Tag};

const LABEL_KIND: u16 = 1_985;
const IMPLIED_NAMESPACE: &str = "ugc";

/// One label and the namespace it belongs to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    pub value: String,
    pub namespace: String,
    /// Present when the namespace is `#<tag>`. The value is that tag's value.
    pub associates_tag: Option<String>,
}

/// An object named by a kind `1985` label.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LabelTarget {
    Event {
        id: String,
        relay: Option<String>,
    },
    Pubkey {
        pubkey: String,
        relay: Option<String>,
    },
    Address {
        address: ReplacementAddress,
        relay: Option<String>,
    },
    Relay(String),
    Topic(String),
}

/// Labels read from an event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Labeling {
    pub content: String,
    pub labels: Vec<Label>,
    pub targets: Vec<LabelTarget>,
    /// True when the labels describe this event rather than `targets`.
    pub self_labeled: bool,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn is_relay(value: &str) -> bool {
    (value.starts_with("ws://") || value.starts_with("wss://"))
        && value.len() <= 2_048
        && !value.chars().any(char::is_whitespace)
}

fn relay_hint(tag: &Tag) -> Result<Option<String>, DomainError> {
    match tag.as_slice().get(2) {
        None => Ok(None),
        Some(value) if value.is_empty() => Err(invalid("a label relay hint is empty")),
        Some(value) if is_relay(value) => Ok(Some(value.to_owned())),
        Some(_) => Err(invalid("a label relay hint must be ws:// or wss://")),
    }
}

fn mark(tag: &Tag) -> Result<Option<&str>, DomainError> {
    match tag.as_slice().get(2) {
        None => Ok(None),
        Some(value) if value.is_empty() => Err(invalid("a label mark is empty")),
        Some(value) => Ok(Some(value.as_str())),
    }
}

fn association(namespace: &str) -> Result<Option<String>, DomainError> {
    let Some(name) = namespace.strip_prefix('#') else {
        return Ok(None);
    };
    if name.is_empty() || name.chars().any(char::is_whitespace) {
        return Err(invalid("a # namespace names a tag"));
    }
    Ok(Some(name.to_owned()))
}

fn namespaces(tags: &[Tag]) -> Result<Vec<String>, DomainError> {
    tags.iter()
        .filter(|tag| tag.name() == Some("L"))
        .map(|tag| {
            let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                return Err(invalid("a label namespace is empty"));
            };
            association(value)?;
            Ok(value.to_owned())
        })
        .collect()
}

fn labels(tags: &[Tag], namespaces: &[String]) -> Result<Vec<Label>, DomainError> {
    let mut labels = Vec::new();
    for tag in tags.iter().filter(|tag| tag.name() == Some("l")) {
        let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
            return Err(invalid("a label value is empty"));
        };
        let namespace = match (namespaces.is_empty(), mark(tag)?) {
            (false, Some(mark)) if namespaces.iter().any(|namespace| namespace == mark) => {
                mark.to_owned()
            }
            (false, _) => {
                return Err(invalid("an l tag must name one of the L namespaces"));
            }
            (true, Some(mark)) => mark.to_owned(),
            (true, None) => IMPLIED_NAMESPACE.to_owned(),
        };
        labels.push(Label {
            value: value.to_owned(),
            associates_tag: association(&namespace)?,
            namespace,
        });
    }
    if labels.is_empty() {
        return Err(invalid("a labeling needs an l tag"));
    }
    Ok(labels)
}

fn pubkey(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "label pubkey")
        .map_err(|_| invalid("a labeled pubkey must be 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn event_id(value: &str) -> Result<String, DomainError> {
    decode_lower_hex::<32>(value, "label event id")
        .map_err(|_| invalid("a labeled event id must be 32 lowercase hex bytes"))?;
    Ok(value.to_owned())
}

fn targets(tags: &[Tag]) -> Result<Vec<LabelTarget>, DomainError> {
    let mut targets = Vec::new();
    for tag in tags {
        match tag.name() {
            Some("e") => {
                let Some(id) = tag.value() else {
                    return Err(invalid("a labeled event id must be 32 lowercase hex bytes"));
                };
                targets.push(LabelTarget::Event {
                    id: event_id(id)?,
                    relay: relay_hint(tag)?,
                });
            }
            Some("p") => {
                let Some(value) = tag.value() else {
                    return Err(invalid("a labeled pubkey must be 32 lowercase hex bytes"));
                };
                targets.push(LabelTarget::Pubkey {
                    pubkey: pubkey(value)?,
                    relay: relay_hint(tag)?,
                });
            }
            Some("a") => {
                let Some(value) = tag.value() else {
                    return Err(invalid("a labeled address is empty"));
                };
                targets.push(LabelTarget::Address {
                    address: ReplacementAddress::from_str(value)?,
                    relay: relay_hint(tag)?,
                });
            }
            Some("r") => {
                let Some(value) = tag.value().filter(|value| is_relay(value)) else {
                    return Err(invalid("a labeled relay must be ws:// or wss://"));
                };
                targets.push(LabelTarget::Relay(value.to_owned()));
            }
            Some("t") => {
                let Some(value) = tag.value().filter(|value| !value.is_empty()) else {
                    return Err(invalid("a labeled topic is empty"));
                };
                targets.push(LabelTarget::Topic(value.to_owned()));
            }
            _ => {}
        }
    }
    if targets.is_empty() {
        return Err(invalid(
            "a kind 1985 label names an e, p, a, r, or t target",
        ));
    }
    Ok(targets)
}

/// Read NIP-32 labels from `event`.
///
/// Kind `1985` must name at least one target. Any other kind that carries
/// `l` or `L` is labeling itself.
pub fn open_labeling(event: &Event) -> Result<Labeling, DomainError> {
    let namespaces = namespaces(&event.tags)?;
    let labels = labels(&event.tags, &namespaces)?;
    let (targets, self_labeled) = if event.kind == LABEL_KIND {
        (targets(&event.tags)?, false)
    } else {
        (Vec::new(), true)
    };
    Ok(Labeling {
        content: event.content.clone(),
        labels,
        targets,
        self_labeled,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{DomainError, EventClass, RelaySigner, Tag, compare_replacement};

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"42".repeat(32)).unwrap()
    }

    fn sign(kind: u16, tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, kind, tags, content.into())
    }

    #[test]
    fn a_label_attaches_a_namespace_to_its_targets() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/32.md"
        ))
        .unwrap();
        assert!(text.contains("kind:1985"));
        assert!(text.contains("ugc"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "32.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "32.md")
        );

        let first = "ab".repeat(32);
        let second = "cd".repeat(32);
        let topic = sign(
            LABEL_KIND,
            vec![
                Tag::new(vec!["L".into(), "#t".into()]),
                Tag::new(vec!["l".into(), "permies".into(), "#t".into()]),
                Tag::new(vec![
                    "p".into(),
                    first.clone(),
                    "wss://relay.example".into(),
                ]),
                Tag::new(vec![
                    "p".into(),
                    second.clone(),
                    "wss://relay.example".into(),
                ]),
            ],
            "associate these pubkeys with the topic",
        );
        topic.validate_structure().unwrap();
        assert_eq!(topic.class(), EventClass::Regular);
        assert!(matches!(
            compare_replacement(&topic, &topic),
            Err(DomainError::NotReplaceable)
        ));
        let labeling = open_labeling(&topic).unwrap();
        assert!(!labeling.self_labeled);
        assert_eq!(labeling.labels.len(), 1);
        assert_eq!(labeling.labels[0].value, "permies");
        assert_eq!(labeling.labels[0].namespace, "#t");
        assert_eq!(labeling.labels[0].associates_tag.as_deref(), Some("t"));
        assert_eq!(
            labeling.targets,
            vec![
                LabelTarget::Pubkey {
                    pubkey: first.clone(),
                    relay: Some("wss://relay.example".into()),
                },
                LabelTarget::Pubkey {
                    pubkey: second,
                    relay: Some("wss://relay.example".into()),
                },
            ]
        );

        let event_id = "11".repeat(32);
        let moderation = sign(
            LABEL_KIND,
            vec![
                Tag::new(vec!["L".into(), "nip28.moderation".into()]),
                Tag::new(vec![
                    "l".into(),
                    "approve".into(),
                    "nip28.moderation".into(),
                ]),
                Tag::new(vec![
                    "e".into(),
                    event_id.clone(),
                    "wss://relay.example".into(),
                ]),
            ],
            "",
        );
        let moderation = open_labeling(&moderation).unwrap();
        assert_eq!(
            moderation.targets,
            vec![LabelTarget::Event {
                id: event_id,
                relay: Some("wss://relay.example".into()),
            }]
        );

        let mismatched = sign(
            LABEL_KIND,
            vec![
                Tag::new(vec!["L".into(), "license".into()]),
                Tag::new(vec!["l".into(), "MIT".into(), "other".into()]),
                Tag::new(vec!["e".into(), "11".repeat(32)]),
            ],
            "",
        );
        assert!(mismatched.validate_structure().is_err());

        let untargeted = sign(
            LABEL_KIND,
            vec![
                Tag::new(vec!["L".into(), "license".into()]),
                Tag::new(vec!["l".into(), "MIT".into(), "license".into()]),
            ],
            "",
        );
        assert!(untargeted.validate_structure().is_err());

        let implied = sign(
            LABEL_KIND,
            vec![
                Tag::new(vec!["l".into(), "spam".into()]),
                Tag::new(vec!["r".into(), "wss://spam.example".into()]),
            ],
            "",
        );
        let implied = open_labeling(&implied).unwrap();
        assert_eq!(implied.labels[0].namespace, "ugc");
        assert_eq!(
            implied.targets,
            vec![LabelTarget::Relay("wss://spam.example".into())]
        );

        let note = sign(
            1,
            vec![
                Tag::new(vec!["L".into(), "ISO-3166-2".into()]),
                Tag::new(vec!["l".into(), "IT-MI".into(), "ISO-3166-2".into()]),
                Tag::new(vec!["p".into(), first]),
            ],
            "It's beautiful here in Milan!",
        );
        note.validate_structure().unwrap();
        let note = open_labeling(&note).unwrap();
        assert!(note.self_labeled);
        assert!(note.targets.is_empty());
        assert_eq!(note.labels[0].namespace, "ISO-3166-2");
        assert_eq!(note.labels[0].value, "IT-MI");
        assert_eq!(note.content, "It's beautiful here in Milan!");
    }
}
