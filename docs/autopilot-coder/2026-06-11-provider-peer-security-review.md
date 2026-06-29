# Provider Peer Security Review

Date: 2026-06-11

## Scope

This review is the Pack B security-review record for provider-peer account
expansion. It covers ChatGPT/Codex, Anthropic Claude, and Google Gemini
provider accounts in the Autopilot account-pool and lease-selection path.

Inputs:

- `docs/autopilot-coder/2026-06-11-provider-peer-tos-compliance-review.md`
- #4771 provider-peer issue thread.
- #4824 Pack B parent.
- #4825 PB1 credential-boundary implementation.
- #4826 PB2 effective config snapshot implementation.
- The provider-account invariant ledger in `apps/openagents.com/INVARIANTS.md`.

This review does not claim that a live Anthropic or Gemini Autopilot run has
completed. #4771 remains the issue that owns that live-readiness proof.

## Decision

Provider-peer expansion may proceed only through BYOK API-key provider accounts
for Anthropic Claude and Google Gemini, and only when the lane cites the typed
security gate implemented in
`apps/openagents.com/workers/api/src/provider-account-security-review.ts`.

Subscription-account capture, cookie replay, OAuth-token capture, Claude.ai /
Pro / Max login, Google account OAuth, Code Assist account reuse, and AI
Pro/Ultra subscription routing remain forbidden unless a later dated review
supersedes the ToS review and adds tests.

## Required Refs Before Broad Provider-Peer Closure

Each provider-peer closeout needs:

- ToS review ref.
- Credential-boundary ref.
- Threat-model ref.
- Telemetry/privacy ref.
- Retention-policy ref.
- Redaction fixture refs.
- Revocation/stale-lease fixture refs.
- High-risk approval, denial, rollback, incident-boundary, and debug-boundary
  refs when the provider flow can mutate account state or broaden background
  execution.

An explicit scoped exception may let a narrow pre-existing slice continue, but
it must keep its blocker refs visible and cannot be used as broad provider-peer
readiness.

## Public And Agent-Readable Boundaries

Provider/account security projections are evidence only. They may expose refs,
provider ids, scope, status, blockers, and caveats. They must not expose raw
API keys, OAuth material, provider responses, raw prompts, private repo data,
raw shell output, local paths, customer-private data, or wallet/payment
material.

## M13 Timing

#4771 should not close on ToS prose alone. It needs either:

- approved Pack B security-review refs plus the remaining live non-Codex run
  evidence, or
- a scoped exception that documents exactly which provider-peer claim remains
  blocked.
