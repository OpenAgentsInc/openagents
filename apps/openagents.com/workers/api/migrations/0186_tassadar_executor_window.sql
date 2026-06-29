-- #5007 Tassadar Launch Step 2: claimable executor-trace work for the run.
-- Seeds one active executor-trace work window under the Monday Tassadar run so
-- an admitted contributor can claim a lease (POST /api/training/leases/claim)
-- and the run projection can reflect an assigned contributor; the digest-pinned
-- workload refs identify the bounded ALM numeric-trace work, and a submitted
-- closeout is verified by the already run-aware exact_trace_replay challenge.
-- No-spend dispatch only; no payout/settlement here, and no promise flips green.

INSERT OR IGNORE INTO training_windows
  (id, window_ref, training_run_ref, state, homework_kind, priority,
   dataset_refs_json, source_refs_json, receipt_refs_json,
   public_projection_json, planned_at, activated_at, updated_at, archived_at)
VALUES (
  'training_window_tassadar_executor_20260615_w1',
  'training.window.tassadar.executor.20260615.w1',
  'run.tassadar.executor.20260615',
  'active',
  'admin_dispatched_homework',
  100,
  '[]',
  '["workload.tassadar_executor.alm_numeric_trace.v1","packages/tassadar-executor/fixtures/tassadar-poc-loop-sum-v1.json","artifact.tassadar_poc.trace_digest.f2995c4e3c959b42bb1e4afbefffbcf7ba6104099621ccc0ac912862dc932a5b"]',
  '["approval.operator.20260614.tassadar_executor_window_issue5007"]',
  '{}',
  '2026-06-14T00:00:00.000Z',
  '2026-06-14T00:00:00.000Z',
  '2026-06-14T00:00:00.000Z',
  NULL
);
