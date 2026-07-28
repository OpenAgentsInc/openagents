-- #9272: Managed Sarah Realtime voice sessions reserve credit before a
-- provider connection starts. A session settles the exact recorded charge and
-- releases the complete reservation in one transaction.

ALTER TABLE agent_balances
  ADD CONSTRAINT agent_balances_available_credit_check
  CHECK (balance_msat >= held_msat) NOT VALID;

CREATE TABLE IF NOT EXISTS sarah_realtime_voice_sessions (
  session_ref              text PRIMARY KEY,
  reservation_ref          text NOT NULL UNIQUE,
  owner_user_id             text NOT NULL,
  owner_actor_ref           text NOT NULL,
  device_ref                text NOT NULL,
  thread_ref                text NOT NULL,
  generation                integer NOT NULL CHECK (generation > 0),
  ticket_digest             text UNIQUE,
  disclosure_ref            text NOT NULL,
  state                     text NOT NULL CHECK (
    state IN ('reserved', 'connected', 'settled', 'released', 'failed')
  ),
  reserved_msat             bigint NOT NULL CHECK (reserved_msat > 0),
  charged_msat              bigint NOT NULL DEFAULT 0 CHECK (
    charged_msat >= 0 AND charged_msat <= reserved_msat
  ),
  input_tokens              bigint NOT NULL DEFAULT 0 CHECK (input_tokens >= 0),
  output_tokens             bigint NOT NULL DEFAULT 0 CHECK (output_tokens >= 0),
  cached_input_tokens       bigint NOT NULL DEFAULT 0 CHECK (cached_input_tokens >= 0),
  audio_input_tokens        bigint NOT NULL DEFAULT 0 CHECK (audio_input_tokens >= 0),
  audio_output_tokens       bigint NOT NULL DEFAULT 0 CHECK (audio_output_tokens >= 0),
  ticket_expires_at         text NOT NULL,
  session_expires_at        text NOT NULL,
  created_at                text NOT NULL,
  updated_at                text NOT NULL,
  connected_at              text,
  settled_at                text,
  close_reason              text,
  settlement_receipt_ref    text UNIQUE
);

CREATE UNIQUE INDEX IF NOT EXISTS sarah_realtime_voice_owner_active_idx
  ON sarah_realtime_voice_sessions (owner_user_id)
  WHERE state IN ('reserved', 'connected');

CREATE INDEX IF NOT EXISTS sarah_realtime_voice_expiry_idx
  ON sarah_realtime_voice_sessions (state, session_expires_at)
  WHERE state IN ('reserved', 'connected');

CREATE TABLE IF NOT EXISTS sarah_realtime_voice_usage (
  session_ref              text NOT NULL,
  provider_response_ref    text NOT NULL,
  input_tokens             bigint NOT NULL CHECK (input_tokens >= 0),
  output_tokens            bigint NOT NULL CHECK (output_tokens >= 0),
  cached_input_tokens      bigint NOT NULL CHECK (cached_input_tokens >= 0),
  audio_input_tokens       bigint NOT NULL CHECK (audio_input_tokens >= 0),
  audio_output_tokens      bigint NOT NULL CHECK (audio_output_tokens >= 0),
  charge_msat              bigint NOT NULL CHECK (charge_msat >= 0),
  observed_at              text NOT NULL,
  PRIMARY KEY (session_ref, provider_response_ref)
);

CREATE INDEX IF NOT EXISTS sarah_realtime_voice_usage_session_idx
  ON sarah_realtime_voice_usage (session_ref, observed_at);
