//! Saved account messages adapted for the desktop and phone transcript renderers.
use crate::openagents::{FileKind, MessageRole, SavedFile, SavedMessage, ToolStatus};
use zeron_doc::{MessagePart, MessageStatus, SessionMessageEntry};
use zeron_proto::ToolCall;

/// Adapt content without importing executable harness state or following embedded URLs.
pub fn entries(messages: &[SavedMessage], generation: u64) -> Vec<SessionMessageEntry> {
    messages
        .iter()
        .map(|message| {
            let mut id = format!("account-{generation}-{}", message.id);
            let mut parts = Vec::new();
            if !message.text.is_empty()
                && !message
                    .tool
                    .as_ref()
                    .is_some_and(|t| t.output == message.text)
            {
                parts.push(if message.role == MessageRole::Thinking {
                    MessagePart::Reasoning {
                        id: format!("{id}-thinking"),
                        text: message.text.clone(),
                    }
                } else {
                    MessagePart::Text {
                        id: format!("{id}-text"),
                        text: message.text.clone(),
                    }
                });
            }
            if let Some(tool) = &message.tool {
                parts.push(MessagePart::Tool {
                    id: format!("{id}-tool"),
                    call: ToolCall::Unknown {
                        name: tool.name.clone(),
                        input: Some(
                            serde_json::from_str(&tool.input)
                                .unwrap_or_else(|_| serde_json::Value::String(tool.input.clone())),
                        ),
                    },
                    is_error: tool.status == ToolStatus::Failed,
                    // Saved activity must never imply a locally running operation.
                    resolved: true,
                    output: Some(if tool.summary.is_empty() {
                        tool.output.clone()
                    } else {
                        format!("{}\n{}", tool.summary, tool.output)
                    }),
                    diff: None,
                    output_ref: None,
                    output_bytes: None,
                    diff_ref: None,
                    diff_stats: None,
                    subagent_ref: None,
                    subagent_status: None,
                    subagent_tail: None,
                });
            }
            for file in &message.files {
                let mime = match file.kind {
                    FileKind::Png => Some("image/png"),
                    FileKind::Jpeg => Some("image/jpeg"),
                    FileKind::Gif => Some("image/gif"),
                    FileKind::Webp => Some("image/webp"),
                    _ => None,
                };
                if let Some(mime) = mime {
                    parts.push(MessagePart::Image {
                        id: format!("{id}-file-{}", file.id),
                        path: file_path(generation, &message.id, file),
                        name: file.name.clone(),
                        mime_type: mime.into(),
                    });
                } else {
                    parts.push(MessagePart::Text {
                        id: format!("{id}-file-{}", file.id),
                        text: format!("Attached file: {}", file.name),
                    });
                }
            }
            if message.role == MessageRole::User && !message.files.is_empty() {
                // Both session renderers read user attachments from the text trailer.
                // These references resolve only through this account's saved-file API.
                let paths = message
                    .files
                    .iter()
                    .map(|file| format!("- {}", file_path(generation, &message.id, file)))
                    .collect::<Vec<_>>()
                    .join("\n");
                parts = vec![MessagePart::Text {
                    id: format!("{id}-text"),
                    text: format!(
                        "{}\n\nAttached images (local files — open them to view):\n{paths}",
                        message.text
                    ),
                }];
            }
            // The session renderer normally sees append-only message parts. Saved
            // revisions can edit equal-length text, so include content in part IDs.
            use sha2::{Digest, Sha256};
            for part in &mut parts {
                let fingerprint = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(part).unwrap_or_default())
                );
                let part_id = match part {
                    MessagePart::Text { id, .. }
                    | MessagePart::Reasoning { id, .. }
                    | MessagePart::Tool { id, .. }
                    | MessagePart::Image { id, .. } => id,
                    _ => continue,
                };
                part_id.push_str(&fingerprint[..16]);
            }
            if message.role == MessageRole::User {
                let fingerprint = format!(
                    "{:x}",
                    Sha256::digest(serde_json::to_vec(&parts).unwrap_or_default())
                );
                id.push_str(&fingerprint[..16]);
            }
            SessionMessageEntry {
                id,
                role: if message.role == MessageRole::User {
                    zeron_doc::MessageRole::User
                } else {
                    zeron_doc::MessageRole::Assistant
                },
                parts,
                created_at: 0,
                device_id: String::new(),
                status: Some(MessageStatus::Complete),
                continuation_of: None,
                duration_ms: None,
            }
        })
        .collect()
}

/// Use the saved image kind for thumbnails, even when the filename has no extension.
pub fn file_path(generation: u64, message: &str, file: &SavedFile) -> String {
    let name = match file.kind {
        FileKind::Png | FileKind::Jpeg | FileKind::Gif | FileKind::Webp => {
            format!("image.{}", file.kind.as_str())
        }
        _ => file.name.clone(),
    };
    attachment_path(generation, message, &file.id, &name)
}

/// An opaque display reference, never a server-provided URL or a local file path.
pub fn attachment_path(generation: u64, message: &str, file: &str, name: &str) -> String {
    use sha2::{Digest, Sha256};
    let key = format!("{generation}:{message}:{file}");
    let hash = format!("{:x}", Sha256::digest(key.as_bytes()));
    let extension = name
        .rsplit_once('.')
        .map(|(_, ext)| ext)
        .filter(|ext| ext.len() <= 8 && ext.chars().all(|c| c.is_ascii_alphanumeric()))
        .unwrap_or("file");
    let display_name: String = name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("attachment")
        .chars()
        .filter(|c| c.is_alphanumeric() || matches!(c, '.' | '_' | '-' | ' '))
        .take(96)
        .collect();
    let display_name = if display_name.is_empty() || display_name == "." || display_name == ".." {
        format!("attachment.{extension}")
    } else {
        display_name
    };
    format!("saved-attachment/{hash}/{display_name}")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn message(role: MessageRole, text: &str) -> SavedMessage {
        SavedMessage {
            id: "message".into(),
            role,
            text: text.into(),
            files: vec![],
            tool: None,
            extra: Default::default(),
        }
    }
    #[test]
    fn roles_and_rich_content_use_session_parts_without_execution() {
        let source = vec![
            message(MessageRole::User, "question"),
            message(
                MessageRole::Assistant,
                "**answer**\n```rust\nfn main() {}\n```",
            ),
            message(MessageRole::Thinking, "considering"),
        ];
        let result = entries(&source, 7);
        assert_eq!(result[0].role, zeron_doc::MessageRole::User);
        assert!(
            matches!(&result[1].parts[0], MessagePart::Text { text, .. } if text == &source[1].text)
        );
        assert!(
            matches!(&result[2].parts[0], MessagePart::Reasoning { text, .. } if text == "considering")
        );
        assert!(
            result
                .iter()
                .all(|entry| entry.status == Some(MessageStatus::Complete))
        );
        assert_ne!(result[0].id, entries(&source, 8)[0].id);
    }
    #[test]
    fn equal_length_revision_edits_invalidate_renderer_caches() {
        let first = entries(&[message(MessageRole::Assistant, "before")], 1);
        let second = entries(&[message(MessageRole::Assistant, "edited")], 1);
        assert_eq!(first[0].id, second[0].id);
        assert_ne!(first[0].parts[0].id(), second[0].parts[0].id());
    }
    #[test]
    fn attachment_names_cannot_become_paths_or_urls() {
        let path = attachment_path(1, "../message", "https://example.com", "../../image.png");
        assert!(path.starts_with("saved-attachment/"));
        assert!(path.ends_with(".png"));
        assert!(!path.contains(".."));
        assert_ne!(
            path,
            attachment_path(2, "../message", "https://example.com", "../../image.png")
        );
    }
}
