-- Workroom-kind templates and workroom template packages.
--
-- Gives the previously type-only definitions in
-- omni-workroom-kind-templates.ts and workroom-template-packages.ts a D1
-- backing. workroom_kind_templates stores one row per workroom kind with the
-- accepted-outcome work kind, privacy/projection/proof/review policy, and the
-- required-evidence/required-artifact lists carried as JSON arrays.
-- workroom_template_packages stores review/projection-only package records,
-- and workroom_template_package_versions stores their versioned ref bundles.

CREATE TABLE IF NOT EXISTS workroom_kind_templates (
  kind TEXT PRIMARY KEY NOT NULL CHECK (
    kind IN (
      'site',
      'coding',
      'crm',
      'investor_ops',
      'project_ops',
      'support',
      'finance_ops',
      'meeting',
      'document',
      'legal_review'
    )
  ),
  accepted_outcome_work_kind TEXT NOT NULL CHECK (
    accepted_outcome_work_kind IN (
      'site',
      'coding',
      'adjustment',
      'existing_project_import',
      'business',
      'legal_sensitive'
    )
  ),
  description_ref TEXT NOT NULL,
  privacy_constraint TEXT NOT NULL CHECK (
    privacy_constraint IN (
      'public_ok',
      'customer_private',
      'team_private',
      'financial_private',
      'legal_private'
    )
  ),
  proof_policy TEXT NOT NULL CHECK (
    proof_policy IN (
      'private_receipt',
      'customer_safe_summary',
      'public_safe_proof',
      'legal_sensitive_private'
    )
  ),
  public_projection_policy TEXT NOT NULL CHECK (
    public_projection_policy IN (
      'none',
      'customer_safe_summary',
      'team_safe_summary',
      'public_safe_proof'
    )
  ),
  review_policy TEXT NOT NULL CHECK (
    review_policy IN (
      'operator_review',
      'customer_review',
      'dual_review',
      'owner_review',
      'no_review'
    )
  ),
  closeout_requirements_json TEXT NOT NULL DEFAULT '[]',
  required_artifacts_json TEXT NOT NULL DEFAULT '[]',
  required_evidence_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_kind_templates_work_kind
  ON workroom_kind_templates(accepted_outcome_work_kind, kind);

CREATE TABLE IF NOT EXISTS workroom_template_packages (
  id TEXT PRIMARY KEY NOT NULL,
  package_ref TEXT NOT NULL UNIQUE,
  version_ref TEXT NOT NULL,
  display_name TEXT NOT NULL,
  state TEXT NOT NULL CHECK (
    state IN (
      'blocked',
      'draft',
      'org_private_enabled',
      'public_projection_ready',
      'review_recorded',
      'runtime_promotion_requested',
      'validation_recorded'
    )
  ),
  authority_boundary TEXT NOT NULL DEFAULT 'package_review_projection_only'
    CHECK (authority_boundary IN ('package_review_projection_only')),
  no_deployment INTEGER NOT NULL DEFAULT 1 CHECK (no_deployment IN (0, 1)),
  no_external_runner_launch INTEGER NOT NULL DEFAULT 1
    CHECK (no_external_runner_launch IN (0, 1)),
  no_marketplace_listing INTEGER NOT NULL DEFAULT 1
    CHECK (no_marketplace_listing IN (0, 1)),
  no_payment_mutation INTEGER NOT NULL DEFAULT 1
    CHECK (no_payment_mutation IN (0, 1)),
  no_runtime_promotion INTEGER NOT NULL DEFAULT 1
    CHECK (no_runtime_promotion IN (0, 1)),
  approval_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  blocker_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  evidence_requirement_refs_json TEXT NOT NULL DEFAULT '[]',
  operator_diagnostic_refs_json TEXT NOT NULL DEFAULT '[]',
  org_private_enablement_refs_json TEXT NOT NULL DEFAULT '[]',
  outcome_template_refs_json TEXT NOT NULL DEFAULT '[]',
  proof_rule_refs_json TEXT NOT NULL DEFAULT '[]',
  promotion_refs_json TEXT NOT NULL DEFAULT '[]',
  public_projection_refs_json TEXT NOT NULL DEFAULT '[]',
  required_artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  review_refs_json TEXT NOT NULL DEFAULT '[]',
  runner_need_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  template_version_refs_json TEXT NOT NULL DEFAULT '[]',
  ui_binding_refs_json TEXT NOT NULL DEFAULT '[]',
  validation_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_template_packages_state_updated
  ON workroom_template_packages(state, updated_at DESC);

CREATE TABLE IF NOT EXISTS workroom_template_package_versions (
  id TEXT PRIMARY KEY NOT NULL,
  package_id TEXT NOT NULL
    REFERENCES workroom_template_packages(id) ON DELETE CASCADE,
  template_version_ref TEXT NOT NULL,
  approval_policy_refs_json TEXT NOT NULL DEFAULT '[]',
  caveat_refs_json TEXT NOT NULL DEFAULT '[]',
  evidence_requirement_refs_json TEXT NOT NULL DEFAULT '[]',
  outcome_template_refs_json TEXT NOT NULL DEFAULT '[]',
  proof_rule_refs_json TEXT NOT NULL DEFAULT '[]',
  required_artifact_refs_json TEXT NOT NULL DEFAULT '[]',
  runner_need_refs_json TEXT NOT NULL DEFAULT '[]',
  source_refs_json TEXT NOT NULL DEFAULT '[]',
  ui_binding_refs_json TEXT NOT NULL DEFAULT '[]',
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_workroom_template_package_versions_package
  ON workroom_template_package_versions(package_id, created_at DESC);
