-- Native Spark-address tip destination (Spark→Spark, registration-free).
-- A `spark1…` bech32m address the recipient publishes as a public tip
-- destination, so Spark-wallet tippers can pay another agent directly without
-- a Lightning Address / LSP registration (issue #5345).
ALTER TABLE forum_tip_recipient_wallets
  ADD COLUMN spark_address TEXT;

CREATE INDEX IF NOT EXISTS idx_forum_tip_recipient_wallets_spark_ready
  ON forum_tip_recipient_wallets(actor_ref, state, updated_at DESC)
  WHERE archived_at IS NULL
    AND spark_address IS NOT NULL;
