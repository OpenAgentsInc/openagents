# Workroom Template Package Model

Date: 2026-06-06

Status: implemented contract note for issue #346 / `OPENAGENTS-DEV-002`.

## Purpose

OpenAgents product surface now has a schema-first model for reviewed and versioned workroom
template packages.

The implementation lives in
`workers/api/src/workroom-template-packages.ts`.

This is a package review and projection layer only. It does not promote
packages to runtime, list them publicly in a marketplace, launch external
runners, deploy anything, or mutate payment state.

## Version Model

`WorkroomTemplatePackageVersion` records:

- template version ref;
- outcome template refs;
- required artifact refs;
- approval policy refs;
- runner need refs;
- UI binding refs;
- proof rule refs;
- evidence requirement refs;
- source refs; and
- caveat refs.

The default fixture is `WORKROOM_TEMPLATE_PACKAGE_VERSION_FIXTURE`.

## Package Model

`WorkroomTemplatePackageRecord` records:

- package ref;
- version ref;
- template version refs;
- outcome template refs;
- required artifact refs;
- approval policy refs;
- runner need refs;
- UI binding refs;
- proof rule refs;
- evidence requirement refs;
- validation refs;
- review refs;
- org-private enablement refs;
- public projection refs;
- promotion refs;
- source refs;
- blocker refs;
- caveat refs; and
- operator diagnostic refs.

The state model keeps these steps separate:

- draft;
- validation recorded;
- review recorded;
- org-private enabled;
- public projection ready;
- runtime promotion requested; and
- blocked.

That separation matters because validation is not review, review is not
org-private enablement, org-private enablement is not public projection, public
projection is not runtime promotion, and a runtime promotion request is not
actual runtime promotion.

## Authority Boundary

The default authority block is
`WORKROOM_TEMPLATE_PACKAGE_REVIEW_ONLY_AUTHORITY`.

It explicitly denies:

- runtime promotion;
- marketplace listing;
- external runner launch;
- deployment; and
- payment mutation.

`workroomTemplatePackageAuthorityIsReviewOnly` returns true only for packages
using that full deny block.

## Projection And Redaction

Public and agent projections hide package source refs, review refs, promotion
refs, org-private enablement refs, and operator diagnostics.

The contract rejects refs containing:

- private package source;
- raw prompts;
- raw package payloads;
- provider payloads, grants, accounts, or tokens;
- private repo refs;
- raw schemas, fixtures, source archives, runner logs, or documents;
- secrets, bearer tokens, OAuth material, cookies, and API keys;
- wallet/payment material; and
- raw timestamps.

Projection times use friendly labels instead of raw timestamps.

## Tests

`workers/api/src/workroom-template-packages.test.ts` covers:

- schema/projection decoding;
- friendly time projection;
- package version source redaction;
- package record source/review/promotion/enablement/diagnostic redaction;
- validation, review, org-private enablement, public projection, and runtime
  promotion request separation;
- hard false runtime promotion, marketplace listing, external runner launch,
  deployment, and payment mutation authority flags; and
- unsafe package/source/provider/private repo/material rejection.
