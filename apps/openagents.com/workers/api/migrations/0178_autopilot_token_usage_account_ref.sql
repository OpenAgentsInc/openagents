-- Attribute autopilot leaderboard token usage rows to the provider-account
-- ref carried by the run's M8/M9 lease. Account-leased work writes the lease's
-- provider_account_ref; paths that genuinely cannot know the account record the
-- typed 'unattributed' sentinel rather than faking attribution.
ALTER TABLE autopilot_token_usage
  ADD COLUMN account_ref TEXT;

CREATE INDEX autopilot_token_usage_account_idx
  ON autopilot_token_usage(account_ref, total_tokens DESC)
  WHERE account_ref IS NOT NULL;
