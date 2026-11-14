-- The character record's v2 (#13519; docs/protocol/everglade-character-v2.md): v1 with the Archery
-- Range's lasting tally, `range`, 61 whole numbers. The record's rules stay Coax's
-- (everglade/model/character.coax, through coax/web/api/character-policy.coax); SQL checks the shape only.
-- A record whose tally is empty is still written as v1, byte for byte, so every record kept today
-- stays as it is: nothing is rewritten here. A v1 record is upgraded as it is read (char_read), and
-- written as v2 only once an arrow's claim gives it a tally.
ALTER TABLE verse.characters DROP CONSTRAINT IF EXISTS characters_body;
ALTER TABLE verse.characters ADD CONSTRAINT characters_body CHECK (jsonb_typeof(body) = 'object' AND body->>'who' = who
  AND jsonb_typeof(body->'known') = 'array' AND jsonb_typeof(body->'bag') = 'array'
  AND jsonb_typeof(body->'worn') = 'array' AND jsonb_array_length(body->'worn') = 10
  AND jsonb_typeof(body->'quests') = 'array'
  AND ((body->>'v' = '1' AND NOT body ? 'range')
    OR (body->>'v' = '2' AND jsonb_typeof(body->'range') = 'array' AND jsonb_array_length(body->'range') = 61)));
