//! SKL revocation helpers with NIP-09 same-pubkey authority semantics.

use super::manifest::KIND_SKILL_MANIFEST;
use crate::nip01::Event;

/// Revocation lookup result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevocationStatus {
    pub revoked: bool,
    pub revocation_event_id: Option<String>,
    pub reason: Option<String>,
}

impl RevocationStatus {
    pub fn active() -> Self {
        Self {
            revoked: false,
            revocation_event_id: None,
            reason: None,
        }
    }

    pub fn revoked(event_id: String, reason: Option<String>) -> Self {
        Self {
            revoked: true,
            revocation_event_id: Some(event_id),
            reason,
        }
    }
}

/// Determine whether a deletion request has authority over a publisher's skill events.
///
/// NIP-09 authority rule: revocation is authoritative only when deletion-request pubkey
/// equals the publisher pubkey of the target SKL manifest.
pub fn is_authoritative_revocation(deletion_request: &Event, publisher_pubkey: &str) -> bool {
    deletion_request.kind == crate::nip09::DELETION_REQUEST_KIND
        && deletion_request.pubkey == publisher_pubkey
}

/// Build deterministic SKL manifest-revocation tags.
///
/// Includes:
/// - `e`: specific manifest event id
/// - `a`: canonical manifest address
/// - `k`: manifest kind (`33400`)
pub fn build_manifest_revocation_tags(
    manifest_event_id: &str,
    manifest_address: &str,
) -> Vec<Vec<String>> {
    vec![
        vec!["e".to_string(), manifest_event_id.to_string()],
        vec!["a".to_string(), manifest_address.to_string()],
        vec!["k".to_string(), KIND_SKILL_MANIFEST.to_string()],
    ]
}

/// Check revocation status for a manifest event/address pair.
pub fn manifest_revocation_status(
    publisher_pubkey: &str,
    manifest_event_id: &str,
    manifest_address: &str,
    deletion_requests: &[Event],
) -> RevocationStatus {
    let mut latest: Option<&Event> = None;

    for deletion in deletion_requests {
        if !is_authoritative_revocation(deletion, publisher_pubkey) {
            continue;
        }
        if !references_manifest(deletion, manifest_event_id, manifest_address) {
            continue;
        }
        if !targets_manifest_kind_or_unspecified(deletion) {
            continue;
        }

        latest = match latest {
            Some(previous) => {
                if (deletion.created_at, &deletion.id) > (previous.created_at, &previous.id) {
                    Some(deletion)
                } else {
                    Some(previous)
                }
            }
            None => Some(deletion),
        };
    }

    if let Some(revocation) = latest {
        let reason = if revocation.content.trim().is_empty() {
            None
        } else {
            Some(revocation.content.clone())
        };
        RevocationStatus::revoked(revocation.id.clone(), reason)
    } else {
        RevocationStatus::active()
    }
}

fn references_manifest(deletion: &Event, manifest_event_id: &str, manifest_address: &str) -> bool {
    deletion.tags.iter().any(|tag| {
        (tag.first().map(String::as_str) == Some("e")
            && tag.get(1).map(String::as_str) == Some(manifest_event_id))
            || (tag.first().map(String::as_str) == Some("a")
                && tag.get(1).map(String::as_str) == Some(manifest_address))
    })
}

fn targets_manifest_kind_or_unspecified(deletion: &Event) -> bool {
    let kinds: Vec<u16> = deletion
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("k"))
        .filter_map(|tag| tag.get(1))
        .filter_map(|kind| kind.parse::<u16>().ok())
        .collect();

    kinds.is_empty() || kinds.contains(&KIND_SKILL_MANIFEST)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deletion_request(
        id: &str,
        pubkey: &str,
        created_at: u64,
        tags: Vec<Vec<String>>,
        content: &str,
    ) -> Event {
        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at,
            kind: crate::nip09::DELETION_REQUEST_KIND,
            tags,
            content: content.to_string(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_non_authoritative_revocation_is_ignored() {
        let status = manifest_revocation_status(
            "skillpub",
            "manifest-id",
            "33400:skillpub:research-assistant",
            &[deletion_request(
                "delete-1",
                "otherpub",
                1_740_400_000,
                build_manifest_revocation_tags("manifest-id", "33400:skillpub:research-assistant"),
                "malicious",
            )],
        );

        assert!(!status.revoked);
    }

    #[test]
    fn test_manifest_revoked_by_same_pubkey_event_ref() {
        let status = manifest_revocation_status(
            "skillpub",
            "manifest-id",
            "33400:skillpub:research-assistant",
            &[deletion_request(
                "delete-1",
                "skillpub",
                1_740_400_000,
                vec![
                    vec!["e".to_string(), "manifest-id".to_string()],
                    vec!["k".to_string(), KIND_SKILL_MANIFEST.to_string()],
                ],
                "critical-vuln",
            )],
        );

        assert!(status.revoked);
        assert_eq!(status.reason, Some("critical-vuln".to_string()));
    }

    #[test]
    fn test_manifest_revoked_by_same_pubkey_address_ref() {
        let status = manifest_revocation_status(
            "skillpub",
            "manifest-id",
            "33400:skillpub:research-assistant",
            &[deletion_request(
                "delete-1",
                "skillpub",
                1_740_400_000,
                vec![
                    vec![
                        "a".to_string(),
                        "33400:skillpub:research-assistant".to_string(),
                    ],
                    vec!["k".to_string(), KIND_SKILL_MANIFEST.to_string()],
                ],
                "",
            )],
        );

        assert!(status.revoked);
        assert_eq!(status.reason, None);
    }

    #[test]
    fn test_build_manifest_revocation_tags() {
        let tags =
            build_manifest_revocation_tags("manifest-id", "33400:skillpub:research-assistant");
        assert_eq!(tags[0], vec!["e", "manifest-id"]);
        assert_eq!(tags[1], vec!["a", "33400:skillpub:research-assistant"]);
        assert_eq!(tags[2], vec!["k", "33400"]);
    }
}
