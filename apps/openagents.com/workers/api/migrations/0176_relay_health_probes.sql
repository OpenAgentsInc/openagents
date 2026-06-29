-- Public relay health monitoring (openagents#4865).
--
-- Orrery's spend probe (topic 499cec6e post 7be6aa0a) hit a relay outage
-- (HTTP 530, refused websocket upgrades, 20:33-20:35Z) that left no public
-- trace because nothing retained probe outcomes. These tables retain the
-- scheduled NIP-11 + websocket REQ/EOSE probe results for the canonical
-- market relay plus the typed healthy<->unhealthy transition events, so a
-- short outage stays publicly citable after recovery.
--
-- Retention is bounded by the scheduled prune in
-- workers/api/src/relay-health.ts (probes 7 days, transitions 30 days).

CREATE TABLE IF NOT EXISTS relay_health_probes (
  id TEXT PRIMARY KEY,
  relay_url TEXT NOT NULL,
  probed_at TEXT NOT NULL,
  nip11_outcome TEXT NOT NULL,
  nip11_http_status INTEGER,
  nip11_latency_ms INTEGER,
  nip11_relay_name TEXT,
  ws_outcome TEXT NOT NULL,
  ws_latency_ms INTEGER,
  status TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relay_health_probes_relay_probed_at
  ON relay_health_probes(relay_url, probed_at DESC);

CREATE TABLE IF NOT EXISTS relay_health_transitions (
  id TEXT PRIMARY KEY,
  relay_url TEXT NOT NULL,
  occurred_at TEXT NOT NULL,
  kind TEXT NOT NULL,
  from_status TEXT NOT NULL,
  to_status TEXT NOT NULL,
  probe_id TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_relay_health_transitions_relay_occurred_at
  ON relay_health_transitions(relay_url, occurred_at DESC);
