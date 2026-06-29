-- Artanis Pylon-support responder: record the asking actor and its bounded
-- provenance class on each responder action (promise
-- artanis.pylon_support_responder.v1, blocker
-- external_contributor_flow_unproven). The scan stage already reads the
-- candidate topic's actor_ref but never persisted it, so there was no
-- machine-auditable way to tell whether the responder had answered a real
-- EXTERNAL contributor (a non-owner, non-operator, non-Artanis identity)
-- versus an operator-authored test article. These columns make the
-- external-contributor end-to-end flow projectable and dereferenceable.
--
-- Public-safe identity fields only: the actor ref (already public on the
-- Forum post) and the bounded provenance enum. No private content.

ALTER TABLE artanis_responder_actions ADD COLUMN asker_actor_ref TEXT;
ALTER TABLE artanis_responder_actions ADD COLUMN asker_provenance TEXT;

CREATE INDEX IF NOT EXISTS idx_artanis_responder_actions_provenance
  ON artanis_responder_actions (asker_provenance, state, created_at DESC);
