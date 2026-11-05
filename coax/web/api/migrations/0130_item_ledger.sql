-- The inventory and reward ledger in the database (#12849): the Postgres form of the transactional
-- owner aethyr/session/ledger.coax models (#authority, its checks and mutants), one statement a
-- mutation, so every check and its change commit together or not at all. Game items only: nothing
-- here is money or redeemable for money.
--
-- One owner a fact: verse.items (each item: its kind, owner and escrow), verse.inventories (each
-- character's version), verse.item_ops (each operation ID, its payload and its recorded result),
-- verse.item_outbox (one entry a change, in the same transaction). A character is a player's
-- coalesce(account, id), as room_vitals keys it; its owner generation is verse.room_owners' (0127).
--
-- verse.item_apply(ID, OP, PID, GEN, VER, ITEM, TO, KIND, SENT_MS) -> (status, item, replay):
--   OP grant | transfer | escrow | settle | cancel. The status is ledger.coax's word:
--   ok; conflict (the ID was used for another payload); stale (GEN below the character's newest
--   owner generation); version (VER isn't the inventory's version); held (the item is escrowed or
--   isn't the character's, or TO is no one or itself); expired (first sent ITEM_KEEP_MS ago or more:
--   its first send may have applied and been forgotten); no_item.
--   The same ID and payload again answers its first result (replay true), refusals too. An ID is
--   kept ITEM_KEEP_MS (a day), then forgotten.
-- POST /api/playtest/room-items (player.c), signed with the room key as vitals are, takes one mutation as
-- JSON and answers its status. verse.item_reconcile() replays the outbox onto empty state and says
-- whether it gives the same items, owners and versions (the restore audit).
-- Additive, and safe to run twice.

CREATE OR REPLACE FUNCTION verse.item_keep_ms() RETURNS bigint LANGUAGE sql IMMUTABLE AS $$ SELECT 86400000::bigint $$;

CREATE TABLE IF NOT EXISTS verse.items (
  id bigserial PRIMARY KEY,
  kind int NOT NULL,
  owner text NOT NULL,
  escrow_to text
);
CREATE INDEX IF NOT EXISTS items_owner ON verse.items (owner);
CREATE TABLE IF NOT EXISTS verse.inventories (
  who text PRIMARY KEY,
  version int NOT NULL DEFAULT 0
);
CREATE TABLE IF NOT EXISTS verse.item_ops (
  id text PRIMARY KEY,
  op text NOT NULL, who text NOT NULL, item bigint, to_who text, kind int,
  result text NOT NULL, made bigint,
  at_ms bigint NOT NULL
);
CREATE TABLE IF NOT EXISTS verse.item_outbox (
  seq bigserial PRIMARY KEY,
  op text NOT NULL, who text NOT NULL, item bigint NOT NULL, to_who text, kind int,
  at_ms bigint NOT NULL
);

-- A player's character (coalesce(account, id)), or NULL.
CREATE OR REPLACE FUNCTION verse.item_who(p text) RETURNS text LANGUAGE sql STABLE AS $$
  SELECT coalesce(account, id) FROM verse.players WHERE id = p $$;

-- The change itself, shared by apply and the reconciliation's replay: on the given tables' rows.
CREATE OR REPLACE FUNCTION verse.item_apply(p_id text, p_op text, p_pid text, p_gen bigint, p_ver int, p_item bigint,
                                            p_to text, p_kind int, p_sent bigint,
                                            OUT status text, OUT item bigint, OUT replay boolean)
LANGUAGE plpgsql AS $$
DECLARE
  now bigint := (extract(epoch FROM clock_timestamp()) * 1000)::bigint;
  who_ text := verse.item_who(p_pid);
  to_ text := CASE WHEN p_to IS NULL OR p_to = '' THEN NULL ELSE verse.item_who(p_to) END;
  o verse.item_ops;
  m verse.items;
  newest bigint;
  v int;
BEGIN
  replay := false;
  IF p_op NOT IN ('grant', 'transfer', 'escrow', 'settle', 'cancel') OR p_id IS NULL OR length(p_id) NOT BETWEEN 1 AND 80 THEN status := 'held'; RETURN; END IF;
  IF who_ IS NULL THEN status := 'held'; RETURN; END IF;
  -- one writer an operation ID at a time, and the character's inventory row locked (versions)
  PERFORM pg_advisory_xact_lock(7731029, hashtext(p_id));
  DELETE FROM verse.item_ops x WHERE x.id = p_id AND x.at_ms <= now - verse.item_keep_ms();
  SELECT * INTO o FROM verse.item_ops x WHERE x.id = p_id;
  IF FOUND THEN
    IF o.op <> p_op OR o.who <> who_ OR o.item IS DISTINCT FROM p_item OR o.to_who IS DISTINCT FROM to_ OR o.kind IS DISTINCT FROM p_kind THEN
      status := 'conflict'; RETURN;
    END IF;
    status := o.result; item := o.made; replay := true; RETURN;
  END IF;
  IF p_sent IS NULL OR now - p_sent >= verse.item_keep_ms() THEN status := 'expired'; RETURN; END IF;
  INSERT INTO verse.inventories (who) VALUES (who_) ON CONFLICT DO NOTHING;
  IF to_ IS NOT NULL THEN INSERT INTO verse.inventories (who) VALUES (to_) ON CONFLICT DO NOTHING; END IF;
  -- lock both inventories in one order, so two transfers between the same pair can't deadlock
  PERFORM 1 FROM verse.inventories x WHERE x.who IN (who_, to_) ORDER BY x.who FOR UPDATE;
  SELECT x.version INTO v FROM verse.inventories x WHERE x.who = who_;
  SELECT x.gen INTO newest FROM verse.room_owners x WHERE x.who = who_;
  status := 'ok';
  IF newest IS NOT NULL AND (p_gen IS NULL OR p_gen < newest) THEN status := 'stale';
  ELSIF p_ver IS DISTINCT FROM v THEN status := 'version';
  ELSIF p_op <> 'grant' THEN
    SELECT * INTO m FROM verse.items x WHERE x.id = p_item FOR UPDATE;
    IF NOT FOUND THEN status := 'no_item';
    ELSIF m.owner <> who_ THEN status := 'held';
    ELSIF p_op IN ('transfer', 'escrow') AND (m.escrow_to IS NOT NULL OR to_ IS NULL OR to_ = who_) THEN status := 'held';
    ELSIF p_op IN ('settle', 'cancel') AND m.escrow_to IS NULL THEN status := 'held';
    END IF;
  END IF;
  IF status = 'ok' THEN
    IF p_op = 'grant' THEN
      INSERT INTO verse.items (kind, owner) VALUES (coalesce(p_kind, 0), who_) RETURNING id INTO item;
      UPDATE verse.inventories x SET version = x.version + 1 WHERE x.who = who_;
    ELSIF p_op = 'transfer' THEN
      item := p_item;
      UPDATE verse.items x SET owner = to_ WHERE x.id = p_item;
      UPDATE verse.inventories x SET version = x.version + 1 WHERE x.who IN (who_, to_);
    ELSIF p_op = 'escrow' THEN
      item := p_item;
      UPDATE verse.items x SET escrow_to = to_ WHERE x.id = p_item;
      UPDATE verse.inventories x SET version = x.version + 1 WHERE x.who = who_;
    ELSIF p_op = 'settle' THEN
      item := p_item; to_ := m.escrow_to;
      UPDATE verse.items x SET owner = m.escrow_to, escrow_to = NULL WHERE x.id = p_item;
      UPDATE verse.inventories x SET version = x.version + 1 WHERE x.who IN (who_, m.escrow_to);
    ELSE
      item := p_item;
      UPDATE verse.items x SET escrow_to = NULL WHERE x.id = p_item;
      UPDATE verse.inventories x SET version = x.version + 1 WHERE x.who = who_;
    END IF;
    INSERT INTO verse.item_outbox (op, who, item, to_who, kind, at_ms) VALUES (p_op, who_, item, to_, p_kind, now);
  END IF;
  -- a refusal is recorded too, so a retry hears the same answer; the payload as asked (to: its character)
  INSERT INTO verse.item_ops (id, op, who, item, to_who, kind, result, made, at_ms)
  VALUES (p_id, p_op, who_, p_item, CASE WHEN p_to IS NULL OR p_to = '' THEN NULL ELSE verse.item_who(p_to) END, p_kind, status,
          CASE WHEN p_op = 'grant' AND status = 'ok' THEN item END, now);
END $$;

-- Restore audit: the outbox replayed in order onto empty state gives the same items, owners,
-- escrows and versions as the tables hold.
CREATE OR REPLACE FUNCTION verse.item_reconcile() RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE e verse.item_outbox; ok boolean;
BEGIN
  CREATE TEMP TABLE IF NOT EXISTS item_r (id bigint PRIMARY KEY, kind int, owner text, escrow_to text) ON COMMIT DROP;
  CREATE TEMP TABLE IF NOT EXISTS inv_r (who text PRIMARY KEY, version int) ON COMMIT DROP;
  DELETE FROM item_r; DELETE FROM inv_r;
  FOR e IN SELECT * FROM verse.item_outbox ORDER BY seq LOOP
    INSERT INTO inv_r VALUES (e.who, 0) ON CONFLICT DO NOTHING;
    IF e.to_who IS NOT NULL THEN INSERT INTO inv_r VALUES (e.to_who, 0) ON CONFLICT DO NOTHING; END IF;
    IF e.op = 'grant' THEN INSERT INTO item_r VALUES (e.item, coalesce(e.kind, 0), e.who, NULL); UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSIF e.op = 'transfer' THEN UPDATE item_r SET owner = e.to_who WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who IN (e.who, e.to_who);
    ELSIF e.op = 'escrow' THEN UPDATE item_r SET escrow_to = e.to_who WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSIF e.op = 'settle' THEN UPDATE item_r SET owner = e.to_who, escrow_to = NULL WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who IN (e.who, e.to_who);
    ELSE UPDATE item_r SET escrow_to = NULL WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    END IF;
  END LOOP;
  SELECT NOT EXISTS (SELECT id, kind, owner, escrow_to FROM verse.items EXCEPT SELECT * FROM item_r)
     AND NOT EXISTS (SELECT * FROM item_r EXCEPT SELECT id, kind, owner, escrow_to FROM verse.items)
     AND NOT EXISTS (SELECT who, version FROM verse.inventories WHERE version > 0 EXCEPT SELECT * FROM inv_r)
     AND NOT EXISTS (SELECT * FROM inv_r EXCEPT SELECT who, version FROM verse.inventories) INTO ok;
  RETURN ok;
END $$;

-- The outbox after sequence AFTER, N entries at most, in order: a consumer keeps the last sequence
-- it applied and asks from there, so a redelivered entry (a sequence it has) is skipped.
CREATE OR REPLACE FUNCTION verse.item_outbox_since(after bigint, n int) RETURNS SETOF verse.item_outbox
LANGUAGE sql STABLE AS $$ SELECT * FROM verse.item_outbox WHERE seq > after ORDER BY seq LIMIT least(greatest(n, 0), 500) $$;
