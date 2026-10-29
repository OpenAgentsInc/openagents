-- Ice Cream Man (/realm/ice-cream-man) joins the analytics contract
-- (coax/web/analytics-contract.json): the realm's page sends these on every
-- visit; its rounds add round-start, objective (a key) and round-end.
INSERT INTO analytics.contract (surface, ev) VALUES
  ('ice-cream-man', 'page-view'), ('ice-cream-man', 'world-enter'), ('ice-cream-man', 'load-engine'),
  ('ice-cream-man', 'world-leave')
ON CONFLICT DO NOTHING;
