CREATE INDEX IF NOT EXISTS idx_forum_l402_challenges_actor_action_created
  ON forum_l402_challenges(actor_ref, action_kind, created_at DESC)
  WHERE archived_at IS NULL;
