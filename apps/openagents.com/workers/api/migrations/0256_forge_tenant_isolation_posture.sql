-- FORGE SU-8 (#6798): tenant isolation posture metadata.
--
-- These nullable fields let external-fleet onboarding and UI read models expose
-- public-safe confidential-workspace posture without storing raw attestations,
-- private knowledge packs, repository contents, prompts, tokens, or secrets.

ALTER TABLE forge_tenants ADD COLUMN confidential_workspace_mode TEXT
  CHECK (
    confidential_workspace_mode IS NULL OR
    confidential_workspace_mode IN ('disabled', 'enabled', 'attested')
  );

ALTER TABLE forge_tenants ADD COLUMN attestation_ref TEXT;

ALTER TABLE forge_tenants ADD COLUMN encrypted_knowledge_pack_ref TEXT;

ALTER TABLE forge_tenants ADD COLUMN refusal_reason TEXT;

ALTER TABLE forge_tenants ADD COLUMN retention_policy_ref TEXT;
