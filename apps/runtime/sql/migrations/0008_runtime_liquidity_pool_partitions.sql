-- Liquidity Pool partitions (segregated sub-ledgers within a single pool_id).
--
-- We choose "one pool with partition sub-ledgers" rather than separate pools per kind so LPs can
-- hold distinct exposures (LLP / CEP / RRP) under the same pool identity while keeping accounting
-- segregated and share pricing partition-scoped.

CREATE SCHEMA IF NOT EXISTS runtime;

ALTER TABLE runtime.liquidity_lp_accounts
    ADD COLUMN IF NOT EXISTS partition_kind TEXT NOT NULL DEFAULT 'llp';

-- Partition kind becomes part of the LP account identity.
ALTER TABLE runtime.liquidity_lp_accounts
    DROP CONSTRAINT IF EXISTS liquidity_lp_accounts_pkey;
ALTER TABLE runtime.liquidity_lp_accounts
    ADD PRIMARY KEY (pool_id, lp_id, partition_kind);

ALTER TABLE runtime.liquidity_deposits
    ADD COLUMN IF NOT EXISTS partition_kind TEXT NOT NULL DEFAULT 'llp';

ALTER TABLE runtime.liquidity_deposits
    DROP CONSTRAINT IF EXISTS liquidity_deposits_pool_id_lp_id_idempotency_key_key;
ALTER TABLE runtime.liquidity_deposits
    ADD CONSTRAINT liquidity_deposits_pool_lp_partition_idem_key
        UNIQUE (pool_id, lp_id, partition_kind, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_liquidity_deposits_pool_partition_status
    ON runtime.liquidity_deposits (pool_id, partition_kind, status);

ALTER TABLE runtime.liquidity_withdrawals
    ADD COLUMN IF NOT EXISTS partition_kind TEXT NOT NULL DEFAULT 'llp';

ALTER TABLE runtime.liquidity_withdrawals
    DROP CONSTRAINT IF EXISTS liquidity_withdrawals_pool_id_lp_id_idempotency_key_key;
ALTER TABLE runtime.liquidity_withdrawals
    ADD CONSTRAINT liquidity_withdrawals_pool_lp_partition_idem_key
        UNIQUE (pool_id, lp_id, partition_kind, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_liquidity_withdrawals_pool_partition_status
    ON runtime.liquidity_withdrawals (pool_id, partition_kind, status);

ALTER TABLE runtime.liquidity_pool_snapshots
    ADD COLUMN IF NOT EXISTS partition_kind TEXT NOT NULL DEFAULT 'llp';

CREATE INDEX IF NOT EXISTS idx_liquidity_pool_snapshots_pool_partition_as_of
    ON runtime.liquidity_pool_snapshots (pool_id, partition_kind, as_of DESC);

