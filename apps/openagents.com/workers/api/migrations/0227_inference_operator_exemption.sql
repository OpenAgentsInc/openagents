-- Inference gateway owner-controlled BALANCE-GATE EXEMPTION (issue #6180):
-- approved/internal keys may test our OWN non-premium inference lanes (e.g.
-- `openagents/khala` / `khala-oss-20b` — GPT-OSS on the hourly Hydralisk box,
-- zero marginal per-token cost to us) WITHOUT a funded balance, while Khala
-- stays a paid product for the public.
--
-- PUBLIC-SAFE: the table carries an owner/account ref, a scope string, an
-- actor ref, a free-text note, and timestamps only — never prompts, completions,
-- wallet/payment material, addresses, tokens, or secrets.
--
-- INERT until INFERENCE_OPERATOR_EXEMPTION_ENABLED is on AND the gateway is
-- enabled: no code path reads/writes this table on the flag-off route.
--
-- IDENTITY: rows key on the VERIFIED OWNER identity (`owner:<userId>`), the SAME
-- owner key the free pool / premium allowlist use, so granting one verified
-- owner covers ALL of that owner's accounts/autopilots. A synthetic unclaimed
-- `account:<ref>` key is NEVER inserted (the owner/admin grant surface refuses
-- it) — an unclaimed account has no verified owner to approve.
--
-- GUARDRAIL (enforced in code, not schema): the exemption applies ONLY to
-- non-premium / own-infra model classes (`gemini`, `open` incl. the `hydralisk`
-- GPT-OSS lane). A premium model (`claude`, `unknown`/passthrough) is NEVER
-- exempted — it still hits the normal balance + premium-grant gates. An exempt
-- request is metered as `operator_credit` (zero credit debit + receipt), an
-- honest zero-debit record, never a silent skip and never a ledger movement.

CREATE TABLE IF NOT EXISTS inference_operator_exemption (
  owner_key TEXT PRIMARY KEY,
  -- Reserves room for per-lane scopes later; today a row exempts the whole
  -- non-premium / own-infra lane set.
  scope TEXT NOT NULL DEFAULT 'own_infra_non_premium',
  -- The owner/admin actor ref that granted the exemption (audit). Never a token.
  granted_by TEXT,
  note TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);
