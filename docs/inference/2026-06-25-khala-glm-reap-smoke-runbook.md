# Khala GLM REAP Smoke Runbook

This runbook covers the CI-safe smoke for GitHub #6259. It verifies that the
public `openagents/khala` gateway is served by the GLM-5.2 REAP Hydralisk lane
when that lane is armed, without printing raw endpoint URLs, bearer tokens,
prompts, completions, or private Harbor logs.

## Command

From `apps/openagents.com`:

```bash
OPENAGENTS_AGENT_TOKEN=oa_agent_... \
HYDRALISK_GLM_52_REAP_504B_ENABLED=ready \
HYDRALISK_GLM_52_REAP_504B_BASE_URL=<secret endpoint> \
HYDRALISK_GLM_52_REAP_504B_BEARER_TOKEN=<secret bearer> \
HYDRALISK_GLM_52_REAP_504B_PREFLIGHT_REF=<public-safe preflight ref> \
HYDRALISK_GLM_52_REAP_504B_RECEIPT_REF=<public-safe receipt ref> \
bun run smoke:khala:glm-reap -- --approve-live-spend
```

If the GLM arming env is absent or unsafe, the script exits `0` with
`state: "skipped"` and lists only public-safe blocker ref names. That keeps CI
green until an owner intentionally arms the lane.

## Checks

- `POST /api/v1/chat/completions` with `model: openagents/khala`
  non-streaming and streaming.
- `openagents` disclosure and dereferenced usage receipt both report
  `supply_lane: hydralisk`, worker `hydralisk-vllm-glm-5p2-reap-504b`, and a
  served model containing `glm-5.2-reap-504b`.
- `GET /api/v1/models` lists only the public Khala model and does not expose the
  raw `openagents/glm-5.2-reap-504b` id.
- `GET /api/public/khala-tokens-served` increases by at least the summed
  non-streaming plus streaming receipt token total, subject to the optional
  `--counter-tolerance-tokens` value.

The script includes public-safe serving profile refs in its output for the GLM
REAP G4 profiles:

- `glm-reap-504b-g4-tp4-minp-rp105`
- `glm-reap-504b-g4-tp4-mtp2-rp105`
- `glm-reap-504b-g4-dual-tp4-minp-rp105`

Add another public-safe profile ref with `--serving-profile-ref <ref>` if the
operator rotates the serving profile.

## Validation

```bash
cd apps/openagents.com
bunx vitest run scripts/khala-glm-reap-smoke.test.ts scripts/khala-production-smoke.test.ts
```

The unit tests cover the unarmed skip, canonical `/api/v1` path use, GLM receipt
classification, raw-model catalog closure, and counter-increment failure mode.
