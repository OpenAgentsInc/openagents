-- Public-safe production activation for Ben's OTEC Site Revision 2.
-- Executed against the remote `openagents-autopilot` D1 database on 2026-06-05.
-- Static artifact:
--   docs/sites/otec/index.html
--   r2:sites/otec/versions/2026-06-05T163000Z-revision-2/index.html
--   sha256:3b485e72f54c39e93a3e12ea2e8f80333381828ba0038b830f09d5ec109494f7

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
  ('site_version_otec_20260605_revision_2',
   'site_project_otec',
   'operator_static',
   NULL,
   NULL,
   NULL,
   NULL,
   'saved',
   'operator_static_r2_upload',
   NULL,
   '{"assets":{"index.html":{"r2Key":"sites/otec/versions/2026-06-05T163000Z-revision-2/index.html","contentType":"text/html; charset=utf-8","cacheControl":"public, max-age=60"}}}',
   NULL,
   NULL,
   '{"revisionNumber":2,"revisionLabel":"Revision 2","customerReviewState":"customer_review_ready","source":"docs/sites/otec/index.html","sha256":"3b485e72f54c39e93a3e12ea2e8f80333381828ba0038b830f09d5ec109494f7","stableUrl":"https://sites.openagents.com/otec","changes":["Replaced dark proof-of-concept shell with a light investor-oriented page.","Removed customer-facing proof/challenge/trace navigation from the Site itself.","Added clearer OTEC/SWAC thesis, integrated system framing, near-term build path, and conservative source starting points."],"failedRunId":"977353f8-f1ac-4f82-8c2f-5d37cde85199","deliveryMode":"local_operator_completion_after_shc_timeout"}',
   'github:14167547',
   '977353f8-f1ac-4f82-8c2f-5d37cde85199',
   '2026-06-05T16:23:23Z',
   '2026-06-05T16:23:23Z',
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
  ('site_build_validation_otec_20260605_revision_2',
   'site_project_otec',
   'site_compatibility_check_otec_20260605_initial',
   'operator_static',
   NULL,
   NULL,
   'sha256:3b485e72f54c39e93a3e12ea2e8f80333381828ba0038b830f09d5ec109494f7',
   'passed',
   NULL,
   'operator_static_r2_upload',
   'operator_static_r2_upload',
   'static',
   'index.html',
   NULL,
   '{"assets":["index.html"],"r2Key":"sites/otec/versions/2026-06-05T163000Z-revision-2/index.html"}',
   '["Parsed docs/sites/otec/index.html successfully with Python html.parser.","Confirmed customer-facing proof/challenge/trace labels are absent.","Uploaded Revision 2 static artifact to R2 with text/html content type."]',
   3,
   0,
   '[{"code":"revision_2_static_artifact_uploaded","summary":"Revision 2 static artifact was uploaded to R2 and linked to a saved Site version."}]',
   '[]',
   '["This is an operator-completed revision after the SHC one-shot run timed out before producing a branch or pull request."]',
   '["docs/sites/otec/index.html","r2:sites/otec/versions/2026-06-05T163000Z-revision-2/index.html","sha256:3b485e72f54c39e93a3e12ea2e8f80333381828ba0038b830f09d5ec109494f7"]',
   'Revision 2 is saved and ready for customer review.',
   'Ask the customer to review the stable Site URL and add any follow-up revision comments from the order page.',
   'github:14167547',
   '2026-06-05T16:23:23Z',
   NULL);

UPDATE site_deployments
   SET status = 'rolled_back',
       rolled_back_at = '2026-06-05T16:23:23Z',
       updated_at = '2026-06-05T16:23:23Z'
 WHERE site_id = 'site_project_otec'
   AND status = 'active';

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
  ('site_deployment_otec_20260605_revision_2',
   'site_project_otec',
   'site_version_otec_20260605_revision_2',
   'otec',
   'https://sites.openagents.com/otec',
   'omega_static_r2',
   NULL,
   NULL,
   'active',
   'github:14167547',
   'r2:sites/otec/versions/2026-06-05T163000Z-revision-2/index.html',
   '2026-06-05T16:23:23Z',
   '2026-06-05T16:23:23Z',
   NULL,
   NULL,
   NULL,
   '2026-06-05T16:23:23Z',
   '2026-06-05T16:23:23Z');

UPDATE site_projects
   SET status = 'approved',
       access_mode = 'public',
       visibility = 'public',
       active_version_id = 'site_version_otec_20260605_revision_2',
       active_deployment_id = 'site_deployment_otec_20260605_revision_2',
       updated_at = '2026-06-05T16:23:23Z'
 WHERE id = 'site_project_otec'
   AND archived_at IS NULL;

UPDATE software_orders
   SET status = 'delivered',
       current_run_id = NULL,
       updated_at = '2026-06-05T16:23:23Z'
 WHERE id = 'software_order_c34f3a52d60b41d699b71525365b6ee5'
   AND archived_at IS NULL;

UPDATE adjutant_assignments
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-05T16:23:23Z',
       blocked_at = NULL,
       updated_at = '2026-06-05T16:23:23Z'
 WHERE id = 'adjutant_assignment_d98b2a644ff742a2b21283653020a8e1'
   AND archived_at IS NULL;

UPDATE agent_goals
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-05T16:23:23Z',
       updated_at = '2026-06-05T16:23:23Z'
 WHERE id = 'agent_goal_otec_revision_2'
   AND archived_at IS NULL;

INSERT OR IGNORE INTO site_events
  (id, site_id, version_id, deployment_id, type, summary, actor_user_id, actor_run_id, payload_json, created_at)
VALUES
  ('site_event_otec_20260605_revision_2_saved',
   'site_project_otec',
   'site_version_otec_20260605_revision_2',
   NULL,
   'site_version.saved',
   'Saved OTEC Revision 2 for customer review.',
   'github:14167547',
   '977353f8-f1ac-4f82-8c2f-5d37cde85199',
   '{"versionId":"site_version_otec_20260605_revision_2","revisionNumber":2,"source":"docs/sites/otec/index.html","sha256":"3b485e72f54c39e93a3e12ea2e8f80333381828ba0038b830f09d5ec109494f7"}',
   '2026-06-05T16:23:23Z'),
  ('site_event_otec_20260605_revision_2_activated',
   'site_project_otec',
   'site_version_otec_20260605_revision_2',
   'site_deployment_otec_20260605_revision_2',
   'site_deployment.activated',
   'Activated OTEC Revision 2 on the stable Site URL.',
   'github:14167547',
   '977353f8-f1ac-4f82-8c2f-5d37cde85199',
   '{"deploymentId":"site_deployment_otec_20260605_revision_2","url":"https://sites.openagents.com/otec","runtimeKind":"omega_static_r2","r2Key":"sites/otec/versions/2026-06-05T163000Z-revision-2/index.html"}',
   '2026-06-05T16:23:23Z');

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
  ('adjutant_assignment_event_otec_revision_2_local_completion_20260605_1623',
   'adjutant_assignment_d98b2a644ff742a2b21283653020a8e1',
   'software_order_c34f3a52d60b41d699b71525365b6ee5',
   'site_project_otec',
   'agent_goal_otec_revision_2',
   '977353f8-f1ac-4f82-8c2f-5d37cde85199',
   'site_revision.completed',
   'public',
   'Completed OTEC Revision 2 locally after SHC timed out without a branch or pull request.',
   'github:14167547',
   '{"versionId":"site_version_otec_20260605_revision_2","deploymentId":"site_deployment_otec_20260605_revision_2","failedRunId":"977353f8-f1ac-4f82-8c2f-5d37cde85199","failureMode":"shc_one_shot_timeout_workspace_removed_no_branch"}',
   '2026-06-05T16:23:23Z');
