# Provider Capacity Marketplace Gate

Implemented: 2026-06-08

Issue: #561

## Summary

Subscription or API-capacity monetization is now represented by an explicit
provider-specific launch gate.

The gate keeps ChatGPT/Codex account connection, provider grant readiness,
route policy, metering, dispatch, pricing, ToS boundaries, assignment receipts,
and Bitcoin settlement receipts separate. It also labels Claude and Venice as
planned or blocked rather than implying support.

## Provider Posture

ChatGPT/Codex is the first modeled provider because Omega already has the
provider-account connection flow. That connection is necessary evidence, but it
is not resale authorization and does not make unused subscription capacity
marketable by itself.

Claude and Venice remain unsupported in this gate. If no evidence exists, they
project as `planned_unsupported`. If someone attempts to attach grant, metering,
dispatch, assignment, pricing, ToS, or settlement refs before provider-specific
schemas and secret handling exist, they project as `blocked_unsupported`.

## Connector States

Provider dashboards must use the explicit connector-state ladder:

- `unsupported`: the provider has no approved connector model.
- `configured`: the provider has connection or configuration evidence, but not
  enough health and quota evidence for sellable capacity.
- `healthy`: connector health and quota evidence exist, but assignment dispatch
  is not ready.
- `assignable`: the provider can be listed for assignment because account
  schema, secret-ref policy, health, quota, route policy, metering, assignment
  mode, pricing, and ToS boundary refs exist.
- `payable`: an assignment receipt exists, but Bitcoin settlement does not.
- `settled`: the assignment receipt also has settlement receipt refs.

Capacity must not appear as sellable before the `assignable` state. Health
alone is not enough; quota evidence and assignment-mode policy are also
required.

## Required Evidence

For ChatGPT/Codex to reach an end-to-end capacity assignment receipt, the gate
requires public-safe refs for:

- typed account schema;
- secret-ref policy;
- provider grant;
- route policy;
- metering receipt;
- connector health;
- quota evidence;
- assignment mode;
- assignment dispatch;
- pricing policy;
- ToS/product boundary;
- assignment receipt.

This allows the product to say an assignment receipt exists, but it still does
not allow Bitcoin marketplace or settlement copy.

Bitcoin monetization copy requires the same chain plus public settlement
receipt refs for that provider.

## Pricing Boundary

The gate distinguishes `agentic_work` from `base_inference_resale`.

`base_inference_resale` is blocked even if other refs exist. The transcript
promise was about monetizing useful agentic work/products, not laundering a
consumer subscription into raw inference resale.

## Public-Safety Boundary

Public refs must not contain provider-account secrets, tokens, raw quota
payloads, subscription cookies, raw metering, raw pricing, payment material,
wallet material, customer material, private data, or raw timestamps.

## Coverage

Primary regression coverage:

- `workers/api/src/provider-capacity-marketplace-gate.test.ts`

## Remaining Work

This is a gate and projection contract. A live marketplace still needs durable
provider-specific storage, dispatch APIs, metering ingestion, pricing review,
ToS/product policy approval, and settlement receipt joins before public copy can
claim capacity monetization is live.
