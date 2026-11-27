-- Unified Cloud Chat Store & Live Sync (#14093):
--   1. Adds provenance, metering (compute cloud|local, cost_msat, device, workspace),
--      attachments metadata, and sidebar flags (pinned/archived) to chat.conversations_v2
--      and chat.turns_v2.
--   2. Adds monotonic conversation stream cursor (event_id, conversation) on chat.turn_events_v2
--      plus an account-level sidebar feed table (chat.account_events_v2) and idempotent
--      sync operation receipts (chat.sync_operations_v2).
--   3. Extends LISTEN/NOTIFY triggers with zero SQL policy so oa-api's non-blocking streaming
--      plane fans out conversation and account sidebar events without polling.

ALTER TABLE chat.conversations_v2
  ADD COLUMN source_device text NOT NULL DEFAULT '',
  ADD COLUMN workspace text NOT NULL DEFAULT '',
  ADD COLUMN pinned boolean NOT NULL DEFAULT false,
  ADD COLUMN archived boolean NOT NULL DEFAULT false;

CREATE INDEX conversations_v2_sidebar ON chat.conversations_v2(account, archived, pinned DESC, updated_ms DESC, id);

ALTER TABLE chat.turns_v2
  ADD COLUMN compute text NOT NULL DEFAULT 'cloud' CHECK (compute IN ('cloud', 'local')),
  ADD COLUMN cost_msat bigint NOT NULL DEFAULT 0 CHECK (cost_msat >= 0 AND (compute <> 'local' OR cost_msat = 0)),
  ADD COLUMN source_device text NOT NULL DEFAULT '',
  ADD COLUMN workspace text NOT NULL DEFAULT '',
  ADD COLUMN attachments_json text NOT NULL DEFAULT '[]';

ALTER TABLE chat.turn_events_v2
  ADD COLUMN event_id bigserial NOT NULL,
  ADD COLUMN conversation text NOT NULL DEFAULT '';

UPDATE chat.turn_events_v2 e
SET conversation = t.conversation
FROM chat.turns_v2 t
WHERE t.account = e.account AND t.id = e.turn AND e.conversation = '';

CREATE UNIQUE INDEX turn_events_v2_event_id ON chat.turn_events_v2(event_id);
CREATE INDEX turn_events_v2_conv_stream ON chat.turn_events_v2(account, conversation, event_id);

CREATE TABLE chat.account_events_v2 (
  id bigserial PRIMARY KEY,
  account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
  conversation text NOT NULL DEFAULT '',
  event_type text NOT NULL,
  record text NOT NULL,
  created_ms bigint NOT NULL
);

CREATE INDEX account_events_v2_stream ON chat.account_events_v2(account, id);

CREATE TABLE chat.sync_operations_v2 (
  account text NOT NULL REFERENCES auth.accounts(id) ON UPDATE CASCADE ON DELETE CASCADE,
  operation text NOT NULL,
  digest text NOT NULL,
  response text NOT NULL,
  created_ms bigint NOT NULL,
  PRIMARY KEY (account, operation)
);

INSERT INTO auth.life_columns(tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('chat.account_events_v2', 'account', 'account', 'move', 'delete', true, '{}', 'live account sidebar feed events for multi-device cloud chat sync'),
  ('chat.sync_operations_v2', 'account', 'account', 'move', 'delete', true, '{}', 'idempotent write-through cloud chat sync receipts');

CREATE OR REPLACE FUNCTION chat.notify_turn_event_v2() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM pg_notify('oa_turn_events_v2', NEW.turn || '|' || NEW.sequence::text);
  IF NEW.conversation <> '' THEN
    PERFORM pg_notify('oa_conv_events_v2', NEW.conversation || '|' || NEW.event_id::text);
  END IF;
  RETURN NULL;
END;
$$;

CREATE OR REPLACE FUNCTION chat.notify_account_event_v2() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM pg_notify('oa_account_events_v2', NEW.account || '|' || NEW.id::text);
  RETURN NULL;
END;
$$;

CREATE TRIGGER account_events_v2_notify
  AFTER INSERT ON chat.account_events_v2
  FOR EACH ROW EXECUTE FUNCTION chat.notify_account_event_v2();
