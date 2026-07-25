-- First-party web analytics V1. This table stores only the closed portable
-- event contract plus trusted server receipt time. It does not store an IP
-- address, user agent, cookie, user identifier, referrer, query string, or
-- arbitrary property bag.

CREATE TABLE IF NOT EXISTS web_analytics_events (
  event_id      text PRIMARY KEY,
  event_name    text NOT NULL,
  client_kind   text NOT NULL,
  route_id      text NOT NULL,
  occurred_at   text NOT NULL,
  received_at   text NOT NULL,
  schema_version text NOT NULL,
  CONSTRAINT web_analytics_events_name
    CHECK (event_name IN ('page_view', 'github_view')),
  CONSTRAINT web_analytics_events_client
    CHECK (client_kind IN ('web', 'mobile', 'desktop')),
  CONSTRAINT web_analytics_events_route
    CHECK (route_id ~ '^/[A-Za-z0-9_./:-]{0,119}$'),
  CONSTRAINT web_analytics_events_schema
    CHECK (schema_version = 'openagents.analytics.events.v1')
);

CREATE INDEX IF NOT EXISTS web_analytics_events_received_idx
  ON web_analytics_events(received_at DESC);

CREATE INDEX IF NOT EXISTS web_analytics_events_name_received_idx
  ON web_analytics_events(event_name, received_at DESC);

CREATE INDEX IF NOT EXISTS web_analytics_events_route_received_idx
  ON web_analytics_events(route_id, received_at DESC);
