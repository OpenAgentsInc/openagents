-- Character checkpoint profile 3 (docs/protocol/character-checkpoint-v3.md, #13046):
-- profile 2's body plus the character's rest, 19 integers. Both profiles share
-- profile 2's table, receipts and ownership fences; a page that asks without
-- rest=1 is given a profile 3 body's profile 2 part.
ALTER TABLE verse.character_checkpoints_v2 DROP CONSTRAINT IF EXISTS character_checkpoints_v2_body_check;
ALTER TABLE verse.character_checkpoints_v2 DROP CONSTRAINT IF EXISTS character_checkpoints_v2_body_profile;
ALTER TABLE verse.character_checkpoints_v2 ADD CONSTRAINT character_checkpoints_v2_body_profile CHECK (
  jsonb_typeof(body)='object' AND jsonb_typeof(body->'state')='array' AND jsonb_array_length(body->'state')=27
  AND ((body->>'v'='2' AND NOT body ? 'rest')
    OR (body->>'v'='3' AND jsonb_typeof(body->'rest')='array' AND jsonb_array_length(body->'rest')=19)));
