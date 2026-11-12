-- The item ledger's two new operations (#13291, AAA P4.2; aethyr/session/ledger.coax LOP_CONSUME and
-- LOP_CLAIM; docs/protocol/everglade-item-op-v1.md). Loot, quest rewards and what a character wears
-- now go through the ledger's route (player.c room_items): a consume uses an item up, and a claim
-- changes the character's record (verse.characters: a quest's stage, coin, XP, what is worn) with an
-- operation ID, once. The rules are Coax's (decision 0006); SQL stores.
--   - verse.item_ops.rider: the claim an operation carried ("QUEST,FROM,STAGE,COIN,XP,SLOT,WEAR"), part
--     of its payload: the same ID with another claim is a conflict. NULL on rows from before this
--     migration, which carried none.
--   - verse.item_reconcile, the restore audit, replays the two new operations: a consume removes its
--     item, and both advance the writer's version. A claim's outbox entry names item 0 (no item).
-- Additive, and safe to run twice.
ALTER TABLE verse.item_ops ADD COLUMN IF NOT EXISTS rider text;

CREATE OR REPLACE FUNCTION verse.item_reconcile() RETURNS boolean LANGUAGE plpgsql AS $$
DECLARE e verse.item_outbox; ok boolean;
BEGIN
  CREATE TEMP TABLE IF NOT EXISTS item_r (id bigint PRIMARY KEY, kind int, owner text, escrow_to text) ON COMMIT DROP;
  CREATE TEMP TABLE IF NOT EXISTS inv_r (who text PRIMARY KEY, version bigint) ON COMMIT DROP;
  DELETE FROM item_r; DELETE FROM inv_r;
  FOR e IN SELECT * FROM verse.item_outbox ORDER BY seq LOOP
    INSERT INTO inv_r VALUES (e.who, 0) ON CONFLICT DO NOTHING;
    IF e.to_who IS NOT NULL THEN INSERT INTO inv_r VALUES (e.to_who, 0) ON CONFLICT DO NOTHING; END IF;
    IF e.op = 'grant' THEN INSERT INTO item_r VALUES (e.item, coalesce(e.kind, 0), e.who, NULL); UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSIF e.op = 'transfer' THEN UPDATE item_r SET owner = e.to_who WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who IN (e.who, e.to_who);
    ELSIF e.op = 'escrow' THEN UPDATE item_r SET escrow_to = e.to_who WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSIF e.op = 'settle' THEN UPDATE item_r SET owner = e.to_who, escrow_to = NULL WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who IN (e.who, e.to_who);
    ELSIF e.op = 'consume' THEN DELETE FROM item_r WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSIF e.op = 'claim' THEN UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    ELSE UPDATE item_r SET escrow_to = NULL WHERE id = e.item; UPDATE inv_r SET version = version + 1 WHERE who = e.who;
    END IF;
  END LOOP;
  SELECT NOT EXISTS (SELECT id, kind, owner, escrow_to FROM verse.items EXCEPT SELECT * FROM item_r)
     AND NOT EXISTS (SELECT * FROM item_r EXCEPT SELECT id, kind, owner, escrow_to FROM verse.items)
     AND NOT EXISTS (SELECT who, version FROM verse.inventories WHERE version > 0 EXCEPT SELECT * FROM inv_r)
     AND NOT EXISTS (SELECT * FROM inv_r EXCEPT SELECT who, version FROM verse.inventories) INTO ok;
  RETURN ok;
END $$;
