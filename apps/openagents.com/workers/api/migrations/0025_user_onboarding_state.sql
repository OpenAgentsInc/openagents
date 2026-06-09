ALTER TABLE users
  ADD COLUMN onboarding_step TEXT NOT NULL DEFAULT 'repository'
    CHECK (onboarding_step IN ('repository', 'billing', 'goal', 'complete'));

ALTER TABLE users
  ADD COLUMN onboarding_completed_at TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_provider TEXT
    CHECK (
      onboarding_repository_provider IS NULL
      OR onboarding_repository_provider IN ('github')
    );

ALTER TABLE users
  ADD COLUMN onboarding_repository_id TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_owner TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_name TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_full_name TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_private INTEGER
    CHECK (
      onboarding_repository_private IS NULL
      OR onboarding_repository_private IN (0, 1)
    );

ALTER TABLE users
  ADD COLUMN onboarding_repository_default_branch TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_html_url TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_description TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_selected_at TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_repository_skipped_at TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_billing_skipped_at TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_goal TEXT;

ALTER TABLE users
  ADD COLUMN onboarding_updated_at TEXT;

CREATE INDEX users_onboarding_step_idx
  ON users(onboarding_step, onboarding_completed_at);
