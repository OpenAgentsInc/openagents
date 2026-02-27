//! SKL trust evaluation using NIP-32 labels.

use crate::nip01::Event;
use std::collections::{BTreeSet, HashSet};

const SKILL_SECURITY_NAMESPACE: &str = "skill-security";

/// Trust decision for skill loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TrustDecision {
    Trusted,
    Untrusted,
    Blocked,
}

/// Trust-evaluation output for runtime gates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustEvaluation {
    pub decision: TrustDecision,
    pub positives: Vec<String>,
    pub negatives: Vec<String>,
    pub supporting_events: Vec<String>,
    pub reasons: Vec<String>,
}

impl TrustEvaluation {
    pub fn is_trusted(&self) -> bool {
        self.decision == TrustDecision::Trusted
    }
}

/// Trust policy for evaluating NIP-32 labels on skills.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrustPolicy {
    /// Minimum number of positive labels required to trust.
    pub minimum_positive_labels: usize,
    /// Kill labels that immediately block at quorum.
    pub kill_labels: HashSet<String>,
    /// Minimum unique publishers needed for kill-label quorum.
    pub minimum_kill_label_publishers: usize,
    /// Require explicit capabilities verification label.
    pub require_capabilities_verified: bool,
    /// Optional allowlist of label-publisher pubkeys.
    pub trusted_issuers: HashSet<String>,
}

impl Default for TrustPolicy {
    fn default() -> Self {
        Self {
            minimum_positive_labels: 1,
            kill_labels: [
                "malicious-confirmed",
                "prompt-injection",
                "credential-exfil",
                "capability-violation",
            ]
            .into_iter()
            .map(str::to_string)
            .collect(),
            minimum_kill_label_publishers: 1,
            require_capabilities_verified: false,
            trusted_issuers: HashSet::new(),
        }
    }
}

/// Evaluate skill trust from NIP-32 label events.
///
/// A label event is considered relevant if it references one of:
/// - `a` tag == `manifest_address`
/// - `e` tag == `manifest_event_id` (if provided)
/// - `p` tag == `publisher_pubkey`
pub fn evaluate_skill_trust(
    manifest_address: &str,
    manifest_event_id: Option<&str>,
    publisher_pubkey: &str,
    label_events: &[Event],
    policy: &TrustPolicy,
) -> TrustEvaluation {
    let positive_labels: HashSet<&str> = [
        "audit-passed",
        "scan-clean",
        "capabilities-verified",
        "delivery-hash-verified",
    ]
    .into_iter()
    .collect();

    let kill_labels: HashSet<&str> = policy.kill_labels.iter().map(String::as_str).collect();
    let mut positives = BTreeSet::new();
    let mut negatives = BTreeSet::new();
    let mut supporting_events = BTreeSet::new();
    let mut kill_label_publishers = BTreeSet::new();

    for event in label_events {
        if event.kind != crate::nip32::KIND_LABEL as u16 {
            continue;
        }
        if !policy.trusted_issuers.is_empty() && !policy.trusted_issuers.contains(&event.pubkey) {
            continue;
        }
        if !targets_skill(event, manifest_address, manifest_event_id, publisher_pubkey) {
            continue;
        }

        let labels = extract_namespace_labels(event, SKILL_SECURITY_NAMESPACE);
        if labels.is_empty() {
            continue;
        }

        supporting_events.insert(event.id.clone());

        for label in labels {
            if positive_labels.contains(label.as_str()) {
                positives.insert(label.clone());
            }
            if kill_labels.contains(label.as_str()) {
                negatives.insert(label.clone());
                kill_label_publishers.insert(event.pubkey.clone());
            }
        }
    }

    let mut reasons = Vec::new();
    let decision = if kill_label_publishers.len() >= policy.minimum_kill_label_publishers {
        reasons.push(format!(
            "kill-label quorum reached ({} publishers)",
            kill_label_publishers.len()
        ));
        TrustDecision::Blocked
    } else if policy.require_capabilities_verified && !positives.contains("capabilities-verified") {
        reasons.push("missing required label: capabilities-verified".to_string());
        TrustDecision::Untrusted
    } else if positives.len() >= policy.minimum_positive_labels {
        TrustDecision::Trusted
    } else {
        reasons.push(format!(
            "insufficient positive labels: need {}, got {}",
            policy.minimum_positive_labels,
            positives.len()
        ));
        TrustDecision::Untrusted
    };

    TrustEvaluation {
        decision,
        positives: positives.into_iter().collect(),
        negatives: negatives.into_iter().collect(),
        supporting_events: supporting_events.into_iter().collect(),
        reasons,
    }
}

fn targets_skill(
    event: &Event,
    manifest_address: &str,
    manifest_event_id: Option<&str>,
    publisher_pubkey: &str,
) -> bool {
    let matches_a = event.tags.iter().any(|tag| {
        tag.first().map(String::as_str) == Some("a")
            && tag.get(1).map(String::as_str) == Some(manifest_address)
    });

    let matches_e = manifest_event_id.is_some_and(|event_id| {
        event.tags.iter().any(|tag| {
            tag.first().map(String::as_str) == Some("e")
                && tag.get(1).map(String::as_str) == Some(event_id)
        })
    });

    let matches_p = event.tags.iter().any(|tag| {
        tag.first().map(String::as_str) == Some("p")
            && tag.get(1).map(String::as_str) == Some(publisher_pubkey)
    });

    matches_a || matches_e || matches_p
}

fn extract_namespace_labels(event: &Event, namespace: &str) -> Vec<String> {
    let namespaces: Vec<&str> = event
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("L"))
        .filter_map(|tag| tag.get(1).map(String::as_str))
        .collect();

    event
        .tags
        .iter()
        .filter(|tag| tag.first().map(String::as_str) == Some("l"))
        .filter_map(|tag| {
            let label = tag.get(1)?.trim().to_lowercase();
            let tag_namespace = if let Some(explicit_namespace) = tag.get(2) {
                explicit_namespace.as_str()
            } else if namespaces.len() == 1 {
                namespaces[0]
            } else {
                "ugc"
            };
            if tag_namespace == namespace {
                Some(label)
            } else {
                None
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn label_event(
        id: &str,
        pubkey: &str,
        labels: &[(&str, &str)],
        targets: &[Vec<&str>],
    ) -> Event {
        let mut tags = Vec::new();
        let mut namespaces = BTreeSet::new();
        for (_, namespace) in labels {
            namespaces.insert(*namespace);
        }
        for namespace in namespaces {
            tags.push(vec!["L".to_string(), namespace.to_string()]);
        }
        for (label, namespace) in labels {
            tags.push(vec![
                "l".to_string(),
                label.to_string(),
                namespace.to_string(),
            ]);
        }
        for target in targets {
            tags.push(target.iter().map(|entry| entry.to_string()).collect());
        }

        Event {
            id: id.to_string(),
            pubkey: pubkey.to_string(),
            created_at: 1_740_400_000,
            kind: crate::nip32::KIND_LABEL as u16,
            tags,
            content: String::new(),
            sig: "sig".to_string(),
        }
    }

    #[test]
    fn test_trust_evaluation_trusted() {
        let address = "33400:skillpub:research-assistant";
        let labels = vec![
            label_event(
                "label-1",
                "auditor-1",
                &[("audit-passed", "skill-security")],
                &[vec!["a", address]],
            ),
            label_event(
                "label-2",
                "auditor-2",
                &[("capabilities-verified", "skill-security")],
                &[vec!["e", "manifest-event-id"]],
            ),
        ];

        let policy = TrustPolicy {
            minimum_positive_labels: 2,
            require_capabilities_verified: true,
            ..Default::default()
        };

        let evaluation = evaluate_skill_trust(
            address,
            Some("manifest-event-id"),
            "skillpub",
            &labels,
            &policy,
        );

        assert_eq!(evaluation.decision, TrustDecision::Trusted);
        assert!(evaluation.positives.contains(&"audit-passed".to_string()));
        assert!(
            evaluation
                .positives
                .contains(&"capabilities-verified".to_string())
        );
    }

    #[test]
    fn test_trust_evaluation_blocked_on_kill_label() {
        let address = "33400:skillpub:research-assistant";
        let labels = vec![label_event(
            "label-1",
            "auditor-1",
            &[("malicious-confirmed", "skill-security")],
            &[vec!["a", address]],
        )];

        let evaluation =
            evaluate_skill_trust(address, None, "skillpub", &labels, &TrustPolicy::default());

        assert_eq!(evaluation.decision, TrustDecision::Blocked);
        assert!(
            evaluation
                .negatives
                .contains(&"malicious-confirmed".to_string())
        );
    }

    #[test]
    fn test_trust_evaluation_respects_trusted_issuers_allowlist() {
        let address = "33400:skillpub:research-assistant";
        let labels = vec![label_event(
            "label-1",
            "unknown-auditor",
            &[("audit-passed", "skill-security")],
            &[vec!["a", address]],
        )];

        let mut policy = TrustPolicy::default();
        policy.trusted_issuers.insert("known-auditor".to_string());

        let evaluation = evaluate_skill_trust(address, None, "skillpub", &labels, &policy);
        assert_eq!(evaluation.decision, TrustDecision::Untrusted);
    }
}
