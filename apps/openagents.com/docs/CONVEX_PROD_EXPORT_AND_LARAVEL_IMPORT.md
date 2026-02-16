# Convex Production Export + Laravel Import Runbook

## Purpose

This runbook documents the exact process to:

1. Export production chat/user data from the legacy `apps/web` Convex deployment.
2. Validate the export artifact.
3. Import that artifact into the Laravel app (`apps/openagents.com`) using `convex:import-chat`.

This is written to be executable by humans and agents, with deterministic commands and verification checks.

---

## Scope

### Included now

- Convex production export command and artifact validation.
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
- Convex CLI available via npm in `apps/web`.
- Valid Convex auth in terminal (`npx convex` can access prod deployment).
- Laravel app migrated and command available in `apps/openagents.com`.
- Sufficient disk space for ZIP artifact.

---

## Source and destination mapping

### Source (Convex export tables)

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

- `apps/openagents.com/app/Support/ConvexImport/ConvexExportReader.php`
- `apps/openagents.com/app/Support/ConvexImport/ConvexChatImportService.php`
- `apps/openagents.com/routes/console.php` (`convex:import-chat`)

Test coverage:

- `apps/openagents.com/tests/Feature/ConvexImportChatCommandTest.php`

---

## Step 1: Export Convex production data

Run from `apps/web`:

```bash
cd apps/web
npx convex export --prod --path /tmp/convex-prod-export-$(date +%Y%m%d-%H%M%S).zip
```

Expected result:

- CLI prints successful export completion.
- ZIP file is created at the given path.

---

## Step 2: Validate export artifact

Given `EXPORT_ZIP=/tmp/convex-prod-export-YYYYMMDD-HHMMSS.zip`:

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
php artisan convex:import-chat "$EXPORT_ZIP" --dry-run
```

Checks:

- Command exits `0`.
- Summary prints `source_*` counts.
- No DB writes occur.

---

## Step 4: Real import into Laravel

### Merge mode (idempotent upsert)

```bash
php artisan convex:import-chat "$EXPORT_ZIP"
```

### Replace mode (truncate target chat tables first)

```bash
php artisan convex:import-chat "$EXPORT_ZIP" --replace
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
cd apps/web
npx convex export --prod --path /tmp/convex-prod-export-20260216-142537.zip
```

#### Convex CLI result

- Created snapshot export timestamp: `1771273539681368685`
- Dashboard artifact URL: `https://dashboard.convex.dev/d/aware-caterpillar-962/settings/snapshot-export`
- Download completed successfully.

#### Export artifact

- Export path: `/tmp/convex-prod-export-20260216-142537.zip`
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
php artisan convex:import-chat /tmp/convex-prod-export-20260216-142537.zip --dry-run
```

