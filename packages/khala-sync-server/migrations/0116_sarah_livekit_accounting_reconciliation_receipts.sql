CREATE TABLE IF NOT EXISTS sarah_livekit_accounting_reconciliations (
  reconciliation_ref                 text PRIMARY KEY,
  reconciliation_receipt_ref         text NOT NULL UNIQUE,
  session_ref                        text NOT NULL UNIQUE
    REFERENCES sarah_realtime_voice_sessions (session_ref),
  generation                         integer NOT NULL CHECK (generation > 0),
  reconciliation_payload_digest      text NOT NULL CHECK (
    reconciliation_payload_digest ~ '^[0-9a-f]{64}$'
  ),
  operator_actor_ref                  text NOT NULL,
  reconciliation_reason               text NOT NULL,
  provider_evidence_refs_json         jsonb NOT NULL CHECK (
    jsonb_typeof(provider_evidence_refs_json) = 'array'
  ),
  credit_rate_msat_per_million_tokens bigint NOT NULL CHECK (
    credit_rate_msat_per_million_tokens > 0
  ),
  created_at                          text NOT NULL
);
