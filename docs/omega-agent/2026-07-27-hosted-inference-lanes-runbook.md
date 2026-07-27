# Omega Agent hosted inference lanes runbook

- Status: production runbook
- Owner: OpenAgents
- Date: 2026-07-27
- Audience: Omega Agent, inference, and production operators
- Production service: `openagents-monolith`
- Production region: `us-central1`

This runbook operates the two explicitly selectable hosted inference
lanes used by Omega Agent:

| Requested model                     | Provider lane | Provider wire model                 |
| ----------------------------------- | ------------- | ----------------------------------- |
| `gemini-3.6-flash`                  | Vertex Gemini | `gemini-3.6-flash`                  |
| `kimi-k3`                           | Fireworks     | `accounts/fireworks/models/kimi-k3` |
| `accounts/fireworks/models/kimi-k3` | Fireworks     | `accounts/fireworks/models/kimi-k3` |

The OpenAI-compatible endpoint is:

```text
POST https://openagents.com/api/v1/chat/completions
```

`/v1/chat/completions` is an equivalent route.

## 1. Access boundary

These are internal hosted lanes, not public catalog models.

1. The authenticated account must appear in
   `INFERENCE_INTERNAL_ACCOUNT_REFS`.
2. The requested provider lane must be armed in the running
   deployment.
3. `/api/v1/models` continues to advertise only
   `openagents/khala`.
4. External accounts cannot use OpenAgents-funded Gemini or Fireworks
   capacity by naming a backing model.
5. Never add either hosted model to the public catalog as part of an
   operations change. Broadening access requires separate owner
   approval and an `INVARIANTS.md` update.

Authentication uses the same `oa_agent_...` bearer credential as the
existing OpenAI-compatible inference route. Do not put credentials in
source, tracked environment files, shell history, screenshots, or
runbook output.

## 2. Select a lane

The `model` property is the lane selector. No extra routing header is
required.

### Gemini Flash

```bash
curl -fsS https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer ${OPENAGENTS_AGENT_TOKEN}" \
  -H "Content-Type: application/json" \
  --data '{
    "model": "gemini-3.6-flash",
    "max_tokens": 128,
    "messages": [
      {
        "role": "user",
        "content": "Reply with the single word ready."
      }
    ]
  }'
```

### Kimi K3

Both the short and provider-qualified IDs are accepted. Prefer the
provider-qualified ID in operational probes because it makes the
intended provider explicit.

```bash
curl -fsS https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer ${OPENAGENTS_AGENT_TOKEN}" \
  -H "Content-Type: application/json" \
  --data '{
    "model": "accounts/fireworks/models/kimi-k3",
    "max_tokens": 128,
    "top_k": 40,
    "presence_penalty": 0,
    "frequency_penalty": 0,
    "messages": [
      {
        "role": "user",
        "content": "Reply with the single word ready."
      }
    ]
  }'
```

Kimi K3 also accepts OpenAI-compatible multimodal message content:

```json
{
  "model": "accounts/fireworks/models/kimi-k3",
  "max_tokens": 131072,
  "top_k": 40,
  "presence_penalty": 0,
  "frequency_penalty": 0,
  "messages": [
    {
      "role": "user",
      "content": [
        {
          "type": "text",
          "text": "Can you describe this image?"
        },
        {
          "type": "image_url",
          "image_url": {
            "url": "https://example.com/image.jpg"
          }
        }
      ]
    }
  ]
}
```

Use a small `max_tokens` value for health probes. The larger value
above documents the admitted request shape. It is not appropriate for
a routine smoke test.

## 3. How routing works

The chat route preserves the requested model after authentication and
checks it against the exact hosted-lane allowlist. The pricing table
classifies Kimi K3 as Fireworks and Gemini Flash as Vertex Gemini.
The router selects the registered adapter for that lane.

For Kimi K3, the Fireworks adapter:

1. Normalizes `kimi-k3` to
   `accounts/fireworks/models/kimi-k3`.
2. Sends the request to
   `https://api.fireworks.ai/inference/v1/chat/completions`.
3. Preserves text and `image_url` content parts.
4. Passes admitted OpenAI sampling properties through.
5. Uses the provider usage receipt for metering.

The text-only projection of a multimodal message remains available for
routing, tracing, and prompt classification. The original structured
content is used on the OpenAI-compatible Fireworks wire request.

The Kimi K3 pricing row records the standard Fireworks rates effective
on 2026-07-27:

| Dimension    | USD per million tokens |
| ------------ | ---------------------: |
| Input        |                   3.00 |
| Cached input |                   0.30 |
| Output       |                  15.00 |

Before changing the row, verify the current official Fireworks
serverless pricing and update the tests with the same change.

## 4. Configuration and secrets

Production configuration is in:

```text
apps/openagents.com/workers/api/scripts/cloudrun/env-production.yaml
```

The deploy script mounts these existing Secret Manager entries:

| Runtime variable    | Secret Manager entry           |
| ------------------- | ------------------------------ |
| `FIREWORKS_API_KEY` | `openagents-fireworks-api-key` |
| `VERTEX_SA_KEY`     | `openagents-vertex-sa-key`     |
| `GEMINI_API_KEY`    | `openagents-gemini-api-key`    |

The deployment must also have `INFERENCE_GATEWAY_ENABLED` enabled and
the intended account in `INFERENCE_INTERNAL_ACCOUNT_REFS`.

Do not retrieve or print a secret merely to confirm it exists. Check
the Cloud Run revision's secret bindings or Secret Manager metadata.

## 5. Pre-deploy verification

Run from `apps/openagents.com`:

```bash
pnpm --filter @openagentsinc/api-worker exec vitest run \
  src/inference/chat-completions-routes.test.ts \
  src/inference/fireworks-adapter.test.ts \
  src/inference/model-router.test.ts \
  src/inference/model-serving-policy.test.ts \
  src/inference/pricing.test.ts

pnpm --filter @openagentsinc/api-worker typecheck

node --import tsx ../../scripts/check-ste.ts INVARIANTS.md

git diff --check
```

The expected focused receipt for the 2026-07-27 admission is:

```text
Test Files  5 passed (5)
Tests       157 passed (157)
```

The full deployment gate is:

```bash
pnpm run check:deploy
```

If `check:effect-topology` reports manifests only under nested
`.claude/worktrees`, `.pylon-local/cache`, or `.worktrees` paths,
record that environmental failure separately. Do not delete, edit, or
commit another operator's worktrees to make the scanner pass. The
change-scoped tests, typecheck, STE check, and deployment build must
still pass.

## 6. Deploy production

Cloud Run is the only production deployment path for this application.
Do not use Wrangler.

From `apps/openagents.com`:

```bash
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  bash workers/api/scripts/deploy-cloudrun.sh production
```

The script builds the Start application and API bundle, stages runtime
dependencies, deploys a new Cloud Run revision, routes traffic, and
runs health, tombstone, and logged-out portal browser smokes.

The initial production admission deployed:

```text
Revision: openagents-monolith-00280-h6r
Traffic: 100 percent
Date: 2026-07-27
```

Treat that revision as a deployment receipt, not as a permanently
configured target.

## 7. Post-deploy verification

### Service and web assets

```bash
curl -fsS \
  https://openagents-monolith-ezxz4mgdsq-uc.a.run.app/internal/healthz

curl -fsSI https://openagents.com/
```

Read the current JavaScript asset path from the homepage and verify
that exact asset:

```bash
page_html="$(curl -fsS https://openagents.com/)"
asset_path="$(
  printf '%s' "${page_html}" |
    rg -o '/assets/[^" ]+\.js' |
    head -n 1
)"
test -n "${asset_path}"
curl -fsSI "https://openagents.com${asset_path}"
```

### Public catalog boundary

```bash
curl -fsS https://openagents.com/api/v1/models |
  jq -e '.data | map(.id) == ["openagents/khala"]'
```

### Authentication boundary

An unauthenticated hosted-lane request must return `401`:

```bash
status="$(
  curl -sS -o /dev/null -w '%{http_code}' \
    -X POST https://openagents.com/api/v1/chat/completions \
    -H 'Content-Type: application/json' \
    --data '{
      "model": "accounts/fireworks/models/kimi-k3",
      "messages": [{"role": "user", "content": "ping"}]
    }'
)"
test "${status}" = "401"
```

### Authenticated lane probes

Use an internal account credential supplied through the operator's
approved secret channel. Run the small Gemini and Kimi requests in
section 2. Confirm:

1. Both responses are HTTP 200.
2. The response `model` or serving receipt matches the intended lane.
3. Usage contains real provider token counts.
4. A Kimi multimodal probe preserves both content parts.
5. No credential or prompt content appears in deployment logs.

## 8. Failure diagnosis

| Symptom                            | Meaning and response                                                                                                                               |
| ---------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `401`                              | Missing or invalid agent bearer. Check credential delivery without printing it.                                                                    |
| `404 model_not_found`              | The account is not internal, the ID is not one of the exact admitted IDs, or a fine-tuned model lookup failed. Check the account ref and spelling. |
| `503 model_unavailable`            | The selected lane is not armed. Check the revision's secret binding and provider adapter registration.                                             |
| `503 platform_funding_unavailable` | The caller is not authorized to use platform capacity. Do not bypass this gate. Use an internal account or the established BYOK path.              |
| Fireworks `401` or `403`           | Check the `FIREWORKS_API_KEY` secret version and binding. Rotate through Secret Manager. Never put the key in source.                              |
| Fireworks model error              | Confirm the exact provider wire ID is still `accounts/fireworks/models/kimi-k3`.                                                                   |
| Missing image at Fireworks         | Confirm `content` is an array of valid `text` and `image_url` parts and that the image URL is reachable by Fireworks.                              |
| Missing usage receipt              | Do not estimate or silently settle usage. Inspect the provider response and adapter receipt handling.                                              |
| Gemini failure with Kimi healthy   | Inspect Vertex service-account binding and the `vertex-gemini` lane independently.                                                                 |
| Kimi failure with Gemini healthy   | Inspect Fireworks secret binding, model availability, and Fireworks status independently.                                                          |

## 9. Rollback

List recent revisions:

```bash
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  gcloud run revisions list \
  --project openagentsgemini \
  --region us-central1 \
  --service openagents-monolith
```

Route all traffic to the last known-good revision:

```bash
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  gcloud run services update-traffic openagents-monolith \
  --project openagentsgemini \
  --region us-central1 \
  --to-revisions LAST_KNOWN_GOOD_REVISION=100
```

Resolve the exact revision from the list before running the command.
Never guess a revision name. After rollback, repeat every check in
section 7.

## 10. Code ownership map

| Concern                                   | Source                                                                     |
| ----------------------------------------- | -------------------------------------------------------------------------- |
| Request decoding and internal access gate | `apps/openagents.com/workers/api/src/inference/chat-completions-routes.ts` |
| Exact hosted-lane policy                  | `apps/openagents.com/workers/api/src/inference/model-serving-policy.ts`    |
| Model classification and pricing          | `apps/openagents.com/workers/api/src/inference/pricing.ts`                 |
| Provider selection                        | `apps/openagents.com/workers/api/src/inference/model-router.ts`            |
| Fireworks wire request                    | `apps/openagents.com/workers/api/src/inference/fireworks-adapter.ts`       |
| Gemini wire request                       | `apps/openagents.com/workers/api/src/inference/vertex-gemini-adapter.ts`   |
| OpenAI-compatible multimodal mapping      | `apps/openagents.com/workers/api/src/inference/openai-chat-compat.ts`      |
| Production deployment                     | `apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh`               |
| Security and product invariant            | `apps/openagents.com/INVARIANTS.md`                                        |

## 11. External references

- Fireworks serverless pricing:
  <https://docs.fireworks.ai/serverless/pricing>
- Fireworks Kimi K3 model announcement and capabilities:
  <https://fireworks.ai/blog/kimik3-on-fireworks>
