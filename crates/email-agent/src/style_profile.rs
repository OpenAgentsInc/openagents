use crate::NormalizedConversationItem;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum StyleTone {
    Formal,
    Neutral,
    Friendly,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct StyleProfile {
    pub profile_id: String,
    pub sample_count: usize,
    pub average_sentence_words_milli: u32,
    pub question_rate_milli: u32,
    pub exclamation_rate_milli: u32,
    pub greeting_markers: Vec<String>,
    pub signoff_markers: Vec<String>,
    pub preferred_tone: StyleTone,
}

pub fn derive_style_profile(
    profile_id: &str,
    items: &[NormalizedConversationItem],
) -> StyleProfile {
    if items.is_empty() {
        return StyleProfile {
            profile_id: profile_id.to_string(),
            sample_count: 0,
            average_sentence_words_milli: 0,
            question_rate_milli: 0,
            exclamation_rate_milli: 0,
            greeting_markers: Vec::new(),
            signoff_markers: Vec::new(),
            preferred_tone: StyleTone::Neutral,
        };
    }

    let mut sentence_count = 0usize;
    let mut word_count = 0usize;
    let mut question_lines = 0usize;
    let mut exclamation_lines = 0usize;
    let mut greeting_markers = Vec::<String>::new();
    let mut signoff_markers = Vec::<String>::new();

    for item in items {
        let summary = item.body_summary.trim();
        if summary.is_empty() {
            continue;
        }

        for line in summary
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
        {
            sentence_count = sentence_count.saturating_add(1);
            word_count = word_count.saturating_add(line.split_whitespace().count());
            if line.contains('?') {
                question_lines = question_lines.saturating_add(1);
            }
            if line.contains('!') {
                exclamation_lines = exclamation_lines.saturating_add(1);
            }
        }

        if let Some(first_line) = summary.lines().map(str::trim).find(|line| !line.is_empty()) {
            if let Some(marker) = normalize_greeting_marker(first_line) {
                greeting_markers.push(marker);
            }
        }

        if let Some(signature) = item.signature_block.as_deref() {
            if let Some(marker) = normalize_signoff_marker(signature) {
                signoff_markers.push(marker);
            }
        }
    }

    greeting_markers.sort();
    greeting_markers.dedup();
    signoff_markers.sort();
    signoff_markers.dedup();

    let average_sentence_words_milli = if sentence_count == 0 {
        0
    } else {
        ((word_count as u32).saturating_mul(1000)) / (sentence_count as u32)
    };
    let question_rate_milli = if sentence_count == 0 {
        0
    } else {
        ((question_lines as u32).saturating_mul(1000)) / (sentence_count as u32)
    };
    let exclamation_rate_milli = if sentence_count == 0 {
        0
    } else {
        ((exclamation_lines as u32).saturating_mul(1000)) / (sentence_count as u32)
    };

    let preferred_tone = if exclamation_rate_milli >= 200 {
        StyleTone::Friendly
    } else if average_sentence_words_milli >= 16000 {
        StyleTone::Formal
    } else {
        StyleTone::Neutral
    };

    StyleProfile {
        profile_id: profile_id.to_string(),
        sample_count: items.len(),
        average_sentence_words_milli,
        question_rate_milli,
        exclamation_rate_milli,
        greeting_markers,
        signoff_markers,
        preferred_tone,
    }
}

fn normalize_greeting_marker(line: &str) -> Option<String> {
    let lowered = line.to_ascii_lowercase();
    for marker in ["hello", "hi", "hey", "good morning", "good afternoon"] {
        if lowered.starts_with(marker) {
            return Some(marker.to_string());
        }
    }
    None
}

fn normalize_signoff_marker(signature_block: &str) -> Option<String> {
    let lowered = signature_block.to_ascii_lowercase();
    for marker in ["thanks", "thank you", "best", "regards", "sincerely"] {
        if lowered.contains(marker) {
            return Some(marker.to_string());
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::{StyleTone, derive_style_profile};
    use crate::NormalizedConversationItem;

    fn item(id: &str, summary: &str, signature: Option<&str>) -> NormalizedConversationItem {
        NormalizedConversationItem {
            normalized_id: format!("gmail:{id}"),
            source_message_id: id.to_string(),
            thread_id: "thread-1".to_string(),
            subject: "Subject".to_string(),
            sender_email: "sender@example.com".to_string(),
            recipient_emails: vec!["recipient@example.com".to_string()],
            body_summary: summary.to_string(),
            quoted_blocks: Vec::new(),
            signature_block: signature.map(ToString::to_string),
            timestamp_ms: 1,
            labels: vec!["INBOX".to_string()],
        }
    }

    #[test]
    fn derive_style_profile_detects_friendly_marker_patterns() {
        let profile = derive_style_profile(
            "profile-1",
            &[
                item(
                    "m1",
                    "Hello team!\nCan you review this?",
                    Some("Thanks\nAgent"),
                ),
                item(
                    "m2",
                    "Hey all!\nSharing another update!",
                    Some("Best\nAgent"),
                ),
            ],
        );

        assert_eq!(profile.profile_id, "profile-1");
        assert_eq!(profile.sample_count, 2);
        assert_eq!(profile.preferred_tone, StyleTone::Friendly);
        assert!(profile.greeting_markers.contains(&"hello".to_string()));
        assert!(profile.signoff_markers.contains(&"thanks".to_string()));
    }

    #[test]
    fn derive_style_profile_is_deterministic_for_empty_input() {
        let profile = derive_style_profile("profile-empty", &[]);
        assert_eq!(profile.sample_count, 0);
        assert_eq!(profile.average_sentence_words_milli, 0);
        assert_eq!(profile.preferred_tone, StyleTone::Neutral);
    }
}
