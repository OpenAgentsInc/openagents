-- Allow outbound treasury attempts to terminally reconcile as failed.
-- Pending means the MDK payment outcome is still unknown; failed means the
-- container reported a terminal failure for the stored internal payment id.
PRAGMA defer_foreign_keys=ON;

CREATE TABLE treasury_transactions_0197_new (
  id TEXT PRIMARY KEY,
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  amount_sat INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL CHECK (state IN ('pending', 'settled', 'expired', 'failed')),
  bolt11 TEXT,
  payment_ref TEXT,
  created_at TEXT NOT NULL,
  settled_at TEXT,
  expires_at TEXT
);

INSERT INTO treasury_transactions_0197_new (
  id,
  direction,
  amount_sat,
  state,
  bolt11,
  payment_ref,
  created_at,
  settled_at,
  expires_at
)
SELECT
  id,
  direction,
  amount_sat,
  state,
  bolt11,
  payment_ref,
  created_at,
  settled_at,
  expires_at
FROM treasury_transactions;

DROP TABLE treasury_transactions;

ALTER TABLE treasury_transactions_0197_new RENAME TO treasury_transactions;

CREATE INDEX IF NOT EXISTS idx_treasury_transactions_created_at
  ON treasury_transactions (created_at DESC);

PRAGMA defer_foreign_keys=OFF;
