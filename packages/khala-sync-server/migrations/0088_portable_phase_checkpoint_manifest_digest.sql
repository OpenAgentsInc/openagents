-- IDE-13 #9041: retain the exact committed checkpoint manifest digest.
--
-- The digest is owner-scoped phase result state. Phase request rows continue to
-- contain only stable refs and checkpoint digests. No artifact bytes are stored.

ALTER TABLE khala_sync_portable_phase_operations
  ADD COLUMN IF NOT EXISTS result_checkpoint_manifest_digest text CHECK (
    result_checkpoint_manifest_digest IS NULL OR
    result_checkpoint_manifest_digest ~ '^sha256:[0-9a-f]{64}$'
  );

ALTER TABLE khala_sync_portable_phase_operations
  ADD CONSTRAINT khala_sync_portable_phase_checkpoint_manifest_digest_shape
  CHECK (
    (state = 'completed' AND kind = 'checkpoint-create')
    OR result_checkpoint_manifest_digest IS NULL
  ) NOT VALID;
