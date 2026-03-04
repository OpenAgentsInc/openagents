use std::collections::BTreeSet;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailMessageHeader {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailMessageBody {
    pub mime_type: String,
    pub data: String,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailMessagePayload {
    pub headers: Vec<GmailMessageHeader>,
    pub body: GmailMessageBody,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailMessageMetadata {
    pub internal_date_ms: u64,
    pub label_ids: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailThreadParticipant {
    pub email: String,
    pub display_name: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailMessage {
    pub id: String,
    pub thread_id: String,
    pub payload: GmailMessagePayload,
    pub participants: Vec<GmailThreadParticipant>,
    pub metadata: GmailMessageMetadata,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailBackfillPage {
    pub message_ids: Vec<String>,
    pub next_page_token: Option<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailBackfillCheckpoint {
    pub next_page_token: Option<String>,
    pub imported_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailBackfillConfig {
    pub page_size: usize,
    pub max_pages: usize,
}

impl Default for GmailBackfillConfig {
    fn default() -> Self {
        Self {
            page_size: 50,
            max_pages: 500,
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailBackfillResult {
    pub imported_messages: Vec<GmailMessage>,
    pub final_checkpoint: GmailBackfillCheckpoint,
    pub page_checkpoints: Vec<GmailBackfillCheckpoint>,
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum GmailConnectorError {
    #[error("provider error: {0}")]
    Provider(String),
    #[error("invalid backfill config: {0}")]
    InvalidConfig(String),
}

pub trait GmailMailboxProvider {
    fn list_messages(
        &self,
        page_token: Option<&str>,
        page_size: usize,
    ) -> Result<GmailBackfillPage, GmailConnectorError>;

    fn get_message(&self, message_id: &str) -> Result<GmailMessage, GmailConnectorError>;
}

pub fn run_gmail_backfill(
    provider: &dyn GmailMailboxProvider,
    start_checkpoint: Option<&GmailBackfillCheckpoint>,
    config: &GmailBackfillConfig,
) -> Result<GmailBackfillResult, GmailConnectorError> {
    if config.page_size == 0 {
        return Err(GmailConnectorError::InvalidConfig(
            "page_size must be greater than zero".to_string(),
        ));
    }
    if config.max_pages == 0 {
        return Err(GmailConnectorError::InvalidConfig(
            "max_pages must be greater than zero".to_string(),
        ));
    }

    let mut imported_messages = Vec::<GmailMessage>::new();
    let mut seen_ids = BTreeSet::<String>::new();
    let mut page_checkpoints = Vec::<GmailBackfillCheckpoint>::new();

    let mut imported_count = start_checkpoint.map_or(0, |checkpoint| checkpoint.imported_count);
    let mut next_page_token =
        start_checkpoint.and_then(|checkpoint| checkpoint.next_page_token.clone());

    for _ in 0..config.max_pages {
        let page = provider.list_messages(next_page_token.as_deref(), config.page_size)?;
        for message_id in page.message_ids {
            if !seen_ids.insert(message_id.clone()) {
                continue;
            }
            let message = provider.get_message(message_id.as_str())?;
            imported_messages.push(message);
            imported_count = imported_count.saturating_add(1);
        }

        next_page_token = page.next_page_token;
        let checkpoint = GmailBackfillCheckpoint {
            next_page_token: next_page_token.clone(),
            imported_count,
        };
        page_checkpoints.push(checkpoint);

        if next_page_token.is_none() {
            break;
        }
    }

    Ok(GmailBackfillResult {
        imported_messages,
        final_checkpoint: GmailBackfillCheckpoint {
            next_page_token,
            imported_count,
        },
        page_checkpoints,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GmailBackfillCheckpoint, GmailBackfillConfig, GmailBackfillPage, GmailConnectorError,
        GmailMailboxProvider, GmailMessage, GmailMessageBody, GmailMessageHeader,
        GmailMessageMetadata, GmailMessagePayload, GmailThreadParticipant, run_gmail_backfill,
    };
    use std::collections::BTreeMap;

    struct MockMailboxProvider {
        first_page: Option<GmailBackfillPage>,
        pages_by_token: BTreeMap<String, GmailBackfillPage>,
        messages: BTreeMap<String, GmailMessage>,
    }

    impl MockMailboxProvider {
        fn new(
            first_page: Option<GmailBackfillPage>,
            pages_by_token: BTreeMap<String, GmailBackfillPage>,
            messages: Vec<GmailMessage>,
        ) -> Self {
            let mut map = BTreeMap::new();
            for message in messages {
                map.insert(message.id.clone(), message);
            }
            Self {
                first_page,
                pages_by_token,
                messages: map,
            }
        }
    }

    impl GmailMailboxProvider for MockMailboxProvider {
        fn list_messages(
            &self,
            page_token: Option<&str>,
            _page_size: usize,
        ) -> Result<GmailBackfillPage, GmailConnectorError> {
            if let Some(page_token) = page_token {
                return self.pages_by_token.get(page_token).cloned().ok_or_else(|| {
                    GmailConnectorError::Provider(format!(
                        "no page available for token {page_token}"
                    ))
                });
            }
            self.first_page.clone().ok_or_else(|| {
                GmailConnectorError::Provider(
                    "no initial page available for test provider".to_string(),
                )
            })
        }

        fn get_message(&self, message_id: &str) -> Result<GmailMessage, GmailConnectorError> {
            self.messages.get(message_id).cloned().ok_or_else(|| {
                GmailConnectorError::Provider(format!("missing message {message_id}"))
            })
        }
    }

    fn sample_message(id: &str) -> GmailMessage {
        GmailMessage {
            id: id.to_string(),
            thread_id: "thread-1".to_string(),
            payload: GmailMessagePayload {
                headers: vec![GmailMessageHeader {
                    name: "Subject".to_string(),
                    value: format!("Subject {id}"),
                }],
                body: GmailMessageBody {
                    mime_type: "text/plain".to_string(),
                    data: format!("Body for {id}"),
                },
            },
            participants: vec![GmailThreadParticipant {
                email: "sender@example.com".to_string(),
                display_name: Some("Sender".to_string()),
            }],
            metadata: GmailMessageMetadata {
                internal_date_ms: 1000,
                label_ids: vec!["INBOX".to_string()],
            },
        }
    }

    #[test]
    fn backfill_imports_messages_and_stops_on_terminal_page() {
        let provider = MockMailboxProvider::new(
            Some(GmailBackfillPage {
                message_ids: vec!["m1".to_string(), "m2".to_string()],
                next_page_token: None,
            }),
            BTreeMap::new(),
            vec![sample_message("m1"), sample_message("m2")],
        );

        let result = run_gmail_backfill(&provider, None, &GmailBackfillConfig::default())
            .expect("backfill should succeed");

        assert_eq!(result.imported_messages.len(), 2);
        assert_eq!(result.final_checkpoint.imported_count, 2);
        assert_eq!(result.final_checkpoint.next_page_token, None);
        assert_eq!(result.page_checkpoints.len(), 1);
    }

    #[test]
    fn backfill_respects_initial_checkpoint_counter() {
        let mut pages_by_token = BTreeMap::new();
        pages_by_token.insert(
            "resume-token".to_string(),
            GmailBackfillPage {
                message_ids: vec!["m3".to_string()],
                next_page_token: None,
            },
        );
        let provider = MockMailboxProvider::new(None, pages_by_token, vec![sample_message("m3")]);

        let start = GmailBackfillCheckpoint {
            next_page_token: Some("resume-token".to_string()),
            imported_count: 7,
        };
        let result = run_gmail_backfill(
            &provider,
            Some(&start),
            &GmailBackfillConfig {
                page_size: 10,
                max_pages: 1,
            },
        )
        .expect("resumed backfill should succeed");

        assert_eq!(result.final_checkpoint.imported_count, 8);
        assert_eq!(result.imported_messages.len(), 1);
    }

    #[test]
    fn backfill_rejects_zero_page_size() {
        let provider = MockMailboxProvider::new(None, BTreeMap::new(), vec![]);
        let error = run_gmail_backfill(
            &provider,
            None,
            &GmailBackfillConfig {
                page_size: 0,
                max_pages: 1,
            },
        )
        .expect_err("zero page_size must fail");
        assert_eq!(
            error,
            GmailConnectorError::InvalidConfig("page_size must be greater than zero".to_string())
        );
    }
}
