CREATE TABLE analytics.contract (
  surface text NOT NULL,
  ev text NOT NULL,
  PRIMARY KEY (surface, ev)
);
INSERT INTO analytics.contract (surface, ev) VALUES
  ('landing', 'session-start'), ('landing', 'page-view'),
  ('galaxy', 'page-view'),
  ('haunt', 'page-view'), ('haunt', 'world-enter'), ('haunt', 'load-engine'), ('haunt', 'area'),
  ('haunt', 'objective'), ('haunt', 'quest'), ('haunt', 'click'), ('haunt', 'world-leave');

CREATE FUNCTION analytics.health(now_ms bigint)
RETURNS TABLE (surface text, last_ms bigint, last_ev text, hour bigint, day bigint, missing text[]) LANGUAGE sql STABLE AS $$
  WITH recent AS (
    SELECT page, ev, ms FROM analytics.events WHERE space = 'web' AND ms >= now_ms - 30 * 86400000::bigint
  ), surfaces AS (
    SELECT DISTINCT c.surface FROM analytics.contract c
    UNION SELECT DISTINCT page FROM recent WHERE ms >= now_ms - 86400000 AND page <> ''
  )
  SELECT s.surface,
    (SELECT max(r.ms) FROM recent r WHERE r.page = s.surface),
    (SELECT r.ev FROM recent r WHERE r.page = s.surface ORDER BY r.ms DESC LIMIT 1),
    (SELECT count(*) FROM recent r WHERE r.page = s.surface AND r.ms >= now_ms - 3600000),
    (SELECT count(*) FROM recent r WHERE r.page = s.surface AND r.ms >= now_ms - 86400000),
    coalesce((SELECT array_agg(c.ev ORDER BY c.ev) FROM analytics.contract c WHERE c.surface = s.surface
      AND NOT EXISTS (SELECT 1 FROM recent r WHERE r.page = c.surface AND r.ev = c.ev AND r.ms >= now_ms - 86400000)), '{}')
  FROM surfaces s
$$;
