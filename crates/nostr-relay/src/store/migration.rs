use std::collections::BTreeMap;

use sha2::{Digest, Sha256};
use tokio_postgres::Client;

use super::StoreError;

const BOOTSTRAP_SQL: &str = r#"
SELECT pg_advisory_xact_lock(1229802831, 1297109836);

CREATE TABLE IF NOT EXISTS schema_migrations (
    version bigint PRIMARY KEY,
    name text COLLATE "C" NOT NULL UNIQUE,
    sha256 text COLLATE "C" NOT NULL,
    applied_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT schema_migrations_sha256_shape CHECK (
        sha256 ~ '^[0-9a-f]{64}$'
    )
);
"#;

const SELECT_MIGRATIONS_SQL: &str =
    "SELECT version, name, sha256 FROM schema_migrations ORDER BY version";
const INSERT_MIGRATION_SQL: &str =
    "INSERT INTO schema_migrations (version, name, sha256) VALUES ($1, $2, $3)";

struct Migration {
    version: i64,
    name: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "store",
        sql: include_str!("../../../../migrations/0001_store.sql"),
    },
    Migration {
        version: 2,
        name: "nip_expansion",
        sql: include_str!("../../../../migrations/0002_nip_expansion.sql"),
    },
    Migration {
        version: 3,
        name: "media",
        sql: include_str!("../../../../migrations/0003_media.sql"),
    },
    Migration {
        version: 4,
        name: "agent_identity_turns",
        sql: include_str!("../../../../migrations/0004_agent_identity_turns.sql"),
    },
    Migration {
        version: 5,
        name: "block_server_handlers",
        sql: include_str!("../../../../migrations/0005_block_server_handlers.sql"),
    },
    Migration {
        version: 6,
        name: "nostr_effect_import",
        sql: include_str!("../../../../migrations/0006_nostr_effect_import.sql"),
    },
    Migration {
        version: 7,
        name: "legacy_expiration",
        sql: include_str!("../../../../migrations/0007_legacy_expiration.sql"),
    },
    Migration {
        version: 8,
        name: "gift_wrap_search_privacy",
        sql: include_str!("../../../../migrations/0008_gift_wrap_search_privacy.sql"),
    },
    Migration {
        version: 9,
        name: "nip29_groups",
        sql: include_str!("../../../../migrations/0009_nip29_groups.sql"),
    },
];

type AppliedMigrations = BTreeMap<i64, (String, String)>;

/// Versions applied by one migration run. An empty list means the schema was
/// already current.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MigrationReport {
    pub applied_versions: Vec<i64>,
}

pub(crate) async fn apply(client: &mut Client) -> Result<MigrationReport, StoreError> {
    let transaction = client.transaction().await?;

    // This is immutable compile-time DDL, not a runtime-built data query. The
    // entire migration set and its ledger are protected by one transaction
    // and one database-wide advisory lock.
    transaction.batch_execute(BOOTSTRAP_SQL).await?;

    let select = transaction.prepare(SELECT_MIGRATIONS_SQL).await?;
    let insert = transaction.prepare(INSERT_MIGRATION_SQL).await?;
    let rows = transaction.query(&select, &[]).await?;
    let applied = collect_applied(rows);
    validate_applied(&applied)?;

    let mut applied_versions = Vec::new();
    for migration in MIGRATIONS {
        if applied.contains_key(&migration.version) {
            continue;
        }
        transaction.batch_execute(migration.sql).await?;
        let hash = migration_hash(migration.sql);
        transaction
            .execute(&insert, &[&migration.version, &migration.name, &hash])
            .await?;
        applied_versions.push(migration.version);
    }

    transaction.commit().await?;
    Ok(MigrationReport { applied_versions })
}

/// Verify an already-migrated schema without requiring DDL privileges.
pub(crate) async fn verify(client: &Client) -> Result<MigrationReport, StoreError> {
    let select = client.prepare(SELECT_MIGRATIONS_SQL).await?;
    let applied = collect_applied(client.query(&select, &[]).await?);
    validate_applied(&applied)?;
    for migration in MIGRATIONS {
        if !applied.contains_key(&migration.version) {
            return Err(StoreError::MigrationDrift(format!(
                "database is missing version {} ({})",
                migration.version, migration.name
            )));
        }
    }
    Ok(MigrationReport {
        applied_versions: Vec::new(),
    })
}

fn collect_applied(rows: Vec<tokio_postgres::Row>) -> AppliedMigrations {
    rows.into_iter()
        .map(|row| {
            (
                row.get::<_, i64>(0),
                (row.get::<_, String>(1), row.get::<_, String>(2)),
            )
        })
        .collect()
}

fn validate_applied(applied: &AppliedMigrations) -> Result<(), StoreError> {
    for (version, (name, hash)) in applied {
        let Some(expected) = MIGRATIONS
            .iter()
            .find(|migration| migration.version == *version)
        else {
            return Err(StoreError::MigrationDrift(format!(
                "database has unknown version {version} ({name})"
            )));
        };
        if expected.name != name {
            return Err(StoreError::MigrationDrift(format!(
                "version {version} is named {name:?}, expected {:?}",
                expected.name
            )));
        }
        let expected_hash = migration_hash(expected.sql);
        if expected_hash != *hash {
            return Err(StoreError::MigrationDrift(format!(
                "version {version} hash is {hash}, expected {expected_hash}"
            )));
        }
    }
    Ok(())
}

fn migration_hash(sql: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let digest = Sha256::digest(sql.as_bytes());
    let mut output = String::with_capacity(64);
    for byte in digest {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}
