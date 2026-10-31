-- Orbit League (/realms/orbit-league) and Belt 2200 (/realms/belt-2200) joined the
-- analytics contract (coax/web/analytics-contract.json) on October 30 without
-- their rows here; test_health.py holds the two the same.
INSERT INTO analytics.contract (surface, ev) VALUES
  ('orbit-league', 'page-view'), ('orbit-league', 'world-enter'), ('orbit-league', 'load-engine'), ('orbit-league', 'world-leave'),
  ('belt-2200', 'page-view'), ('belt-2200', 'world-enter'), ('belt-2200', 'load-engine'), ('belt-2200', 'world-leave')
ON CONFLICT DO NOTHING;
