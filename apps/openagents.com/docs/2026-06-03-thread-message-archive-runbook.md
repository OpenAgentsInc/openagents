# Thread and Team Message Archive Runbook

This runbook archives visible Autopilot threads and team room messages without
deleting audit, billing, file, or token-usage records.

Archive is implemented with nullable `archived_at` columns on:

- `agent_runs`
- `team_chat_messages`
- `thread_messages`

User-facing reads filter archived rows out. OpenAgents Sync snapshots also need
delete patches because snapshots are reconstructed from `sync_changes`.

## Procedure

From `workers/api`, choose a UTC cutoff first:

```bash
date -u +%Y-%m-%dT%H:%M:%SZ
```

Check current active rows at or before the cutoff:

```bash
bunx wrangler d1 execute openagents-autopilot --remote --command "
SELECT 'active_agent_runs' AS name, COUNT(*) AS count
FROM agent_runs
WHERE archived_at IS NULL
  AND created_at <= '<cutoff>'
UNION ALL
SELECT 'active_team_chat_messages', COUNT(*)
FROM team_chat_messages
WHERE archived_at IS NULL
  AND deleted_at IS NULL
  AND created_at <= '<cutoff>'
UNION ALL
SELECT 'active_thread_messages', COUNT(*)
FROM thread_messages
WHERE archived_at IS NULL
  AND deleted_at IS NULL
  AND created_at <= '<cutoff>';
"
```

Archive source rows:

```bash
bunx wrangler d1 execute openagents-autopilot --remote --command "
UPDATE agent_runs
SET archived_at = '<cutoff>',
    updated_at = '<cutoff>'
WHERE archived_at IS NULL
  AND created_at <= '<cutoff>';

UPDATE team_chat_messages
SET archived_at = '<cutoff>',
    updated_at = '<cutoff>'
WHERE archived_at IS NULL
  AND created_at <= '<cutoff>';

UPDATE thread_messages
SET archived_at = '<cutoff>',
    updated_at = '<cutoff>'
WHERE archived_at IS NULL
  AND created_at <= '<cutoff>';
"
```

Append sync delete patches for synced thread/message collections:

```bash
bunx wrangler d1 execute openagents-autopilot --remote --command "
WITH to_delete AS (
  SELECT DISTINCT scope, collection, entity_id
  FROM sync_changes
  WHERE collection IN ('team_chat_messages', 'agent_runs', 'missions')
),
numbered AS (
  SELECT
    d.scope,
    d.collection,
    d.entity_id,
    COALESCE(s.last_seq, 0) AS base_seq,
    ROW_NUMBER() OVER (
      PARTITION BY d.scope
      ORDER BY d.collection, d.entity_id
    ) AS seq_offset
  FROM to_delete d
  LEFT JOIN sync_scopes s ON s.scope = d.scope
)
INSERT INTO sync_changes (
  scope, seq, collection, op, entity_id, value_json, patch_json,
  mutation_id, actor_id, created_at
)
SELECT
  scope,
  base_seq + seq_offset,
  collection,
  'delete',
  entity_id,
  NULL,
  NULL,
  NULL,
  NULL,
  '<cutoff>'
FROM numbered
WHERE NOT EXISTS (
  SELECT 1
  FROM sync_changes existing
  WHERE existing.scope = numbered.scope
    AND existing.collection = numbered.collection
    AND existing.entity_id = numbered.entity_id
    AND existing.op = 'delete'
    AND existing.created_at = '<cutoff>'
);

INSERT INTO sync_scopes (scope, last_seq, created_at, updated_at)
SELECT scope, MAX(seq), '<cutoff>', '<cutoff>'
FROM sync_changes
GROUP BY scope
ON CONFLICT(scope) DO UPDATE SET
  last_seq = CASE
    WHEN excluded.last_seq > sync_scopes.last_seq
      THEN excluded.last_seq
    ELSE sync_scopes.last_seq
  END,
  updated_at = excluded.updated_at;
"
```

Notify active sync streams after direct D1 archive work:

```bash
# From workers/api:
source ../../../.secrets/vortex-admin.env

curl -fsS https://openagents.com/api/admin/sync/notify \
  -H "Authorization: Bearer ${OPENAGENTS_ADMIN_API_TOKEN}" \
  -H "Content-Type: application/json" \
  -d '{"scopes":["team:team_openagents_core"]}'
```

Direct D1 writes do not automatically wake the Sync Durable Object. Without the
notify call, connected browsers may keep stale rows until a page refresh or
stream reconnect loads the newer snapshot.

Verify:

```bash
bunx wrangler d1 execute openagents-autopilot --remote --command "
SELECT 'active_agent_runs' AS name, COUNT(*) AS count
FROM agent_runs
WHERE archived_at IS NULL
UNION ALL
SELECT 'active_team_chat_messages', COUNT(*)
FROM team_chat_messages
WHERE archived_at IS NULL
  AND deleted_at IS NULL
UNION ALL
SELECT 'active_thread_messages', COUNT(*)
FROM thread_messages
WHERE archived_at IS NULL
  AND deleted_at IS NULL
UNION ALL
SELECT 'archive_sync_deletes_current', COUNT(*)
FROM sync_changes
WHERE created_at = '<cutoff>'
  AND op = 'delete';
"
```

## 2026-06-03 Receipt

Cutoff: `2026-06-03T17:55:26Z`

Archived:

- `1` active `agent_runs` row
- `2` active `team_chat_messages` rows
- `0` active `thread_messages` rows

Sync cleanup:

- `32` sync delete patches inserted at the cutoff

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`

## 2026-06-03 Late Receipt

Cutoff: `2026-06-03T23:41:54Z`

Archived:

- `1` active `agent_runs` row
- `0` active `team_chat_messages` rows
- `0` active `thread_messages` rows

Sync cleanup:

- `4` sync delete patches inserted at the cutoff
- notified `workspace:github:14167547`, `team:team_openagents_core`,
  `thread:38e31d3f-9e60-42f4-bd5c-f21de1702297`, and
  `agent-run:38e31d3f-9e60-42f4-bd5c-f21de1702297`

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`

## 2026-06-04 Receipt

Cutoff: `2026-06-04T00:04:42Z`

Archived:

- `1` active `agent_runs` row: `cf44c410-3f0a-40a1-a3f6-4086091bc28a`
- `0` active `team_chat_messages` rows
- `0` active `thread_messages` rows

Sync cleanup:

- `86` sync delete patches inserted at the cutoff
- notified the workspace, OpenAgents Core Team, and affected thread/run sync
  scopes

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`

## 2026-06-04 Late Receipt

Cutoff: `2026-06-04T00:35:56Z`

Archived:

- `2` active `agent_runs` rows:
  - `409e6f90-ea73-46ca-9094-6d70d169e9d3`
  - `5cea4022-b02a-46f6-a3c5-8dc2481c184f`
- `4` active `team_chat_messages` rows:
  - `team_chat_51a50f84-b70f-4bff-b7a2-8e3f71146169`
  - `team_chat_answer_409e6f90-ea73-46ca-9094-6d70d169e9d3`
  - `team_chat_35fcebe0-c1e2-4f9a-b2e1-541eb6a38c79`
  - `team_chat_answer_5cea4022-b02a-46f6-a3c5-8dc2481c184f`
- `0` active `thread_messages` rows

Sync cleanup:

- `12` sync delete patches inserted at the cutoff
- notified `team:team_openagents_core`, `workspace:github:14167547`,
  the two affected `agent-run:*` scopes, and the two affected `thread:*`
  scopes

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`
- operator project chat list for `project_artanis`: `0` messages
- operator team chat list: `0` messages

## 2026-06-04 Current Receipt

Cutoff: `2026-06-04T06:54:06Z`

Archived:

- `5` active `agent_runs` rows:
  - `11300cc8-17ba-4387-b9b9-881676c5ee37`
  - `62fac3fa-56e1-4aee-b672-51999f3dacf2`
  - `e62a692d-de8c-4121-abe2-8af2035a2767`
  - `772f3453-c384-4b7c-87ca-a76c0826acb8`
  - `1cb11e49-862e-4c7e-bac0-56b978d47134`
- `6` active `team_chat_messages` rows:
  - `team_chat_c912d35f-acff-4cf3-b103-7b672ddaf9b6`
  - `team_chat_answer_11300cc8-17ba-4387-b9b9-881676c5ee37`
  - `team_chat_ccb092b7-0e20-42d2-914f-02577289e0f1`
  - `team_chat_answer_62fac3fa-56e1-4aee-b672-51999f3dacf2`
  - `team_chat_f99164a2-50bc-48f5-9a3c-763fa52a2d21`
  - `team_chat_answer_e62a692d-de8c-4121-abe2-8af2035a2767`
- `0` active `thread_messages` rows

Sync cleanup:

- `144` sync delete patches present at the cutoff after direct archive plus SHC
  cleanup callbacks.
- Notified `workspace:github:14167547`, `team:team_openagents_core`, and the
  five affected `agent-run:*` / `thread:*` scopes.

SHC cleanup:

- Confirmed no matching active SHC processes for the five archived run IDs.
- Sent SHC control cancel requests for all five run IDs; each request was
  accepted and returned `status: canceled`.
- Removed the exact five matching run/job state directories under
  `/var/lib/openagents/codex-control`.
- Verified no matching SHC process or state directory remained afterward.

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`

## 2026-06-04 Refresh Receipt

Cutoff: `2026-06-04T07:09:58Z`

Archived:

- `0` active `agent_runs` rows
- `0` active `team_chat_messages` rows
- `0` active `thread_messages` rows

Sync cleanup:

- `124` sync delete patches inserted at the cutoff to refresh stale synced
  thread, mission, agent-run, and team-message projections.
- Notified `workspace:github:14167547`, `team:team_openagents_core`, and all
  affected `agent-run:*` / `thread:*` scopes returned by the cutoff delete
  patch query.

Verification after the archive:

- `active_agent_runs`: `0`
- `active_team_chat_messages`: `0`
- `active_thread_messages`: `0`
- `archive_sync_deletes_current`: `124`
