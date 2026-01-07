//! NIP-32: Labeling
//!
//! Content labeling for reputation, quality scores, and categorization.
//! Used for tracking inference quality in the Pylon network.

/// Label event kind
pub const KIND_LABEL: u16 = 1985;

/// Common label namespaces for OpenAgents
pub const NAMESPACE_OPENAGENTS: &str = "openagents/inference";
pub const NAMESPACE_QUALITY: &str = "quality";
pub const NAMESPACE_LATENCY: &str = "latency";
pub const NAMESPACE_RELIABILITY: &str = "reliability";

/// Check if an event is a label event
pub fn is_label_kind(kind: u16) -> bool {
    kind == KIND_LABEL
}

/// Label quality values
#[derive(Debug, Clone, PartialEq)]
pub enum QualityLabel {
    Good,
    Bad,
    Neutral,
}

impl QualityLabel {
    pub fn as_str(&self) -> &str {
        match self {
            QualityLabel::Good => "quality/good",
            QualityLabel::Bad => "quality/bad",
            QualityLabel::Neutral => "quality/neutral",
        }
    }
}

/// Extract labels from a label event
pub fn extract_labels(event: &nostr::Event) -> Vec<(String, String)> {
    if event.kind != KIND_LABEL {
        return Vec::new();
    }

    let mut labels = Vec::new();

    // Get namespace from L tag
    let mut namespace = String::new();
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "L" {
            namespace = tag[1].clone();
            break;
        }
    }

    // Get labels from l tags
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "l" {
            let label = tag[1].clone();
            let ns = if tag.len() >= 3 {
                tag[2].clone()
            } else {
                namespace.clone()
            };
            labels.push((ns, label));
        }
    }

    labels
}

/// Extract the target event ID from a label event
pub fn get_target_event_id(event: &nostr::Event) -> Option<String> {
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "e" {
            return Some(tag[1].clone());
        }
    }
    None
}

/// Extract the target pubkey from a label event
pub fn get_target_pubkey(event: &nostr::Event) -> Option<String> {
    for tag in &event.tags {
        if tag.len() >= 2 && tag[0] == "p" {
            return Some(tag[1].clone());
        }
    }
    None
}

/// Calculate a simple reputation score from label events
/// Returns a score between 0.0 and 1.0
pub fn calculate_reputation(labels: &[(String, String)]) -> f64 {
    let mut good = 0;
    let mut bad = 0;

    for (_, label) in labels {
        if label.contains("good") || label.contains("fast") || label.contains("reliable") {
            good += 1;
        } else if label.contains("bad") || label.contains("slow") || label.contains("unreliable") {
            bad += 1;
        }
    }

    let total = good + bad;
    if total == 0 {
        0.5 // Neutral default
    } else {
        good as f64 / total as f64
    }
}
