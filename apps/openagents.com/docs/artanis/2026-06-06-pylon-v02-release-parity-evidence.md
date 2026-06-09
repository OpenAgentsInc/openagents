# Artanis Pylon v0.2 Release-Parity Evidence

Date: 2026-06-06 America/Chicago, 2026-06-07 UTC evidence window

Issue: #419 / `ARTANIS-033`

Status: implemented as a public-safe release-parity projection and report
blocker.

## Purpose

Pylon v0.2 source support is not the same as a shipped Pylon v0.2 release.

Artanis needs a typed way to keep those claims separate in the public report:

- source-level LDK-compatible payout-target support;
- release tag and package version;
- platform assets and smokes;
- runtime smoke;
- eligibility telemetry;
- payment target registration;
- accepted-work proof;
- paid-work receipts;
- settlement receipts.

## Current Finding

Current evidence keeps Pylon v0.2 release parity blocked:

- latest public release found: `pylon-v0.1.23`;
- missing release: `pylon-v0.2.0`;
- package version evidence still points to `0.1.23`;
- source-level v0.2 payout-target support exists;
- v0.2 release assets are not retained;
- runtime smoke for a v0.2 release artifact is not retained;
- Linux, WSL Ubuntu, and native Windows platform smokes are not retained;
- eligibility telemetry and payment target registration refs are not retained
  as release-parity proof;
- accepted-work, paid-work, and settlement receipt chains are not retained.

## Implementation

Code lives in:

- `workers/api/src/artanis-pylon-v02-release-parity.ts`
- `workers/api/src/artanis-pylon-v02-release-parity.test.ts`
- `workers/api/src/artanis-public-report.ts`
- `workers/api/src/artanis-public-report.test.ts`

The public report now includes `pylonReleaseParity` with:

- `sourceLevelSupportVisible`;
- `releaseReady`;
- `packageVersionMatched`;
- `platformReady`;
- `eligibilityReady`;
- `acceptedWorkClaimAllowed`;
- `paidClaimAllowed`;
- `settledClaimAllowed`;
- `shippedClaimAllowed`;
- `generalAvailabilityClaimAllowed`;
- public blocker refs;
- public stage-summary refs.

## Required Release-Parity Gates

| Gate | Required evidence |
| --- | --- |
| Source support | Public source/docs refs for the LDK-compatible v0.2 payout-target contract. |
| Release assets | `pylon-v0.2.0` tag plus retained macOS Apple Silicon, Linux, WSL Ubuntu, and native Windows assets/checksums. |
| Package version | Package version refs matching `0.2.0`. |
| Runtime smoke | First-boot/runtime smoke on the v0.2 release artifact. |
| Platform smoke | Retained platform smoke refs for Linux, macOS Apple Silicon, WSL Ubuntu, and native Windows. |
| Eligibility | Eligibility telemetry plus LDK-compatible payment target registration refs. |
| Accepted work | Public accepted-work proof refs. |
| Paid work | Public paid-work receipt refs. |
| Settlement | Public settlement receipt refs. |

## Public Claim Rules

Allowed now:

```text
Pylon has source-level support for the v0.2 LDK-compatible payout-target
contract.
```

Blocked now:

```text
Pylon v0.2 is shipped.
Pylon v0.2 is ready for everyone.
Pylon v0.2 work has been accepted, paid, or settled.
```

The public report must expose blocked-claim refs, not the literal false public
copy. #419 also tightens the production launch-gate projection so the public
report no longer serializes the exact false phrase `Pylon v0.2 is shipped`.

## Redaction Boundary

The projection rejects:

- raw payout targets;
- wallet or payment material;
- provider secrets;
- raw release command output;
- private node telemetry;
- private customer material;
- raw timestamps;
- private evidence refs on public projections.

Operator/private projections may retain safe evidence refs by reference only.
They still cannot use this release-parity projection as authority to publish
releases, mutate providers, spend wallet funds, dispatch payouts, or upgrade
public claims.

## Verification

Coverage lives in:

- `workers/api/src/artanis-pylon-v02-release-parity.test.ts`
- `workers/api/src/artanis-public-report.test.ts`
- `workers/api/src/artanis-production-launch-gate.test.ts`

Tests cover:

- no-release evidence;
- source-only support;
- release tag without required assets;
- package version mismatch;
- missing platform smokes;
- missing eligibility telemetry and payment target registration;
- missing accepted-work proof;
- missing paid and settled receipts;
- fully release-ready modeled evidence;
- public redaction of private/operator refs;
- rejection of raw payout target, wallet, provider, release command, private
  telemetry, and timestamp material;
- public report blocking literal shipped/ready-for-everyone false copy.
