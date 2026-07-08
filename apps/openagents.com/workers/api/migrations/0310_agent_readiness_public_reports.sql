-- OB-3 (#8560): hosted, public-safe, tokenized agent-readiness reports.
--
-- One row per rendered LG-1/LG-5 report, keyed by an unguessable public
-- `report_token` (never a sequential or guessable id). The stored
-- `assessment_json` is the OB-3 15-step rubric projection
-- (`openagents.agent_readiness_fifteen_step_assessment.v1`) built by
-- `renderAgentReadinessFifteenStepAssessment` in `@openagentsinc/agent-readiness` —
-- it only ever contains facts about the prospect's OWN public domain
-- (their own score, their own public findings, their own evidence refs
-- pointing at their own site). `pipeline_ref` and `source_ref` are internal
-- CRM/attribution refs and are never returned from the public read route;
-- they exist only so a report click can be attributed back to the funnel
-- via `business_funnel_events` (LG-6 source-ref convention).
--
-- No prospect name, contact email, Apollo enrichment payload, quoted deal
-- band, or vertical assignment is stored in this table.

CREATE TABLE IF NOT EXISTS agent_readiness_public_reports (
  id TEXT PRIMARY KEY NOT NULL,
  report_token TEXT NOT NULL UNIQUE,
  pipeline_ref TEXT NOT NULL,
  source_ref TEXT NOT NULL,
  domain TEXT NOT NULL,
  score INTEGER NOT NULL,
  grade TEXT NOT NULL CHECK (grade IN ('A', 'B', 'C', 'D', 'F')),
  assessment_json TEXT NOT NULL,
  click_count INTEGER NOT NULL DEFAULT 0 CHECK (click_count >= 0),
  last_clicked_at TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_agent_readiness_public_reports_pipeline
  ON agent_readiness_public_reports(pipeline_ref, created_at);

CREATE INDEX IF NOT EXISTS idx_agent_readiness_public_reports_domain
  ON agent_readiness_public_reports(domain, created_at);
