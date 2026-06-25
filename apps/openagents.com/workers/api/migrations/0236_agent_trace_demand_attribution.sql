-- #6298: public-safe demand-origin segmentation for captured traces.
-- Nullable so existing traces remain readable and uploads without demand
-- metadata keep their historical behaviour.
ALTER TABLE agent_traces
  ADD COLUMN demand_kind TEXT
  CHECK (demand_kind IN ('external', 'internal', 'own_capacity', 'unlabeled'));

ALTER TABLE agent_traces
  ADD COLUMN demand_source TEXT;

CREATE INDEX IF NOT EXISTS idx_agent_traces_owner_demand_created
  ON agent_traces (owner_user_id, demand_kind, created_at DESC);
