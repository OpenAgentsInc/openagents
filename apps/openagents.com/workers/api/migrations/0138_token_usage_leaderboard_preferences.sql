CREATE TABLE IF NOT EXISTS token_usage_leaderboard_preferences (
  subject_kind TEXT NOT NULL CHECK (subject_kind IN ('account', 'team', 'user')),
  subject_ref TEXT NOT NULL,
  leaderboard_participation TEXT NOT NULL DEFAULT 'eligible'
    CHECK (leaderboard_participation IN ('eligible', 'opted_out')),
  leaderboard_visibility TEXT NOT NULL DEFAULT 'internal'
    CHECK (leaderboard_visibility IN ('internal', 'private')),
  updated_at TEXT NOT NULL,
  updated_by_user_id TEXT,
  PRIMARY KEY (subject_kind, subject_ref)
);

CREATE INDEX IF NOT EXISTS idx_token_usage_leaderboard_preferences_participation
  ON token_usage_leaderboard_preferences (
    subject_kind,
    leaderboard_participation,
    leaderboard_visibility
  );
