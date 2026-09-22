//! NIP-56 reporting.
//!
//! Kind `1984` names a user (`p`), a note (`e`), or a blob (`x`). The third
//! value of the tag being reported is one of the pinned report types. A blob
//! report also names the event that carries the blob. The pinned blob example
//! has no `p` tag, so that form may omit the user.
//!
//! `l` and `L` tags follow NIP-32 and describe the report itself. Kind `1984`
//! is a regular event. The relay stores it and does not delete or hide the
//! referenced event. The hash and the `server` URL are not fetched.

use super::hex::decode_lower_hex;
use super::{DomainError, Event, Tag};

const KIND: u16 = 1_984;

/// The report types the pinned text allows in the third tag position.
pub const REPORT_TYPES: &[&str] = &[
    "nudity",
    "malware",
    "profanity",
    "illegal",
    "spam",
    "impersonation",
    "other",
];

/// One user, note, or blob named by a report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReportTarget {
    User {
        pubkey: String,
        report_type: Option<String>,
    },
    Note {
        id: String,
        report_type: Option<String>,
    },
    Blob {
        hash: String,
        report_type: Option<String>,
    },
}

/// A kind `1984` report.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Report {
    pub content: String,
    pub targets: Vec<ReportTarget>,
    pub servers: Vec<String>,
}

fn invalid(reason: &str) -> DomainError {
    DomainError::InvalidEvent(reason.to_owned())
}

fn hex32(value: &str, reason: &'static str) -> Result<(), DomainError> {
    decode_lower_hex::<32>(value, reason).map_err(|_| invalid(reason))?;
    Ok(())
}

fn report_type(tag: &Tag) -> Result<Option<String>, DomainError> {
    match tag.as_slice().get(2) {
        None => Ok(None),
        Some(value) if value.is_empty() => Err(invalid("a report type is empty")),
        Some(value) if REPORT_TYPES.contains(&value.as_str()) => Ok(Some(value.clone())),
        Some(_) => Err(invalid(
            "a report type is nudity, malware, profanity, illegal, spam, impersonation, or other",
        )),
    }
}

fn valid_http_url(value: &str) -> bool {
    let Some(rest) = value
        .strip_prefix("http://")
        .or_else(|| value.strip_prefix("https://"))
    else {
        return false;
    };
    let authority = rest.split(['/', '?', '#']).next().unwrap_or_default();
    !authority.is_empty() && value.len() <= 2_048 && !value.chars().any(char::is_whitespace)
}

/// Read a kind `1984` report. The relay does not fetch or moderate from it.
///
/// # Errors
///
/// Returns a sentence when the kind, a target, a type, or a server URL does
/// not match the pinned text.
pub fn open_report(event: &Event) -> Result<Report, DomainError> {
    if event.kind != KIND {
        return Err(invalid("a report has kind 1984"));
    }
    let mut targets = Vec::new();
    let mut saw_p = false;
    let mut saw_e = false;
    let mut saw_x = false;
    let mut saw_type = false;
    for tag in &event.tags {
        match tag.name() {
            Some("p") => {
                let Some(pubkey) = tag.value() else {
                    return Err(invalid("a reported pubkey must be 32 lowercase hex bytes"));
                };
                hex32(pubkey, "a reported pubkey must be 32 lowercase hex bytes")?;
                let report_type = report_type(tag)?;
                saw_type |= report_type.is_some();
                saw_p = true;
                targets.push(ReportTarget::User {
                    pubkey: pubkey.to_owned(),
                    report_type,
                });
            }
            Some("e") => {
                let Some(id) = tag.value() else {
                    return Err(invalid(
                        "a reported event id must be 32 lowercase hex bytes",
                    ));
                };
                hex32(id, "a reported event id must be 32 lowercase hex bytes")?;
                let report_type = report_type(tag)?;
                saw_type |= report_type.is_some();
                saw_e = true;
                targets.push(ReportTarget::Note {
                    id: id.to_owned(),
                    report_type,
                });
            }
            Some("x") => {
                let Some(hash) = tag.value() else {
                    return Err(invalid(
                        "a reported blob hash must be 32 lowercase hex bytes",
                    ));
                };
                hex32(hash, "a reported blob hash must be 32 lowercase hex bytes")?;
                let report_type = report_type(tag)?;
                saw_type |= report_type.is_some();
                saw_x = true;
                targets.push(ReportTarget::Blob {
                    hash: hash.to_owned(),
                    report_type,
                });
            }
            _ => {}
        }
    }
    if targets.is_empty() {
        return Err(invalid("a report names a p, e, or x tag"));
    }
    if !saw_type {
        return Err(invalid(
            "a report type is the third value of the p, e, or x tag",
        ));
    }
    if saw_x && !saw_e {
        return Err(invalid("a blob report also names the event in an e tag"));
    }
    if !saw_p && !saw_x {
        return Err(invalid("a report names the user in a p tag"));
    }
    let mut servers = Vec::new();
    for tag in event.tags.iter().filter(|tag| tag.name() == Some("server")) {
        let Some(url) = tag.value().filter(|url| valid_http_url(url)) else {
            return Err(invalid("a report server tag is an http:// or https:// URL"));
        };
        servers.push(url.to_owned());
    }
    Ok(Report {
        content: event.content.clone(),
        targets,
        servers,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        DeletionRequest, DomainError, EventClass, RelaySigner, Tag, compare_replacement,
        open_labeling,
    };

    fn signer() -> RelaySigner {
        RelaySigner::from_secret_hex(&"56".repeat(32)).unwrap()
    }

    fn sign(tags: Vec<Tag>, content: &str) -> Event {
        signer().sign(1_700_000_000, KIND, tags, content.into())
    }

    #[test]
    fn a_report_names_the_user_or_the_blob_and_does_not_delete_it() {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../nips/official/56.md"
        ))
        .unwrap();
        assert!(text.contains("kind 1984"));
        assert!(text.contains("nudity"));
        assert!(text.contains("impersonation"));
        assert!(text.contains("He's insulting the king!"));
        let row = crate::lane::PROVEN
            .iter()
            .find(|row| row.file == "56.md")
            .unwrap();
        assert_eq!(row.status, "configured-and-proven");
        assert!(
            crate::lane::SHAPES
                .iter()
                .all(|shape| shape.file != "56.md")
        );

        let user = "56".repeat(32);
        let note = "ab".repeat(32);
        let blob = "cd".repeat(32);
        let profile = sign(
            vec![
                Tag::new(vec!["p".into(), user.clone(), "nudity".into()]),
                Tag::new(vec!["L".into(), "social.nos.ontology".into()]),
                Tag::new(vec![
                    "l".into(),
                    "NS-nud".into(),
                    "social.nos.ontology".into(),
                ]),
            ],
            "",
        );
        profile.validate_structure().unwrap();
        assert_eq!(profile.class(), EventClass::Regular);
        assert!(matches!(
            compare_replacement(&profile, &profile),
            Err(DomainError::NotReplaceable)
        ));
        assert!(matches!(
            DeletionRequest::from_event(&profile),
            Err(DomainError::NotDeletionRequest)
        ));
        let opened = open_report(&profile).unwrap();
        assert_eq!(
            opened.targets,
            vec![ReportTarget::User {
                pubkey: user.clone(),
                report_type: Some("nudity".into()),
            }]
        );
        let labeling = open_labeling(&profile).unwrap();
        assert!(labeling.self_labeled);
        assert_eq!(labeling.labels[0].value, "NS-nud");
        assert_eq!(labeling.labels[0].namespace, "social.nos.ontology");

        let insult = sign(
            vec![
                Tag::new(vec!["e".into(), note.clone(), "illegal".into()]),
                Tag::new(vec!["p".into(), user.clone()]),
            ],
            "He's insulting the king!",
        );
        insult.validate_structure().unwrap();
        let insult = open_report(&insult).unwrap();
        assert_eq!(insult.content, "He's insulting the king!");
        assert!(insult.targets.iter().any(|target| matches!(
            target,
            ReportTarget::Note {
                id,
                report_type: Some(kind),
            } if id == &note && kind == "illegal"
        )));

        let impersonation = "Profile is impersonating nostr:<victim bech32 pubkey>";
        let impersonation = sign(
            vec![Tag::new(vec![
                "p".into(),
                user.clone(),
                "impersonation".into(),
            ])],
            impersonation,
        );
        impersonation.validate_structure().unwrap();
        assert_eq!(
            open_report(&impersonation).unwrap().content,
            "Profile is impersonating nostr:<victim bech32 pubkey>"
        );

        let server = "https://you-may-find-the-blob-here.com/path-to-url.ext";
        let malware = sign(
            vec![
                Tag::new(vec!["x".into(), blob.clone(), "malware".into()]),
                Tag::new(vec!["e".into(), note, "malware".into()]),
                Tag::new(vec!["server".into(), server.into()]),
            ],
            "This file contains malware software in it.",
        );
        malware.validate_structure().unwrap();
        let malware = open_report(&malware).unwrap();
        assert_eq!(malware.servers, vec![server.to_owned()]);
        assert!(malware.targets.iter().any(|target| matches!(
            target,
            ReportTarget::Blob {
                hash,
                report_type: Some(kind),
            } if hash == &blob && kind == "malware"
        )));

        let untyped = sign(vec![Tag::new(vec!["p".into(), user])], "");
        assert!(untyped.validate_structure().is_err());
        let blob_only = sign(vec![Tag::new(vec!["x".into(), blob, "malware".into()])], "");
        assert!(blob_only.validate_structure().is_err());
        let unknown = sign(
            vec![Tag::new(vec!["p".into(), "56".repeat(32), "robot".into()])],
            "",
        );
        assert!(unknown.validate_structure().is_err());
    }
}
