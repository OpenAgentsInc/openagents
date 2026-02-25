//! Subscription query-set definitions and index coverage checks.

use crate::schema::CORE_INDEXES;

/// Supported subscription query sets for sync fanout/replay.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SubscriptionQuerySet {
    StreamEvents { stream_id: String, after_seq: u64 },
    SessionPresenceByNode { node_id: String },
    ProviderAssignmentsByStatus { provider_id: String, status: String },
    BridgeOutboxPending,
}

/// Query plan metadata.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QueryPlan {
    pub table: &'static str,
    pub predicate: &'static str,
    pub order_by: &'static str,
    pub required_index_names: &'static [&'static str],
}

/// Returns the canonical query plan for a subscription query set.
pub fn query_plan(query: &SubscriptionQuerySet) -> QueryPlan {
    match query {
        SubscriptionQuerySet::StreamEvents { .. } => QueryPlan {
            table: "sync_event",
            predicate: "stream_id = ? AND seq > ?",
            order_by: "ORDER BY seq ASC",
            required_index_names: &["ux_sync_event_stream_seq"],
        },
        SubscriptionQuerySet::SessionPresenceByNode { .. } => QueryPlan {
            table: "session_presence",
            predicate: "node_id = ?",
            order_by: "ORDER BY last_seen_unix_ms DESC",
            required_index_names: &["ix_session_presence_last_seen"],
        },
        SubscriptionQuerySet::ProviderAssignmentsByStatus { .. } => QueryPlan {
            table: "compute_assignment",
            predicate: "provider_id = ? AND status = ?",
            order_by: "ORDER BY updated_at_unix_ms DESC",
            required_index_names: &[
                "ux_compute_assignment_request",
                "ix_compute_assignment_status_updated",
            ],
        },
        SubscriptionQuerySet::BridgeOutboxPending => QueryPlan {
            table: "bridge_outbox",
            predicate: "status = 'pending'",
            order_by: "ORDER BY created_at_unix_ms ASC",
            required_index_names: &["ix_bridge_outbox_status_created"],
        },
    }
}

/// Returns missing index names required for a query plan.
pub fn missing_indexes(plan: &QueryPlan) -> Vec<String> {
    let available = CORE_INDEXES.iter().map(|index| index.name).collect::<Vec<_>>();
    plan.required_index_names
        .iter()
        .filter(|required| !available.contains(required))
        .map(|missing| (*missing).to_string())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{SubscriptionQuerySet, missing_indexes, query_plan};

    #[test]
    fn stream_query_has_index_coverage() {
        let query = SubscriptionQuerySet::StreamEvents {
            stream_id: "runtime.codex.worker.worker_1".to_string(),
            after_seq: 10,
        };
        let plan = query_plan(&query);
        let missing = missing_indexes(&plan);
        assert!(missing.is_empty(), "missing indexes: {missing:?}");
    }

    #[test]
    fn provider_assignment_query_has_index_coverage() {
        let query = SubscriptionQuerySet::ProviderAssignmentsByStatus {
            provider_id: "provider_1".to_string(),
            status: "open".to_string(),
        };
        let plan = query_plan(&query);
        let missing = missing_indexes(&plan);
        assert!(missing.is_empty(), "missing indexes: {missing:?}");
    }

    #[test]
    fn bridge_outbox_query_has_index_coverage() {
        let query = SubscriptionQuerySet::BridgeOutboxPending;
        let plan = query_plan(&query);
        let missing = missing_indexes(&plan);
        assert!(missing.is_empty(), "missing indexes: {missing:?}");
    }
}

