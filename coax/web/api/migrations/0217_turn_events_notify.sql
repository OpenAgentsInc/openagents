-- Notify streaming listeners whenever a durable turn event row commits (#14088).
-- Zero policy in SQL: emits only the canonical turn id and sequence so the
-- non-blocking streaming plane wakes up without polling.
CREATE OR REPLACE FUNCTION chat.notify_turn_event_v2() RETURNS trigger
LANGUAGE plpgsql AS $$
BEGIN
  PERFORM pg_notify('oa_turn_events_v2', NEW.turn || '|' || NEW.sequence::text);
  RETURN NULL;
END;
$$;

CREATE TRIGGER turn_events_v2_notify
  AFTER INSERT ON chat.turn_events_v2
  FOR EACH ROW EXECUTE FUNCTION chat.notify_turn_event_v2();
