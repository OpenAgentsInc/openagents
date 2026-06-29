-- Paid-privacy / confidential-compute capture opt-OUT (openagents #6295, child
-- of #6293, epic #6206).
--
-- Default-on free-tier trace capture (#6293) captures everything UNLESS the
-- caller is paying for privacy. This table is the per-account "paid-for-privacy
-- / zero-retention / confidential" marker resolved at the chat seam alongside
-- the free-tier signal. A row here means: this account's traffic is NEVER
-- auto-captured (the emit closure short-circuits), regardless of the global
-- capture flag.
--
-- The capture decision is `captureDefault = freeTier.free && !paidPrivacy`. A
-- read error on this table resolves FAIL-CLOSED-TO-PRIVATE (treat as
-- paid-privacy => do not capture), the inverse of the free-tier gate's
-- fail-closed-to-paid. The deployment-wide confidential-compute mode
-- (env INFERENCE_CONFIDENTIAL_COMPUTE_ENABLED) is a separate, stronger signal
-- that excludes EVERY caller without needing a per-account row.
--
-- PUBLIC-SAFE: every row carries an account ref, a bounded tier label, an
-- optional public-safe note, and timestamps only — never prompts, completions,
-- wallet/payment material, raw tokens, or secrets.

CREATE TABLE IF NOT EXISTS inference_privacy_entitlements (
  -- The authenticated account ref (`agent:<userId>` / `user:<userId>`), the SAME
  -- principal the balance gate, free-tier gate, and metering hook key on.
  account_ref TEXT PRIMARY KEY,
  -- Reserves room for future privacy tiers; today a row marks the account as
  -- paying for privacy / confidential compute (excluded from capture).
  privacy_tier TEXT NOT NULL DEFAULT 'paid_privacy',
  -- Optional public-safe note (audit). Never payment material or secrets.
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
