-- Book P0-3 (#6086): finish the streaming/async split.
--
-- The async batch-job lane (#6028 / EPIC #6017) persists a row at submit and the
-- consumer drives it to a terminal state OFF the request path. To make
-- long-running work AUDITABLE the closeout receipt must disclose the batch WAIT —
-- how long the job sat between being enqueued and the consumer actually starting
-- it (the P0-1 telemetry `batchWaitMs` field). The original `0217` table only
-- tracked `created_at`/`updated_at`, which cannot distinguish "submitted" from
-- "consumer picked it up". These two timestamps close that gap:
--
--   - enqueued_at: when the submit route handed the executable message to the
--     queue producer (the start of the batch wait).
--   - started_at:  when the consumer began processing the job (the end of the
--     batch wait). batchWaitMs = started_at - enqueued_at.
--
-- Both are NULLABLE: a job submitted before this migration, or a token-only job
-- that was never enqueued, has no enqueue/start time, so the receipt honestly
-- reports `not_measured` for batchWaitMs rather than a fabricated number.

ALTER TABLE inference_batch_jobs ADD COLUMN enqueued_at TEXT;
ALTER TABLE inference_batch_jobs ADD COLUMN started_at TEXT;
