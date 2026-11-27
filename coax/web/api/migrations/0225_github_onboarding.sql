-- Mandatory GitHub onboarding & $10 free credit (one per numeric GitHub ID).
--
-- Adds account-linking mode ('oauth_link' + link_account) to auth.workos_pending,
-- records per-account GitHub onboarding links in auth.github_links (non-unique on
-- github_id so multiple OpenAgents accounts can pass onboarding with the same GitHub
-- account, while referral.github_claims enforces strictly one $10 promo credit per
-- numeric GitHub ID), backfills existing GitHub claims, and updates github_connect_micros
-- from $5 (5000000) to $10 (10000000).

ALTER TABLE auth.workos_pending DROP CONSTRAINT workos_pending_kind_check;
ALTER TABLE auth.workos_pending ADD CONSTRAINT workos_pending_kind_check
  CHECK (kind IN ('magic', 'oauth', 'oauth_link', 'org'));

ALTER TABLE auth.workos_pending
  ADD COLUMN IF NOT EXISTS link_account text REFERENCES auth.accounts (id);

CREATE TABLE IF NOT EXISTS auth.github_links (
  account_id text PRIMARY KEY REFERENCES auth.accounts (id),
  github_id text NOT NULL CHECK (length(github_id) BETWEEN 1 AND 64),
  github_login text NOT NULL DEFAULT '',
  github_avatar text NOT NULL DEFAULT '',
  workos_user_id text NOT NULL DEFAULT '',
  linked_at timestamptz NOT NULL DEFAULT now(),
  updated_at timestamptz NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS auth_github_links_github_id_idx
  ON auth.github_links (github_id);

-- Register auth.github_links for account merge, export, and 30-day purge (0149_auth_life.sql).
INSERT INTO auth.life_columns (tbl, col, holds, on_merge, on_purge, export, omit, why) VALUES
  ('auth.github_links', 'account_id', 'account', 'move', 'delete', true, '{}', 'Linked GitHub onboarding identity')
ON CONFLICT (tbl, col) DO NOTHING;

-- Backfill existing verified GitHub claims so already-connected accounts stay onboarded.
INSERT INTO auth.github_links (account_id, github_id, github_login, github_avatar, linked_at, updated_at)
SELECT gc.account_id, gc.github_id_key, gc.github_login, gc.github_avatar, gc.claimed_at, gc.updated_at
  FROM referral.github_claims gc
  JOIN auth.accounts a ON a.id = gc.account_id
ON CONFLICT (account_id) DO NOTHING;

-- Upgrade GitHub connect bonus to $10.00 (10,000,000 USD micros).
ALTER TABLE referral.config ALTER COLUMN github_connect_micros SET DEFAULT 10000000;
UPDATE referral.config
   SET github_connect_micros = 10000000, updated_at = now()
 WHERE id = 1;

DO $$
DECLARE r text;
BEGIN
  FOREACH r IN ARRAY ARRAY['oa_api', 'oa_api_staging'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT, INSERT, UPDATE, DELETE ON auth.github_links TO %I', r);
    END IF;
  END LOOP;
  FOREACH r IN ARRAY ARRAY['oa_dump', 'oa_readonly'] LOOP
    IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
      EXECUTE format('GRANT SELECT ON auth.github_links TO %I', r);
    END IF;
  END LOOP;
END $$;
