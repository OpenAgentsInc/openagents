# Provider Capacity Marketplace Gate

> **SUPERSEDED (2026-06-10).** OpenAgents does not resell, rent, proxy, or
> broker subscription or API provider capacity, and no capacity marketplace
> is planned. The fifth revenue stream is the agent labor market:
> contributors use their own compliant provider usage to do useful work and
> sell the accepted output (`provider.compliant_usage_labor.v1`, registry
> 2026-06-10.3). See
> `2026-06-10-five-bitcoin-revenue-streams-promise-audit.md`. This document
> is retained as history only; the matching worker gate module was removed.
>
> **Reconciled (2026-06-13).** The blanket "no API provider capacity resale"
> line above is scoped to *subscription* resale, which remains the non-waivable
> prohibition. Reselling **API inference bought on OpenAgents' own commercial /
> API-key accounts** (the OpenRouter-style gateway, "Model 2" of the cloud
> remote-execution commercial plan) is a normal, allowed business and is now an
> authorized, ref-gated path: see
> `workers/api/src/inference-resale-authorization.ts` and the gate clause in
> `apps/openagents.com/INVARIANTS.md`. This authorizes the mechanism only;
> public marketplace/monetization copy stays gated until the full
> grant→settlement-receipt chain exists. See
> `docs/autopilot-coder/2026-06-13-cloud-remote-execution-commercial-plan.md`.

Implemented: 2026-06-08

Issue: #561

## Summary

Subscription or API-capacity monetization is now represented by an explicit
provider-specific launch gate.

The gate keeps ChatGPT/Codex account connection, provider grant readiness,
route policy, metering, dispatch, pricing, ToS boundaries, assignment receipts,
and Bitcoin settlement receipts separate. It also labels unsupported prepaid providers as
planned or blocked rather than implying support.

## Provider Posture

ChatGPT/Codex is the first modeled provider because OpenAgents product surface already has the
provider-account connection flow. That connection is necessary evidence, but it
is not resale authorization and does not make unused subscription capacity
marketable by itself.

Unsupported prepaid provider capacity remains unsupported in this gate. If no evidence exists, they
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
