use anyhow::{Context, Result, anyhow};
use sha2::{Digest, Sha256};
use tokio_postgres::NoTls;
use tracing_subscriber::EnvFilter;

struct EmbeddedMigration {
    version: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[EmbeddedMigration] = &[EmbeddedMigration {
    version: "0001_runtime_sync_bootstrap",
    sql: include_str!("../../sql/migrations/0001_runtime_sync_bootstrap.sql"),
}];

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,runtime_migrate=debug")),
        )
        .with_current_span(true)
        .init();

    let database_url = std::env::var("DB_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .context("DB_URL or DATABASE_URL must be set for runtime-migrate")?;
    run_migrations(&database_url).await
}

async fn run_migrations(database_url: &str) -> Result<()> {
    let (mut client, connection) = tokio_postgres::connect(database_url, NoTls)
        .await
        .context("connect to postgres")?;
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            tracing::error!(reason = %error, "runtime-migrate postgres connection error");
        }
    });

    client
        .batch_execute(
            r#"
            CREATE SCHEMA IF NOT EXISTS runtime;
            CREATE TABLE IF NOT EXISTS runtime.schema_migrations (
              version TEXT PRIMARY KEY,
              checksum TEXT NOT NULL,
              applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
            );
            "#,
        )
        .await
        .context("bootstrap runtime.schema_migrations")?;

    for migration in MIGRATIONS {
        apply_migration(&mut client, migration).await?;
    }

    tracing::info!(
        migration_count = MIGRATIONS.len(),
        "runtime migrations complete"
    );
    Ok(())
}

async fn apply_migration(
    client: &mut tokio_postgres::Client,
    migration: &EmbeddedMigration,
) -> Result<()> {
    let checksum = checksum_hex(migration.sql);
    let tx = client
        .transaction()
        .await
        .context("begin migration transaction")?;

    let existing = tx
        .query_opt(
            "SELECT checksum FROM runtime.schema_migrations WHERE version = $1",
            &[&migration.version],
        )
        .await
        .with_context(|| format!("query migration state for {}", migration.version))?;

    if let Some(row) = existing {
        let existing_checksum: String = row.get("checksum");
        if existing_checksum != checksum {
            return Err(anyhow!(
                "migration checksum mismatch for {} (existing={}, current={})",
                migration.version,
                existing_checksum,
                checksum
            ));
        }

        tracing::info!(version = migration.version, "migration already applied");
        tx.commit()
            .await
            .context("commit no-op migration transaction")?;
        return Ok(());
    }

    tracing::info!(version = migration.version, "applying migration");
    tx.batch_execute(migration.sql)
        .await
        .with_context(|| format!("execute migration {}", migration.version))?;
    tx.execute(
        "INSERT INTO runtime.schema_migrations (version, checksum) VALUES ($1, $2)",
        &[&migration.version, &checksum],
    )
    .await
    .with_context(|| format!("record migration {}", migration.version))?;
    tx.commit().await.context("commit migration transaction")?;
    Ok(())
}

fn checksum_hex(contents: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(contents.as_bytes());
    hex::encode(hasher.finalize())
}
