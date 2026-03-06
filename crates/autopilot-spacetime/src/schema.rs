//! Canonical core schema metadata for OpenAgents Spacetime sync.

/// Table specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableSpec {
    pub name: &'static str,
    pub description: &'static str,
}

/// Index specification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IndexSpec {
    pub name: &'static str,
    pub table: &'static str,
    pub columns: &'static [&'static str],
    pub unique: bool,
}

/// Core Spacetime schema tables for the sync replacement track.
pub const CORE_TABLES: &[TableSpec] = &[
    TableSpec {
        name: "sync_stream",
        description: "Logical stream registration and metadata.",
    },
    TableSpec {
        name: "sync_event",
        description: "Append-only stream events with hash and idempotency keys.",
    },
    TableSpec {
        name: "sync_checkpoint",
        description: "Per-client stream watermark checkpoints for resume.",
    },
    TableSpec {
        name: "session_presence",
        description: "Active peer/session liveness state.",
    },
    TableSpec {
        name: "provider_capability",
        description: "Provider capability and routing metadata.",
    },
    TableSpec {
        name: "compute_assignment",
        description: "Compute request assignment lifecycle projection.",
    },
    TableSpec {
        name: "bridge_outbox",
        description: "Policy-gated mirror queue for bridge publication.",
    },
    TableSpec {
        name: "presence_event",
        description: "Transient presence signal events.",
    },
    TableSpec {
        name: "coordination_event",
        description: "Transient coordination signal events.",
    },
    TableSpec {
        name: "conflict_event",
        description: "Transient conflict signal events.",
    },
];

/// Required indexes for query and replay semantics.
pub const CORE_INDEXES: &[IndexSpec] = &[
    IndexSpec {
        name: "ux_sync_stream_stream_id",
        table: "sync_stream",
        columns: &["stream_id"],
        unique: true,
    },
    IndexSpec {
        name: "ux_sync_event_stream_seq",
        table: "sync_event",
        columns: &["stream_id", "seq"],
        unique: true,
    },
    IndexSpec {
        name: "ux_sync_event_stream_idempotency",
        table: "sync_event",
        columns: &["stream_id", "idempotency_key"],
        unique: true,
    },
    IndexSpec {
        name: "ix_sync_event_commit_ts",
        table: "sync_event",
        columns: &["committed_at_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ux_sync_checkpoint_client_stream",
        table: "sync_checkpoint",
        columns: &["client_id", "stream_id"],
        unique: true,
    },
    IndexSpec {
        name: "ix_session_presence_last_seen",
        table: "session_presence",
        columns: &["last_seen_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ux_provider_capability_provider",
        table: "provider_capability",
        columns: &["provider_id"],
        unique: true,
    },
    IndexSpec {
        name: "ux_compute_assignment_request",
        table: "compute_assignment",
        columns: &["request_id"],
        unique: true,
    },
    IndexSpec {
        name: "ix_compute_assignment_status_updated",
        table: "compute_assignment",
        columns: &["status", "updated_at_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ix_bridge_outbox_status_created",
        table: "bridge_outbox",
        columns: &["status", "created_at_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ix_presence_event_committed",
        table: "presence_event",
        columns: &["committed_at_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ix_coordination_event_committed",
        table: "coordination_event",
        columns: &["committed_at_unix_ms"],
        unique: false,
    },
    IndexSpec {
        name: "ix_conflict_event_committed",
        table: "conflict_event",
        columns: &["committed_at_unix_ms"],
        unique: false,
    },
];

/// Canonical SQL DDL for the current core schema.
pub fn core_schema_sql() -> &'static str {
    include_str!("../schema/core.sql")
}

#[cfg(test)]
mod tests {
    use super::{CORE_INDEXES, CORE_TABLES, core_schema_sql};

    #[test]
    fn core_schema_has_required_tables() {
        let ddl = core_schema_sql();
        for table in CORE_TABLES {
            let marker = format!("CREATE TABLE IF NOT EXISTS {}", table.name);
            assert!(
                ddl.contains(marker.as_str()),
                "missing table ddl for {}",
                table.name
            );
        }
    }

    #[test]
    fn core_schema_has_required_indexes() {
        let ddl = core_schema_sql();
        for index in CORE_INDEXES {
            let marker = format!("CREATE INDEX IF NOT EXISTS {}", index.name);
            let unique_marker = format!("CREATE UNIQUE INDEX IF NOT EXISTS {}", index.name);
            let found = ddl.contains(marker.as_str()) || ddl.contains(unique_marker.as_str());
            assert!(found, "missing index ddl for {}", index.name);
        }
    }
}
