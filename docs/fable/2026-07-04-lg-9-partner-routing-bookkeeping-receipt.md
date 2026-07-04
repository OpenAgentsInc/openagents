# LG-9 Partner-Org Routing Bookkeeping Receipt

Date: 2026-07-04
Issue: [#8270](https://github.com/OpenAgentsInc/openagents/issues/8270)

LG-9 adds manual partner-route bookkeeping to the BF-9.2 business pipeline
queue. It does not implement the BF-8.5 overflow/peer marketplace green gate,
settlement, payout mechanics, automated routing, public peer signup, or
marketplace copy.

## Shipped Shape

- `business_pipeline_rows` now carries `partnerRoute.state`:
  `none | candidate | offered | accepted | declined`.
- Rows keep the legacy `partnerRouteFlag` compatibility bit so partner-routed
  opportunities remain suppressed from cold outreach drafts.
- Partner-routed rows expose `provenanceLabel: "partner"`; all other rows
  expose `"direct"`.
- Partner-route refs are opaque only:
  `peerRef`, `approvalReceiptRef`, `offerRef`, `scopeSummaryRef`,
  `dueWindowRef`, `budgetRangeRef`, and `privacyTierRef`.
- `offered`, `accepted`, and `declined` require the operator approval receipt
  plus every BF-8.5 offer ref above. The approval receipt is appended to the
  pipeline row's `receiptRefs`.
- Metrics still count quoted partner-routed opportunities in the top-level
  qualified pipeline total and now add `provenanceBreakdown` for `direct` vs
  `partner`.

## Operator Surface

The admin-only route is:

```sh
POST /api/operator/business/pipeline/{pipelineRef}/partner-route
```

The operator CLI wrapper is:

```sh
bun apps/openagents.com/scripts/operator-business-pipeline.ts partner-route \
  --pipeline-ref biz-pipe-YYYYwNN-001 \
  --state offered \
  --peer-ref peer.partner_agency_001 \
  --approval-receipt-ref receipt.operator.partner_route_approval.001 \
  --offer-ref overflow_offer.partner_001 \
  --scope-summary-ref scope_summary.partner.offer_001 \
  --due-window-ref due_window.partner.offer_001 \
  --budget-range-ref budget_range.partner.offer_001 \
  --privacy-tier-ref privacy_tier.standard
```

## Boundary

This is bookkeeping only under the BF-8.5 section 8 gate. The implementation
stores no customer names, raw scope details, peer contract terms, wallet data,
settlement payloads, provider payloads, raw prompts, local paths, or private
customer facts. The only safe product claim is that operator-approved partner
routing can be tracked as pipeline provenance.
