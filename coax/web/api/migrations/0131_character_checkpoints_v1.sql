-- Inactive receiver for character-checkpoint-v1 (#12838). The signed receiver
-- in player.c locks room_owners, runs checkpoint-policy.coax, then commits with
-- synchronous_commit=on before replying. No legacy vitals or tickets read it.
CREATE TABLE IF NOT EXISTS verse.character_checkpoints_v1 (
  who text PRIMARY KEY,
  owner bigint NOT NULL CHECK (owner BETWEEN 1 AND 2147483647),
  process bigint NOT NULL CHECK (process BETWEEN 1 AND 9007199254740991),
  rev bigint NOT NULL CHECK (rev BETWEEN 1 AND 9007199254740991),
  body jsonb NOT NULL CHECK (jsonb_typeof(body) = 'object' AND body->>'v' = '1'),
  accepted_at bigint NOT NULL CHECK (accepted_at > 0)
);
