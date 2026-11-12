-- The characters context's outbox (#13319, DDD.8; proto/README.md "Domain events"): one row for each
-- domain event a character's record publishes, written in the transaction that changes the record.
-- The first event is characters.character.levelled.v1: the record's level rose (a seat ticket, an
-- accepted checkpoint, or a ledger operation's XP claim raised its XP past a level's threshold).
-- Whether the level rose is Coax's (everglade/model/events.coax ev_levelled, through
-- character-policy.coax); SQL stores the row. The event's bytes are the codec's
-- (everglade/model/events.coax), made from a row's columns.
-- A reader asks for the rows after the last sequence it took (verse.character_outbox_since), as the
-- item ledger's readers do (verse.item_outbox_since, 0130). Additive, and safe to run twice.
CREATE TABLE IF NOT EXISTS verse.character_outbox (
  seq bigserial PRIMARY KEY,
  event text NOT NULL,
  who text NOT NULL,
  level int NOT NULL CHECK (level BETWEEN 1 AND 20),
  xp bigint NOT NULL CHECK (xp >= 0),
  at_ms bigint NOT NULL CHECK (at_ms > 0)
);
CREATE OR REPLACE FUNCTION verse.character_outbox_since(after bigint, n int) RETURNS SETOF verse.character_outbox
LANGUAGE sql STABLE AS $$ SELECT * FROM verse.character_outbox WHERE seq > after ORDER BY seq LIMIT least(greatest(n, 0), 500) $$;
