//! NIP-56: Reporting
//!
//! Implements reporting events (kind 1984) for signaling objectionable content.
//! Reports can target users (pubkeys), notes (events), or blobs (files).
//!
//! Report types:
//! - nudity: Depictions of nudity, porn, etc.
//! - malware: Virus, trojan, spyware, ransomware, etc.
//! - profanity: Profanity, hateful speech, etc.
//! - illegal: Content that may be illegal in some jurisdiction
//! - spam: Spam content
//! - impersonation: Someone pretending to be someone else
//! - other: Reports that don't fit above categories
//!
//! See: <https://github.com/nostr-protocol/nips/blob/master/56.md>

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Event kind for reports
pub const REPORT_KIND: u16 = 1984;

/// Errors that can occur during NIP-56 operations
#[derive(Debug, Error)]
pub enum Nip56Error {
    #[error("missing required field: {0}")]
    MissingRequired(String),

    #[error("invalid report type: {0}")]
    InvalidReportType(String),

    #[error("invalid format: {0}")]
    InvalidFormat(String),
}

/// Report types for objectionable content
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReportType {
    /// Depictions of nudity, porn, etc.
    Nudity,
    /// Virus, trojan horse, worm, malware, etc.
    Malware,
    /// Profanity, hateful speech, etc.
    Profanity,
    /// Something which may be illegal in some jurisdiction
    Illegal,
    /// Spam
    Spam,
    /// Someone pretending to be someone else
    Impersonation,
    /// Other reports that don't fit above categories
    Other,
}

impl ReportType {
    /// Parse report type from string
    pub fn parse(s: &str) -> Result<Self, Nip56Error> {
        match s.to_lowercase().as_str() {
            "nudity" => Ok(ReportType::Nudity),
            "malware" => Ok(ReportType::Malware),
            "profanity" => Ok(ReportType::Profanity),
            "illegal" => Ok(ReportType::Illegal),
            "spam" => Ok(ReportType::Spam),
            "impersonation" => Ok(ReportType::Impersonation),
            "other" => Ok(ReportType::Other),
            _ => Err(Nip56Error::InvalidReportType(s.to_string())),
        }
    }

    /// Convert to string
    pub fn to_string(&self) -> &str {
        match self {
            ReportType::Nudity => "nudity",
            ReportType::Malware => "malware",
            ReportType::Profanity => "profanity",
            ReportType::Illegal => "illegal",
            ReportType::Spam => "spam",
            ReportType::Impersonation => "impersonation",
            ReportType::Other => "other",
        }
    }
}

/// Target of a report
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReportTarget {
    /// Report a user by pubkey
    User {
        pubkey: String,
        report_type: ReportType,
    },
    /// Report a note/event
    Event {
        event_id: String,
        pubkey: String,
        report_type: ReportType,
    },
    /// Report a blob/file
    Blob {
        hash: String,
        event_id: String,
        pubkey: String,
        report_type: ReportType,
        server: Option<String>,
    },
}

/// Complete report event
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Report {
    /// Target of the report
    pub target: ReportTarget,

    /// Additional information about the report
    pub content: String,

    /// Optional NIP-32 labels
    pub labels: Vec<(String, String)>, // (label_value, label_namespace)
}

impl Report {
    /// Create new user report
    pub fn user(pubkey: String, report_type: ReportType) -> Self {
        Self {
            target: ReportTarget::User {
                pubkey,
                report_type,
            },
            content: String::new(),
            labels: Vec::new(),
        }
    }

    /// Create new event report
    pub fn event(event_id: String, pubkey: String, report_type: ReportType) -> Self {
        Self {
            target: ReportTarget::Event {
                event_id,
                pubkey,
                report_type,
            },
            content: String::new(),
            labels: Vec::new(),
        }
    }

    /// Create new blob report
    pub fn blob(hash: String, event_id: String, pubkey: String, report_type: ReportType) -> Self {
        Self {
            target: ReportTarget::Blob {
                hash,
                event_id,
                pubkey,
                report_type,
                server: None,
            },
            content: String::new(),
            labels: Vec::new(),
        }
    }

    /// Set content
    pub fn with_content(mut self, content: String) -> Self {
        self.content = content;
        self
    }

    /// Add NIP-32 label
    pub fn add_label(mut self, label: String, namespace: String) -> Self {
        self.labels.push((label, namespace));
        self
    }

    /// Set server for blob reports
    pub fn with_server(mut self, server: String) -> Self {
        if let ReportTarget::Blob {
            server: ref mut server_field,
            ..
        } = self.target
        {
            *server_field = Some(server);
        }
        self
    }

    /// Convert to event tags
    pub fn to_tags(&self) -> Vec<Vec<String>> {
        let mut tags = Vec::new();

        match &self.target {
            ReportTarget::User {
                pubkey,
                report_type,
            } => {
                tags.push(vec![
                    "p".to_string(),
                    pubkey.clone(),
                    report_type.to_string().to_string(),
                ]);
            }
            ReportTarget::Event {
                event_id,
                pubkey,
                report_type,
            } => {
                tags.push(vec![
                    "e".to_string(),
                    event_id.clone(),
                    report_type.to_string().to_string(),
                ]);
                tags.push(vec!["p".to_string(), pubkey.clone()]);
            }
            ReportTarget::Blob {
                hash,
                event_id,
                pubkey,
                report_type,
                server,
            } => {
                tags.push(vec![
                    "x".to_string(),
                    hash.clone(),
                    report_type.to_string().to_string(),
                ]);
                tags.push(vec![
                    "e".to_string(),
                    event_id.clone(),
                    report_type.to_string().to_string(),
                ]);
                tags.push(vec!["p".to_string(), pubkey.clone()]);
                if let Some(srv) = server {
                    tags.push(vec!["server".to_string(), srv.clone()]);
                }
            }
        }

        // Add NIP-32 labels
        if !self.labels.is_empty() {
            // Group by namespace
            let mut namespaces: Vec<String> = self
                .labels
                .iter()
                .map(|(_, ns)| ns.clone())
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();
            namespaces.sort();

            for namespace in namespaces {
                tags.push(vec!["L".to_string(), namespace.clone()]);
                for (label, ns) in &self.labels {
                    if ns == &namespace {
                        tags.push(vec!["l".to_string(), label.clone(), namespace.clone()]);
                    }
                }
            }
        }

        tags
    }

    /// Parse from event tags and content
    pub fn from_tags(tags: &[Vec<String>], content: &str) -> Result<Self, Nip56Error> {
        let mut pubkey: Option<String> = None;
        let mut event_id: Option<String> = None;
        let mut blob_hash: Option<String> = None;
        let mut report_type: Option<ReportType> = None;
        let mut server: Option<String> = None;
        let mut labels: Vec<(String, String)> = Vec::new();
        let mut current_namespace: Option<String> = None;

        for tag in tags {
            if tag.is_empty() {
                continue;
            }

            match tag[0].as_str() {
                "p" if tag.len() >= 2 => {
                    pubkey = Some(tag[1].clone());
                    if tag.len() >= 3 {
                        report_type = Some(ReportType::parse(&tag[2])?);
                    }
                }
                "e" if tag.len() >= 2 => {
                    event_id = Some(tag[1].clone());
                    if tag.len() >= 3 && report_type.is_none() {
                        report_type = Some(ReportType::parse(&tag[2])?);
                    }
                }
                "x" if tag.len() >= 2 => {
                    blob_hash = Some(tag[1].clone());
                    if tag.len() >= 3 && report_type.is_none() {
                        report_type = Some(ReportType::parse(&tag[2])?);
                    }
                }
                "server" if tag.len() >= 2 => {
                    server = Some(tag[1].clone());
                }
                "L" if tag.len() >= 2 => {
                    current_namespace = Some(tag[1].clone());
                }
                "l" if tag.len() >= 3 => {
                    if let Some(ref ns) = current_namespace {
                        labels.push((tag[1].clone(), ns.clone()));
                    }
                }
                _ => {} // Ignore unknown tags
            }
        }

        // Validate required fields
        let pubkey =
            pubkey.ok_or_else(|| Nip56Error::MissingRequired("pubkey (p tag)".to_string()))?;
        let report_type =
            report_type.ok_or_else(|| Nip56Error::MissingRequired("report type".to_string()))?;

        // Determine target type
        let target = if let Some(hash) = blob_hash {
            let event_id = event_id.ok_or_else(|| {
                Nip56Error::MissingRequired(
                    "event_id (e tag) required for blob reports".to_string(),
                )
            })?;
            ReportTarget::Blob {
                hash,
                event_id,
                pubkey,
                report_type,
                server,
            }
        } else if let Some(event_id) = event_id {
            ReportTarget::Event {
                event_id,
                pubkey,
                report_type,
            }
        } else {
            ReportTarget::User {
                pubkey,
                report_type,
            }
        };

        Ok(Self {
            target,
            content: content.to_string(),
            labels,
        })
    }
}

/// Check if a kind is a report event
pub fn is_report_kind(kind: u16) -> bool {
    kind == REPORT_KIND
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_report_type_parse() {
        assert_eq!(ReportType::parse("nudity").unwrap(), ReportType::Nudity);
        assert_eq!(ReportType::parse("NUDITY").unwrap(), ReportType::Nudity);
        assert_eq!(ReportType::parse("spam").unwrap(), ReportType::Spam);
        assert_eq!(ReportType::parse("illegal").unwrap(), ReportType::Illegal);
        assert!(ReportType::parse("unknown").is_err());
    }

    #[test]
    fn test_report_type_to_string() {
        assert_eq!(ReportType::Nudity.to_string(), "nudity");
        assert_eq!(ReportType::Malware.to_string(), "malware");
        assert_eq!(ReportType::Spam.to_string(), "spam");
    }

    #[test]
    fn test_report_user() {
        let report = Report::user("pubkey123".to_string(), ReportType::Spam);
        assert_eq!(
            report.target,
            ReportTarget::User {
                pubkey: "pubkey123".to_string(),
                report_type: ReportType::Spam
            }
        );
    }

    #[test]
    fn test_report_event() {
        let report = Report::event(
            "event123".to_string(),
            "pubkey456".to_string(),
            ReportType::Illegal,
        );
        assert_eq!(
            report.target,
            ReportTarget::Event {
                event_id: "event123".to_string(),
                pubkey: "pubkey456".to_string(),
                report_type: ReportType::Illegal
            }
        );
    }

    #[test]
    fn test_report_blob() {
        let report = Report::blob(
            "hash789".to_string(),
            "event123".to_string(),
            "pubkey456".to_string(),
            ReportType::Malware,
        )
        .with_server("https://example.com/file.bin".to_string());

        if let ReportTarget::Blob { server, .. } = &report.target {
            assert_eq!(server, &Some("https://example.com/file.bin".to_string()));
        } else {
            panic!("Expected blob target");
        }
    }

    #[test]
    fn test_report_with_content() {
        let report = Report::user("pubkey123".to_string(), ReportType::Impersonation)
            .with_content("This user is impersonating someone".to_string());
        assert_eq!(report.content, "This user is impersonating someone");
    }

    #[test]
    fn test_report_with_labels() {
        let report = Report::user("pubkey123".to_string(), ReportType::Nudity)
            .add_label("NS-nud".to_string(), "social.nos.ontology".to_string());
        assert_eq!(report.labels.len(), 1);
        assert_eq!(
            report.labels[0],
            ("NS-nud".to_string(), "social.nos.ontology".to_string())
        );
    }

    #[test]
    fn test_report_user_to_tags() {
        let report = Report::user("pubkey123".to_string(), ReportType::Spam);
        let tags = report.to_tags();

        assert!(tags.contains(&vec![
            "p".to_string(),
            "pubkey123".to_string(),
            "spam".to_string()
        ]));
    }

    #[test]
    fn test_report_event_to_tags() {
        let report = Report::event(
            "event123".to_string(),
            "pubkey456".to_string(),
            ReportType::Illegal,
        )
        .with_content("Illegal content".to_string());

        let tags = report.to_tags();

        assert!(tags.contains(&vec![
            "e".to_string(),
            "event123".to_string(),
            "illegal".to_string()
        ]));
        assert!(tags.contains(&vec!["p".to_string(), "pubkey456".to_string()]));
    }

    #[test]
    fn test_report_blob_to_tags() {
        let report = Report::blob(
            "hash789".to_string(),
            "event123".to_string(),
            "pubkey456".to_string(),
            ReportType::Malware,
        )
        .with_server("https://example.com/malware.exe".to_string());

        let tags = report.to_tags();

        assert!(tags.contains(&vec![
            "x".to_string(),
            "hash789".to_string(),
            "malware".to_string()
        ]));
        assert!(tags.contains(&vec![
            "e".to_string(),
            "event123".to_string(),
            "malware".to_string()
        ]));
        assert!(tags.contains(&vec!["p".to_string(), "pubkey456".to_string()]));
        assert!(tags.contains(&vec![
            "server".to_string(),
            "https://example.com/malware.exe".to_string()
        ]));
    }

    #[test]
    fn test_report_from_tags_user() {
        let tags = vec![vec![
            "p".to_string(),
            "pubkey123".to_string(),
            "spam".to_string(),
        ]];

        let report = Report::from_tags(&tags, "").unwrap();

        assert_eq!(
            report.target,
            ReportTarget::User {
                pubkey: "pubkey123".to_string(),
                report_type: ReportType::Spam
            }
        );
    }

    #[test]
    fn test_report_from_tags_event() {
        let tags = vec![
            vec![
                "e".to_string(),
                "event123".to_string(),
                "illegal".to_string(),
            ],
            vec!["p".to_string(), "pubkey456".to_string()],
        ];

        let report = Report::from_tags(&tags, "Bad content").unwrap();

        assert_eq!(
            report.target,
            ReportTarget::Event {
                event_id: "event123".to_string(),
                pubkey: "pubkey456".to_string(),
                report_type: ReportType::Illegal
            }
        );
        assert_eq!(report.content, "Bad content");
    }

    #[test]
    fn test_report_from_tags_missing_pubkey() {
        let tags = vec![vec![
            "e".to_string(),
            "event123".to_string(),
            "spam".to_string(),
        ]];

        let result = Report::from_tags(&tags, "");
        assert!(result.is_err());
    }

    #[test]
    fn test_report_roundtrip() {
        let original = Report::event(
            "event123".to_string(),
            "pubkey456".to_string(),
            ReportType::Profanity,
        )
        .with_content("Hateful speech".to_string())
        .add_label("hate".to_string(), "custom.labels".to_string());

        let tags = original.to_tags();
        let reconstructed = Report::from_tags(&tags, &original.content).unwrap();

        assert_eq!(reconstructed.target, original.target);
        assert_eq!(reconstructed.content, original.content);
    }

    #[test]
    fn test_is_report_kind() {
        assert!(is_report_kind(1984));
        assert!(!is_report_kind(1));
        assert!(!is_report_kind(1985));
    }
}
