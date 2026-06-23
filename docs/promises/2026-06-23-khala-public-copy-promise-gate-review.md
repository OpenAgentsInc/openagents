# Khala Public Copy And Promise Gate Review

Date: 2026-06-23

Issue: OpenAgentsInc/openagents#6111

Scope:

- `apps/openagents.com/apps/web/src/page/loggedOut/page/khala.ts`
- `docs/promises/**`
- `/terms` and `/privacy` legal-review status

This review flips no product-promise state. It records the copy gate for the
public Khala page after the production-readiness, billing/MPP, khala-code
verifier, settlement, benchmark, operator-runbook, telemetry, quantization,
speculation, and Verse-closeout lanes.

## Findings

### `/khala` Public Copy

The page now stays inside the evidence envelope:

- `openagents/khala-mini` and `openagents/khala-code` are described as public
  catalog ids, with availability controlled by gateway readiness and lane
  arming.
- `openagents/khala-code` no longer promises that tests simply pass; it says the
  receipt records whether executable acceptance actually ran, failed, or remains
  unverified.
- `verified:true` is explicitly reserved for an executed acceptance verdict.
- Credits/pricing copy says usage is priced and metered through the receipt
  path, while broad self-serve card, Bitcoin, and MPP funding remain evidence-
  and owner-gated.

### Legal Copy

`/terms` and `/privacy` include visible pending-review notices and source
comments stating that the pages are pending owner/legal review. This satisfies
the public-copy gate for launch posture: the pages may be available, but broad
launch still needs owner/legal sign-off before treating the copy as final.

### Promise States

The relevant registry records remain conservative:

- `inference.gateway_credits_business.v1` stays red because broad paid-credit
  funding still needs owner-armed Stripe/MPP inputs and a dereferenceable real or
  approved staging-to-production paid receipt.
- `inference.free_tier_taste.v1` stays yellow because the free allowance is a
  bounded taste layered on top of a paid loop that is not green.
- `inference.fireworks_open_model_provider.v1` stays yellow because a live supply
  lane and cost basis are not the same as a sellable paid open-model product.
- `inference.decentralized_serving_fabric.v1` stays red because Pylon serving
  runtime, broad payout authority, and first real serving-node payout evidence
  remain owner/compute gated.

No green flip is justified by this review. Any future green transition still
needs exact evidence refs, receipt-first transition proof, and owner sign-off per
`proof.claim_upgrade_receipts.v1`.

## Launch Copy Rule

Until the paid loop is green, public copy may say:

- Khala is an OpenAI-compatible endpoint.
- The gateway exposes public Khala model ids and serves only armed lanes.
- Responses carry OpenAgents receipt disclosure.
- `khala-code` verification is honest and execution-bound.
- Self-serve paid funding and machine-payment flows are gated until proof is
  complete.

Public copy must not say:

- Broad paid Khala is generally launched.
- Any customer can fund inference spend end to end through card, Bitcoin, or MPP.
- `khala-code` is verified unless the receipt contains an executed acceptance
  verdict.
- Pylon contributors are paid from Khala serving without owner-armed settlement
  gates and receipts.
