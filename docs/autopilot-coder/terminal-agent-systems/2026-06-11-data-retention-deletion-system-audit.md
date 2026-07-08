# Data Retention And Deletion System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #54 from the Bun/Effect terminal-agent systems list. It defines
how terminal-agent data is retained, deleted, tombstoned, exported, and kept
out of stale projections.

## Target

Build a retention and deletion system for sessions, event logs, artifacts,
memory, telemetry, credentials metadata, plugin state, workspace caches, and
public projections.

Deletion should be explicit and auditable. Retention should be declared before
data is written.

## User-Visible Capability

Users should be able to:

- Delete a session or artifact.
- Clear local caches.
- Delete or correct memory.
- Export their retained data.
- See retention policy for each data class.
- Know which deletion requests leave tombstones.
- Verify public projections no longer claim deleted or stale data as current.

## Data Classes

Recommended classes:

- Ephemeral capture data.
- Temporary workspace material.
- Private event log payloads.
- Public-safe event refs.
- Session summaries.
- Memory records.
- Artifact payloads.
- Artifact indexes.
- Telemetry aggregates.
- Credential metadata.
- Product receipts.

Each class should declare retention, deletion, export, tombstone, and
projection behavior.

## Bun/Effect Boundary

Use Effect services for:

- `RetentionPolicyService`: resolves data-class policy.
- `DeletionService`: deletes, tombstones, or schedules deletion.
- `ExportService`: builds private and public-safe exports.
- `TombstoneService`: prevents stale rehydration.
- `ProjectionInvalidationService`: refreshes derived views after deletion.

Use Schema for data classes, deletion requests, tombstones, and export
manifests. Use Schedule for delayed deletion and retention sweeps.

## Safety Rules

- Deleting private payloads must not delete required public-safe receipts
  unless policy permits.
- Tombstones should prevent caches from reintroducing deleted data.
- Secrets and credential values are deleted or delegated to secure storage
  revocation, not written into exports.
- Public projections carry freshness and deletion caveats.
- Retention sweeps must be idempotent.
- Legal or payment retention requirements must be explicit.
- Deletion logs contain refs, not deleted payloads.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has projection staleness invariants, redaction
policy, product-promise refs, and payment/receipt retention needs. The
terminal-agent README does not yet include a retention/deletion audit.

Related open issue anchors:

- #4778 mission/work-order unification.
- #4785 settlement visibility law.
- #4770 spend-to-evidence join.
- #4772 MVP exit review.

No deletion or retention claim should be green until data classes, tombstones,
exports, and projection invalidation have tests.

## Tests

Minimum coverage:

- Delete private sessions, artifacts, and memory records.
- Preserve required public-safe receipts.
- Prevent tombstoned records from reappearing in caches.
- Export retained user data without secrets.
- Run retention sweeps idempotently.
- Invalidate projections after deletion.
- Show deletion caveats for payment or legal records.
- Verify deletion logs are ref-only.

## Decision

Retention and deletion should be designed into every persisted data class. The
runtime should not discover later that it cannot honor deletion because raw
data was mixed with public evidence.
