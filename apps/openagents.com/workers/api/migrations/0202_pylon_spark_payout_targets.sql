-- #5252: private operator-only store for raw Spark payout targets.
-- The raw `spark1…` is PAYMENT MATERIAL. It lives ONLY in this private table,
-- keyed by pylon_ref and bound to the owning agent (owner_agent_user_id). It is
-- never projected into a public event, log, or public projection. Public
-- surfaces carry only the redacted `payout.spark.<digest>` ref
-- (payout_target_ref). The settlement payout resolver reads raw_spark_address
-- from this private table to pay a registered recipient natively over Spark.
CREATE TABLE IF NOT EXISTS pylon_spark_payout_targets (
  pylon_ref TEXT PRIMARY KEY,
  owner_agent_user_id TEXT NOT NULL,
  payout_target_ref TEXT NOT NULL,
  raw_spark_address TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (pylon_ref) REFERENCES pylon_api_registrations(pylon_ref)
);

CREATE INDEX IF NOT EXISTS idx_pylon_spark_payout_targets_owner
  ON pylon_spark_payout_targets(owner_agent_user_id, updated_at DESC);
