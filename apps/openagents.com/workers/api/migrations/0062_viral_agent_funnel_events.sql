CREATE TABLE IF NOT EXISTS viral_agent_funnel_events (
  id TEXT PRIMARY KEY NOT NULL,
  event_kind TEXT NOT NULL CHECK (
    event_kind IN (
      'capability_manifest_read',
      'openapi_read',
      'agent_doc_read',
      'skill_doc_read',
      'public_proof_read',
      'public_challenge_read',
      'first_scoped_action_attempt'
    )
  ),
  route TEXT NOT NULL,
  actor_class TEXT NOT NULL CHECK (
    actor_class IN (
      'public_anonymous',
      'signed_in_browser_possible',
      'scoped_agent_possible'
    )
  ),
  user_agent_class TEXT NOT NULL CHECK (
    user_agent_class IN (
      'agent_or_cli',
      'browser',
      'crawler',
      'unknown'
    )
  ),
  site_slug TEXT,
  proof_ref TEXT,
  metadata_json TEXT NOT NULL DEFAULT '{}',
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS viral_agent_funnel_events_kind_created_idx
  ON viral_agent_funnel_events(event_kind, created_at DESC);

CREATE INDEX IF NOT EXISTS viral_agent_funnel_events_route_created_idx
  ON viral_agent_funnel_events(route, created_at DESC);

CREATE INDEX IF NOT EXISTS viral_agent_funnel_events_site_created_idx
  ON viral_agent_funnel_events(site_slug, created_at DESC)
  WHERE site_slug IS NOT NULL;
