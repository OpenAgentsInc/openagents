-- A GameBuilder build agent is any person's own account's (GameBuilder issue 15; INVARIANTS.md "A
-- GameBuilder build agent is any person's own account's"). Its durable turn is admitted twice: when
-- it is started (POST /api/gamebuilder/agents/v1) and again when the hosted runner takes it up,
-- where no route or Origin is known. So the turn itself says what it was admitted for: '' (a
-- durable chat turn, the team's funding as before) or 'gamebuilder-agent'. The decision that reads
-- it is account-chat-policy.coax act_funded_turn. Every turn stored before this is ''.
ALTER TABLE chat.turns_v2 ADD COLUMN admitted_for text NOT NULL DEFAULT ''
  CONSTRAINT turns_v2_admitted_for_check CHECK (admitted_for IN ('', 'gamebuilder-agent'));
