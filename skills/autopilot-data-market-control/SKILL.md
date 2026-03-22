---
name: autopilot-data-market-control
description: Typed OpenAgents DS-first Data Market tool contract for seller publication and market read-back.
metadata:
  oa:
    project: openagents
    identifier: autopilot-data-market-control
    version: "0.2.0"
    expires_at_unix: 1798761600
    capabilities:
      - codex:tool-call
      - data-market:tool-control
      - data-market:relay-readback
      - data-market:publish-discipline
---

# Autopilot Data Market Control

Use this skill when working inside the `Data Seller` pane or when inspecting the
relay-backed Data Market state the app maintains locally.

## Tool Contract

Use the typed OpenAgents data-market tools in the normal order:

1. `openagents.data_market.seller_status`
2. `openagents.data_market.draft_asset`
3. `openagents.data_market.preview_asset`
4. `openagents.data_market.publish_asset`
5. `openagents.data_market.draft_grant`
6. `openagents.data_market.preview_grant`
7. `openagents.data_market.publish_grant`
8. `openagents.data_market.snapshot`

## Operating Rules

1. Treat DS listings, DS offers, DS access contracts, and DS-DVM events as the
   public market truth.
2. Treat seller drafts and previews as pre-publication state, not market truth.
3. Use preview responses to explain blockers before asking for more detail.
4. Publish only after the pane records explicit confirmation state.
5. After any mutation, fetch the relay-backed snapshot or seller status before
   summarizing outcome.
6. Treat DS-DVM request/result traffic as fulfillment state, not as the
   catalog/publication layer.

## Forbidden Shortcuts

- Do not mark a listing or grant as published based on prose alone.
- Do not bypass typed data-market tools with generic pane-input poking for core publication flows.
- Do not present local preview payloads as canonical published DS objects.
