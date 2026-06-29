-- Rename the training launch promise id forward:
-- training.monday_decentralized_training_launch.v1
--   -> training.decentralized_training_launch.v1
-- ("Monday" dropped; it is just the decentralized training launch now,
-- registry 2026-06-17.6). This is a forward-looking identifier rename only:
-- the run stays in its current state and the real-settlement evidence is
-- unchanged. Migration 0185 seeded the run row under the old promise id and is
-- left intact as applied history; this migration updates the live row so the
-- projected promiseRef matches the renamed registry promise. The old id stays
-- recorded in historical registry notes and prior promise_transition receipts.

UPDATE training_runs
SET promise_ref = 'training.decentralized_training_launch.v1',
    updated_at = '2026-06-17T00:00:00.000Z'
WHERE training_run_ref = 'run.tassadar.executor.20260615'
  AND promise_ref = 'training.monday_decentralized_training_launch.v1';
