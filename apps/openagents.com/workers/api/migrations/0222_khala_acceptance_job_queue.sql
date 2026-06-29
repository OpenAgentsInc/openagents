-- Out-of-Worker acceptance-runner PULL queue (EPIC #6017).
--
-- The gateway dispatch enqueues acceptance jobs, but the out-of-Worker runner CANNOT be
-- a Cloudflare Queue consumer (a consumer is a Worker; chromium never runs in a Worker),
-- so it PULLS its work over an authenticated lease endpoint. This table is the durable
-- pull queue the lease/ack routes operate on: a job is `pending` until leased, then
-- `leased` with a lease expiry (so a crashed runner's job is re-leasable after the
-- timeout), then removed when the verdict callback backfills (acked delivered) or
-- re-`pending` (acked retryable / lease expired).
--
-- INERT: nothing writes rows here until the dispatch producer is wired to enqueue into
-- this table (a NEEDS-OWNER step owned by the dispatch lane) AND the lease routes are
-- armed. With no rows, the lease endpoint always returns "no job" (204) and the runner
-- idles. This table only stores a dereferenceable artifact ref + the typed job payload,
-- NEVER raw artifact bytes (those live in R2) and NEVER prompts/credentials.
CREATE TABLE IF NOT EXISTS khala_acceptance_jobs (
    -- The inference response id the verdict backfills (one job per khala-code outcome).
    request_id TEXT PRIMARY KEY,
    -- 'pending' (available to lease) | 'leased' (in flight, lease_expires_at set).
    status TEXT NOT NULL DEFAULT 'pending',
    -- The full typed AcceptanceJobMessage as JSON text (schema-validated on lease).
    job_payload TEXT NOT NULL,
    -- The opaque lease handle the runner echoes back to ack; null while pending.
    lease_id TEXT,
    -- When the current lease expires and the job becomes re-leasable; null while pending.
    lease_expires_at TEXT,
    -- How many times this job has been leased (a poison job can be capped by the routes).
    attempts INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

-- Lease query: oldest pending (or lease-expired) job first.
CREATE INDEX IF NOT EXISTS idx_khala_acceptance_jobs_lease
    ON khala_acceptance_jobs (status, lease_expires_at, created_at);
