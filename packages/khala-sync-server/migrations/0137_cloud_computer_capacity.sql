-- Durable cloud-computer inventory, quota observations, admission reservations,
-- and reconciliation evidence. Provider enumeration is evidence, never the
-- admission authority.

CREATE TABLE IF NOT EXISTS khala_sync_cloud_computers (
  computer_ref text PRIMARY KEY,
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  conversation_ref text NOT NULL,
  work_unit_ref text NOT NULL,
  kind text NOT NULL CHECK (kind IN ('interactive_retained', 'one_shot_batch')),
  runtime_class text NOT NULL CHECK (runtime_class IN ('standard', 'strong', 'batch')),
  generation bigint NOT NULL CHECK (generation > 0),
  version bigint NOT NULL CHECK (version > 0),
  runtime_profile_ref text NOT NULL,
  authority_snapshot_digest text NOT NULL CHECK (authority_snapshot_digest ~ '^sha256:[0-9a-f]{64}$'),
  budget_snapshot_digest text NOT NULL CHECK (budget_snapshot_digest ~ '^sha256:[0-9a-f]{64}$'),
  capability_refs jsonb NOT NULL,
  state text NOT NULL CHECK (state IN ('cold', 'queued', 'starting', 'active', 'stopping', 'failed', 'destroyed')),
  active_lease_ref text,
  latest_checkpoint_ref text,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (computer_ref, generation, owner_ref, tenant_ref, conversation_ref, runtime_class),
  CHECK (jsonb_typeof(capability_refs) = 'array'),
  CHECK ((state IN ('active', 'stopping')) = (active_lease_ref IS NOT NULL)),
  CHECK (updated_at >= created_at)
);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_computers_conversation_inventory
  ON khala_sync_cloud_computers (tenant_ref, conversation_ref, state);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_capacity_observations (
  observation_ref text PRIMARY KEY,
  observation_digest text NOT NULL CHECK (observation_digest ~ '^sha256:[0-9a-f]{64}$'),
  provider text NOT NULL,
  region text NOT NULL,
  quota_units bigint NOT NULL CHECK (quota_units >= 0),
  allocatable_units bigint NOT NULL CHECK (allocatable_units >= 0),
  drained_units bigint NOT NULL DEFAULT 0 CHECK (drained_units >= 0),
  quota_resources jsonb NOT NULL,
  allocatable_resources jsonb NOT NULL,
  drain_adjusted_resources jsonb NOT NULL,
  budget_resources jsonb NOT NULL,
  observed_at timestamptz NOT NULL,
  expires_at timestamptz NOT NULL CHECK (expires_at > observed_at),
  observation_json jsonb NOT NULL,
  UNIQUE (provider, region, observed_at),
  UNIQUE (observation_ref, provider, region)
);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_capacity_observations_latest
  ON khala_sync_cloud_capacity_observations (provider, region, observed_at DESC);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_capacity_policies (
  policy_ref text PRIMARY KEY,
  policy_digest text NOT NULL CHECK (policy_digest ~ '^sha256:[0-9a-f]{64}$'),
  provider text NOT NULL,
  region text NOT NULL,
  runtime_class text NOT NULL CHECK (runtime_class IN ('standard', 'strong', 'batch')),
  policy_json jsonb NOT NULL,
  updated_at timestamptz NOT NULL,
  UNIQUE (provider, region, runtime_class, policy_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_capacity_reservations (
  reservation_ref text PRIMARY KEY,
  command_ref text NOT NULL UNIQUE,
  request_digest text NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),
  computer_ref text NOT NULL REFERENCES khala_sync_cloud_computers (computer_ref),
  generation bigint NOT NULL CHECK (generation > 0),
  owner_ref text NOT NULL,
  tenant_ref text NOT NULL,
  conversation_ref text NOT NULL,
  runtime_class text NOT NULL CHECK (runtime_class IN ('standard', 'strong', 'batch')),
  priority text NOT NULL CHECK (priority IN ('normal', 'cleanup', 'replacement', 'recovery')),
  provider text NOT NULL,
  region text NOT NULL,
  status text NOT NULL CHECK (status IN ('queued', 'reserved', 'provisioning', 'provisioning_uncertain', 'starting', 'active', 'releasing', 'release_uncertain', 'released', 'expired', 'quarantined', 'refused')),
  reason text NOT NULL,
  cpu_millis bigint NOT NULL CHECK (cpu_millis > 0),
  memory_mib bigint NOT NULL CHECK (memory_mib > 0),
  scratch_mib bigint NOT NULL CHECK (scratch_mib >= 0),
  duration_seconds bigint NOT NULL CHECK (duration_seconds > 0),
  cost_micros bigint NOT NULL CHECK (cost_micros >= 0),
  queue_weight integer NOT NULL CHECK (queue_weight > 0),
  priority_rank integer NOT NULL CHECK (priority_rank BETWEEN 0 AND 3),
  virtual_finish numeric(30, 9) NOT NULL CHECK (virtual_finish >= 0),
  queue_sequence bigint GENERATED ALWAYS AS IDENTITY,
  observation_ref text,
  policy_ref text NOT NULL REFERENCES khala_sync_cloud_capacity_policies (policy_ref),
  policy_digest text NOT NULL CHECK (policy_digest ~ '^sha256:[0-9a-f]{64}$'),
  protected_authority_ref text,
  provider_operation_ref text,
  operation_revision bigint NOT NULL DEFAULT 0 CHECK (operation_revision >= 0),
  provider_lease_ref text,
  not_before_at timestamptz NOT NULL,
  started_at timestamptz,
  deadline_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  released_at timestamptz,
  demand_json jsonb NOT NULL,
  receipt_json jsonb NOT NULL,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  FOREIGN KEY (computer_ref, generation, owner_ref, tenant_ref, conversation_ref, runtime_class)
    REFERENCES khala_sync_cloud_computers
      (computer_ref, generation, owner_ref, tenant_ref, conversation_ref, runtime_class),
  FOREIGN KEY (observation_ref, provider, region)
    REFERENCES khala_sync_cloud_capacity_observations
      (observation_ref, provider, region),
  CHECK (deadline_at > created_at),
  CHECK (not_before_at >= created_at AND not_before_at < deadline_at),
  CHECK ((priority = 'normal' AND protected_authority_ref IS NULL)
      OR (priority <> 'normal' AND protected_authority_ref IS NOT NULL)),
  CHECK (provider_lease_ref IS NULL OR status IN ('starting', 'active', 'releasing', 'release_uncertain', 'released', 'quarantined')),
  CHECK ((released_at IS NOT NULL) = (status IN ('released', 'expired', 'refused')))
);

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_cloud_capacity_live_generation
  ON khala_sync_cloud_capacity_reservations (computer_ref, generation)
  WHERE status IN ('reserved', 'provisioning', 'provisioning_uncertain', 'starting', 'active', 'releasing', 'release_uncertain', 'quarantined');

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_cloud_capacity_provider_lease
  ON khala_sync_cloud_capacity_reservations (provider, region, provider_lease_ref)
  WHERE provider_lease_ref IS NOT NULL;

CREATE UNIQUE INDEX IF NOT EXISTS khala_sync_cloud_capacity_provider_operation
  ON khala_sync_cloud_capacity_reservations (provider_operation_ref)
  WHERE provider_operation_ref IS NOT NULL;

CREATE INDEX IF NOT EXISTS khala_sync_cloud_capacity_scope_usage
  ON khala_sync_cloud_capacity_reservations
    (provider, region, tenant_ref, conversation_ref, runtime_class, status);

CREATE INDEX IF NOT EXISTS khala_sync_cloud_capacity_fair_queue
  ON khala_sync_cloud_capacity_reservations
    (provider, region, status, priority_rank, virtual_finish, queue_sequence);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_capacity_fair_flows (
  provider text NOT NULL,
  region text NOT NULL,
  tenant_ref text NOT NULL,
  conversation_ref text NOT NULL,
  tenant_virtual_finish numeric(30, 9) NOT NULL DEFAULT 0,
  conversation_virtual_finish numeric(30, 9) NOT NULL DEFAULT 0,
  revision bigint NOT NULL DEFAULT 1 CHECK (revision > 0),
  PRIMARY KEY (provider, region, tenant_ref, conversation_ref)
);

CREATE TABLE IF NOT EXISTS khala_sync_cloud_capacity_drift (
  drift_ref text PRIMARY KEY,
  provider text NOT NULL,
  region text NOT NULL,
  provider_lease_ref text NOT NULL,
  reservation_ref text REFERENCES khala_sync_cloud_capacity_reservations (reservation_ref),
  computer_ref text,
  expected_generation bigint CHECK (expected_generation IS NULL OR expected_generation > 0),
  observed_generation bigint CHECK (observed_generation IS NULL OR observed_generation > 0),
  kind text NOT NULL CHECK (kind IN ('leaked', 'missing', 'double_claimed', 'generation_mismatch', 'operation_mismatch', 'quarantined')),
  evidence_digest text NOT NULL CHECK (evidence_digest ~ '^sha256:[0-9a-f]{64}$'),
  observed_at timestamptz NOT NULL,
  resolved_at timestamptz,
  resolution_kind text,
  resolution_evidence_digest text CHECK (resolution_evidence_digest IS NULL OR resolution_evidence_digest ~ '^sha256:[0-9a-f]{64}$'),
  resolver_operation_ref text,
  UNIQUE (provider, region, provider_lease_ref, kind, observed_at)
);
