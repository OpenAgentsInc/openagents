-- Generation-bound cumulative token reservations below managed-sandbox prompts.

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_provider_budgets (
  sandbox_ref text NOT NULL,
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  turn_ref text NOT NULL,
  capability_ref text NOT NULL,
  max_tokens bigint NOT NULL CHECK (max_tokens > 0),
  consumed_tokens bigint NOT NULL DEFAULT 0 CHECK (consumed_tokens >= 0),
  deadline_at timestamptz NOT NULL,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY (sandbox_ref, resource_generation, turn_ref, capability_ref),
  CHECK (consumed_tokens <= max_tokens)
);

CREATE TABLE IF NOT EXISTS khala_sync_managed_sandbox_provider_budget_reservations (
  reservation_ref text PRIMARY KEY,
  sandbox_ref text NOT NULL,
  resource_generation bigint NOT NULL CHECK (resource_generation > 0),
  turn_ref text NOT NULL,
  capability_ref text NOT NULL,
  reserved_tokens bigint NOT NULL CHECK (reserved_tokens > 0),
  observed_at timestamptz NOT NULL,
  FOREIGN KEY (sandbox_ref, resource_generation, turn_ref, capability_ref)
    REFERENCES khala_sync_managed_sandbox_provider_budgets
      (sandbox_ref, resource_generation, turn_ref, capability_ref)
    ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS khala_sync_managed_sandbox_provider_budget_turn
  ON khala_sync_managed_sandbox_provider_budget_reservations
    (sandbox_ref, resource_generation, turn_ref);
