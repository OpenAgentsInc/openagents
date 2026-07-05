-- Wave 1 cleanup (#8387): the standalone Khala MPP/x402 chat route and its
-- only production writers for `mpp_lightning_replay` / `mpp_spt_replay` were
-- removed in 87e6992d1e, and both tables were dropped from the treasury
-- mirror registry and contract-test fixtures in the same commit. D1 keeps
-- the (now write-dead) originals; these Postgres twins are pure
-- idempotency/replay-guard caches with zero remaining readers or writers.
--
-- Ledger-reconciliation note (KS-6.4 migration-ledger audit): this exact
-- DROP already ran against both `khala_sync_staging` and `khala_sync_prod`
-- via the normal `scripts/migrate.ts` runner on 2026-07-05 (staging
-- 12:17:03 UTC, prod 12:17:04 UTC) as `0036_drop_treasury_mpp_replay_tables.sql`,
-- but the file itself was never committed to git — confirmed absent from
-- every ref in `git log --all` and never touched by any commit (`git log
-- --all -S`). Both ledger rows recorded the identical sha256, and
-- `to_regclass('mpp_lightning_replay')` / `to_regclass('mpp_spt_replay')`
-- return NULL on both databases, confirming the drop genuinely happened and
-- is safe (matches the already-merged, zero-writer state from #8387). This
-- file is added after the fact to restore filesystem/ledger consistency;
-- since the original bytes are unrecoverable, the `khala_sync_migrations`
-- ledger `sha256` for this filename was updated (on staging and prod) to
-- this file's actual hash in the same reconciliation pass. See
-- docs/khala-sync/RUNBOOK.md "Migration runner" for the incident record.

DROP TABLE IF EXISTS mpp_lightning_replay;
DROP TABLE IF EXISTS mpp_spt_replay;
