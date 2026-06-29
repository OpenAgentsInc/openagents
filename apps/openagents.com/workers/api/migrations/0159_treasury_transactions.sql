-- Campaign treasury public transaction ledger (issues #4698-#4700).
-- Public projections expose direction, amount, time, and state only;
-- payment_ref and bolt11 stay internal except on the donation's own page.
CREATE TABLE IF NOT EXISTS treasury_transactions (
  id TEXT PRIMARY KEY,
  direction TEXT NOT NULL CHECK (direction IN ('in', 'out')),
  amount_sat INTEGER NOT NULL DEFAULT 0,
  state TEXT NOT NULL CHECK (state IN ('pending', 'settled', 'expired')),
  bolt11 TEXT,
  payment_ref TEXT,
  created_at TEXT NOT NULL,
  settled_at TEXT,
  expires_at TEXT
);

CREATE INDEX IF NOT EXISTS idx_treasury_transactions_created_at
  ON treasury_transactions (created_at DESC);
