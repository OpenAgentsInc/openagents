# RX-10 Reactor Data Liberation Pipeline Receipt

**STATUS (2026-07-08): POSTPONED — parked behind the Khala Code +
business focus (MASTER_ROADMAP rev 6).** Direction retained;
implementation resumes only when MASTER_ROADMAP sequences it or
the owner pulls it forward. Do not route new work from it now.


Date: 2026-07-04
Issue: [#8278](https://github.com/OpenAgentsInc/openagents/issues/8278)
Status: source fixture landed; no customer migration, package copy, or pricing
authority.

RX-10 adds the machinery-only Data Liberation pipeline for Reactor's
walled-garden export/transform/verify quick-win lane. The implementation lives
in `packages/reactor-contracts` and exports:

- `openagents.reactor.data_liberation_adapter_config.v1`
- `openagents.reactor.data_liberation_record_class_verification_receipt.v1`
- `openagents.reactor.data_liberation_pipeline_report.v1`
- `runReactorDataLiberationPipeline`

The pipeline shape is:

1. Per-vendor export adapter config.
2. Schema-mapped transform into an open CRM-style target schema.
3. Customer-controlled store ref for the load destination.
4. Verification receipts per record class with counts, checksums, failed row
   refs, partial row refs, and spot-diff hashes.

Adapters are config, not forks. The same runner handles the generic CSV/API
SaaS fixture, the Salesforce-contact-shaped fixture, and a renamed-column test
mapping.

## Seed Adapters

The seed set is synthetic and public-safe:

- `generic_csv_api_saas_export`: two synthetic contact rows pass.
- `salesforce_contact_export`: three synthetic Salesforce-shaped contact rows
  produce a partial report because one row is missing required `Email`.

The partial report keeps the missing-email row in `failedRowRefs` and emits
`blocker.reactor.data_liberation.pipeline_partial`; it does not silently drop
the row.

Reports explicitly set `customerDataLogged: false`,
`customerEngagementAuthorized: false`, and `packageCopyAuthorized: false`.
Raw fixture values stay in synthetic source rows only; verification reports
carry checksums and spot-diff hashes rather than raw emails or names.

## Verification

The guard runs in two places:

- `packages/reactor-contracts/src/index.test.ts`
- `apps/openagents.com/workers/api/src/reactor-data-liberation.test.ts`

The Worker test is included in `apps/openagents.com` `check:deploy`.

## Boundary

This clears only the source-level Data Liberation adapter/verification blocker.
It does not create a customer migration, customer data-custody proof, customer
contract, public package copy, public pricing, external pilot, compliance
claim, payout, or settlement.
