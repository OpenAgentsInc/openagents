# Khala Production Export + Laravel Import Runbook

## Purpose

This runbook documents the exact process to:

1. Export production chat/user data from the legacy Khala deployment (when you have Khala CLI and prod access).
2. Validate the export artifact.
3. Import that artifact into the Laravel app (`apps/openagents.com`) using `khala:import-chat`.

This is written to be executable by humans and agents, with deterministic commands and verification checks.

---

## Scope

### Included now

- Khala production export command and artifact validation.
- Laravel import command (`dry-run` and write mode).
- Local verification checks.
- Operational notes for production run.

### Not included

- Automatic backfill scheduling.
- Zero-downtime dual-write cutover.
- Reconciliation tooling beyond table-count checks.

---

## Prerequisites

- Local checkout of this repo.
- Khala CLI available (e.g. `npx khala` with valid prod auth).
- Valid Khala auth in terminal (`npx khala` can access prod deployment).
- Laravel app migrated and command available in `apps/openagents.com`.
- Sufficient disk space for ZIP artifact.

---

## Source and destination mapping

### Source (Khala export tables)

- `users`
- `threads`
- `runs`
- `messages`
- `receipts`

### Destination (Laravel tables)

- `users`
- `threads`
- `runs`
- `messages`
- `run_events`
- `agent_conversations`
- `agent_conversation_messages`

Importer implementation:

- `apps/openagents.com/app/Support/KhalaImport/KhalaExportReader.php`
- `apps/openagents.com/app/Support/KhalaImport/KhalaChatImportService.php`
- `apps/openagents.com/routes/console.php` (`khala:import-chat`)

Test coverage:

- `apps/openagents.com/tests/Feature/KhalaImportChatCommandTest.php`

---

## Step 1: Export Khala production data

From repo root (or any directory with Khala project config and prod auth):

```bash
npx khala export --prod --path /tmp/khala-prod-export-$(date +%Y%m%d-%H%M%S).zip
```

Expected result:

- CLI prints successful export completion.
- ZIP file is created at the given path.

---

## Step 2: Validate export artifact

Given `EXPORT_ZIP=/tmp/khala-prod-export-YYYYMMDD-HHMMSS.zip`:

```bash
ls -lh "$EXPORT_ZIP"
shasum -a 256 "$EXPORT_ZIP"
unzip -l "$EXPORT_ZIP" | sed -n '1,120p'
```

Checks:

- Non-zero file size.
- SHA256 recorded in migration notes/change log.
- ZIP contains `<table>/documents.jsonl` entries.

---

## Step 3: Dry-run import into Laravel

```bash
cd apps/openagents.com
php artisan khala:import-chat "$EXPORT_ZIP" --dry-run
```

Checks:

- Command exits `0`.
- Summary prints `source_*` counts.
- No DB writes occur.

---

## Step 4: Real import into Laravel

### Merge mode (idempotent upsert)

```bash
php artisan khala:import-chat "$EXPORT_ZIP"
```

### Replace mode (truncate target chat tables first)

```bash
php artisan khala:import-chat "$EXPORT_ZIP" --replace
```

Use `--replace` only when intentionally replacing existing Laravel chat data.

---

## Step 5: Post-import verification

```bash
php artisan tinker --execute="echo 'users:'.DB::table('users')->count().PHP_EOL; echo 'threads:'.DB::table('threads')->count().PHP_EOL; echo 'runs:'.DB::table('runs')->count().PHP_EOL; echo 'messages:'.DB::table('messages')->count().PHP_EOL; echo 'run_events:'.DB::table('run_events')->count().PHP_EOL; echo 'agent_conversations:'.DB::table('agent_conversations')->count().PHP_EOL; echo 'agent_conversation_messages:'.DB::table('agent_conversation_messages')->count().PHP_EOL;"
```

Also verify app-level behavior:

- Can open chat list.
- Can open older conversations.
- Messages and run metadata appear as expected.

---

## Safety notes

- Always run dry-run first in production.
- Keep a copy of export ZIP and its checksum until verification is complete.
- If running with `--replace`, take DB backup/snapshot first.
- Import skips anonymous/ownerless threads by design.

---

## Current execution log

### 2026-02-16 (this session)

- Action target: **prod export only** (per request).
- Status: **completed**.

#### Command executed

```bash
npx khala export --prod --path /tmp/khala-prod-export-20260216-142537.zip
```

#### Khala CLI result

- Created snapshot export timestamp: `1771273539681368685`
- Dashboard artifact URL: `https://dashboard.khala.dev/d/aware-caterpillar-962/settings/snapshot-export`
- Download completed successfully.

#### Export artifact

- Export path: `/tmp/khala-prod-export-20260216-142537.zip`
- File size: `319K`
- SHA256: `e9891613fd93f8e9402221aefa2fd0238cdcb52b30796686441037cda4e34f2f`

#### ZIP validation summary

- Archive contains expected chat tables:
  - `users/documents.jsonl`
  - `threads/documents.jsonl`
  - `runs/documents.jsonl`
  - `messages/documents.jsonl`
  - `receipts/documents.jsonl`
- Archive also contains additional product/runtime tables (Lightning + DSE + blueprint/messageParts), total 76 files.

#### Next step (not run in this session)

- Run Laravel dry-run import from this exact ZIP:

```bash
cd apps/openagents.com
php artisan khala:import-chat /tmp/khala-prod-export-20260216-142537.zip --dry-run
```


---

## Importer v2 (identity + personality migration)

As of issue `#1654`, `khala:import-chat` supports identity hydration and blueprint personality migration in one pass.

### New source table usage

In addition to `users`, `threads`, `runs`, `messages`, and `receipts`, importer v2 reads:

- `blueprints/documents.jsonl`

### New command options

```bash
php artisan khala:import-chat "$EXPORT_ZIP" --dry-run --resolve-workos-users
php artisan khala:import-chat "$EXPORT_ZIP" --resolve-workos-users
php artisan khala:import-chat "$EXPORT_ZIP" --replace --resolve-workos-users
php artisan khala:import-chat "$EXPORT_ZIP" --skip-blueprints
```

### New summary counters

Importer output now includes:

- `users_resolved_via_workos`
- `users_unresolved_placeholder`
- `users_email_conflicts`
- `autopilots_created`
- `autopilot_profiles_upserted`
- `autopilot_policies_upserted`
- `blueprints_mapped`
- `threads_linked_backfilled`
- `runs_linked_backfilled`
- `messages_linked_backfilled`
- `run_events_linked_backfilled`

### Identity behavior

- If Khala user email is blank and the owner id looks like a WorkOS id (`user_*`), importer can resolve email/name/avatar from WorkOS (`--resolve-workos-users`).
- If lookup fails, importer deterministically falls back to `migrated+...@openagents.local`.
- If WorkOS email conflicts with an existing user email, importer records conflict metric and keeps deterministic placeholder for that WorkOS user.

### Blueprint -> Autopilot mapping behavior

Importer ensures one autopilot per owner user (idempotent) and upserts:

- `autopilots`
- `autopilot_profiles`
- `autopilot_policies`

Then importer backfills autopilot linkage into legacy runtime rows:

- `threads.autopilot_id`
- `runs.autopilot_id` and `runs.autopilot_config_version`
- `messages.autopilot_id`
- `run_events.autopilot_id` and actor autopilot fields where applicable

---

## Production backfill runbook (v2)

Use this exact sequence for production migrations.

### 1) Snapshot before counts

```bash
cd apps/openagents.com
php artisan tinker --execute="echo 'users='.DB::table('users')->count().PHP_EOL; echo 'threads='.DB::table('threads')->count().PHP_EOL; echo 'runs='.DB::table('runs')->count().PHP_EOL; echo 'messages='.DB::table('messages')->count().PHP_EOL; echo 'run_events='.DB::table('run_events')->count().PHP_EOL; echo 'autopilots='.DB::table('autopilots')->count().PHP_EOL; echo 'autopilot_profiles='.DB::table('autopilot_profiles')->count().PHP_EOL; echo 'autopilot_policies='.DB::table('autopilot_policies')->count().PHP_EOL;"
```

### 2) Dry-run with WorkOS hydration

```bash
php artisan khala:import-chat "$EXPORT_ZIP" --dry-run --resolve-workos-users
```

Capture the summary output in release notes / migration log.

### 3) Real import

```bash
php artisan khala:import-chat "$EXPORT_ZIP" --resolve-workos-users
```

Use `--replace` only for planned full replacement windows.

### 4) Snapshot after counts

```bash
php artisan tinker --execute="echo 'users='.DB::table('users')->count().PHP_EOL; echo 'threads='.DB::table('threads')->count().PHP_EOL; echo 'runs='.DB::table('runs')->count().PHP_EOL; echo 'messages='.DB::table('messages')->count().PHP_EOL; echo 'run_events='.DB::table('run_events')->count().PHP_EOL; echo 'autopilots='.DB::table('autopilots')->count().PHP_EOL; echo 'autopilot_profiles='.DB::table('autopilot_profiles')->count().PHP_EOL; echo 'autopilot_policies='.DB::table('autopilot_policies')->count().PHP_EOL; echo 'threads_with_autopilot='.DB::table('threads')->whereNotNull('autopilot_id')->count().PHP_EOL; echo 'runs_with_autopilot='.DB::table('runs')->whereNotNull('autopilot_id')->count().PHP_EOL; echo 'messages_with_autopilot='.DB::table('messages')->whereNotNull('autopilot_id')->count().PHP_EOL; echo 'run_events_with_autopilot='.DB::table('run_events')->whereNotNull('autopilot_id')->count().PHP_EOL;"
```

### 5) Rollback strategy

- Preferred rollback: restore DB snapshot/backup captured immediately before step 3.
- If rollback is only needed for imported runtime data and not other writes, run import again in `--replace` mode from a known-good export artifact.
- Keep the export ZIP and SHA256 hash until verification is complete.

### 6) Verification checklist

- `autopilots`, `autopilot_profiles`, and `autopilot_policies` are non-zero for imported owners.
- Imported owner threads/runs/messages/events have autopilot linkage.
- At least one imported user with missing Khala email resolves via WorkOS in summary counters.
- Settings page can edit autopilot profile; a subsequent run reflects updated profile context.
