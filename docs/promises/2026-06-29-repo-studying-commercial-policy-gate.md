# Repo Studying Commercial Policy Gate

Date: 2026-06-29
Status: yellow gate narrowing; no green transition
Issue: `#7028`
Promise ids:

- `autopilot.repo_study_packets.v1`
- `autopilot.external_repo_studying_pilot.v1`

## Decision

Repo-studying now has an explicit commercial preflight for marketplace metering,
package/pricing policy, entitlement policy, payout policy, settlement gates, and
refund/dispute policy before any paid-work or package claim can be planned.

This is not a green transition. The preflight is refs-only and inert:
`marketplacePackageAllowed=false`, `payoutEligible=false`,
`settlementReady=false`, and `effectsApplied=false` always.

## Evidence

- `packages/probe/packages/runtime/src/benchmark/external-repo-studying-commercial-policy.ts`
- `packages/probe/packages/runtime/tests/external-repo-studying-commercial-policy.test.ts`
- `docs/promises/2026-06-17-repo-studying-product-promise-gate-review.md`

## Cleared Blockers

- `blocker.product_promises.repo_studying_marketplace_metering_missing`
- `blocker.product_promises.repo_studying_pricing_package_policy_missing`
- `blocker.product_promises.repo_studying_payout_settlement_gates_missing`
- `blocker.product_promises.external_repo_studying_marketplace_metering_missing`
- `blocker.product_promises.external_repo_studying_pricing_package_policy_missing`
- `blocker.product_promises.external_repo_studying_payout_settlement_gates_missing`

## Remaining Blockers

- `blocker.product_promises.repo_studying_privacy_review_missing`
- `blocker.product_promises.repo_studying_product_copy_review_missing`
- `blocker.product_promises.external_repo_studying_privacy_policy_missing`
- `blocker.product_promises.external_repo_studying_self_serve_upload_missing`

## Public Boundary

Public projections stay refs/hashes/lift-only. The commercial preflight admits
only public-safe refs: customer ref, study packet ref, validation ref,
usage-subject ref, metering policy ref, package policy ref, pricing policy ref,
entitlement policy ref, payout policy ref, settlement gate ref, refund/dispute
policy ref, and optional reviewer ref.

It must not include raw private task text, gold answers, rubric claims, private
repo contents, evidence excerpts, raw invoices, wallet material, payment
preimages, local paths, or provider payloads.

## Verification

- `bun test packages/probe/packages/runtime/tests/external-repo-studying-commercial-policy.test.ts`
- `bun test packages/probe/packages/runtime/tests/openagents-customer-private-validation.test.ts packages/probe/packages/runtime/tests/external-repo-studying-pilot-admission.test.ts packages/probe/packages/runtime/tests/external-repo-studying-self-serve-upload.test.ts packages/probe/packages/runtime/tests/external-repo-studying-privacy-review.test.ts`
- `bun test apps/openagents.com/workers/api/src/product-promises.test.ts`
