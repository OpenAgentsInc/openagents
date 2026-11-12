-- Moderation (#13302, AAA P5.8; docs/protocol/moderation-v1.md): bans and mutes on the account, and
-- the log of every moderator action. The rules are Coax's (aethyr/session/moderation.coax, through
-- coax/web/api/moderation-policy.coax): player.c and room.c read these rows, ask the rules and write
-- what they answer. SQL stores the rows and keeps the log append-only.
--
--   verse.mod_log        one row an action: who did what to whom, why, when, from where. A room's
--                        report names each action (shard, gen, seq), kept once. No row is ever
--                        changed or removed: UPDATE, DELETE and TRUNCATE raise, for every role.
--   verse.mod_sanctions  the ban and the mute a character's owner has now (who: the account, else the
--                        player, as room_vitals keeps them). until_ms 0: no end.
-- Kicks stay in verse.room_kicks (0115), 24 hours, and every kick writes a log row too: an admin's
-- (player.c room_mod) and a report's HIDE (villa.report_hide, below).
-- `team` marks an action on one of the team's players (analytics.team), so counts can leave it out.
-- Additive.

CREATE TABLE IF NOT EXISTS verse.mod_log (
  n bigserial PRIMARY KEY,
  at_ms bigint NOT NULL CHECK (at_ms > 0),
  by_who text NOT NULL CHECK (by_who ~ '^[a-z0-9-]{1,40}$'),     -- the moderator's player id, 'site' (a site admin with no player key) or 'report'
  verb text NOT NULL CHECK (verb IN ('look', 'move', 'kick', 'mute', 'ban', 'unkick', 'unmute', 'unban')),
  whom text NOT NULL CHECK (whom ~ '^[a-z0-9-]{1,40}$'),         -- the player it is about
  who text NOT NULL,                                             -- whose sanction it is: their account, else the player
  minutes int NOT NULL CHECK (minutes BETWEEN 0 AND 525600),
  until_ms bigint NOT NULL CHECK (until_ms >= 0),                -- when what it set ends; 0: no end, or it set nothing
  reason text NOT NULL CHECK (length(reason) BETWEEN 1 AND 120),
  source text NOT NULL CHECK (source IN ('room', 'api', 'report')),
  room text,
  shard int, gen bigint, seq bigint,                             -- a room's operation ID
  outcome text NOT NULL CHECK (outcome IN ('ok', 'self', 'no_player')),          -- done, or why not
  team boolean NOT NULL DEFAULT false,
  CONSTRAINT mod_log_op UNIQUE (shard, gen, seq),
  CONSTRAINT mod_log_room_op CHECK ((source = 'room') = (shard IS NOT NULL AND gen IS NOT NULL AND seq IS NOT NULL AND room IS NOT NULL))
);
CREATE INDEX IF NOT EXISTS mod_log_whom ON verse.mod_log (whom, n);
CREATE INDEX IF NOT EXISTS players_account ON verse.players (account);   -- room-control finds a banned account's players

CREATE OR REPLACE FUNCTION verse.mod_log_frozen() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'verse.mod_log is append-only (docs/protocol/moderation-v1.md)' USING ERRCODE = 'insufficient_privilege';
END $$;
DROP TRIGGER IF EXISTS mod_log_frozen ON verse.mod_log;
CREATE TRIGGER mod_log_frozen BEFORE UPDATE OR DELETE ON verse.mod_log FOR EACH ROW EXECUTE FUNCTION verse.mod_log_frozen();
DROP TRIGGER IF EXISTS mod_log_kept ON verse.mod_log;
CREATE TRIGGER mod_log_kept BEFORE TRUNCATE ON verse.mod_log FOR EACH STATEMENT EXECUTE FUNCTION verse.mod_log_frozen();
REVOKE UPDATE, DELETE, TRUNCATE ON verse.mod_log FROM PUBLIC;

CREATE TABLE IF NOT EXISTS verse.mod_sanctions (
  who text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('ban', 'mute')),
  until_ms bigint NOT NULL CHECK (until_ms >= 0),
  reason text NOT NULL,
  by_who text NOT NULL,
  at_ms bigint NOT NULL,
  PRIMARY KEY (who, kind)
);

-- A report's HIDE and RESTORE of a player (0115): the same kick, and now its log row.
CREATE OR REPLACE FUNCTION villa.report_hide(kind_ text, ref_ text) RETURNS boolean LANGUAGE plpgsql AS $$
BEGIN
  IF kind_ <> 'player' THEN RETURN villa.report_hide_0034(kind_, ref_); END IF;
  IF ref_ !~ '^[0-9a-f]{16}$' OR NOT EXISTS (SELECT 1 FROM verse.players WHERE id = 'p-' || ref_) THEN RETURN false; END IF;
  INSERT INTO verse.room_kicks (player, by_who, until_ms, at_ms) VALUES ('p-' || ref_, 'report', verse.now_ms() + 86400000, verse.now_ms())
  ON CONFLICT (player) DO UPDATE SET by_who = 'report', until_ms = EXCLUDED.until_ms, at_ms = EXCLUDED.at_ms;
  INSERT INTO verse.mod_log (at_ms, by_who, verb, whom, who, minutes, until_ms, reason, source, outcome, team)
  SELECT verse.now_ms(), 'report', 'kick', p.id, coalesce(p.account, p.id), 0, verse.now_ms() + 86400000, 'a report hidden', 'report', 'ok',
         EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'player' AND t.value = p.id)
  FROM verse.players p WHERE p.id = 'p-' || ref_;
  RETURN true;
END $$;
CREATE OR REPLACE FUNCTION villa.report_restore(kind_ text, ref_ text) RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE n int;
BEGIN
  IF kind_ <> 'player' THEN RETURN villa.report_restore_0034(kind_, ref_); END IF;
  DELETE FROM verse.room_kicks WHERE ref_ ~ '^[0-9a-f]{16}$' AND player = 'p-' || ref_;
  GET DIAGNOSTICS n = ROW_COUNT;
  IF n > 0 THEN
    INSERT INTO verse.mod_log (at_ms, by_who, verb, whom, who, minutes, until_ms, reason, source, outcome, team)
    SELECT verse.now_ms(), 'report', 'unkick', p.id, coalesce(p.account, p.id), 0, 0, 'a report restored', 'report', 'ok',
           EXISTS (SELECT 1 FROM analytics.team t WHERE t.kind = 'player' AND t.value = p.id)
    FROM verse.players p WHERE p.id = 'p-' || ref_;
  END IF;
  RETURN n > 0;
END $$;
