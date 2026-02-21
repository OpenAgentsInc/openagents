# 2026-02-21 Laravel DB Backup (Pre-Decommission Gate)

Issue: OA-RUST-111 (`#1936`)  
Timestamp (UTC): 2026-02-21

## Purpose

Capture fresh Laravel database backups before any legacy deletion phase.

## Command lane

```bash
PROJECT=openagentsgemini \
INSTANCE=l402-aperture-db \
BACKUP_BUCKET=gs://openagentsgemini_cloudbuild \
DATABASES_CSV=openagents_web,openagents_web_staging \
scripts/release/backup-laravel-db.sh
```

## Backup artifacts

- `gs://openagentsgemini_cloudbuild/backups/laravel/openagents_web-20260221T212513Z.sql.gz`
- `gs://openagentsgemini_cloudbuild/backups/laravel/openagents_web_staging-20260221T212513Z.sql.gz`
- `gs://openagentsgemini_cloudbuild/backups/laravel/openagents_web_staging-20260221T213651Z.sql.gz` (script validation run)

## Object metadata

1. `openagents_web-20260221T212513Z.sql.gz`
- size: `442883` bytes
- created: `2026-02-21T21:28:10Z`

2. `openagents_web_staging-20260221T212513Z.sql.gz`
- size: `193315` bytes
- created: `2026-02-21T21:31:36Z`

3. `openagents_web_staging-20260221T213651Z.sql.gz`
- size: `193322` bytes
- created: `2026-02-21T21:39:52Z`

## Notes

1. Cloud SQL service account write permission was required on the backup bucket and has been granted for repeatable exports.
2. This backup evidence is required but not sufficient for Phase C deletion; data-port verification remains a separate gate.
