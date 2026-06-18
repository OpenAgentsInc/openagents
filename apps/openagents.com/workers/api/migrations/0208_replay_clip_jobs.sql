-- Replay clip jobs (EPIC #5411, issue #5432).
--
-- Hosts clip-job create/read records inside the existing api Worker + D1. The
-- Worker only stores job records and serves public-safe refs; it NEVER renders
-- frames or runs native binaries. Rendering happens on the owned render box
-- (#5431), which claims `queued` jobs, renders, uploads to R2 (NEEDS-OWNER:
-- the R2 bucket is owner-provisioned), and reports a finished manifest ref.
--
-- PUBLIC-SAFE / PROJECTION INVARIANT: every column here is public-safe. A job
-- references a published replay bundle slug/ref or a public activity-timeline
-- cursor range, a bounded render spec, a bounded camera-path DSL value, public
-- source refs, public caveat/blocker refs, and (once finished) a public
-- manifest URL. No raw traces, prompts, seeds, provider material, payout
-- targets, invoices, preimages, tokens, wallet material, mnemonics, local
-- filesystem paths, or customer-private data belong in any column.
--
-- AUTHORITY BOUNDARY: clip jobs are observation/projection records only. They
-- grant no settlement, payout, deployment, accepted-work, provider, wallet, or
-- public-claim authority.

CREATE TABLE IF NOT EXISTS replay_clip_jobs (
  job_ref TEXT PRIMARY KEY NOT NULL,
  status TEXT NOT NULL CHECK (
    status IN ('queued', 'rendering', 'succeeded', 'failed', 'blocked')
  ),
  -- The clip-job request as submitted (openagents.replay_clip_job.v1), JSON.
  request_json TEXT NOT NULL,
  -- Public source refs (JSON array of strings).
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  -- Public caveat refs (JSON array of strings).
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  -- Typed blocker refs for blocked/failed states (JSON array of strings).
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  -- Public manifest URL once succeeded; null otherwise.
  manifest_ref TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS replay_clip_jobs_status_idx
  ON replay_clip_jobs(status, updated_at DESC);

CREATE INDEX IF NOT EXISTS replay_clip_jobs_updated_idx
  ON replay_clip_jobs(updated_at DESC);
