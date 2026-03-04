use crate::{GmailMessage, GmailMessageHeader};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NormalizationConfig {
    pub summary_max_chars: usize,
}

impl Default for NormalizationConfig {
    fn default() -> Self {
        Self {
            summary_max_chars: 220,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct NormalizedConversationItem {
    pub normalized_id: String,
    pub source_message_id: String,
    pub thread_id: String,
    pub subject: String,
    pub sender_email: String,
    pub recipient_emails: Vec<String>,
    pub body_summary: String,
    pub quoted_blocks: Vec<String>,
    pub signature_block: Option<String>,
    pub timestamp_ms: u64,
    pub labels: Vec<String>,
}

pub fn normalize_gmail_message(
    message: &GmailMessage,
    config: &NormalizationConfig,
) -> NormalizedConversationItem {
    let subject = header_value(message.payload.headers.as_slice(), "subject").unwrap_or_default();
    let sender_email = header_value(message.payload.headers.as_slice(), "from").unwrap_or_default();
    let recipient_emails = header_value(message.payload.headers.as_slice(), "to")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|entry| !entry.is_empty())
                .map(ToString::to_string)
                .collect::<Vec<String>>()
        })
        .unwrap_or_default();

    let body = message.payload.body.data.trim();
    let (primary_body, quoted_blocks, signature_block) = split_body_sections(body);
    let body_summary = truncate_primary_body(primary_body.as_str(), config.summary_max_chars);

    NormalizedConversationItem {
        normalized_id: format!("gmail:{}", message.id),
        source_message_id: message.id.clone(),
        thread_id: message.thread_id.clone(),
        subject,
        sender_email,
        recipient_emails,
        body_summary,
        quoted_blocks,
        signature_block,
        timestamp_ms: message.metadata.internal_date_ms,
        labels: message.metadata.label_ids.clone(),
    }
}

fn header_value(headers: &[GmailMessageHeader], key: &str) -> Option<String> {
    headers
        .iter()
        .find(|header| header.name.eq_ignore_ascii_case(key))
        .map(|header| header.value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn split_body_sections(body: &str) -> (String, Vec<String>, Option<String>) {
    let signature_marker = "\n-- \n";
    let (without_signature, signature_block) = if let Some(index) = body.find(signature_marker) {
        (
            body[..index].trim_end().to_string(),
            Some(body[index + signature_marker.len()..].trim().to_string()),
        )
    } else {
        (body.to_string(), None)
    };

    let mut primary = String::new();
    let mut quoted_blocks = Vec::<String>::new();
    let mut current_quote = Vec::<String>::new();

    for line in without_signature.lines() {
        if line.trim_start().starts_with('>') {
            current_quote.push(line.trim_start_matches('>').trim().to_string());
            continue;
        }
        if !current_quote.is_empty() {
            quoted_blocks.push(current_quote.join("\n").trim().to_string());
            current_quote.clear();
        }
        if !line.trim().is_empty() {
            if !primary.is_empty() {
                primary.push('\n');
            }
            primary.push_str(line.trim_end());
        }
    }

    if !current_quote.is_empty() {
        quoted_blocks.push(current_quote.join("\n").trim().to_string());
    }

    let quoted_blocks = quoted_blocks
        .into_iter()
        .filter(|block| !block.is_empty())
        .collect();

    (primary.trim().to_string(), quoted_blocks, signature_block)
}

fn truncate_primary_body(body: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if body.chars().count() <= max_chars {
        return body.to_string();
    }

    let mut truncated = body.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::{NormalizationConfig, normalize_gmail_message};
    use crate::{
        GmailMessage, GmailMessageBody, GmailMessageHeader, GmailMessageMetadata,
        GmailMessagePayload, GmailThreadParticipant,
    };

    fn message_with_body(body: &str) -> GmailMessage {
        GmailMessage {
            id: "msg-1".to_string(),
            thread_id: "thread-9".to_string(),
            payload: GmailMessagePayload {
                headers: vec![
                    GmailMessageHeader {
                        name: "Subject".to_string(),
                        value: "Status update".to_string(),
                    },
                    GmailMessageHeader {
                        name: "From".to_string(),
                        value: "sender@example.com".to_string(),
                    },
                    GmailMessageHeader {
                        name: "To".to_string(),
                        value: "a@example.com, b@example.com".to_string(),
                    },
                ],
                body: GmailMessageBody {
                    mime_type: "text/plain".to_string(),
                    data: body.to_string(),
                },
            },
            participants: vec![GmailThreadParticipant {
                email: "sender@example.com".to_string(),
                display_name: Some("Sender".to_string()),
            }],
            metadata: GmailMessageMetadata {
                internal_date_ms: 1_735_689_600_000,
                label_ids: vec!["INBOX".to_string(), "IMPORTANT".to_string()],
            },
        }
    }

    #[test]
    fn normalization_extracts_primary_quote_and_signature_sections() {
        let body = "Hello team\n\nThanks for the update\n> prior context line 1\n> prior context line 2\n\n-- \nBest\nAgent";
        let normalized =
            normalize_gmail_message(&message_with_body(body), &NormalizationConfig::default());

        assert_eq!(normalized.normalized_id, "gmail:msg-1");
        assert_eq!(normalized.body_summary, "Hello team\nThanks for the update");
        assert_eq!(normalized.quoted_blocks.len(), 1);
        assert!(
            normalized
                .signature_block
                .as_deref()
                .is_some_and(|signature| signature.contains("Best"))
        );
        assert_eq!(normalized.recipient_emails.len(), 2);
    }

    #[test]
    fn normalization_truncates_summary_deterministically() {
        let body = "abcdefghijklmnopqrstuvwxyz";
        let normalized = normalize_gmail_message(
            &message_with_body(body),
            &NormalizationConfig {
                summary_max_chars: 8,
            },
        );
        assert_eq!(normalized.body_summary, "abcdefgh...");
    }
}
