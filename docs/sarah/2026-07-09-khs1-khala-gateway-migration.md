# KHS-1: Sarah inference through the Khala gateway (#8600)

Status: **shipped flag-gated, default OFF**. With no new env set, Sarah's
behavior is byte-for-byte the pre-KHS-1 direct-Google path (raw
`GEMINI_API_KEY` → `generativelanguage.googleapis.com`). Epic: #8599.
Foundation: `docs/fable/2026-07-09-sarah-khala-connection-assessment.md`
§KH-S-1.

## What changed

`apps/sarah/src/services/google-inference.ts` now carries two transports
behind the same `generateSarahGemmaReply` / `streamSarahGemmaReply` contract:

1. **Khala gateway** (new, KHS-1): OpenAI-compatible
   `POST {SARAH_INFERENCE_GATEWAY_URL}/chat/completions` with the agent
   bearer token, requesting `openagents/khala`. The Khala conversational
   adapter plan LEADS with the same Gemma 4 gcloud lane Sarah pins today and
   overflows to Vertex Gemini → Fireworks → GLM, replacing Sarah's
   hand-rolled `SARAH_TEXT_MODEL_FALLBACKS` 429 model chain with the
   gateway's quota-aware multi-lane dispatch (the durable fix for the
   2026-07-09 RPM 429 storm).
2. **Direct Google** (legacy): unchanged, used only while the gateway env is
   absent.

Preserved on both transports:

- **Streaming with fast first byte** — gateway deltas are forwarded
  frame-by-frame off the gateway's pass-through stream; the avatar brain's
  immediate role-chunk + keepalive behavior in `llm-openai-compat.ts` is
  untouched.
- **Thought filtering** — the gateway's Gemma 4 adapter already strips
  `thought: true` scratchpad parts from `content` and routes them to the
  separate `reasoning_content` delta channel. Sarah reads ONLY
  `delta.content` / `message.content`, so scratchpad text cannot surface on
  either transport (tested).
- **Deterministic pricing guard** — upstream of the model on every path;
  not touched by this change.
- **Typed errors** — gateway errors are `gateway_inference_http_{status}` /
  `gateway_inference_timeout` / `gateway_inference_unreachable` /
  `gateway_inference_empty_reply`; callers classify busy-vs-broken via
  `isSarahInferenceBusyError`.
- **Exact-only usage** — non-streaming usage maps from the gateway's
  provider-reported `usage`; `thoughtTokens` is the exact reconciliation gap
  `total − (prompt + completion)` (the same `unaccountedTokens` derivation
  the gateway telemetry discloses). Streaming usage reads the terminal
  chunk's `openagents.telemetry` token counts; `not_measured` sentinels
  degrade to 0, never an estimate.

## Receipts / attribution

Every gateway turn lands an exact `token_usage_events` row via the gateway's
served-tokens recorder. Sarah self-attributes as internal demand with the
existing header rail (`chat-completions-routes.ts`
`requestAttributionFromHeaders`):

- `x-openagents-demand-kind: internal`
- `x-openagents-demand-source: sarah`
- `x-openagents-client: sarah-server`

`demand_source` is a bounded free-form token, so no API-side enum change was
needed — `sarah` rows are distinguishable in the demand ledger exactly like
`heartbeat` / `canary`. This is deliberately NOT the org-cloud no-meter
header (`x-openagents-org-cloud-runtime-no-meter`): Sarah's usage is metered
own/internal demand with receipts, never a metering bypass.

## Cost cap (deliverable 3)

`SARAH_TEXT_DAILY_TOKEN_CAP` (unset = no-op): a process-local UTC-day
counter of provider-reported total tokens (exact-only, both transports).
Once reached, calls refuse with the typed
`sarah_daily_token_cap_exceeded` BEFORE any provider call; callers surface
the canned busy reply. Best-effort by design — the authoritative ledger is
`token_usage_events`.

## Arming plan (staging first)

1. Mint/choose Sarah's agent account on the target environment and get its
   bearer token (the standard per-account agent credential the
   `/api/v1/chat/completions` auth resolves). Add its account ref to
   `INFERENCE_INTERNAL_ACCOUNT_REFS` on the worker (header-independent
   internal-attribution backstop, `inference-internal-account.ts`).
2. Fund the lane one of two documented ways (owner choice):
   - grant the account internal credits, or
   - use the operator exemption (`INFERENCE_OPERATOR_EXEMPTION_ENABLED` +
     owner grant, `inference-operator-exemption.ts`) — honest
     `operator_credit` zero-debit receipts; allowed because
     `openagents/khala`'s conversational lanes are non-premium classes.
3. Set on apps/sarah (staging):
   - `SARAH_INFERENCE_GATEWAY_URL=https://<staging-host>/api/v1`
   - `SARAH_INFERENCE_GATEWAY_TOKEN=<agent bearer token>`
   - optional `SARAH_INFERENCE_GATEWAY_MODEL` (default `openagents/khala`)
   - optional `SARAH_TEXT_DAILY_TOKEN_CAP`
4. Verify: `GET /sarah/api/ops` shows
   `modelPath: khala_gateway_live:openagents/khala`; run a text turn and a
   streaming avatar turn; confirm a `token_usage_events` row with
   `demand_kind=internal`, `demand_source=sarah` per turn and that first-byte
   latency on the avatar lane stays fast under sustained speech (no
   429-storm fallbacks).
5. Prod: repeat 3-4 on prod, then (exit criterion for #8599) remove
   `GEMINI_API_KEY` from apps/sarah's env so zero raw provider keys remain —
   the code no longer needs it once the gateway env is armed.

Rollback at any point: unset the two `SARAH_INFERENCE_GATEWAY_*` vars — the
direct-Google path resumes unchanged.
