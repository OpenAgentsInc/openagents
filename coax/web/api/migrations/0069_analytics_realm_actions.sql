-- Every realm's actions (apps/openagents-web/analytics.coax track_act, track_once,
-- track_tally, track_walk; docs/verse/analytics-events.md lists them by realm)
-- and the iPhone app's own events (page 'app', OpenAgents/Platform/AppAnalytics.swift).
-- The ingest is unchanged: an action is an event like any other, and the
-- many-a-minute ones (spells, kills, fires) come as one 'tally' event every
-- 30 s whose properties are the counts. The Sandbox, the Grid, Everglade and
-- Fairy Forest join the analytics contract (coax/web/analytics-contract.json),
-- so a release stops when one of them stops sending its visit.
INSERT INTO analytics.contract (surface, ev) VALUES
  ('sandbox', 'page-view'), ('sandbox', 'world-enter'), ('sandbox', 'load-engine'), ('sandbox', 'world-leave'),
  ('the-grid', 'page-view'), ('the-grid', 'world-enter'), ('the-grid', 'load-engine'), ('the-grid', 'world-leave'),
  ('everglade', 'page-view'), ('everglade', 'world-enter'), ('everglade', 'load-engine'), ('everglade', 'world-leave'),
  ('fairy-forest', 'page-view'), ('fairy-forest', 'world-enter'), ('fairy-forest', 'load-engine'), ('fairy-forest', 'world-leave')
ON CONFLICT DO NOTHING;

-- The dashboard's realm cards read one page's events at a time.
CREATE INDEX IF NOT EXISTS events_page_ms ON analytics.events (page, ms);
