ALTER TABLE provider_accounts
  ADD COLUMN operator_label TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN lease_limit INTEGER NOT NULL DEFAULT 1;

ALTER TABLE provider_accounts
  ADD COLUMN last_parallel_probe_at TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN last_parallel_probe_result TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN last_successful_launch_at TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN last_failed_launch_at TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN reauth_required_reason TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN operator_note TEXT;

ALTER TABLE provider_accounts
  ADD COLUMN refill_note TEXT;
