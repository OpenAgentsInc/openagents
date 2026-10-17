//! Continuation context belongs to the session document, never a registry summary.
use crate::{DocError, SessionDoc};
use chat_home::Continued;
use loro::{LoroValue, ValueOrContainer};
use serde::{Deserialize, Serialize};

pub fn pending(doc: &SessionDoc) -> Option<chat_home::Source> {
    let ValueOrContainer::Value(LoroValue::String(value)) = doc
        .doc()
        .get_map("meta")
        .get("openagentsContinuationPending")?
    else {
        return None;
    };
    serde_json::from_str(&value).ok()
}
pub fn set_pending(doc: &SessionDoc, source: &chat_home::Source) -> Result<(), DocError> {
    doc.doc().get_map("meta").insert(
        "openagentsContinuationPending",
        serde_json::to_string(source)?,
    )?;
    doc.doc().commit();
    Ok(())
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Stored {
    pub continued: Continued,
    pub backlink: String,
}
impl Stored {
    pub fn prompt(
        &self,
        doc: &SessionDoc,
        current: Option<&str>,
        prompt: &str,
        include_local: bool,
    ) -> Option<String> {
        if !include_local {
            return self.continued.seed.prompt(prompt);
        }
        let entries = doc.read_entries().ok()?;
        let mut recent = entries
            .iter()
            .rev()
            .filter(|e| Some(e.id.as_str()) != current)
            .filter_map(|e| {
                let role = match e.role {
                    crate::MessageRole::User => chat_home::Role::User,
                    crate::MessageRole::Assistant => chat_home::Role::Assistant,
                    _ => return None,
                };
                let mut text = String::new();
                for part in &e.parts {
                    if let crate::MessagePart::Text { text: value, .. } = part {
                        for c in value.chars() {
                            if text.len() + c.len_utf8() > 1024 {
                                break;
                            }
                            text.push(c);
                        }
                    }
                    if text.len() >= 1021 {
                        break;
                    }
                }
                Some((role, text))
            })
            .take(chat_home::MAX_RECENT)
            .collect::<Vec<_>>();
        recent.reverse();
        self.continued
            .seed
            .with_recent(recent.iter().map(|(r, t)| (r.clone(), t.as_str())))
            .prompt(prompt)
    }
}
pub fn get(doc: &SessionDoc) -> Option<Stored> {
    let ValueOrContainer::Value(LoroValue::String(value)) =
        doc.doc().get_map("meta").get(chat_home::DOCUMENT_KEY)?
    else {
        return None;
    };
    let value: Stored = serde_json::from_str(&value).ok()?;
    (value.continued.source.valid()
        && value.continued.target.valid()
        && value.continued.seed.valid())
    .then_some(value)
}
pub fn set(doc: &SessionDoc, value: &Stored) -> Result<(), DocError> {
    if !value.continued.seed.valid()
        || !value.continued.source.valid()
        || !value.continued.target.valid()
    {
        return Err(DocError::Schema("Invalid continuation context".into()));
    }
    if let Some(previous) = get(doc) {
        return if previous.continued == value.continued {
            Ok(())
        } else {
            Err(DocError::Schema(
                "The conversation already has a different source".into(),
            ))
        };
    }
    doc.doc()
        .get_map("meta")
        .insert(chat_home::DOCUMENT_KEY, serde_json::to_string(value)?)?;
    doc.doc().commit();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MessagePart, MessageRole, SessionMessageEntry};
    #[test]
    fn provider_restarts_get_bounded_context_and_source_cannot_be_replaced() {
        let source = "d75ef8a1-16ca-49b6-83be-000000000001";
        let target = "d75ef8a1-16ca-49b6-83be-000000000002";
        let doc = SessionDoc::init(target).unwrap();
        let stored = Stored {
            continued: chat_home::Continued {
                account: "fixture-account".into(),
                source: chat_home::PartRef {
                    id: source.into(),
                    home: chat_home::Home::Cloud,
                },
                target: chat_home::PartRef {
                    id: target.into(),
                    home: chat_home::Home::Computer("fixture-mac".into()),
                },
                seed: chat_home::Seed::new(
                    "Earlier conversation",
                    [(chat_home::Role::User, "Earlier flower: violet rose")],
                ),
            },
            backlink: format!("https://example.invalid/chat/{source}"),
        };
        set(&doc, &stored).unwrap();
        for i in 0..500 {
            doc.push_message(&SessionMessageEntry {
                id: format!("local-{i}"),
                role: MessageRole::User,
                parts: vec![MessagePart::Text {
                    id: "text".into(),
                    text: format!("Local request {i}"),
                }],
                created_at: i,
                device_id: "fixture-mac".into(),
                status: None,
                continuation_of: None,
                duration_ms: None,
            })
            .unwrap();
        }
        let prompt = get(&doc)
            .unwrap()
            .prompt(&doc, Some("local-499"), "Current question", true)
            .unwrap();
        assert!(prompt.len() < chat_home::MAX_SEED_BYTES + 1024);
        assert!(
            prompt.contains("violet rose")
                && prompt.contains("Local request 498")
                && !prompt.contains("Local request 0")
        );
        assert!(!prompt.contains("Local request 499"));
        let mut changed = stored.clone();
        changed.continued.source.id = target.into();
        assert!(set(&doc, &changed).is_err());
        assert_eq!(get(&doc).unwrap().continued, stored.continued);
    }
}
