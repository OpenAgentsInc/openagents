use std::collections::BTreeSet;

#[derive(Debug, Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
pub enum GmailDeltaOperation {
    Create,
    Update,
    Delete,
}

impl GmailDeltaOperation {
    const fn label(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub struct GmailDeltaItem {
    pub message_id: String,
    pub operation: GmailDeltaOperation,
    pub history_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailSyncCursor {
    pub history_id: u64,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailSyncBatch {
    pub next_history_id: u64,
    pub deltas: Vec<GmailDeltaItem>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailSyncOutcome {
    pub rebootstrap_required: bool,
    pub reason: Option<String>,
    pub applied_deltas: Vec<GmailDeltaItem>,
    pub next_cursor: Option<GmailSyncCursor>,
    pub duplicate_drop_count: usize,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct GmailSyncState {
    pub cursor: Option<GmailSyncCursor>,
    pub known_message_ids: BTreeSet<String>,
    pub processed_delta_keys: BTreeSet<String>,
    pub duplicate_drop_count: usize,
    pub rebootstrap_count: usize,
}

impl Default for GmailSyncState {
    fn default() -> Self {
        Self {
            cursor: None,
            known_message_ids: BTreeSet::new(),
            processed_delta_keys: BTreeSet::new(),
            duplicate_drop_count: 0,
            rebootstrap_count: 0,
        }
    }
}

#[derive(Debug, thiserror::Error, Eq, PartialEq)]
pub enum GmailSyncError {
    #[error("sync provider error: {0}")]
    Provider(String),
    #[error("invalid sync config: {0}")]
    InvalidConfig(String),
}

pub trait GmailHistoryProvider {
    fn fetch_history_since(
        &self,
        since_history_id: Option<u64>,
        max_results: usize,
    ) -> Result<GmailSyncBatch, GmailSyncError>;
}

pub fn apply_gmail_incremental_sync(
    state: &mut GmailSyncState,
    provider: &dyn GmailHistoryProvider,
    max_results: usize,
) -> Result<GmailSyncOutcome, GmailSyncError> {
    if max_results == 0 {
        return Err(GmailSyncError::InvalidConfig(
            "max_results must be greater than zero".to_string(),
        ));
    }

    let prior_cursor = state.cursor.clone();
    let batch = provider.fetch_history_since(
        prior_cursor.as_ref().map(|cursor| cursor.history_id),
        max_results,
    )?;

    if let Some(prior_cursor) = prior_cursor.as_ref() {
        if batch.next_history_id < prior_cursor.history_id {
            state.rebootstrap_count = state.rebootstrap_count.saturating_add(1);
            return Ok(GmailSyncOutcome {
                rebootstrap_required: true,
                reason: Some(format!(
                    "history cursor moved backwards (prior={}, next={})",
                    prior_cursor.history_id, batch.next_history_id
                )),
                applied_deltas: Vec::new(),
                next_cursor: None,
                duplicate_drop_count: 0,
            });
        }
    }

    let mut applied_deltas = Vec::<GmailDeltaItem>::new();
    let mut duplicate_drop_count = 0usize;

    for delta in batch.deltas {
        let delta_key = format!(
            "{}:{}:{}",
            delta.history_id,
            delta.operation.label(),
            delta.message_id
        );
        if !state.processed_delta_keys.insert(delta_key) {
            duplicate_drop_count = duplicate_drop_count.saturating_add(1);
            continue;
        }

        match delta.operation {
            GmailDeltaOperation::Create | GmailDeltaOperation::Update => {
                state.known_message_ids.insert(delta.message_id.clone());
            }
            GmailDeltaOperation::Delete => {
                state.known_message_ids.remove(delta.message_id.as_str());
            }
        }
        applied_deltas.push(delta);
    }

    state.duplicate_drop_count = state
        .duplicate_drop_count
        .saturating_add(duplicate_drop_count);
    state.cursor = Some(GmailSyncCursor {
        history_id: batch.next_history_id,
    });

    Ok(GmailSyncOutcome {
        rebootstrap_required: false,
        reason: None,
        applied_deltas,
        next_cursor: state.cursor.clone(),
        duplicate_drop_count,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        GmailDeltaItem, GmailDeltaOperation, GmailHistoryProvider, GmailSyncBatch, GmailSyncCursor,
        GmailSyncError, GmailSyncState, apply_gmail_incremental_sync,
    };
    use std::collections::VecDeque;
    use std::sync::Mutex;

    struct MockHistoryProvider {
        batches: Mutex<VecDeque<GmailSyncBatch>>,
    }

    impl MockHistoryProvider {
        fn new(batches: Vec<GmailSyncBatch>) -> Self {
            Self {
                batches: Mutex::new(batches.into()),
            }
        }
    }

    impl GmailHistoryProvider for MockHistoryProvider {
        fn fetch_history_since(
            &self,
            _since_history_id: Option<u64>,
            _max_results: usize,
        ) -> Result<GmailSyncBatch, GmailSyncError> {
            let mut batches = self
                .batches
                .lock()
                .map_err(|_| GmailSyncError::Provider("batch lock poisoned".to_string()))?;
            batches
                .pop_front()
                .ok_or_else(|| GmailSyncError::Provider("no sync batch available".to_string()))
        }
    }

    #[test]
    fn incremental_sync_applies_deltas_and_updates_cursor() {
        let provider = MockHistoryProvider::new(vec![GmailSyncBatch {
            next_history_id: 11,
            deltas: vec![
                GmailDeltaItem {
                    message_id: "m1".to_string(),
                    operation: GmailDeltaOperation::Create,
                    history_id: 10,
                },
                GmailDeltaItem {
                    message_id: "m1".to_string(),
                    operation: GmailDeltaOperation::Update,
                    history_id: 11,
                },
            ],
        }]);
        let mut state = GmailSyncState::default();

        let outcome =
            apply_gmail_incremental_sync(&mut state, &provider, 100).expect("sync should work");

        assert!(!outcome.rebootstrap_required);
        assert_eq!(outcome.applied_deltas.len(), 2);
        assert_eq!(
            outcome.next_cursor,
            Some(GmailSyncCursor { history_id: 11 })
        );
        assert!(state.known_message_ids.contains("m1"));
    }

    #[test]
    fn incremental_sync_drops_duplicate_deltas() {
        let provider = MockHistoryProvider::new(vec![GmailSyncBatch {
            next_history_id: 20,
            deltas: vec![
                GmailDeltaItem {
                    message_id: "m2".to_string(),
                    operation: GmailDeltaOperation::Create,
                    history_id: 20,
                },
                GmailDeltaItem {
                    message_id: "m2".to_string(),
                    operation: GmailDeltaOperation::Create,
                    history_id: 20,
                },
            ],
        }]);
        let mut state = GmailSyncState::default();

        let outcome =
            apply_gmail_incremental_sync(&mut state, &provider, 100).expect("sync should work");

        assert_eq!(outcome.applied_deltas.len(), 1);
        assert_eq!(outcome.duplicate_drop_count, 1);
        assert_eq!(state.duplicate_drop_count, 1);
    }

    #[test]
    fn incremental_sync_flags_rebootstrap_when_history_moves_backwards() {
        let provider = MockHistoryProvider::new(vec![GmailSyncBatch {
            next_history_id: 8,
            deltas: vec![],
        }]);
        let mut state = GmailSyncState {
            cursor: Some(GmailSyncCursor { history_id: 9 }),
            ..GmailSyncState::default()
        };

        let outcome =
            apply_gmail_incremental_sync(&mut state, &provider, 100).expect("sync should work");

        assert!(outcome.rebootstrap_required);
        assert!(
            outcome
                .reason
                .as_deref()
                .is_some_and(|reason| reason.contains("moved backwards"))
        );
        assert_eq!(state.rebootstrap_count, 1);
    }
}
