-- The item ledger has one rules owner (#13318; docs/decisions/0006-item-ledger-rules-in-coax.md):
-- aethyr/session/ledger.coax decides every ask, and player.c room_items (through item-policy.coax)
-- writes what it decides. The tables of 0130 stay, and so do their locks, the operation-ID key and
-- the outbox; the database no longer decides.
--   - verse.item_apply, the plpgsql copy of the rules, and verse.item_keep_ms, its keep time, go.
--     The keep time is ledger.coax's LEDGER_KEEP (a day, in ms).
--   - A character's inventory version is a bigint, as ledger.coax's is an int: a write that would
--     move it past the largest is refused (exhausted) by the rules before it changes.
-- verse.item_reconcile stays: the restore audit replays the outbox on its own and compares it with
-- the rows the rules wrote. verse.item_who (0130) names the character, coalesce(account, id).

DROP FUNCTION IF EXISTS verse.item_apply(text, text, text, bigint, int, bigint, text, int, bigint);
DROP FUNCTION IF EXISTS verse.item_keep_ms();
ALTER TABLE verse.inventories ALTER COLUMN version TYPE bigint;
