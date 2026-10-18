//! Indexed sessions retain their own identity and authorization, separate from saved Conversations.
use super::*;
use openagents_account::openagents::sessions::{SessionStatus, SessionSummary};

#[derive(Clone, uniffi::Record)]
pub struct SavedIndexedRow {
    pub key: String,
    pub id: String,
    pub device_id: String,
    pub title: String,
    pub subtitle: String,
    pub project_id: Option<String>,
    pub agent_name: Option<String>,
    pub status: Option<String>,
    pub unseen: Option<bool>,
    pub online: bool,
    pub activity_revision: Option<String>,
}
#[derive(Clone, uniffi::Record)]
pub struct SavedIndexedTranscript {
    pub row: SavedIndexedRow,
    /// None means no authorized transcript copy is currently available.
    pub messages: Option<Vec<SavedMessagePart>>,
}
fn status(value: &SessionStatus) -> Option<String> {
    match value {
        SessionStatus::Idle => None,
        SessionStatus::Working => Some("Working".into()),
        SessionStatus::AwaitingInput => Some("Waiting for you".into()),
        SessionStatus::Errored => Some("Couldn't finish".into()),
        SessionStatus::Other(_) => None,
    }
}
pub(super) fn row(value: &SessionSummary) -> SavedIndexedRow {
    let device_id = match &value.home {
        openagents_account::openagents::sessions::Home::Computer(d) => d.clone(),
        _ => String::new(),
    };
    SavedIndexedRow {
        key: value.navigation_key(),
        id: value.id.clone(),
        device_id,
        title: value.title.clone(),
        subtitle: value.computer.clone(),
        project_id: value.project.clone(),
        agent_name: value.agent.clone(),
        status: status(&value.status),
        unseen: value.unseen,
        online: value.online,
        activity_revision: value.activity_revision.clone(),
    }
}
fn indexed_error(error: openagents_account::openagents::Error) -> CoreError {
    use openagents_account::openagents::Error;
    match error {
        Error::Cancelled => CoreError::Closed,
        Error::Unauthorized
        | Error::ExpiredSession
        | Error::WrongAccount
        | Error::WrongDeployment
        | Error::InvalidCredential => unavailable(),
        Error::Refused { status: 409, .. } => CoreError::InvalidArgument {
            message: "The chat changed. Open it again.".into(),
        },
        Error::NotFound | Error::Forbidden | Error::Denied => failed(error),
        _ => CoreError::HostError {
            message: "Couldn't load this chat. Try again.".into(),
        },
    }
}
impl CoreSavedHistory {
    async fn load_indexed_detail(
        &self,
        key: String,
    ) -> CoreResult<openagents_account::openagents::sessions::SessionDetail> {
        if !self.current() {
            return Err(unavailable());
        }
        let history = self.history.clone();
        let detail = openagents_client::runtime::shared()
            .spawn(async move { history.indexed_detail(&key).await })
            .await
            .map_err(failed)?
            .map_err(indexed_error)?;
        if !self.current() {
            return Err(unavailable());
        }
        Ok(detail)
    }
    pub(crate) async fn indexed_layout(
        &self,
        key: String,
    ) -> CoreResult<(SavedIndexedRow, crate::layout::TranscriptInput)> {
        let detail = self.load_indexed_detail(key).await?;
        let row = row(&detail.session);
        let messages = detail.messages.ok_or_else(|| CoreError::NotFound {
            message: "A copy of this chat isn't available.".into(),
        })?;
        let input = indexed_input(&row.key, messages);
        Ok((row, input))
    }
}
fn indexed_input(
    key: &str,
    messages: Vec<openagents_account::openagents::sessions::CopiedMessage>,
) -> crate::layout::TranscriptInput {
    use openagents_account::openagents::sessions::CopiedRole;
    use openagents_doc::{
        parts::{MessagePart, MessageStatus},
        schema::{MessageRole, SessionMessageEntry},
    };
    crate::layout::TranscriptInput {
        entries: messages
            .into_iter()
            .enumerate()
            .map(|(at, message)| {
                Arc::new(SessionMessageEntry {
                    id: format!("indexed:{key}:{at}"),
                    role: match message.role {
                        CopiedRole::User => MessageRole::User,
                        CopiedRole::Assistant => MessageRole::Assistant,
                        CopiedRole::Tool => MessageRole::System,
                    },
                    parts: vec![MessagePart::Text {
                        id: "text".into(),
                        text: message.text,
                    }],
                    created_at: 0,
                    device_id: String::new(),
                    status: Some(MessageStatus::Complete),
                    continuation_of: None,
                    duration_ms: None,
                })
            })
            .collect(),
        ..Default::default()
    }
}

#[uniffi::export]
impl CoreSavedHistory {
    /// Load an existing indexed session in this history's current account/project scope.
    pub async fn indexed_session(&self, key: String) -> CoreResult<SavedIndexedTranscript> {
        let detail = self.load_indexed_detail(key).await?;
        let messages = detail.messages.map(|messages| {
            messages
                .into_iter()
                .map(|m| SavedMessagePart {
                    role: match m.role {
                        openagents_account::openagents::sessions::CopiedRole::User => "You",
                        openagents_account::openagents::sessions::CopiedRole::Assistant => {
                            "Assistant"
                        }
                        openagents_account::openagents::sessions::CopiedRole::Tool => "Tool",
                    }
                    .into(),
                    text: m.text,
                    part: 0,
                    parts: 1,
                    files: vec![],
                })
                .collect()
        });
        Ok(SavedIndexedTranscript {
            row: row(&detail.session),
            messages,
        })
    }
    /// Call only after the indexed transcript mounted successfully, using its exact revision.
    pub async fn mark_indexed_seen(
        &self,
        key: String,
        activity_revision: String,
    ) -> CoreResult<()> {
        if !self.current() {
            return Err(unavailable());
        }
        let history = self.history.clone();
        openagents_client::runtime::shared()
            .spawn(async move { history.mark_indexed_seen(&key, &activity_revision).await })
            .await
            .map_err(failed)?
            .map_err(indexed_error)?;
        if !self.current() {
            return Err(unavailable());
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn indexed_metadata_comes_from_the_authorized_row_without_legacy_id_inference() {
        let value:SessionSummary=serde_json::from_value(serde_json::json!({"id":"00000000-0000-0000-0000-000000000001","home":"computer:device-a","title":"Example","computer":"Synthetic","status":"working","last_activity_unix":10,"agent":"Custom agent","project":"prj_shared","unseen":true})).unwrap();
        let projected = row(&value);
        assert_eq!(projected.agent_name.as_deref(), Some("Custom agent"));
        assert_eq!(projected.project_id.as_deref(), Some("prj_shared"));
        assert_eq!(projected.status.as_deref(), Some("Working"));
        assert_eq!(projected.unseen, Some(true));
        let mut old = value;
        old.unseen = None;
        assert_eq!(row(&old).unseen, None);
    }
    #[test]
    fn indexed_failures_preserve_cancellation_and_activity_conflict() {
        use openagents_account::openagents::Error;
        assert!(matches!(indexed_error(Error::Cancelled), CoreError::Closed));
        assert!(matches!(
            indexed_error(Error::Unauthorized),
            CoreError::Auth { .. }
        ));
        assert!(matches!(
            indexed_error(Error::Refused {
                status: 409,
                code: Some("activity_changed")
            }),
            CoreError::InvalidArgument { .. }
        ));
    }
    #[test]
    fn indexed_shared_member_keys_do_not_alias_the_same_session_uuid() {
        let value = serde_json::json!({"id":"00000000-0000-0000-0000-000000000001","home":"computer:device-a","title":"Example","computer":"Synthetic","status":"idle","last_activity_unix":10,"record_key":"a".repeat(64)});
        let a: SessionSummary = serde_json::from_value(value.clone()).unwrap();
        let mut b = value;
        b["record_key"] = serde_json::json!("b".repeat(64));
        let b: SessionSummary = serde_json::from_value(b).unwrap();
        assert_eq!(a.id, b.id);
        assert_ne!(row(&a).key, row(&b).key);
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use openagents_account::openagents::sessions::{CopiedMessage, CopiedRole};
    #[test]
    fn indexed_copies_keep_their_identity_roles_and_rust_layout() {
        let input = indexed_input(
            "session:synthetic",
            vec![
                CopiedMessage {
                    role: CopiedRole::User,
                    text: "question".into(),
                },
                CopiedMessage {
                    role: CopiedRole::Assistant,
                    text: "answer".into(),
                },
                CopiedMessage {
                    role: CopiedRole::Tool,
                    text: "tool".into(),
                },
            ],
        );
        assert_eq!(input.entries.len(), 3);
        assert_eq!(input.entries[0].id, "indexed:session:synthetic:0");
        assert!(matches!(
            input.entries[0].role,
            openagents_doc::schema::MessageRole::User
        ));
        assert!(matches!(
            input.entries[1].role,
            openagents_doc::schema::MessageRole::Assistant
        ));
        assert!(matches!(
            input.entries[2].role,
            openagents_doc::schema::MessageRole::System
        ));
        assert!(!input.working && !input.streaming);
        assert_ne!(
            input.entries[0].id,
            indexed_input(
                "other",
                vec![CopiedMessage {
                    role: CopiedRole::User,
                    text: "question".into(),
                }]
            )
            .entries[0]
                .id
        );
    }
}
