-- Public-safe production activation for Ben's OTEC Site Revision 3.
-- Executed against the remote `openagents-autopilot` D1 database on 2026-06-05.
-- Static artifact:
--   docs/sites/otec/index.html
--   r2:sites/otec/versions/2026-06-05T171000Z-revision-3/index.html
--   sha256:271a9db474bae8b6e74030351e4f4eb13c9c3b94371845e8db92063da0c19c8b

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
  ('site_version_otec_20260605_revision_3',
   'site_project_otec',
   'operator_static',
   NULL,
   NULL,
   NULL,
   NULL,
   'saved',
   'operator_static_r2_upload',
   NULL,
   '{"assets":{"index.html":{"r2Key":"sites/otec/versions/2026-06-05T171000Z-revision-3/index.html","contentType":"text/html; charset=utf-8","cacheControl":"public, max-age=60"}}}',
   NULL,
   NULL,
   '{"revisionNumber":3,"revisionLabel":"Revision 3","customerReviewState":"customer_review_ready","source":"docs/sites/otec/index.html","sha256":"271a9db474bae8b6e74030351e4f4eb13c9c3b94371845e8db92063da0c19c8b","stableUrl":"https://sites.openagents.com/otec","changes":["Added real public-source image references from U.S. government sources, including the DOE public-domain OTEC plant photo and the EIA OTEC cycle diagram.","Reworked the page around Ben''s 1000m ocean tower concept: 20m cold-water pipe, 40m total diameter, compute annulus, 50m above-water housing and tourism zone, equatorial siting, UHPC/geopolymer/shotcrete construction, mineral-rich water, aeration, seawater mineral mining, and future international-water city growth.","Reduced the main display text scale and shifted the voice away from research prompts toward a concise investor-oriented infrastructure thesis."],"deliveryMode":"local_operator_completion_after_adjutant_adjustment_storage_error","visualAssetRequirements":{"required":true,"satisfied":true,"sources":["https://www.eia.gov/energyexplained/hydropower/images/OTEC_in_Hawaii.jpg","https://www.eia.gov/energyexplained/hydropower/images/oceanthermal.png"],"notes":"EIA labels the OTEC plant photo as U.S. Department of Energy public domain; the OTEC diagram is sourced from the U.S. Energy Information Administration page."},"addressedFeedbackIds":["site_feedback_8cc12f3fb4fc4d68a93b2126b1fcf5bc","site_feedback_f0b08047abf34505be5a2522af988a40"]}',
   'github:14167547',
   NULL,
   '2026-06-05T17:10:00Z',
   '2026-06-05T17:10:00Z',
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
  ('site_build_validation_otec_20260605_revision_3',
   'site_project_otec',
   'site_compatibility_check_otec_20260605_initial',
   'operator_static',
   NULL,
   NULL,
   'sha256:271a9db474bae8b6e74030351e4f4eb13c9c3b94371845e8db92063da0c19c8b',
   'passed',
   NULL,
   'operator_static_r2_upload',
   'operator_static_r2_upload',
   'static',
   'index.html',
   NULL,
   '{"assets":["index.html"],"r2Key":"sites/otec/versions/2026-06-05T171000Z-revision-3/index.html"}',
   '["Parsed docs/sites/otec/index.html successfully with Python html.parser.","Confirmed customer-facing proof/challenge/trace labels are absent.","Confirmed requested visual assets are present as HTML img elements with public-source captions.","Uploaded Revision 3 static artifact to R2 with text/html content type."]',
   4,
   0,
   '[{"code":"revision_3_static_artifact_uploaded","summary":"Revision 3 static artifact was uploaded to R2 and linked to a saved Site version."},{"code":"required_visual_asset_present","summary":"Requested image assets are present and source-captioned."},{"code":"latest_feedback_incorporated","summary":"The latest queued feedback asking for smaller main text and a more investor-focused voice is reflected in the artifact."}]',
   '[]',
   '["This is an operator-completed revision after the supported Adjutant adjustment endpoint returned storage_error while creating duplicate requested adjustment records."]',
   '["docs/sites/otec/index.html","r2:sites/otec/versions/2026-06-05T171000Z-revision-3/index.html","sha256:271a9db474bae8b6e74030351e4f4eb13c9c3b94371845e8db92063da0c19c8b","https://www.eia.gov/energyexplained/hydropower/ocean-thermal-energy-conversion.php"]',
   'Revision 3 is saved and ready for customer review.',
   'Review the stable Site URL or open this revision from the order page, then add any follow-up comments.',
   'github:14167547',
   '2026-06-05T17:10:00Z',
   NULL);

UPDATE site_deployments
   SET status = 'disabled',
       disabled_at = '2026-06-05T17:10:00Z',
       rolled_back_at = NULL,
       updated_at = '2026-06-05T17:10:00Z'
 WHERE site_id = 'site_project_otec'
   AND status IN ('active', 'rolled_back');

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
  ('site_deployment_otec_20260605_revision_3',
   'site_project_otec',
   'site_version_otec_20260605_revision_3',
   'otec',
   'https://sites.openagents.com/otec',
   'omega_static_r2',
   NULL,
   NULL,
   'active',
   'github:14167547',
   'r2:sites/otec/versions/2026-06-05T171000Z-revision-3/index.html',
   '2026-06-05T17:10:00Z',
   '2026-06-05T17:10:00Z',
   NULL,
   NULL,
   NULL,
   '2026-06-05T17:10:00Z',
   '2026-06-05T17:10:00Z');

UPDATE site_projects
   SET status = 'approved',
       access_mode = 'public',
       visibility = 'public',
       active_version_id = 'site_version_otec_20260605_revision_3',
       active_deployment_id = 'site_deployment_otec_20260605_revision_3',
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id = 'site_project_otec'
   AND archived_at IS NULL;

UPDATE software_orders
   SET status = 'delivered',
       current_run_id = NULL,
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id = 'software_order_c34f3a52d60b41d699b71525365b6ee5'
   AND archived_at IS NULL;

UPDATE site_revision_feedback
   SET status = 'addressed',
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id IN (
       'site_feedback_8cc12f3fb4fc4d68a93b2126b1fcf5bc',
       'site_feedback_f0b08047abf34505be5a2522af988a40'
   )
   AND archived_at IS NULL;

UPDATE adjutant_adjustment_requests
   SET status = 'completed',
       resulting_version_id = 'site_version_otec_20260605_revision_3',
       completed_at = '2026-06-05T17:10:00Z',
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id IN (
       'adjutant_adjustment_fd98742fe2394802b9d32d55f65cba8a',
       'adjutant_adjustment_734caf6cd49f408b9dd302d5fa155b5c'
   )
   AND archived_at IS NULL;

UPDATE adjutant_adjustment_requests
   SET status = 'failed',
       completed_at = '2026-06-05T17:10:00Z',
       archived_at = '2026-06-05T17:10:00Z',
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id IN (
       'adjutant_adjustment_232828087b9848ada5f105af30a6d4f5',
       'adjutant_adjustment_23ef38a01ab74014891dd8c45871202f'
   )
   AND archived_at IS NULL;

UPDATE adjutant_assignments
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-05T17:10:00Z',
       blocked_at = NULL,
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id IN (
       'adjutant_assignment_d98b2a644ff742a2b21283653020a8e1',
       'adjutant_assignment_f26d55c3a8344211b92b1c73cfbe3e38'
   )
   AND archived_at IS NULL;

UPDATE agent_goals
   SET status = 'complete',
       current_run_id = NULL,
       completed_at = '2026-06-05T17:10:00Z',
       updated_at = '2026-06-05T17:10:00Z'
 WHERE id = 'agent_goal_otec_revision_2'
   AND archived_at IS NULL;

INSERT OR IGNORE INTO site_events
  (id, site_id, version_id, deployment_id, type, summary, actor_user_id, actor_run_id, payload_json, created_at)
VALUES
  ('site_event_otec_20260605_revision_3_saved',
   'site_project_otec',
   'site_version_otec_20260605_revision_3',
   NULL,
   'site_version.saved',
   'Saved OTEC Revision 3 for customer review.',
   'github:14167547',
   NULL,
   '{"versionId":"site_version_otec_20260605_revision_3","revisionNumber":3,"source":"docs/sites/otec/index.html","sha256":"271a9db474bae8b6e74030351e4f4eb13c9c3b94371845e8db92063da0c19c8b","addressedFeedbackIds":["site_feedback_8cc12f3fb4fc4d68a93b2126b1fcf5bc","site_feedback_f0b08047abf34505be5a2522af988a40"]}',
   '2026-06-05T17:10:00Z'),
  ('site_event_otec_20260605_revision_3_activated',
   'site_project_otec',
   'site_version_otec_20260605_revision_3',
   'site_deployment_otec_20260605_revision_3',
   'site_deployment.activated',
   'Activated OTEC Revision 3 on the stable Site URL.',
   'github:14167547',
   NULL,
   '{"deploymentId":"site_deployment_otec_20260605_revision_3","url":"https://sites.openagents.com/otec","runtimeKind":"omega_static_r2","r2Key":"sites/otec/versions/2026-06-05T171000Z-revision-3/index.html"}',
   '2026-06-05T17:10:00Z');

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
  ('adjutant_assignment_event_otec_revision_3_local_completion_20260605_1710',
   'adjutant_assignment_d98b2a644ff742a2b21283653020a8e1',
   'software_order_c34f3a52d60b41d699b71525365b6ee5',
   'site_project_otec',
   'agent_goal_otec_revision_2',
   NULL,
   'site_revision.completed',
   'public',
   'Completed OTEC Revision 3 locally after the Adjutant adjustment endpoint returned storage_error.',
   'github:14167547',
   '{"versionId":"site_version_otec_20260605_revision_3","deploymentId":"site_deployment_otec_20260605_revision_3","failureMode":"adjutant_adjustment_endpoint_storage_error","visualAssets":["https://www.eia.gov/energyexplained/hydropower/images/OTEC_in_Hawaii.jpg","https://www.eia.gov/energyexplained/hydropower/images/oceanthermal.png"]}',
   '2026-06-05T17:10:00Z'),
  ('adjutant_assignment_event_otec_revision_3_latest_feedback_completion_20260605_1710',
   'adjutant_assignment_f26d55c3a8344211b92b1c73cfbe3e38',
   'software_order_c34f3a52d60b41d699b71525365b6ee5',
   'site_project_otec',
   NULL,
   NULL,
   'site_revision.completed',
   'public',
   'Completed the latest OTEC feedback in Revision 3: smaller main text and a more investor-focused page.',
   'github:14167547',
   '{"versionId":"site_version_otec_20260605_revision_3","deploymentId":"site_deployment_otec_20260605_revision_3","addressedFeedbackId":"site_feedback_f0b08047abf34505be5a2522af988a40"}',
   '2026-06-05T17:10:00Z');
