# Targeted Site Discovery Planner

Implemented: 2026-06-05

Issue: #182 / OPENAGENTS-SITES-OUTREACH-002

## Summary

OpenAgents product surface now has an Exa-backed discovery planner for targeted Site remake and
outreach campaigns.

The planner is a typed adapter boundary. It builds bounded Exa company-search
requests from campaign criteria, normalizes results into public-safe source
cards, dedupes targets by normalized domain, and can persist candidate
prospects through the targeted outreach repository added in issue #181.

## Service Boundary

`workers/api/src/targeted-site-discovery-planner.ts` exposes:

- `buildTargetedSiteDiscoveryPlan`;
- `sourceCardsFromExaResults`;
- `runTargetedSiteDiscoveryPlan`.

`runTargetedSiteDiscoveryPlan` accepts an `ExaClientShape`, so tests and future
operator flows can supply a fake adapter without making live Exa calls.

## Plan Inputs

The planner supports:

- campaign id;
- vertical;
- geography;
- website-quality signals;
- max result caps;
- dry-run mode;
- source run ref;
- prospect idempotency key prefix;
- optional include/exclude domain filters.

Result count is clamped to 1 through 25. Dry-run mode is the default.

## Source Cards

Each source card contains:

- campaign id;
- source run ref;
- result URL;
- normalized domain;
- title and snippet when public-safe;
- confidence;
- source ref;
- prospect idempotency key.

Cards are deduped by normalized domain before persistence.

## Persistence

When `dryRun` is false, source cards are persisted with
`upsertTargetedSiteProspect`. The repository remains the authority for domain
normalization, idempotency, campaign-domain dedupe, suppression state, and
public-safe metadata.

## Safety Rules

The planner does not store raw Exa payloads.

It drops or redacts result fields that contain:

- raw contact data;
- provider payload markers;
- OAuth or provider-account material;
- wallet, payment, Lightning, or MDK secrets;
- private operator notes.

Exa API keys remain inside the existing `ExaClient` boundary and never appear
in source cards, prospects, docs, or customer-safe projections.

## Remaining Work

Issue #183 adds the respectful capture policy gate that decides whether a
persisted prospect may be fetched at all.
