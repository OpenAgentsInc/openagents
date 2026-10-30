-- Hecate's Cemetery (/realms/hecates-cemetery, Planetary Villa) joins the
-- analytics contract (coax/web/analytics-contract.json): the realm's page sends
-- these on every visit; its steps add the villa-* events (villapet.coax EV_NAMES)
-- and villa-flight, which the dashboard's HECATE'S CEMETERY card counts.
INSERT INTO analytics.contract (surface, ev) VALUES
  ('hecates-cemetery', 'page-view'), ('hecates-cemetery', 'world-enter'), ('hecates-cemetery', 'load-engine'),
  ('hecates-cemetery', 'world-leave')
ON CONFLICT DO NOTHING;
