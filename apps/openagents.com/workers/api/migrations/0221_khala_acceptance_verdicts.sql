-- Khala acceptance-verification verdict store (EPIC #6017).
--
-- The hot gateway path writes the HONEST `unverified` row at completion time for a
-- khala-code completion with an executable artifact; the out-of-Worker headless
-- runner posts an executed `AcceptanceVerdict` to the authenticated verdict callback,
-- which BACKFILLS this row to `test_passed`/`failed`, `verified`, `scalar_reward`, and
-- per-test results. The public receipt read projects from this row.
--
-- This is the EXECUTION verdict (did we run it and did it do what the user asked) —
-- distinct from the financial `pay_ins` charge ledger (`inference-receipts.ts`).
CREATE TABLE IF NOT EXISTS khala_acceptance_verdicts (
    request_id TEXT PRIMARY KEY,
    verification TEXT NOT NULL,
    verified INTEGER NOT NULL DEFAULT 0,
    executed INTEGER NOT NULL DEFAULT 0,
    scalar_reward REAL NOT NULL DEFAULT 0,
    rubric_ref TEXT NOT NULL,
    passed_checks TEXT NOT NULL DEFAULT '[]',
    failed_checks TEXT NOT NULL DEFAULT '[]',
    verification_receipt_ref TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_khala_acceptance_verdicts_state
    ON khala_acceptance_verdicts (verification, updated_at DESC);
