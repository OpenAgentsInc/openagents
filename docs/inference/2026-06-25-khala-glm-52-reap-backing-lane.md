# Khala GLM-5.2 REAP Hydralisk Backing Lane

Date: 2026-06-25

## Summary

Khala now has a private Hydralisk GLM-5.2 REAP 504B backing lane in the
gateway mix. The public API surface remains the single model selector:

```text
openagents/khala
```

When the lane is armed, the router tries the private GLM-5.2 REAP worker first,
then the existing Hydralisk GPT-OSS workers, then Vertex Gemini as the final
degradation lane. Raw supply-lane IDs are not advertised in the public catalog.

## Operator Contract

The GLM lane is fail-closed and only arms when all transport and evidence fields
are present:

```text
HYDRALISK_GLM_52_REAP_504B_ENABLED=ready
HYDRALISK_GLM_52_REAP_504B_BASE_URL=<private OpenAI-compatible base URL>
HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<secret bearer token>
HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-safe preflight evidence ref>
HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-safe smoke/receipt evidence ref>
```

Only the variable names and evidence-reference names belong in tracked files.
Endpoint values, bearer tokens, and private topology stay in Worker secrets or
operator-only notes.

## Expected Behavior

- `openagents/khala` uses the GLM Hydralisk adapter first when the GLM lane is
  armed and registered.
- If the GLM worker is not armed, the router skips it and continues through the
  rest of the Khala plan.
- The GLM backing lane counts as Hydralisk supply for disclosures, usage
  receipts, and billing context.
- Direct customer selection remains closed; public model listing still exposes
  Khala, not internal supply workers.

## Verification

The code path is covered by API tests for:

- Khala router ordering.
- Direct internal adapter selection.
- Fail-closed arming behavior.
- Catalog exclusion for the raw supply lane.
- Chat-completions routing, disclosure, and metering context.

Run the deployment gate from `apps/openagents.com` before production rollout:

```sh
bun run check:deploy
```
