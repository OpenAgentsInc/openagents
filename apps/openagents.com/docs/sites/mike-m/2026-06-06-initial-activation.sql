-- Public-safe production activation for Mike M's counter dashboard Site.
-- Executed against the remote `openagents-autopilot` D1 database on 2026-06-06.
-- Static artifact:
--   docs/sites/mike-m/index.html
--   r2:sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html
--   sha256:217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766

INSERT OR REPLACE INTO site_versions
  (id,
   site_id,
   source_kind,
   source_commit_sha,
   source_archive_r2_key,
   artifact_manifest_r2_key,
   build_log_r2_key,
   build_status,
   build_command,
   worker_module_r2_key,
   static_assets_manifest_json,
   d1_binding_name,
   r2_binding_name,
   metadata_json,
   created_by_user_id,
   created_by_run_id,
   created_at,
   saved_at,
   rejected_at)
VALUES
  ('site_version_mike_m_20260606_initial',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'operator_static',
   NULL,
   NULL,
   NULL,
   NULL,
   'saved',
   'operator_static_r2_upload',
   NULL,
   '{"assets":{"index.html":{"r2Key":"sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html","contentType":"text/html; charset=utf-8","cacheControl":"public, max-age=60"}}}',
   NULL,
   NULL,
   '{"revisionNumber":1,"revisionLabel":"Initial review","customerReviewState":"customer_review_ready","runtimeActivationPolicy":"latest_successful_revision","customerAccepted":false,"source":"docs/sites/mike-m/index.html","sha256":"217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766","stableUrl":"https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781","versionUrl":"https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781/versions/site_version_mike_m_20260606_initial","deliveryMode":"local_operator_completion","changes":["Built a self-contained single-page counter dashboard with six default counters.","Added increment, decrement, reset, global reset, editable labels, and add-counter controls.","Persisted counter state to localStorage with graceful fallback for missing or malformed saved data.","Added smooth number-change animation with reduced-motion support and responsive desktop/mobile layouts."],"verification":["Playwright Chromium flow passed for increment, decrement, label edit, add counter, refresh persistence, and summary totals.","Playwright Chromium mobile check at 390px reported no horizontal overflow.","Screenshots captured at /tmp/mike-m-dashboard-desktop.png and /tmp/mike-m-dashboard-mobile.png."],"sourceNotes":{"externalLibraries":[],"images":[],"fonts":["system UI stack"],"approvedReferenceUrls":["https://github.com/Haseeb-MernStack/focusflow-productivity-dashboard","https://github.com/lakshyaelite/tally-counter-app","https://github.com/didoghosh143/Productivity-Dashboard"]}}',
   'github:14167547',
   NULL,
   '2026-06-06T18:00:46Z',
   '2026-06-06T18:00:46Z',
   NULL);

INSERT OR REPLACE INTO site_compatibility_checks
  (id,
   site_id,
   source_kind,
   source_repository_json,
   status,
   confidence,
   package_manager,
   build_command,
   output_kind,
   output_path,
   worker_module_path,
   needs_d1,
   needs_r2,
   needs_workspace_auth,
   needs_public_auth,
   env_keys_json,
   findings_json,
   blockers_json,
   warnings_json,
   evidence_refs_json,
   customer_safe_status,
   customer_safe_next_action,
   checked_by_user_id,
   created_at,
   archived_at)
VALUES
  ('site_compatibility_check_mike_m_20260606_initial',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'operator_static',
   NULL,
   'ready',
   'high',
   NULL,
   'operator_static_r2_upload',
   'static',
   'index.html',
   NULL,
   0,
   0,
   0,
   0,
   '[]',
   '[{"code":"single_static_artifact","summary":"Mike M initial review version is a single static HTML artifact served from R2."},{"code":"client_side_persistence_only","summary":"Counter state persists in browser localStorage without server state."}]',
   '[]',
   '[]',
   '["docs/sites/mike-m/index.html","r2:sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html","sha256:217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766"]',
   'The counter dashboard is compatible with the current static Sites runtime.',
   'Review the live dashboard and reply with any broken, confusing, or missing behavior.',
   'github:14167547',
   '2026-06-06T18:00:46Z',
   NULL);

INSERT OR REPLACE INTO site_build_validations
  (id,
   site_id,
   compatibility_check_id,
   source_kind,
   source_repository_json,
   source_commit_sha,
   source_hash,
   status,
   package_manager,
   requested_build_command,
   build_command,
   output_kind,
   output_path,
   worker_module_path,
   manifest_json,
   bounded_logs_json,
   log_line_count,
   log_truncated,
   findings_json,
   blockers_json,
   warnings_json,
   evidence_refs_json,
   customer_safe_status,
   customer_safe_next_action,
   validated_by_user_id,
   created_at,
   archived_at)
VALUES
  ('site_build_validation_mike_m_20260606_initial',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'site_compatibility_check_mike_m_20260606_initial',
   'operator_static',
   NULL,
   NULL,
   'sha256:217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766',
   'passed',
   NULL,
   'operator_static_r2_upload',
   'operator_static_r2_upload',
   'static',
   'index.html',
   NULL,
   '{"assets":["index.html"],"r2Key":"sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html"}',
   '["Served docs/sites/mike-m/index.html from a local HTTP server.","Playwright Chromium verified counter increment, decrement, edit, add, refresh persistence, and summary totals.","Playwright Chromium verified the 390px mobile layout has no horizontal overflow.","Uploaded the static artifact to production R2 with text/html content type."]',
   4,
   0,
   '[{"code":"public_static_artifact_uploaded","summary":"Static dashboard artifact was uploaded to R2 and linked to the saved Site version."},{"code":"browser_flow_verified","summary":"Desktop and mobile browser checks passed for the requested counter behavior."}]',
   '[]',
   '[]',
   '["docs/sites/mike-m/index.html","r2:sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html","sha256:217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766","screenshot:/tmp/mike-m-dashboard-desktop.png","screenshot:/tmp/mike-m-dashboard-mobile.png"]',
   'The first dashboard build validation passed for static R2 deployment.',
   'Review the live dashboard and reply with any bugs or requested changes.',
   'github:14167547',
   '2026-06-06T18:00:46Z',
   NULL);

UPDATE site_deployments
   SET status = 'rolled_back',
       rolled_back_at = '2026-06-06T18:00:46Z',
       updated_at = '2026-06-06T18:00:46Z'
 WHERE site_id = 'site_project_1c1769628bfd41dcb52547df72381468'
   AND status = 'active'
   AND id <> 'site_deployment_mike_m_20260606_initial';

INSERT OR REPLACE INTO site_deployments
  (id,
   site_id,
   version_id,
   slug,
   url,
   runtime_kind,
   runtime_script_name,
   dispatch_namespace,
   status,
   deployed_by_user_id,
   external_deployment_id,
   started_at,
   activated_at,
   failed_at,
   disabled_at,
   rolled_back_at,
   created_at,
   updated_at)
VALUES
  ('site_deployment_mike_m_20260606_initial',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'site_version_mike_m_20260606_initial',
   'mike-m-dd2f2917274c4e64bf1d6781',
   'https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781',
   'omega_static_r2',
   NULL,
   NULL,
   'active',
   'github:14167547',
   'r2:sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html',
   '2026-06-06T18:00:46Z',
   '2026-06-06T18:00:46Z',
   NULL,
   NULL,
   NULL,
   '2026-06-06T18:00:46Z',
   '2026-06-06T18:00:46Z');

UPDATE site_projects
   SET status = 'needs_review',
       access_mode = 'public',
       visibility = 'public',
       active_version_id = 'site_version_mike_m_20260606_initial',
       active_deployment_id = 'site_deployment_mike_m_20260606_initial',
       updated_at = '2026-06-06T18:00:46Z'
 WHERE id = 'site_project_1c1769628bfd41dcb52547df72381468'
   AND archived_at IS NULL;

UPDATE software_orders
   SET status = 'delivered',
       current_run_id = NULL,
       updated_at = '2026-06-06T18:00:46Z'
 WHERE id = 'software_order_dd2f2917274c4e64bf1d678127dd6fa6'
   AND archived_at IS NULL;

UPDATE order_triage_records
   SET hold_reason = NULL,
       next_action = 'Review the live counter dashboard and reply with any bugs, confusing behavior, or requested changes.',
       customer_safe_status = 'review_ready',
       customer_safe_summary = 'OpenAgents prepared the first review version of the counter dashboard with persisted counters, reset controls, editable labels, and responsive layout.',
       reviewer_user_id = 'github:14167547',
       reviewed_at = '2026-06-06T18:00:46Z',
       updated_at = '2026-06-06T18:00:46Z'
 WHERE software_order_id = 'software_order_dd2f2917274c4e64bf1d678127dd6fa6'
   AND archived_at IS NULL;

UPDATE adjutant_assignments
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-06T18:00:46Z',
       blocked_at = NULL,
       updated_at = '2026-06-06T18:00:46Z'
 WHERE id = 'adjutant_assignment_a779d4df16bd407ea90adb299752f989'
   AND archived_at IS NULL;

UPDATE agent_goals
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-06T18:00:46Z',
       blocked_at = NULL,
       updated_at = '2026-06-06T18:00:46Z'
 WHERE id = 'agent_goal_6480cf25f0b84062985c71ff5de1d5a9'
   AND archived_at IS NULL;

INSERT OR IGNORE INTO site_events
  (id, site_id, version_id, deployment_id, type, summary, actor_user_id, actor_run_id, payload_json, created_at)
VALUES
  ('site_event_mike_m_20260606_initial_saved',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'site_version_mike_m_20260606_initial',
   NULL,
   'site_version.saved',
   'Saved Mike M counter dashboard initial version for customer review.',
   'github:14167547',
   NULL,
   '{"versionId":"site_version_mike_m_20260606_initial","revisionNumber":1,"source":"docs/sites/mike-m/index.html","sha256":"217b91816ab36958897065166ca6d32647005ce886dbab0e6cc69dd460ef9766"}',
   '2026-06-06T18:00:46Z'),
  ('site_event_mike_m_20260606_initial_activated',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'site_version_mike_m_20260606_initial',
   'site_deployment_mike_m_20260606_initial',
   'site_deployment.activated',
   'Activated Mike M counter dashboard on the stable Site URL.',
   'github:14167547',
   NULL,
   '{"deploymentId":"site_deployment_mike_m_20260606_initial","url":"https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781","runtimeKind":"omega_static_r2","r2Key":"sites/mike-m-dd2f2917274c4e64bf1d6781/versions/2026-06-06T180046Z-initial/index.html"}',
   '2026-06-06T18:00:46Z');

INSERT OR IGNORE INTO adjutant_assignment_events
  (id,
   assignment_id,
   software_order_id,
   site_id,
   goal_id,
   run_id,
   event_type,
   visibility,
   summary,
   actor_user_id,
   payload_json,
   created_at)
VALUES
  ('adjutant_assignment_event_mike_m_local_completion_20260606_180046',
   'adjutant_assignment_a779d4df16bd407ea90adb299752f989',
   'software_order_dd2f2917274c4e64bf1d678127dd6fa6',
   'site_project_1c1769628bfd41dcb52547df72381468',
   'agent_goal_6480cf25f0b84062985c71ff5de1d5a9',
   '3f51eaf4-1754-4277-9faf-19f6d424e7a1',
   'site_generation.completed',
   'public',
   'Completed Mike M counter dashboard locally and activated the first review version.',
   'github:14167547',
   '{"versionId":"site_version_mike_m_20260606_initial","deploymentId":"site_deployment_mike_m_20260606_initial","stableUrl":"https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781","versionUrl":"https://sites.openagents.com/mike-m-dd2f2917274c4e64bf1d6781/versions/site_version_mike_m_20260606_initial","deliveryMode":"local_operator_completion"}',
   '2026-06-06T18:00:46Z');
