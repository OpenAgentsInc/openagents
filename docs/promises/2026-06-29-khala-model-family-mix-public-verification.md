# Khala Model-Family Mix Public Verification

Date: 2026-06-29

Issue: #7016

Promise: `metrics.khala_model_family_mix_public.v1`

## Result

`GET https://openagents.com/api/public/khala-tokens-served/model-mix` was
re-run against production data on 2026-06-29. The projection is live at read and
public-safe:

- `schemaVersion`: `openagents.public_khala_model_mix.v1`
- `window`: `30d`
- `totalTokens`: positive production aggregate
- `groups`: aggregate public family rows only: `family`, `label`, `tokens`,
  `reqs`, `pct`
- `liveAt` and `generatedAt`: present on the response
- `staleness`: `projection_staleness.v1`, `composition: "live_at_read"`,
  `maxStalenessSeconds: 0`, `rebuildsOn: ["token_usage_events"]`

The response groups collapse raw provider/model identities into stable public
families such as `pylon_codex`, `fireworks_deepseek`, `glm`, `gemini`,
`pylon_claude`, and `other`. The public payload did not expose raw provider ids,
raw model ids, user/team identifiers, prompts, raw traces, payment material, or
secrets.

## Promise Decision

The source machinery and production projection evidence are sufficient for the
live-at-read public transparency surface. The promise remains `yellow` because
this task did not record an owner-signed yellow-to-green transition receipt.

Remaining blocker:

- `blocker.product_promises.model_mix_green_owner_signoff_pending`: record an
  owner-signed `promise_transition` receipt under
  `proof.claim_upgrade_receipts.v1` before flipping the registry state to
  `green`.

No cached or fuzzy freshness wording is authorized. Public copy must keep saying
`maxStalenessSeconds:0` for this projection.
