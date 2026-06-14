-- #5006 Tassadar Launch Step 1: training run authority.
-- Adds the public-safe run launch manifest column and creates the Monday
-- Tassadar executor-trace run in the `active` state so the run authority can
-- report a live (non-`planned`) run with manifest fields. Run state still moves
-- only through the run state-transition route; this seed authors the launch run
-- itself. No promise flips green from this migration.

ALTER TABLE training_runs ADD COLUMN manifest_json TEXT;

INSERT OR IGNORE INTO training_runs
  (id, training_run_ref, promise_ref, state, manifest_json,
   source_refs_json, receipt_refs_json, public_projection_json,
   created_at, updated_at, archived_at)
VALUES (
  'training_run_tassadar_executor_20260615',
  'run.tassadar.executor.20260615',
  'training.monday_decentralized_training_launch.v1',
  'active',
  '{"objective":"Grow the Tassadar verified-trace corpus via paid executor-trace work, verified by exact replay.","workloadFamily":"executor-trace","verifierPolicy":"exact_trace_replay","admissionRule":"Contributor nodes declaring the executor-trace capability are admitted through the reasoned device-admission gates; owner-operated nodes do not count as independent contributor proof.","paymentMode":"operator_approved_small_sats","settlementState":"pending","spendCapSats":100000,"statusUrl":"https://openagents.com/api/training/runs/run.tassadar.executor.20260615","abortRule":"Halt admission and dispatch if a settlement lands undereferenceable or a payout cannot be shown to its recipient; resume after the public projection is fixed.","blockerRefs":["blocker.training.monday_launch_self_serve_stranger_payout_pending","blocker.training.live_settlement_projection_pending","blocker.training.autopilot_install_bundled_node_pending"]}',
  '["JUNE15_LAUNCH_PLAN.md","docs/2026-06-12-episode-236-training-launch-gap-audit.md","docs/transcripts/236.md","issue.github.openagents.5006"]',
  '["approval.operator.20260614.tassadar_run_authority_issue5006"]',
  '{}',
  '2026-06-14T00:00:00.000Z',
  '2026-06-14T00:00:00.000Z',
  NULL
);
