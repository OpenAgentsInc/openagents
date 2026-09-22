//! NIP-78 arbitrary application data.
//!
//! Kind `30078` is an addressable record. Its `d` tag names the app and
//! the context, or any other string. Kind `78` is a regular event for
//! many rows of the same type. Content and the other tags stay opaque.
//!
//! Kind `30078` stays out of search. The relay does not decrypt the
//! content or decide which app owns an identifier. Neither kind is
//! added to the NIP-11 list.

use super::{DomainError, Event};

const MANY_KIND: u16 = 78;
const RECORD_KIND: u16 = 30_078;

/// One tag other than the identifier.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppTag {
    pub name: String,
    pub values: Vec<String>,
}

/// A kind `30078` record or a kind `78` event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppData {
    pub kind: u16,
    pub identifier: Option<String>,
    pub content: String,
    pub tags: Vec<AppTag>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn identifier(value: &str) -> Result<String, DomainError> {
    if value.is_empty() || value.len() > 1_024 || value.chars().any(char::is_control) {
        return Err(invalid("an application identifier is 1 to 1024 characters"));
    }
    Ok(value.to_owned())
}

/// Read kind `30078` or kind `78` application data.
///
/// # Errors
///
/// Returns a sentence when the kind or the identifier is refused.
pub fn open_app_data(event: &Event) -> Result<AppData, DomainError> {
    if !matches!(event.kind, MANY_KIND | RECORD_KIND) {
        return Err(invalid("application data has kind 30078 or 78"));
    }
    let identifiers = event
        .tags
        .iter()
        .filter(|tag| tag.name() == Some("d"))
        .collect::<Vec<_>>();
    let identifier = if event.kind == RECORD_KIND {
        if identifiers.len() != 1 {
            return Err(invalid("an application record has one d tag"));
        }
        let Some(value) = identifiers[0].value() else {
            return Err(invalid("an application record has one d tag"));
        };
        Some(identifier(value)?)
    } else {
        match identifiers.len() {
            0 => None,
            1 => {
                let Some(value) = identifiers[0].value() else {
                    return Err(invalid("an application identifier is 1 to 1024 characters"));
                };
                Some(identifier(value)?)
            }
            _ => return Err(invalid("an application event has one d tag")),
        }
    };
    let tags = event
        .tags
        .iter()
        .filter(|tag| tag.name() != Some("d"))
        .map(|tag| AppTag {
            name: tag.name().unwrap_or_default().to_owned(),
            values: tag.as_slice().iter().skip(1).cloned().collect(),
        })
        .collect();
    Ok(AppData {
        kind: event.kind,
        identifier,
        content: event.content.clone(),
        tags,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DomainError, EventClass, RelaySigner, ReplacementDecision, Tag, compare_replacement,
        search_matches,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"78".repeat(32)).unwrap()
    }

    #[test]
    fn an_application_record_replaces_on_its_identifier_and_a_plain_event_does_not() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/78.md"
        ))
        .unwrap();
        assert!(text.contains("30078"));
        assert!(text.contains("kind `78`"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "78.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "78.md")
        );

        let author = signer();
        let record = author.sign(
            1_700_000_000,
            RECORD_KIND,
            vec![
                Tag::new(vec!["d".into(), "settings:theme".into()]),
                Tag::new(vec!["t".into(), "client".into()]),
            ],
            r#"{"theme":"amber"}"#.into(),
        );
        record.validate_structure().unwrap();
        assert_eq!(record.class(), EventClass::Addressable);
        let opened = open_app_data(&record).unwrap();
        assert_eq!(opened.identifier.as_deref(), Some("settings:theme"));
        assert_eq!(opened.content, r#"{"theme":"amber"}"#);
        assert_eq!(opened.tags.len(), 1);
        assert_eq!(opened.tags[0].name, "t");
        assert!(!search_matches("amber", RECORD_KIND, &record.content));
        let newer = author.sign(
            1_700_000_100,
            RECORD_KIND,
            vec![Tag::new(vec!["d".into(), "settings:theme".into()])],
            r#"{"theme":"ink"}"#.into(),
        );
        assert_eq!(
            compare_replacement(&record, &newer).unwrap(),
            ReplacementDecision::ReplaceCurrent
        );
        let other = author.sign(
            1_700_000_200,
            RECORD_KIND,
            vec![Tag::new(vec!["d".into(), "settings:font".into()])],
            String::new(),
        );
        assert!(matches!(
            compare_replacement(&record, &other),
            Err(DomainError::ReplacementAddressMismatch)
        ));
        let missing = author.sign(1_700_000_300, RECORD_KIND, Vec::new(), String::new());
        assert!(missing.validate_structure().is_err());

        let many = author.sign(
            1_700_000_400,
            MANY_KIND,
            vec![Tag::new(vec!["d".into(), "log".into()])],
            "one row".into(),
        );
        many.validate_structure().unwrap();
        assert_eq!(many.class(), EventClass::Regular);
        let plain = open_app_data(&many).unwrap();
        assert_eq!(plain.identifier.as_deref(), Some("log"));
        assert!(search_matches("row", MANY_KIND, &many.content));
        let later = author.sign(
            1_700_000_500,
            MANY_KIND,
            vec![Tag::new(vec!["d".into(), "log".into()])],
            "another row".into(),
        );
        assert!(matches!(
            compare_replacement(&many, &later),
            Err(DomainError::NotReplaceable)
        ));
    }
}
