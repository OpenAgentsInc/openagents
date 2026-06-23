-- 0224_autopilot_onboarding_sessions.sql
--
-- Server-side onboarding program session store (EPIC #6123, issue #6126).
--
-- The Khala onboarding program drives the productized business intake interview
-- (docs/business/2026-06-20-openagents-business-intake-spec.md) over the
-- OpenAgents OpenAI-compatible inference gateway (`/v1/chat/completions`,
-- model `openagents/khala-mini`). Each onboarding SESSION persists its full
-- transcript plus the accumulated 10-section Output Spec so a multi-turn
-- conversation can advance across requests and the structured artifact can be
-- consumed by later stages.
--
-- Conventions (match recent migrations, e.g. 0218_crm_contacts.sql,
-- 0221_khala_acceptance_verdicts.sql):
--   * TEXT primary keys (compactRandomId refs), ISO-8601 TEXT timestamps.
--   * enums via CHECK constraints; JSON as *_json TEXT.
--
-- This migration is data-model only. The turn route and session program ship in
-- the same issue and write/read these rows.

CREATE TABLE IF NOT EXISTS autopilot_onboarding_sessions (
  -- Caller-chosen session id (the `{sessionId}` path param). A turn against an
  -- unknown id creates the session on first use, so this is the only key.
  id TEXT PRIMARY KEY NOT NULL,
  -- Optional vertical overlay slot. When set, the program injects extra
  -- vertical guidance into the system prompt (used later by /autopilot/legal).
  vertical_overlay TEXT,
  -- Lifecycle state. `interviewing` while areas remain unasked; `complete` once
  -- the program has landed on a quick win + relationship picture and the Output
  -- Spec is fully populated.
  status TEXT NOT NULL DEFAULT 'interviewing'
    CHECK (status IN ('interviewing', 'complete')),
  -- Full chat transcript as a JSON array of { role, content } turns. The system
  -- prompt is NOT stored here (it is rebuilt deterministically every turn from
  -- the intake spec + live promise registry + overlay); only user/assistant
  -- turns persist so the conversation can resume.
  transcript_json TEXT NOT NULL DEFAULT '[]',
  -- Accumulated 10-section Output Spec, one nullable field per section as the
  -- interview fills it in. A partial spec is valid mid-interview.
  output_spec_json TEXT NOT NULL DEFAULT '{}',
  -- Monotonic turn counter (number of completed user->assistant exchanges).
  turn_count INTEGER NOT NULL DEFAULT 0,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS autopilot_onboarding_sessions_updated_idx
  ON autopilot_onboarding_sessions(updated_at DESC);
