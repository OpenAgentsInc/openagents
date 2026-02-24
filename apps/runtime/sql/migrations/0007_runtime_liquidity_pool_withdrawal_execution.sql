CREATE SCHEMA IF NOT EXISTS runtime;

ALTER TABLE runtime.liquidity_withdrawals
    ADD COLUMN IF NOT EXISTS payout_invoice_bolt11 TEXT;

