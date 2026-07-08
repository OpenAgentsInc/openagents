# Migration System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #50 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent local state, event logs, settings, memory, artifacts,
indexes, and workspace metadata evolve safely across versions.

## Target

Build a migration system that upgrades durable local and account-bound state
with versioned schemas, restore points, validation, rollback boundaries, and
public-safe migration receipts.

## User-Visible Capability

Users should be able to:

- Start a new version without losing sessions or settings.
- See when migration is required.
- Understand when migration failed and how to recover.
- Export a diagnostic summary.
- Keep old private data private during migration.
- Choose whether to migrate optional caches or rebuild them.

Migrations should be quiet when successful and explicit when risky.

## State Domains

Migration domains:

- Settings.
- Credentials metadata, never credential values in logs.
- Event logs.
- Session summaries.
- Memory records.
- Repository profiles.
- Artifact indexes.
- Tool and permission caches.
- Plugin and skill registries.
- Telemetry preferences.
- Release-channel metadata.

Each domain should have its own schema version and migration receipt.

## Bun/Effect Boundary

Use Effect services for:

- `MigrationRegistryService`: lists migrations and schema domains.
- `MigrationRunnerService`: runs migrations in order.
- `MigrationSnapshotService`: snapshots pre-migration state when required.
- `MigrationValidationService`: checks post-migration invariants.
- `MigrationProjectionService`: reports status to users and operators.

Use Schema for each persisted domain. Use Scope for temporary snapshot and
rollback resources. Use Effect error types for retryable, blocking, and
manual-recovery failures.

## Safety Rules

- Migrations must be idempotent.
- Private payloads do not enter migration logs.
- Credential values are never written into migration receipts.
- Failed required migrations block startup or enter recovery mode.
- Optional cache migrations can fall back to rebuild.
- Public projection migrations require freshness and safety checks.
- Schema downgrades must be explicit; silent downgrade is unsafe.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents uses migrations in product and worker surfaces,
and has public projection staleness requirements. The terminal-agent README
does not yet include a terminal migration audit.

Related open issue anchors:

- #4778 mission/work-order unification will require cross-record migration
  discipline.
- #4769 repo connect and data-scope UX depends on state migration for repo
  profiles.
- #4772 MVP exit review should include migration smoke status.

No terminal state-migration claim should be green until versioned fixtures,
rollback behavior, and redaction checks exist.

## Tests

Minimum coverage:

- Run migrations from each supported prior schema fixture.
- Re-run migrations idempotently.
- Fail safely on corrupted state.
- Preserve private data without logging it.
- Rebuild optional indexes.
- Block startup on required migration failure.
- Validate post-migration event replay.
- Record migration receipts with safe refs only.

## Decision

Migrations should be domain-scoped, versioned, and reversible where possible.
They are runtime authority changes and should be tested like production data
paths, not treated as incidental startup code.
