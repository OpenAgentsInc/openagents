INSERT OR IGNORE INTO software_orders (
  id,
  user_id,
  status,
  visibility,
  request,
  repository_provider,
  repository_owner,
  repository_name,
  repository_full_name,
  repository_private,
  repository_default_branch,
  repository_html_url,
  public_work_acknowledged_at,
  data_use_acknowledged_at,
  compute_payment_acknowledged_at,
  provider_account_required,
  free_slice_cents,
  quote_cents,
  current_run_id,
  agent_started_at,
  created_at,
  updated_at
)
SELECT
  'software_order_backfill_' ||
    replace(substr(users.id, instr(users.id, ':') + 1), '-', '_'),
  users.id,
  'submitted',
  'public',
  trim(users.onboarding_goal),
  users.onboarding_repository_provider,
  users.onboarding_repository_owner,
  users.onboarding_repository_name,
  COALESCE(
    users.onboarding_repository_full_name,
    CASE
      WHEN users.onboarding_repository_owner IS NOT NULL
       AND users.onboarding_repository_name IS NOT NULL
      THEN users.onboarding_repository_owner || '/' || users.onboarding_repository_name
      ELSE NULL
    END
  ),
  users.onboarding_repository_private,
  COALESCE(
    users.onboarding_repository_default_branch,
    CASE
      WHEN users.onboarding_repository_provider = 'github' THEN 'main'
      ELSE NULL
    END
  ),
  users.onboarding_repository_html_url,
  users.onboarding_completed_at,
  users.onboarding_completed_at,
  users.onboarding_completed_at,
  0,
  5000,
  NULL,
  NULL,
  NULL,
  users.onboarding_completed_at,
  users.onboarding_completed_at
FROM users
WHERE users.deleted_at IS NULL
  AND users.kind = 'human'
  AND users.onboarding_completed_at IS NOT NULL
  AND trim(COALESCE(users.onboarding_goal, '')) <> ''
  AND NOT EXISTS (
    SELECT 1
      FROM software_orders
     WHERE software_orders.user_id = users.id
       AND software_orders.archived_at IS NULL
  );
