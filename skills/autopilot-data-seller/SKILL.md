---
name: autopilot-data-seller
description: Conversational seller-authoring policy for Data Market listings in OpenAgents.
metadata:
  oa:
    project: openagents
    identifier: autopilot-data-seller
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - codex:tool-call
      - data-market:seller-authoring
      - data-market:draft-normalization
      - data-market:publish-discipline
---

# Autopilot Data Seller

Use this skill for turns coming from the dedicated `Data Seller` pane.

## Objective

- Gather only the missing facts required to describe a saleable data asset.
- Normalize conversational seller intent into structured draft fields.
- Keep truth aligned with the local draft, preview posture, and published kernel state.
- Never imply a listing is live until preview and explicit confirmation have happened.

## Required Tools

Prefer only these tools for seller flows:

- `openagents.data_market.seller_status`
- `openagents.data_market.draft_asset`
- `openagents.data_market.preview_asset`
- `openagents.data_market.publish_asset`
- `openagents.data_market.draft_grant`
- `openagents.data_market.preview_grant`
- `openagents.data_market.publish_grant`
- `openagents.data_market.prepare_delivery`
- `openagents.data_market.issue_delivery`
- `openagents.data_market.revoke_grant`
- `openagents.data_market.snapshot`

Use generic `openagents.pane.*` tools only for inspection or recovery when a
typed data-market tool cannot provide the needed truth.

## Operating Contract

1. Start from seller truth, not prose confidence.
2. Ask only for missing or contradictory listing facts.
3. Normalize what the seller says into concrete draft fields.
4. Surface readiness blockers clearly and concisely.
5. Preview before publish every time.
6. Require explicit seller confirmation before publish.
7. Read back published state after mutation.
8. For paid targeted requests, prepare delivery explicitly, then issue delivery so kernel truth exists before the NIP-90 result is published.
9. Revoke or expire access only through the typed revocation tool, and only after the seller has stated the intended reason and explicitly confirmed the mutation.

## Safety Rules

- Do not claim a listing is live unless publish succeeded and the published id is visible.
- Do not invent provenance, digest, price, policy, or delivery posture.
- Do not silently widen permissions beyond what the seller confirmed.
- Do not skip preview because the request "sounds obvious."
- Do not claim delivery succeeded unless the `DeliveryBundle` exists and the linked NIP-90 result publish completed.
- Do not claim access has been revoked or expired unless the `RevocationReceipt` exists and the grant/delivery read-back reflects the resulting terminal state.
