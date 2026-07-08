# M13 Live Gemini Provider Gate Record

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-12

Issue: #4771

## Claim

The remaining M13 live leg is satisfied for Google Gemini API-key BYOK:

- a non-Codex provider account is connected in production,
- the account is visible in the provider-account pool dashboard projection,
- the operator lease policy can select it with `requiredProvider: google_gemini`,
- the issued grant resolves to a Probe-compatible Gemini materialization plan,
- a Probe runner can consume that production-resolved grant shape for a live
  Gemini backend call, and
- the lease is released back to zero active leases.

This record does not claim Anthropic subscription-account support. The
2026-06-11 ToS review remains binding: Anthropic and Gemini subscription
account connects are not offered; provider peers use API-key BYOK only.

## Production Changes

- Stored the provided Gemini key as the Worker `GEMINI_API_KEY` secret for
  `openagents-autopilot` through Wrangler stdin. The key value is not tracked.
- Deployed `openagents-autopilot` Worker version
  `a10d2d08-fd81-4f50-ba01-e06ee90822ed`.
- Applied D1 migration
  `0173_provider_account_peer_provider_checks.sql`, widening provider-account,
  connection-attempt, grant, lease, and sanity-check constraints from
  ChatGPT/Codex-only to `chatgpt_codex`, `anthropic_claude`, and
  `google_gemini`.
- Created the production Gemini BYOK account for `github:14167547`:
  - provider account ref:
    `provider-account_ref_m13_google_gemini_d2fc43560602`
  - secret ref:
    `provider-account://google-gemini/user-api-key/provider-account_ref_m13_google_gemini_d2fc43560602`
  - D1 status: `connected`, `healthy`
  - raw key storage: `AUTH_STORAGE` KV under
    `provider-auth:provider-account_ref_m13_google_gemini_d2fc43560602`

## Live Evidence

Provider pool before lease:

- route: `GET /api/provider-accounts/pool`
- auth: registered-agent bearer
- status: `200`
- account:
  - provider: `google_gemini`
  - eligibility: `eligible`
  - active leases: `0`
  - health/status: `healthy` / `connected`
- generated at: `2026-06-12T03:32:55.146Z`

Lease:

- route: `POST /api/operator/provider-accounts/chatgpt-codex/leases`
- `requiredProvider`: `google_gemini`
- lease ref:
  `provider-account-lease_ref_6de27b1ad36944b382362ca73c23f20a`
- run ref: `run.m13.google_gemini.live.20260612T033255Z`
- assignment ref: `assignment.m13.google_gemini.live.20260612T033255Z`
- selected account:
  `provider-account_ref_m13_google_gemini_d2fc43560602`
- policy: `provider-account-lease-policy:v2`

Grant:

- route: `POST /api/operator/provider-accounts/chatgpt-codex/leases/grant`
- grant ref:
  `provider-auth-grant_grant_ref_19dc435bf3204a70a6b7b853206f357d`
- runner session: `run.m13.google_gemini.live.20260612T033255Z`
- grant status: `issued`

Grant resolution:

- route: `POST /api/provider-accounts/chatgpt-codex/grants/resolve`
- auth: registered-agent bearer
- status: `resolved`
- provider: `google_gemini`
- provider secret ref:
  `provider-account://google-gemini/user-api-key/provider-account_ref_m13_google_gemini_d2fc43560602`
- materialization:
  - kind: `probe_gemini_api_key`
  - target: `GOOGLE_GENERATIVE_AI_API_KEY`
  - home isolation: `per_run`
  - scrub after closeout: `true`

Runner smoke:

- command shape: one-off Probe runtime invocation using the production-resolved
  grant payload from `/tmp/openagents-m13-live-evidence-20260612T033255Z/resolve.json`
- backend kind: `gemini_api`
- completion length: `32`
- provider account ref:
  `provider-account_ref_m13_google_gemini_d2fc43560602`
- grant ref:
  `provider-auth-grant_grant_ref_19dc435bf3204a70a6b7b853206f357d`
- materialization kind: `probe_gemini_api_key`
- target env: `GOOGLE_GENERATIVE_AI_API_KEY`
- secret redaction check: passed

Lease closeout:

- route: `POST /api/operator/provider-accounts/chatgpt-codex/leases/release`
- terminal status: `succeeded`
- terminal outcome: `m13_live_gemini_grant_resolved`
- provider pool after release:
  - active leases for the Gemini account: `0`
  - eligibility: `eligible`
  - last selected at: `2026-06-12T03:32:55.453Z`

## Verification

- `bunx vitest run src/operator-provider-account-routes.test.ts src/provider-account-credential-boundary.test.ts src/provider-account-api-key.test.ts src/provider-account-lease-policy.test.ts src/provider-account-pool-routes.test.ts src/provider-account-gemini-routes.test.ts`
  - 6 files, 57 tests passed.
- `bun run check:architecture`
  - passed.
- `bun run --cwd apps/openagents.com/workers/api typecheck --pretty false`
  - exited successfully; the only remaining output is the upstream
    `nostr-effect` advisory message.
- `bun run --cwd apps/openagents.com/workers/api deploy`
  - deploy checks passed, D1 migrations 0171 and 0172 applied, web build
    succeeded; final Wrangler deploy failed only because Docker was not
    running for unchanged configured Containers.
- `bun run check:deploy`
  - passed after the final provider guard changes.
- `bunx wrangler deploy --containers-rollout=none --env=""`
  - deployed Worker version `a10d2d08-fd81-4f50-ba01-e06ee90822ed`.
- `PROBE_GEMINI_MANAGED_LIVE_SMOKE=1 bun test packages/probe/packages/runtime/tests/managed-gemini-e2e.test.ts`
  - 2 tests passed, including the live Gemini API-key backend call.

## Boundary

This closes #4771's live Gemini acceptance leg. It does not by itself close the
M10 overnight proof, M14 MVP door-open decision, live market paid-labor proof,
settlement visibility, or Pack B hardening issues.
