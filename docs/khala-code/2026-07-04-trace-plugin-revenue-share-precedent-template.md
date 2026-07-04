# Khala Code Trace Plugin Revenue-Share Precedent Template

Status: public-safe operator template for RL-7 / issue #8251.

This template records the first n=1 trace -> plugin -> routed request ->
revenue-share settlement precedent after the owner-run money-moving path has
already produced public-safe receipt refs. The intake route does not move sats
and must not receive raw traces, prompts, invoices, payment hashes, preimages,
payout destinations, wallet material, provider payloads, private source refs, or
raw timestamps.

## Required Evidence

The operator must provide public-safe refs for:

- one consented trace receipt and one redacted trace digest
- one plugin admission receipt and one plugin registry receipt
- one routable plugin ref, digest ref, and route ref
- one routed request ref, exact usage event ref, and idempotency ref
- one contributor attribution ref
- one gross revenue amount and one whole-sat contributor-share amount, both in
  millisats
- one owner amount-envelope ref
- one Spark payout receipt ref and one public settlement receipt ref

This is a plumbing precedent, not market proof. It does not define a future
rate, create a paid-to-free pool, or move any product promise by itself.

## Manual Shape

Save a JSON body like this after the owner has completed and reviewed the live
settlement receipt:

```json
{
  "schemaVersion": "openagents.khala_code.trace_plugin_revenue_share_precedent_intake.v1",
  "consent": {
    "publicReceipt": true,
    "noPrivateDataIncluded": true,
    "realSettlementReceiptSupplied": true
  },
  "consentedTraceReceiptRef": "receipt.khala_code.trace_capture.redacted_001",
  "traceDigestRef": "digest.khala_code.trace.sha256_redacted_001",
  "pluginAdmissionReceiptRef": "receipt.khala_code.plugin_admission.redacted_001",
  "pluginRegistryReceiptRef": "receipt.khala_code.plugin_registry.redacted_001",
  "pluginRef": "plugin.khala_code.trace_derived.redacted_001",
  "pluginDigestRef": "digest.khala_code.plugin.sha256_redacted_001",
  "pluginRouteRef": "route.khala_code.plugin.redacted_001",
  "routedRequestRef": "request.khala_code.plugin.redacted_001",
  "usageEventRef": "usage.khala_code.plugin.redacted_001",
  "usageIdempotencyRef": "idempotency.khala_code.plugin.redacted_001",
  "contributorAttributionRef": "attribution.khala_code.contributor.redacted_001",
  "grossRevenueMsats": 5000,
  "contributorShareMsats": 1000,
  "amountEnvelopeRef": "envelope.khala_code.plugin_revenue_share.one_sat_001",
  "payoutRail": "spark",
  "payoutReceiptRef": "receipt.khala_code.plugin_revenue_share.payout_001",
  "settlementReceiptRef": "settlement.public.khala_code.plugin_revenue_share.one_sat_001",
  "idempotencyKey": "trace-plugin-rs-REPLACE-ME"
}
```

```sh
curl -sS https://openagents.com/api/operator/khala-code/trace-plugin-revenue-share-precedents \
  -H "authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -H 'content-type: application/json' \
  --data @khala-code-trace-plugin-revenue-share.json
```

The response returns `receipt.receiptRef` and `receipt.receiptUrl`. Cite the
public receipt URL:

```sh
curl -sS "https://openagents.com/api/public/khala-code/trace-plugin-revenue-share-precedents/$RECEIPT_REF"
```

Do not cite this as a broad plugin marketplace, live user-payment rate, market
demand proof, paid-to-free pool, or promise-green transition.
