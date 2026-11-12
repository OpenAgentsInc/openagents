-- A made realm's moves and shapes stand in entity order, so an UNDO gives back the bytes it undid
-- (GitHub #13354).
--
-- Before this, `moved` and `shaped` (0093, 0109) kept a save's own order, and UNDO and REDO (0111)
-- put a slot back by replacing its entry where it stood or adding it at the end. A save that took
-- several shapes away and its undo gave the same entities back in another order: the same realm,
-- but other bytes and another SHA-256 than the revision it restored. Now every write keeps both
-- lists in one order: by feature (bytewise), then by index as a number, then by the name bytewise
-- ("rocks:2" before "rocks:10" before "trees:0"). A document is then one set of bytes for one realm,
-- so an undo's document is the revision it restored, byte for byte but for `rev`, and so is a redo's.
-- aethyr/forge/edit.coax keeps its overrides in the same order (forge_entity_before), and
-- proto/verse/realm-document-v1.json pins it: after each undo and redo, the document is the
-- earlier revision's text with only its `rev` changed.
--   Existing data: no row is rewritten. A realm keeps the order it has until its next save, undo or
--   redo, which writes the lists in this order (a save that only reorders makes a new revision and
--   logs no step: no slot changed). Kept revisions keep their bytes and hashes.

-- A list of {entity, ...} objects in entity order (NULL stays NULL).
CREATE FUNCTION verse.realm_by_entity(a jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT CASE WHEN a IS NULL THEN NULL ELSE coalesce((
    SELECT jsonb_agg(x ORDER BY split_part(x->>'entity', ':', 1) COLLATE "C",
                                CASE WHEN split_part(x->>'entity', ':', 2) ~ '^[0-9]{1,9}$' THEN split_part(x->>'entity', ':', 2)::int END,
                                x->>'entity' COLLATE "C")
    FROM jsonb_array_elements(a) x), '[]') END $$;

-- SAVE's moves (0093's) and shapes (0109's), held in entity order.
ALTER FUNCTION verse.realm_moves(jsonb) RENAME TO realm_moves_0093;
CREATE FUNCTION verse.realm_moves(v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.realm_by_entity(verse.realm_moves_0093(v)) $$;
ALTER FUNCTION verse.realm_shapes(jsonb) RENAME TO realm_shapes_0109;
CREATE FUNCTION verse.realm_shapes(v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.realm_by_entity(verse.realm_shapes_0109(v)) $$;

-- UNDO's and REDO's entity slot (0111's realm_entry): set, added or taken away, then in entity order.
ALTER FUNCTION verse.realm_entry(jsonb, text, jsonb) RENAME TO realm_entry_0111;
CREATE FUNCTION verse.realm_entry(a jsonb, e text, v jsonb) RETURNS jsonb LANGUAGE sql IMMUTABLE AS $$
  SELECT verse.realm_by_entity(verse.realm_entry_0111(a, e, v)) $$;
