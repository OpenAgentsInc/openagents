# Khala Sync Runtime Dogfood Receipt

Date: 2026-07-05
Issue: OpenAgentsInc/openagents#8375
Mode: simulator-only

## Status

Simulator-only evidence is attached. This closes the roadmap issue's minimum
receipt requirement without claiming a physical Tailnet device run or full web
projection parity.

The committed bundle is:

```text
docs/khala-sync/receipts/2026-07-05-runtime-ai-sdk-shaped-dogfood.simulator.json
```

## What It Proves

- A mobile-origin runtime control flow can be represented as public-safe Khala
  Sync evidence using only refs, counts, and latency buckets.
- The evidence shape covers create thread, append user message, runtime control
  intent acceptance, desktop observation without restart, runtime event
  catch-up on mobile after restart/resume, and zero duplicate events after
  resume.
- The validator rejects raw prompts, chat bodies, provider chunks, local paths,
  token-shaped material, and secret-shaped material before evidence can be used
  as a public receipt.

## Gaps Recorded

- `gap.khala_sync.runtime.physical_tailnet_owner_run`: physical owner device
  proof over Tailnet is not attached here.
- `gap.khala_sync.runtime.web_projection_runtime_stream`: web projection parity
  is still recorded as a gap; no claim is made that web consumes the runtime
  event stream yet.

## Verification

```sh
bun run test:khala-sync-runtime-dogfood-evidence
bun scripts/validate-khala-sync-runtime-dogfood-evidence.ts docs/khala-sync/receipts/2026-07-05-runtime-ai-sdk-shaped-dogfood.simulator.json
```
