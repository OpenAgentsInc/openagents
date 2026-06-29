-- Trace upload data market (openagents #6221, epic #6206). ADDITIVE columns on
-- the landed agent trace store (#6208, migration 0228) so a signed-in user (web
-- session) or a registered agent (bearer) can upload a trace they OWN, grant or
-- withhold its use as training/eval data for Khala, and have an INERT
-- reward-eligibility marker recorded (economics TBD, owner-gated — no money
-- moves until armed).
--
-- Invariants (see apps/openagents.com/INVARIANTS.md "Agent Trace Store" and
-- "Trace Upload Data Market"):
-- * `training_consent` is the uploader's explicit grant to use the trace as
--   training/eval data for Khala. It DEFAULTS TO 0 (WITHHELD): consent is never
--   assumed, only captured. Surfaced public-safe in the read projection.
-- * `license` is an optional public-safe license label the uploader attaches
--   (e.g. a SPDX-style id). It carries no secret material.
-- * `content_digest` is a SHA-256 hex digest over the canonical public-safe
--   trace payload, used ONLY to dedup uploads per owner (a duplicate digest is
--   rejected and never earns a second reward). It is not a settlement digest.
-- * `reward_eligible` / `reward_amount_sats` are an INERT revshare STUB. The
--   amount stays NULL ("reward TBD"); these columns grant NO payout, settlement,
--   spend, or accepted-work authority and move no money. Eligibility is only
--   recorded when the data-market reward flag is armed AND consent was granted
--   AND the upload is not a duplicate.
-- * `upload_source` records whether the trace arrived via a registered-agent
--   bearer ('agent') or an authenticated user web session ('user_session').
ALTER TABLE agent_traces
    ADD COLUMN training_consent INTEGER NOT NULL DEFAULT 0
        CHECK (training_consent IN (0, 1));

ALTER TABLE agent_traces
    ADD COLUMN license TEXT;

ALTER TABLE agent_traces
    ADD COLUMN content_digest TEXT;

ALTER TABLE agent_traces
    ADD COLUMN reward_eligible INTEGER NOT NULL DEFAULT 0
        CHECK (reward_eligible IN (0, 1));

ALTER TABLE agent_traces
    ADD COLUMN reward_amount_sats INTEGER;

ALTER TABLE agent_traces
    ADD COLUMN upload_source TEXT NOT NULL DEFAULT 'agent'
        CHECK (upload_source IN ('agent', 'user_session'));

-- Per-owner content dedup: at most one stored trace per (owner, content_digest).
-- A repeated digest from the same owner is rejected at ingest (no double reward).
CREATE UNIQUE INDEX IF NOT EXISTS idx_agent_traces_owner_digest
    ON agent_traces (owner_user_id, content_digest)
    WHERE content_digest IS NOT NULL;
