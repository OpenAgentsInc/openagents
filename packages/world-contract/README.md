# @openagentsinc/world-contract

Effect Schema contracts for the Cloudflare Verse World Service.

This package is transport-neutral. It defines the public-safe world rows,
commands, receipts, deltas, subscription plans, diagnostics, bridge payloads,
errors, and read-model projection shapes that both the Worker and clients use.
It intentionally imports no Worker, Durable Object, D1, WebSocket, Three.js,
Foldkit, or backend runtime code.

## P1 Scope

- Shared branded refs, cursors, timestamps, sequence numbers, positions, and
  bounded quantities.
- Public-safe row schemas for region, avatar, pose, pylon station, chat,
  emote, intent, run/entity/edge/proof/settlement/event, projection cursor, and
  bridge health records.
- User and service command envelopes plus typed command receipts.
- Sparse `WorldDelta` frames and WoC-style `WorldReadModel` / `ClientWorld`
  projection schemas.
- Subscription plans with interest hysteresis, near/far tier policy,
  selected-target promotion, and cursor resume fields.
- Tagged errors and helpers for public-safety, avatar refs, character id
  sanitization, region bounds, row keys, and deterministic event refs.

## Projection Bridge Notes

P8 consumes these contracts from `apps/openagents-world/src/bridge.ts`.
`WorldBridgePayload` is intentionally row-shaped and public-safe: bridge code
must replay public source refs into `WorldRow` values, run
`assertWorldPublicSafety`, and key persistence with `worldRowKey`. The contract
does not encode private source payloads, provider traces, raw prompts, or
proof/settlement authority.
