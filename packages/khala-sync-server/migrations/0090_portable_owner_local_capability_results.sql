-- IDE-13 #9041: preserve the distinct install and wipe result identities.
--
-- An install returns one terminal receipt, one installation ref, and one
-- evidence ref. A wipe returns one terminal receipt. The queue remains refs-only.

ALTER TABLE khala_sync_portable_owner_local_capability_operations
  ADD COLUMN result_installation_ref text;

DO $$
DECLARE
  constraint_name text;
BEGIN
  FOR constraint_name IN
    SELECT conname
    FROM pg_constraint
    WHERE conrelid = 'khala_sync_portable_owner_local_capability_operations'::regclass
      AND contype = 'c'
      AND pg_get_constraintdef(oid) ILIKE '%receipt_ref%'
      AND pg_get_constraintdef(oid) ILIKE '%error_ref%'
  LOOP
    EXECUTE format(
      'ALTER TABLE khala_sync_portable_owner_local_capability_operations DROP CONSTRAINT %I',
      constraint_name
    );
  END LOOP;
END $$;

ALTER TABLE khala_sync_portable_owner_local_capability_operations
  ADD CONSTRAINT khala_sync_portable_owner_local_capability_state_result_check CHECK (
    (state = 'pending'
      AND claim_ref IS NULL AND claim_fingerprint IS NULL
      AND worker_instance_ref IS NULL AND claim_generation IS NULL
      AND lease_revision IS NULL AND claimed_at IS NULL AND lease_expires_at IS NULL
      AND result_ref IS NULL AND result_fingerprint IS NULL AND result_status IS NULL
      AND result_installation_ref IS NULL AND receipt_ref IS NULL
      AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state = 'claimed'
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND lease_expires_at > claimed_at
      AND result_ref IS NULL AND result_fingerprint IS NULL AND result_status IS NULL
      AND result_installation_ref IS NULL AND receipt_ref IS NULL
      AND error_ref IS NULL AND completed_at IS NULL)
    OR
    (state IN ('completed', 'failed')
      AND claim_ref IS NOT NULL AND claim_fingerprint IS NOT NULL
      AND worker_instance_ref IS NOT NULL AND claim_generation IS NOT NULL
      AND lease_revision IS NOT NULL AND claimed_at IS NOT NULL AND lease_expires_at IS NOT NULL
      AND result_ref IS NOT NULL AND result_fingerprint IS NOT NULL
      AND result_status = state AND completed_at IS NOT NULL)
    OR
    (state = 'expired'
      AND result_ref IS NOT NULL AND result_fingerprint IS NULL
      AND result_status = 'expired' AND result_installation_ref IS NULL
      AND receipt_ref IS NULL AND error_ref IS NULL AND completed_at IS NOT NULL)
  ),
  ADD CONSTRAINT khala_sync_portable_owner_local_capability_action_result_check CHECK (
    (state = 'completed' AND action = 'install'
      AND result_installation_ref IS NOT NULL AND receipt_ref IS NOT NULL
      AND jsonb_array_length(result_evidence_refs_json) = 1 AND error_ref IS NULL)
    OR
    (state = 'completed' AND action = 'wipe'
      AND result_installation_ref IS NULL AND receipt_ref IS NOT NULL
      AND jsonb_array_length(result_evidence_refs_json) = 0 AND error_ref IS NULL)
    OR
    (state = 'failed' AND result_installation_ref IS NULL
      AND receipt_ref IS NULL AND error_ref IS NOT NULL)
    OR
    (state NOT IN ('completed', 'failed')
      AND result_installation_ref IS NULL AND receipt_ref IS NULL AND error_ref IS NULL)
  );
